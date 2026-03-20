---
status: accepted
date: 2026-03-19
decision-makers:
  - Tom
consulted: []
informed: []
---

# Document Parse Pipeline: Pull-Model LineJoiner with Two-State Loop

## Context and Problem Statement

The `document_parse.rs` module is the top-level orchestrator of the slash-parser v0.3.0 engine. It
must wire together normalization (§2.1), POSIX-style backslash line joining (§2.2), command
detection and classification (§3), a two-state machine (§4), fenced payload accumulation (§5.2),
text block collection (§6), sequential ID assignment (§7), and correct `raw` field construction
(§8.2), all within a single forward pass (§1.1) that supports incremental emission (§9.2).

The fundamental tension is that the spec describes line joining as a "pre-pass" (§2.2) but mandates
fence immunity (§2.3): lines inside a fenced block must never be subject to joining. Fence
boundaries are only discoverable during command parsing, so a naive eager pre-pass is impossible
without either a second pass or buffering the entire input.

How should `document_parse.rs` orchestrate the pipeline to satisfy all spec requirements in a single
forward pass?

## Decision Drivers

- Spec §2.3 (fence immunity) and §2.2 (line joining) create a circular dependency: joining must
  happen before parsing, but fence boundaries are only known during parsing.
- Spec §9.2 requires incremental emission: each command or text block must be finalizable as soon as
  its last physical line is consumed. Memory must be bounded by the largest single element, not
  total input size.
- Spec §1.1 requires a single forward pass after normalization. No backtracking.
- Spec §8.2 requires `raw` to contain the exact normalized source text before line joining,
  including backslash characters and `\n` separators between physical lines.
- Spec §6 and §9.4 (round-trip fidelity) require text block content and ranges to use physical line
  numbers and physical line content so a formatter can reconstruct the original input.
- Spec §7 requires independent zero-based ID sequences for commands (`cmd-0`, `cmd-1`) and text
  blocks (`text-0`, `text-1`).
- Architecture doc §9.2 recommends an explicit loop over iterator chains when the source of data
  changes mid-stream.
- Architecture doc §9.1 notes that inputs are small (chatops messages, git comments), favoring
  clarity over allocation avoidance.

## Considered Options

1. Pull-model LineJoiner with explicit two-state `while` loop
2. Pure iterator chain with `scan`/`flat_map` adapters
3. Two-pass approach: locate fence boundaries first, then join outside them
4. Zero-copy slice tracking instead of allocated `LogicalLine` strings
5. Mutable `String` builder for `raw` field construction
6. Single shared counter for command and text block IDs
7. Text block content from joined logical lines (post-join text)
8. Text block content from raw physical lines (pre-join text)

## Decision Outcome

Chosen options:

- **Option 1** for the overall pipeline architecture.
- **Option 8** for text block content representation.

Combined, these produce a pipeline where:

1. `normalize(input)` converts all line endings to LF (§2.1).
2. The normalized string is split on `\n` into `physical_lines: Vec<&str>`, retained for `raw`
   reconstruction and text block content.
3. An owned copy is passed to `LineJoiner::new(owned)`.
4. A `while` loop drives a `ParserState` enum (`Idle` | `InFence(PendingCommand)`).
5. In `Idle`, `joiner.next_logical()` produces joined `LogicalLine` values. Each is classified.
   Commands start accumulation. Text lines feed physical line content into `PendingText` via
   `start_text`/`append_text`.
6. In `InFence`, `joiner.next_physical()` produces raw lines. Each is fed to `accept_line`. On close
   or EOF, the command is finalized and the state returns to `Idle`.
7. Before starting a command, `header.raw` is overwritten with
   `physical_lines[first..=last].join("\n")` to satisfy §8.2.
8. Independent `cmd_seq: usize` and `text_seq: usize` counters are maintained and passed to
   `start_command` and `finalize_text` respectively.
9. At EOF, any pending text block is finalized. The result is returned with no `context` field (SDKs
   inject that).

### Consequences

**Good:**

- Fence immunity is achieved without a second pass. The shared cursor inside `LineJoiner` means
  calling `next_physical()` in `InFence` bypasses joining for exactly the lines inside the fence,
  then `next_logical()` resumes joining when the state returns to `Idle`.
- Incremental emission is preserved. Each command and text block is finalized as soon as its last
  line is consumed. No global buffering beyond the current pending element.
