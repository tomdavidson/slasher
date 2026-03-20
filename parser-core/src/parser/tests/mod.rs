use super::*;
use crate::domain::{ArgumentMode, ParserContext};

mod proptest;

// --- Test helpers ---

fn default_context() -> ParserContext {
    ParserContext::default()
}

fn parse(input: &str) -> SlashParseResult {
    parse_to_domain(input, default_context())
}

fn assert_single_command(input: &str) -> Command {
    let result = parse(input);
    assert_eq!(result.commands.len(), 1, "expected exactly 1 command");
    result.commands.into_iter().next().unwrap()
}

fn assert_no_commands(input: &str) {
    let result = parse(input);
    assert!(result.commands.is_empty(), "expected no commands, got {}", result.commands.len());
}

// --- Single-line commands ---

#[test]
fn single_line_simple_command_parses_name() {
    let cmd = assert_single_command("/hello world");
    assert_eq!(cmd.name, "hello");
}

#[test]
fn single_line_simple_command_parses_header() {
    let cmd = assert_single_command("/hello world");
    assert_eq!(cmd.arguments.header, "world");
}

#[test]
fn single_line_simple_command_has_single_line_mode() {
    let cmd = assert_single_command("/hello world");
    assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
}

#[test]
fn single_line_simple_command_payload_matches_header() {
    let cmd = assert_single_command("/hello world");
    assert_eq!(cmd.arguments.payload, "world");
}

#[test]
fn single_line_command_with_no_args_has_empty_header() {
    let cmd = assert_single_command("/ping");
    assert_eq!(cmd.arguments.header, "");
    assert_eq!(cmd.arguments.payload, "");
}

#[test]
fn single_line_command_with_complex_args() {
    let cmd = assert_single_command("/mcp call_tool read_file {\"path\": \"src/index.ts\"}");
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.arguments.header, "call_tool read_file {\"path\": \"src/index.ts\"}");
}

#[test]
fn single_line_command_id_is_cmd_0() {
    let cmd = assert_single_command("/test");
    assert_eq!(cmd.id, "cmd-0");
}

#[test]
fn single_line_command_range_is_same_line() {
    let cmd = assert_single_command("/test");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 0);
}

// --- Command name validation ---

#[test]
fn command_name_with_hyphens_parses() {
    let cmd = assert_single_command("/my-command arg");
    assert_eq!(cmd.name, "my-command");
}

#[test]
fn command_name_with_digits_parses() {
    let cmd = assert_single_command("/cmd2 arg");
    assert_eq!(cmd.name, "cmd2");
}

#[test]
fn command_starting_with_digit_is_not_command() {
    assert_no_commands("/2fast");
}

#[test]
fn command_starting_with_uppercase_is_not_command() {
    assert_no_commands("/Hello");
}

#[test]
fn slash_alone_is_not_command() {
    assert_no_commands("/");
}

// --- Leading whitespace ---

#[test]
fn leading_spaces_before_slash_are_ignored_for_detection() {
    let cmd = assert_single_command("  /hello world");
    assert_eq!(cmd.name, "hello");
}

#[test]
fn leading_tabs_before_slash_are_ignored_for_detection() {
    let cmd = assert_single_command("\t/hello world");
    assert_eq!(cmd.name, "hello");
}

// --- Continuation ---

#[test]
fn continuation_two_lines_produces_continuation_mode() {
    let input = "/mcp call_tool read_file /\n{\"path\": \"src/index.ts\"}";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Continuation);
}

#[test]
fn continuation_header_strips_trailing_marker() {
    let input = "/mcp call_tool read_file /\n{\"path\": \"src/index.ts\"}";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.header, "call_tool read_file /");
}

#[test]
fn continuation_payload_preserves_newlines() {
    let input = "/mcp call_tool read_file /\n{\"path\": \"src/index.ts\"}";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.payload, "call_tool read_file\n{\"path\": \"src/index.ts\"}\n");
}

#[test]
fn continuation_three_lines() {
    let input = "/cmd first /\nsecond /\nthird";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.payload, "first\nsecond\nthird\n");
}

#[test]
fn continuation_range_spans_all_lines() {
    let input = "/cmd first /\nsecond /\nthird";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 2);
}

#[test]
fn slash_at_end_without_space_is_not_continuation() {
    let input = "/path /var/log/";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
    assert_eq!(cmd.arguments.payload, "/var/log/");
}

// --- Fenced blocks ---

#[test]
fn closing_fence_can_be_longer_than_opener() {
    let input = "/cmd ```\ncontent\n````";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.payload, "content\n");
}

#[test]
fn inline_fence_mode_is_fence() {
    let input = "/mcp call_tool write_file ```jsonl\n{\"a\":1}\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
}

#[test]
fn inline_fence_captures_language() {
    let input = "/mcp call_tool write_file ```jsonl\n{\"a\":1}\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.fence_lang, Some("jsonl".to_string()));
}

