//! Orchestration tests for the parse.rs state machine.
//!
//! These test the DECISIONS and WIRING that parse.rs performs when
//! composing sub-modules. Sub-module behavior (classify, join, fence,
//! text, normalize, single_line) is covered by their own #[cfg(test)].
//! Cross-module composition (normalize→join, classify→fence) is covered
//! by integration_tests/.
//!
//! What lives here:
//!   - State transitions (Idle ↔ InFence)
//!   - flush_text timing
//!   - accumulate_text physical-line folding
//!   - raw field reconstruction (phys[first..last].join)
//!   - split_physical_lines edge cases
//!   - cmd_seq / text_seq counter management
//!   - ParseResult envelope (version, empty input)

use solidus_engine::{ArgumentMode, LineRange, SPEC_VERSION, parse_document};

// =========================================================================
// Envelope: ParseResult shape regardless of content
// Engine Spec §14, §3.1, §8.2
// =========================================================================

#[test]
fn empty_input_produces_empty_envelope() {
    // Engine Spec §14: version always set. §8.2: total function.
    let r = parse_document("");
    assert_eq!(r.version, SPEC_VERSION);
    assert!(r.commands.is_empty());
    assert!(r.textblocks.is_empty());
    assert!(r.warnings.is_empty());
}

#[test]
fn version_populated_on_any_input() {
    // Engine Spec §14: version is SPEC_VERSION regardless of content.
    let r = parse_document("/cmd arg");
    assert_eq!(r.version, SPEC_VERSION);
}

// =========================================================================
// split_physical_lines: trailing newline handling
// Engine Spec §5.2 (private to parse.rs)
// =========================================================================

#[test]
fn trailing_lf_does_not_create_empty_text_block() {
    // Engine Spec §5.2: trailing empty element popped after split.
    let r = parse_document("/cmd arg\n");
    assert_eq!(r.commands.len(), 1);
    assert!(r.textblocks.is_empty());
}

#[test]
fn single_lf_produces_one_empty_text_line() {
    // Engine Spec §5.2: "\n" → ["", ""], pop trailing → [""].
    let r = parse_document("\n");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "");
}

// =========================================================================
// State: Idle → single-line → Idle
// Orchestration: classify returns Command(SingleLine), finalize_single_line
// called, state remains Idle for next iteration.
// =========================================================================

#[test]
fn single_line_command_returns_to_idle() {
    // Two consecutive single-line commands prove Idle→Idle loop works.
    let r = parse_document("/cmd1 a\n/cmd2 b");
    assert_eq!(r.commands.len(), 2);
    assert_eq!(r.commands[0].arguments.mode, ArgumentMode::SingleLine);
    assert_eq!(r.commands[1].arguments.mode, ArgumentMode::SingleLine);
}

// =========================================================================
// State: Idle → InFence → Idle
// Orchestration: classify returns Command(Fence), open_fence called,
// state becomes InFence. Closer detected → finalize_fence, back to Idle.
// =========================================================================

#[test]
fn fence_completes_and_returns_to_idle() {
    // Fence followed by another command proves Idle restored after fence.
    let r = parse_document("/fenced ```\nbody\n```\n/after arg");
    assert_eq!(r.commands.len(), 2);
    assert_eq!(r.commands[0].arguments.mode, ArgumentMode::Fence);
    assert_eq!(r.commands[1].arguments.mode, ArgumentMode::SingleLine);
}

// =========================================================================
// State: InFence → EOF (unclosed)
// Orchestration: loop breaks during InFence, finalize_fence(true) called.
// =========================================================================

#[test]
fn unclosed_fence_at_eof_produces_warning_and_partial() {
    // RFC §5.2.4: unclosed_fence warning + accumulated payload.
    let r = parse_document("/cmd ```\npartial body");
    assert_eq!(r.commands.len(), 1);
    assert_eq!(r.commands[0].arguments.payload, "partial body");
    assert_eq!(r.warnings.len(), 1);
    assert_eq!(r.warnings[0].wtype, "unclosed_fence");
}

#[test]
fn unclosed_fence_with_no_body_at_eof() {
    // RFC §5.2.4: fence opened then immediate EOF → empty payload + warning.
    let r = parse_document("/cmd ```");
    assert_eq!(r.commands[0].arguments.payload, "");
    assert_eq!(r.warnings.len(), 1);
}

// =========================================================================
// flush_text timing
// Orchestration: flush_text called BEFORE start_new_command (on command
// trigger) and AFTER main loop (at EOF).
// =========================================================================

#[test]
fn text_flushed_before_command_trigger() {
    // Orchestration: text must be finalized before the command is created.
    let r = parse_document("prose\n/cmd arg");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "prose");
    assert_eq!(r.textblocks[0].id, "text-0");
    assert_eq!(r.commands[0].id, "cmd-0");
}

