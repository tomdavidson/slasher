---
status: accepted
date: 2026-03-19
updated: 2026-03-21
decision-makers:
  - Tom Davidson
consulted: []
informed: []
---

# Document Parse Pipeline: Pull-Model LineJoiner with Two-State Loop

## Context and Problem Statement

The `document_parse.rs` module is the top-level orchestrator of the slash-parser engine. It must wire
together normalization ([SYNTAX-RFC] §3.2), true POSIX backslash line joining ([SYNTAX-RFC] §4),
command detection and classification ([SYNTAX-RFC] §5), a two-state machine ([SYNTAX-RFC] §7.1),
fenced payload accumulation ([SYNTAX-RFC] §6.2), text block collection ([SYNTAX-RFC] §7.3),
sequential ID assignment ([SYNTAX-RFC] §8), and correct `raw` field construction ([ENGINE-SPEC] §9),
all within a single forward pass ([SYNTAX-RFC] §1) that supports incremental emission
([SYNTAX-RFC] §10.4).

The fundamental tension is that the syntax RFC describes line joining as conceptually preceding
command parsing ([SYNTAX-RFC] §4) but mandates fence immunity ([SYNTAX-RFC] §4.3): lines inside a
fenced block must never be subject to joining. Fence boundaries are only discoverable during command
parsing, so a naive eager pre-pass is impossible without either a second pass or buffering the entire
input.

How should `document_parse.rs` orchestrate the pipeline to satisfy all requirements in a single
forward pass?

## Decision Drivers

- [SYNTAX-RFC] §4.3 (fence immunity) and §4 (line joining) create an apparent circular dependency:
  joining must happen before parsing, but fence boundaries are only known during parsing.
  [SYNTAX-RFC] §4.3 resolves this by specifying sequential processing: the parser always knows
  whether a fence is open before deciding whether to join.
- [SYNTAX-RFC] §10.4 requires incremental emission: each command or text block must be finalizable
  as soon as its last physical line is consumed. Memory must be bounded by the largest single
  element, not total input size.
- [SYNTAX-RFC] §1 requires a single forward pass after normalization. No backtracking.
- [SYNTAX-RFC] §9.2 requires `raw` to contain the exact normalized source text before line joining,
  including backslash characters and `\n` separators between physical lines.
- [SYNTAX-RFC] §7.3 and §10.5 (round-trip fidelity) require text block content and ranges to use
  physical line numbers and physical line content so a formatter can reconstruct the original input.
- [SYNTAX-RFC] §8 requires independent zero-based ID sequences for commands (`cmd-0`, `cmd-1`) and
  text blocks (`text-0`, `text-1`).
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

1. `normalize(input)` converts all line endings to LF ([SYNTAX-RFC] §3.2).
2. The normalized string is split on `\n` into `physical_lines: Vec<&str>`, retained for `raw`
   reconstruction and text block content.
3. An owned copy is passed to `LineJoiner::new(owned)`.
4. A `while` loop drives a `ParserState` enum (`Idle` | `InFence(PendingCommand)`).
5. In `Idle`, `joiner.next_logical()` produces joined `LogicalLine` values. Joining uses true POSIX
   semantics: the backslash and line boundary are removed and lines are concatenated directly with
   no separator character inserted ([SYNTAX-RFC] §4.1). Each logical line is classified. Commands
   start accumulation. Text lines feed physical line content into `PendingText` via
   `start_text`/`append_text`.
6. In `InFence`, `joiner.next_physical()` produces raw lines. Each is fed to `accept_line`. On close
   or EOF, the command is finalized and the state returns to `Idle`.
7. Before starting a command, `header.raw` is overwritten with
   `physical_lines[first..=last].join("\n")` to satisfy [SYNTAX-RFC] §9.2.
8. Independent `cmd_seq: usize` and `text_seq: usize` counters are maintained and passed to
   `start_command` and `finalize_text` respectively.
