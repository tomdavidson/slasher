---
number: 4
title: State Machine Parser with No AST
date: 2026-03-13
status: accepted
---

# 4. State Machine Parser with No AST

Date: 2026-03-13

## Status

Accepted

## Context

The slash-cmd format is line-oriented with three distinct argument modes (single-line, continuation,
fenced). A traditional recursive-descent parser or AST-building approach would add unnecessary
complexity for a format that can be fully parsed in a single forward pass over lines. The parser
needs to track which mode it is in, accumulate payloads accordingly, and emit finalized commands and
text blocks without constructing an intermediate tree representation.

## Decision

The parser uses a line-based state machine with three primary states and no intermediate AST.

### States

- `idle`: not currently accumulating a command. The parser scans each line for command detection
  (ADR-0001).
- `accumulating`: collecting continuation-mode arguments for a command (ADR-0002). Each line is
  tested for continuation markers, empty-line terminators, fence openers, or literal content.
- `inFence`: collecting raw lines inside a fenced block for a command (ADR-0003). All lines are
  literal until a closing fence is detected.

### Tracked state per command

While processing the current command, the parser tracks:

- `name`: the command name from the first line.
- `arguments.header`: the header text after the command name on the first line, before any fence
  opener.
- `arguments.mode`: one of `single-line`, `continuation`, or `fence`.
- `arguments.fence_lang`: optional language identifier when in fence mode.
- `arguments.payload`: the assembled argument string, with newlines between logical payload lines.

### Multi-command scanning

The parser walks the normalized input (ADR-0010) line by line:

1. In `idle`, when it encounters a command line, it starts a new command according to argument mode
   rules (ADR-0005 for single-line, ADR-0002 for continuation, ADR-0003 for fenced).
2. After a command is finalized (single-line completion, empty-line termination in continuation, or
   fence close), the parser returns to `idle` and continues scanning.
3. Non-command lines encountered in `idle` are collected into text blocks. Each text block records
   its line range (start_line, end_line inclusive) and content with internal newline separators
   reflecting the normalized input.
4. Commands and text blocks are emitted in document order.

### No AST

The parser emits a flat list of commands and text blocks directly. There is no intermediate abstract
syntax tree. The `children` field on commands is reserved for future hierarchical structures but is
currently always empty.

## Consequences

- The single-pass, line-based approach is simple to implement, test, and reason about. Each state
  has a small, well-defined set of transitions.
- Memory usage is proportional to the largest single command payload, not the entire document, since
  commands are finalized and emitted incrementally.
- No AST means no tree-manipulation overhead. Consumers work directly with the flat command and text
  block lists.
- Adding new states or argument modes in the future requires extending the state machine rather than
  restructuring a grammar.
- The three-state design maps directly to the three argument modes, making the correspondence
  between spec and implementation transparent.
