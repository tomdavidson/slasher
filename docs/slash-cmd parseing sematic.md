<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# can you write a spec that use the parseing sematics and and the implemationstion's output's jsonschema?

Yes. Here’s a compact spec that combines the parsing semantics with the JSON Schema for the output.

## 1. Overview

This document specifies:

1. A line-oriented **slash-command language** for embedding commands in text.
2. The **JSON output format** produced by a conforming parser, including a JSON Schema.

The language is designed to:

- Support multiple `/commands` in a single text payload.
- Allow single-line arguments, multi-line arguments with continuation, and fenced blocks.
- Preserve enough structure and context to map commands to MCP/tool calls.

***

## 2. Parsing semantics

### 2.1 Input model

- Input is a sequence of lines, each terminated by `\n`.
- The newline is not part of the line’s content.


### 2.2 Command detection

A **command line** is any line whose first non-whitespace character is `/`.

Command line structure:

```
/<command-name>[<whitespace><arguments-prefix>]
```

- `<command-name>`:
    - Regex: `[a-z][a-z0-9-]*`
    - Ends at the first whitespace or end-of-line.
- `<arguments-prefix>`:
    - Optional.
    - Everything after the first whitespace following `<command-name>`.
    - May contain:
        - Inline arguments.
        - An inline fence opener (```lang).

Any line that does not begin (after leading whitespace) with `/` is a **non-command line**.

### 2.3 States

The parser is a line-based state machine with three states:

- `idle` – not currently accumulating a command.
- `accumulating` – accumulating arguments for a command outside of a fence.
- `inFence` – collecting raw lines inside a fenced block attached to a command.

The parser also tracks:

- The current command’s `name`.
- The header portion after the command name, `arguments.header`.
- The accumulated `arguments.payload`.
- Fence metadata when in `inFence` (fence marker and optional language).

***

## 3. Argument modes

### 3.1 Single-line arguments

If a command line:

- Does not end with a continuation marker, and
- Does not contain a fence opener,

then the text after `<command-name>` (trimmed of leading whitespace once) is the **entire** argument string, and the command is finalized on that line.

Example:

```
/mcp call_tool read_file {"path": "src/index.ts"}
```

- `name`: `"mcp"`
- `arguments.header`: `"call_tool read_file {"path": "src/index.ts"}"`
- `arguments.mode`: `"single-line"`
- `arguments.payload`: `"call_tool read_file {"path": "src/index.ts"}"`


### 3.2 Continuation with `" /"` (space + slash)

A line may explicitly continue its arguments onto the next line by ending with:

- A **space** followed by a slash (`" /"`) immediately before the newline.

Continuation rules (in `accumulating` state or for the first command line):

- If the line ends with `" /"`:
    - Strip the final `" /"` from the line content.
    - Append the remaining content plus a newline `\n` to the current command’s `arguments.payload`.
    - Stay in `accumulating` state; the next line is part of the same command.
- If the line does **not** end with `" /"` and is not starting a fence:
    - Append the full line content (without the newline) and then append `\n`.
    - Finalize the command.
    - Return to `idle`.

This preserves newlines in the payload and avoids accidental continuation when a line ends in a slash that is part of data (since there is no preceding space).

Example:

```
/mcp call_tool read_file / 
{"path": "src/index.ts"}
```

Payload:

```
call_tool read_file 
{"path": "src/index.ts"}
```

`arguments.mode = "continuation"`.

### 3.3 Fenced block arguments

Fenced blocks allow attaching a raw multi-line payload to a command. They follow markdown code-fence semantics.

#### 3.3.1 Fence opener forms

There are two valid ways to enter fence mode:

1. **Inline fence on the command line**

```text
/<command-name> <arguments-prefix> ```[lang]
```

    - The first occurrence of three or more consecutive backticks (```…) in `<arguments-prefix>` is treated as the fence opener.
    - Any text before the opener remains in `arguments.header`.
    - The parser records the fence marker length and optional language (e.g., `jsonl`) and enters `inFence` state.
2. **Fence on the next line after continuation**

```text
/<command-name> <arguments-prefix> / 
```[lang]
<payload>
```

```

- The command line ends with `" /"`, so the parser starts `accumulating` and preserves a newline.
- The next line is a fence opener (```[lang]); the parser switches into `inFence` state.
- All subsequent lines until the closing fence are payload.

```


#### 3.3.2 Fence mode semantics

While in `inFence`:

- The parser ignores continuation markers (`" /"`); they are treated as part of the content.
- It collects lines verbatim, appending `\n` after each.
- It looks for a line that consists of the same number of backticks as the opener (e.g., ```), optionally surrounded by whitespace, with no other non-backtick characters [][].
- That closing fence line is not included in the payload.
- When the closing fence is found:
    - The command is finalized.
    - `arguments.mode` is `"fence"`.
    - `arguments.fence_lang` is set to the captured language identifier or `null`.
    - The parser returns to `idle`.

Example (inline opener):

```text
/mcp call_tool write_file ```jsonl
{"type": "function_call_start", "name": "call_tool_read", "call_id": 1}
{"type": "parameter", "key": "name", "value": "server:tool_name"}
```

```

Payload:

```text
{"type": "function_call_start", "name": "call_tool_read", "call_id": 1}
{"type": "parameter", "key": "name", "value": "server:tool_name"}
```

`arguments.mode = "fence"`, `arguments.fence_lang = "jsonl"`.

---

## 4. Multiple commands in one payload

The parser:

- Scans line by line.
- Whenever it sees a new command line in `idle`, it starts a new command.
- After a command is finalized, it returns to `idle` and continues scanning for the next command.

Non-command text between commands can optionally be returned as `text_blocks`.

