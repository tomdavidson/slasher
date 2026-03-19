// Layer 2 integration tests: cross-module composition within the application layer.
//
// These tests exercise combinations of modules that no single file's tests can cover.
// They use only the public API of each module.

use super::line_classify::{classify_line, LineKind};
use super::line_join::LineJoiner;
use super::normalize::normalize;
use super::document_parse::parse_document;
use crate::domain::ArgumentMode;

// --- normalize + line_join ---

#[test]
fn crlf_continuation_joins_same_as_lf() {
    // §2.1 rules 1-2: CRLF and bare CR both normalize to LF before any other processing.
    // §2.2: line joining runs after normalization, so a backslash before a CRLF boundary
    // must produce the same logical line as the equivalent LF-only input.
    let crlf = "/deploy production\\\r\n  --region us-west-2";
    let lf   = "/deploy production\\\n  --region us-west-2";

    let crlf_lines: Vec<String> = normalize(crlf).split('\n').map(|s| s.to_string()).collect();
    let lf_lines:   Vec<String> = normalize(lf).split('\n').map(|s| s.to_string()).collect();

    let crlf_ll = LineJoiner::new(crlf_lines).next_logical().unwrap();
    let lf_ll   = LineJoiner::new(lf_lines).next_logical().unwrap();

    assert_eq!(crlf_ll.text, lf_ll.text);
    assert_eq!(crlf_ll.first_physical, lf_ll.first_physical);
    assert_eq!(crlf_ll.last_physical, lf_ll.last_physical);
}

#[test]
fn bare_cr_continuation_joins_same_as_lf() {
    // §2.1 rule 2: remaining bare CR characters are replaced with LF after CRLF removal.
    // §2.2: joining is agnostic to the original line-ending style.
    let cr = "/deploy production\\\r  --region us-west-2";
    let lf = "/deploy production\\\n  --region us-west-2";

    let cr_lines: Vec<String> = normalize(cr).split('\n').map(|s| s.to_string()).collect();
    let lf_lines: Vec<String> = normalize(lf).split('\n').map(|s| s.to_string()).collect();

    let cr_ll = LineJoiner::new(cr_lines).next_logical().unwrap();
    let lf_ll = LineJoiner::new(lf_lines).next_logical().unwrap();

    assert_eq!(cr_ll.text, lf_ll.text);
}

#[test]
fn mixed_crlf_multi_line_join_matches_lf() {
    // §2.1: normalization applies to all line endings uniformly.
    // §2.2 step 4: joining repeats while the accumulated line still ends with `\`,
    // so three physical lines collapse into one logical line regardless of ending style.
    // §2.2.1: last_physical must be the zero-based index of the last consumed physical line.
    let crlf = "/mcp call_tool read_file\\\r\n  --path src/index.ts\\\r\n  --format json";
    let lf   = "/mcp call_tool read_file\\\n  --path src/index.ts\\\n  --format json";

    let crlf_lines: Vec<String> = normalize(crlf).split('\n').map(|s| s.to_string()).collect();
    let lf_lines:   Vec<String> = normalize(lf).split('\n').map(|s| s.to_string()).collect();

    let crlf_ll = LineJoiner::new(crlf_lines).next_logical().unwrap();
    let lf_ll   = LineJoiner::new(lf_lines).next_logical().unwrap();

    assert_eq!(crlf_ll.text, lf_ll.text);
    assert_eq!(crlf_ll.first_physical, 0);
    assert_eq!(crlf_ll.last_physical, 2);
}

// --- normalize + line_join + line_classify ---

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
    assert_eq!(ll.text, "/mcp call_tool write_file  ```json");
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
    let input = normalize("/deploy production\\\n  --region us-west-2");
    let lines: Vec<String> = input.split('\n').map(|s| s.to_string()).collect();
    let ll = LineJoiner::new(lines).next_logical().unwrap();

    match classify_line(&ll.text) {
        LineKind::Command(h) => {
            assert_eq!(h.name, "deploy");
            assert_eq!(h.header_text, "production   --region us-west-2");
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
    let input = normalize("/Hello world\\\n  more args");
    let lines: Vec<String> = input.split('\n').map(|s| s.to_string()).collect();
    let ll = LineJoiner::new(lines).next_logical().unwrap();

    assert_eq!(classify_line(&ll.text), LineKind::Text);
}


// --- Full pipeline: document_parse ---
// These tests exercise parse_document end-to-end across multiple modules.
// They verify cross-module composition that no single file's Layer 1 tests can cover.


// --- Backslash joining through full pipeline (§2.2 + §2.3) ---

#[test]
fn joined_multi_line_command_through_pipeline() {
    // §2.2: backslash continuation joins physical lines into one logical line.
    // §5.1: the joined result with no fence opener is single-line mode.
    let result = parse_document("/deploy production\\\n  --region us-west-2");
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.name, "deploy");
    assert_eq!(cmd.arguments.header, "production   --region us-west-2");
    assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
}