- The fence-opener edge case (§5.2.6) resolves implicitly. When backslash joining merges a command
  line with a line containing a fence opener, `next_logical()` produces the merged text and
  `classify_line` detects the fence naturally. No special-casing in the orchestrator.
- `raw` is always correct. Slicing from the retained `physical_lines` vector guarantees the exact
  normalized source text including backslashes.
- Text block content stores physical lines, making round-trip reconstruction (§9.4) straightforward.
  A formatter can use `content` directly without needing to reverse the joining process.
- Two independent ID counters match the spec's independent sequences exactly.

**Neutral:**

- The `physical_lines: Vec<&str>` borrows from the normalized string, which must live for the
  duration of the parse. This is trivially satisfied since `parse_document` owns the normalized
  string on the stack.
- An owned `Vec<String>` copy is created for the `LineJoiner`. This doubles memory for the line
  storage, but inputs are small and the clarity benefit is significant.

**Bad:**

- Text block content containing backslash-continued lines will include the trailing backslash on
  each physical line. A consumer expecting "clean" joined text from text blocks would need to
  re-join. This is the correct behavior for round-trip fidelity but may surprise consumers who
  expect text blocks to contain joined content. This tradeoff is accepted because ranges already use
  physical line numbers, and consistency between range and content is more important than consumer
  convenience.
- The `LineJoiner` allocates a new `String` for every logical line produced by `next_logical()`. For
  inputs with many non-continued lines, this is one allocation per line. Acceptable for
  chatops-scale inputs.

## Pros and Cons of the Options

### Option 1: Pull-model LineJoiner with explicit two-state `while` loop

The `LineJoiner` exposes `next_logical()` (joins) and `next_physical()` (raw) on a shared cursor.
The orchestrator drives it from a `while` loop matching on `ParserState`.

- Good, because the state machine directly controls which consumption mode is active, making fence
  immunity explicit and auditable.
- Good, because the shared cursor guarantees no lines are skipped or double-consumed at the
  Idle/InFence transition boundary.
- Good, because an explicit loop is more readable than iterator chains when the data source changes
  mid-stream (architecture doc §9.2).
- Good, because incremental emission is trivially satisfied since each element is finalized inline.
- Neutral, because it requires the orchestrator to own the loop and match arms, which is more code
  than a fold but more explicit.

### Option 2: Pure iterator chain with `scan`/`flat_map` adapters

Express the entire pipeline as a chain of iterator adapters, using `scan` to carry `ParserState` and
`flat_map` to switch between logical and physical line sources.

- Good, because it would be concise and leverage Rust's iterator optimizations (lazy evaluation,
  potential vectorization).
- Bad, because switching the source of lines (logical vs physical) mid-stream requires either
  duplicating the joiner or using interior mutability and `Peekable` with complex lookahead.
- Bad, because lifetime and borrowing constraints make it difficult to hold a mutable reference to
  the joiner inside a `scan` closure while also producing items.
- Bad, because the control flow for fence immunity (switching consumption modes) is obscured inside
  adapter closures, making the code harder to audit against the spec.

### Option 3: Two-pass approach (locate fence boundaries first, then join)

First pass: scan all physical lines to identify fence-open/close pairs and their line ranges. Second
pass: apply line joining only to regions outside fences, then run the state machine on the mixed
result.

- Good, because fence immunity is guaranteed by construction. The joiner only ever sees non-fence
  lines.
- Bad, because it violates §9.2 (incremental emission). The first pass must buffer all fence
  boundary positions before any command can be emitted.
- Bad, because identifying fence boundaries requires knowing which lines are command lines (to find
  fence openers), which means the first pass duplicates much of the classification logic.
- Bad, because it requires two iterations over the input, violating the single-forward-pass
  principle (§1.1).

### Option 4: Zero-copy slice tracking

Represent joined logical lines as `Vec<(start_byte, end_byte)>` ranges into the normalized input
string, avoiding `String` allocation entirely.

- Good, because it eliminates allocation for joined lines, potentially improving throughput on large
  inputs.
- Bad, because backslash removal creates non-contiguous content. A joined line spanning physical
  lines `"a\"` and `"b"` maps to bytes `[0..1]` + space + `[3..4]`, requiring a non-contiguous slice
  assembly that complicates all downstream consumers.
- Bad, because `classify_line` and other downstream functions expect `&str`, requiring either
  on-the-fly assembly or API changes throughout the pipeline.
