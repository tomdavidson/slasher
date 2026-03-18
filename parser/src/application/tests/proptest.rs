use proptest::prelude::*;

use crate::{
    application::{
        command_accumulate::{accept_line, start_command},
        command_finalize::finalize_command,
        line_classify::{LineKind, classify_line},
        text_collect::{append_text, finalize_text, start_text},
    },
    domain::ArgumentMode,
};

fn valid_command_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9\\-]{0,15}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
}

proptest! {
    /// classify -> accumulate -> finalize roundtrip never panics.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn classify_accumulate_finalize_roundtrip(
        name in valid_command_name(),
        body_lines in prop::collection::vec("[a-zA-Z0-9]{1,30}", 0..8)
    ) {
        let input = format!("/{name} ```");
        let header = match classify_line(&input) {
            LineKind::Command(h) => h,
            LineKind::Text => panic!("expected command"),
        };

        let cmd = start_command(header, 0);

        let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
            let (next, _) = accept_line(cmd, i + 1, line);
            next
        });
        let (cmd, _) = accept_line(cmd, body_lines.len() + 1, "```");

        let finalized = finalize_command(cmd);
        prop_assert_eq!(finalized.command.name, name);
        prop_assert_eq!(finalized.command.arguments.mode, ArgumentMode::Fence);
        prop_assert!(finalized.warnings.is_empty());
    }

    /// Text lines classified as Text always produce valid text blocks.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn text_lines_through_classify_and_collect(
        lines in prop::collection::vec("[a-zA-Z0-9 !.,]{1,40}", 1..15)
    ) {
        for line in &lines {
            prop_assert!(matches!(classify_line(line), LineKind::Text));
        }

        let pending = lines.iter().enumerate().skip(1).fold(
            start_text(0, &lines[0]),
            |text, (i, line)| append_text(text, i, line),
        );
        let block = finalize_text(pending);
        prop_assert_eq!(block.content, lines.join("\n"));
    }
}
