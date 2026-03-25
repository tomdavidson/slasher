#!/usr/bin/env bash
# ./fuzz-triage.sh
#
# Minimizes crash artifacts and generates regression tests.
# Run after fuzz-until-saturated.sh finds crashes.
#
# Usage:
#   bash ./fuzz-triage.sh <target>
#
# Arguments:
#   target   Fuzz target name matching a [[bin]] in fuzz/Cargo.toml
#
# Examples:
#   bash ./fuzz-triage.sh parse_document
#   bash ./fuzz-triage.sh parse_document_structured
#
# What it does:
#   1. Minimizes each crash artifact via cargo fuzz tmin
#   2. Copies minimized inputs into fuzz/corpus/<target>/ for replay
#   3. Generates tests/fuzz_regressions/<target>.rs with a #[test]
#      per crash that runs under cargo nextest run
#
# After running:
#   1. Review the generated test file
#   2. Run: cargo nextest run
#   3. Fix the bugs
#   4. Commit regression tests AND minimized inputs
#   5. Do NOT delete crash artifacts until fixes are merged

set -euo pipefail

cd "$(git rev-parse --show-toplevel)/engine" || exit 1

TARGET="${1:?Usage: fuzz-triage.sh <target>}"
ARTIFACT_DIR="fuzz/artifacts/${TARGET}"
REGRESSION_DIR="tests/fuzz_regressions"

if [[ ! -d "$ARTIFACT_DIR" ]] || \
   [[ -z "$(find "$ARTIFACT_DIR" -name 'crash-*' ! -name '*-minimized' 2>/dev/null)" ]]; then
    echo "No crashes to triage for ${TARGET}."
    exit 0
fi

mkdir -p "$REGRESSION_DIR"
mkdir -p "fuzz/corpus/${TARGET}"

COUNT=0

for crash in "$ARTIFACT_DIR"/crash-*; do
    [[ "$crash" == *-minimized ]] && continue
    COUNT=$((COUNT + 1))
    HASH=$(basename "$crash" | head -c 20)
    MINIMIZED="${crash}-minimized"

    echo "── Triaging: $(basename "$crash")"

    if [[ ! -f "$MINIMIZED" ]]; then
        rustup run nightly cargo fuzz tmin "$TARGET" "$crash" -o "$MINIMIZED"
    fi

    cp "$MINIMIZED" "fuzz/corpus/${TARGET}/regression-${HASH}"

    echo "   Minimized: $(wc -c < "$MINIMIZED") bytes"
done

REGFILE="${REGRESSION_DIR}/${TARGET}.rs"

cat > "$REGFILE" <<'HEADER'
//! Auto-generated fuzz regression tests.
//! Each test replays a minimized crash input through parse_document.
//! Do not delete these tests. See testing-rust.md § Fuzz Testing.

use solidus_engine::parse_document;

HEADER

INDEX=0
for crash in "$ARTIFACT_DIR"/crash-*; do
    [[ "$crash" == *-minimized ]] && continue
    MINIMIZED="${crash}-minimized"
    [[ -f "$MINIMIZED" ]] || continue

    HASH=$(basename "$crash" | head -c 20)

    cat >> "$REGFILE" <<EOF
#[test]
fn regression_${TARGET}_${INDEX}_${HASH}() {
    let input = include_str!("../../fuzz/artifacts/${TARGET}/$(basename "$MINIMIZED")");
    let result = parse_document(input);
    // Must not panic. Verify result is structurally valid.
    for (i, cmd) in result.commands.iter().enumerate() {
        assert_eq!(cmd.id, format!("cmd-{i}"));
    }
    for (i, tb) in result.textblocks.iter().enumerate() {
        assert_eq!(tb.id, format!("text-{i}"));
    }
}

EOF
    INDEX=$((INDEX + 1))
done

echo ""
echo "════════════════════════════════════"
echo "  Triage Complete"
echo "════════════════════════════════════"
echo "  Crashes processed: ${COUNT}"
echo "  Regression file:   ${REGFILE}"
echo "  Corpus updated:    fuzz/corpus/${TARGET}/"
echo ""
echo "  Next steps:"
echo "    1. Review ${REGFILE}"
echo "    2. Run: cargo nextest run"
echo "    3. Fix the bugs"
echo "    4. Commit the regression tests AND minimized inputs"
echo "    5. Do NOT delete crash artifacts until fixes are merged"
