# Slash Command Parser Engine Specification v0.5.0

> **Author:** Tom D. Davidson  
> **Email:** tom@tomdavidson.org  
> **URI:** https://tomdavidson.org  
> **Location:** Utah  
> **Date:** March 2026  
> **Copyright:** (c) 2026 Tom D. Davidson. All rights reserved.  
> **Distribution:** Unlimited.

## Abstract

This document specifies the Slash Command Parser engine: a Rust library crate that implements the syntax rules defined in the Slash Command Syntax v1.1.0 [SYNTAX-RFC]. The engine consumes UTF-8 text and produces Rust domain types. It performs no serialization and has no dependencies on JSON, serde, or any output format.

Serialization to JSON, WASM bindings, WASI targets, CLI tools, and language-specific SDKs are defined in a separate SDK specification [SDK-SPEC].

## 1. Introduction

The Slash Command Parser engine is the core Rust crate that implements the parsing rules defined in [SYNTAX-RFC]. It is a pure library with the following properties:

- No I/O. The engine does not read files, make network requests, or interact with the environment.
- No serialization. The engine produces Rust types (structs and enums). It has no dependency on serde, serde_json, or any serialization framework.
- No global mutable state. All inputs are passed as function parameters. All outputs are returned as owned values.
- No unsafe code. The engine uses only safe Rust.
- Minimal dependencies. The only runtime dependency is thiserror for error type derivation.

The engine crate is designed to be consumed by three targets:

1. Rust SDK: adds serde serialization, JSON schema conformance, and a higher-level API.
2. WASM target: compiled to WebAssembly with bindings via wasm-bindgen.
3. WASI target: compiled for WASI runtimes with stdin/stdout interfaces.

All three targets are specified in [SDK-SPEC]. This document covers only the engine itself.

## 2. Scope and Boundaries

The engine is responsible for:

- Implementing all syntax rules from [SYNTAX-RFC] Sections 3-8.
- Producing output conforming to [SYNTAX-RFC] Section 7.
- Exposing domain types sufficient for downstream serialization.

The engine is NOT responsible for:

- JSON serialization or JSON schema. These are SDK concerns.
- Context object handling. The engine accepts a version string. Context pass-through is the SDK's responsibility.
- JSONL streaming. The engine parses one document at a time. Pipeline orchestration is the SDK's responsibility.
- CLI argument parsing, file I/O, or any environment interaction.

## 3. Domain Types

The engine exposes the following public types from its domain module. All types derive `Debug`, `Clone`, `PartialEq`, and `Eq` (except `ParseResult`, which derives `Debug`, `Clone`, `PartialEq`). All fields are public.

### 3.1. ParseResult

The top-level return type from `parse_document`.

```rust
struct ParseResult {
    version:     String,
    commands:    Vec<Command>,
    text_blocks: Vec<TextBlock>,
    warnings:    Vec<Warning>,
}
```

- `version` is always set to `SPEC_VERSION` (Section 14).
- `commands` contains commands in document order per [SYNTAX-RFC] Section 6.5.
- `text_blocks` contains text blocks in document order per [SYNTAX-RFC] Section 6.5.
- `warnings` contains non-fatal issues. Empty if none.

`ParseResult` has no `context` field. Context is a serialization concern handled by the SDK. The SDK wraps `ParseResult` into an envelope that includes caller-supplied context.

### 3.2. Command

```rust
struct Command {
    id:        String,
    name:      String,
    raw:       String,
    range:     LineRange,
    arguments: CommandArguments,
}
```

- `id` follows the pattern `cmd-{n}` where n is a zero-based sequential index per [SYNTAX-RFC] Section 6.5.
- `name` matches `[a-z]([a-z0-9-]*[a-z0-9])?` per [SYNTAX-RFC] Section 4.1. A command name MUST NOT end with a hyphen.
- `raw` contains the exact normalized source text (before line joining) per [SYNTAX-RFC] Section 7.1.
- `range` covers zero-based physical line numbers per [SYNTAX-RFC] Section 3.3.

### 3.3. CommandArguments

```rust
struct CommandArguments {
    header:     String,
    mode:       ArgumentMode,
    fence_lang: Option<String>,
    payload:    String,
}
```

