Your direction matches your own Rust + clean-arch guidelines and is idiomatic Rust, with a couple of
naming tweaks I’d suggest.

## Function structure and colocated tests

Breaking `to_plaintext` into helpers like this is exactly what you want:

```rust
fn write_command(output: &mut String, cmd: &Command) {
    match cmd.arguments.mode {
        ArgumentMode::SingleLine => write_single_line(output, cmd),
        ArgumentMode::Continuation => write_continuation(output, cmd),
        ArgumentMode::Fence => write_fence(output, cmd),
    }
}
```

Having `write_command`, `write_single_line`, `write_continuation`, and `write_fence` in the same
file, plus a `#[cfg(test)] mod tests` at the bottom with fast, happy-path unit tests, is perfectly
aligned with your Rust testing doc: fast tests live in `cfg(test) mod tests` in the same file;
heavier property tests and integration tests move to `tests/` and fuzz harnesses live under `fuzz/`.

That gives you:

- Small, focused functions per concern.
- Colocated “fast” tests with direct positive/negative assertions.
- Property-based and fuzz tests elsewhere, as you prefer.

## Folder and layer structure

For the `parser` crate’s internal layout, mapping your JS boundaries to Rust:

- **domain/**: pure types and invariants: command/text-block types, argument modes, maybe
  lexer/token types later. No serde, no IO. This matches your “domain.rs pure, no serde” rule.
- **application/**: parsing state machine and formatting / printing (JSON and plaintext), since they
  orchestrate domain types and build DTOs. This matches your “use cases / orchestration / ports”
  bucket.
- **infrastructure/**: WASM bindings, FFI shims, logging/tracing glue. That’s the adapter layer.
- **tests/**: slower integration-style tests that exercise whole flows or multiple modules together.
- **proptest-regressions/** and **fuzz/**: stay as-is, under the parser crate instead of repo root.

In Rust terms, you might land on something like:

```text
parser/
  src/
    domain/
      mod.rs       // re-exports
      types.rs     // Command, TextBlock, ArgumentMode, etc.
      errors.rs    // ParseError, domain-level errors
    application/
      mod.rs
      parse.rs     // parse_to_domain, state machine
      format_json.rs   // JSON serialization boundary
      format_text.rs   // to_plaintext + helpers
    infrastructure/
      mod.rs
      wasm_js.rs   // wasm-bindgen adapter
      wasm_wasi.rs // WIT adapter later
    lib.rs         // pub use domain::*, application::parse_to_domain, etc.
  tests/
    roundtrip.rs   // slower multi-file/integration-ish tests
  proptest-regressions/
  fuzz/
```

This lines up with the Rust patterns doc: organize by feature, then by layer (`domain.rs`, `app.rs`,
`infra.rs`), promote to multiple files once a module grows, and keep domain free of serde.

## Naming recommendations

You’re right that `serialize.rs` vs `to_plaintext.rs` is asymmetric. I’d suggest:

- `format_json.rs` (or `output_json.rs`): owns DTOs + `to_json`.
- `format_text.rs` (or `output_text.rs`): owns `to_plaintext` and helpers.

Both live under `application/` since they are output-format concerns, not infrastructure (they don’t
talk to IO, just shape data). That also distances them from your infra error-mapping and transport
adapters in other projects.

Within `application::format_text`, functions like:

```rust
pub fn to_plaintext(result: &SlashParseResult) -> String { … }

fn write_command(output: &mut String, cmd: &Command) { … }

fn write_single_line(output: &mut String, cmd: &Command) { … }

fn write_continuation(output: &mut String, cmd: &Command) { … }

fn write_fence(output: &mut String, cmd: &Command) { … }

#[cfg(test)]
mod tests { … }
```

fit your “free functions for domain logic, not methods” pattern.

## Clean-arch fit and “is this bad practice?”

Given your own rules:

- Domain imports nothing external; DTOs and serde live outside domain.
- Application orchestrates domain, defines ports, but doesn’t know about transport details.
- Infrastructure implements ports and touches IO.

Your proposed layout is consistent with that and is not “bad Rust practice.” Rust doesn’t require
flat `src/lib.rs` with everything; feature-and-layer submodules are common in more complex crates,
and your approach is in line with your Rust patterns doc’s “organize by feature, then by layer,
promote to workspace crates when it gets big.”

One minor tweak: I’d keep `parser/` as the crate name rather than `parser-core` so it matches
ADR-0006 and avoids “application/parser.rs” repetition. That’s purely naming and you already plan to
rename.

## Clarifying question

For the textual formatting layer: do you want `to_plaintext` to be strictly “round-trip faithful” to
spec semantics only, or byte-for-byte identical to original input (including all whitespace), where
that’s possible? Your testing doc leans hard into roundtrip correctness, but exact byte preservation
may constrain how we model headers vs payloads.