#[test]
fn text_flushed_at_eof() {
    // Orchestration: trailing text finalized after loop breaks.
    let r = parse_document("/cmd arg\ntrailing");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "trailing");
}

#[test]
fn no_spurious_text_between_consecutive_commands() {
    // Orchestration: flush_text no-ops when current_text is None.
    let r = parse_document("/a x\n/b y\n/c z");
    assert_eq!(r.commands.len(), 3);
    assert!(r.textblocks.is_empty());
}

#[test]
fn no_spurious_text_after_fence_closer() {
    // Fence closer → Idle, immediate command → no empty text block.
    let r = parse_document("/f ```\n```\n/cmd arg");
    assert_eq!(r.commands.len(), 2);
    assert!(r.textblocks.is_empty());
}

#[test]
fn text_after_fence_forms_separate_block() {
    // RFC §6.4: text after command gets its own block.
    let r = parse_document("/f ```\nbody\n```\ntrailing text");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "trailing text");
}

// =========================================================================
// accumulate_text: physical-line folding
// Orchestration: text blocks use physical lines (pre-join), not logical.
// This is parse.rs-specific logic via fold_physical_lines.
// =========================================================================

#[test]
fn text_block_preserves_backslash_as_physical_lines() {
    // Engine Spec §10: text content stores physical lines verbatim.
    // The backslash is NOT a join marker for text — it's literal content.
    let r = parse_document("hello \\\nworld");
    assert_eq!(r.textblocks[0].content, "hello \\\nworld");
}

#[test]
fn text_block_range_covers_physical_lines() {
    // Engine Spec §3.6: range spans all physical lines of the text block.
    let r = parse_document("line one\nline two\nline three");
    assert_eq!(r.textblocks[0].range.start_line, 0);
    assert_eq!(r.textblocks[0].range.end_line, 2);
}

#[test]
fn multi_line_text_joined_with_lf() {
    // RFC §6.1 + Engine Spec §10.3: content = lines.join("\n").
    let r = parse_document("a\nb\nc");
    assert_eq!(r.textblocks[0].content, "a\nb\nc");
}

// =========================================================================
// raw field reconstruction
// Orchestration: step_idle builds raw via phys[first..last].join("\n").
// This is glue code in parse.rs, not tested by sub-modules.
// =========================================================================

#[test]
fn single_line_raw_is_source_text() {
    // RFC §7.1: raw = exact source text.
    let r = parse_document("/echo hello world");
    assert_eq!(r.commands[0].raw, "/echo hello world");
}

#[test]
fn joined_command_raw_preserves_backslashes_and_newlines() {
    // RFC §7.1: raw includes physical lines with backslashes and LF.
    // Orchestration: header.raw = phys[first..last].join("\n").
    let r = parse_document("/deploy \\\n--region us-west-2");
    assert_eq!(r.commands[0].raw, "/deploy \\\n--region us-west-2");
}

#[test]
fn fenced_raw_includes_opener_body_closer() {
    // RFC §7.1: raw = opener + body + closer, all joined with LF.
    let r = parse_document("/cmd ```\nbody\n```");
    assert_eq!(r.commands[0].raw, "/cmd ```\nbody\n```");
}

// =========================================================================
// Counter management: cmd_seq and text_seq
// Orchestration: independent zero-based counters incremented in
// start_new_command and flush_text respectively.
// =========================================================================

#[test]
fn command_ids_sequential_across_modes() {
    // RFC §6.5: cmd-0, cmd-1, cmd-2 regardless of single-line vs fence.
    let r = parse_document("/a x\n/b ```\nbody\n```\n/c z");
    assert_eq!(r.commands[0].id, "cmd-0");
    assert_eq!(r.commands[1].id, "cmd-1");
    assert_eq!(r.commands[2].id, "cmd-2");
}

#[test]
fn text_ids_sequential() {
    // RFC §6.5: text-0, text-1.
    let r = parse_document("first\n/cmd x\nsecond");
    assert_eq!(r.textblocks[0].id, "text-0");
    assert_eq!(r.textblocks[1].id, "text-1");
}

#[test]
fn command_and_text_counters_independent() {
    // Engine Spec §3.2 + §3.5: cmd and text sequences are independent.
    let r = parse_document("prose\n/cmd arg\nmore prose");
    assert_eq!(r.commands[0].id, "cmd-0");
    assert_eq!(r.textblocks[0].id, "text-0");
    assert_eq!(r.textblocks[1].id, "text-1");
}

// =========================================================================
// Range wiring: orchestrator passes correct physical line indices
// Engine Spec §3.6
// =========================================================================

