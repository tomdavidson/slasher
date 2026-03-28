//! Fuzz target: parse_document
//!
//! Feeds arbitrary UTF-8 strings into the sole public entry point.
//! Engine Spec §4.2 (total function guarantee): parse_document MUST
//! return a valid ParseResult for any input. It MUST NOT panic.

#![no_main]

#[cfg(all(feature = "libfuzzer", feature = "libafl"))]
compile_error!("Enable exactly one of `libfuzzer` or `libafl`, not both.");

#[cfg(feature = "libafl")]
use libafl_libfuzzer::fuzz_target;
#[cfg(feature = "libfuzzer")]
use libfuzzer_sys::fuzz_target;
use solidus_engine::{ArgumentMode, parse_document};

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let result = parse_document(input);

        // §14: version is always populated.
        assert!(!result.version.is_empty());

        // §6.5: command IDs are sequential.
        for (i, cmd) in result.commands.iter().enumerate() {
            assert_eq!(cmd.id, format!("cmd-{i}"));
        }

        // §6.5: text block IDs are sequential.
        for (i, tb) in result.textblocks.iter().enumerate() {
            assert_eq!(tb.id, format!("text-{i}"));
        }

        // §5: every command has a valid argument mode.
        for cmd in &result.commands {
            assert!(
                cmd.arguments.mode == ArgumentMode::SingleLine || cmd.arguments.mode == ArgumentMode::Fence
            );
        }

        // §8.4: determinism.
        let result2 = parse_document(input);
        assert_eq!(result, result2);
    }
});
