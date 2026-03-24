# Slash Command Parser SDK Specification v0.2.0

> **Author:** Tom D. Davidson  
> **Email:** tom@tomdavidson.org  
> **URI:** https://tomdavidson.org  
> **Location:** Utah  
> **Date:** March 2026  
> **Copyright:** (c) 2026 Tom D. Davidson. All rights reserved.  
> **Distribution:** Unlimited.

## Abstract

This document specifies the SDK layer for the Slash Command Parser. The SDK wraps the engine crate [ENGINE-SPEC] and provides serialization, bindings, and runtime integration for three targets: a Rust SDK crate, a WebAssembly (WASM) module, and a WASI executable.

The engine is a pure parsing library that produces Rust types. The SDK is responsible for everything between those Rust types and the outside world: JSON serialization, the context pass-through envelope, JSONL streaming, JavaScript interop via wasm-bindgen, and stdin/stdout I/O for WASI runtimes.

## 1. Introduction

The Slash Command Parser engine [ENGINE-SPEC] is a pure Rust library that consumes UTF-8 text and produces Rust domain types. It has no knowledge of JSON, JavaScript, file I/O, or any serialization format.

The SDK bridges the engine to consumers:

- Rust applications import the SDK crate and receive serde-enabled types that serialize to a JSON schema conforming to [SYNTAX-RFC].
- JavaScript/TypeScript applications import a WASM module that accepts a string and returns a JSON object matching the same schema.
- WASI runtimes execute a binary that reads UTF-8 from stdin and writes JSON (or JSONL) to stdout.

All three targets produce output conforming to the same JSON schema. The SDK is the single point where serialization decisions are made.

## 2. Scope and Boundaries

The SDK is responsible for:

- JSON serialization of engine types.
- The context pass-through envelope (Section 5).
- JSONL streaming for pipeline use cases (Section 9).
- WASM bindings via wasm-bindgen (Section 7).
- WASI binary with stdin/stdout I/O (Section 8).
- TypeScript type declarations for the WASM module (Section 7.5).

The SDK is NOT responsible for:

- Parsing logic. All parsing is delegated to the engine.
- CLI argument parsing beyond what is needed for the WASI binary.
- Any behavior not defined in [SYNTAX-RFC] or [ENGINE-SPEC].

## 3. Crate Architecture

The SDK is a separate crate (or set of crates) that depends on the engine crate. The recommended workspace layout:

```
workspace/
  parser/              Engine crate [ENGINE-SPEC]
  parser-sdk/          Rust SDK crate (this spec)
    src/
      lib.rs           Public API, re-exports
      envelope.rs      SlashParserResult assembly
      serial.rs        Serde configuration
    Cargo.toml
  parser-wasm/         WASM target crate
    src/
      lib.rs           wasm-bindgen exports
    Cargo.toml
  parser-wasi/         WASI target crate
    src/
      main.rs          stdin/stdout binary
    Cargo.toml
```

The three output targets (Rust SDK, WASM, WASI) MAY be separate crates or feature-gated within a single crate. This spec defines them as separate crates for clarity. Implementations MAY consolidate them if the dependency footprint is acceptable.

Dependency direction:

```
parser-sdk   --> parser (engine)
parser-wasm  --> parser-sdk --> parser
parser-wasi  --> parser-sdk --> parser
```

The engine crate MUST NOT depend on the SDK.

## 4. JSON Serialization

### 4.1. Envelope Assembly

The engine produces a `ParseResult` containing commands, text_blocks, warnings, and a version string. The SDK wraps this into a `SlashParserResult` envelope by adding the context object.

```rust
struct SlashParserResult {
    version:     String,
    context:     serde_json::Value,
    commands:    Vec<Command>,
    text_blocks: Vec<TextBlock>,
    warnings:    Vec<Warning>,
}
```

The SDK constructs the envelope by:

1. Calling `parser::parse_document(input)` to obtain `ParseResult`.
2. Combining `ParseResult` fields with the caller-supplied context (or an empty object) to produce `SlashParserResult`.
3. Serializing `SlashParserResult` to JSON via serde_json.

The engine's `ParseResult` is never exposed directly in the SDK's public API. Consumers interact only with `SlashParserResult`.

### 4.2. Field Naming

All JSON field names use snake_case. Serde's default Rust field naming matches snake_case, so no rename attributes are needed for most fields.

Exception: the `Warning` struct's Rust field `w_type` MUST be serialized as `type` in JSON output. This requires:

```rust
#[serde(rename = "type")]
pub w_type: String,
```

### 4.3. ArgumentMode Serialization

