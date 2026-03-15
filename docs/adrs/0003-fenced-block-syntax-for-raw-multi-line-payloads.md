---
number: 3
title: Fenced Block Syntax for Raw Multi-line Payloads
date: 2026-03-13
status: accepted
---

# 3. Fenced Block Syntax for Raw Multi-line Payloads

Date: 2026-03-13

## Status

Accepted

## Context

Some commands need to carry large, raw, multi-line payloads (JSON documents, code snippets,
configuration blocks) where the content itself may contain sequences that would otherwise be
interpreted as continuation markers (`/`) or command triggers (`/`). Continuation mode (ADR-0002)
cannot safely transport such content because every line is subject to marker detection. A verbatim
transport mechanism is needed that suspends all parsing rules until an explicit closing delimiter.

## Decision

Fenced blocks use markdown-style backtick fences to wrap raw payloads attached to a command.

### Fence openers

There are two ways to enter fence mode:

1. Inline fence on the command line:

   ````text
   /<command-name> <arguments-prefix> ```[lang]
   <payload lines>
   ````
   ```
   The first occurrence of three or more consecutive backticks in the arguments prefix is treated as the fence opener. Text before the opener is kept in `arguments.header`. The parser records the fence marker length (number of backticks) and the optional language identifier (e.g., `jsonl`).
   ```

2. Fence on the next line after continuation:

   ````text
   /<command-name> <arguments-prefix> /
   ```[lang]
   <payload lines>
   ````
   ```
   The first line ends with ` /` (entering continuation per ADR-0002), but the next line is recognized as a fence opener. The parser transitions from `accumulating` to `inFence` for this command.
   ```

### Fence mode semantics

While in `inFence` state (see ADR-0004):

- All lines (including blank lines) are appended to `arguments.payload` with newline separators.
- Continuation markers (`/`) inside a fence are treated as literal content, not syntax.
- Command triggers (`/`) at the start of a line inside a fence are treated as literal content.

### Closing fence

A line is a closing fence if, after trimming leading and trailing whitespace:

- It consists solely of backticks.
- The number of backticks is greater than or equal to the opener's count.

The closing fence line itself is not appended to `arguments.payload`.

### Resulting command state

Once the closing fence is found:

- `arguments.mode` is set to `fence`.
- `arguments.fence_lang` is the captured language tag or null.
- The command is finalized and the parser transitions back to `idle`.

## Consequences

- Payloads inside fenced blocks are completely verbatim; no parsing rules apply to content lines,
  eliminating escaping concerns.
- The variable-length backtick fence (3 or more) means content containing triple backticks can be
  fenced with four or more backticks, avoiding collision.
- The inline and continuation-then-fence entry paths give authors flexibility in how they structure
  command lines.
- Language tags (e.g., `jsonl`, `yaml`) are captured as metadata, enabling downstream consumers to
  apply format-specific parsing to the payload.
- Fenced blocks have a clear, mandatory closing delimiter, so the parser always knows when the
  payload ends (unlike continuation mode which relies on empty line termination).
