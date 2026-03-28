// fuzz/harnesses/parse_document_structured.rs

#![no_main]

#[cfg(all(feature = "libfuzzer", feature = "libafl"))]
compile_error!("Enable exactly one of `libfuzzer` or `libafl`, not both.");

#[cfg(feature = "libfuzzer")]
use libfuzzer_sys::fuzz_target;

#[cfg(feature = "libafl")]
use libafl_libfuzzer::fuzz_target;

use solidus_engine::parse_document;
use engine_fuzz_common::*;

fuzz_target!(|doc: FuzzDoc| {
    let input = render_doc(&doc);
    let result = parse_document(&input);

    assert!(!result.version.is_empty());
    assert_ids_sequential(&result);
    assert_argument_modes(&result);
    assert_unclosed_fence_warning(&doc, &result);

    // §12.1: determinism.
    let result2 = parse_document(&input);
    assert_eq!(result, result2);
});