#[test]
fn inline_fence_header_is_before_backticks() {
    let input = "/mcp call_tool write_file ```jsonl\n{\"a\":1}\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.header, "call_tool write_file");
}

#[test]
fn inline_fence_payload_is_fence_content() {
    let input = "/mcp call_tool write_file ```jsonl\n{\"a\":1}\n{\"b\":2}\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.payload, "{\"a\":1}\n{\"b\":2}\n");
}

#[test]
fn inline_fence_without_language() {
    let input = "/cmd header ```\nsome content\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.fence_lang, None);
}

#[test]
fn fence_after_continuation() {
    let input = "/cmd header /\n```json\n{\"key\": \"value\"}\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    // The continuation strips " /" from "header /", appends "header\n",
    // then the fence content follows.
    assert_eq!(cmd.arguments.payload, "header\n{\"key\": \"value\"}\n");
}

#[test]
fn fence_continuation_marker_inside_fence_is_literal() {
    let input = "/cmd ```\nline with /\nnormal line\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.payload, "line with /\nnormal line\n");
}

#[test]
fn fence_closing_must_match_opener_length() {
    let input = "/cmd ````\nline\n```\nstill inside\n````";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.payload, "line\n```\nstill inside\n");
}

#[test]
fn fence_line_with_extra_chars_is_not_closing() {
    let input = "/cmd ```\nline\n``` not-closing\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.payload, "line\n``` not-closing\n");
}

#[test]
fn fence_range_includes_closing_fence() {
    let input = "/cmd ```\nline\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 2);
}

// --- Multiple commands ---

#[test]
fn two_single_line_commands_parsed() {
    let input = "/first arg1\n/second arg2";
    let result = parse(input);
    assert_eq!(result.commands.len(), 2);
}

#[test]
fn multiple_commands_have_sequential_ids() {
    let input = "/first arg1\n/second arg2\n/third arg3";
    let result = parse(input);
    assert_eq!(result.commands[0].id, "cmd-0");
    assert_eq!(result.commands[1].id, "cmd-1");
    assert_eq!(result.commands[2].id, "cmd-2");
}

#[test]
fn commands_interspersed_with_text() {
    let input = "some text\n/first arg\nmore text\n/second arg";
    let result = parse(input);
    assert_eq!(result.commands.len(), 2);
    assert_eq!(result.text_blocks.len(), 2);
}

// --- Text blocks ---

#[test]
fn consecutive_blank_lines_are_preserved_in_text_blocks() {
    let input = "hello\n\n\nworld\n/cmd arg";
    let result = parse(input);
    assert_eq!(result.text_blocks.len(), 1);
    assert_eq!(result.text_blocks[0].content, "hello\n\n\nworld");
    assert_eq!(result.text_blocks[0].range.start_line, 0);
    assert_eq!(result.text_blocks[0].range.end_line, 3);
}

#[test]
fn text_before_command_captured() {
    let input = "hello world\n/cmd arg";
    let result = parse(input);
    assert_eq!(result.text_blocks.len(), 1);
    assert_eq!(result.text_blocks[0].content, "hello world");
    assert_eq!(result.text_blocks[0].id, "text-0");
}

#[test]
fn text_after_command_captured() {
    let input = "/cmd arg\nsome trailing text";
    let result = parse(input);
    assert_eq!(result.text_blocks.len(), 1);
    assert_eq!(result.text_blocks[0].content, "some trailing text");
}

#[test]
fn text_block_range_is_correct() {
    let input = "line0\nline1\n/cmd arg";
    let result = parse(input);
    assert_eq!(result.text_blocks[0].range.start_line, 0);
    assert_eq!(result.text_blocks[0].range.end_line, 1);
}

#[test]
fn only_text_produces_no_commands() {
    let input = "just some text\nno commands here";
    let result = parse(input);
    assert!(result.commands.is_empty());
    assert_eq!(result.text_blocks.len(), 1);
}

#[test]
fn empty_input_produces_empty_result() {
    let result = parse("");
    assert!(result.commands.is_empty());
    assert_eq!(result.text_blocks.len(), 1);
    assert_eq!(result.text_blocks[0].content, "");
}

// --- CRLF normalization ---

#[test]
fn continuation_blank_line_via_marker_is_payload() {
    let input = "/cmd header /\nline1\n /\nline2";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Continuation);
    assert_eq!(cmd.arguments.payload, "header\nline1\n\nline2\n");
}

#[test]
fn continuation_ends_on_true_blank_line() {
    let input = "/cmd header /\nline1\n\ntrailing";
    let result = parse(input);
    assert_eq!(result.commands.len(), 1);
    assert_eq!(result.commands[0].arguments.mode, ArgumentMode::Continuation);
    assert_eq!(result.commands[0].arguments.payload, "header\nline1\n");
}

