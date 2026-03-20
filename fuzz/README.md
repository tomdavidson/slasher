# Fuzz Testing

Fuzz targets for the slash-parser, powered by
[honggfuzz](https://github.com/rust-fuzz/honggfuzz-rs).

## Setup

```bash
cargo install honggfuzz
```

You also need a C compiler and `libunwind` headers (usually already present on Linux). For
debugger-based crash replay, install `lldb` or `gdb` (see Debugging Crashes).

## Targets

| Target            | Description                                                         |
| ----------------- | ------------------------------------------------------------------- |
| `fuzz_parse`      | Asserts `parse_slash_commands` never panics on arbitrary input      |
| `parse_to_domain` | Roundtrip: parse → plaintext → re-parse, assert structural equality |

## Running

All commands assume you are in the `fuzz/` directory.

```bash
cd fuzz

# Run a target (runs indefinitely until Ctrl-C)
cargo hfuzz run fuzz_parse
cargo hfuzz run parse_to_domain

# Use more threads for faster throughput
HFUZZ_RUN_ARGS="-n 4" cargo hfuzz run fuzz_parse

# Stop on first crash (useful for quick checks or CI)
HFUZZ_RUN_ARGS="--exit_upon_crash" cargo hfuzz run parse_to_domain
```

### How long to run

Watch the `Cov Update` field in the dashboard. Once it exceeds 10-15 minutes without a new coverage
discovery, the fuzzer has likely exhausted reachable paths. For CI smoke tests, 5-10 minutes is
usually enough. For thorough pre-release runs, a few hours or overnight is standard.

### Dashboard quick reference

| Field         | Meaning                                                           |
| ------------- | ----------------------------------------------------------------- |
| Iterations    | Total mutated inputs tested                                       |
| Speed         | Executions per second (current / average)                         |
| Crashes       | Total crashes found, with unique count                            |
| Corpus Size   | Number of coverage-maximizing inputs accumulated                  |
| Cov Update    | Time since the last new coverage discovery                        |
| Coverage edge | Code edges hit out of total edges (primary coverage metric)       |
| Coverage cmp  | Comparison operands learned (helps the fuzzer solve magic values) |

## Debugging Crashes

When honggfuzz finds a crash it writes two files into `hfuzz_workspace/<target>/`:

- `HONGGFUZZ.REPORT.TXT` containing the signal, stack hash, and raw stack trace.
- A `.fuzz` file named after the signal and stack (e.g.
  `SIGABRT.PC.7ffff7c9eb2c.STACK.f2069d7a5.CODE.-6.ADDR.0.INSTR.mov____%eax,%r14d.fuzz`). This is
  the raw binary input that triggered the crash.

The `.fuzz` file is the crash reproducer. The `.honggfuzz.cov` files in `input/` are the coverage
corpus (not crashes).

### Step 1: Inspect the crash input

Crash inputs are often small. Start with `xxd` to see the raw bytes:

```bash
xxd hfuzz_workspace/parse_to_domain/SIGABRT.*.fuzz
```

### Step 2: Write a standalone repro test

The most reliable way to reproduce and debug a crash is a standard `cargo test` in `parser-core`.
This avoids honggfuzz's build-environment checks and gives you full `RUST_BACKTRACE` output,
`eprintln!` diagnostics, and IDE debugger support.

Use the bytes from `xxd` to construct the input literal:

```rust
#[test]
fn fuzz_crash_repro() {
    let input = "\t/ru4\x0b/"; // from xxd of the .fuzz file

    let ast1 = parse_to_domain(input, ParserContext::default());
    let plaintext = to_plaintext(&ast1);

    eprintln!("input bytes:     {:?}", input.as_bytes());
    eprintln!("plaintext bytes: {:?}", plaintext.as_bytes());
    eprintln!("ast1: {:#?}", ast1.commands);

    let ast2 = parse_to_domain(&plaintext, ParserContext::default());
    eprintln!("ast2: {:#?}", ast2.commands);

    assert_eq!(ast1.commands.len(), ast2.commands.len());
    for (a, b) in ast1.commands.iter().zip(ast2.commands.iter()) {
        assert_eq!(a.name, b.name, "Command name mismatch");
        assert_eq!(a.arguments.mode, b.arguments.mode, "Mode mismatch");
        assert_eq!(a.arguments.payload, b.arguments.payload, "Payload mismatch");
    }
}
```

Run with:

```bash
cd parser-core
RUST_BACKTRACE=1 cargo test fuzz_crash_repro -- --nocapture
```

### Step 3 (optional): Replay under a debugger

`cargo hfuzz run-debug` replays the `.fuzz` file through the fuzz harness built in debug mode. It
requires `lldb` by default.

```bash
# Using lldb (default, requires: sudo apt install lldb)
cargo hfuzz run-debug parse_to_domain \
  hfuzz_workspace/parse_to_domain/SIGABRT.*.fuzz

# Using gdb instead
HFUZZ_DEBUGGER=rust-gdb cargo hfuzz run-debug parse_to_domain \
  hfuzz_workspace/parse_to_domain/SIGABRT.*.fuzz
```

The debugger approach is most useful for segfaults or memory corruption. For assertion failures
(`SIGABRT`), the standalone repro test in Step 2 is usually faster and gives better output.

### Crash assertion reference

Common assertion messages from `parse_to_domain`:

| Message                                | Likely cause                                   |
| -------------------------------------- | ---------------------------------------------- |
| Command count mismatch on roundtrip    | `to_plaintext` loses or creates commands       |
| Command name mismatch                  | Roundtrip changes a command name               |
| Mode mismatch / Payload mismatch       | Argument serialization is lossy                |
| Text block count mismatch on roundtrip | Plaintext conversion drops or adds text blocks |

### Minimizing crash inputs

Crash files are often larger than necessary. Copy the `.fuzz` file into the input corpus and run the
fuzzer in minimize mode:

```bash
mkdir -p hfuzz_workspace/parse_to_domain/input
cp hfuzz_workspace/parse_to_domain/*.fuzz hfuzz_workspace/parse_to_domain/input/
HFUZZ_RUN_ARGS="-M" cargo hfuzz run parse_to_domain
```

## Regression Tests

After fixing a bug found by fuzzing, promote the repro test to a permanent regression test. Store
the crash input either as an inline string literal or as a file in `fuzz/regressions/`:

```rust
// Option A: inline (preferred for small inputs)
#[test]
fn fuzz_roundtrip_regression_001() {
    let input = "\t/ru4\x0b/";
    let ast1 = parse_to_domain(input, ParserContext::default());
    let plaintext = to_plaintext(&ast1);
    let ast2 = parse_to_domain(&plaintext, ParserContext::default());
    assert_eq!(ast1.commands.len(), ast2.commands.len());
    for (a, b) in ast1.commands.iter().zip(ast2.commands.iter()) {
        assert_eq!(a.arguments.mode, b.arguments.mode);
        assert_eq!(a.arguments.payload, b.arguments.payload);
    }
}

// Option B: file-based (for larger inputs)
#[test]
fn fuzz_roundtrip_regression_002() {
    let input = include_str!("../../fuzz/regressions/crash_002.txt");
    // ... same assertions
}
```

Commit regression test inputs in `fuzz/regressions/` so the bugs stay caught.

## Coverage Reporting

Honggfuzz's dashboard shows edge coverage during a run, but for detailed line-level reports you can
replay the corpus through LLVM's coverage tooling. Output is available as HTML, JSON, or LCOV for
integration with other reporters.

```bash
# 1. Build with LLVM coverage instrumentation (not honggfuzz instrumentation)
RUSTFLAGS="-C instrument-coverage" cargo build --bin parse_to_domain

# 2. Replay the corpus through the instrumented binary
LLVM_PROFILE_FILE="fuzz-%p.profraw" ./target/debug/parse_to_domain \
  hfuzz_workspace/parse_to_domain/input/

# 3. Merge raw profiles
llvm-profdata merge -sparse fuzz-*.profraw -o fuzz.profdata

# 4a. HTML report
llvm-cov show ./target/debug/parse_to_domain \
  -instr-profile=fuzz.profdata \
  -format=html -output-dir=coverage-html/

# 4b. LCOV export (for Codecov, Coveralls, etc.)
llvm-cov export ./target/debug/parse_to_domain \
  -instr-profile=fuzz.profdata \
  -format=lcov > fuzz-coverage.lcov

# 4c. JSON export
llvm-cov export ./target/debug/parse_to_domain \
  -instr-profile=fuzz.profdata \
  -format=text > fuzz-coverage.json
```

## Corpus Management

The corpus lives in `hfuzz_workspace/<target>/input/`. Each `.honggfuzz.cov` file is a single test
input that contributes unique coverage.

### Minimize the corpus

After a long run, the corpus accumulates redundant inputs. Minimize it to the smallest set that
preserves total coverage:

```bash
HFUZZ_RUN_ARGS="-M" cargo hfuzz run fuzz_parse
```

### Seeding

To give the fuzzer a head start, drop real input files into the input directory before starting a
run. Test fixtures from `riff/tests/fixtures/` are good candidates:

```bash
mkdir -p hfuzz_workspace/fuzz_parse/input
cp ../riff/tests/fixtures/*.md hfuzz_workspace/fuzz_parse/input/
cargo hfuzz run fuzz_parse
```

## Directory Layout

| Path                                            | Contents                                     | In git? |
| ----------------------------------------------- | -------------------------------------------- | ------- |
| `fuzz/`                                         | Fuzz target source code and Cargo.toml       | Yes     |
| `fuzz/regressions/`                             | Minimized crash inputs for regression tests  | Yes     |
| `hfuzz_target/`                                 | Compiled instrumented binaries (build cache) | No      |
| `hfuzz_workspace/<target>/input/`               | Coverage corpus (`.honggfuzz.cov` files)     | No      |
| `hfuzz_workspace/<target>/*.fuzz`               | Crash reproducer inputs                      | No      |
| `hfuzz_workspace/<target>/HONGGFUZZ.REPORT.TXT` | Crash signal/stack report                    | No      |
| `parser-core/proptest-regressions/`             | Proptest seeds (complementary to fuzzing)    | Yes     |
