<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

## `riff` CLI Specification

### 1. Overview

`riff` is a command-line tool that parses text containing slash commands and emits structured JSON.
It is built in Rust, with the core parsing logic in `src/lib.rs` (shared with the WebAssembly
target) and the CLI wrapper in `src/main.rs`.

The CLI follows UNIX conventions: it reads from files or stdin, writes JSON/JSONL to stdout, and
reports errors to stderr.

### 2. Installation and Build

```bash
```

### 3. Usage

```text
Usage: riff [OPTIONS] [FILES]...

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

The CLI accepts zero or one positional file arguments supporting globs/wildcard matching.

```bash
riff ./prompt.md
riff prompt-*.{txt,md} > output.json
```

There are cli crates that can be used for the wilidcared matchign and loading multiple files.

#### 4.2 Stdin

If no file arguments are provided, or if a file argument is literally `"-"`, the CLI reads from
stdin.

```bash
cat prompts/*.md | riff 
echo "/help" | riff
riff - < prompt.md
```

### 5. Output

#### 5.1 Default: JSONL (one result per line)

By default, each parsed command produces one compact JSON object on its own line:

```bash
riff mesg.txt > output.jsonl
```

#### 5.2 Pretty: JSON Array

With `-p` or `--pretty`, the output is a single formatted JSON array containing all results:

```bash
cat *.md | riff -p 
riff mesg-*.txt -p
```

```json
[
  {
    "version": "0.1.0",
    "context": {},
    "commands": [...],
    "text_blocks": [...]
  },
  {
    "version": "0.1.0",
    "context": {},
    "commands": [...],
    "text_blocks": [...]
  }
]
```

If there is only one input, `--pretty` still wraps it in an array for consistency, or
implementations may choose to emit a single object. Document the choice.

### 6. Context Injection (`-c`, `--context`)

The `-c` flag is repeatable. Each occurrence provides additional context that is merged into the
output's `context` object. The value maybe a string or a file path.

```bash
riff -c env=prod -c ../even-more-context-*.json .-c /prompt.md
```

#### 6.1 Merge Order

1. Start with an empty JSON object `{}`.
2. Process each `-c` value in the order provided, left to right.
3. Later values overwrite earlier values for the same key (shallow merge).

#### 6.2 Detection Logic

For each `-c <VALUE>`, the CLI determines the format using this precedence:

1. **File path:** If `<VALUE>` exists as a file on disk, read the file and parse based on extension:

- `.json`
- `.toml`
- `.yaml/yml`
- `.env/txt`

There is bound to be a good libray choice to support this functionality accross OS platforms and
formats.

2. **Inline JSON:** If `<VALUE>` starts with `{`, parse as a JSON object.
3. **Inline key=value:** If `<VALUE>` contains `=`, split on the first `=`. Left side is the key,
   right side is the value (string).

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
riff -c user=tom -c env=prod ./prompt.md

# From a .env file
riff -c ./config.env ./prompt.md
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

- Keys matching known fields (`source`, `timestamp`, `user`, `session_id`) populate those fields
  directly.
- All other keys are placed into `context.extra`.

Example:

```bash
riff -c '{"user":"tom","pipeline_id":"42"}' ./prompt.md
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
- If a `-c` value cannot be parsed (invalid JSON, missing file, malformed TOML), print an error to
  stderr and exit with code `1`.
- If the input text contains an unclosed fence (EOF before closing fence), the parser should
  finalize the command with whatever payload has been accumulated and include a `"warnings"` array
  in the output (non-fatal).
- If stdin is empty and no files are provided, emit an empty result:
  `{"version":"0.1.0","context":{"source":"stdin"},"commands":[],"text_blocks":[]}`.

### 8. Exit Codes

| Code | Meaning                                                      |
| :--- | :----------------------------------------------------------- |
| `0`  | Success. All inputs parsed.                                  |
| `1`  | Error. File not found, unreadable, or invalid context value. |
| `2`  | Usage error. Invalid arguments (handled by `clap`).          |

### 9. Dependencies

| Crate | Purpose |
| :---- | :------ |

### 10. Project Structure

### 11. Example Session

```bash
# Single file, default output
riff ./prompt.md

# Multiple files with context
riff -c user=tom -c ./project.toml ./prompts/a.md ./prompts/b.md

# Stdin pipeline
cat ./prompts/*.md | riff -c env=ci

# Pretty output with inline JSON context
riff -p -c '{"user":"tom","run_id":"abc-123"}' ./prompt.md

# Context from multiple sources merged together
riff -c ./defaults.json -c ./overrides.env -c debug=true ./prompt.md
```

parser/ core Rust library, internal to repo, not published wasm-js/ wasm wasm-bindgen module for
JS/TS runtimes wasm-wasi/ wasm WASI module for polyglot SDK consumption slash-js-web/ sdk Browser
runtime SDK (depends on wasm-js) slash-js-bundle/ sdk ESM server-side SDK (depends on wasm-js)
slash-py/ sdk Python SDK (depends on wasm-wasi) slash-ruby/ sdk Ruby SDK (depends on wasm-wasi)
slash-php/ sdk PHP SDK (depends on wasm-wasi) slash-elixir/ sdk Elixir SDK (depends on wasm-wasi)
slash-ocaml/ sdk OCaml SDK (depends on wasm-wasi) slash-haskell/ sdk Haskell SDK (depends on
wasm-wasi) slash-dart/ sdk Dart SDK (depends on wasm-wasi) slash-java/ sdk Java SDK (depends on
wasm-wasi) slash-go/ sdk Go SDK (depends on wasm-wasi) slash-zig/ sdk Zig SDK (native FFI or
wasm-wasi) slash-rs/ sdk Rust SDK, thin published crate wrapping parser riff/ cli CLI binary
(depends on slash-rs) riff-deb/ pkg Debian package riff-rpm/ pkg RPM package riff-oci/ pkg OCI
container image riff-proto/ pkg Proto toolchain plugin website/ docs Static site for documentation
and promotion docs/ docs ADRs and formal spec (bundled into website)
