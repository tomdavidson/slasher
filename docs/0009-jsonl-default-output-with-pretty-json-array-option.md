---
number: 9
title: JSONL Default Output with Pretty JSON Array Option
date: 2026-03-13
status: accepted
---

# 9. JSONL Default Output with Pretty JSON Array Option

Date: 2026-03-13

## Status

Accepted

## Context

The parser needs a serialization format for its output that is both machine-readable for downstream
tooling and human-inspectable for debugging. The output must represent the full result of a parse
run, including metadata, commands with their argument payloads, and interleaved text blocks. Two
common JSON-based approaches exist: a single JSON document (easy to pretty-print, harder to stream)
and JSONL (one object per line, streamable, harder to read).

## Decision

The parser supports two output formats, selectable via CLI flag (see ADR-0007):

- JSONL (default): one JSON object per line, suitable for streaming and piping.
- Pretty JSON array: a single formatted JSON document, suitable for human inspection.

### Output envelope

A parser run produces a JSON object with the following top-level structure:

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
  "commands": [],
  "text_blocks": []
}
```

- `version`: semantic version of the output schema.
- `context`: metadata about the parse run. Fields `source`, `timestamp`, `user`, and `session_id`
  are populated from CLI flags (ADR-0007) or defaults. `extra` is a free-form object for additional
  metadata.
- `commands`: ordered array of Command objects.
- `text_blocks`: ordered array of TextBlock objects.

### Command schema

Each element in `commands` is:

````json
{
  "id": "cmd-0",
  "name": "mcp",
  "raw": "/mcp call_tool write_file ```jsonl\n...\n```",
  "range": { "start_line": 10, "end_line": 20 },
  "arguments": {
    "header": "call_tool write_file",
    "mode": "fence",
    "fence_lang": "jsonl",
    "payload": "{\n  \"path\": \"...\"\n}"
  },
  "children": []
}
````

- `id`: unique identifier per ADR-0008 (`cmd-0`, `cmd-1`, etc.).
- `name`: command name without the leading `/`.
- `raw`: exact source slice for the command (header + all argument lines).
- `range`: inclusive line range the command covers (`start_line`, `end_line`).
- `arguments`: argument payload object per ADR-0005 with `header`, `mode`, `fence_lang`, and
  `payload` fields.
- `children`: reserved for future hierarchical structures, currently always empty.

### TextBlock schema

Each element in `text_blocks` is:

```json
{ "id": "text-0", "range": { "start_line": 0, "end_line": 9 }, "content": "arbitrary text\n..." }
```

- `id`: unique identifier per ADR-0008 (`text-0`, `text-1`, etc.).
- `range`: inclusive line range.
- `content`: exact text for the block with internal newline separators from the normalized input.

## Consequences

- JSONL as default enables streaming pipelines where consumers process commands as they are emitted
  without waiting for the full document.
- Pretty JSON mode provides a single-document view for debugging and manual inspection.
- The envelope schema captures both parse results and run metadata, making output self-describing
  and reproducible.
- The `raw` field on commands preserves the original source text for round-trip inspection or error
  reporting.
- The `children` field is a forward-compatible extension point that does not affect current
  consumers.
- Text blocks between commands are explicitly captured rather than discarded, ensuring no input
  content is silently lost.