#[test]
fn trailing_backslash_at_eof_removed_through_pipeline() {
    // §2.2.2: trailing backslash at EOF is silently removed.
    // The command parses as single-line with the backslash stripped.
    let result = parse_document("/echo hello\\");
    assert_eq!(result.commands.len(), 1);
    assert_eq!(result.commands.first().unwrap().arguments.payload, "hello");
    assert!(result.warnings.is_empty());
}

#[test]
fn fence_immunity_backslash_inside_fence_is_literal() {
    // §2.3: trailing backslash inside fence is literal content, not a join marker.
    let result = parse_document("/cmd ```\nline one\\\nline two\n```");
    assert_eq!(result.commands.len(), 1);
    assert_eq!(result.commands.first().unwrap().arguments.payload, "line one\\\nline two");
}

#[test]
fn joining_into_fence_opener_through_pipeline() {
    // §5.2.6: backslash joining merges command line with fence opener line.
    // Physical lines 0-1 join into the command header; lines 2-3 are fence body + closer.
    let result = parse_document("/mcp call_tool write_file \\\n```json\n{\"path\": \"foo\"}\n```");
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.arguments.header, "call_tool write_file");
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    assert_eq!(cmd.arguments.payload, "{\"path\": \"foo\"}");
}

// --- Text block content with continuation lines (ADR-NNNN) ---

#[test]
fn text_block_with_continuation_preserves_backslash() {
    // ADR-NNNN: text block content stores physical lines. Backslashes are retained
    // because text blocks capture pre-join content for round-trip fidelity.
    let result = parse_document("hello \\\nworld");
    assert_eq!(result.textblocks.len(), 1);
    assert_eq!(result.textblocks.first().unwrap().content, "hello \\\nworld");
}

#[test]
fn text_block_continuation_range_covers_all_physical_lines() {
    // ADR-NNNN + §2.2.1: range spans all physical lines consumed by a joined logical line.
    let result = parse_document("hello \\\nworld");
    assert_eq!(result.textblocks.len(), 1);
    assert_eq!(result.textblocks.first().unwrap().range.start_line, 0);
    assert_eq!(result.textblocks.first().unwrap().range.end_line, 1);
}

// --- Appendix A spec examples ---
// Full pipeline assertions matching spec appendix examples exactly.
// These are the Layer 2 "acceptance test" equivalents.

#[test]
fn spec_a1_single_line_command() {
    // Appendix A.1: "/echo hello world"
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
fn spec_a2_joined_multi_line_command() {
    // Appendix A.2: three physical lines joined into one command.
    let input = "/deploy production \\\n  --region us-west-2 \\\n  --canary";
    let result = parse_document(input);
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.id, "cmd-0");
    assert_eq!(cmd.name, "deploy");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 2);
    assert_eq!(cmd.arguments.header, "production   --region us-west-2   --canary");
    assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
    assert_eq!(cmd.arguments.payload, "production   --region us-west-2   --canary");
    // §8.2: raw contains physical lines with backslashes and \n separators.
    assert_eq!(cmd.raw, "/deploy production \\\n  --region us-west-2 \\\n  --canary");
}

