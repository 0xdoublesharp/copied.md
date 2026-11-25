//! Newline-delimited JSON-RPC 2.0 transport for the conversion tools.

use copied_core::{ConversionOptions, CopyTarget, TARGETS, convert_markdown, resolve_target};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::io::{self, BufRead, Write};

use crate::cli::write_json;

const PROTOCOL_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];
const LIST: &str = "list_copy_targets";
const CONVERT: &str = "convert_markdown";

/// Serves requests on a pair of streams, flushing each response before reading the next request.
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut line = Vec::new();
    while input.read_until(b'\n', &mut line)? != 0 {
        if !line.iter().all(u8::is_ascii_whitespace)
            && let Some(response) = handle_line(&line)
        {
            write_json(&mut output, &response)?;
            output.flush()?;
        }
        line.clear();
    }
    Ok(())
}

fn handle_line(line: &[u8]) -> Option<Value> {
    let Ok(request) = serde_json::from_slice::<Value>(line) else {
        return Some(rpc_error(&Value::Null, -32700, "Parse error"));
    };
    let id = request.get("id");
    let valid_id = id.is_none_or(|value| {
        value.is_string() || value.as_i64().is_some() || value.as_u64().is_some()
    });
    let method = request.get("method").and_then(Value::as_str);
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !valid_id
        || method.is_none()
    {
        return Some(rpc_error(&Value::Null, -32600, "Invalid request"));
    }
    let id = id?;
    let method = method.expect("method was checked");
    Some(match dispatch(method, request.get("params")) {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(error) => rpc_error(id, error.code, &error.message),
    })
}

fn rpc_error(id: &Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

#[derive(Debug)]
struct RpcError {
    code: i32,
    message: String,
}

impl RpcError {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }
}

fn dispatch(method: &str, params: Option<&Value>) -> Result<Value, RpcError> {
    match method {
        "initialize" => {
            let protocol = params
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RpcError::invalid_params("initialize requires a protocolVersion string")
                })?;
            let protocol = if PROTOCOL_VERSIONS.contains(&protocol) {
                protocol
            } else {
                PROTOCOL_VERSIONS[0]
            };
            Ok(json!({
                "protocolVersion": protocol,
                "capabilities": {"tools": {}},
                "serverInfo": {"name":"copied-md","version":env!("CARGO_PKG_VERSION")},
            }))
        }
        "ping" | "tools/list" => {
            if params.is_some_and(|p| !p.is_object()) {
                return Err(RpcError::invalid_params("params must be an object"));
            }
            Ok(if method == "ping" {
                json!({})
            } else {
                tool_list()
            })
        }
        "tools/call" => call_tool(params),
        _ => Err(RpcError {
            code: -32601,
            message: format!("Method not found: {method}"),
        }),
    }
}

fn tool_list() -> Value {
    let annotations = json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false,
    });
    json!({"tools": [
        {
            "name": LIST,
            "description": "List the available Markdown conversion targets.",
            "inputSchema": {"type":"object","properties":{}},
            "annotations": annotations,
        }, {
            "name": CONVERT,
            "description": "Convert supplied Markdown into clipboard representations for a chosen destination.",
            "inputSchema": {
                "$schema":"http://json-schema.org/draft-07/schema#",
                "type":"object",
                "properties":{
                    "markdown":{"type":"string"},
                    "target":{"type":"string","enum":TARGETS.iter().map(|def| def.id.id()).collect::<Vec<_>>()},
                    "sourceUrl":{"type":"string","format":"uri"},
                    "decorativeUnicode":{"type":"boolean"},
                },
                "required":["markdown","target"],
            },
            "annotations": annotations,
        }
    ]})
}

fn call_tool(params: Option<&Value>) -> Result<Value, RpcError> {
    let params = params
        .and_then(Value::as_object)
        .ok_or_else(|| RpcError::invalid_params("params must be an object"))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("name must be a string"))?;
    let arguments = params
        .get("arguments")
        .map(|args| {
            args.as_object()
                .ok_or_else(|| RpcError::invalid_params("arguments must be an object"))
        })
        .transpose()?;
    match name {
        LIST => tool_result(&json!({"targets": TARGETS})),
        CONVERT => match conversion_arguments(arguments) {
            Ok((markdown, target, options)) => {
                tool_result(&convert_markdown(markdown, target, &options))
            }
            Err(message) => Ok(json!({
                "content":[{"type":"text","text":message}],
                "isError":true,
            })),
        },
        name => Err(RpcError::invalid_params(format!("Unknown tool: {name}"))),
    }
}

fn conversion_arguments(
    arguments: Option<&Map<String, Value>>,
) -> Result<(&str, CopyTarget, ConversionOptions), &'static str> {
    let params = arguments.ok_or("convert_markdown requires arguments")?;
    let markdown = params
        .get("markdown")
        .and_then(Value::as_str)
        .ok_or("markdown must be a string")?;
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .and_then(resolve_target)
        .ok_or("target must be a valid copy target")?;
    let source_url = params
        .get("sourceUrl")
        .map(|value| {
            value
                .as_str()
                .filter(|value| url::Url::parse(value).is_ok())
                .map(str::to_owned)
                .ok_or("sourceUrl must be a valid URL")
        })
        .transpose()?;
    let decorative_unicode = params.get("decorativeUnicode").map_or(Ok(false), |value| {
        value.as_bool().ok_or("decorativeUnicode must be a boolean")
    })?;
    Ok((
        markdown,
        target,
        ConversionOptions {
            source_url,
            decorative_unicode,
        },
    ))
}

fn tool_result(value: &impl Serialize) -> Result<Value, RpcError> {
    let internal = |error: serde_json::Error| RpcError {
        code: -32603,
        message: error.to_string(),
    };
    // Serialize the result to a Value once, then use the same fields for both representations.
    let value = serde_json::to_value(value).map_err(internal)?;
    let text = serde_json::to_string(&value).map_err(internal)?;
    Ok(json!({
        "content":[{"type":"text","text":text}],
        "structuredContent":value,
    }))
}

#[cfg(test)]
mod tests;
