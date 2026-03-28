#!/usr/bin/env bash
# ./fuzz-manage.sh
#
# Manages fuzzing artifacts, regressions, and corpus compaction.
#
# Usage:
#   bash ./fuzz-manage.sh <command> <target>
#
# Commands:
#   triage  - Minimize crash artifacts (tmin) and save them to regressions/
#   compact - Minimize the fuzzing corpus (cmin) by removing redundant inputs

set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

cd "$(git rev-parse --show-toplevel)/engine/fuzz" || exit 1

COMMAND="${1:-}"
TARGET="${2:-}"

if [[ -z "$COMMAND" || -z "$TARGET" ]]; then
    echo "Usage: $0 <command> <target>"
    echo "Commands: triage, compact"
    exit 1
fi

if [[ "$COMMAND" == "triage" ]]; then
    ARTIFACT_DIR="artifacts/${TARGET}"
    REGRESSION_DIR="regressions/${TARGET}"

    if [[ ! -d "$ARTIFACT_DIR" ]]; then
        echo "No artifacts found for ${TARGET}."
        exit 0
    fi

    mkdir -p "$REGRESSION_DIR"
    TRIAGE_COUNT=0

    for crash in "$ARTIFACT_DIR"/crash-*; do
        [[ -e "$crash" ]] || continue

        HASH=$(basename "$crash" | head -c 20)
        REGRESSION_FILE="${REGRESSION_DIR}/regression-${HASH}"

        if [[ -f "$REGRESSION_FILE" ]]; then
            echo "  Skipping (already triaged): $(basename "$crash")"
            continue
        fi

        TRIAGE_COUNT=$((TRIAGE_COUNT + 1))
        echo "  Triaging: $(basename "$crash")"

        rm -f "$ARTIFACT_DIR"/minimized-from-*

        cargo fuzz tmin "$TARGET" "$crash" || true

        MINIMIZED_TMP=$(ls -t "$ARTIFACT_DIR"/minimized-from-* 2>/dev/null | head -1 || true)

        if [[ -n "$MINIMIZED_TMP" && -f "$MINIMIZED_TMP" ]]; then
            cp "$MINIMIZED_TMP" "$REGRESSION_FILE"
            rm "$MINIMIZED_TMP"
        else
            echo "  (Could not minimize further, using original crash file)"
            cp "$crash" "$REGRESSION_FILE"
        fi

        echo "  Minimized: $(wc -c < "$REGRESSION_FILE") bytes -> $REGRESSION_FILE"
    done

    echo ""
    echo "Triaged ${TRIAGE_COUNT} new crash(es) for ${TARGET}."

elif [[ "$COMMAND" == "compact" ]]; then
    echo "  Compacting corpus for: ${TARGET}"
    cargo fuzz cmin "$TARGET"
    echo "  Compaction complete for ${TARGET}."

else
    echo "Error: Unknown command '$COMMAND'"
    echo "Commands: triage, compact"
    exit 1
fi
