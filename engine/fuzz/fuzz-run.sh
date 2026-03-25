#!/usr/bin/env bash
# ./fuzz-run
#
# Runs a cargo-fuzz target with a time budget and reports results.
# Exits 0 on clean run, 1 if new crashes were found.
#
# Usage:
#   bash ./fuzz-run <target> [max_time_secs]
#
# Arguments:
#   target          Fuzz target name matching a [[bin]] in fuzz/Cargo.toml
#   max_time_secs   Maximum run duration in seconds (default: 300)
#
# Examples:
#   bash ./fuzz-run parse_document
#   bash ./fuzz-run parse_document 600
#   bash ./fuzz-run parse_document_structured 3600
#
# LibAFL:
#   To run with LibAFL instead of libFuzzer, set FUZZ_FEATURES:
#   FUZZ_FEATURES="--no-default-features --features libafl" bash ./fuzz-run parse_document
#
# Output:
#   Prints a summary with corpus size, crash count, and next steps.
#   New crash artifacts are saved to fuzz/artifacts/<target>/.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)/engine" || exit 1

TARGET="${1:?Usage: fuzz-run <target> [max_time]}"
MAX_TIME="${2:-300}"
ARTIFACT_DIR="fuzz/artifacts/${TARGET}"

mkdir -p "$ARTIFACT_DIR"

TIMESTAMP=$(mktemp)

PRE_COUNT=$(find "$ARTIFACT_DIR" -name 'crash-*' ! -name '*-minimized' 2>/dev/null | wc -l)

set +e
rustup run nightly cargo fuzz run "$TARGET" -- \
    -max_total_time="$MAX_TIME" \
    -print_final_stats=1
FUZZ_EXIT=$?
set -e

POST_COUNT=$(find "$ARTIFACT_DIR" -name 'crash-*' ! -name '*-minimized' 2>/dev/null | wc -l)
NEW_CRASHES=$((POST_COUNT - PRE_COUNT))


# Build/config errors: fuzzer failed but no crashes on disk
if [[ "$FUZZ_EXIT" -ne 0 ]] && [[ "$NEW_CRASHES" -eq 0 ]]; then
    echo "  Fuzzer failed (exit $FUZZ_EXIT) but no crash artifacts found."
    echo "  This is likely a build or configuration error."
    exit "$FUZZ_EXIT"
fi


CORPUS_COUNT=$(find "fuzz/corpus/${TARGET}" -type f 2>/dev/null | wc -l)

echo ""
echo "════════════════════════════════════"
echo "  Fuzz Report: ${TARGET}"
echo "════════════════════════════════════"
echo "  Duration:      ${MAX_TIME}s"
echo "  Corpus size:   ${CORPUS_COUNT} inputs"
echo "  New crashes:   ${NEW_CRASHES}"
echo "  Total crashes: ${POST_COUNT}"
echo "  Exit code:     ${FUZZ_EXIT}"
echo ""

if [[ "$NEW_CRASHES" -gt 0 ]]; then
    echo "  New crash artifacts:"
    find "$ARTIFACT_DIR" -name 'crash-*' ! -name '*-minimized' -newer "$TIMESTAMP" \
        -exec basename {} \; | while read -r f; do echo "    - $f"; done
    echo ""
    echo "  Reproduce with:"
    echo "    rustup run nightly rustup run nightly cargo fuzz run ${TARGET} ${ARTIFACT_DIR}/<crash-file>"
    echo ""
    echo "  Minimize with:"
    echo "    rustup run nightly cargo fuzz tmin ${TARGET} ${ARTIFACT_DIR}/<crash-file>"
    rm -f "$TIMESTAMP"
    exit 1
fi

rm -f "$TIMESTAMP"
exit 0
