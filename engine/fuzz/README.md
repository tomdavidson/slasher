# Solidus Engine Fuzzing System

This directory contains an automated fuzz testing pipeline for the Solidus Engine. It uses both
libFuzzer and LibAFL, driven by Moonrepo task orchestration, with continuous regression testing
via Cargo build scripts and GitHub Actions CI.


## Architecture

The pipeline has three phases that form a feedback loop: Discovery, Triage, and Regression
Prevention.

` ``text
┌────────────────────────────────────────────────────────────────────────┐
│ 1. Discovery (moon run engine-fuzz:fuzz-saturate)                      │
│                                                                        │
│   [ libFuzzer ] ─────┬───► Raw unstructured bytes ───────┐             │
│                      │                                   │             │
│   [ LibAFL ] ────────┘                                   ▼             │
│                                                   [ engine/src/lib.rs ]│
│   [ libFuzzer ] ─────┬───► arbitrary::Arbitrary ─────────▲             │
│                      │     (Structured generator)        │             │
│   [ LibAFL ] ────────┘                                   │             │
└──────────────────────┼───────────────────────────────────┼─────────────┘
                       │ (Crashes)                         │ (Valid inputs)
                       ▼                                   ▼
              fuzz/artifacts/<target>/             fuzz/corpus/<target>/
                       │                                   │
┌──────────────────────┼───────────────────────────────────┼─────────────┐
│ 2. Triage & Compact  │                                   │             │
│                      ▼                                   ▼             │
│           moon run :fuzz-triage               moon run :fuzz-compact   │
│            (cargo fuzz tmin)                   (cargo fuzz cmin)       │
│                      │                                   │             │
│                      ▼                                   ▼             │
│          fuzz/regressions/<target>/              fuzz/corpus/<target>/ │
└──────────────────────┼─────────────────────────────────────────────────┘
                       │ (Minimized crashes)
                       ▼
┌────────────────────────────────────────────────────────────────────────┐
│ 3. Regression Prevention (cargo test)                                  │
│                                                                        │
│   [ engine/build.rs ] ──► Reads fuzz/regressions/ at compile time      │
│           │                                                            │
│           ▼                                                            │
│   Generates fuzz_regression_<target>.rs in $OUT_DIR                    │
│           │                                                            │
│           ▼                                                            │
│   [ cargo test ] ───────► Executes minimized crashes as unit tests     │
│                           every build, preventing regressions          │
└────────────────────────────────────────────────────────────────────────┘
` ``


## How It Works

### Multi-Engine Fuzzing (`fuzz-run.sh`)

Different fuzzing engines use different mutation heuristics, so we run a 2x2 matrix:

- Targets: `parse_document_unstructured` (raw bytes) and `parse_document_structured`
  (`arbitrary::Arbitrary` AST-aware generation).
- Engines: Standard LLVM libFuzzer and LibAFL, selected via the `FUZZ_FEATURES` env var
  (e.g. `--no-default-features --features libafl`).

`fuzz-run.sh` accepts a target name, max time, job count, and an optional `--replay` flag. It
handles CWD resolution, pre/post crash counting, and sweeps stray `fuzz-*.log` files into
`artifacts/<target>/logs/`.


### Triage and Compaction (`fuzz-manage.sh`)

Two subcommands manage the data fuzzers generate:

- `triage` (`cargo fuzz tmin`): Minimizes crash files from `artifacts/<target>/` to the smallest
  byte sequence that still triggers the panic. Saves results into `regressions/<target>/` with a
  hash-based filename to avoid duplicates.
- `compact` (`cargo fuzz cmin`): Removes redundant corpus entries that don't increase coverage,
  keeping only the most efficient inputs.


### Automated Regression Tests (`build.rs`)

Once a crash is minimized into `regressions/`, it becomes a permanent test with no manual wiring.

- During `cargo build` or `cargo test`, `engine/build.rs` scans the `regressions/` directory.
- It copies files into Cargo's `$OUT_DIR` and generates `#[test]` functions using
  `include_bytes!(...)`.
- These tests execute on every build, ensuring historical crashes can never silently regress.


## Moon Task Graph

Moon manages the parallelization of the full 2x2 engine/target matrix.

` ``text
fuzz
├── fuzz-saturate (parallel)
│   ├── fuzz-structured-saturate
│   │   ├── libfuzz-structured-saturate    (fuzz-run.sh ... 86400 180)
│   │   └── libafl-structured-saturate     (fuzz-run.sh ... 86400 180 + FUZZ_FEATURES)
│   │   └── compact (fuzz-manage.sh compact)
│   └── fuzz-unstructured-saturate
│       ├── libfuzz-unstructured-saturate
│       └── libafl-unstructured-saturate
│       └── compact
└── fuzz-triage (sequential, after saturate)
    ├── triage parse_document_unstructured
    └── triage parse_document_structured

fuzz-ci (parallel, all four variants with -runs=0)
├── libfuzz-structured-ci
├── libfuzz-unstructured-ci
├── libafl-structured-ci
└── libafl-unstructured-ci
` ``

### Commands

` ``bash
# CI replay: re-run existing corpus with -runs=0 (seconds, catches regressions)
moon run engine-fuzz:fuzz-ci

# Full saturation: 24-hour parallel fuzzing + compaction (local or CI)
moon run engine-fuzz:fuzz-saturate

# Root task: saturate then triage sequentially
moon run engine-fuzz:fuzz

# Triage only: minimize new crash artifacts into regressions/
moon run engine-fuzz:fuzz-triage
` ``


## CI Automation

Two GitHub Actions workflows automate fuzzing in CI.

### Trunk Saturation (`main-fuzz.yml`)

Runs weekly (Saturday 2 AM UTC) and on manual dispatch. Discovers all projects with a `fuzz-ci`
task via `moon-q-projects`, then runs `fuzz-saturate` across them in a matrix. If crashes are found,
it triages them automatically and opens a PR with the minimized regression files via the `pr-create`
composite action.

### PR Saturation (`pr-fuzz.yml`)

Triggered by adding the `fuzz` label to a PR, or via `workflow_dispatch` with a PR number. Runs the
same discover/saturate/triage cycle against the PR branch. Regression files are committed directly
to the PR branch (attributed to the last branch author). Results are reported as a commit status and
sticky PR comment via the `pr-set-status` composite action.

Both workflows use these shared composite actions:

- `setup`: Checkout, toolchain installation, Cargo/Moon/pnpm caches
- `moon-q-projects`: Discover fuzz-capable projects with metadata
- `moon-run`: Execute moon tasks with retrospect reporting
- `fuzz-cache`: Per-project corpus caching with main/PR isolation
- `git-stage-artifacts`: Download and place matrix artifacts into project directories
- `git-commit-push`: Commit and push with last-author attribution
- `pr-create`: Idempotent PR creation (trunk workflow)
- `pr-set-status`: Commit status + sticky PR comment (PR workflow)
- `resolve-pr`: Normalize PR metadata across event types


## State Management

- `fuzz/corpus/<target>/`: Gitignored. Persisted exclusively via GitHub Actions caching
  (`fuzz-cache` action) to avoid repository bloat.
- `fuzz/artifacts/<target>/`: Gitignored. Temporary storage for raw crashes, minimized files, and
  log output.
- `fuzz/regressions/<target>/`: Committed. Tiny, minimized crash files that serve as source of
  truth for `cargo test`.
  