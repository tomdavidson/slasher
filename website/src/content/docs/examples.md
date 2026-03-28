---
title: Set List
description: Parsing examples drawn from the Slash Command Syntax specification.
---

Each example shows input text and the resulting parse output. These are drawn from Appendix B of the
[Syntax Specification](/spec/syntax/).

## Single-Line Command

A command on one line with inline arguments. The entire arguments portion becomes both the header
and the payload.

**Input:**

```
/echo hello world
```

**Result:**

```json
{
  "commands": [
    {
      "id": "cmd-0",
      "name": "echo",
      "arguments": {
        "mode": "single-line",
        "header": "hello world",
        "fence_lang": null,
        "payload": "hello world"
      },
      "range": { "start_line": 0, "end_line": 0 }
    }
  ],
  "text_blocks": [],
  "warnings": []
}
```

## Joined Multi-Line Command

Three physical lines joined by backslash continuation into a single logical line. The join removes
each trailing `\` and concatenates with the next line.

**Input:**

```
/deploy production \
  --region us-west-2 \
  --canary
```

**After joining:**

```
/deploy production   --region us-west-2   --canary
```

**Result:**

```json
{
  "commands": [
    {
      "id": "cmd-0",
      "name": "deploy",
      "arguments": {
        "mode": "single-line",
        "header": "production   --region us-west-2   --canary",
        "fence_lang": null,
        "payload": "production   --region us-west-2   --canary"
      },
      "range": { "start_line": 0, "end_line": 2 }
    }
  ],
  "text_blocks": [],
  "warnings": []
}
```

## Fenced Command with Header

A command with both a header (inline arguments before the fence opener) and a fenced payload. The
fence language identifier `json` is extracted from the opener.

**Input:**

````
/mcp call_tool write_file ```json
{ "path": "/src/index.ts" }
```
````

**Result:**

```json
{
  "commands": [
    {
      "id": "cmd-0",
      "name": "mcp",
      "arguments": {
        "mode": "fence",
        "header": "call_tool write_file",
        "fence_lang": "json",
        "payload": "{ \"path\": \"/src/index.ts\" }"
      },
      "range": { "start_line": 0, "end_line": 2 }
    }
  ],
  "text_blocks": [],
  "warnings": []
}
```

## Backslash Join into Fence

The command line and fence opener span two physical lines. Backslash continuation joins them before
fence detection occurs.

**Input:**

````
/mcp call_tool write_file \
```json
{ "path": "foo" }
```
````

**After joining lines 0-1:**

````
/mcp call_tool write_file ```json
````

**Result:**

```json
{
  "commands": [
    {
      "id": "cmd-0",
      "name": "mcp",
      "arguments": {
        "mode": "fence",
        "header": "call_tool write_file",
        "fence_lang": "json",
        "payload": "{ \"path\": \"foo\" }"
      },
      "range": { "start_line": 0, "end_line": 3 }
    }
  ],
  "text_blocks": [],
  "warnings": []
}
```

## Text Blocks and Multiple Commands

Text, commands, and more text. The parser partitions everything in document order, assigning
sequential IDs to commands and text blocks independently.

**Input:**

```
Welcome to the deployment system.

/deploy staging
/notify team --channel ops
Deployment complete.
```

**Result:**

```json
{
  "text_blocks": [
    {
      "id": "text-0",
      "content": "Welcome to the deployment system.\n",
      "range": { "start_line": 0, "end_line": 1 }
    },
    {
      "id": "text-1",
      "content": "Deployment complete.",
      "range": { "start_line": 4, "end_line": 4 }
    }
  ],
  "commands": [
    {
      "id": "cmd-0",
      "name": "deploy",
      "arguments": {
        "mode": "single-line",
        "header": "staging",
        "fence_lang": null,
        "payload": "staging"
      },
      "range": { "start_line": 2, "end_line": 2 }
    },
    {
      "id": "cmd-1",
      "name": "notify",
      "arguments": {
        "mode": "single-line",
        "header": "team --channel ops",
        "fence_lang": null,
        "payload": "team --channel ops"
      },
      "range": { "start_line": 3, "end_line": 3 }
    }
  ],
  "warnings": []
}
```

## Invalid Slash Lines

Lines starting with `/` that don't match the command name pattern are classified as text, not
commands. This includes bare `/`, numbers after the slash, capitalized names, and names ending with
a hyphen.

**Input:**

```
/123 not a command
/ bare slash
/Hello capitalized
/deploy staging
```

**Result:**

```json
{
  "text_blocks": [
    {
      "id": "text-0",
      "content": "/123 not a command\n/ bare slash\n/Hello capitalized",
      "range": { "start_line": 0, "end_line": 2 }
    }
  ],
  "commands": [
    {
      "id": "cmd-0",
      "name": "deploy",
      "arguments": {
        "mode": "single-line",
        "header": "staging",
        "fence_lang": null,
        "payload": "staging"
      },
      "range": { "start_line": 3, "end_line": 3 }
    }
  ],
  "warnings": []
}
```

## Unclosed Fence

When the input ends before a fence closer is found, the command is finalized with whatever payload
has accumulated. An `unclosed_fence` warning is emitted.

**Input:**

````
/mcp call_tool ```json
{ "incomplete": true }
````

**Result:**

```json
{
  "commands": [
    {
      "id": "cmd-0",
      "name": "mcp",
      "arguments": {
        "mode": "fence",
        "header": "call_tool",
        "fence_lang": "json",
        "payload": "{ \"incomplete\": true }"
      },
      "range": { "start_line": 0, "end_line": 1 }
    }
  ],
  "text_blocks": [],
  "warnings": [
    {
      "type": "unclosed_fence",
      "start_line": 0,
      "message": "Fence opened at line 0 was not closed before end of input."
    }
  ]
}
```