---

## 5. JSON output format

### 5.1 Top-level JSON

A parser run over a single input payload produces a JSON object:

```json
{
  "version": "0.1.0",
  "context": {
    "source": "string",
    "timestamp": "2026-03-13T10:40:00Z",
    "user": "string",
    "session_id": "string",
    "extra": {}
  },
  "commands": [ /* Command[] */ ],
  "text_blocks": [ /* TextBlock[] */ ]
}
```

- `version`: schema/protocol version.
- `context`: arbitrary metadata about where/how the text was obtained.
- `commands`: ordered list of parsed commands.
- `text_blocks`: optional list of non-command spans.


### 5.2 Command object

Each `commands[i]` is:

```json
{
  "id": "cmd-1",
  "name": "mcp",
  "raw": "/mcp call_tool write_file ```jsonl\n...\n```",
  "range": {
    "start_line": 10,
    "end_line": 20
  },
  "arguments": {
    "header": "call_tool write_file",
    "mode": "fence",
    "fence_lang": "jsonl",
    "payload": "{\n  \"path\": \"...\"\n}"
  },
  "children": []
}
```

- `id`: unique identifier for the command instance.
- `name`: the command name after the leading `/` (e.g., `"mcp"`).
- `raw`: the exact text span (from input) for this command, including its header and argument lines.
- `range.start_line` / `range.end_line`: zero- or one-based line indices (convention is implementation-defined; schema only constrains them as integers).
- `arguments.header`: whatever followed the command name on the header line, before any fence opener.
- `arguments.mode`: one of:
    - `"single-line"`
    - `"continuation"`
    - `"fence"`
- `arguments.fence_lang`: language tag from the fence opener (e.g., `"jsonl"`), or `null`.
- `arguments.payload`: final assembled argument string, with newlines preserved.
- `children`: reserved for future hierarchical command structures (may be an empty array).


### 5.3 TextBlock object

Optional representation of non-command text regions:

```json
{
  "id": "text-1",
  "range": {
    "start_line": 0,
    "end_line": 9
  },
  "content": "arbitrary text\n..."
}
```


---

## 6. JSON Schema

A formal JSON Schema for the parser output:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://example.com/schemas/slash-commands-output.json",
  "title": "Slash Command Parser Output",
  "type": "object",
  "required": ["version", "context", "commands"],
  "properties": {
    "version": {
      "type": "string",
      "description": "Output schema/protocol version."
    },
    "context": {
      "type": "object",
      "description": "Metadata for the parsed payload.",
      "required": ["source"],
      "properties": {
        "source": {
          "type": "string",
          "description": "Identifier for the source (file path, URI, etc.)."
        },
        "timestamp": {
          "type": "string",
          "format": "date-time"
        },
        "user": {
          "type": "string"
        },
        "session_id": {
          "type": "string"
        },
        "extra": {
          "type": "object",
          "description": "Arbitrary additional context.",
          "additionalProperties": true
        }
      },
      "additionalProperties": true
    },
    "commands": {
      "type": "array",
      "description": "List of parsed slash commands in order of appearance.",
      "items": {
        "$ref": "#/$defs/command"
      }
    },
    "text_blocks": {
      "type": "array",
      "description": "Optional non-command text spans between commands.",
      "items": {
        "$ref": "#/$defs/textBlock"
      }
    }
  },
  "$defs": {
    "range": {
      "type": "object",
      "required": ["start_line", "end_line"],
      "properties": {
        "start_line": {
          "type": "integer",
          "minimum": 0
        },
        "end_line": {
          "type": "integer",
          "minimum": 0
        }
      },
      "description": "Inclusive line range for a command or text block."
    },
    "arguments": {
      "type": "object",
      "required": ["mode", "payload"],
      "properties": {
        "header": {
          "type": "string",
          "description": "Header arguments present on the command line before any fence."
        },
        "mode": {
          "type": "string",
          "enum": ["single-line", "continuation", "fence"],
          "description": "How the payload was assembled."
        },
        "fence_lang": {
          "type": ["string", "null"],
          "description": "Fence language identifier if a fenced block was used."
        },
        "payload": {
          "type": "string",
          "description": "Final assembled argument string with newlines preserved."
        }
      },
      "additionalProperties": false
    },
    "command": {
      "type": "object",
      "required": ["id", "name", "range", "arguments"],
      "properties": {
        "id": {
          "type": "string",
          "description": "Unique identifier for this command instance."
        },
        "name": {
          "type": "string",
          "description": "Command name, extracted after the leading slash."
        },
        "raw": {
          "type": "string",
          "description": "Original text span (header + arguments) for this command."
        },
        "range": {
          "$ref": "#/$defs/range"
        },
        "arguments": {
          "$ref": "#/$defs/arguments"
        },
        "children": {
          "type": "array",
          "description": "Nested commands if supported by the implementation.",
          "items": {
            "$ref": "#/$defs/command"
          }
        }
      },
      "additionalProperties": false
    },
    "textBlock": {
      "type": "object",
      "required": ["id", "range", "content"],
      "properties": {
        "id": {
          "type": "string"
        },
        "range": {
          "$ref": "#/$defs/range"
        },
        "content": {
          "type": "string",
          "description": "Raw text content between commands."
        }
      },
      "additionalProperties": false
    }
  },
  "additionalProperties": false
}
```

This ties the parsing semantics directly to a concrete, machine-checkable output format. You can evolve `version` and `context.extra` without breaking consumers, and your MCP/tool layer can safely consume `commands[*].arguments` knowing how they were assembled.

Would you like a minimal TypeScript `parseSlashCommands(text: string)` stub that returns an object conforming to this schema?

