//! normalize + join + classify composition tests.
//!
//! Verify that logical lines produced by joining are correctly classified
//! as commands or text, including fence detection across joined lines (§5.2.6).

use proptest::prelude::*;

use crate::{
    ArgumentMode, LineRange,
    classify::{LineKind, classify_line},
    fence::{accept_fence_line, finalize_fence, open_fence},
    join::LineJoiner,
    normalize::normalize,
    test_helper::{feed_body, valid_command_name},
    text::{append_text, finalize_text, start_text},
};

// --- Deterministic tests ---

#[test]
fn joining_into_fence_opener_spec_5_2_6() {
    // §5.2.6: "When backslash joining merges a command line with a line containing
    // a fence opener, the fence is detected in the resulting logical line."
    // §2.2.1: the logical line maps back to physical lines 0-1.
    // §5.2.1: text before the backtick run becomes header; text after becomes fence_lang.
    // §5.2.1: fence_backtick_count records the run length for the closer check.
    //
    // Input (two physical lines):
    //   /mcp call_tool write_file \
    //   ```json
    //
    // After join: "/mcp call_tool write_file ```json"
    // (two spaces: one trailing space before `\` + one joiner separator)
    let input = normalize("/mcp call_tool write_file \\\n```json");
    let lines: Vec<String> = input.split('\n').map(|s| s.to_string()).collect();
    let mut joiner = LineJoiner::new(lines);

    let ll = joiner.next_logical().unwrap();
    assert_eq!(ll.text, "/mcp call_tool write_file ```json");
    assert_eq!(ll.first_physical, 0);
    assert_eq!(ll.last_physical, 1);
    assert!(joiner.is_exhausted());

    match classify_line(&ll.text) {
        LineKind::Command(h) => {
            assert_eq!(h.name, "mcp");
            assert_eq!(h.header_text, "call_tool write_file");
            assert_eq!(h.fence_lang, Some("json".to_string()));
            assert_eq!(h.fence_backtick_count, 3);
        }
        LineKind::Text => panic!("expected fence command, got text"),
    }
}

#[test]
fn joined_logical_line_classifies_as_single_line_command() {
    // §2.2: line joining produces logical lines; the state machine then classifies them.
    // §5.1: a joined result with no fence opener in the args is a single-line command.
    let input = normalize("/deploy production\\\n --region us-west-2");
    let lines: Vec<String> = input.split('\n').map(|s| s.to_string()).collect();
    let ll = LineJoiner::new(lines).next_logical().unwrap();

    match classify_line(&ll.text) {
        LineKind::Command(h) => {
            assert_eq!(h.name, "deploy");
            assert_eq!(h.header_text, "production --region us-west-2");
            assert_eq!(h.mode, ArgumentMode::SingleLine);
            assert_eq!(h.fence_backtick_count, 0);
        }
        LineKind::Text => panic!("expected single-line command, got text"),
    }
}

#[test]
fn joined_invalid_slash_line_classifies_as_text() {
    // §3.2: a line whose slash is not followed by a valid name is treated as text.
    // §2.2: this check runs on the logical line after joining, not on the raw physical lines.
    let input = normalize("/Hello world\\\n more args");
    let lines: Vec<String> = input.split('\n').map(|s| s.to_string()).collect();
    let ll = LineJoiner::new(lines).next_logical().unwrap();

    assert_eq!(classify_line(&ll.text), LineKind::Text);
}

// --- Property tests ---

proptest! {
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn classify_open_accept_finalize_roundtrip(
        name in valid_command_name(),
        body_lines in prop::collection::vec("[a-zA-Z0-9]{1,30}", 0..8)
    ) {
        // §5.2: classify -> open_fence -> accept_fence_line -> finalize_fence roundtrip.
        let input = format!("/{} ```", name);
        let header = match classify_line(&input) {
            LineKind::Command(h) => h,
            LineKind::Text => panic!("expected command"),
        };
        let raw = header.raw.clone();
        let range = LineRange { start_line: 0, end_line: 0 };
        let fence = open_fence(header, raw, 0, range);
        let fence = feed_body(fence, &body_lines);
        let (fence, _) = accept_fence_line(fence, body_lines.len() + 1, "```");
        let (cmd, warnings) = finalize_fence(fence, false);
        prop_assert_eq!(cmd.name, name);
        prop_assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
        prop_assert!(warnings.is_empty());
    }

    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn text_lines_through_classify_and_collect(
        lines in prop::collection::vec("[a-zA-Z0-9 !.,]{1,40}", 1..15)
    ) {
        // §6: non-command lines are collected into text blocks via classify + text collect.
        for line in &lines {
            prop_assert!(matches!(classify_line(line), LineKind::Text));
        }
        let pending = lines.iter().enumerate().skip(1).fold(
            start_text(0, &lines[0]),
            |text, (i, line)| append_text(text, i, line),
        );
        let block = finalize_text(pending, 0);
        prop_assert_eq!(block.content, lines.join("\n"));
    }
}
