---
title: Soundcheck
description: How Solidus is tested. Three layers, property-based testing, structured fuzz testing, and five provable guarantees.
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

- **Totality:** `parse_document` never panics on arbitrary input up to 500 characters.
- **Determinism:** parsing the same input twice always produces identical output.
- **ID sequencing:** command IDs are always `cmd-0`, `cmd-1`, ... in order. Text block IDs are
  always `text-0`, `text-1`, ... in order.
- **Completeness:** every physical line in the input is accounted for by exactly one command or text
  block. No line is lost or double-counted.
- **Text-only invariant:** input with no slash-prefixed lines produces zero commands and at least
  one text block.
- **Fence body preservation:** fenced content passes through the state machine verbatim. No joining,
  no escaping, no transformation.
- **Unclosed fence warning:** any fence reaching EOF without a closer always produces exactly one
  `unclosed_fence` warning.

When proptest finds a failing input, it automatically shrinks it to the minimal reproducing case and
writes it to a regression file. These files are committed to version control. Once a failure class
is found, it can never silently return.

## Fuzz Testing

Fuzz testing is where the parser meets genuine adversarial input. Two fuzz targets attack the parser
from complementary angles, and each target runs on two independent fuzzing engines.

### Two Targets

**`parse_document_unstructured`** feeds raw arbitrary bytes (filtered to valid UTF-8) into
`parse_document`. This is pure chaos: the fuzzer has no concept of slash commands, fences, or line
structure. It explores the full input space looking for panics and invariant violations.

```rust
fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let result = parse_document(input);
        assert!(!result.version.is_empty());
        // ... sequential IDs, valid argument modes, determinism
    }
});
```

**`parse_document_structured`** uses the `engine-fuzz-common` crate to generate syntactically
plausible documents via Rust's `Arbitrary` trait. A `FuzzDoc` is a sequence of up to 20
`Fragment` variants (text lines, single-line commands, fenced commands, unclosed fences, joined
commands, invalid slashes, blanks) that are rendered into a concrete input string before parsing.
This approach biases the fuzzer toward structurally interesting inputs that exercise deep parser
states.

```rust
fuzz_target!(|doc: FuzzDoc| {
    let input = render_doc(&doc);
    let result = parse_document(&input);
    assert_ids_sequential(&result);
    assert_argument_modes(&result);
    assert_unclosed_fence_warning(&doc, &result);
    // ... determinism
});
```

Both harnesses assert the same invariants: version is populated, IDs are sequential, argument modes
are valid (`SingleLine` or `Fence`), and output is deterministic. The structured harness adds a
semantic check: if the last fragment is an `UnclosedFence`, the result must contain an
`unclosed_fence` warning.

### Two Engines

Each target compiles against two fuzzing engines via Cargo features:

| Engine | Feature flag | Strength |
|---|---|---|
| libFuzzer | `libfuzzer` | Industry-standard coverage-guided fuzzer from LLVM |
| LibAFL | `libafl` | Rust-native fuzzer with advanced scheduling and mutation strategies |

A compile-time guard prevents enabling both simultaneously. The 2x2 matrix (2 targets x 2 engines)
provides four independent attack vectors against the parser.

### Shared Assertion Library

The `engine-fuzz-common` crate is shared between the fuzz harnesses and the regression test
harnesses. It contains:

- **Types:** `FuzzDoc`, `Fragment`, `CmdName`, `Payload`, `Header`, `FenceLang`, `FenceBody`,
  `InvalidSlashKind` with bounded `Arbitrary` implementations.
- **Renderer:** `render_doc` converts a `FuzzDoc` into a concrete input string, sanitizing
  newlines, escaping backticks, and stripping trailing join markers between fragments.
- **Assertions:** `assert_ids_sequential`, `assert_argument_modes`, `assert_unclosed_fence_warning`
  validate structural invariants shared by harnesses and regression tests.

### Regression Tests

Crash inputs discovered by fuzzing are minimized and saved to `fuzz/regressions/`. Two
`datatest-stable` harnesses auto-discover regression files and replay them as standard
`cargo test` cases:

- `fuzz_regression_parse_document_unstructured` replays raw bytes from
  `fuzz/regressions/parse_document_unstructured/`.
- `fuzz_regression_parse_document_structured` deserializes `FuzzDoc` via `Arbitrary` from
  `fuzz/regressions/parse_document_structured/`.

Both replay harnesses assert the same invariants as the live fuzz targets, plus determinism (parse
twice, compare results). To add a regression: drop a file in the appropriate directory and run
tests. No test registration, no boilerplate. The `datatest-stable` harness discovers it
automatically.

### CI Automation

Two GitHub Actions workflows manage fuzz runs:

**`main-fuzz.yml` (weekly saturation)** runs every Saturday at 2:00 AM UTC. It discovers all
projects with a `fuzz-ci` moon task, pulls the cached corpus, runs each target for up to 4 hours
with 4 parallel jobs, then pushes the grown corpus back to cache. If crashes are found, the
workflow triages and minimizes them, uploads artifacts, and opens a pull request to commit the new
regression files.

**`pr-fuzz.yml` (label-triggered)** activates when a `fuzz` label is added to a pull request (or
via manual dispatch). It runs the same saturation flow against the PR branch for up to 5 hours per
target. On completion, it commits any regression files directly to the PR branch and posts a status
comment: green for clean, red with file counts and artifact links for crashes.

Both workflows use the `fuzz-cache` composite action for two-tier corpus persistence: CI cache for
fast restore, with the corpus growing monotonically across runs. Paths are resolved from moon
project metadata (`corpus-path`, `regressions-path`).

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
