<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

## `slash-parse` CLI Specification

### 1. Overview

`slash-parse` is a command-line tool that parses text containing slash commands and emits structured JSON. It is built in Rust, with the core parsing logic in `src/lib.rs` (shared with the WebAssembly target) and the CLI wrapper in `src/main.rs`.

The CLI follows UNIX conventions: it reads from files or stdin, writes JSON to stdout, and reports errors to stderr.

### 2. Installation and Build

```bash
cargo build --release
```

The binary is `target/release/slash-parse`.

### 3. Usage

```text
Usage: slash-parse [OPTIONS] [FILES]...

Arguments:
  [FILES]...  One or more input files to parse. If omitted or '-', reads from stdin.

Options:
  -c, --context <VALUE>  Context to merge into the output (repeatable). See Section 6.
  -p, --pretty           Pretty-print JSON output. Changes JSONL to a JSON Array.
  -h, --help             Print help
  -V, --version          Print version
```


### 4. Input Handling

#### 4.1 File Inputs

The CLI accepts zero or more positional file arguments.

```bash
slash-parse ./prompt.md
slash-parse ./prompt1.md ./prompt2.md ./prompts/draft.md
```

Each file is parsed independently. The parser automatically sets `context.source` to the file path for each file.

#### 4.2 Stdin

If no file arguments are provided, or if a file argument is literally `"-"`, the CLI reads from stdin.

```bash
cat prompts/*.md | slash-parse
echo "/help" | slash-parse
slash-parse - < prompt.md
```

When reading from stdin, `context.source` is set to `"stdin"`.

#### 4.3 Mixed

File arguments and stdin can be mixed. A `-` in the file list represents stdin at that position.

```bash
slash-parse ./header.md - ./footer.md < body.md
```

This parses `header.md`, then stdin, then `footer.md`, in order.

### 5. Output

#### 5.1 Default: JSONL (one result per line)

By default, each parsed input produces one compact JSON object on its own line:

```bash
slash-parse ./a.md ./b.md
```

```jsonl
{"version":"0.1.0","context":{"source":"./a.md"},"commands":[...],"text_blocks":[...]}
{"version":"0.1.0","context":{"source":"./b.md"},"commands":[...],"text_blocks":[...]}
```

This is ideal for piping into `jq`, `grep`, or other line-oriented tools.

#### 5.2 Pretty: JSON Array

With `-p` or `--pretty`, the output is a single formatted JSON array containing all results:

```bash
slash-parse -p ./a.md ./b.md
```

```json
[
  {
    "version": "0.1.0",
    "context": {
      "source": "./a.md"
    },
    "commands": [...],
    "text_blocks": [...]
  },
  {
    "version": "0.1.0",
    "context": {
      "source": "./b.md"
    },
    "commands": [...],
    "text_blocks": [...]
  }
]
```

If there is only one input, `--pretty` still wraps it in an array for consistency, or implementations may choose to emit a single object. Document the choice.

### 6. Context Injection (`-c`, `--context`)

The `-c` flag is repeatable. Each occurrence provides additional context that is merged into the output's `context` object.

```bash
slash-parse -c '{"user":"tom"}' -c env=prod ./prompt.md
```


#### 6.1 Merge Order

1. Start with an empty JSON object `{}`.
2. Process each `-c` value in the order provided, left to right.
3. Later values overwrite earlier values for the same key (shallow merge).
4. For file inputs, `context.source` is set to the filename after all `-c` merges. This means the filename always wins over a `source` key provided via `-c`.

#### 6.2 Detection Logic

For each `-c <VALUE>`, the CLI determines the format using this precedence:

1. **File path:** If `<VALUE>` exists as a file on disk, read the file and parse based on extension:


| Extension | Parser |
| :-- | :-- |
| `.json` | JSON (`serde_json`) |
| `.toml` | TOML (`toml` crate) |
| `.env` | Line-by-line `key=value` (see 6.3) |
| Other/none | Line-by-line `key=value` |

2. **Inline JSON:** If `<VALUE>` starts with `{`, parse as a JSON object.
3. **Inline key=value:** If `<VALUE>` contains `=`, split on the first `=`. Left side is the key, right side is the value (string).

#### 6.3 Key=Value Format (Files and Inline)

For `.env` files or inline `key=value`:

- One pair per line (for files). Inline is a single pair.
- Lines starting with `#` are comments (files only).
- Empty lines are ignored (files only).
- Keys are trimmed of whitespace.
- Values are trimmed of whitespace. Surrounding quotes (`"` or `'`) are stripped if present.

Examples:

```bash
# Inline
slash-parse -c user=tom -c env=prod ./prompt.md

# From a .env file
slash-parse -c ./config.env ./prompt.md
```

Where `config.env` contains:

```env
# Project context
user=tom
env=prod
pipeline_id=42
```


#### 6.4 Mapping to ParserContext

The merged JSON object is mapped to the `ParserContext` struct:

- Keys matching known fields (`source`, `timestamp`, `user`, `session_id`) populate those fields directly.
- All other keys are placed into `context.extra`.

Example:

```bash
slash-parse -c '{"user":"tom","pipeline_id":"42"}' ./prompt.md
```

Produces:

```json
{
  "version": "0.1.0",
  "context": {
    "source": "./prompt.md",
    "user": "tom",
    "extra": {
      "pipeline_id": "42"
    }
  },
  "commands": [...]
}
```


### 7. Error Handling

- If a file does not exist or is unreadable, print an error to stderr and exit with code `1`.
- If a `-c` value cannot be parsed (invalid JSON, missing file, malformed TOML), print an error to stderr and exit with code `1`.
- If the input text contains an unclosed fence (EOF before closing fence), the parser should finalize the command with whatever payload has been accumulated and include a `"warnings"` array in the output (non-fatal).
- If stdin is empty and no files are provided, emit an empty result: `{"version":"0.1.0","context":{"source":"stdin"},"commands":[],"text_blocks":[]}`.


### 8. Exit Codes

| Code | Meaning |
| :-- | :-- |
| `0` | Success. All inputs parsed. |
| `1` | Error. File not found, unreadable, or invalid context value. |
| `2` | Usage error. Invalid arguments (handled by `clap`). |

### 9. Dependencies

| Crate | Purpose |
| :-- | :-- |
| `clap` (v4, `derive` feature) | CLI argument parsing |
| `serde` + `serde_json` | JSON serialization/deserialization |
| `toml` | TOML context file support |

The core parser in `src/lib.rs` depends only on `serde` and `serde_json`. The `clap` and `toml` crates are CLI-only dependencies.

### 10. Project Structure

```
slash-parse/
  Cargo.toml
  src/
    lib.rs          # Core parser + data models + WASM bindings
    main.rs         # CLI wrapper (clap, file I/O, context merging)
```

`lib.rs` is compiled as both:

- A `cdylib` target (for `wasm-pack` / WebAssembly).
- A `lib` target consumed by `main.rs`.

`Cargo.toml` should define:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```


### 11. Example Session

```bash
# Single file, default output
slash-parse ./prompt.md

# Multiple files with context
slash-parse -c user=tom -c ./project.toml ./prompts/a.md ./prompts/b.md

# Stdin pipeline
cat ./prompts/*.md | slash-parse -c env=ci

# Pretty output with inline JSON context
slash-parse -p -c '{"user":"tom","run_id":"abc-123"}' ./prompt.md

# Context from multiple sources merged together
slash-parse -c ./defaults.json -c ./overrides.env -c debug=true ./prompt.md
```

