use crate::domain::{LineRange, TextBlock};

/// Accumulated state for a text block being built line by line.
#[derive(Debug, Clone)]
pub struct PendingText {
    pub start_line: usize,
    pub end_line: usize,
    pub lines: Vec<String>,
}

/// Start a new pending text block at the given physical line index.
pub fn start_text(line_index: usize, line: &str) -> PendingText {
    PendingText {
        start_line: line_index,
        end_line: line_index,
        lines: vec![line.to_string()],
    }
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
        range: LineRange {
            start_line: text.start_line,
            end_line: text.end_line,
        },
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // --- start_text ---

    #[test]
    fn start_text_single_line_range() {
        // §6: "Text blocks use physical line numbers for their range."
        // A freshly started block spans exactly one physical line.
        let pt = start_text(3, "hello");
        assert_eq!(pt.start_line, 3);
        assert_eq!(pt.end_line, 3);
        assert_eq!(pt.lines, vec!["hello"]);
    }

    #[test]
    fn start_text_preserves_content_verbatim() {
        // §6: "Text block content preserves the original lines." Leading/trailing
        // whitespace and punctuation are not modified.
        let pt = start_text(0, "  indented line! ");
        assert_eq!(pt.lines[0], "  indented line! ");
    }

    // --- append_text ---

    #[test]
    fn append_text_updates_end_line_only() {
        // §6: "Text blocks use physical line numbers for their range." Appending
        // advances end_line but must never change start_line.
        let pt = start_text(2, "first");
        let pt = append_text(pt, 3, "second");
        assert_eq!(pt.start_line, 2);
        assert_eq!(pt.end_line, 3);
    }

    #[test]
    fn append_text_accumulates_lines_in_order() {
        // §6: "Consecutive non-command lines form a single text block." Lines must
        // appear in document order.
        let pt = start_text(0, "a");
        let pt = append_text(pt, 1, "b");
        let pt = append_text(pt, 2, "c");
        assert_eq!(pt.lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn append_text_preserves_blank_lines() {
        // §6: "Blank lines that are part of a text region are included in the
        // text block content."
        let pt = start_text(0, "before");
        let pt = append_text(pt, 1, "");
        let pt = append_text(pt, 2, "after");
        assert_eq!(pt.lines, vec!["before", "", "after"]);
    }

    #[test]
    fn append_text_preserves_whitespace_only_lines() {
        // §6: blank lines (including whitespace-only lines) are included verbatim.
        let pt = start_text(0, "text");
        let pt = append_text(pt, 1, "   ");
        assert_eq!(pt.lines[1], "   ");
    }

    // --- finalize_text ---

    #[test]
    fn finalize_text_id_format() {
        // §7: "Text blocks are assigned IDs independently in the same manner:
        // text-0, text-1, text-2, and so on."
        let block = finalize_text(start_text(0, "x"), 0);
        assert_eq!(block.id, "text-0");

        let block = finalize_text(start_text(0, "x"), 7);
        assert_eq!(block.id, "text-7");
    }

    #[test]
    fn finalize_text_content_lines_joined_with_newline() {
        // §6: "Text block content preserves the original lines joined with `\n`
        // separators."
        let pt = start_text(0, "line one");
        let pt = append_text(pt, 1, "line two");
        let pt = append_text(pt, 2, "line three");
        let block = finalize_text(pt, 0);
        assert_eq!(block.content, "line one\nline two\nline three");
    }

    #[test]
    fn finalize_text_single_line_no_trailing_newline() {
        // §6 (implied): a single-line block has no separator — content equals
        // the line itself with no added newline.
        let block = finalize_text(start_text(0, "hello"), 0);
        assert_eq!(block.content, "hello");
    }

    #[test]
    fn finalize_text_range_covers_physical_lines() {
        // §6: "Text blocks use physical line numbers for their range." The range
        // must span from the first to the last physical line consumed.
        let pt = start_text(4, "a");
        let pt = append_text(pt, 5, "b");
        let block = finalize_text(pt, 1);
        assert_eq!(block.range.start_line, 4);
        assert_eq!(block.range.end_line, 5);
    }

    #[test]
    fn finalize_text_blank_line_in_content() {
        // §6: "Blank lines that are part of a text region are included in the
        // text block content."
        let pt = start_text(0, "before");
        let pt = append_text(pt, 1, "");
        let pt = append_text(pt, 2, "after");
        let block = finalize_text(pt, 0);
        assert_eq!(block.content, "before\n\nafter");
    }

    #[test]
    fn finalize_text_incremental_emission_possible() {
        // §9.2: "A conforming implementation must support finalizing each … text
        // block as soon as its last physical line has been consumed." finalize_text
        // requires only the accumulated PendingText — no global state.
        let block = finalize_text(start_text(10, "standalone"), 3);
        assert_eq!(block.id, "text-3");
        assert_eq!(block.range.start_line, 10);
        assert_eq!(block.content, "standalone");
    }

    // --- Property tests ---

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn id_matches_text_n_pattern(id in 0usize..1000) {
            // §7: IDs are "text-0, text-1, text-2, and so on" — the pattern is
            // always `text-` followed by the decimal zero-based index.
            let block = finalize_text(start_text(0, "x"), id);
            prop_assert_eq!(block.id, format!("text-{id}"));
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn content_equals_lines_joined_with_newline(
            lines in prop::collection::vec("[a-zA-Z0-9 !.,]{0,60}", 1..20)
        ) {
            // §6: "Text block content preserves the original lines joined with
            // `\n` separators."
            let expected = lines.join("\n");
            let pt = lines.iter().enumerate().fold(
                start_text(0, &lines[0]),
                |acc, (i, line)| if i == 0 { acc } else { append_text(acc, i, line) },
            );
            let block = finalize_text(pt, 0);
            prop_assert_eq!(block.content, expected);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn range_covers_exactly_the_lines_provided(
            start in 0usize..100,
            extra in 0usize..20
        ) {
            // §6: "Text blocks use physical line numbers for their range." The range
            // must be [start, start + extra] with no gaps or extensions.
            let mut pt = start_text(start, "first");
            for i in 1..=extra {
                pt = append_text(pt, start + i, "line");
            }
            let block = finalize_text(pt, 0);
            prop_assert_eq!(block.range.start_line, start);
            prop_assert_eq!(block.range.end_line, start + extra);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn append_never_changes_start_line(
            start in 0usize..100,
            lines in prop::collection::vec("[a-zA-Z]{1,20}", 1..10)
        ) {
            // §6: start_line is fixed at the first physical line of the block;
            // subsequent appends must not modify it.
            let mut pt = start_text(start, "first");
            for (i, line) in lines.iter().enumerate() {
                pt = append_text(pt, start + i + 1, line);
            }
            prop_assert_eq!(pt.start_line, start);
        }
    }
}
