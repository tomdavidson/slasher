---
title: Soundcheck
description: How Solidus is tested. Three layers, property-based testing, fuzz testing, and five provable guarantees.
---

The parser is a total function. It accepts any UTF-8 input and always returns a valid result. It
cannot panic, cannot return an error, and cannot fail. The testing strategy is designed to prove
that guarantee holds under adversarial conditions.

## Three-Layer Test Architecture

Tests are organized by scope, not by type.

**Layer 1: Unit tests** sit inside every module file, exercising individual functions with direct
calls. Each parsing stage (normalization, line joining, line classification, command accumulation,
text collection, finalization) has its own unit tests that verify behavior in isolation. These are
the atoms.

**Layer 2: Integration tests** span multiple modules within the application layer. They exercise
cross-module composition through `parse_document`, including every parsing example from Appendix B
of the Syntax Specification. These are the molecules.

**Layer 3: Cross-layer tests** cover the full public API surface. They validate structural
invariants that span the entire pipeline: ID sequencing, line-range consistency, deterministic
output, and roundtrip fidelity.

All three layers run on every commit via `cargo nextest run`. Property tests (see below) are gated
behind a `tdd` feature flag so watch-mode iteration stays fast while the full suite runs before
every push.

## Property-Based Testing

Randomly generated inputs validate structural invariants that must hold for every possible document.
The engine uses [proptest](https://crates.io/crates/proptest) to generate inputs at scale, testing
properties like:

- Roundtrip fidelity: parse, format, parse again yields structurally equivalent output.
- Determinism: parsing the same input twice always produces identical output.
- ID sequencing: command IDs are always `cmd-0`, `cmd-1`, ... in order. Text block IDs are always
  `text-0`, `text-1`, ... in order.
- Line-range consistency: every element's line range falls within the input's physical line count.
  Ranges never overlap across elements. Ranges appear in document order.
- Completeness: every physical line in the input is accounted for by exactly one command or text
  block.

When proptest finds a failing input, it automatically shrinks it to the minimal reproducing case and
writes it to a regression file. These files are committed to version control. Once a failure class
is found, it can never silently return.

## Fuzz Testing

A [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) harness feeds arbitrary `&[u8]` to the
parser and asserts the engine never panics. The harness is simple because the parser's total
function guarantee makes the contract clear: if it doesn't panic, it passes.

```rust
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // should never panic, regardless of input
        let _ = solidus::parse_document(s);
    }
});
```

Crash inputs are automatically minimized and added as permanent regression tests. The fuzz corpus
grows monotonically: every interesting input discovered during a run is preserved for future
sessions. Fuzz runs are scheduled (not per-commit) to allow for deep exploration without blocking
the development loop.

## What We Prove

These aren't aspirations. They're properties enforced by the architecture and verified by the test
suite.

**Deterministic.** Identical input always produces identical output. The engine uses no randomness,
no HashMap iteration order, and no floating point arithmetic.

**Total.** Every input produces a valid `ParseResult`. There is no `Result<_, E>` return type. There
is no error path. Empty input returns empty results. Malformed input returns partial results with
warnings.

**Safe.** No `unsafe` code anywhere in the engine. The only runtime dependency is `thiserror` for
error type derivation.

**Faithful.** Parse, format, parse again yields a structurally equivalent result. This is the
roundtrip fidelity invariant defined in the Syntax Specification (Section 8.3).

**Opaque.** The parser never interprets, tokenizes, or validates argument content. Your payload is
your business. The parser's job is to find the boundaries and hand you the content verbatim.