The engine's `ArgumentMode` enum has variants `SingleLine` and `Fence`. These MUST serialize to the JSON string values `"single-line"` and `"fence"` respectively.

```rust
#[serde(rename_all = "kebab-case")]
pub enum ArgumentMode {
    SingleLine,
    Fence,
}
```

This produces `"single-line"` and `"fence"`, matching [SYNTAX-RFC] Section 7.1.

### 4.4. Warning Type Serialization

Warning type values use snake_case as plain strings. The `type` field is a `String`, not an enum, to allow forward-compatible extension.

Currently defined types:

- `"unclosed_fence"`

The SDK MUST NOT validate or restrict the warning type string. Future engine versions may introduce new warning types without requiring SDK changes.

### 4.5. Null Handling

Optional fields that are `None` in Rust MUST serialize to JSON `null`, not be omitted. Specifically:

- `fence_lang`: null when not in fence mode or no language specified.
- `start_line` and `message` on Warning: null when not applicable.

Omitting them would violate the schema. The SDK MUST ensure required fields are always present in the output, even when their value is null.

Serde configuration for nullable fields:

```rust
#[serde(serialize_with = "serialize_option_as_null")]
```

Or, more simply, `Option<T>` with serde's default behavior combined with the schema's `required` constraint.

### 4.6. Schema Conformance

The serialized JSON output of the SDK MUST validate against the JSON schema defined for [SYNTAX-RFC]. The SDK's test suite SHOULD include schema validation tests that deserialize SDK output and validate it against the schema programmatically.

## 5. Context Object

### 5.1. Pass-Through Semantics

The context object is caller-supplied metadata that the SDK passes through to the output envelope unchanged. The engine never sees it.

The SDK accepts context as a `serde_json::Value` (for the Rust SDK), a `JsValue` (for WASM), or a JSON string (for WASI stdin).

The SDK MUST NOT:

- Validate context fields.
- Add, remove, or modify context fields.
- Interpret context content in any way.

The only constraint is that context MUST be a JSON object (not an array, string, number, or null). If a non-object is provided, the SDK SHOULD substitute an empty object `{}`.

### 5.2. Default Context

When no context is provided by the caller, the SDK uses an empty JSON object `{}`.

The Rust SDK:

```rust
pub fn parse(input: &str) -> SlashParserResult
pub fn parse_with_context(
    input: &str,
    context: serde_json::Value
) -> SlashParserResult
```

The first form uses `{}` as context. The second passes through the provided value.

## 6. Rust SDK

### 6.1. Public API

The Rust SDK crate exposes:

Functions:

- `parse(input: &str) -> SlashParserResult`
- `parse_with_context(input: &str, context: serde_json::Value) -> SlashParserResult`

Types (all with `Serialize` and `Deserialize`):

- `SlashParserResult`
- `Command`
- `CommandArguments`
- `ArgumentMode`
- `TextBlock`
- `LineRange`
- `Warning`

These are SDK wrapper types, not the engine's domain types. They mirror the engine types but add serde derives and any field renames needed for JSON conformance.

### 6.2. SlashParserResult

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashParserResult {
    pub version: String,
    pub context: serde_json::Value,
    pub commands: Vec<Command>,
    pub text_blocks: Vec<TextBlock>,
    pub warnings: Vec<Warning>,
}
```

The SDK constructs this from the engine's `ParseResult` plus the caller-supplied context.

### 6.3. Serde Derives

All SDK types derive both `Serialize` and `Deserialize`. This enables:

- Serialization to JSON for output.
- Deserialization from JSON for testing, roundtrip verification, and consumer convenience.

The SDK depends on:

- `serde = { version = "1", features = ["derive"] }`
- `serde_json = "1"`

### 6.4. Feature Flags

The SDK crate MAY expose feature flags to control optional functionality:

| Feature | Purpose |
|---|---|
| `"default"` | Rust SDK only (serde + serde_json) |
| `"schema"` | Includes JSON schema validation utilities |

The WASM and WASI targets are separate crates and do not need feature flags in the SDK crate.

## 7. WASM Target

### 7.1. Build Target

The WASM crate compiles to `wasm32-unknown-unknown` using wasm-bindgen for JavaScript interop.

Build command:

```sh
wasm-pack build --target web parser-wasm
```

Or for Node.js:

```sh
wasm-pack build --target nodejs parser-wasm
```

### 7.2. Exported Functions

The WASM module exports two functions:

```rust
#[wasm_bindgen]
pub fn parse(input: &str) -> JsValue

