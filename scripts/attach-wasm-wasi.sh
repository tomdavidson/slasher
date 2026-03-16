#!/usr/bin/env bash
set -euo pipefail

# Attach wasm-wasi + WIT artifacts to the current GitHub Release
# Expects GITHUB_TOKEN and GITHUB_REF_NAME to be set

wasm_dir="wasm-wasi/target/wasm32-wasip2/release"
wit_dir="wasm-wasi/wit"

# Upload WASM component if built
if [ -d "$wasm_dir" ]; then
  for file in "$wasm_dir"/*.wasm; do
    [ -f "$file" ] || continue
    echo "Uploading $(basename "$file")"
    gh release upload "$GITHUB_REF_NAME" "$file" --repo "$GITHUB_REPOSITORY" --clobber
  done
else
  echo "WARN: WASM build output directory $wasm_dir not found"
fi

# Upload WIT definitions
if [ -d "$wit_dir" ]; then
  for file in "$wit_dir"/*.wit; do
    [ -f "$file" ] || continue
    echo "Uploading $(basename "$file")"
    gh release upload "$GITHUB_REF_NAME" "$file" --repo "$GITHUB_REPOSITORY" --clobber
  done

  # Also create a bundled wit archive
  tar -czf wit-definitions.tar.gz -C wasm-wasi wit/
  gh release upload "$GITHUB_REF_NAME" "wit-definitions.tar.gz" --repo "$GITHUB_REPOSITORY" --clobber
  rm -f wit-definitions.tar.gz
else
  echo "WARN: WIT directory $wit_dir not found"
fi
