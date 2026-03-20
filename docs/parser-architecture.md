# Slash Command Parser (`//parser`) Architecture

Spec: Slash Command Parser Specification v0.3.0 ADR: 0006 – Repository Structure and Project
Boundaries Patterns: Tom's Clean Code, Tom's Clean Architecture, Rust Patterns, Universal Rust/WASM
Library Pattern

## 1. Purpose

`//parser` is the internal Rust library that implements the Slash Command Parser v0.3.0 engine. It
is the single source of truth for all parsing behavior. No parsing logic exists outside this crate.

It accepts a UTF-8 string and returns a structured Rust result representing all detected slash
commands, interleaved text blocks, and any parser warnings. The parser is deterministic, pure, and
total: it always produces a valid result for any input.

## 2. Boundaries

`//parser` is consumed by exactly three projects within the monorepo:

- `//wasm-javascript` (wasm-bindgen adapter for JS/TS runtimes)
- `//wasm-wasi` (WASI P2 component for polyglot SDK consumption)
- `//slash-rust` (thin published Rust crate wrapping `//parser`)

External Rust consumers use `//slash-rust`, not `//parser` directly. This keeps `//parser` internal
and free to evolve its API without semver concerns.

### What `//parser` Does

- Normalize line endings (CRLF/CR to LF).
- Perform POSIX-style backslash line joining with physical line tracking.
- Detect commands, classify argument modes (single-line or fence).
- Accumulate fenced payloads verbatim.
- Collect non-command lines into text blocks.
- Emit warnings for malformed constructs (e.g., unclosed fences).
- Provide a minimal infrastructure layer for JSON serialization of the result.

### What `//parser` Does Not Do

