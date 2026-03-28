use super::{
    classify::{CommandHeader, LineKind, classify_line},
    fence::{FenceResult, PendingFence, accept_fence_line, finalize_fence, open_fence},
    join::LineJoiner,
    normalize::normalize,
    single_line::finalize_single_line,
    text::{PendingText, append_text, finalize_text, start_text},
};
use crate::{ArgumentMode, Command, LineRange, ParseResult, SPEC_VERSION, TextBlock, Warning};

// --- Types ---

#[derive(Debug)]
enum ParserState {
    Idle,
    InFence(PendingFence),
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
            start_new_command(ctx, header, ll.first_physical, ll.last_physical);
        }
        LineKind::Text => {
            accumulate_text(ctx, ll.first_physical, ll.last_physical, phys);
        }
    }
    LoopAction::Continue
}

fn step_in_fence(ctx: &mut ParseCtx, joiner: &mut LineJoiner, fence: PendingFence) -> LoopAction {
    let Some((line_idx, line)) = joiner.next_physical() else {
        let (cmd, warnings) = finalize_fence(fence, true);
        ctx.commands.push(cmd);
        ctx.warnings.extend(warnings);
        return LoopAction::Break;
    };
    let (updated, result) = accept_fence_line(fence, line_idx, &line);
    match result {
        FenceResult::Consumed => {
            ctx.state = ParserState::InFence(updated);
        }
        FenceResult::Completed => {
            let (cmd, warnings) = finalize_fence(updated, false);
            ctx.commands.push(cmd);
            ctx.warnings.extend(warnings);
        }
    }
    LoopAction::Continue
}

// --- Context helpers ---

fn start_new_command(ctx: &mut ParseCtx, header: CommandHeader, first_physical: usize, last_physical: usize) {
    let raw = header.raw.clone();
    let range = LineRange { start_line: first_physical, end_line: last_physical };
    match header.mode {
        ArgumentMode::SingleLine => {
            let cmd = finalize_single_line(header, raw, ctx.cmd_seq, range);
            ctx.commands.push(cmd);
            ctx.cmd_seq += 1;
        }
        ArgumentMode::Fence => {
            let fence = open_fence(header, raw, ctx.cmd_seq, range);
            ctx.cmd_seq += 1;
            ctx.state = ParserState::InFence(fence);
        }
    }
}

fn flush_text(ctx: &mut ParseCtx) {
    let Some(text) = ctx.current_text.take() else {
        return;
    };
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

fn fold_physical_lines(mut text: PendingText, from: usize, to: usize, phys: &[&str]) -> PendingText {
    for (idx, line) in (from..=to).zip(&phys[from..=to]) {
        text = append_text(text, idx, line);
    }
    text
}

// parse.rs is the public orchestrator: it composes normalize, join,
// classify, singleline, fence, and text into parse_document. Each
// sub-module carries its own unit tests. Orchestration logic (state
// transitions, flush timing, counter management) is covered by
// integration tests in tests/orchestration_tests.rs and
// tests/integration_tests/.

#[cfg(test)]
mod tests {
    use super::{ParseCtx, accumulate_text, flush_text, fold_physical_lines, split_physical_lines};
    use crate::{SPEC_VERSION, text::start_text};

    // --- split_physical_lines ---
    // RFC §3.1: the normalized input is split on LF to produce a
    // sequence of physical lines. A trailing LF produces a trailing
    // empty line, which split_physical_lines pops.

    #[test]
    fn split_empty_string_produces_empty_vec() {
        // "" -> [""] -> popped -> []
        assert_eq!(split_physical_lines(""), Vec::<&str>::new());
    }

    #[test]
    fn split_no_trailing_newline_kept() {
        // "abc" -> ["abc"], no trailing empty element to pop
        assert_eq!(split_physical_lines("abc"), vec!["abc"]);
    }

    #[test]
    fn split_trailing_newline_popped() {
        // "abc\n" -> ["abc", ""] -> popped -> ["abc"]
        assert_eq!(split_physical_lines("abc\n"), vec!["abc"]);
    }

    #[test]
    fn split_single_newline_produces_one_empty_line() {
        // "\n" -> ["", ""] -> popped -> [""]
        assert_eq!(split_physical_lines("\n"), vec![""]);
    }

    #[test]
    fn split_multiple_newlines_popped_once() {
        // "a\n\n" -> ["a", "", ""] -> popped -> ["a", ""]
        assert_eq!(split_physical_lines("a\n\n"), vec!["a", ""]);
    }

    // --- fold_physical_lines ---
    // Folds a slice of physical lines into a PendingText, appending
    // each line via text::append_text. Pure parse.rs helper.

    #[test]
    fn folds_multiple_lines_into_pending_text() {
        let initial = start_text(0, "line zero");
        let phys = vec!["line zero", "line one", "line two", "line three"];
        let result = fold_physical_lines(initial, 1, 2, &phys);
        assert_eq!(result.start_line, 0);
        assert_eq!(result.end_line, 2);
        assert_eq!(result.lines, vec!["line zero", "line one", "line two"]);
    }

    // --- accumulate_text ---
    // Mutates ParseCtx.current_text. Creates a new PendingText when
    // None, appends to existing when Some.

    #[test]
    fn accumulate_text_creates_new_block_when_none() {
        let mut ctx = ParseCtx::new();
        let phys = vec!["line 0", "line 1"];
        accumulate_text(&mut ctx, 0, 1, &phys);
        let text = ctx.current_text.unwrap();
        assert_eq!(text.start_line, 0);
        assert_eq!(text.end_line, 1);
        assert_eq!(text.lines, vec!["line 0", "line 1"]);
    }

    #[test]
    fn accumulate_text_appends_to_existing_block() {
        let mut ctx = ParseCtx::new();
        ctx.current_text = Some(start_text(0, "line 0"));
        let phys = vec!["line 0", "line 1", "line 2"];
        accumulate_text(&mut ctx, 1, 2, &phys);
        let text = ctx.current_text.unwrap();
        assert_eq!(text.start_line, 0);
        assert_eq!(text.end_line, 2);
        assert_eq!(text.lines, vec!["line 0", "line 1", "line 2"]);
    }

    // --- flush_text ---
    // Drains ParseCtx.current_text into textblocks, assigns the
    // next sequential id, increments textseq.

    #[test]
    fn flush_text_pushes_block_and_increments_seq() {
        // RFC §6.5: text blocks assigned sequential zero-based IDs.
        let mut ctx = ParseCtx::new();
        ctx.current_text = Some(start_text(0, "line 0"));
        flush_text(&mut ctx);
        assert!(ctx.current_text.is_none());
        assert_eq!(ctx.textblocks.len(), 1);
        assert_eq!(ctx.textblocks[0].id, "text-0");
        assert_eq!(ctx.text_seq, 1);
    }

    #[test]
    fn flush_text_noops_when_empty() {
        let mut ctx = ParseCtx::new();
        flush_text(&mut ctx);
        assert!(ctx.textblocks.is_empty());
        assert_eq!(ctx.text_seq, 0);
    }

    // --- ParseCtx::into_result ---

    #[test]
    fn parse_ctx_into_result_injects_version() {
        let ctx = ParseCtx::new();
        let result = ctx.into_result();
        assert_eq!(result.version, SPEC_VERSION);
        assert!(result.commands.is_empty());
        assert!(result.textblocks.is_empty());
        assert!(result.warnings.is_empty());
    }
}
