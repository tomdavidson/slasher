use super::{
    command_accumulate::{AcceptResult, PendingCommand, accept_line, start_command},
    command_finalize::finalize_command,
    line_classify::{CommandHeader, LineKind, classify_line},
    line_join::LineJoiner,
    normalize::normalize,
    text_collect::{PendingText, append_text, finalize_text, start_text},
};
use crate::domain::{Command, ParseResult, TextBlock, Warning, SPEC_VERSION};

// --- Types ---

#[derive(Debug)]
enum ParserState {
    Idle,
    InFence(PendingCommand),
}

#[derive(PartialEq, Eq)]
enum LoopAction {
    Continue,
    Break,
}

struct ParseCtx {
    state: ParserState,
    current_text: Option<PendingText>,
    commands: Vec<Command>,
    textblocks: Vec<TextBlock>,
    warnings: Vec<Warning>,
    cmd_seq: usize,
    text_seq: usize,
}

impl ParseCtx {
    fn new() -> Self {
        Self {
            state: ParserState::Idle,
            current_text: None,
            commands: Vec::new(),
            textblocks: Vec::new(),
            warnings: Vec::new(),
            cmd_seq: 0,
            text_seq: 0,
        }
    }

    fn into_result(self) -> ParseResult {
        ParseResult {
            version: SPEC_VERSION.to_owned(),
            commands: self.commands,
            textblocks: self.textblocks,
            warnings: self.warnings,
        }
    }
}

// --- Public entry point ---

pub fn parse_document(input: &str) -> ParseResult {
    let normalized = normalize(input);
    let physical_lines = split_physical_lines(&normalized);
    let owned: Vec<String> = physical_lines.iter().map(|s| s.to_string()).collect();
    let mut joiner = LineJoiner::new(owned);
    let mut ctx = ParseCtx::new();

    while step(&mut ctx, &mut joiner, &physical_lines) == LoopAction::Continue {}

    flush_text(&mut ctx);
    ctx.into_result()
}

// --- Pipeline ---

