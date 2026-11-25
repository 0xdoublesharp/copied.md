//! Shared helpers for integration tests.
#![allow(dead_code)]

use copied_core::{ConversionOptions, MarkdownConversion};
use serde_json::Value;

/// Golden corpus case.
#[derive(Debug, Clone)]
pub struct GoldenRun {
    pub case: String,
    pub markdown: String,
    pub option_set: String,
    pub options: ConversionOptions,
    pub results: Vec<(String, Value)>,
}

pub fn corpus() -> (String, Vec<GoldenRun>) {
    let raw = include_str!("../golden/corpus.json");
    let value: Value = serde_json::from_str(raw).expect("corpus.json parses");
    let tag = value["oracleTag"].as_str().unwrap_or_default().to_owned();
    let mut runs = Vec::new();
    for case in value["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name").to_owned();
        let markdown = case["markdown"].as_str().expect("markdown").to_owned();
        for run in case["runs"].as_array().expect("runs") {
            let options: ConversionOptions = serde_json::from_value(run["options"].clone()).expect("options");
            let results = run["results"]
                .as_object()
                .expect("results")
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            runs.push(GoldenRun {
                case: name.clone(),
                markdown: markdown.clone(),
                option_set: run["optionSet"].as_str().expect("optionSet").to_owned(),
                options,
                results,
            });
        }
    }
    (tag, runs)
}

pub fn to_value(conversion: &MarkdownConversion) -> Value {
    serde_json::to_value(conversion).expect("serializes")
}

/// One `[[divergence]]` entry from `tests/parity/divergences.toml`.
#[derive(Debug, Clone, Default)]
pub struct Divergence {
    pub id: String,
    pub title: String,
    pub rationale: String,
    pub targets: Vec<String>,
    pub fields: Vec<String>,
    pub cases: Vec<String>,
}

fn parse_string(raw: &str, line: usize) -> String {
    let inner = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or_else(|| panic!("line {line}: expected a string"));
    assert!(!inner.contains('\\'), "line {line}: escapes are not supported");
    inner.to_owned()
}

/// Parses the small TOML subset used by `divergences.toml`; unknown keys panic.
pub fn divergences() -> Vec<Divergence> {
    let source = include_str!("../parity/divergences.toml");
    let mut entries: Vec<Divergence> = Vec::new();
    let mut lines = source.lines().enumerate().peekable();
    while let Some((number, line)) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[divergence]]" {
            entries.push(Divergence::default());
            continue;
        }
        let (key, value) = line.split_once('=').unwrap_or_else(|| panic!("line {}: expected key = value", number + 1));
        let (key, value) = (key.trim(), value.trim());
        let entry = entries.last_mut().unwrap_or_else(|| panic!("line {}: key outside [[divergence]]", number + 1));
        let text = if value == "\"\"\"" {
            let mut collected = Vec::new();
            loop {
                let (_, next) = lines.next().unwrap_or_else(|| panic!("line {}: unterminated string", number + 1));
                if next.trim() == "\"\"\"" {
                    break;
                }
                collected.push(next.trim());
            }
            Some(collected.join(" "))
        } else if value.starts_with('"') {
            Some(parse_string(value, number + 1))
        } else {
            None
        };
        let list = || -> Vec<String> {
            let inner = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')).unwrap_or_else(|| panic!("line {}: expected an array", number + 1));
            inner.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| parse_string(s, number + 1)).collect()
        };
        match key {
            "id" => entry.id = text.expect("id is a string"),
            "title" => entry.title = text.expect("title is a string"),
            "rationale" => entry.rationale = text.expect("rationale is a string"),
            "targets" => entry.targets = list(),
            "fields" => entry.fields = list(),
            "cases" => entry.cases = list(),
            other => panic!("line {}: unknown key `{other}`", number + 1),
        }
    }
    for entry in &entries {
        assert!(!entry.id.is_empty() && !entry.title.is_empty() && entry.rationale.len() > 40, "incomplete entry {entry:?}");
        assert!(!entry.targets.is_empty() && !entry.fields.is_empty() && !entry.cases.is_empty(), "empty scope in {}", entry.id);
        for field in &entry.fields {
            assert!(FIELDS.contains(&field.as_str()), "{}: unknown field {field}", entry.id);
        }
    }
    entries
}

/// Comparable fields of a conversion result.
pub const FIELDS: &[&str] = &["text/plain", "text/html", "text/rtf", "text/signal", "diagnostics", "metrics"];

pub fn field(value: &Value, name: &str) -> Value {
    match name {
        "diagnostics" | "metrics" => value[name].clone(),
        key => value["representations"][key].clone(),
    }
}