- `header` is the inline argument text before any fence opener per [SYNTAX-RFC] Section 4.4.
- `mode` indicates how the payload was assembled.
- `fence_lang` is `Some(lang)` when the fence opener included a language identifier, `None` otherwise.
- `payload` is the assembled argument content per [SYNTAX-RFC] Section 5.

### 3.4. ArgumentMode

```rust
enum ArgumentMode {
    SingleLine,
    Fence,
}
```

Corresponds to the mode string values "single-line" and "fence" in [SYNTAX-RFC] Section 7.1. String serialization is the SDK's responsibility.

### 3.5. TextBlock

```rust
struct TextBlock {
    id:      String,
    range:   LineRange,
    content: String,
}
```

- `id` follows the pattern `text-{n}` per [SYNTAX-RFC] Section 6.5.
- `content` contains physical lines joined with LF separators per [SYNTAX-RFC] Section 7.2.

### 3.6. LineRange

```rust
struct LineRange {
    start_line: usize,
    end_line:   usize,
}
```

Both fields are zero-based physical line indices from the normalized input (after line-ending normalization, before line joining). The range is inclusive on both ends.

### 3.7. Warning

```rust
struct Warning {
    w_type:     String,
    start_line: Option<usize>,
    message:    Option<String>,
}
```

- `w_type` is a snake_case warning category identifier.
- `start_line` is the physical line where the issue was detected.
- `message` is a human-readable description.

The field is named `w_type` rather than `type` because `type` is a reserved keyword in Rust. The SDK serializes this field as `type` in JSON output.

Defined warning types:

| w_type | When Emitted | Reference |
|---|---|---|
| `"unclosed_fence"` | Fence reaches EOF without a valid closer | [SYNTAX-RFC] Section 5.2.4 |

## 4. Entry Points

### 4.1. parse_document

```rust
pub fn parse_document(input: &str) -> ParseResult
```

This is the sole public entry point of the engine. It accepts a UTF-8 string slice and returns a `ParseResult`.

The function:

1. Normalizes line endings per [SYNTAX-RFC] Section 3.1.
2. Splits into physical lines per [SYNTAX-RFC] Section 3.1.
3. Processes lines sequentially per [SYNTAX-RFC] Section 6.
4. Returns a complete `ParseResult`.

There is no second entry point that accepts context. Context is added by the SDK wrapper.

### 4.2. Total Function Guarantee

`parse_document` MUST return a valid `ParseResult` for any input. It MUST NOT panic, return an error type, or use any fallible return mechanism. There is no `Result<ParseResult, E>` variant. Malformed input produces commands with partial data and corresponding entries in the warnings vector.

This requirement derives from [SYNTAX-RFC] Section 8.2.

## 5. Processing Pipeline

The engine processes input through three stages. The stages are conceptually sequential but may be interleaved in implementation.

### 5.1. Stage 1: Normalization

Implements [SYNTAX-RFC] Section 3.1.

- Input: `&str` (raw UTF-8 input)
- Output: `String` (all line endings are LF)

The normalize module performs two replacements:

1. CRLF -> LF
2. Bare CR -> LF

This stage is a pure string transformation with no state.

### 5.2. Stage 2: Physical Line Splitting

Implements [SYNTAX-RFC] Section 3.1.

- Input: `&str` (normalized text)
- Output: `Vec<&str>` (physical lines)

The normalized text is split on LF. Each LF terminates a line and is not part of the line content. A trailing LF produces a trailing empty string.

Implementation note: the current engine pops a trailing empty element when the input ends with LF. This matches [SYNTAX-RFC] Section 3.1 because the trailing empty line contains no content and cannot affect parsing. Both behaviors (keeping or popping the trailing empty) are conformant as long as the output is identical.

### 5.3. Stage 3: Sequential Line Processing

Implements [SYNTAX-RFC] Sections 4, 5, 6, and 7.

The engine maintains two states:

- Idle: the engine obtains the next logical line via the line joiner (Section 7), classifies it (Section 8), and dispatches to command or text block accumulation.
- In-fence: the engine obtains the next physical line directly (bypassing the line joiner), checks for a fence closer, and either appends to the payload or finalizes the command.

