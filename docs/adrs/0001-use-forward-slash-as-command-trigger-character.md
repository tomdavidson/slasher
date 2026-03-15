---
number: 1
title: Use Forward Slash as Command Trigger Character
date: 2026-03-13
status: accepted
---

# 1. Use Forward Slash as Command Trigger Character

Date: 2026-03-13

## Status

Accepted

## Context

The parser needs a reliable way to distinguish command lines from free-form text in a mixed-content
input stream. The trigger character must be unambiguous in the first column, easy to type, and
familiar to users of chat-style slash command interfaces (Discord, Slack, etc.). It must also allow
the parser to detect commands in a single left-to-right scan without backtracking.

## Decision

A command line is any line whose first non-whitespace character is `/` (forward slash, U+002F).

Command line structure:

```text
/<command-name>[<whitespace><arguments-prefix>]
```

Command name rules:

- Regex: `[a-z][a-z0-9-]*`
- Starts immediately after the leading `/` with no intervening space.
- Ends at the first whitespace character or end-of-line.
- Must begin with a lowercase ASCII letter and may contain lowercase letters, digits, and hyphens.

Arguments prefix (optional):

- Everything after the first whitespace following the command name.
- May contain inline arguments, an inline fence opener (see ADR-0003), or a continuation marker (see
  ADR-0002).

Any line that does not begin (after optional leading whitespace) with `/` is a non-command line.
Non-command lines are collected into text blocks (see ADR-0009).

A line where the first non-whitespace character is `/` but the text after `/` does not match the
command name regex (e.g., a bare `/` or `/123`) is treated context-dependently: during accumulation
it becomes a literal payload line; in idle state it is treated as a non-command line.

## Consequences

- Command detection is a single character check on each line after skipping leading whitespace,
  making it O(1) per line.
- The forward slash convention aligns with established chat/CLI slash command patterns, reducing
  user learning curve.
- The strict command name regex prevents ambiguity with path-like content (e.g., `/usr/bin` fails
  the regex because `usr/bin` contains `/`).
- Optional leading whitespace tolerance means indented commands (e.g., in nested contexts) are still
  recognized.
