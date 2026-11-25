//! Golden parity against the frozen `ts-oracle-v1` TypeScript engine.

mod common;

use copied_core::oracle::convert_markdown_ts_v1;
use copied_core::resolve_target;

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
                failures.push(format!(
                    "{} [{}] {}:\n  expected {}\n  actual   {}",
                    run.case, run.option_set, target_id, expected, actual
                ));
            }
        }
    }
    assert_eq!(total, 960);
    assert!(failures.is_empty(), "{} of {total} differ:\n{}", failures.len(), failures.join("\n"));
}