The state transitions correspond exactly to [SYNTAX-RFC] Appendix C.

The key invariant: the engine always knows whether a fence is open before deciding whether to apply line joining to the current physical line. This eliminates any circular dependency between joining and fence detection.

## 6. Whitespace

Per [SYNTAX-RFC], whitespace means exactly two characters: U+0020 (SPACE) and U+0009 (HORIZONTAL TAB).

All whitespace checks in the engine MUST use this definition. Specifically:

- Command name termination: the first SP or HTAB after the command name ends the name.
- Argument separator: the whitespace between command name and arguments is SP or HTAB.
- Fence closer trimming: leading and trailing SP and HTAB are removed before checking for solely-backtick content.
- Header trimming: trailing SP and HTAB are removed from text before a fence opener.

The engine MUST NOT use Rust's `char::is_whitespace()` or `char::is_ascii_whitespace()` for these purposes. `char::is_whitespace()` includes 25+ Unicode whitespace characters (U+00A0, U+1680, U+2000-200A, U+2028, U+2029, U+202F, U+205F, U+3000) that are not recognized by this specification. `char::is_ascii_whitespace()` includes U+000A (LF), U+000C (FF), and U+000D (CR) which have already been consumed by normalization and line splitting.

Implementations SHOULD define a helper function:

```rust
fn is_wsp(c: char) -> bool {
    c == ' ' || c == '\t'
}
```

and use it consistently throughout all modules.

## 7. Line Joining

### 7.1. POSIX Semantics

Implements [SYNTAX-RFC] Section 3.2.

When a physical line ends with U+005C ("\\"), the backslash and the line boundary are removed and the remainder is concatenated directly with the next physical line. No separator character is inserted.

This is true POSIX backslash-newline removal. The v0.3.0 engine inserted a single space between joined lines. This is no longer the case.

### 7.2. Fence Immunity

Per [SYNTAX-RFC] Section 5.2.2, the line joiner MUST NOT be invoked for physical lines consumed while the parser is in in-fence state.

The engine enforces this by having the document parser call `next_physical()` (which returns the raw physical line) instead of `next_logical()` (which applies joining) when in in-fence state.

### 7.3. LogicalLine

The line joiner produces `LogicalLine` values:

```rust
struct LogicalLine {
    text:           String,
    first_physical: usize,
    last_physical:  usize,
}
```

- `text` is the joined content.
- `first_physical` is the zero-based index of the first physical line that contributed to this logical line.
- `last_physical` is the zero-based index of the last physical line that contributed.

`LogicalLine` is an internal type, not part of the public API.

## 8. Line Classification

Implements [SYNTAX-RFC] Section 4.

The line classifier receives a logical line and produces one of:

```rust
enum LineKind {
    Command(CommandHeader),
    Text,
}
```

`CommandHeader` is an internal struct carrying the parsed fields: raw, name, header_text, mode, fence_lang, and fence_backtick_count.

Classification rules:

1. Trim leading whitespace (SP and HTAB only).
2. If the first character is not "/", return `Text`.
3. Extract the command name after "/". If it does not match `[a-z]([a-z0-9-]*[a-z0-9])?`, return `Text`. Names ending with a hyphen are invalid per [SYNTAX-RFC] Section 4.1.
4. Extract the arguments portion after the first whitespace.
5. Search the arguments for a fence opener (three or more consecutive backticks). If found, set mode to `Fence` and extract header_text and fence_lang. Otherwise, set mode to `SingleLine`.
6. Return `Command(header)`.

## 9. Command Accumulation

### 9.1. PendingCommand

While a command is being assembled (during fence mode), the engine maintains a `PendingCommand`:

```rust
struct PendingCommand {
    id:                   usize,
    name:                 String,
    header_text:          String,
    mode:                 ArgumentMode,
    fence_lang:           Option<String>,
    fence_backtick_count: usize,
    start_line:           usize,
    end_line:             usize,
    payload_lines:        Vec<String>,
    raw_lines:            Vec<String>,
    is_open:              bool,
}
```

`PendingCommand` is an internal type, not part of the public API.