#[test]
fn spec_a3_fenced_command_with_header() {
    // Appendix A.3: "/mcp call_tool write_file ```json\n{...}\n```"
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
fn spec_a4_backslash_join_into_fence() {
    // Appendix A.4: four physical lines, join merges 0+1 into command+fence opener.
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
    // §8.2: raw includes all physical lines from opener through closer.
    assert_eq!(cmd.raw, "/mcp call_tool write_file \\\n```json\n{\"path\": \"foo\"}\n```");
}

#[test]
fn spec_a5_text_blocks_and_multiple_commands() {
    // Appendix A.5: text, two commands, trailing text.
    let input = "Welcome to the deployment system.\n\n/deploy staging\n/notify team --channel ops\nDeployment complete.";
    let result = parse_document(input);

    // Two commands in order.
    assert_eq!(result.commands.len(), 2);
    assert_eq!(result.commands.first().unwrap().id, "cmd-0");
    assert_eq!(result.commands.first().unwrap().name, "deploy");
    assert_eq!(result.commands.first().unwrap().arguments.payload, "staging");
    assert_eq!(result.commands.first().unwrap().range.start_line, 2);
    assert_eq!(result.commands.get(1).unwrap().id, "cmd-1");
    assert_eq!(result.commands.get(1).unwrap().name, "notify");
    assert_eq!(result.commands.get(1).unwrap().arguments.payload, "team --channel ops");
    assert_eq!(result.commands.get(1).unwrap().range.start_line, 3);

    // Two text blocks in order.
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
fn spec_a7_unclosed_fence() {
    // Appendix A.7: fence never closed, command emitted with partial payload + warning.
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
    assert_eq!(result.warnings.first().unwrap().wtype, "unclosed-fence");
    assert_eq!(result.warnings.first().unwrap().start_line, Some(0));
}

#[test]
fn spec_a8_closing_fence_with_trailing_backslash() {
    // Appendix A.8: "```\" is NOT a valid closer (not solely backticks after trim).
    // The fence never closes; all remaining lines become payload. Warning emitted.
    let input = "/mcp call_tool write_file -c \\\n```json\n{\"path\": \"foo\"}\n```\\\n\\\nproduction";
    let result = parse_document(input);
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    // Fence never closes: lines 2-5 are all payload.
    assert!(cmd.arguments.payload.contains("{\"path\": \"foo\"}"));

    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings.first().unwrap().wtype, "unclosed-fence");
}

#[test]
fn spec_a9_proper_fence_close_followed_by_content() {
    // Appendix A.9: fence closes on line 3, lines 4-5 join into text block.
    let input = "/mcp call_tool write_file -c \\\n```json\n{\"path\": \"foo\"}\n```\n\\\nproduction";
    let result = parse_document(input);

    // One command with closed fence.
    assert_eq!(result.commands.len(), 1);
    let cmd = result.commands.first().unwrap();
    assert_eq!(cmd.name, "mcp");
    assert_eq!(cmd.arguments.header, "call_tool write_file -c");
    assert_eq!(cmd.arguments.mode, ArgumentMode::Fence);
    assert_eq!(cmd.arguments.fence_lang, Some("json".to_string()));
    assert_eq!(cmd.arguments.payload, "{\"path\": \"foo\"}");
    assert_eq!(cmd.range.start_line, 0);
    assert_eq!(cmd.range.end_line, 3);

    // Lines 4-5 join into "production" text block.
    assert_eq!(result.textblocks.len(), 1);
    assert_eq!(result.textblocks.first().unwrap().content, "production");
    assert_eq!(result.textblocks.first().unwrap().range.start_line, 4);
    assert_eq!(result.textblocks.first().unwrap().range.end_line, 5);
}

// --- Property tests (Layer 2) ---
// Cross-module invariants over arbitrary input.

use proptest::prelude::*;

proptest! {

 
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn text_only_input_produces_zero_commands(
        lines in prop::collection::vec("[^/\\n][^\\n]{0,40}", 1..10)
    ) {
        // §6 + §7: input with no slash-prefixed lines produces no commands.
        // Lines are guaranteed not to start with '/' so none can be commands.
        let input = lines.join("\n");
        let result = parse_document(&input);
        prop_assert_eq!(result.commands.len(), 0);
        prop_assert!(result.textblocks.len() >= 1);
    }

    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn fenced_content_preserved_verbatim(body in "[^`\\n]{1,80}") {
        // §5.2.2: fence body is verbatim, no joining, no escaping.
        let input = format!("/cmd ```\n{}\n```", body);
        let result = parse_document(&input);
        prop_assert_eq!(result.commands.len(), 1);
        prop_assert_eq!(&result.commands.first().unwrap().arguments.payload, &body);
    }

    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn unclosed_fence_always_produces_warning(body in "[^`]{0,80}") {
        // §5.2.5: any fence reaching EOF without closer produces exactly one warning.
        let input = format!("/cmd ```\n{}", body);
        let result = parse_document(&input);
        prop_assert_eq!(result.commands.len(), 1);
        prop_assert_eq!(result.warnings.len(), 1);
        prop_assert_eq!(&result.warnings.first().unwrap().wtype, "unclosed-fence");
    }
}