9. At EOF, any pending text block is finalized. The result is returned with no `context` field
   ([ENGINE-SPEC] §2: context is the SDK's responsibility).

### Consequences

**Good:**

- Fence immunity is achieved without a second pass. The shared cursor inside `LineJoiner` means
  calling `next_physical()` in `InFence` bypasses joining for exactly the lines inside the fence,
  then `next_logical()` resumes joining when the state returns to `Idle`.
- Incremental emission is preserved. Each command and text block is finalized as soon as its last
  line is consumed. No global buffering beyond the current pending element.
- The fence-opener edge case ([SYNTAX-RFC] §6.2.5) resolves implicitly. When backslash joining
  merges a command line with a line containing a fence opener, `next_logical()` produces the merged
  text and `classify_line` detects the fence naturally. No special-casing in the orchestrator.
- `raw` is always correct. Slicing from the retained `physical_lines` vector guarantees the exact
  normalized source text including backslashes.
- Text block content stores physical lines, making round-trip reconstruction ([SYNTAX-RFC] §10.5)
  straightforward. A formatter can use `content` directly without needing to reverse the joining
  process.
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
- Bad, because it violates [SYNTAX-RFC] §10.4 (incremental emission). The first pass must buffer
  all fence boundary positions before any command can be emitted.
- Bad, because identifying fence boundaries requires knowing which lines are command lines (to find
  fence openers), which means the first pass duplicates much of the classification logic.
- Bad, because it requires two iterations over the input, violating the single-forward-pass
  principle ([SYNTAX-RFC] §1).

### Option 4: Zero-copy slice tracking

Represent joined logical lines as `Vec<(start_byte, end_byte)>` ranges into the normalized input
string, avoiding `String` allocation entirely.

- Good, because it eliminates allocation for joined lines, potentially improving throughput on large
  inputs.
- Bad, because backslash removal creates non-contiguous content. A joined line spanning physical
  lines `"a\"` and `"b"` maps to bytes `[0..1]` concatenated directly with `[3..4]` (true POSIX,
  no space), requiring a non-contiguous slice assembly that complicates all downstream consumers.
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
- Bad, because it violates [SYNTAX-RFC] §8, which specifies that command IDs and text block IDs are
  independent zero-based sequences. The spec examples show `cmd-0` and `text-0` coexisting, not
  `cmd-0` and `text-1`.

### Option 7: Text block content from joined logical lines

Store the `LogicalLine.text` (post-join, backslash removed) as text block content.

- Good, because consumers see "clean" text without trailing backslashes or continuation artifacts.
- Bad, because the range already uses physical line numbers. A text block with
  `range: {start_line: 0, end_line: 1}` but content of one joined line creates an inconsistency: the
  range says two physical lines but the content has no `\n` separator.
- Bad, because round-trip reconstruction ([SYNTAX-RFC] §10.5) becomes lossy. A formatter cannot
  distinguish between a two-physical-line text region that was joined (backslash removed) and a
  single physical line, because both would produce the same content string.
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

### Document references

Syntax RFC ([SYNTAX-RFC]): Slash Command Parser Syntax Specification v0.3.1

- §1 Introduction (single forward pass, incremental emission, deterministic, total)
- §3.2 Line-Ending Normalization
- §3.3 Physical Lines
- §3.4 Whitespace (SP and HTAB only)
- §4 Line Joining (true POSIX backslash continuation, no space insertion)
- §4.1 Backslash Continuation
- §4.2 Trailing Backslash at EOF
- §4.3 Fence Immunity (sequential processing resolves circular dependency)
- §4.4 Physical-Line Tracking
- §5 Command Detection
- §5.1 Command Name
- §6.1 Single-Line Mode
- §6.2 Fence Mode (opener, body, closer, unclosed, joining around fences)
- §6.2.3 Fence Closer (solely backticks after trimming; backslash prevents closure)
- §6.2.5 Joining Around Fences
- §7.1 Processing Model (idle and in-fence states)
- §7.3 Text Blocks
- §8 Identification and Ordering (independent ID sequences)
- §9.2 The raw Field
- §10.4 Incremental Emission
- §10.5 Roundtrip Fidelity

Engine Spec ([ENGINE-SPEC]): Slash Command Parser Engine Specification v0.4.0

- §2 Scope and Boundaries (no context, no JSON, no I/O)
- §4 Entry Points (parse_document)
- §5 Processing Pipeline (three stages)
- §6 Whitespace (SP and HTAB only; must not use char::is_whitespace())
- §7 Line Joining (true POSIX, no space insertion)
- §9 Command Accumulation
- §10 Text Block Accumulation
- §13 Internal Module Architecture


### Changes from v0.3.0 revision (2026-03-19)

- Updated all section references from monolithic v0.3.0 implementation spec to [SYNTAX-RFC] v0.3.1
  and [ENGINE-SPEC] v0.4.0.
- Decision-makers field corrected to "Tom Davidson".
- Pipeline step 5: clarified that line joining uses true POSIX semantics (direct concatenation, no
  space insertion). Previously the spec inserted a single space between joined lines.
- Pipeline step 9: added note that context is the SDK's responsibility, referencing [ENGINE-SPEC] §2.
- Option 4 (zero-copy): updated the non-contiguous slice description to reflect true POSIX (no space
  between fragments).
- Context paragraph updated from "v0.3.0 engine" to reference the current engine spec version.
- Fence closer discussion updated to reference [SYNTAX-RFC] §6.2.3 (solely backticks rule;
  backslash prevents closure).
- Circular dependency framing updated: [SYNTAX-RFC] §4.3 now explicitly states that sequential
  processing resolves this, rather than leaving it implicit.