fn split_physical_lines(normalized: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

fn step(ctx: &mut ParseCtx, joiner: &mut LineJoiner, phys: &[&str]) -> LoopAction {
    let state = std::mem::replace(&mut ctx.state, ParserState::Idle);
    match state {
        ParserState::Idle => step_idle(ctx, joiner, phys),
        ParserState::InFence(cmd) => step_in_fence(ctx, joiner, cmd),
    }
}

// --- State handlers ---

fn step_idle(ctx: &mut ParseCtx, joiner: &mut LineJoiner, phys: &[&str]) -> LoopAction {
    let Some(ll) = joiner.next_logical() else {
        return LoopAction::Break;
    };
    match classify_line(&ll.text) {
        LineKind::Command(mut header) => {
            header.raw = phys[ll.first_physical..=ll.last_physical].join("\n");
            flush_text(ctx);
            start_new_command(ctx, header, ll.first_physical);
        }
        LineKind::Text => {
            accumulate_text(ctx, ll.first_physical, ll.last_physical, phys);
        }
    }
    LoopAction::Continue
}

fn step_in_fence(
    ctx: &mut ParseCtx,
    joiner: &mut LineJoiner,
    cmd: PendingCommand,
) -> LoopAction {
    let Some((line_idx, line)) = joiner.next_physical() else {
        absorb_command(ctx, cmd);
        return LoopAction::Break;
    };
    let (updated, result) = accept_line(cmd, line_idx, &line);
    match result {
        AcceptResult::Consumed => {
            ctx.state = ParserState::InFence(updated);
        }
        AcceptResult::Completed | AcceptResult::Rejected => {
            absorb_command(ctx, updated);
        }
    }
    LoopAction::Continue
}

// --- Context helpers ---

fn start_new_command(ctx: &mut ParseCtx, header: CommandHeader, first_physical: usize) {
    let cmd = start_command(header, first_physical, ctx.cmd_seq);
    ctx.cmd_seq += 1;
    if cmd.is_open {
        ctx.state = ParserState::InFence(cmd);
    } else {
        absorb_command(ctx, cmd);
    }
}

fn absorb_command(ctx: &mut ParseCtx, cmd: PendingCommand) {
    let finalized = finalize_command(cmd);
    ctx.commands.push(finalized.command);
    ctx.warnings.extend(finalized.warnings);
}

fn flush_text(ctx: &mut ParseCtx) {
    let Some(text) = ctx.current_text.take() else { return };
    ctx.textblocks.push(finalize_text(text, ctx.text_seq));
    ctx.text_seq += 1;
}

fn accumulate_text(ctx: &mut ParseCtx, first: usize, last: usize, phys: &[&str]) {
    let text = match ctx.current_text.take() {
        Some(existing) => fold_physical_lines(existing, first, last, phys),
        None => {
            let started = start_text(first, phys[first]);
            fold_physical_lines(started, first + 1, last, phys)
        }
    };
    ctx.current_text = Some(text);
}

fn fold_physical_lines(
    mut text: PendingText,
    from: usize,
    to: usize,
    phys: &[&str],
) -> PendingText {
    for (idx, line) in (from..=to).zip(&phys[from..=to]) {
        text = append_text(text, idx, line);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::parse_document;
    use crate::domain::{ArgumentMode, SPEC_VERSION};

    // --- Category 1: Empty / trivial input ---

    #[test]
    fn empty_input_produces_empty_result() {
        // §8.1 (implied): empty input produces envelope with empty arrays.
        let result = parse_document("");
        assert!(result.commands.is_empty());
        assert!(result.textblocks.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn whitespace_only_input_is_text() {
        // §6 (implied): lines that are not commands become text blocks.
        let result = parse_document("   ");
        assert!(result.commands.is_empty());
        assert_eq!(result.textblocks.len(), 1);
    }

    // --- Category 2: Single-line command threading ---

    #[test]
    fn single_line_command_parses_name() {
        // §5.1: command name is extracted and threaded to output.
        let result = parse_document("/deploy production");
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands.first().unwrap().name, "deploy");
    }

    #[test]
    fn single_line_command_with_no_args_has_empty_payload() {
        // §5.1 (implied): command with no arguments produces empty header and payload.
        let result = parse_document("/ping");
        let cmd = result.commands.first().unwrap();
        assert_eq!(cmd.arguments.header, "");
        assert_eq!(cmd.arguments.payload, "");
    }

    #[test]
    fn single_line_command_range_is_same_line() {
        // §2.2.1: range uses physical line numbers. Single logical line -> start == end.
        let result = parse_document("/deploy production");
        let cmd = result.commands.first().unwrap();
        assert_eq!(cmd.range.start_line, 0);
        assert_eq!(cmd.range.end_line, 0);
    }

    #[test]
    fn single_line_mode_threads_through() {
        // §5.1: mode is single-line when no fence opener present.
        let result = parse_document("/deploy production");
        assert_eq!(
            result.commands.first().unwrap().arguments.mode,
            ArgumentMode::SingleLine
        );
    }

    #[test]
    fn single_line_payload_threads_through() {
        // §5.1: payload equals the full arguments text.
        let result = parse_document("/deploy production --region us-west-2");
        assert_eq!(
            result.commands.first().unwrap().arguments.payload,
            "production --region us-west-2"
        );
    }

    // --- Category 3: Fenced command pipeline ---

    #[test]
    fn fenced_command_parses_through_document() {
        // §5.2.2: fence body lines appended verbatim, joined with \n separators.
        let result = parse_document("/cmd ```\nline one\nline two\n```");
        assert_eq!(result.commands.len(), 1);
        assert_eq!(
            result.commands.first().unwrap().arguments.payload,
            "line one\nline two"
        );
    }

    #[test]
    fn fenced_command_range_covers_opener_through_closer() {
        // §2.2.1: range.start_line and range.end_line are physical line numbers.
        let result = parse_document("/cmd ```\nbody\n```");
        let cmd = result.commands.first().unwrap();
        assert_eq!(cmd.range.start_line, 0);
        assert_eq!(cmd.range.end_line, 2);
    }

    #[test]
    fn fence_lang_threads_through() {
        // §5.2.1: fence_lang is the language identifier from the opener.
        let result = parse_document("/cmd ```json\n{}\n```");
        assert_eq!(
            result.commands.first().unwrap().arguments.fence_lang,
            Some("json".to_string())
        );
    }

    // --- Category 4: Unclosed fence ---

    #[test]
    fn unclosed_fence_produces_warning() {
        // §5.2.5: unclosed fence emits warning with type "unclosed-fence".
        let result = parse_document("/cmd ```\norphaned body");
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings.first().unwrap().wtype, "unclosed-fence");
    }

    #[test]
    fn unclosed_fence_still_produces_command() {
        // §5.2.5: parser emits the command with its partial payload.
        let result = parse_document("/cmd ```\npartial");
        assert_eq!(result.commands.len(), 1);
        assert_eq!(
            result.commands.first().unwrap().arguments.payload,
            "partial"
        );
    }

    // --- Category 5: Text block accumulation ---

    #[test]
    fn text_only_produces_one_text_block() {
        // §6: consecutive non-command lines form a single text block.
        let result = parse_document("just some text");
        assert_eq!(result.textblocks.len(), 1);
        assert_eq!(result.textblocks.first().unwrap().content, "just some text");
    }

    #[test]
    fn adjacent_text_lines_merge_into_one_block() {
        // §6: consecutive non-command lines form a single text block.
        let result = parse_document("line one\nline two\nline three");
        assert_eq!(result.textblocks.len(), 1);
        assert_eq!(
            result.textblocks.first().unwrap().content,
            "line one\nline two\nline three"
        );
    }

    #[test]
    fn text_block_content_uses_physical_lines() {
        // ADR-NNNN: text block content preserves original physical lines.
        let result = parse_document("hello\nworld");
        assert_eq!(result.textblocks.first().unwrap().content, "hello\nworld");
    }

    #[test]
    fn text_block_range_covers_physical_lines() {
        // §6: text blocks use physical line numbers for their range.
        let result = parse_document("line one\nline two");
        let tb = result.textblocks.first().unwrap();
        assert_eq!(tb.range.start_line, 0);
        assert_eq!(tb.range.end_line, 1);
    }

    // --- Category 6: Interleaving commands and text ---

    #[test]
    fn text_before_command_is_captured() {
        // §6: non-command lines before a command form a text block.
        let result = parse_document("preamble\n/cmd arg");
        assert_eq!(result.textblocks.len(), 1);
        assert_eq!(result.textblocks.first().unwrap().content, "preamble");
        assert_eq!(result.commands.len(), 1);
    }

    #[test]
    fn text_after_command_is_captured() {
        // §6: new text block begins after a command is finalized.
        let result = parse_document("/cmd arg\npostamble");
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.textblocks.len(), 1);
        assert_eq!(result.textblocks.first().unwrap().content, "postamble");
    }

    #[test]
    fn text_between_commands_is_captured() {
        // §6: text between two commands forms its own block.
        let result = parse_document("/cmd1 a\nmiddle text\n/cmd2 b");
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.textblocks.len(), 1);
        assert_eq!(result.textblocks.first().unwrap().content, "middle text");
    }

    #[test]
    fn two_single_line_commands_both_parse() {
        // §7: multiple commands assigned sequential IDs.
        let result = parse_document("/cmd1 a\n/cmd2 b");
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.commands.first().unwrap().name, "cmd1");
        assert_eq!(result.commands.get(1).unwrap().name, "cmd2");
    }

    #[test]
    fn fence_followed_by_new_command() {
        // §4.1: after fence closes, parser returns to idle and scans for next command.
        let result = parse_document("/cmd1 ```\nbody\n```\n/cmd2 arg");
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.commands.first().unwrap().arguments.mode, ArgumentMode::Fence);
        assert_eq!(result.commands.get(1).unwrap().arguments.mode, ArgumentMode::SingleLine);
    }

    // --- Category 7: ID assignment ---

    #[test]
    fn command_ids_are_sequential_zero_based() {
        // §7: commands assigned cmd-0, cmd-1, cmd-2.
        let result = parse_document("/a x\n/b y\n/c z");
        assert_eq!(result.commands.first().unwrap().id, "cmd-0");
        assert_eq!(result.commands.get(1).unwrap().id, "cmd-1");
        assert_eq!(result.commands.get(2).unwrap().id, "cmd-2");
    }

    #[test]
    fn text_block_ids_are_sequential_zero_based() {
        // §7: text blocks assigned text-0, text-1, text-2.
        let result = parse_document("aaa\n/cmd x\nbbb\n/cmd y\nccc");
        assert_eq!(result.textblocks.first().unwrap().id, "text-0");
        assert_eq!(result.textblocks.get(1).unwrap().id, "text-1");
        assert_eq!(result.textblocks.get(2).unwrap().id, "text-2");
    }

    #[test]
    fn command_and_text_ids_are_independent() {
        // §7: command and text block ID sequences are independent.
        let result = parse_document("prose\n/cmd arg\nmore prose");
        assert_eq!(result.commands.first().unwrap().id, "cmd-0");
        assert_eq!(result.textblocks.first().unwrap().id, "text-0");
        assert_eq!(result.textblocks.get(1).unwrap().id, "text-1");
    }

    // --- Category 8: raw field ---

    #[test]
    fn single_line_raw_is_header_line_only() {
        // §8.2: single-line command raw contains only the original command line.
        let result = parse_document("/echo hello world");
        assert_eq!(result.commands.first().unwrap().raw, "/echo hello world");
    }

    #[test]
    fn joined_command_raw_includes_backslashes() {
        // §8.2 + ADR-NNNN: raw contains physical lines with backslashes and \n separators.
        // The physical_lines slice and join logic is exclusively in document_parse.rs.
        let result = parse_document("/deploy prod \\\n  --region us-west-2");
        assert_eq!(
            result.commands.first().unwrap().raw,
            "/deploy prod \\\n  --region us-west-2"
        );
    }

    #[test]
    fn fenced_raw_includes_opener_body_and_closer() {
        // §8.2: fenced command raw includes command line, body lines, and closing fence.
        let result = parse_document("/cmd ```\nbody\n```");
        assert_eq!(result.commands.first().unwrap().raw, "/cmd ```\nbody\n```");
    }

    // --- Category 10: Trailing newline edge case ---

    #[test]
    fn trailing_newline_does_not_create_empty_text_block() {
        // §2.1 + ADR-NNNN: split_physical_lines pops the trailing empty element.
        let result = parse_document("/cmd arg\n");
        assert_eq!(result.commands.len(), 1);
        assert!(result.textblocks.is_empty());
    }

    // --- Category 12: Version ---

    #[test]
    fn version_is_set() {
        // §8.1: envelope contains version field set to SPEC_VERSION.
        let result = parse_document("");
        assert_eq!(result.version, SPEC_VERSION);
    }

    // --- Property tests ---
    // Separated from watch mode via tdd feature flag.

    use proptest::prelude::*;

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn never_panics_on_arbitrary_input(input in "\\PC{0,500}") {
            // §8.1 (total function): parser always produces a valid envelope, never panics.
            let _ = parse_document(&input);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn version_is_always_spec_version(input in "\\PC{0,200}") {
            // §8.1: version field is always SPEC_VERSION regardless of input.
            let result = parse_document(&input);
            prop_assert_eq!(result.version, SPEC_VERSION);
        }
    }

}
