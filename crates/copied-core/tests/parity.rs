//! Golden parity against the frozen `ts-oracle-v1` TypeScript engine.
//!
//! * Oracle mode (`copied_core::oracle`) must reproduce all 960 golden results.
//! * The public `convert_markdown` must match every golden field except those
//!   covered by `tests/parity/divergences.toml`, which must equal the reviewed
//!   values in `tests/parity/improved.json`.
//!
//! Regenerate the reviewed values after an intentional change with
//! `COPIED_BLESS=1 cargo test -p copied-core --test parity -- --ignored bless`.

mod common;

use copied_core::oracle::convert_markdown_ts_v1;
use copied_core::{convert_markdown, resolve_target};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

fn key(case: &str, option_set: &str, target: &str, field: &str) -> String {
    format!("{case}|{option_set}|{target}|{field}")
}

#[test]
fn oracle_mode_matches_all_golden_outputs() {
    let (tag, runs) = common::corpus();
    assert_eq!(tag, "ts-oracle-v1");
    let mut total = 0;
    let mut failures = Vec::new();
    for run in &runs {
        for (target_id, expected) in &run.results {
            total += 1;
            let target = resolve_target(target_id).expect("known target");
            let actual = common::to_value(&convert_markdown_ts_v1(&run.markdown, target, &run.options));
            if &actual != expected {
                failures.push(format!("{} [{}] {target_id}:\n  expected {expected}\n  actual   {actual}", run.case, run.option_set));
            }
        }
    }
    assert_eq!(total, 960);
    assert!(failures.is_empty(), "{} of {total} differ:\n{}", failures.len(), failures.join("\n"));
}

/// Fields where the improved engine differs from the oracle.
fn improved_differences() -> BTreeMap<String, (Value, Value)> {
    let (_, runs) = common::corpus();
    let mut differences = BTreeMap::new();
    for run in &runs {
        for (target_id, expected) in &run.results {
            let target = resolve_target(target_id).expect("known target");
            let actual = common::to_value(&convert_markdown(&run.markdown, target, &run.options));
            assert_eq!(actual["schemaVersion"], expected["schemaVersion"]);
            assert_eq!(actual["target"], expected["target"]);
            for field in common::FIELDS {
                let (oracle, improved) = (common::field(expected, field), common::field(&actual, field));
                if oracle != improved {
                    differences.insert(key(&run.case, &run.option_set, target_id, field), (oracle, improved));
                }
            }
        }
    }
    differences
}

fn improved_snapshot() -> Map<String, Value> {
    let raw = include_str!("parity/improved.json");
    serde_json::from_str::<Value>(raw).expect("improved.json parses").as_object().cloned().expect("object")
}

#[test]
fn improved_engine_matches_oracle_except_documented_divergences() {
    let divergences = common::divergences();
    let differences = improved_differences();
    let snapshot = improved_snapshot();
    let mut problems = Vec::new();
    let mut used: BTreeSet<&str> = BTreeSet::new();
    for (name, (oracle, improved)) in &differences {
        let parts: Vec<&str> = name.split('|').collect();
        let (case, target, field) = (parts[0], parts[2], parts[3]);
        let covering: Vec<&common::Divergence> = divergences
            .iter()
            .filter(|d| d.cases.iter().any(|c| c == case) && d.targets.iter().any(|t| t == target) && d.fields.iter().any(|f| f == field))
            .collect();
        if covering.is_empty() {
            problems.push(format!("undocumented divergence {name}:\n  oracle   {oracle}\n  improved {improved}"));
            continue;
        }
        used.extend(covering.iter().map(|d| d.id.as_str()));
        match snapshot.get(name) {
            Some(reviewed) if reviewed == improved => {}
            Some(reviewed) => problems.push(format!("{name} changed:\n  reviewed {reviewed}\n  actual   {improved}")),
            None => problems.push(format!("{name} has no reviewed value in improved.json")),
        }
    }
    for name in snapshot.keys() {
        if !differences.contains_key(name) {
            problems.push(format!("{name} is in improved.json but now matches the oracle"));
        }
    }
    for divergence in &divergences {
        if !used.contains(divergence.id.as_str()) {
            problems.push(format!("divergence `{}` no longer differs from the oracle", divergence.id));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // Every divergent field must keep metrics consistent with its plain text.
    let (_, runs) = common::corpus();
    for run in &runs {
        for (target_id, _) in &run.results {
            let conversion = convert_markdown(&run.markdown, resolve_target(target_id).unwrap(), &run.options);
            if target_id != "twitter" {
                assert_eq!(conversion.metrics.output_length, conversion.representations.plain.chars().count());
            }
        }
    }
}

#[test]
#[ignore = "writes tests/parity/improved.json when COPIED_BLESS=1"]
fn bless_improved_snapshot() {
    if std::env::var("COPIED_BLESS").as_deref() != Ok("1") {
        return;
    }
    let snapshot: Map<String, Value> = improved_differences().into_iter().map(|(name, (_, improved))| (name, improved)).collect();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/parity/improved.json");
    let mut text = serde_json::to_string_pretty(&Value::Object(snapshot)).expect("serializes");
    text.push('\n');
    std::fs::write(path, text).expect("write improved.json");
}
