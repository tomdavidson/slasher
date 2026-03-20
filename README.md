# slash-parser

A Rust + WebAssembly slash command parser. Extracts `/commands` from text with support for
single-line arguments, multi-line continuation (`/`), and fenced code blocks.

## Quick start

Managed by [Moon](https://moonrepo.dev). All tool versions are pinned via `.prototools`.

```bash
# Run all checks on affected projects
moon run :lint :fmt-check :test

# Build CLI
moon run slash-parse:build

# Build WASM for JS
moon run slash-parser-js:wasm-build
```

Or with cargo directly:

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

## CLI usage

```bash
# Single file
slash-parse ./prompt.md

# Multiple files (streaming JSONL)
slash-parse ./a.md ./b.md

# Stdin
echo "/help me" | slash-parse

# With context injection
slash-parse -c user=tom -c ./config.toml ./prompt.md

# Pretty output (JSON array)
slash-parse -p -c '{"user":"tom"}' ./prompt.md
```

### Context (-c flag)

The `-c` flag is repeatable. Each value is detected as:

1. **File path** — `.json`, `.toml`, `.env`, or plain key=value files
2. **Inline JSON** — starts with `{`
3. **Inline key=value** — contains `=`

Values merge left to right. File source always overrides `-c source=...`.

### Exit codes

| Code | Meaning                     |
| ---- | --------------------------- |
| 0    | Success                     |
| 1    | File/context error          |
| 2    | Usage error (bad arguments) |

## Input format

````text
/command single-line argument

/command multi-line /
continuation here

/command fenced block ```json
{"key": "value"}
````

````
## Output

JSON conforming to the `SlashParseResult` schema:

```json
{
  "version": "0.1.0",
  "context": { "source": "file.md", "user": "tom" },
  "commands": [
    {
      "id": "cmd-0",
      "name": "command",
      "raw": "/command argument",
      "range": { "start_line": 0, "end_line": 0 },
      "arguments": {
        "header": "argument",
        "mode": "single-line",
        "payload": "argument"
      },
      "children": []
    }
  ]
}
````

## JS/TS usage

```typescript
import init, { parseText, parseTextWithContext, version } from '@slash-parser/js'

await init()

const result = parseTextWithContext(sourceText, { source: 'file://path/to/doc.md', user: 'tom' })

console.log(result.commands)
```

TypeScript types are provided in `types/index.d.ts`.

## Testing

- **Unit tests**: 63 tests covering all argument modes, edge cases, CRLF normalization
- **Integration tests**: 17 CLI tests covering file/stdin/mixed input, context injection, pretty
  output, error codes
- **Property tests**: proptest strategies for no-panic guarantees and invariant verification
- **Fuzz harness**: `cargo fuzz run fuzz_parse` for arbitrary input crash testing

## License

MIT