#[test]
fn single_line_command_range() {
    // Orchestration: first_physical == last_physical for non-joined command.
    let r = parse_document("text\n/cmd arg");
    assert_eq!(r.commands[0].range, LineRange { start_line: 1, end_line: 1 });
}

#[test]
fn joined_command_range_spans_physical_lines() {
    // Orchestration: range from logical line's first/last physical.
    let r = parse_document("/deploy \\\n--region \\\nus-west-2");
    assert_eq!(r.commands[0].range, LineRange { start_line: 0, end_line: 2 });
}

#[test]
fn fenced_command_range_includes_closer() {
    // Engine Spec §3.6: range covers opener through closer.
    let r = parse_document("/cmd ```\nbody\n```");
    assert_eq!(r.commands[0].range, LineRange { start_line: 0, end_line: 2 });
}

// =========================================================================
// CRLF normalization threading
// Orchestration: normalize() called before split_physical_lines.
// Proves the orchestrator's first pipeline step works.
// =========================================================================

#[test]
fn crlf_normalized_before_processing() {
    // RFC §3.1: CRLF → LF before any other processing.
    let r = parse_document("/cmd arg\r\ntext line");
    assert_eq!(r.commands.len(), 1);
    assert_eq!(r.textblocks[0].content, "text line");
}

// =========================================================================
// Full interleaving: mixed document
// Orchestration: all state transitions, flush timing, and counters
// exercised in a single realistic scenario.
// =========================================================================

#[test]
fn full_mixed_document() {
    // Preamble text → single-line cmd → fenced cmd → trailing text.
    let input = "Preamble.\n/cmd1 hello\n/cmd2 ```json\n{\"a\":1}\n```\nEpilogue.";
    let r = parse_document(input);

    assert_eq!(r.textblocks.len(), 2);
    assert_eq!(r.textblocks[0].content, "Preamble.");
    assert_eq!(r.textblocks[0].id, "text-0");
    assert_eq!(r.textblocks[1].content, "Epilogue.");
    assert_eq!(r.textblocks[1].id, "text-1");

    assert_eq!(r.commands.len(), 2);
    assert_eq!(r.commands[0].id, "cmd-0");
    assert_eq!(r.commands[0].arguments.mode, ArgumentMode::SingleLine);
    assert_eq!(r.commands[1].id, "cmd-1");
    assert_eq!(r.commands[1].arguments.mode, ArgumentMode::Fence);
    assert_eq!(r.commands[1].arguments.payload, "{\"a\":1}");

    assert!(r.warnings.is_empty());
}

// =========================================================================
// Trailing backslash at EOF
// Orchestration: joiner strips trailing backslash when no next line
// exists. State machine must not emit a warning for this case.
// Engine Spec §7.1, RFC §2.2.2
// =========================================================================

#[test]
fn trailing_backslash_at_eof_stripped_silently() {
    // §2.2.2: trailing backslash at EOF is removed, no warning emitted.
    // Orchestration: joiner consumes the backslash, step_idle receives
    // the stripped logical line, classifies it as single-line command.
    let r = parse_document("/echo hello\\");
    assert_eq!(r.commands.len(), 1);
    assert_eq!(r.commands[0].arguments.payload, "hello");
    assert!(r.warnings.is_empty());
}

// --- Text accumulation edge cases ---
// Orchestration: accumulatetext and flushtext handle text-only, whitespace-only,
// blank-line-containing, and between-commands scenarios correctly.

#[test]
fn whitespace_only_input_is_text() {
    // Orchestration: classify returns Text for whitespace-only lines,
    // accumulatetext collects them, flushtext emits one text block at EOF.
    // RFC §6.3: whitespace-only lines are text lines.
    let r = parse_document("   ");
    assert!(r.commands.is_empty());
    assert_eq!(r.textblocks.len(), 1);
}

#[test]
fn text_only_input_produces_single_block() {
    // Orchestration: when every logical line classifies as Text, the state
    // machine never leaves Idle and all lines accumulate into one block.
    // RFC §6.4: consecutive text lines form a single text block.
    // RFC §7.2: content is lines joined with LF.
    let r = parse_document("line one\ntwo\nthree");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "line one\ntwo\nthree");
    assert_eq!(r.textblocks[0].range.start_line, 0);
    assert_eq!(r.textblocks[0].range.end_line, 2);
}

#[test]
fn text_between_commands_forms_own_block() {
    // Orchestration: flushtext is called before each command trigger,
    // so text sandwiched between two single-line commands becomes its
    // own block with the correct id and content.
    // RFC §6.4: text between two commands forms its own block.
    let r = parse_document("/cmd1 a\nmiddle text\n/cmd2 b");
    assert_eq!(r.commands.len(), 2);
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "middle text");
}

