use super::{
    command_accumulate::{AcceptResult, PendingCommand, accept_line, start_command},
    command_finalize::finalize_command,
    line_classify::{LineKind, classify_line},
    text_collect::{PendingText, append_text, finalize_text, start_text},
};
use crate::domain::{Command, ParseResult, ParseWarning, ParserContext, SPEC_VERSION, TextBlock};

struct ParseState {
    current_cmd: Option<PendingCommand>,
    current_text: Option<PendingText>,
    commands: Vec<Command>,
    text_blocks: Vec<TextBlock>,
    warnings: Vec<ParseWarning>,
}

pub fn parse_document(input: &str) -> ParseResult {
    let state = input
        .lines()
        .enumerate()
        .fold(empty_state(), |state, (i, line)| step(state, i, line));

    finalize(state)
}

fn empty_state() -> ParseState {
    ParseState {
        current_cmd: None,
        current_text: None,
        commands: Vec::new(),
        text_blocks: Vec::new(),
        warnings: Vec::new(),
    }
}

fn step(mut state: ParseState, line_index: usize, line: &str) -> ParseState {
    if try_feed_command(&mut state, line_index, line) {
        return state;
    }

    classify_fresh_line(&mut state, line_index, line);
    state
}

fn try_feed_command(state: &mut ParseState, line_index: usize, line: &str) -> bool {
    let Some(cmd) = state.current_cmd.take() else {
        return false;
    };

    if !cmd.is_open {
        absorb_command(state, cmd);
        return false;
    }

    let (updated_cmd, result) = accept_line(cmd, line_index, line);
    match result {
        AcceptResult::Consumed => {
            state.current_cmd = Some(updated_cmd);
            true
        }
        AcceptResult::Completed => {
            absorb_command(state, updated_cmd);
            true
        }
        AcceptResult::Rejected => {
            absorb_command(state, updated_cmd);
            false
        }
    }
}

fn classify_fresh_line(state: &mut ParseState, line_index: usize, line: &str) {
    match classify_line(line) {
        LineKind::Command(header) => {
            flush_text(state);
            state.current_cmd = Some(start_command(header, line_index));
        }
        LineKind::Text => {
            accumulate_text(state, line_index, line);
        }
    }
}

fn absorb_command(state: &mut ParseState, cmd: PendingCommand) {
    let finalized = finalize_command(cmd);
    state.commands.push(finalized.command);
    state.warnings.extend(finalized.warnings);
}

fn flush_text(state: &mut ParseState) {
    if let Some(text) = state.current_text.take() {
        state.text_blocks.push(finalize_text(text));
    }
}

fn accumulate_text(state: &mut ParseState, line_index: usize, line: &str) {
    state.current_text = match state.current_text.take() {
        Some(text) => Some(append_text(text, line_index, line)),
        None => Some(start_text(line_index, line)),
    };
}