#[wasm_bindgen]
pub fn parse_with_context(
    input: &str,
    context: JsValue
) -> JsValue
```

Both return a `JsValue` containing the `SlashParserResult` as a JavaScript object.

### 7.3. Input and Output

- Input: a JavaScript string (UTF-16, converted to UTF-8 by wasm-bindgen).
- Output: a JavaScript object matching the JSON schema. The SDK serializes to `serde_json::Value` and converts to `JsValue` via serde-wasm-bindgen or `JsValue::from_serde`.
- Context: a JavaScript object. If `undefined` or `null` is passed, the SDK uses an empty object.

### 7.4. Error Handling

The WASM functions MUST NOT throw. The engine is a total function; the SDK wraps it in an envelope. There is no input that produces an error.

If context conversion fails (e.g., a non-object is passed), the SDK substitutes an empty object and proceeds normally.

### 7.5. TypeScript Declarations

The WASM build MUST produce TypeScript declaration files (`.d.ts`) that describe the exported functions and the shape of the return value.

Minimal declaration:

```typescript
export interface SlashParserResult {
    version: string;
    context: Record<string, unknown>;
    commands: Command[];
    text_blocks: TextBlock[];
    warnings: Warning[];
}

export interface Command {
    id: string;
    name: string;
    raw: string;
    range: LineRange;
    arguments: CommandArguments;
}

export interface CommandArguments {
    header: string;
    mode: "single-line" | "fence";
    fence_lang: string | null;
    payload: string;
}

export interface TextBlock {
    id: string;
    range: LineRange;
    content: string;
}

export interface LineRange {
    start_line: number;
    end_line: number;
}

export interface Warning {
    type: string;
    start_line?: number;
    message?: string;
}

