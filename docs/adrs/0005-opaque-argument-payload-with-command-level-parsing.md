---
number: 5
title: Opaque Argument Payload with Command-Level Parsing
date: 2026-03-13
status: accepted
---

# 5. Opaque Argument Payload with Command-Level Parsing

Date: 2026-03-13

## Status

Accepted

## Context

Different commands have wildly different argument shapes: some take a single keyword, others take
key-value pairs, and others need entire JSON or YAML documents. If the parser tried to understand
argument structure (quoting, escaping, key-value syntax), it would need to know every command's
grammar. This couples the parser to command definitions and makes the format difficult to extend.

The parser needs to deliver arguments to commands as an opaque payload, letting each command define
its own interpretation.

## Decision

The parser treats all argument content as opaque byte sequences. It determines argument boundaries
and transport mode but never interprets argument semantics.

### Argument structure

Every command carries an `arguments` object with the following fields:

- `header`: the text after the command name on the first line, before any fence opener. This is the
  inline portion of the arguments.
- `mode`: one of `single-line`, `continuation`, or `fence`, indicating how the payload was
  assembled.
- `fence_lang`: the language tag from a fence opener (e.g., `jsonl`), or null if not in fence mode.
- `payload`: the final assembled argument string with newlines between logical payload lines.

### Three argument modes

The parser supports exactly three modes for assembling arguments:

1. Single-line mode (this ADR): the command and all its arguments fit on one line.
2. Continuation mode (ADR-0002): arguments span multiple lines via trailing `/` markers.
3. Fence mode (ADR-0003): arguments are wrapped in backtick fences for verbatim transport.

### Single-line mode

A command is single-line if its first line does not end with a continuation marker (`/` per
ADR-0002) and does not contain a fence opener (per ADR-0003).

In single-line mode:

- The text after the command name, trimmed of the initial separating space, is the entire argument.
- The command is finalized immediately on that line.
- `arguments.mode` is set to `single-line`.
- `arguments.payload` is exactly that argument string. The parser does not append a trailing
  newline.

Example: `/echo hello world` produces `arguments.payload = "hello world"` with
`arguments.mode = "single-line"`.

### Command-level parsing

The parser's job ends at delivering the opaque payload. Each command implementation is responsible
for interpreting its own payload content (splitting on spaces, parsing JSON, etc.). The parser makes
no assumptions about quoting, escaping, or structure within the payload.

## Consequences

- The parser is command-agnostic. New commands can be added without modifying parser logic.
- Argument parsing errors are the command's responsibility, not the parser's, which simplifies error
  boundaries.
- The three-mode system gives authors a clear choice: single-line for simple arguments, continuation
  for moderate multi-line, and fenced blocks for complex or ambiguous payloads.
- The `header` field preserves inline arguments even when a fence is used, allowing commands to
  accept both positional arguments and a fenced body (e.g., ``/mcp call_tool write_file ```jsonl``).
- No quoting or escaping conventions are imposed by the parser, avoiding conflicts with
  payload-specific syntax like JSON strings or shell expressions.
- Textual reconstruction (format_text) guarantees spec-level semantic roundtrip fidelity, not
  byte-for-byte reproduction of the original input. Line ending normalization (ADR-0010), whitespace
  handling, and continuation marker stripping are lossy transformations. The roundtrip invariant is:
  `parse(to_plaintext(parse(input)))` produces structurally equivalent commands and text blocks
  (same names, modes, payloads, and text block content), not identical bytes.