For single-line commands, `PendingCommand` is created and immediately finalized (`is_open` is false). For fence commands, `PendingCommand` stays open until a closer is found or EOF.

### 9.2. AcceptResult

When a physical line is offered to an open `PendingCommand`:

```rust
enum AcceptResult {
    Consumed,    // Line appended to payload.
    Completed,   // Line was a valid closer; command finalized.
    Rejected,    // Command was already closed; line not consumed.
}
```

### 9.3. Finalization

The `finalize_command` function consumes a `PendingCommand` and produces a `FinalizedCommand`:

```rust
struct FinalizedCommand {
    command:  Command,
    warnings: Vec<Warning>,
}
```

Finalization:

1. Formats `id` as `cmd-{n}` from the sequential counter.
2. Joins `raw_lines` with "\n" to produce the `raw` field.
3. Joins `payload_lines` with "\n" to produce the `payload` field.
4. If the command is a fence and `is_open` is true (EOF reached), emits an `"unclosed_fence"` warning.
5. Returns the `Command` and any warnings.

`FinalizedCommand` is an internal type.

## 10. Text Block Accumulation

Text blocks are accumulated via `PendingText`:

```rust
struct PendingText {
    start_line: usize,
    end_line:   usize,
    lines:      Vec<String>,
}
```

Physical lines covered by non-command logical lines are appended to `PendingText`. When a command is encountered or EOF is reached, the pending text block is finalized:

1. Join lines with "\n" to produce `content`.
2. Format `id` as `text-{n}` from the text block counter.
3. Set range from `start_line` and `end_line`.

Text blocks store physical lines (before joining). A logical line formed by backslash continuation contributes all of its constituent physical lines (with backslashes intact) to the text block content. This preserves the original source for roundtrip fidelity.

## 11. Warning Types

All warning type strings use snake_case.

The engine defines the following warning types:

| w_type | When Emitted |
|---|---|
| `"unclosed_fence"` | EOF reached while in in-fence state |

Additional warning types MAY be added in future versions. The `Warning` struct's `w_type` field is a `String` (not an enum) to allow forward-compatible extension without breaking changes.

## 12. Conformance to Syntax RFC

The engine implements all normative requirements from [SYNTAX-RFC] Section 8.

### 12.1. Determinism

Per [SYNTAX-RFC] Section 8.4, `parse_document` MUST produce identical output for identical input. The engine uses no randomness, no hash maps with non-deterministic iteration order, and no floating point arithmetic.

### 12.2. Incremental Emission

The engine finalizes each command and text block as soon as its last physical line is consumed.

In the current implementation, finalized commands and text blocks are pushed to vectors in the `ParseCtx` and assembled into `ParseResult` at the end. This satisfies incremental emission because each element is finalized (not revisited) on the spot.

A future streaming implementation could emit elements via a callback or channel instead of collecting into vectors. The engine architecture supports this without structural changes.

### 12.3. Opaque Payload

Per [SYNTAX-RFC] Section 8.2, the engine never interprets argument content. The line classifier extracts argument boundaries but does not tokenize, parse, or validate the header or payload strings.

### 12.4. Roundtrip Fidelity

Per [SYNTAX-RFC] Section 8.3, the engine preserves sufficient information for a formatter to reconstruct input that re-parses to a structurally equivalent result. The `raw` field and text block `content` field contain the pre-join physical lines needed for reconstruction.

## 13. Internal Module Architecture

### 13.1. Module Map

The engine crate has the following module structure:

```
parser/
  src/
    lib.rs                  Public API re-exports.
    domain/
      mod.rs                Type re-exports.
      types.rs              ParseResult, Command, CommandArguments,
                            ArgumentMode, TextBlock, LineRange,
                            SPEC_VERSION.
      errors.rs             Warning.
    application/
      mod.rs                Module declarations, re-export of
                            parse_document.
      normalize.rs          Line-ending normalization (Stage 1).
      line_join.rs          Backslash continuation (Stage 3, idle).
      line_classify.rs      Command detection (Stage 3).
      command_accumulate.rs PendingCommand, start_command,
                            accept_line.
      command_finalize.rs   finalize_command.
      text_collect.rs       PendingText, start_text, append_text,
                            finalize_text.
      document_parse.rs     parse_document orchestration.
      tests/
        mod.rs              Cross-module integration tests.
        proptest.rs         Property-based tests.
```

