use super::*;
use copied_core::CopyTarget;
use std::{cell::Cell, io::Cursor};

fn request(id: impl Serialize, method: &str, params: Value) -> Value {
    let mut message = json!({"jsonrpc":"2.0","id":id,"method":method});
    message["params"] = params;
    message
}

fn response(request: &Value) -> Value {
    let mut output = Vec::new();
    serve(Cursor::new(request.to_string()), &mut output).unwrap();
    serde_json::from_slice(&output).unwrap()
}

fn call(name: &str, arguments: Value) -> Value {
    let mut params = json!({"name":name});
    params["arguments"] = arguments;
    response(&request("call", "tools/call", params))
}

fn check_content(result: &Value, expected: &Value) {
    assert_eq!(&result["structuredContent"], expected);
    let text = serde_json::to_string(expected).unwrap();
    assert_eq!(result["content"], json!([{"type":"text","text":text}]));
}

#[test]
fn serves_initialization_ping_and_notifications_without_protocol_noise() {
    let input = concat!(
        "\n",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\r\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"unknown\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"ping\",\"method\":\"ping\"}"
    );
    let flushes = Cell::new(0);
    let mut output = Vec::new();
    serve(
        Cursor::new(input),
        CountingWriter {
            output: &mut output,
            flushes: &flushes,
        },
    )
    .unwrap();
    assert_eq!(flushes.get(), 2);
    let responses: Vec<Value> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(
        responses,
        [
            json!({"jsonrpc":"2.0","id":1,"result":{
                "protocolVersion":"2025-06-18", "capabilities":{"tools":{}},
                "serverInfo":{"name":"copied-md","version":env!("CARGO_PKG_VERSION")},
            }}),
            json!({"jsonrpc":"2.0","id":"ping","result":{}}),
        ]
    );

    let future = response(&request(
        1,
        "initialize",
        json!({"protocolVersion":"2099-12-31"}),
    ));
    assert_eq!(future["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
}

struct CountingWriter<'a> {
    output: &'a mut Vec<u8>,
    flushes: &'a Cell<usize>,
}

impl Write for CountingWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes.set(self.flushes.get() + 1);
        Ok(())
    }
}

#[test]
fn tools_list_preserves_the_schemas_and_read_only_annotations() {
    let body = response(&request(2, "tools/list", json!({})));
    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "list_copy_targets");
    assert_eq!(
        tools[0]["inputSchema"],
        json!({"type":"object","properties":{}})
    );
    assert_eq!(tools[1]["name"], "convert_markdown");
    assert_eq!(
        tools[1]["inputSchema"],
        json!({
            "$schema":"http://json-schema.org/draft-07/schema#",
            "type":"object",
            "properties":{
                "markdown":{"type":"string"},
                "target":{"type":"string","enum":TARGETS.iter().map(|target| target.id.id()).collect::<Vec<_>>()},
                "sourceUrl":{"type":"string","format":"uri"},
                "decorativeUnicode":{"type":"boolean"},
            },
            "required":["markdown","target"],
        })
    );
    for tool in tools {
        assert_eq!(
            tool["annotations"],
            json!({
                "readOnlyHint":true, "destructiveHint":false, "idempotentHint":true, "openWorldHint":false,
            })
        );
    }
}

#[test]
fn list_tool_returns_identical_text_and_structured_content() {
    let response = response(&request(
        3,
        "tools/call",
        json!({"name":"list_copy_targets"}),
    ));
    check_content(&response["result"], &json!({"targets":TARGETS}));
    let response = call("list_copy_targets", json!({}));
    check_content(&response["result"], &json!({"targets":TARGETS}));
}

