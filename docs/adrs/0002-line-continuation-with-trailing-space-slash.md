---
number: 2
title: Line Continuation with Trailing Space-Slash
date: 2026-03-13
status: accepted
---

# 2. Line Continuation with Trailing Space-Slash

Date: 2026-03-13

## Status

Accepted

## Context

Commands often need arguments that are too long or too complex for a single line. The parser needs
an explicit, opt-in mechanism for spanning a logical command across multiple physical lines. The
marker must be unambiguous (not confused with path separators or content containing slashes) and
must compose cleanly with the other argument modes (single-line and fenced blocks).

## Decision

Continuation mode is activated by a trailing space-slash (`/`) marker at the end of a line.

### Marker definition

A continuation marker is a line (after normalization per ADR-0010) whose content ends with a space
followed by a slash (`/`) immediately before the line terminator, with nothing after the slash
except optional trailing whitespace.

Formally, a line is a continuation marker if the last two non-whitespace-trailing characters are `/`
(space + slash).

Optional leading whitespace is allowed. For example, `/echo  /` and `/` (space+slash with
indentation) both qualify.

A special case: a line that, aside from leading whitespace, is exactly `/` represents a blank
payload line in continuation mode.

### First command line with continuation

When a command's first line ends with `/`:

1. The parser strips the final `/` from that line's content.
2. The remaining content after the command name (which may be empty) becomes the first segment of
   `arguments.payload`, followed by a newline separator.
3. `arguments.mode` is set to `continuation`.
4. The parser transitions to the `accumulating` state (see ADR-0004).

If the first line does not end with `/` and has no fence opener, the command is single-line (see
ADR-0005).

### Lines in accumulating state

For each subsequent line while in `accumulating`:

- Continuation marker line (ends in `/`): represents a blank payload line. The parser appends a
  newline to `arguments.payload` and remains in `accumulating`.
- Empty line (zero characters after normalization): the parser finalizes the command and transitions
  to `idle`. This blank line is not appended to the payload.
- Any other non-empty line: the parser appends the line content plus a newline to
  `arguments.payload` and remains in `accumulating`.

A fence opener encountered on a continuation line transitions the command from `accumulating` to
`inFence` (see ADR-0003).

### Bare slash is not continuation

A bare `/` at the end of a line (no preceding space) is not a continuation marker. If the line's
first non-whitespace character is `/`, it is processed via command detection (ADR-0001). If the `/`
appears elsewhere, it is literal content. Users needing complex or ambiguous multi-line payloads
should use fenced blocks (ADR-0003).

Example: the input

```text
/echo /
ooga booga
/
testing 123
```

does not include `testing 123` in `/echo`'s payload because the bare `/` line is not a `/` marker.

## Consequences

- Continuation is explicit and opt-in; lines without the marker never accidentally join.
- The space-slash marker avoids ambiguity with path-like content (e.g., `/usr/bin/` has no preceding
  space before the trailing slash).
- Empty lines serve as a natural, visible terminator for multi-line commands.
- Blank payload lines can be represented by the `/` special case, so continuation mode can express
  payloads containing empty lines.
- Round-trip fidelity of original whitespace at continuation boundaries is intentionally lost; the
  canonical form uses newline separators between payload segments.
- Fenced blocks (ADR-0003) remain available as the preferred alternative when payloads are complex
  or contain content that could be confused with markers.