export function parse(input: string): SlashParserResult;
export function parse_with_context(
    input: string,
    context: Record<string, unknown>
): SlashParserResult;
```

The declaration file MAY be generated by wasm-bindgen or hand-maintained. Either way, it MUST match the actual output.

### 7.6. npm Package

The WASM crate SHOULD be publishable as an npm package. The package name, version, and metadata are configured in `parser-wasm/Cargo.toml` via wasm-pack conventions.

The package includes:

- The compiled `.wasm` binary.
- JavaScript glue code (generated by wasm-pack).
- TypeScript declarations (Section 7.5).
- A README with usage examples.

## 8. WASI Target

### 8.1. Execution Model

The WASI crate compiles to `wasm32-wasip1` (or `wasm32-wasip2` when stable) and runs in any WASI-compatible runtime (Wasmtime, Wasmer, WasmEdge).

The binary is a filter: it reads input from stdin, processes it, and writes output to stdout.

### 8.2. I/O Protocol

Single-document mode (default):

1. Read all of stdin as UTF-8.
2. Parse with `parse_document`.
3. Write one `SlashParserResult` JSON object to stdout.
4. Terminate.

Context may be supplied via a command-line argument or environment variable (implementation-defined).

JSONL mode (when `--jsonl` flag is present or when stdin contains multiple documents separated by a delimiter): see Section 9.

### 8.3. JSONL Streaming

In JSONL mode, the WASI binary reads multiple documents from stdin. Document boundaries are implementation-defined but MUST be unambiguous. Two approaches are acceptable:

1. One file path per line on stdin: the binary reads each file and emits one JSONL line per file.
2. A delimiter-separated stream: documents are separated by a specific marker (e.g., a NUL byte or a line containing only `---`).

Each document produces one JSONL line on stdout per Section 9.

### 8.4. Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success (even if warnings were emitted) |
| 1 | I/O error (stdin read failure, stdout write) |
| 2 | Invalid command-line arguments |

The parser never fails. Exit code 0 is expected for all valid I/O operations, regardless of parse warnings.

## 9. JSONL Streaming

### 9.1. Line Format

Each line of JSONL output is exactly one `SlashParserResult` envelope, serialized as a single JSON object on one line (no embedded newlines within the JSON).

JSONL mode does NOT emit one command per line. Commands remain elements of the `commands` array inside each envelope.

### 9.2. Envelope Scoping

Each JSONL line is a self-contained parse result:

- Command IDs (`cmd-0`, `cmd-1`) are scoped to their envelope and reset for each input document.
- Text block IDs (`text-0`, `text-1`) are scoped to their envelope and reset for each input document.
- Warnings are scoped to their envelope.

Consumers can treat each line as an independent unit of work.

### 9.3. Pipeline Semantics

In a pipeline processing multiple input documents, each input produces one JSONL line, in the order the inputs were supplied.

The parser itself is not aware of the pipeline. The JSONL framing is the responsibility of the WASI binary or the Rust SDK consumer. The engine processes one document per call to `parse_document`.

## 10. Version Mapping

Three version numbers are in play:

| Version | Source | Location |
|---|---|---|
| Syntax RFC version | [SYNTAX-RFC] | `version` field in JSON envelope |
| Engine spec version | [ENGINE-SPEC] | `SPEC_VERSION` const |
| SDK spec version | This document | `Cargo.toml` |

The `version` field in the JSON envelope MUST contain the syntax RFC version (`"1.1.0"`), not the engine or SDK version. This is because consumers validate output against the syntax RFC's JSON schema, and the version field is how they determine which schema to use.

The SDK is responsible for mapping `SPEC_VERSION` from the engine to the correct syntax RFC version in the serialized envelope. This mapping is maintained in the SDK crate.

If the engine's `SPEC_VERSION` changes without a corresponding syntax RFC change, the envelope version stays the same. If the syntax RFC version changes, the SDK MUST be updated to emit the new version.

## 11. Conformance

### 11.1. JSON Schema Validation

The serialized output of all three targets (Rust SDK, WASM, WASI) MUST validate against the JSON schema defined for [SYNTAX-RFC].

The SDK test suite SHOULD include automated schema validation for a representative set of inputs.

### 11.2. Determinism

Per [SYNTAX-RFC] Section 8.4, given identical input and context, the serialized JSON output MUST be byte-for-byte identical across invocations.

This requires deterministic JSON serialization. serde_json produces deterministic output for a given Rust struct layout. The SDK MUST NOT introduce non-determinism through HashMap iteration order in the context pass-through. If context is accepted as `serde_json::Value`, the `Value::Object` variant uses serde_json's `Map` (which preserves insertion order), which is deterministic.

### 11.3. Roundtrip Fidelity

Per [SYNTAX-RFC] Section 8.3, the SDK supports roundtrip testing. The SDK MAY provide a formatter utility that reconstructs plaintext from a `SlashParserResult`. If provided, the roundtrip invariant `P(F(P(I))) = P(I)` MUST hold.

## 12. Security Considerations

The SDK inherits the engine's security properties (no I/O, no command execution, pure function). Additional considerations for the SDK layer:

- WASM: the module runs in a sandboxed environment. It has no access to the filesystem, network, or host memory beyond its linear memory. Output size is bounded by input size.
- WASI: the binary has access to stdin, stdout, and (if file paths are used) the pre-opened filesystem directories granted by the runtime. Implementations SHOULD NOT request more capabilities than needed.
- JSON output: the SDK serializes user-supplied content (command arguments, text blocks) into JSON strings. serde_json handles escaping. The SDK MUST NOT perform any additional escaping or sanitization that would alter the content.

## 13. Future Work

The following items are explicitly out of scope for this version but may be addressed in future revisions:

- Per-command JSONL streaming (one command per line instead of one envelope per line).
- Binary serialization formats (MessagePack, CBOR) as alternatives to JSON.
- A C FFI for integration with C/C++/Python/Ruby via shared library.
- A formatter utility for roundtrip reconstruction.
- Language-specific SDK packages beyond JavaScript/TypeScript (Python, Go, etc.).

## 14. Migration Notes from v0.1.0

### 14.1. Syntax RFC Reference

- v0.1.0: Referenced Slash Command Parser Syntax Specification v0.3.1.
- v0.2.0: References Slash Command Syntax v1.1.0 [SYNTAX-RFC].

Impact: Update the version string emitted in the JSON envelope `version` field from `"0.3.1"` to `"1.1.0"`.

### 14.2. Engine Spec Reference

- v0.1.0: Referenced Engine Specification v0.4.0.
- v0.2.0: References Engine Specification v0.5.0 [ENGINE-SPEC].

Impact: The engine now rejects command names ending with a hyphen (per [SYNTAX-RFC] Section 4.1). Inputs like `/cmd-` are classified as text, not commands. SDK tests that relied on trailing-hyphen command names must be updated.

### 14.3. Document Format

- v0.1.0: RFC-style plaintext (.txt).
- v0.2.0: Markdown (.md), consistent with the engine spec.

This is an internal project document, not a published RFC.

## Normative References

- **[SYNTAX-RFC]** Davidson, T. D., "Slash Command Syntax, Version 1.1.0", March 2026.
- **[ENGINE-SPEC]** Davidson, T. D., "Slash Command Parser Engine Specification, Version 0.5.0", March 2026.

## Author

Tom D. Davidson  
Email: tom@tomdavidson.org  
URI: https://tomdavidson.org  
Utah