#[test]
fn convert_tool_matches_core_and_accepts_aliases() {
    let input = "[Notes](./notes)";
    let url = "https://example.org/posts/a";
    let args =
        json!({"markdown":input,"target":"telegram","sourceUrl":url,"decorativeUnicode":false});
    let response = call("convert_markdown", args);
    let expected = convert_markdown(
        input,
        CopyTarget::Telegram,
        &ConversionOptions {
            source_url: Some(url.into()),
            decorative_unicode: false,
        },
    );
    check_content(&response["result"], &json!(expected));
    assert_eq!(response["result"]["structuredContent"]["schemaVersion"], 1);
    assert_eq!(
        response["result"]["structuredContent"]["target"],
        "telegram"
    );

    let alias = call(
        "convert_markdown",
        json!({"markdown":"hi","target":"slack"}),
    );
    check_content(
        &alias["result"],
        &json!(convert_markdown(
            "hi",
            CopyTarget::SlackPaste,
            &ConversionOptions::default()
        )),
    );
}

#[test]
fn invalid_conversion_arguments_return_actionable_tool_errors() {
    for args in [
        json!({}),
        json!({"markdown":"# Hi","target":"unknown"}),
        json!({"markdown":false,"target":"telegram"}),
        json!({"markdown":"# Hi","target":false}),
        json!({"markdown":"# Hi","target":"telegram","sourceUrl":"/relative"}),
        json!({"markdown":"# Hi","target":"telegram","sourceUrl":"https://["}),
        json!({"markdown":"# Hi","target":"telegram","sourceUrl":null}),
        json!({"markdown":"# Hi","target":"telegram","decorativeUnicode":"true"}),
        json!({"markdown":"# Hi","target":"telegram","decorativeUnicode":null}),
    ] {
        let response = call("convert_markdown", args);
        assert_eq!(response["id"], "call");
        assert_eq!(response["result"]["isError"], true, "{response}");
        let result = &response["result"];
        assert!(result.get("structuredContent").is_none());
        assert_eq!(result["content"][0]["type"], "text");
        assert_ne!(result["content"][0]["text"].as_str().unwrap(), "");
    }
    let missing = response(&request(
        1,
        "tools/call",
        json!({"name":"convert_markdown"}),
    ));
    assert_eq!(missing["result"]["isError"], true);
}

#[test]
fn malformed_call_params_and_unknown_tools_return_invalid_params() {
    for (name, args) in [
        ("unknown", json!({})),
        ("list_copy_targets", json!(null)),
        ("convert_markdown", json!(null)),
        ("convert_markdown", json!([])),
    ] {
        let result = call(name, args);
        assert_eq!(result["error"]["code"], -32602);
        assert!(result.get("result").is_none());
    }
    for params in [json!({}), json!(null), json!({"name":3})] {
        let result = response(&request(1, "tools/call", params));
        assert_eq!(result["error"]["code"], -32602);
    }
}

#[test]
fn malformed_lines_and_unknown_methods_do_not_end_the_stream() {
    let input = concat!(
        "(broken JSON)\n",
        "{\"jsonrpc\":\"1.0\",\"id\":1,\"method\":\"ping\"}\n",
        "[]\n",
        "{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"ping\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":4}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"missing\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"initialize\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/list\",\"params\":[] }\n",
        "{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"ping\"}\n",
    );
    let mut output = Vec::new();
    serve(Cursor::new(input), &mut output).unwrap();
    let responses: Vec<Value> = BufRead::lines(Cursor::new(output))
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
        .collect();
    assert_eq!(responses.len(), 9);
    for (response, code) in responses.iter().zip([
        -32700, -32600, -32600, -32600, -32600, -32601, -32602, -32602,
    ]) {
        assert_eq!(response["error"]["code"], code);
    }
    assert_eq!(responses[0]["id"], Value::Null);
    assert_eq!(responses[5]["id"], 5);
    assert_eq!(responses[8], json!({"jsonrpc":"2.0","id":8,"result":{}}));
}

#[test]
fn serialization_failure_becomes_internal_error() {
    struct CannotSerialize;
    impl Serialize for CannotSerialize {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("serialization failed"))
        }
    }
    let error = tool_result(&CannotSerialize).unwrap_err();
    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "serialization failed");
}
