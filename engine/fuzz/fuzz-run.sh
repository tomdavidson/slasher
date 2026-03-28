#!/usr/bin/env bash
# ./fuzz-run.sh
#
# Runs a cargo-fuzz target with a time budget and reports results.
# Exits 0 on clean run, 1 if new crashes were found.
#
# Usage:
#   bash ./fuzz-run.sh <target> [max_time_secs] [jobs] [--replay]
#
# Arguments:
#   target          Fuzz target name matching a [[bin]] in fuzz/Cargo.toml
#   max_time_secs   Maximum run duration in seconds (default: 300)
#   jobs            Number of parallel fuzzing jobs (default: half of CPU cores)
#   --replay        Replay existing corpus only (no new fuzzing). Used for CI.
#
# Examples:
#   bash ./fuzz-run.sh parse_document_unstructured
#   bash ./fuzz-run.sh parse_document_unstructured 600
#   bash ./fuzz-run.sh parse_document_unstructured 3600 4
#   bash ./fuzz-run.sh parse_document_unstructured 10 10 --replay
#
# LibAFL:
#   To run with LibAFL instead of libFuzzer, set FUZZ_FEATURES:
#   FUZZ_FEATURES="--no-default-features --features libafl" bash ./fuzz-run.sh parse_document_unstructured

set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

cd "$(git rev-parse --show-toplevel)/engine/fuzz" || exit 1

TARGET="${1:?Usage: fuzz-run.sh <target> [max_time] [jobs] [--replay]}"
MAXTIME="${FUZZ_MAXTIME:-${2:-300}}"


CORES=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)
DEFAULT_JOBS=$(( CORES / 2 ))
[[ "$DEFAULT_JOBS" -lt 1 ]] && DEFAULT_JOBS=1
JOBS="${FUZZ_JOBS:-${3:-$DEFAULT_JOBS}}"

REPLAY=false
for arg in "${@:4}"; do
    if [[ "$arg" == "--replay" ]]; then
        REPLAY=true
    fi
done

ARTIFACT_DIR="artifacts/${TARGET}"
CORPUS_DIR="corpus/${TARGET}"
REGRESSION_DIR="regressions/${TARGET}"
LOG_DIR="${ARTIFACT_DIR}/logs"


mkdir -p "$ARTIFACT_DIR" "$CORPUS_DIR" "$LOG_DIR"

CORPUS_ARGS=("$CORPUS_DIR")
if [[ -d "$REGRESSION_DIR" ]] && [[ -n "$(ls -A "$REGRESSION_DIR" 2>/dev/null)" ]]; then
    CORPUS_ARGS+=("$REGRESSION_DIR")
fi

PRE_COUNT=$(find "$ARTIFACT_DIR" -name 'crash-*' 2>/dev/null | wc -l)

if [[ -n "${FUZZ_FEATURES:-}" ]]; then
    ENGINE_NAME="libafl"
    # shellcheck disable=SC2206
    FEATURE_ARGS=($FUZZ_FEATURES)
else
    ENGINE_NAME="libfuzzer"
    FEATURE_ARGS=()
fi

MODE="saturate"
if [[ "$REPLAY" == true ]]; then
    MODE="replay"
fi

echo ""
echo "════════════════════════════════════"
echo "  Fuzz: ${TARGET} (${ENGINE_NAME})"
echo "════════════════════════════════════"
echo "  Mode:         ${MODE}"
echo "  Time budget:  ${MAXTIME}s"
echo "  Jobs:         ${JOBS}"
echo "  Corpus:       ${CORPUS_ARGS[*]}"
echo ""

set +e
if [[ "$ENGINE_NAME" == "libfuzzer" ]]; then
    FUZZER_ARGS=(
        -max_total_time="$MAXTIME"
        -print_final_stats=1
        -jobs="$JOBS"
    )
    if [[ "$REPLAY" == true ]]; then
        FUZZER_ARGS+=(-runs=0)
    fi
    cargo fuzz run "$TARGET" \
        "${CORPUS_ARGS[@]}" \
        -- \
        "${FUZZER_ARGS[@]}"
else
    # LibAFL's runtime does not support libfuzzer's -max_total_time, -jobs,
    # -print_final_stats, or -runs flags.
    #
    # Replay mode: passing corpus dirs causes LibAFL to run each input once.
    # Saturate mode: LibAFL fuzzes until killed. Moon's task timeout or an
    # external `timeout` command handles the time budget.
    cargo fuzz run "$TARGET" \
        "${FEATURE_ARGS[@]}" \
        "${CORPUS_ARGS[@]}"
fi
FUZZ_EXIT=$?
set -e

shopt -s nullglob
for logfile in fuzz-*.log; do
    mv "$logfile" "$LOG_DIR/"
done
shopt -u nullglob

POST_COUNT=$(find "$ARTIFACT_DIR" -name 'crash-*' 2>/dev/null | wc -l)
NEW_CRASHES=$((POST_COUNT - PRE_COUNT))
CORPUS_COUNT=$(find "$CORPUS_DIR" -type f 2>/dev/null | wc -l)

if [[ "$FUZZ_EXIT" -ne 0 ]] && [[ "$NEW_CRASHES" -eq 0 ]]; then
    echo ""
    echo "════════════════════════════════════"
    echo "  Fuzz FAILED: ${TARGET} (${ENGINE_NAME})"
    echo "════════════════════════════════════"
    echo "  Fuzzer exited with code ${FUZZ_EXIT} but no crash artifacts found."
    echo "  This is likely a build or configuration error."
    echo "  Check ${ARTIFACT_DIR}/fuzz-*.log files for worker output."
    exit "$FUZZ_EXIT"
fi

echo ""
echo "════════════════════════════════════"
if [[ "$NEW_CRASHES" -gt 0 ]]; then
    echo "  Fuzz CRASHED: ${TARGET} (${ENGINE_NAME})"
else
    echo "  Fuzz PASSED: ${TARGET} (${ENGINE_NAME})"
fi
echo "════════════════════════════════════"
echo "  Mode:          ${MODE}"
echo "  Duration:      ${MAXTIME}s"
echo "  Jobs:          ${JOBS}"
echo "  Engine:        ${ENGINE_NAME}"
echo "  Corpus size:   ${CORPUS_COUNT} inputs"
echo "  New crashes:   ${NEW_CRASHES}"
echo "  Total crashes: ${POST_COUNT}"
echo "  Exit code:     ${FUZZ_EXIT}"

if [[ "$NEW_CRASHES" -gt 0 ]]; then
    echo ""
    echo "  New crash artifacts:"
    find "$ARTIFACT_DIR" -name 'crash-*' -newer "$CORPUS_DIR" 2>/dev/null \
        | sort \
        | while read -r f; do echo "    - $(basename "$f")"; done
    echo ""
    echo "  Triage with:"
    echo "    moon run engine-fuzz:fuzz-triage"
    echo ""
    echo "  Reproduce individually:"
    echo "    cargo fuzz run $TARGET artifacts/$TARGET/<crash-file>"
    exit 1
fi

exit 0
