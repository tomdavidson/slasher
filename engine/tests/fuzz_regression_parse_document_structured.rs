//! Fuzz regression tests for parse_document_structured.
//!
//! Regression files are auto-discovered from fuzz/regressions/parse_document_structured/.
//! To add a regression: drop a file there and run tests.

use arbitrary::{Arbitrary, Unstructured};
use engine_fuzz_common::*;
use solidus_engine::parse_document;
use std::path::Path;

fn replay(path: &Path) -> datatest_stable::Result<()> {
    let data = std::fs::read(path)?;
    let mut u = Unstructured::new(&data);
    let Ok(doc) = FuzzDoc::arbitrary(&mut u) else { return Ok(()) };

    let input = render_doc(&doc);
    let result = parse_document(&input);

    assert!(!result.version.is_empty());
    assert_ids_sequential(&result);
    assert_argument_modes(&result);
    assert_unclosed_fence_warning(&doc, &result);

    let result2 = parse_document(&input);
    assert_eq!(result, result2);
    Ok(())
}

datatest_stable::harness! {
    { test = replay, root = "fuzz/regressions/parse_document_structured", pattern = r"^regression-" },
}