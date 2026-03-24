/// Normalize line endings in `input` to LF only.
///
/// Step 1: replace all CRLF (`\r\n`) with LF (`\n`).
/// Step 2: replace any remaining bare CR (`\r`) with LF (`\n`).
///
/// All other bytes, including literal `\n` escape sequences inside content,
/// are preserved verbatim.
pub fn normalize(input: &str) -> String { input.replace("\r\n", "\n").replace('\r', "\n") }

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // =========================================================================
    // No-op cases (input already normalized or empty)
    // RFC §3.1 / Engine Spec §5.1
    // =========================================================================

    #[test]
    fn empty_input_is_unchanged() {
        // RFC §3.1 (implied): normalize is a pure function; empty -> empty.
        // Engine Spec §5.1: "pure string transformation with no state."
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn lf_only_input_is_unchanged() {
        // RFC §3.1: LF is the normalized form; LF-only input needs no changes.
        let input = "line one\nline two\nline three";
        assert_eq!(normalize(input), input);
    }

    #[test]
    fn no_line_endings_unchanged() {
        // RFC §3.1: content with no CR or LF is unaffected.
        let input = "no line endings here";
        assert_eq!(normalize(input), input);
    }

    // =========================================================================
    // CRLF replacement (step 1)
    // RFC §3.1 step 1: "Replace all CRLF (U+000D U+000A) sequences with LF."
    // Engine Spec §5.1 step 1.
    // =========================================================================

    #[test]
    fn crlf_becomes_lf() {
        // RFC §3.1 step 1.
        assert_eq!(normalize("a\r\nb"), "a\nb");
    }

    #[test]
    fn multiple_crlf_all_converted() {
        // RFC §3.1 step 1: all occurrences replaced, not just the first.
        assert_eq!(normalize("a\r\nb\r\nc"), "a\nb\nc");
    }

    #[test]
    fn crlf_at_boundaries() {
        // RFC §3.1 step 1: replacement applies at start and end of input.
        assert_eq!(normalize("\r\nhello\r\n"), "\nhello\n");
    }

    // =========================================================================
    // Bare CR replacement (step 2)
    // RFC §3.1 step 2: "Replace all remaining bare CR (U+000D) with LF."
    // Engine Spec §5.1 step 2.
    // =========================================================================

    #[test]
    fn bare_cr_becomes_lf() {
        // RFC §3.1 step 2.
        assert_eq!(normalize("a\rb"), "a\nb");
    }

    #[test]
    fn consecutive_bare_cr() {
        // RFC §3.1 step 2: each bare CR individually becomes LF.
        assert_eq!(normalize("\r\r\r"), "\n\n\n");
    }

    // =========================================================================
    // Ordering: CRLF before bare CR (steps 1 then 2)
    // RFC §3.1: step 1 runs before step 2 to avoid double-conversion.
    // =========================================================================

    #[test]
    fn mixed_endings_no_double_conversion() {
        // RFC §3.1 steps 1-2: CRLF replaced first, so the CR in \r\n is not
        // re-matched as bare CR (which would produce \n\n).
        assert_eq!(normalize("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }

    // =========================================================================
    // Literal escape sequences preserved
    // RFC §3.1: "Literal escape sequences inside content (e.g., the
    // two-character sequence '\' 'n') are ordinary characters."
    // =========================================================================

    #[test]
    fn literal_backslash_n_preserved() {
        // RFC §3.1: two-char sequence backslash + 'n' is not a line terminator.
        let input = "before\\nafter";
        assert_eq!(normalize(input), "before\\nafter");
    }

    // =========================================================================
    // Boundary: single-character / single-sequence inputs
    // RFC §3.1
    // =========================================================================

    #[test]
    fn lone_crlf_becomes_lone_lf() {
        // RFC §3.1 step 1: a single CRLF with no surrounding content.
        assert_eq!(normalize("\r\n"), "\n");
    }

    #[test]
    fn lone_bare_cr_becomes_lf() {
        // RFC §3.1 step 2: a single bare CR with no surrounding content.
        assert_eq!(normalize("\r"), "\n");
    }

    #[test]
    fn lone_lf_unchanged() {
        // RFC §3.1: LF is already normalized.
        assert_eq!(normalize("\n"), "\n");
    }

    // =========================================================================
    // Non-ASCII / multi-byte UTF-8 preservation
    // RFC §3.1: only CR and CRLF are affected; all other bytes preserved.
    // =========================================================================

    #[test]
    fn multibyte_utf8_preserved() {
        // RFC §3.1: normalization only affects U+000D and U+000D U+000A.
        // Multi-byte content (CJK, emoji) must survive unchanged.
        let input = "héllo\r\nwörld\r🌍";
        assert_eq!(normalize(input), "héllo\nwörld\n🌍");
    }

    #[test]
    fn astral_plane_chars_preserved() {
        // RFC §3.1: non-BMP characters are not line terminators.
        let input = "𝕳𝖊𝖑𝖑𝖔\r\n𝖂𝖔𝖗𝖑𝖉";
        assert_eq!(normalize(input), "𝕳𝖊𝖑𝖑𝖔\n𝖂𝖔𝖗𝖑𝖉");
    }

    // =========================================================================
    // Property tests
    // =========================================================================

    proptest! {
        // RFC §3.1: "After normalization, all line terminators are LF."
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn output_never_contains_cr(input in "[\\x00-\\x7F]{0,500}") {
            let result = normalize(&input);
            prop_assert!(!result.contains('\r'));
        }

        // RFC §3.1 (implied): normalizing already-normalized output is a no-op.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn idempotent(input in "[\\x00-\\x7F]{0,500}") {
            let once = normalize(&input);
            let twice = normalize(&once);
            prop_assert_eq!(once, twice);
        }

        // RFC §3.1: input containing no CR requires no changes.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn clean_input_is_unchanged(input in "[\\x20-\\x7E\\n]{0,500}") {
            prop_assert_eq!(normalize(&input), input);
        }

        // RFC §3.1 step 2: each bare CR becomes LF, so LF count can only
        // stay the same or increase.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn lf_count_gte_original_lf_count(input in "[\\x00-\\x7F]{0,500}") {
            let original_lf = input.chars().filter(|&c| c == '\n').count();
            let result = normalize(&input);
            let result_lf = result.chars().filter(|&c| c == '\n').count();
            prop_assert!(result_lf >= original_lf);
        }

            // =========================================================================
    // Property tests (add inside existing proptest! block)
    // =========================================================================

    // RFC §3.1 steps 1-2: CRLF (2 bytes) becomes LF (1 byte), bare CR (1 byte)
    // becomes LF (1 byte). Output can never be longer than input.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn output_length_lte_input(input in "[\\x00-\\x7F]{0,500}") {
        let result = normalize(&input);
        prop_assert!(result.len() <= input.len());
    }

    // RFC §3.1: normalize is a pure function of its input. Same input always
    // produces the same output (no hidden state).
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn deterministic(input in "[\\x00-\\x7F]{0,300}") {
        prop_assert_eq!(normalize(&input), normalize(&input));
    }

    }
}
