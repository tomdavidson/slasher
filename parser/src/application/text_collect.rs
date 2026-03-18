use crate::domain::{LineRange, TextBlock};

/// In-progress accumulation of contiguous non-command lines.
#[derive(Debug, Clone)]
pub struct PendingText {
    pub start_line: usize,
    pub end_line: usize,
    pub lines: Vec<String>,
}

pub fn start_text(line_index: usize, line: &str) -> PendingText {
    PendingText {
        start_line: line_index,
        end_line: line_index,
        lines: vec![line.to_string()],
    }
}

pub fn append_text(mut text: PendingText, line_index: usize, line: &str) -> PendingText {
    text.end_line = line_index;
    text.lines.push(line.to_string());
    text
}

pub fn finalize_text(text: PendingText) -> TextBlock {
    let content = text.lines.join("\n");
    TextBlock {
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

    #[test]
    fn single_line_text_block() {
        let pending = start_text(0, "hello");
        let block = finalize_text(pending);
        assert_eq!(block.content, "hello");
        assert_eq!(block.range.start_line, 0);
        assert_eq!(block.range.end_line, 0);
    }

    #[test]
    fn multi_line_text_block() {
        let text = start_text(2, "first");
        let text = append_text(text, 3, "second");
        let text = append_text(text, 4, "third");
        let block = finalize_text(text);
        assert_eq!(block.content, "first\nsecond\nthird");
        assert_eq!(block.range.start_line, 2);
        assert_eq!(block.range.end_line, 4);
    }

    #[test]
    fn empty_line_preserved_in_content() {
        let text = start_text(0, "before");
        let text = append_text(text, 1, "");
        let text = append_text(text, 2, "after");
        let block = finalize_text(text);
        assert_eq!(block.content, "before\n\nafter");
    }

    #[test]
    fn whitespace_only_line_preserved() {
        let pending = start_text(5, " ");
        let block = finalize_text(pending);
        assert_eq!(block.content, " ");
        assert_eq!(block.range.start_line, 5);
    }

    #[test]
    fn append_updates_end_line_only() {
        let text = start_text(3, "first");
        let text = append_text(text, 4, "second");
        assert_eq!(text.start_line, 3);
        assert_eq!(text.end_line, 4);
    }

    // --- Property tests ---

    use proptest::prelude::*;

    fn valid_command_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9\\-]{0,20}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
    }

    fn arbitrary_line() -> impl Strategy<Value = String> {
        prop_oneof![
            // plain text
            "[a-zA-Z0-9 !.,]{0,80}",
            // command-like
            valid_command_name().prop_flat_map(|name| {
                "[a-zA-Z0-9 ]{0,40}".prop_map(move |args| format!("/{name} {args}"))
            }),
            // leading whitespace + command
            valid_command_name().prop_flat_map(|name| {
                (1usize..5, "[a-zA-Z0-9 ]{0,40}").prop_map(move |(spaces, args)| {
                    format!("{}/{} {}", " ".repeat(spaces), name, args)
                })
            }),
        ]
    }
    use crate::{
        application::line_classify::{LineKind, classify_line},
        domain::ArgumentMode,
    };
    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]



        fn classify_never_panics(line in "[\\x00-\\x7F]{0,200}") {
            let _ = classify_line(&line);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn valid_name_always_produces_command(name in valid_command_name(), args in "[a-z0-9 ]{0,40}") {
            let input = format!("/{name} {args}");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.name, name),
                LineKind::Text => panic!("expected Command for input: {input}"),
            }
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn text_without_slash_is_never_command(line in "[a-zA-Z0-9 !.,]{1,80}") {
            prop_assert!(matches!(classify_line(&line), LineKind::Text));
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn raw_field_preserves_original_input(line in arbitrary_line()) {
            if let LineKind::Command(h) = classify_line(&line) {
                prop_assert_eq!(h.raw, line);
            }
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fence_mode_iff_backticks_present(name in valid_command_name(), lang in "[a-z]{0,10}") {
            let input = format!("/{name} ```{lang}");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.mode, ArgumentMode::Fence),
                _ => panic!("expected Command"),
            }
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn continuation_mode_iff_trailing_backslash(
            name in valid_command_name(),
            args in "[a-z0-9 ]{1,30}"
        ) {
            let input = format!("/{name} {args} \\");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.mode, ArgumentMode::Continuation),
                _ => panic!("expected Command"),
            }
        }
    }
}