- Neutral, because inputs are small (chatops messages, git comments). Profiling has not shown
  allocation as a bottleneck. Documented as a future optimization path if profiling reveals a need.

### Option 5: Mutable `String` builder for `raw` field

Maintain a mutable `String` that accumulates physical lines (with `\n` separators) as they are
consumed, becoming the `raw` field when the command is finalized.

- Good, because it avoids the post-hoc slice-and-join from `physical_lines`.
- Bad, because the builder must be reset for each command and coordinated with the transition
  between Idle and InFence states. For fenced commands, `accept_line` already accumulates
  `raw_lines` on `PendingCommand`, so the builder would duplicate that responsibility.
- Bad, because it obscures the source of truth. The spec defines `raw` as the exact normalized
  source text, which is more clearly expressed by slicing the immutable `physical_lines` vector.

### Option 6: Single shared counter for command and text block IDs

Use one global counter and assign IDs like `cmd-0`, `text-1`, `cmd-2` in encounter order.

- Good, because it simplifies bookkeeping to a single `usize`.
- Bad, because it violates §7, which specifies that command IDs and text block IDs are independent
  zero-based sequences. The spec examples show `cmd-0` and `text-0` coexisting, not `cmd-0` and
  `text-1`.

### Option 7: Text block content from joined logical lines

Store the `LogicalLine.text` (post-join, backslash removed) as text block content.

- Good, because consumers see "clean" text without trailing backslashes or continuation artifacts.
- Bad, because the range already uses physical line numbers. A text block with
  `range: {start_line: 0, end_line: 1}` but content of one joined line creates an inconsistency: the
  range says two physical lines but the content has no `\n` separator.
- Bad, because round-trip reconstruction (§9.4) becomes lossy. A formatter cannot distinguish
  between a two-physical-line text region that was joined (backslash removed) and a single physical
  line, because both would produce the same content string.
- Bad, because it creates an asymmetry with commands: `command.raw` stores physical lines (pre-join)
  but `textblock.content` would store logical lines (post-join).

### Option 8: Text block content from raw physical lines

Feed `physical_lines[idx]` (pre-join, backslashes retained) into `start_text`/`append_text` for each
physical line covered by a logical line.

- Good, because content and range are consistent. A range spanning physical lines 0-1 will always
  have content with exactly one `\n` separator.
- Good, because round-trip reconstruction is lossless. The formatter can reproduce the exact
  normalized input from the content and range of each text block and command.
- Good, because it is symmetric with `command.raw`, which also stores physical lines.
- Neutral, because text block content for continuation lines will contain trailing backslashes.
  Consumers expecting "clean" text must handle this, but the spec does not promise that text blocks
  strip join markers.

## More Information

### Spec references

- §1.1 Design Principles (single forward pass, incremental emission, deterministic, total)
- §2.1 Line Ending Normalization
- §2.2 Line Joining (Backslash Continuation)
- §2.2.1 Physical Line Tracking
- §2.2.2 Trailing Backslash at EOF
- §2.3 Fence Immunity
- §3 Command Detection
- §4 Parser States (Idle, InFence)
- §5.1 Single-Line Mode
- §5.2 Fence Mode (opener, body, lifetime, closer, unclosed, joining around fences)
- §6 Text Blocks
- §7 Multiple Commands and Ordering (independent ID sequences)
- §8.2 The `raw` Field
- §9.2 Incremental Emission
- §9.4 Roundtrip Fidelity Invariant

### Architecture references

- parser-architecture.md §3.2 `documentparse.rs` pipeline description
- parser-architecture.md §5 State Machine (Idle, InFence)
- parser-architecture.md §9.1 Logical Line Intermediate Type (allocated vs zero-copy)
- parser-architecture.md §9.2 Iterator vs Imperative Loop

### Design council findings

Three independent model analyses were conducted. All three converged on Option 1 (pull-model with
explicit loop) and confirmed that Option 3 (two-pass) violates incremental emission. The key
divergence was around text block content: one model identified a potential bug where physical lines
with trailing backslashes would appear in text block content, which after analysis was determined to
be the correct behavior for round-trip fidelity (Option 8). Another model independently confirmed
that the fence-opener edge case (§5.2.6) resolves implicitly through separation of concerns between
the joiner and classifier, requiring no special handling in the orchestrator.
