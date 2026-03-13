# small Rust+WASM “slash command parser” component with a JSON-in / JSON-out API, 

## style and patterns
code snippets and patterns in this doc other than the parser semantics are only examples. 
The language, build, and testing patterns override and should be followed.

- toms-clean-code.md
- toms-clean-arch.md
- lang-rust.md
- testing.md
- testing-rust.md
- universal-rust-wasm-lib.md

## 1. Component overview

- Name: `slash_parser`
- Implementation language: Rust
- Compilation target: WebAssembly
- Binding strategy: `wasm-bindgen` for JS/TS environments.[^1][^2]
- Interface style:
    - Input: UTF‑8 text (string).
    - Output: UTF‑8 JSON string conforming to a provided JSON Schema.

Responsibilities:

1. Parse an input text buffer using the **slash-command semantics** described below.
2. Emit a JSON document describing:
    - All detected commands and their arguments,
    - Optional non-command text blocks,
    - A context object.

The WASM module must be deterministic and pure (no I/O, no global mutable state aside from internal caches).

***

## 2. Parsing semantics (Rust core)

The Rust core must implement the exact language you’ve defined:

### 2.1 Input

- Input is a `&str` (UTF‑8 text).
- The parser processes it line-by-line, splitting on `\n`.
- `\r\n` must be normalized to `\n` before parsing.


### 2.2 Command lines

A **command line** is any line whose first non-whitespace character is `/`.

Structure:

- Leading whitespace (spaces/tabs) before `/` is ignored for detection but not included in `raw`.
- After the first `/`, parse `<command-name>` using regex `[a-z][a-z0-9-]*`.
- The rest of the line, after one or more whitespace characters, is `arguments.header` prefix.

Non-command lines:

- Lines that do not start with `/` (ignoring leading whitespace) are considered **non-command text** when the parser is in `idle` state.
- If a command is currently being accumulated, non-command lines are appended to that command’s payload (depending on the state).


### 2.3 States

The parser is a state machine:

- `Idle` – not inside any command.
- `Accumulating` – building `arguments.payload` for a command outside a fence.
- `InFence` – inside a fenced block attached to a command.

State transitions are driven by:

- New command detection,
- Continuation marker `" /"`,
- Fence openers / closers.


### 2.4 Continuation with `" /"`

Continuation is explicit and conservative:

- A line continues the current command if and only if it ends with **space + slash** (`" /"`) immediately before the newline.

Semantics:

- In `Accumulating` (or on the first header line of a command):
    - If the line ends with `" /"`:
        - Remove the trailing `" /"` from the line content.
        - Append the remaining content plus `\n` to `arguments.payload`.
        - Stay in `Accumulating`.
    - If the line does not end with `" /"` and does not start a fence:
        - Append the full line content plus `\n`.
        - Finalize the command and return to `Idle`.

Note: `" /"` must be checked literally; a line ending in `/` with no preceding space is **not** a continuation marker. This avoids collisions with paths like `/var/log/`.

### 2.5 Fenced block arguments

Use markdown-style fenced code blocks:[^3][^4]

#### Fence opener detection

Fence openers can appear:

1. **Inline on the command line**:
    - Inside `<arguments.header>`, find the first occurrence of three or more consecutive backticks: ```…
    - Everything before the backticks remains in `arguments.header`.
    - The backticks and optional language identifier (e.g., `jsonl`) mark the start of fence mode.
2. **On the next line after continuation**:
    - The command line ends with `" /"` and the parser enters `Accumulating`.
    - The next line starts with the fence opener ```[lang].
    - The parser transitions to `InFence`.

Fence metadata:

- Record:
    - Fence marker: number of backticks (typically 3).
    - Optional `lang` following the backticks (up to first whitespace).


#### Fence mode

In `InFence`:

- Every line is appended verbatim to `arguments.payload` with a trailing `\n`.
- `" /"` has no special meaning inside the fence.
- The parser looks for a closing fence: a line whose first non-whitespace characters are the same number of backticks and nothing else but whitespace.

On closing fence:

- The closing fence line is not included in the payload.
- Command is finalized.
- `arguments.mode = "fence"`.
- `arguments.fence_lang` is set to the captured language or `null`.
- Parser returns to `Idle`.


### 2.6 Multiple commands

- In `Idle`, when a command line appears, start a new command.
- When a command is finalized, append it to the `commands` array and go back to `Idle`.
- Non-command text between commands may be recorded as `text_blocks`.

You can treat lines that fall entirely outside commands as text blocks grouped by contiguous ranges.

***

## 3. JSON output format

The Rust core will produce a JSON value of type `SlashParseResult`:

