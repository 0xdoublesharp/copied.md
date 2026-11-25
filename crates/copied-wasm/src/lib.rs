use copied_core::{
    ConversionOptions, MarkdownConversion, TARGETS, convert_markdown, resolve_target,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Lists the copy targets as a JSON string.
#[wasm_bindgen]
pub fn list_targets_json() -> String {
    serde_json::to_string(TARGETS).expect("the target catalog always serializes")
}

/// Resolves a target ID or legacy alias to its canonical ID.
#[wasm_bindgen]
pub fn resolve_target_id(value: &str) -> Option<String> {
    resolve_target(value).map(|target| target.id().to_owned())
}

/// Converts Markdown for a target ID or alias and returns the conversion as a JSON string.
#[wasm_bindgen]
pub fn convert_markdown_json(
    markdown: &str,
    target: &str,
    source_url: Option<String>,
    decorative_unicode: bool,
) -> Result<String, JsValue> {
    let conversion = convert(markdown, target, source_url, decorative_unicode)
        .map_err(|error| JsValue::from_str(&error))?;
    serde_json::to_string(&conversion).map_err(|error| JsValue::from_str(&error.to_string()))
}

/// Lists the copy targets as a JavaScript array.
#[wasm_bindgen]
pub fn list_targets() -> Result<JsValue, JsValue> {
    serialize_js(&TARGETS)
}

/// Converts Markdown for a target ID or alias and returns a JavaScript object.
#[wasm_bindgen]
pub fn convert_markdown_js(
    markdown: &str,
    target: &str,
    source_url: Option<String>,
    decorative_unicode: bool,
) -> Result<JsValue, JsValue> {
    let conversion = convert(markdown, target, source_url, decorative_unicode)
        .map_err(|error| JsValue::from_str(&error))?;
    serialize_js(&conversion)
}

fn convert(
    markdown: &str,
    target: &str,
    source_url: Option<String>,
    decorative_unicode: bool,
) -> Result<MarkdownConversion, String> {
    let target = resolve_target(target).ok_or_else(|| format!("Unknown copy target: {target}"))?;
    Ok(convert_markdown(
        markdown,
        target,
        &ConversionOptions {
            source_url,
            decorative_unicode,
        },
    ))
}

fn serialize_js(value: &impl Serialize) -> Result<JsValue, JsValue> {
    // Use plain JS objects for maps and match the omitted fields in the JSON export.
    value
        .serialize(&serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use copied_core::CopyTarget;

    #[test]
    fn catalog_json_contains_all_targets_in_order() {
        let json: serde_json::Value = serde_json::from_str(&list_targets_json()).unwrap();
        assert_eq!(json, serde_json::to_value(TARGETS).unwrap());
        assert_eq!(json.as_array().unwrap().len(), 15);
    }

    #[test]
    fn target_resolution_supports_the_catalog_and_legacy_aliases() {
        for target in TARGETS {
            assert_eq!(
                resolve_target_id(target.id.id()).as_deref(),
                Some(target.id.id())
            );
        }
        assert_eq!(resolve_target_id("slack").as_deref(), Some("slack-paste"));
        assert_eq!(resolve_target_id("html").as_deref(), Some("rich-text"));
        assert_eq!(resolve_target_id("wrong"), None);
    }

    #[test]
    fn conversion_forwards_options_and_keeps_the_core_json_contract() {
        let markdown = "**😀** [Notes](./notes)";
        let url = "https://example.org/posts/a".to_owned();
        let options = ConversionOptions {
            source_url: Some(url.clone()),
            decorative_unicode: true,
        };
        let expected = convert_markdown(markdown, CopyTarget::SlackPaste, &options);
        assert_eq!(
            convert(markdown, "slack", Some(url.clone()), true).unwrap(),
            expected
        );
        let json: serde_json::Value = serde_json::from_str(
            &convert_markdown_json(markdown, "slack", Some(url), true).unwrap(),
        )
        .unwrap();
        assert_eq!(json, serde_json::to_value(expected).unwrap());
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["target"], "slack-paste");
        assert_eq!(
            json["metrics"]["sourceCodeUnits"].as_u64().unwrap(),
            u64::try_from(markdown.encode_utf16().count()).unwrap()
        );
    }

    #[test]
    fn unknown_target_is_a_native_error_before_js_conversion() {
        assert_eq!(
            convert("hello", "wrong", None, false).unwrap_err(),
            "Unknown copy target: wrong"
        );
    }
}