### 13.2. Dependency Rules

Modules within the engine follow a strict dependency hierarchy:

1. `domain/` depends on nothing.
2. `application/` modules depend on `domain/` types.
3. `application/` modules do not depend on each other except through `document_parse.rs`, which orchestrates all modules.
4. No module depends on any serialization library.
5. No module performs I/O.

### 13.3. Test Layering

Tests are organized in two layers:

Layer 1 (unit tests): Each module file contains `#[cfg(test)]` tests that exercise only that module's logic. These tests use direct function calls, not `parse_document`.

Layer 2 (integration tests): The `application/tests/` directory contains tests that exercise cross-module composition via `parse_document`. These include the Appendix B examples from [SYNTAX-RFC].

Property-based tests use the proptest crate. Slow property tests are gated behind the "tdd" feature flag and excluded from watch-mode iteration:

```rust
#[cfg_attr(feature = "tdd", ignore)]
```

This allows fast red-green-refactor cycles with:

```sh
cargo nextest run --features tdd
```

## 14. Version Constant

```rust
pub const SPEC_VERSION: &str = "0.5.0";
```

This constant is set in `domain/types.rs` and populated into `ParseResult.version` by `parse_document`.

The version tracks the engine spec version. The SDK may override the version in the serialized envelope to match the syntax RFC version ("1.1.0") if required by [SYNTAX-RFC].

## 15. Migration Notes from v0.4.0

The following changes affect existing code when upgrading from engine v0.4.0 to v0.5.0:

### 15.1. Command Name: Trailing Hyphen Prohibition

- v0.4.0: Command names matched `[a-z][a-z0-9-]*`, allowing trailing hyphens.
- v0.5.0: Command names match `[a-z]([a-z0-9-]*[a-z0-9])?`. Names ending with a hyphen are invalid and the line is classified as text.

Impact: Update the regex or matching logic in `line_classify.rs`. Add test cases for inputs like `/cmd-` (should be text, not a command).

### 15.2. Syntax RFC Reference

- v0.4.0: Referenced Slash Command Parser Syntax Specification v0.3.1.
- v0.5.0: References Slash Command Syntax v1.1.0 [SYNTAX-RFC].

Impact: Update any documentation or comments that reference the old spec version.

### 15.3. SPEC_VERSION Constant

- v0.4.0: `SPEC_VERSION = "0.4.0"`
- v0.5.0: `SPEC_VERSION = "0.5.0"`

Impact: Update `domain/types.rs` and any tests that assert on the version string.

## 16. Migration Notes from v0.3.0

These notes are retained for projects upgrading from v0.3.0 directly.

### 16.1. Line Joining: No Space Insertion

- v0.3.0: Joined lines were concatenated with a single space.
- v0.4.0+: Joined lines are concatenated directly (true POSIX).

Impact: Remove the space insertion in `line_join.rs`. Update all test assertions that included the inserted space.

### 16.2. Whitespace Definition

- v0.3.0: Used Rust's `char::is_whitespace()` in some code paths.
- v0.4.0+: All whitespace checks use SP (U+0020) and HTAB (U+0009) only.

Impact: Replace all uses of `char::is_whitespace()` and `char::is_ascii_whitespace()` with a dedicated `is_wsp()` helper.

### 16.3. Warning Type String

- v0.3.0: Used `"unclosed-fence"` (kebab-case).
- v0.4.0+: Uses `"unclosed_fence"` (snake_case).

Impact: Update the string literal in `command_finalize.rs` and all test assertions that match on the warning type.

## Normative References

- **[SYNTAX-RFC]** Davidson, T. D., "Slash Command Syntax, Version 1.1.0", March 2026.
- **[SDK-SPEC]** Davidson, T. D., "Slash Command Parser SDK Specification" (forthcoming).

## Author

Tom D. Davidson  
Email: tom@tomdavidson.org  
URI: https://tomdavidson.org  
Utah