#[test]
fn bare_slash_is_treated_as_payload_in_continuation() {
    let input = "/echo /\nooga booga \n/\ntesting 123";
    let result = parse(input);
    let cmd = &result.commands[0];
    assert_eq!(cmd.arguments.mode, ArgumentMode::Continuation);
    assert!(cmd.arguments.payload.contains("ooga booga \n"));
    assert!(cmd.arguments.payload.contains("/\n"));
    assert!(cmd.arguments.payload.contains("testing 123\n"));
}

// --- CRLF normalization ---

#[test]
fn carriage_returns_are_normalized_to_newlines() {
    let input = "/cmd first /\r\nsecond\nthird\r\nfourth";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Continuation);
    assert_eq!(cmd.arguments.payload, "first\nsecond\nthird\nfourth\n");
}

#[test]
fn crlf_normalized_to_lf() {
    let input = "/cmd arg1\r\n/cmd2 arg2";
    let result = parse(input);
    assert_eq!(result.commands.len(), 2);
}

#[test]
fn crlf_continuation_works() {
    let input = "/cmd first /\r\nsecond";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Continuation);
    assert_eq!(cmd.arguments.payload, "first\nsecond\n");
}

// --- Raw field ---

#[test]
fn raw_single_line_is_trimmed_line() {
    let cmd = assert_single_command("  /hello world");
    assert_eq!(cmd.raw, "/hello world");
}

#[test]
fn raw_continuation_includes_all_lines() {
    let input = "/cmd first /\nsecond";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.raw, "/cmd first /\nsecond");
}

#[test]
fn raw_fence_includes_opener_and_closer() {
    let input = "/cmd ```\ncontent\n```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.raw, "/cmd ```\ncontent\n```");
}

// --- Version and context ---

#[test]
fn result_version_is_0_1_0() {
    let result = parse("/test");
    assert_eq!(result.version, "0.1.0");
}

#[test]
fn context_fields_pass_through() {
    let ctx = ParserContext {
        source: Some("test.md".to_string()),
        user: Some("tom".to_string()),
        ..Default::default()
    };
    let result = parse_to_domain("/test", ctx);
    assert_eq!(result.context.source, Some("test.md".to_string()));
    assert_eq!(result.context.user, Some("tom".to_string()));
}

// --- JSON output ---

#[test]
fn parse_slash_commands_returns_valid_json() {
    let json = parse_slash_commands("/test arg", default_context()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["version"], "0.1.0");
    assert_eq!(parsed["commands"][0]["name"], "test");
}

#[test]
fn json_mode_field_is_single_line_string() {
    let json = parse_slash_commands("/test arg", default_context()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["commands"][0]["arguments"]["mode"], "single-line");
}

#[test]
fn json_mode_field_is_continuation_string() {
    let json = parse_slash_commands("/test arg /\nmore", default_context()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["commands"][0]["arguments"]["mode"], "continuation");
}

#[test]
fn json_mode_field_is_fence_string() {
    let json = parse_slash_commands("/test ```\ncontent\n```", default_context()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["commands"][0]["arguments"]["mode"], "fence");
}

// --- Edge cases ---

#[test]
fn unclosed_fence_at_eof_finalizes_command() {
    let input = "/cmd ```\nline1\nline2";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.payload, "line1\nline2\n");
}

#[test]
fn continuation_at_eof_finalizes_command() {
    let input = "/cmd first /";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Continuation);
}

#[test]
fn multiple_spaces_before_slash_in_continuation() {
    let input = "/cmd header /\n  /next-cmd arg";
    let result = parse(input);
    assert_eq!(result.commands.len(), 1);
    assert!(result.commands[0].arguments.payload.contains("/next-cmd"));
}

#[test]
fn fence_with_indented_closing() {
    let input = "/cmd ```\ncontent\n  ```";
    let cmd = assert_single_command(input);
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.payload, "content\n");
}

// hfuzz found cases
#[test]
fn fuzz_roundtrip_mode_mismatch() {
    use crate::{domain::ParserContext, parser::parse_to_domain, to_plaintext::to_plaintext};

    let input = "\t/ru4\x0b/";

    let ast1 = parse_to_domain(input, ParserContext::default());
    let plaintext = to_plaintext(&ast1);

    eprintln!("input bytes: {:?}", input.as_bytes());
    eprintln!("plaintext bytes: {:?}", plaintext.as_bytes());
    eprintln!("ast1 commands: {:#?}", ast1.commands);

    let ast2 = parse_to_domain(&plaintext, ParserContext::default());
    eprintln!("ast2 commands: {:#?}", ast2.commands);

    for (a, b) in ast1.commands.iter().zip(ast2.commands.iter()) {
        assert_eq!(a.name, b.name, "Command name mismatch");
        assert_eq!(a.arguments.mode, b.arguments.mode, "Mode mismatch");
        assert_eq!(a.arguments.payload, b.arguments.payload, "Payload mismatch");
    }
}
