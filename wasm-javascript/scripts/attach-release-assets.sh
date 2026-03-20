#!/usr/bin/env bash
set -euo pipefail

# Attach wasm-javascript build artifacts to the current GitHub Release
# Run from repo root. Expects GITHUB_TOKEN and GITHUB_REF_NAME to be set.

artifact_dir="wasm-javascript/pkg"

if [ ! -d "$artifact_dir" ]; then
  echo "ERROR: Build output directory $artifact_dir not found"
  exit 1
fi

for file in "$artifact_dir"/*; do
  [ -f "$file" ] || continue
  name="$(basename "$file")"
  echo "Uploading $name"
  gh release upload "$GITHUB_REF_NAME" "$file" --repo "$GITHUB_REPOSITORY" --clobber
done
