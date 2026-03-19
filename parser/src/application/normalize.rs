/// Normalize line endings in `input` to LF only.
///
/// Step 1: replace all CRLF (`\r\n`) with LF (`\n`).
/// Step 2: replace any remaining bare CR (`\r`) with LF (`\n`).
///
/// All other bytes, including literal `\n` escape sequences inside content,
/// are preserved verbatim.
pub fn normalize(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // --- Unit tests ---

    #[test]
    fn clean_lf_input_is_unchanged() {
        // §2.1: LF is the normalized form; input already using LF needs no changes.
        let input = "line one\nline two\nline three";
        assert_eq!(normalize(input), input);
    }

    #[test]
    fn empty_input_is_unchanged() {
        // §2.1 (implied): normalize is a total function — empty input produces empty output.
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn crlf_becomes_lf() {
        // §2.1 rule 1: "Replace all \r\n (CRLF) sequences with \n (LF)."
        assert_eq!(normalize("a\r\nb"), "a\nb");
    }

    #[test]
    fn bare_cr_becomes_lf() {
        // §2.1 rule 2: "Replace all remaining \r (bare CR) characters with \n."
        assert_eq!(normalize("a\rb"), "a\nb");
    }

    #[test]
    fn mixed_endings_all_become_lf() {
        // §2.1 rules 1-2: CRLF replacement runs first so the CR in \r\n is not
        // re-matched as a bare CR and doubled into \n\n.
        assert_eq!(normalize("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }

    #[test]
    fn multiple_crlf_all_converted() {
        // §2.1 rule 1: all CRLF occurrences are replaced, not just the first.
        assert_eq!(normalize("a\r\nb\r\nc"), "a\nb\nc");
    }

    #[test]
    fn crlf_at_start_and_end() {
        // §2.1 rule 1: replacement applies anywhere in the input, including boundaries.
        assert_eq!(normalize("\r\nhello\r\n"), "\nhello\n");
    }

    #[test]
    fn only_cr_sequence() {
        // §2.1 rule 2: bare CR with no following LF is still converted.
        assert_eq!(normalize("\r\r\r"), "\n\n\n");
    }

    #[test]
    fn content_without_any_line_endings_unchanged() {
        // §2.1: content with no CR or LF characters is unaffected by normalization.
        let input = "no line endings here";
        assert_eq!(normalize(input), input);
    }

    #[test]
    fn literal_backslash_n_in_content_is_not_a_line_terminator() {
        // §2.1: "Literal escape sequences inside content (e.g., \n in a JSON string
        // \"blah\nblah\") are ordinary characters, not line terminators. They are
        // preserved verbatim in the output."
        // Rust "\\n" = two chars: backslash + n. Not a real newline.
        let input = "before\\nafter";
        assert_eq!(normalize(input), "before\\nafter");
    }

    // --- Property tests ---

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn output_never_contains_cr(input in "[\x00-\x7F]{0,500}") {
            // §2.1: "After normalization, all line terminators are LF." No CR may remain.
            let result = normalize(&input);
            prop_assert!(!result.contains('\r'));
        }


        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn idempotent(input in "[\x00-\x7F]{0,500}") {
            // §2.1 (implied): normalizing already-normalized output is a no-op.
            let once = normalize(&input);
            let twice = normalize(&once);
            prop_assert_eq!(once, twice);
        }


        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn clean_input_is_unchanged(input in "[\x20-\x7E\n]{0,500}") {
            // §2.1: input containing no CR characters requires no changes.
            prop_assert_eq!(normalize(&input), input);
        }


        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn lf_count_gte_original_lf_count(input in "[\x00-\x7F]{0,500}") {
            // §2.1 rule 2: each bare CR becomes an LF, so LF count can only stay
            // the same or increase after normalization.
            let original_lf = input.chars().filter(|&c| c == '\n').count();
            let result = normalize(&input);
            let result_lf = result.chars().filter(|&c| c == '\n').count();
            prop_assert!(result_lf >= original_lf);
        }
    }
}
