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
│          fuzz/regressions/<target>/              fuzz/corpus/<target>/│
└──────────────────────┤───────────────────────────────────┤─────────────┘
                       │ (Minimized crashes)               │ (Compacted)
                       ▼                                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ 3. Persistence                                                          │
│                                                                          │
│   [ regressions/ ] ──► Committed to main (datatest-stable auto-discovers)│
│   [ corpus/ ] ───────► Synced to orphan `corpus` branch + CI cache       │
└──────────────────────────────────────────────────────────────────────────┘
```


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


## Corpus Management

Corpus inputs are stored with two-tier persistence to avoid repository bloat while maintaining
shared progress across CI runs and local sessions.

### CI: Actions Cache + Orphan Branch

The `fuzz-cache` composite action manages both tiers via `mode: pull` and `mode: push` inputs:

- `actions/cache@v4` provides fast, per-run restoration. Cache keys are isolated by main vs PR
  (`fuzz-corpus-main-*` vs `fuzz-corpus-pr-<number>-*`). PR caches fall back to main if no
  PR-specific cache exists.
- An orphan `corpus` branch provides durable, shared storage. On `pull`, the action fetches the
  branch and seeds the corpus directory via `rsync`. On `push`, it syncs the updated corpus back
  with retry logic. Branch operations are main-only (silently skipped for PRs). The branch is
  self-initializing: created automatically on first push if it doesn't exist.

Workflow sequence (main-fuzz.yml):
1. `fuzz-cache mode: pull` restores from CI cache, then seeds from `corpus` branch
2. `fuzz-saturate` runs all four engine/target variants
3. `fuzz-cache mode: push` syncs compacted corpus back to `corpus` branch
4. `fuzz-triage` minimizes crashes into `regressions/` (only if crashes found)

### Local: Git Worktree

For local sessions, a git worktree at `//.corpus/` provides access to the orphan branch
without switching branches:

```bash
# One-time setup
git worktree add .corpus corpus

# Typical local fuzzing session
moon run engine-fuzz:fuzz-corpus-pull    # seed from CI's latest corpus
moon run engine-fuzz:fuzz-saturate       # fuzz locally
moon run engine-fuzz:fuzz-corpus-push    # triage, compact, push back
```

The `fuzz-corpus-push` task triages crashes, compacts the corpus, pulls any new CI discoveries
made during the session, and pushes the merged result to the `corpus` branch.


## Moon Task Graph

Moon manages the parallelization of the full 2x2 engine/target matrix.

` ``text
fuzz
├── fuzz-saturate (parallel)
│   ├── fuzz-structured-saturate
│   │   ├── libfuzz-structured-saturate    (fuzz-run.sh ... 14400 2)
│   │   ├── libafl-structured-saturate     (fuzz-run.sh ... 14400 2 + FUZZ_FEATURES)
│   │   └── compact (fuzz-manage.sh compact)
│   └── fuzz-unstructured-saturate
│       ├── libfuzz-unstructured-saturate
│       ├── libafl-unstructured-saturate
│       └── compact
└── fuzz-triage (sequential, after saturate)
    ├── triage parse_document_unstructured
    └── triage parse_document_structured

fuzz-triage (sequential, after gate signals crashes)
├── triage parse_document_unstructured
└── triage parse_document_structured

fuzz (quick replay, libFuzzer only, -runs=0)
├── libfuzz-structured     (fuzz-run.sh ... --replay)
└── libfuzz-unstructured   (fuzz-run.sh ... --replay)

fuzz-corpus-pull / fuzz-corpus-push (local worktree sync)
```

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

# Corpus sync (local, requires git worktree at .corpus/)
moon run engine-fuzz:fuzz-corpus-pull
moon run engine-fuzz:fuzz-corpus-push
```


## CI Automation

Two GitHub Actions workflows automate fuzzing in CI.

### Trunk Saturation (`main-fuzz.yml`)

Runs weekly (Saturday 2 AM UTC) and on manual dispatch. Discovers all projects with a `fuzz-ci`
task via `moon-q-projects`, then runs `fuzz-saturate` across them in a matrix. Corpus is pulled
from cache and the orphan branch before fuzzing, and pushed back after. If crashes are found, it
triages them and opens a PR with the minimized regression files via the `pr-create` composite
action.

### PR Saturation (`pr-fuzz.yml`)

Triggered by adding the `fuzz` label to a PR, or via `workflow_dispatch` with a PR number. Runs
the same discover/saturate/triage cycle against the PR branch. Corpus is seeded from cache only
(falls back to main cache, no branch interaction). Regression files are committed directly to
the PR branch. Results are reported as a commit status and sticky PR comment via the
`pr-set-status` composite action.

Both workflows use these shared composite actions:

- `setup`: Checkout, toolchain installation, Cargo/Moon/pnpm caches
- `moon-q-projects`: Discover fuzz-capable projects with metadata
- `moon-run`: Execute moon tasks with retrospect reporting
- `fuzz-cache`: Two-tier corpus persistence (CI cache + orphan branch)
- `git-stage-artifacts`: Download and place matrix artifacts into project directories
- `git-commit-push`: Commit and push with last-author attribution
- `pr-create`: Idempotent PR creation (trunk workflow)
- `pr-set-status`: Commit status + sticky PR comment (PR workflow)
- `resolve-pr`: Normalize PR metadata across event types


## State Management

| Directory | Tracked | Persistence |
|---|---|---|
| `fuzz/corpus/<target>/` | Gitignored | CI cache (`actions/cache`) + orphan `corpus` branch. Main and PR caches isolated; branch ops main-only. |
| `fuzz/artifacts/<target>/` | Gitignored | Ephemeral. Raw crashes, minimized files, crash summaries, logs. |
| `fuzz/regressions/<target>/` | Committed | Minimized crash files. Source of truth for `cargo test` via `datatest-stable`. |
| `.corpus/` (repo root) | Gitignored | Local git worktree pointing at the orphan `corpus` branch. |
