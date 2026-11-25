use copied_core::{ConversionOptions, TARGETS, convert_markdown, resolve_target};
use wasm_bindgen::prelude::*;

/// Lists all 15 copy targets as a JSON string.
#[wasm_bindgen]
pub fn list_targets_json() -> String {
    serde_json::to_string(TARGETS).unwrap_or_else(|_| String::from("[]"))
}

/// Converts Markdown for a target ID (or legacy alias) and returns `MarkdownConversion` JSON.
#[wasm_bindgen]
pub fn convert_markdown_json(
    markdown: &str,
    target: &str,
    source_url: Option<String>,
    decorative_unicode: bool,
) -> Result<String, JsValue> {
    let target = resolve_target(target)
        .ok_or_else(|| JsValue::from_str(&format!("Unknown copy target: {target}")))?;
    let options = ConversionOptions {
        source_url,
        decorative_unicode,
    };
    let result = convert_markdown(markdown, target, &options);
    serde_json::to_string(&result).map_err(|err| JsValue::from_str(&err.to_string()))
}
