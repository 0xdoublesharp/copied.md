//! Differential check against a corpus produced by `tools/differential.mjs`.
//!
//! Ignored by default because it needs a generated file:
//! `COPIED_ORACLE_JSON=/path/diff.json cargo test -p copied-core --test differential -- --ignored`

mod common;

use copied_core::oracle::convert_markdown_ts_v1;
use copied_core::{ConversionOptions, resolve_target};
use serde_json::Value;
use std::collections::BTreeMap;

#[test]
#[ignore = "needs COPIED_ORACLE_JSON from tools/differential.mjs"]
fn oracle_mode_matches_differential_corpus() {
    let Ok(path) = std::env::var("COPIED_ORACLE_JSON") else {
        return;
    };
    let runs: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read corpus")).expect("json");
    let limit: usize = std::env::var("COPIED_DIFF_SHOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let filter = std::env::var("COPIED_DIFF_TARGET").ok();
    let mut per_target: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut shown = 0;
    for run in &runs {
        let markdown = run["markdown"].as_str().expect("markdown");
        let options: ConversionOptions =
            serde_json::from_value(run["options"].clone()).expect("options");
        for (id, expected) in run["results"].as_object().expect("results") {
            let target = resolve_target(id).expect("target");
            let actual = common::to_value(&convert_markdown_ts_v1(markdown, target, &options));
            let entry = per_target.entry(id.clone()).or_default();
            entry.0 += 1;
            if &actual != expected {
                entry.1 += 1;
                if shown < limit && filter.as_deref().is_none_or(|f| f == id) {
                    shown += 1;
                    eprintln!(
                        "--- {id} {:?}\n  md {markdown:?}\n  E {expected}\n  A {actual}",
                        run["options"]
                    );
                }
            }
        }
    }
    let mut mismatches = 0;
    for (id, (total, bad)) in &per_target {
        eprintln!("{id:>12}: {bad}/{total} differ");
        mismatches += bad;
    }
    assert_eq!(mismatches, 0, "differential mismatches");
}