fn finalize(mut state: ParseState) -> ParseResult {
    if let Some(cmd) = state.current_cmd.take() {
        absorb_command(&mut state, cmd);
    }

    flush_text(&mut state);

    ParseResult {
        commands: state.commands,
        text_blocks: state.text_blocks,
        warnings: state.warnings,
        version: SPEC_VERSION.to_owned(),
        context: ParserContext::default(),
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::{application::document_parse::parse_document, domain::ArgumentMode};

    fn parse(input: &str) -> crate::domain::ParseResult {
        parse_document(input)
    }

    fn assert_single_command(input: &str) -> crate::domain::Command {
        let result = parse(input);
        assert_eq!(result.commands.len(), 1, "expected exactly 1 command");
        result.commands.into_iter().next().unwrap()
    }

    #[test]
    fn single_line_simple_command_parses_name() {
        let cmd = assert_single_command("/hello world");
        assert_eq!(cmd.name, "hello");
    }

    #[test]
    fn trailing_newline_does_not_create_empty_text_block() {
        let result = parse("/cmd arg\n");
        assert_eq!(result.text_blocks.len(), 0);
    }

    #[test]
    fn single_line_mode_threads_through_from_classify() {
        let cmd = assert_single_command("/hello world");
        assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
    }

    #[test]
    fn single_line_payload_threads_through_from_classify() {
        let cmd = assert_single_command("/hello world");
        assert_eq!(cmd.arguments.payload, "world");
    }

    #[test]
    fn unclosed_continuation_produces_warning() {
        let input = "/cmd start \\";
        let result = parse(input);
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn leading_whitespace_command_threads_through() {
        let cmd = assert_single_command("   /hello world");
        assert_eq!(cmd.name, "hello");
    }

    #[test]
    fn single_line_command_with_no_args_has_empty_header() {
        let cmd = assert_single_command("/ping");
        assert_eq!(cmd.arguments.header, "");
        assert_eq!(cmd.arguments.payload, "");
    }

    #[test]
    fn single_line_command_with_complex_args() {
        let cmd = assert_single_command(r#"/mcp call_tool read_file {"path": "src/index.ts"}"#);
        assert_eq!(cmd.name, "mcp");
        assert_eq!(
            cmd.arguments.header,
            r#"call_tool read_file {"path": "src/index.ts"}"#
        );
    }

    #[test]
    fn single_line_command_range_is_same_line() {
        let cmd = assert_single_command("/test");
        assert_eq!(cmd.range.start_line, 0);
        assert_eq!(cmd.range.end_line, 0);
    }

    #[test]
    fn parses_text_and_single_command() {
        let input = "intro\n/cmd arg\noutro";
        let result = parse_document(input);
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.text_blocks.len(), 2);
        assert_eq!(result.commands[0].name, "cmd");
        assert_eq!(result.commands[0].arguments.mode, ArgumentMode::SingleLine);
    }

    #[test]
    fn command_on_line_two_has_correct_range() {
        let input = "text\nmore text\n/cmd arg";
        let result = parse(input);
        assert_eq!(result.commands[0].range.start_line, 2);
        assert_eq!(result.commands[0].range.end_line, 2);
    }

    #[test]
    fn two_single_line_commands_both_parse() {
        let result = parse("/first a\n/second b");
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.commands[0].name, "first");
        assert_eq!(result.commands[1].name, "second");
    }

    #[test]
    fn empty_input_produces_empty_result() {
        let result = parse("");
        assert_eq!(result.commands.len(), 0);
        assert_eq!(result.text_blocks.len(), 0);
        assert_eq!(result.warnings.len(), 0);
    }

    #[test]
    fn text_only_produces_one_text_block() {
        let result = parse("just some text");
        assert_eq!(result.commands.len(), 0);
        assert_eq!(result.text_blocks.len(), 1);
    }

    #[test]
    fn adjacent_text_lines_merge_into_one_block() {
        let result = parse("line one\nline two\nline three");
        assert_eq!(result.text_blocks.len(), 1);
        assert_eq!(result.text_blocks[0].range.start_line, 0);
        assert_eq!(result.text_blocks[0].range.end_line, 2);
    }

    #[test]
    fn text_block_before_command_has_correct_content() {
        let result = parse("intro line\n/cmd arg");
        assert_eq!(result.text_blocks.len(), 1);
        assert_eq!(result.text_blocks[0].content, "intro line");
    }

    #[test]
    fn text_block_after_command_has_correct_range() {
        let result = parse("/cmd arg\noutro line");
        assert_eq!(result.text_blocks.len(), 1);
        assert_eq!(result.text_blocks[0].range.start_line, 1);
    }

    #[test]
    fn continuation_command_parses_through_document() {
        let input = "/cmd first \\\nsecond \\\nthird";
        let cmd = assert_single_command(input);
        assert_eq!(cmd.arguments.mode, ArgumentMode::Continuation);
        assert_eq!(cmd.arguments.payload, "first\nsecond\nthird");
    }

    #[test]
    fn fenced_command_parses_through_document() {
        let input = "/cmd ```\nline one\nline two\n```";
        let cmd = assert_single_command(input);
        assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
        assert_eq!(cmd.arguments.payload, "line one\nline two");
    }

    #[test]
    fn unclosed_fence_produces_warning() {
        let input = "/cmd ```\nline one\nline two";
        let result = parse(input);
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn fence_followed_by_new_command() {
        let input = "/first ```\nbody\n```\n/second arg";
        let result = parse(input);
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.commands[0].name, "first");
        assert_eq!(result.commands[1].name, "second");
    }

    #[test]
    fn continuation_followed_by_new_command() {
        let input = "/first start \\\nend\n/second arg";
        let result = parse(input);
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.commands[0].name, "first");
        assert_eq!(result.commands[1].name, "second");
    }

    #[test]
    fn version_is_set() {
        let result = parse("");
        assert_eq!(result.version, crate::domain::SPEC_VERSION);
    }

    // --- Property tests ---

    fn valid_command_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9\\-]{0,15}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
    }

    fn command_line() -> impl Strategy<Value = String> {
        (valid_command_name(), "[a-z0-9 ]{0,30}").prop_map(|(name, args)| format!("/{name} {args}"))
    }

    fn text_line() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 !.,]{1,60}".prop_filter("must not start with slash", |s| {
            !s.trim_start().starts_with('/')
        })
    }

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn never_panics_on_arbitrary_input(input in "[\\x00-\\x7F]{0,500}") {
            let _ = parse_document(&input);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn command_count_matches_slash_lines(
            commands in prop::collection::vec(command_line(), 1..10)
        ) {
            let input = commands.join("\n");
            let result = parse_document(&input);
            prop_assert_eq!(result.commands.len(), commands.len());
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn text_only_input_produces_zero_commands(
            lines in prop::collection::vec(text_line(), 1..20)
        ) {
            let input = lines.join("\n");
            let result = parse_document(&input);
            prop_assert!(result.commands.is_empty());
            prop_assert!(!result.text_blocks.is_empty());
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn version_is_always_spec_version(input in "[\\x20-\\x7E]{0,200}") {
            let result = parse_document(&input);
            prop_assert_eq!(&result.version, crate::domain::SPEC_VERSION);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fenced_content_preserved_verbatim(
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9 !]{1,50}", 1..5)
        ) {
            let body = body_lines.join("\n");
            let input = format!("/{name} ```\n{body}\n```");
            let result = parse_document(&input);
            prop_assert_eq!(result.commands.len(), 1);
            prop_assert_eq!(&result.commands[0].arguments.payload, &body);
            prop_assert_eq!(&result.commands[0].arguments.mode, &ArgumentMode::Fence);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn interleaved_text_and_commands_both_captured(
            cmd_count in 1usize..5
        ) {
            let mut lines = Vec::new();
            for i in 0..cmd_count {
                lines.push(format!("text line {i}"));
                lines.push(format!("/cmd{i} arg"));
            }
            let input = lines.join("\n");
            let result = parse_document(&input);
            prop_assert_eq!(result.commands.len(), cmd_count);
            prop_assert_eq!(result.text_blocks.len(), cmd_count);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn unclosed_fence_always_produces_warning(
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9]{1,30}", 1..5)
        ) {
            let body = body_lines.join("\n");
            let input = format!("/{name} ```\n{body}");
            let result = parse_document(&input);
            prop_assert_eq!(result.commands.len(), 1);
            prop_assert_eq!(result.warnings.len(), 1);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn text_block_content_matches_non_command_lines(
            lines in prop::collection::vec(text_line(), 1..10)
        ) {
            let input = lines.join("\n");
            let result = parse_document(&input);
            prop_assert_eq!(result.text_blocks.len(), 1);
            prop_assert_eq!(&result.text_blocks[0].content, &input);
        }

                fn continuation_payload_matches_joined_lines(
            name in valid_command_name(),
            middle_lines in prop::collection::vec("[a-z0-9]{1,20}", 0..5),
            final_line in "[a-z0-9]{1,20}"
        ) {
            let mut input_lines = vec![format!("/{name} start \\")];
            let mut expected_parts = vec!["start".to_string()];
            for line in &middle_lines {
                input_lines.push(format!("{line} \\"));
                expected_parts.push(line.clone());
            }
            input_lines.push(final_line.clone());
            expected_parts.push(final_line);
            let input = input_lines.join("\n");

            let result = parse_document(&input);
            prop_assert_eq!(result.commands.len(), 1);
            prop_assert_eq!(&result.commands[0].arguments.mode, &ArgumentMode::Continuation);
            prop_assert_eq!(&result.commands[0].arguments.payload, &expected_parts.join("\n"));
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn unclosed_continuation_always_produces_warning(
            name in valid_command_name(),
            middle_lines in prop::collection::vec("[a-z0-9]{1,20}", 0..3)
        ) {
            let mut input_lines = vec![format!("/{name} start \\")];
            for line in &middle_lines {
                input_lines.push(format!("{line} \\"));
            }
            let input = input_lines.join("\n");

            let result = parse_document(&input);
            prop_assert_eq!(result.commands.len(), 1);
            prop_assert_eq!(result.warnings.len(), 1);
        }

    }
}
