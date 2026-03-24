//! Appendix B spec examples: acceptance tests.
//!
//! Full pipeline assertions matching spec appendix examples exactly.
//! Each test corresponds to one numbered example from the specification.

use crate::{ArgumentMode, parse::parse_document};

#[test]
fn spec_b1_single_line_command() {
    let result = parse_document("/echo hello world");
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.id, "cmd-0");
    assert_eq!(cmd.name, "echo");
    assert_eq!(cmd.raw, "/echo hello world");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 0);
    assert_eq!(cmd.arguments.header, "hello world");
    assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
    assert_eq!(cmd.arguments.fence_lang, None);
    assert_eq!(cmd.arguments.payload, "hello world");
}

#[test]
fn spec_b2_joined_multi_line_command() {
    // B.2: three physical lines joined into one command.
    // POSIX joining: backslash removed, lines concatenated directly.
    // Leading space on continuation lines provides the separator.
    let input = "/deploy production\\\n --region us-west-2\\\n --canary";
    let result = parse_document(input);
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.id, "cmd-0");
    assert_eq!(cmd.name, "deploy");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 2);
    assert_eq!(cmd.arguments.header, "production --region us-west-2 --canary");
    assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
    assert_eq!(cmd.arguments.payload, "production --region us-west-2 --canary");
    assert_eq!(cmd.raw, "/deploy production\\\n --region us-west-2\\\n --canary");
}

#[test]
fn spec_b3_fenced_command_with_header() {
    let input = "/mcp call_tool write_file ```json\n{\"path\": \"src/index.ts\"}\n```";
    let result = parse_document(input);
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.id, "cmd-0");
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 2);
    assert_eq!(cmd.arguments.header, "call_tool write_file");
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    assert_eq!(cmd.arguments.payload, "{\"path\": \"src/index.ts\"}");
}

#[test]
fn spec_b4_backslash_join_into_fence() {
    // B.4: trailing space before \ keeps one space before backticks after join.
    let input = "/mcp call_tool write_file \\\n```json\n{\"path\": \"foo\"}\n```";
    let result = parse_document(input);
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.id, "cmd-0");
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 3);
    assert_eq!(cmd.arguments.header, "call_tool write_file");
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    assert_eq!(cmd.arguments.payload, "{\"path\": \"foo\"}");
    assert_eq!(cmd.raw, "/mcp call_tool write_file \\\n```json\n{\"path\": \"foo\"}\n```");
}

#[test]
fn spec_b5_text_blocks_and_multiple_commands() {
    let input = "Welcome to the deployment system.\n\n/deploy staging\n/notify team --channel ops\nDeployment complete.";
    let result = parse_document(input);

    assert_eq!(result.commands.len(), 2);
    assert_eq!(result.commands.first().unwrap().id, "cmd-0");
    assert_eq!(result.commands.first().unwrap().name, "deploy");
    assert_eq!(result.commands.first().unwrap().arguments.payload, "staging");
    assert_eq!(result.commands.first().unwrap().range.start_line, 2);
    assert_eq!(result.commands.get(1).unwrap().id, "cmd-1");
    assert_eq!(result.commands.get(1).unwrap().name, "notify");
    assert_eq!(result.commands.get(1).unwrap().arguments.payload, "team --channel ops");
    assert_eq!(result.commands.get(1).unwrap().range.start_line, 3);

    assert_eq!(result.textblocks.len(), 2);
    assert_eq!(result.textblocks.first().unwrap().id, "text-0");
    assert_eq!(result.textblocks.first().unwrap().content, "Welcome to the deployment system.\n");
    assert_eq!(result.textblocks.first().unwrap().range.start_line, 0);
    assert_eq!(result.textblocks.first().unwrap().range.end_line, 1);
    assert_eq!(result.textblocks.get(1).unwrap().id, "text-1");
    assert_eq!(result.textblocks.get(1).unwrap().content, "Deployment complete.");
    assert_eq!(result.textblocks.get(1).unwrap().range.start_line, 4);
    assert_eq!(result.textblocks.get(1).unwrap().range.end_line, 4);
}

#[test]
fn spec_b6_invalid_slash_lines() {
    // B.6: /123, / (bare slash), /Hello are all invalid slash lines (§4.5).
    // They are text lines (§6.3), and consecutive text lines merge into
    // a single text block (§6.4). The valid command follows on line 3.
    let input = "/123\n/ bare slash\n/Hello\n/deploy staging";
    let result = parse_document(input);

    assert_eq!(result.textblocks.len(), 1);
    assert_eq!(result.textblocks.first().unwrap().id, "text-0");
    assert_eq!(result.textblocks.first().unwrap().content, "/123\n/ bare slash\n/Hello");
    assert_eq!(result.textblocks.first().unwrap().range.start_line, 0);
    assert_eq!(result.textblocks.first().unwrap().range.end_line, 2);

    assert_eq!(result.commands.len(), 1);
    assert_eq!(result.commands.first().unwrap().id, "cmd-0");
    assert_eq!(result.commands.first().unwrap().name, "deploy");
    assert_eq!(result.commands.first().unwrap().arguments.payload, "staging");
    assert_eq!(result.commands.first().unwrap().arguments.mode, ArgumentMode::SingleLine);
    assert_eq!(result.commands.first().unwrap().range.start_line, 3);
    assert_eq!(result.commands.first().unwrap().range.end_line, 3);

    assert!(result.warnings.is_empty());
}

#[test]
fn spec_b7_unclosed_fence() {
    let input = "/mcp call_tool ```json\n{\"incomplete\": true}";
    let result = parse_document(input);
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.id, "cmd-0");
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.arguments.header, "call_tool");
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    assert_eq!(cmd.arguments.payload, "{\"incomplete\": true}");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 1);

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings.first().unwrap().wtype, "unclosed_fence");
    assert_eq!(result.warnings.first().unwrap().start_line, Some(0));
}

#[test]
fn spec_b8_closing_fence_with_trailing_backslash() {
    // B.8: "```\" is NOT a valid closer. Fence never closes.
    // Trailing space before \ on line 0 keeps one space before backticks.
    let input = "/mcp call_tool write_file -c \\\n```json\n{\"path\": \"foo\"}\n```\\\n\\\nproduction";
    let result = parse_document(input);
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    assert!(cmd.arguments.payload.contains("{\"path\": \"foo\"}"));

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings.first().unwrap().wtype, "unclosed_fence");
}

#[test]
fn spec_b9_proper_fence_close_followed_by_content() {
    // B.9: fence closes on line 3, lines 4-5 join into text block.
    let input = "/mcp call_tool write_file -c \\\n```json\n{\"path\": \"foo\"}\n```\n\\\nproduction";
    let result = parse_document(input);

    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.arguments.header, "call_tool write_file -c");
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    assert_eq!(cmd.arguments.payload, "{\"path\": \"foo\"}");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 3);

    assert_eq!(result.textblocks.len(), 1);
    assert_eq!(result.textblocks.first().unwrap().content, "\\\nproduction");
    assert_eq!(result.textblocks.first().unwrap().range.start_line, 4);
    assert_eq!(result.textblocks.first().unwrap().range.end_line, 5);
}
