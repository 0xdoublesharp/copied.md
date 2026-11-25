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