#[test]
fn blank_lines_do_not_split_text_block() {
    // Orchestration: blank lines classify as Text, so accumulatetext
    // keeps appending them to the current pending block. No flush occurs.
    // RFC §6.4: blank lines within a text region are included in the text block.
    let r = parse_document("a\n\n\na");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "a\n\n\na");
}

// --- splitphysicallines edge cases ---
// Orchestration: splitphysicallines (private to parse.rs) determines how
// normalized input becomes physical lines. These verify boundary behaviors
// that propagate through the full pipeline.

#[test]
fn double_lf_produces_two_empty_text_lines() {
    // Orchestration: "\n\n" splits to ["", "", ""], trailing pop → ["", ""].
    // Both empty strings classify as Text, accumulate into one block.
    // Engine Spec §5.2.
    let r = parse_document("\n\n");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "\n");
}

#[test]
fn no_newline_input_is_single_physical_line() {
    // Orchestration: "abc" splits to ["abc"], no trailing empty to pop.
    // Single text line produces one text block.
    // Engine Spec §5.2.
    let r = parse_document("abc");
    assert_eq!(r.textblocks.len(), 1);
    assert_eq!(r.textblocks[0].content, "abc");
}

// =========================================================================
// Property tests
// =========================================================================

use proptest::prelude::*;

proptest! {
    // Engine Spec §8.2: total function, never panics on any input.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn never_panics_on_arbitrary_input(input in "\\PC{0,500}") {
        let _ = parse_document(&input);
    }

    // Engine Spec §14: version is always SPEC_VERSION.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn version_always_spec_version(input in "\\PC{0,200}") {
        let r = parse_document(&input);
        prop_assert_eq!(r.version, SPEC_VERSION);
    }

    // Orchestration invariant: commands + textblocks partition all
    // non-empty physical lines. No line is lost or double-counted.
    // Orchestration invariant: commands + textblocks partition all
    // non-empty physical lines. No line is lost or double-counted.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn output_items_cover_all_lines(input in "[a-zA-Z0-9/ `\n]{1,300}") {
        let r = parse_document(&input);
        let line_count = input.lines().count().max(1);
        let mut covered = vec![false; line_count];

        let lines: Vec<usize> = r.commands.iter().map(|c| &c.range)
            .chain(r.textblocks.iter().map(|t| &t.range))
            .flat_map(|range| range.start_line..=range.end_line)
            .filter(|&i| i < line_count)
            .collect();

        for i in lines {
            prop_assert!(!covered[i], "line {} double-covered", i);
            covered[i] = true;
        }
    }


    // Orchestration invariant: command IDs are always sequential.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn command_ids_always_sequential(input in "[a-zA-Z0-9/ `\n]{0,300}") {
        let r = parse_document(&input);
        for (i, cmd) in r.commands.iter().enumerate() {
            prop_assert_eq!(&cmd.id, &format!("cmd-{i}"));
        }
    }

    // Orchestration invariant: text IDs are always sequential.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn text_ids_always_sequential(input in "[a-zA-Z0-9/ `\n]{0,300}") {
        let r = parse_document(&input);
        for (i, tb) in r.textblocks.iter().enumerate() {
            prop_assert_eq!(&tb.id, &format!("text-{i}"));
        }
    }

        // Orchestration invariant: input with no slash-prefixed lines
    // produces zero commands and at least one text block.
    // §6 + §7: non-command lines are always collected as text.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn text_only_input_produces_zero_commands(
        lines in prop::collection::vec("[^/\\n\\r][^\\n\\r]{0,40}", 1..10)
    ) {
        let input = lines.join("\n");
        let r = parse_document(&input);
        prop_assert_eq!(r.commands.len(), 0);
        prop_assert!(!r.textblocks.is_empty());
    }

    // Orchestration invariant: fence body passes through the state
    // machine verbatim. No joining, no escaping, no transformation.
    // §5.2.2: fence body content is opaque payload.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn fenced_content_preserved_verbatim(body in "[^`\\n\\r]{1,80}") {
        let input = format!("/cmd ```\n{}\n```", body);
        let r = parse_document(&input);
        prop_assert_eq!(r.commands.len(), 1);
        prop_assert_eq!(&r.commands.first().unwrap().arguments.payload, &body);
    }

    // Orchestration invariant: any fence reaching EOF without a closer
    // always produces exactly one warning. The state machine must
    // call finalize_fence(true) when the loop breaks during InFence.
    // §5.2.4: unclosed_fence warning is mandatory.
    #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn unclosed_fence_always_produces_warning(body in "[^`]{0,80}") {
        let input = format!("/cmd ```\n{}", body);
        let r = parse_document(&input);
        prop_assert_eq!(r.commands.len(), 1);
        prop_assert_eq!(r.warnings.len(), 1);
        prop_assert_eq!(&r.warnings.first().unwrap().wtype, "unclosed_fence");
    }


}