- JSONL framing (responsibility of CLI and SDKs).
- Context propagation (the spec's `context` object is injected by SDKs).
- Argument interpretation (all argument content is opaque).
- IO, logging, configuration, or environment access.
- WASM/WASI binding (handled by `//wasm-javascript` and `//wasm-wasi`).

## 3. Layered Structure

Following Tom's Clean Architecture and Rust Patterns, `//parser` is organized into three layers with
strict inward-pointing dependencies.

```
parser/
  src/
    lib.rs                 # Public API re-exports
    domain/
      mod.rs
      types.rs             # Pure domain types (no serde)
    application/
      mod.rs
      normalize.rs         # Line ending normalization
      line_join.rs         # POSIX backslash line joining
      line_classify.rs     # Command vs text classification
      fence_collect.rs     # Fence state accumulation
      document_parse.rs    # Top-level pipeline orchestration
      tests/
        mod.rs
        proptest.rs        # Layer 2 cross-module property tests
    infrastructure/
      mod.rs
      json.rs              # Serde DTOs and JSON serialization
```

### 3.1 Domain Layer (`domain/`)

Pure types with no external dependencies. No serde, no framework crates.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentMode {
    SingleLine,
    Fence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRange {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandArguments {
    pub header: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub name: String,
    pub raw: String,
    pub range: LineRange,
    pub arguments: CommandArguments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    pub id: String,
    pub range: LineRange,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub wtype: String,
    pub start_line: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    pub version: String,
    pub commands: Vec<Command>,
    pub textblocks: Vec<TextBlock>,
    pub warnings: Vec<Warning>,
}

pub const SPEC_VERSION: &str = "0.3.0";
```

### 3.2 Application Layer (`application/`)

Orchestrates the parsing pipeline using domain types. Contains all parsing logic. No IO, no serde,
no framework imports.

#### `normalize.rs`

Pure function. Replaces CRLF with LF, then remaining bare CR with LF.

```rust
pub fn normalize(input: &str) -> String
```

#### `line_join.rs`

Implements POSIX-style backslash line joining (spec Section 2.2).

For each physical line ending with `\`:

1. Remove the trailing backslash.
2. Join with the next physical line, separated by a single space.
3. Repeat if the joined result still ends with `\`.
4. At EOF, a trailing backslash is silently removed.

The joiner produces an iterator of `LogicalLine` values that carry the joined text and the physical
line range they originated from.

```rust
pub struct LogicalLine {
    pub text: String,
    pub first_physical: usize,
    pub last_physical: usize,
}
```

The joiner must not join lines that are inside a fenced block (spec Section 2.3: Fence Immunity).
Because fence detection happens during command parsing, the joiner and the state machine must
cooperate. The implementation uses a "pull" model: the state machine drives the joiner, and when
entering `InFence` state it switches to consuming raw physical lines directly until the fence
closes.

**Alternative: Zero-Copy with Slice Indices**

Instead of allocating `String`s for joined lines, track `(start_byte, end_byte)` ranges into the
normalized input. Joined lines would be represented as a `Vec` of non-contiguous slices. This avoids
allocation but complicates fence detection and command parsing because the backslash and newline are
physically removed during joining, requiring non-contiguous slice assembly. Given that inputs are
small (chatops messages, git comments), the allocation approach is preferred for clarity. The
zero-copy alternative is documented here for future reference if profiling reveals a need.

#### `line_classify.rs`

Classifies a single logical line as either a command or text.

```rust
pub enum LineKind {
    Command(CommandHeader),
    Text,
}

pub struct CommandHeader {
    pub raw: String,
    pub name: String,
    pub header_text: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
    pub fence_backtick_count: Option<usize>,
}

pub fn classify_line(line: &str) -> LineKind
```

The classifier detects:

- Valid command names (`[a-z][a-z0-9-]*`).
- Fence openers (three or more backticks in the arguments portion).
- Single-line mode (everything else after the command name).
- Invalid slash lines (bare `/`, uppercase, digits) are classified as `Text`.

#### `fence_collect.rs`

Manages the `InFence` state. Consumes raw physical lines until a valid closing fence or EOF.

```rust
pub struct FenceCollector {
    pub backtick_count: usize,
    pub payload_lines: Vec<String>,
    pub start_line: usize,
    pub end_line: usize,
}
```

A closing fence is a physical line that, after trimming leading and trailing whitespace, consists
solely of backtick characters with a count >= the opener's count.

At EOF without a closing fence, the collector signals that the fence is unclosed. The caller emits
an `unclosedfence` warning.

#### `document_parse.rs`

Top-level orchestration. This is the only module that composes the full pipeline.

Pipeline:

1. `normalize(input)` produces a normalized string.
2. Split into physical lines with indices.
3. Drive a two-state machine (`Idle` / `InFence`) over the physical lines:
   - In `Idle`: consume joined logical lines via the line joiner.
     - Classify each logical line.
     - Command without fence: finalize as single-line, append to results.
     - Command with fence: record header/metadata, switch to `InFence`.
     - Text: append to current pending text block.
   - In `InFence`: consume raw physical lines via the fence collector.
     - On fence close: finalize fenced command, return to `Idle`.
     - On EOF: finalize with warning, return to `Idle`.
4. At EOF: finalize any pending text block.
5. Assign sequential IDs (`cmd-0`, `cmd-1`, `text-0`, `text-1`).
6. Return `ParseResult`.

```rust
pub fn parse_document(input: &str) -> ParseResult
```

The `raw` field on each command captures the exact source text from the normalized input (before
line joining), including backslashes, physical newlines, and fence delimiters.

### 3.3 Infrastructure Layer (`infrastructure/`)

Minimal. Exists solely to support serialization for downstream WASM wrappers and the Rust SDK.

#### `json.rs`

Serde-derived DTOs that map 1:1 to the JSON schema in spec Section 8.5. Domain types remain free of
serde derives.

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct ParseResultDto {
    pub version: String,
    pub commands: Vec<CommandDto>,
    pub textblocks: Vec<TextBlockDto>,
    pub warnings: Vec<WarningDto>,
}

#[derive(Serialize)]
pub struct CommandDto {
    pub id: String,
    pub name: String,
    pub raw: String,
    pub range: LineRangeDto,
    pub arguments: CommandArgumentsDto,
}

#[derive(Serialize)]
pub struct CommandArgumentsDto {
    pub header: String,
    pub mode: String, // "single-line" or "fence"
    #[serde(rename = "fencelang")]
    pub fence_lang: Option<String>,
    pub payload: String,
}

#[derive(Serialize)]
pub struct LineRangeDto {
    #[serde(rename = "startline")]
    pub start_line: usize,
    #[serde(rename = "endline")]
    pub end_line: usize,
}

#[derive(Serialize)]
pub struct TextBlockDto {
    pub id: String,
    pub range: LineRangeDto,
    pub content: String,
}

#[derive(Serialize)]
pub struct WarningDto {
    #[serde(rename = "type")]
    pub wtype: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "startline")]
    pub start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
```

Conversion from domain types:

```rust
impl From<&ParseResult> for ParseResultDto { ... }
impl From<&Command> for CommandDto { ... }
// etc.
```

Public serialization functions:

```rust
pub fn to_json_string(result: &ParseResult) -> String {
    let dto = ParseResultDto::from(result);
    serde_json::to_string(&dto).expect("serialization is infallible for valid domain types")
}
```

Note: `to_json_value` and `to_value` helpers for `JsValue` or `serde_json::Value` are not provided
here. Those belong in `//wasm-javascript` and `//wasm-wasi` respectively.

## 4. Public API

`lib.rs` re-exports the public surface:

```rust
mod application;
mod domain;
mod infrastructure;

// Domain types
pub use domain::types::{
    ArgumentMode, Command, CommandArguments, LineRange, ParseResult, SPEC_VERSION, TextBlock,
    Warning,
};

// Engine entry point
pub use application::document_parse::parse_document;

// JSON serialization (infrastructure)
pub use infrastructure::json::to_json_string;
```

The primary entry point:

```rust
/// Total function. Always returns a valid ParseResult for any input.
/// Never panics. Malformed constructs produce warnings, not errors.
pub fn parse_document(input: &str) -> ParseResult
```

There is no `Result` wrapper. The spec requires a total function. All malformed input is reflected
via the `warnings` vec, never via control flow. SDKs that want strict mode can inspect `warnings`
and reject results containing them.

There is no `context` parameter. The spec's `context` object is injected by SDKs when they build the
final JSON envelope for their consumers.

## 5. State Machine

Two states, matching spec Section 4.

```
┌──────┐  command + fence opener   ┌─────────┐
│ Idle │ ─────────────────────────▶│ InFence │
│      │◀───────────────────────── │         │
└──────┘  closing fence or EOF     └─────────┘
```

### Idle

- Consumes logical lines (joined).
- Classifies each line.
- Single-line commands are finalized immediately.
- Fence-opening commands transition to `InFence`.
- Non-command lines accumulate into the current text block.
- On command or EOF, any pending text block is finalized.

### InFence

- Consumes raw physical lines (no joining).
- Appends each line to the fence payload verbatim.
- Checks each line for a valid closing fence.
- On close: finalizes the fenced command, returns to `Idle`.
- On EOF without close: finalizes with partial payload, emits `unclosedfence` warning, returns to
  `Idle`.

## 6. Dependencies

```toml
[package]
name = "parser"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
proptest = "1"
proptest-derive = "0.5"

[features]
tdd = []
```

`serde` and `serde_json` are used only in `infrastructure/json.rs`. Domain and application layers
have zero external dependencies beyond `std`.

## 7. Testing Strategy

### 7.1 Layer 1: In-File Unit Tests

Each application module has `#[cfg(test)] mod tests` at the bottom.

- `normalize.rs`: CRLF, bare CR, mixed endings, no-op on clean input.
- `line_join.rs`: single join, multi-join chains, trailing backslash at EOF, lines without backslash
  pass through, fence immunity integration.
- `line_classify.rs`: valid commands, invalid slash lines, fence opener detection, fence language
  extraction, edge cases (bare `/`, uppercase, digits).
- `fence_collect.rs`: closing fence detection, variable-length backticks, unclosed fence at EOF,
  payload verbatim preservation.
- `document_parse.rs`: all Appendix A examples from the spec, text block accumulation, interleaved
  commands and text, empty input, text-only input, multiple commands, warning emission.
- `infrastructure/json.rs`: DTO conversion correctness, field naming matches spec schema.

### 7.2 Layer 2: Application Property Tests

`parser/src/application/tests/proptest.rs` covers cross-module composition.

Key properties:

- Never panics on arbitrary ASCII/UTF-8 input (total function).
- Command count equals the number of logical lines starting with a valid slash command.
- Text-only input produces zero commands and non-empty text blocks.
- Fenced content is preserved verbatim (no joining, no escaping).
- Fence immunity: backslashes inside fences are literal, never trigger joining.
- Physical line ranges are always valid (start <= end, within input bounds).
- Version is always `SPEC_VERSION`.
- Unclosed fences always produce exactly one warning.
- Backslash joining around fences: joined lines before a fence opener produce correct header; lines
  after a fence closer join correctly as text.

All property tests use `#[cfg_attr(feature = "tdd", ignore)]` to stay out of watch mode.

### 7.3 Layer 3: Integration Tests

`tests/` directory at the crate root.

- `parse_examples.rs`: exact output assertions for every example in Appendix A (A.1 through A.9).
- `json_output.rs`: end-to-end `parse_document` -> `to_json_string` -> deserialize and assert
  against spec schema.
- Future: roundtrip fidelity (`P(F(P(I))) == P(I)`) once a formatter exists.

## 8. Error Philosophy

`//parser` has no error type at its public API boundary. The `parse_document` function is total. All
malformed input produces a valid `ParseResult` with appropriate `Warning` entries. This matches the
spec's requirement that the parser never fails.

SDKs are responsible for deciding whether warnings constitute errors for their consumers. A strict
SDK might reject any result with warnings. A lenient SDK might pass them through as metadata.

## 9. Decisions and Alternatives

### 9.1 Logical Line Intermediate Type

Decision: use `LogicalLine { text: String, first_physical: usize, last_physical: usize }` as the
intermediate between the line joiner and the state machine.

Alternative: zero-copy slice tracking. Represent joined lines as indices into the normalized input
rather than allocated strings. This avoids allocation but complicates the implementation because
backslash removal creates non-contiguous content. Given inputs are small (chatops, git comments),
allocation is preferred for clarity. Revisit if profiling shows a need.

### 9.2 Iterator vs Imperative Loop for Pipeline

Decision: prefer functional iterator chains where the pipeline is clear and readable. Use an
explicit loop when fence immunity requires switching between logical-line and physical-line
consumption mid-stream.

The top-level `document_parse` will likely use a `while let` loop driving a `ParserState` enum,
because the `Idle` -> `InFence` transition changes the _source_ of lines (logical vs physical). This
is one of the cases where a loop is more readable and maintainable than a pure iterator chain, per
Rust Patterns: "Use for loops when the iterator chain becomes unclear due to lifetimes, borrowing,
or complex control flow."

Individual stages (normalize, classify, fence check) remain pure functions composed via iterators
where appropriate.

### 9.3 Context Handling

Decision: `ParseResult` has no context field. SDKs inject context when building the final JSON
envelope. This keeps `//parser` focused on parsing.

### 9.4 JSONL Streaming

Decision: not a concern of `//parser`. The engine emits one `ParseResult` per call. JSONL framing is
the responsibility of the CLI (`//riff-cli`) and language SDKs.

### 9.5 Return Type

Decision: `pub fn parse_document(&str) -> ParseResult` with no `Result` wrapper. The function is
total. Warnings are data, not control flow.

### 9.6 Serde Location

Decision: serde derives live only in `infrastructure/json.rs` on DTO types. Domain types in
`domain/types.rs` have zero serde annotations. Conversion happens via `From` impls at the
infrastructure boundary.
