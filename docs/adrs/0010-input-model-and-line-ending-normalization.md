---
number: 10
title: Input Model and Line Ending Normalization
date: 2026-03-14
status: accepted
---

# 10. Input Model and Line Ending Normalization

Date: 2026-03-14

## Status

Accepted

## Context

The parser consumes text input from various sources (files, stdin, editor buffers) that may use
different line ending conventions: CRLF (Windows), LF (Unix), or bare CR (legacy Mac). Without a
normalization step, every downstream rule (command detection, continuation markers, fence
boundaries) would need to handle all three variants independently, increasing complexity and bug
surface.

A clear input model also needs to distinguish between structural line terminators (which delimit
parser lines) and literal backslash-n sequences that appear inside content such as JSON strings.

## Decision

The parser applies line ending normalization as a preprocessing step before any parsing logic runs.

The normalization rules are:

1. Replace all CRLF (\r\n) sequences with LF (\n).
2. Replace all remaining bare CR (\r) characters with LF (\n).

After normalization:

- All line terminators are LF regardless of the original encoding.
- The parser iterates over the normalized input using LF as the line separator.
- Each LF terminates a line; the LF itself is not part of the line content.
- A trailing LF at the end of input produces a trailing empty line.
- Literal escape sequences like `\\n` inside content (e.g., JSON strings `"blah\\nblah"`) are
  ordinary characters, not line terminators. They are preserved verbatim in payloads.

The parser then operates on a sequence of logical lines (possibly including blank lines) derived
solely from the normalized line terminators.

## Consequences

- All downstream parsing rules (command detection per ADR-0001, continuation per ADR-0002, fencing
  per ADR-0003) operate on a uniform LF-only input model.
- Input from any OS line ending convention is handled identically with no special-casing in the
  state machine (ADR-0004).
- The normalization is a single pass over the input, adding negligible overhead.
- Content-embedded escape sequences are never confused with structural line terminators.
- The preprocessing step is the first thing that runs, establishing a clean invariant that all
  subsequent parsing logic can rely on.
