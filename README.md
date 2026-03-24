# Solidus

**The gold standard for slash command parsing.**

Solidus parses `/commands` in UTF-8 text. It implements the
[Slash Command Syntax v1.1.0](https://your-solidus-site.dev/spec/syntax/)
specification with a pure Rust engine: no IO, no serialization, no unsafe,
no global state. `parse_document` is a total function. It accepts any input
and always returns a valid result.

## How it works

Given this input:

````text
Welcome to the deployment system.

/deploy staging
/mcp call_tool write_file ```json
{ "path": "/src/index.ts" }
```
/notify team --channel ops

Deployment complete.
````

Solidus partitions it into commands and text blocks in document order:

```
text-0   "Welcome to the deployment system.\n"        lines 0-1
cmd-0    /deploy staging                    (single-line)   line 2
cmd-1    /mcp call_tool write_file ```json  (fence, json)   lines 3-5
cmd-2    /notify team --channel ops         (single-line)   line 6
text-1   "Deployment complete."                        lines 7-7
```

Every physical line belongs to exactly one element. Commands get sequential
IDs (`cmd-0`, `cmd-1`, ...), text blocks get their own (`text-0`, `text-1`,
...), and the parser never interprets argument content. Your payload is
your business.

## Architecture

```
solidus/
  parser/          Engine crate — pure Rust, no dependencies beyond thiserror
  fuzz/            cargo-fuzz harness
```

The engine crate is the single source of truth. It produces Rust domain types
(`ParseResult`, `Command`, `TextBlock`, `Warning`) with no serialization
opinion. SDK crates for JSON serialization, WASM bindings, and a WASI binary
are specified but not yet implemented.

## Quick start

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

## Usage

```rust
use solidus::parse_document;

let result = parse_document("/echo hello world");

assert_eq!(result.commands.len(), 1);
assert_eq!(result.commands[0].name, "echo");
assert_eq!(result.commands[0].arguments.payload, "hello world");
assert!(result.warnings.is_empty());
```

`parse_document` returns a `ParseResult`:

```rust
pub struct ParseResult {
    pub version: String,
    pub commands: Vec<Command>,
    pub text_blocks: Vec<TextBlock>,
    pub warnings: Vec<Warning>,
}
```

There is no `Result<_, E>` wrapper. There is no error type. The function
succeeds for every input, including empty strings, binary-looking UTF-8,
and malformed fences.

## Input format

Three argument modes are supported:

**Single-line** — arguments on the same logical line as the command:

```text
/deploy staging --region us-west-2
```

**Backslash continuation** — physical lines joined before parsing:

```text
/deploy production \
  --region us-west-2 \
  --canary
```

**Fenced** — multi-line verbatim payloads delimited by backticks:

````text
/mcp call_tool write_file ```json
{ "path": "/src/index.ts" }
```
````

Fence content is never subject to line joining. Backslashes, command
triggers, and blank lines inside a fence are literal payload.

## Guarantees

| Property | What it means |
|---|---|
| Total function | Every input produces a valid `ParseResult`. No panics, no errors. |
| Deterministic | Identical input always produces identical output. No randomness, no HashMap iteration, no floats. |
| No unsafe | Safe Rust only across the entire engine. |
| Roundtrip fidelity | Parse → format → parse yields structurally equivalent output. |
| Opaque payload | The parser never interprets, tokenizes, or validates argument content. |

## Testing

- Unit tests in every module exercising each parsing stage in isolation
- Integration tests covering all Appendix B examples from the syntax spec
- Property-based tests (proptest) validating structural invariants across
  randomly generated inputs
- Fuzz testing (cargo-fuzz / libFuzzer) asserting the engine never panics
  on arbitrary bytes

Proptest regressions and fuzz crash inputs are committed as permanent
regression tests. The fuzz corpus only grows.

```bash
# Full test suite
cargo nextest run

# Watch mode (skips slow property tests)
cargo nextest run --features tdd

# Fuzz
cargo fuzz run fuzz_parse
```

## Roadmap

| Target | Status |
|---|---|
| Engine crate (parser) | Done |
| Syntax spec (v1.1.0) | Done |
| Engine spec (v0.5.0) | Done |
| Rust SDK (serde + JSON envelope) | Planned |
| WASM module (wasm-bindgen + TypeScript) | Planned |
| WASI binary (stdin/stdout, JSONL) | Planned |
| `riff` CLI | Planned |

## Documentation

Full specification, parsing examples, and testing details are available at
the [Solidus documentation site](https://your-solidus-site.dev).

## License

MIT
