---
number: 8
title: Sequential Zero-Based Command IDs
date: 2026-03-13
status: accepted
---

# 8. Sequential Zero-Based Command IDs

Date: 2026-03-13

## Status

Accepted

## Context

The parser output contains multiple commands and text blocks that consumers need to reference,
correlate, and process. Each element needs a stable, predictable identifier that conveys its type
and position within the document. UUIDs or random IDs would make output non-deterministic and harder
to test. Line numbers alone are fragile because they change when content is inserted or removed
above.

## Decision

Every command and text block receives a sequential, zero-based identifier with a type prefix.

### ID format

- Commands: `cmd-0`, `cmd-1`, `cmd-2`, etc.
- Text blocks: `text-0`, `text-1`, `text-2`, etc.

The prefix indicates the element type. The numeric suffix is a zero-based counter that increments
independently for each type, in document order.

### Assignment rules

1. IDs are assigned in the order elements appear in the normalized input.
2. The command counter and text block counter are independent sequences, both starting at 0.
3. IDs are deterministic: the same input always produces the same IDs.

### Range tracking

In addition to the ID, every command and text block carries a `range` object with:

- `start_line`: the first line of the element (inclusive).
- `end_line`: the last line of the element (inclusive).

Line numbers provide a secondary reference for diagnostics and error reporting, complementing the
stable ID.

## Consequences

- Output is fully deterministic, making snapshot testing and diffing straightforward.
- The type prefix eliminates ambiguity when referencing elements (no confusion between command 1 and
  text block 1).
- Zero-based indexing aligns with array indexing in most programming languages, simplifying consumer
  code.
- Independent counters mean inserting a new text block does not change command IDs, and vice versa.
- The `range` field preserves source location for error reporting without overloading the ID with
  positional information.
