use crate::{LineRange, TextBlock};

/// Accumulated state for a text block being built line by line.
#[derive(Debug, Clone)]
pub struct PendingText {
    pub start_line: usize,
    pub end_line: usize,
    pub lines: Vec<String>,
}

/// Start a new pending text block at the given physical line index.
pub fn start_text(line_index: usize, line: &str) -> PendingText {
    PendingText { start_line: line_index, end_line: line_index, lines: vec![line.to_string()] }
}

/// Append one more physical line to an in-progress text block.
pub fn append_text(mut text: PendingText, line_index: usize, line: &str) -> PendingText {
    text.end_line = line_index;
    text.lines.push(line.to_string());
    text
}

/// Finalize a pending text block, assigning it the given sequential id.
///
/// The caller supplies a zero-based counter; this function formats it as
/// `text-{id}` per §7. The caller is responsible for incrementing the counter
/// after each call so that IDs are unique and sequential within an envelope.
pub fn finalize_text(text: PendingText, id: usize) -> TextBlock {
    let content = text.lines.join("\n");
    TextBlock {
        id: format!("text-{id}"),
        range: LineRange { start_line: text.start_line, end_line: text.end_line },
        content,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    // NOTE: LineRange, TextBlock imported from crate::domain.

    // =========================================================================
    // start_text — initial state
    // RFC §6.1 / Engine Spec §10.1
    // =========================================================================

    #[test]
    fn start_text_initial_state() {
        // RFC §6.1: "Consecutive non-command logical lines form a single
        // text block." The first line seeds the block.
        // Engine Spec §10.1: PendingText start_line == end_line, lines
        // contains exactly the first line verbatim.
        let pt = start_text(3, "  indented line!  ");
        assert_eq!(pt.start_line, 3);
        assert_eq!(pt.end_line, 3);
        assert_eq!(pt.lines, vec!["  indented line!  "]);
    }

    // =========================================================================
    // append_text — accumulation
    // RFC §6.1 / Engine Spec §10.2
    // =========================================================================

    #[test]
    fn append_advances_end_line_preserves_start() {
        // RFC §6.1: text block range spans first to last physical line.
        // Engine Spec §10.2: append updates end_line, never start_line.
        let pt = start_text(2, "first");
        let pt = append_text(pt, 3, "second");
        assert_eq!(pt.start_line, 2);
        assert_eq!(pt.end_line, 3);
    }

    #[test]
    fn append_accumulates_in_document_order() {
        // RFC §6.1: "Consecutive non-command lines form a single text block."
        // Lines must appear in the order they were encountered.
        let pt = start_text(0, "a");
        let pt = append_text(pt, 1, "b");
        let pt = append_text(pt, 2, "c");
        assert_eq!(pt.lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn append_preserves_blank_and_whitespace_lines() {
        // RFC §6.1: "Blank lines that are part of a text region are included
        // in the text block content."
        let pt = start_text(0, "before");
        let pt = append_text(pt, 1, "");
        let pt = append_text(pt, 2, "   ");
        let pt = append_text(pt, 3, "after");
        assert_eq!(pt.lines, vec!["before", "", "   ", "after"]);
    }

    // =========================================================================
    // finalize_text — output fields
    // RFC §6.5 / RFC §7.1 / Engine Spec §10.3
    // =========================================================================

    #[test]
    fn finalize_id_format() {
        // RFC §6.5: "Text blocks are assigned IDs: text-0, text-1, …"
        // Engine Spec §10.3: id = format!("text-{}", counter).
        assert_eq!(finalize_text(start_text(0, "x"), 0).id, "text-0");
        assert_eq!(finalize_text(start_text(0, "x"), 7).id, "text-7");
    }

    #[test]
    fn finalize_content_joined_with_lf() {
        // RFC §6.1: "Text block content preserves the original lines joined
        // with LF separators."
        // Engine Spec §10.3: lines.join("\n").
        let pt = start_text(0, "line one");
        let pt = append_text(pt, 1, "line two");
        let pt = append_text(pt, 2, "line three");
        let block = finalize_text(pt, 0);
        assert_eq!(block.content, "line one\nline two\nline three");
    }

    #[test]
    fn finalize_single_line_no_trailing_lf() {
        // RFC §6.1 (implied): single-line block has no separator.
        let block = finalize_text(start_text(0, "hello"), 0);
        assert_eq!(block.content, "hello");
    }

    #[test]
    fn finalize_range() {
        // Engine Spec §3.6: LineRange inclusive on both ends.
        let pt = start_text(4, "a");
        let pt = append_text(pt, 5, "b");
        let block = finalize_text(pt, 1);
        assert_eq!(block.range.start_line, 4);
        assert_eq!(block.range.end_line, 5);
    }

    #[test]
    fn finalize_blank_line_in_content() {
        // RFC §6.1: blank lines included. join("\n") on ["before","","after"]
        // produces "before\n\nafter".
        let pt = start_text(0, "before");
        let pt = append_text(pt, 1, "");
        let pt = append_text(pt, 2, "after");
        let block = finalize_text(pt, 0);
        assert_eq!(block.content, "before\n\nafter");
    }

    // =========================================================================
    // Zero-append finalize (text block boundary at EOF)
    // RFC §6.1 / Engine Spec §10.3
    // =========================================================================

    #[test]
    fn finalize_immediately_after_start() {
        // RFC §6.1: text block may end on the same line it started (single
        // logical line before EOF or command trigger).
        let block = finalize_text(start_text(5, "only line"), 2);
        assert_eq!(block.id, "text-2");
        assert_eq!(block.content, "only line");
        assert_eq!(block.range.start_line, 5);
        assert_eq!(block.range.end_line, 5);
    }

    // =========================================================================
    // All-blank-lines text block
    // RFC §6.1
    // =========================================================================

    #[test]
    fn all_blank_lines_produce_newline_only_content() {
        // RFC §6.1: blank lines are included verbatim.
        // Three empty strings joined with "\n" produce "\n\n".
        let pt = start_text(0, "");
        let pt = append_text(pt, 1, "");
        let pt = append_text(pt, 2, "");
        let block = finalize_text(pt, 0);
        assert_eq!(block.content, "\n\n");
    }

    // =========================================================================
    // Content preservation: verbatim pass-through
    // RFC §6.1
    // =========================================================================

    #[test]
    fn content_with_special_characters_preserved() {
        // RFC §6.1: text block content preserves lines verbatim.
        // Backslashes, backticks, and unicode are not interpreted.
        let pt = start_text(0, "trailing backslash\\");
        let pt = append_text(pt, 1, "```not a fence```");
        let pt = append_text(pt, 2, "emoji: 🎉");
        let block = finalize_text(pt, 0);
        assert_eq!(block.content, "trailing backslash\\\n```not a fence```\nemoji: 🎉");
    }

    // =========================================================================
    // Property tests
    // =========================================================================

    proptest! {
        // RFC §6.5: id pattern is always "text-{n}".
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn id_matches_text_n_pattern(id in 0usize..1000) {
            let block = finalize_text(start_text(0, "x"), id);
            prop_assert_eq!(block.id, format!("text-{id}"));
        }

        // RFC §6.1: content == lines.join("\n").
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn content_equals_lines_joined_with_newline(
            lines in prop::collection::vec("[a-zA-Z0-9 !.,]{0,60}", 1..20)
        ) {
            let expected = lines.join("\n");
            let pt = lines.iter().enumerate().fold(
                start_text(0, &lines[0]),
                |acc, (i, line)| if i == 0 { acc } else { append_text(acc, i, line) },
            );
            let block = finalize_text(pt, 0);
            prop_assert_eq!(block.content, expected);
        }

        // Engine Spec §10.2: range is [start, start + extra].
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn range_covers_exactly_the_lines_provided(
            start in 0usize..100,
            extra in 0usize..20
        ) {
            let mut pt = start_text(start, "first");
            for i in 1..=extra {
                pt = append_text(pt, start + i, "line");
            }
            let block = finalize_text(pt, 0);
            prop_assert_eq!(block.range.start_line, start);
            prop_assert_eq!(block.range.end_line, start + extra);
        }

        // Engine Spec §10.2: start_line is immutable after creation.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn append_never_changes_start_line(
            start in 0usize..100,
            lines in prop::collection::vec("[a-zA-Z]{1,20}", 1..10)
        ) {
            let mut pt = start_text(start, "first");
            for (i, line) in lines.iter().enumerate() {
                pt = append_text(pt, start + i + 1, line);
            }
            prop_assert_eq!(pt.start_line, start);
        }
    }
}

// =============================================================================
// TEST GAPS: spec areas this file's functions touch but are not tested
// =============================================================================
//
// | Spec Section                    | Gap                                             | Severity |
// |---------------------------------|-------------------------------------------------|----------|
// | RFC §6.1                        | LOGICAL vs PHYSICAL LINES: The spec says text   | MEDIUM   |
// |                                 | blocks are built from logical lines (post-join). |          |
// |                                 | This module accepts raw strings, so the caller   |          |
// |                                 | (state machine) must feed logical lines. No test |          |
// |                                 | verifies the contract that a joined logical line |          |
// |                                 | spanning multiple physical lines is passed as a  |          |
// |                                 | single string. The line_index parameter naming   |          |
// |                                 | is ambiguous (physical? logical?).               |          |
// |---------------------------------|-------------------------------------------------|----------|
// | RFC §6.1                        | TEXT BLOCK BOUNDARY: "A text block ends when the | LOW      |
// |                                 | next logical line is a command trigger or EOF."  |          |
// |                                 | This is the state machine's responsibility, not  |          |
// |                                 | this module's, but there's no test that          |          |
// |                                 | finalize_text can be called at any point         |          |
// |                                 | (including after zero appends) without panic.    |          |
// |                                 | The start_text + finalize_text path is tested    |          |
// |                                 | but not explicitly named as a "zero-append" case.|          |
// |---------------------------------|-------------------------------------------------|----------|
// | Engine Spec §10.3               | NO WARNINGS: Unlike finalize_fence, finalize_text| LOW     |
// |                                 | never produces warnings. This is correct per     |          |
// |                                 | spec, but the asymmetry with finalize_fence is   |          |
// |                                 | not documented or tested (e.g., no test asserting|          |
// |                                 | there's no warnings field/return).               |          |
// |---------------------------------|-------------------------------------------------|----------|
// | RFC §6.1                        | CONTENT WITH ONLY BLANK LINES: No test for a    | LOW      |
// |                                 | text block consisting entirely of blank lines    |          |
// |                                 | (e.g., three empty strings). The join produces   |          |
// |                                 | "\n\n" which is valid but untested.              |          |
