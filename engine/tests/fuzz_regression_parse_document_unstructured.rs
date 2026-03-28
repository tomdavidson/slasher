//! Fuzz regression tests for parse_document (unstructured).
//!
//! Regression files are auto-discovered from fuzz/regressions/parse_document_unstructured/.
//! To add a regression: drop a file there and run tests.

use engine_fuzz_common::{assert_argument_modes, assert_ids_sequential};
use solidus_engine::parse_document;
use std::path::Path;

fn replay(path: &Path) -> datatest_stable::Result<()> {
    let data = std::fs::read(path)?;
    let Ok(input) = std::str::from_utf8(&data) else { return Ok(()) };

    let result = parse_document(input);
    assert!(!result.version.is_empty());
    assert_ids_sequential(&result);
    assert_argument_modes(&result);

    let result2 = parse_document(input);
    assert_eq!(result, result2);
    Ok(())
}

datatest_stable::harness! {
    { test = replay, root = "fuzz/regressions/parse_document_unstructured", pattern = r"^regression-" },
}