```ts
interface SlashParseResult {
  version: string;            // e.g., "0.1.0"
  context: {
    source?: string;
    timestamp?: string;       // ISO 8601, if provided by caller
    user?: string;
    session_id?: string;
    extra?: Record<string, unknown>;
    [key: string]: unknown;
  };
  commands: Command[];
  text_blocks?: TextBlock[];
}

interface CommandRange {
  start_line: number;         // 0-based or 1-based (document in spec)
  end_line: number;
}

type ArgumentMode = "single-line" | "continuation" | "fence";

interface CommandArguments {
  header?: string;
  mode: ArgumentMode;
  fence_lang?: string | null;
  payload: string;
}

interface Command {
  id: string;
  name: string;
  raw?: string;
  range: CommandRange;
  arguments: CommandArguments;
  children?: Command[];       // reserved
}

interface TextBlock {
  id: string;
  range: CommandRange;
  content: string;
}
```

The JSON Schema in your previous step applies; the Rust types should be defined to match that schema, using `serde` for serialization.[^5][^6][^7]

***

## 4. WASM API design

### 4.1 Rust public API (pre-wasm)

Define a core Rust function:

```rust
pub struct ParserContext {
    pub source: Option<String>,
    pub timestamp: Option<String>,
    pub user: Option<String>,
    pub session_id: Option<String>,
    pub extra: Option<serde_json::Value>,
}

pub fn parse_slash_commands(
    input: &str,
    context: ParserContext,
) -> Result<SlashParseResult, ParseError>;
```

- `ParseError` should include at least a message and optionally a line/column.


### 4.2 wasm-bindgen exports

Use `wasm-bindgen` to expose a JS-friendly API:[^8][^2][^1]

- Module name: `slash_parser`.
- Exported functions:

```rust
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
pub fn parse_text(input: &str) -> Result<JsValue, JsValue>;

#[wasm_bindgen]
pub fn parse_text_with_context(input: &str, context: &JsValue) -> Result<JsValue, JsValue>;
```

Semantics:

- `parse_text`:
    - Uses a default `ParserContext { source: None, ... }`.
    - Returns a `JsValue` containing a JSON object conforming to `SlashParseResult`, via `JsValue::from_serde(&result)`.
- `parse_text_with_context`:
    - `context` is optional; if provided, it is interpreted as a JS object containing any of:
        - `source`, `timestamp`, `user`, `session_id`, `extra`.
    - Rust side should `from_serde` the context into `ParserContext` (or build it manually from `JsValue`).[^9]
    - Returns the same JSON structure as `parse_text`.

On error:

- Return a `JsValue` representing a JS `Error` or a structured `{ error: string, line?: number, column?: number }`.


### 4.3 JS / TS usage expectations

Consumers (e.g. a Node-based or browser-based tool) will:

```ts
import init, { parse_text, parse_text_with_context } from "slash_parser";

await init(); // or equivalent wasm-bindgen init

const result = parse_text_with_context(sourceText, {
  source: "file://path/to/doc.md",
  user: "tom",
});

// `result` is a JS object matching SlashParseResult
console.log(result.commands);
```

The bindings should be compatible with bundlers (Vite, Webpack) and Node 18+.

***

## 5. Non-goals / constraints

To keep Perplexity Computer’s scope tight:

- No I/O: the parser does not read files or network; it only parses the provided string.
- No global config: all configuration is via parameters or context.
- No incremental parsing: first version can parse whole strings only.
- No reliance on external crates beyond:
    - `serde`, `serde_json`,
    - `wasm-bindgen` (and its minimal dependencies).



[^1]: https://rustwasm.github.io/docs/wasm-bindgen/

[^2]: https://rustwasm.github.io/docs/wasm-bindgen/print.html

[^3]: https://www.markdownguide.org/extended-syntax/

[^4]: https://python-markdown.github.io/extensions/fenced_code_blocks/

[^5]: https://json-schema.org/learn/json-schema-examples

[^6]: https://json-schema.org/understanding-json-schema/basics

[^7]: https://blog.promptlayer.com/how-json-schema-works-for-structured-outputs-and-tool-integration/

[^8]: https://rustwasm.github.io/docs/wasm-bindgen/contributing/design/index.html

[^9]: https://users.rust-lang.org/t/how-do-i-sent-a-js-object-to-rust-through-wasm/80007

[^10]: https://www.reddit.com/r/rust/comments/15sjuyo/interfacing_complex_types_in_wasm/

[^11]: https://github.com/rustwasm/wasm-bindgen/discussions/2883

[^12]: https://stackoverflow.com/questions/65242336/js-binding-for-large-rust-object-using-wasm-bindgen

[^13]: https://nickb.dev/blog/recommendations-when-publishing-a-wasm-library/

[^14]: https://www.speakeasy.com/blog/building-speakeasy-openapi-go-library

[^15]: https://github.com/douglance/valrs

[^16]: https://hacks.mozilla.org/2019/08/webassembly-interface-types/

[^17]: https://github.com/WebAssembly/design/blob/main/Rationale.md

[^18]: https://docs.rs/valrs-wasm

[^19]: https://gov.near.org/t/proposal-use-webassembly-interface-types-to-describe-all-standards-interfaces-and-application-contract-interface-aci/23256

[^20]: https://developer.mozilla.org/en-US/docs/WebAssembly/Guides/Concepts

