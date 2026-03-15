## 1. Input model and line normalization

- The parser consumes a single text input as a byte/char stream.
- It treats certain characters as **line terminators** in the input stream, distinct from literal
  `\n` characters that might appear inside string data.

### 1.1 Line ending normalization

Before any parsing:

- Replace all `"\r\n"` with `"\n"`.
- Replace all remaining `"\r"` with `"\n"`.

After normalization:

- `CRLF`, `LF`, and bare `CR` are all treated equivalently as a single `LF`.
- The parser then **iterates over the input using these `\n` characters as separators** to obtain
  lines. Conceptually:

  - Each `\n` in the normalized input terminates a line.
  - The `\n` itself is not part of the line’s content.
  - A final `\n` at the end of input produces a trailing empty line.

- Literal `\n` sequences that appear inside line content (for example in JSON strings like
  `"blah\\nblah"`) are **not** treated as line terminators. They are ordinary characters and are
  preserved verbatim in the payload.

Result: the parser operates on a sequence of logical lines (possibly including blank lines) derived
solely from the normalized line terminators.

## 2. Command detection

A **command line** is any line whose first non-whitespace character is `/`.

Command line structure:

```text
/<command-name>[<whitespace><arguments-prefix>]
```

- `<command-name>`:
  - Regex: `[a-z][a-z0-9-]*`.
  - Starts immediately after the leading `/`.
  - Ends at the first whitespace or end-of-line.
- `<arguments-prefix>` (optional):
  - Everything after the first whitespace following `<command-name>`.
  - May contain:
    - Inline arguments.
    - An inline fence opener (```lang).

Any line that does **not** begin (after leading whitespace) with `/` is a **non-command line**.

## 3. Parser state machine

The parser is line-based and uses three primary states:

- `idle` – not currently accumulating a command.
- `accumulating` – collecting continuation-mode arguments for a command.
- `inFence` – collecting raw lines inside a fenced block for a command.

For the **current command**, the parser tracks:

- `name`: the command name from the first line.
- `arguments.header`: the header text after the command name on the first line, **before** any fence
  opener.
- `arguments.mode`: one of `"single-line"`, `"continuation"`, or `"fence"`.
- `arguments.fence_lang`: optional language identifier when in fence mode.
- `arguments.payload`: the assembled argument string, with `\n` between logical payload lines.

## 4. Argument modes

### 4.1 Single-line mode

If a command line:

- Does **not** end with a continuation marker (see 4.2), and
- Does **not** contain a fence opener,

then:

- The text after `<command-name>` (trimmed once for the initial space) is the entire argument.
- The command is finalized on that line.

Properties:

- `arguments.mode = "single-line"`.
- `arguments.payload` is exactly that argument string (the parser does not append a newline simply
  for being the last argument).

### 4.2 Continuation mode (explicit `" /"` marker)

Lines that end with " /" are continuation markers and contribute payload lines as described.

Lines that are completely empty end the command.

Lines whose first non-whitespace is / but do not form a valid command name are treated as literal
payload lines.

Lines that form a valid new command name are out of scope for this command (you can either treat
them as payload or define that they end the current command; your current implementation treats the
invalid / as payload).

Continuation is **opt-in** and **strict**.

#### 4.2.1 Continuation marker definition

- A **continuation marker** is defined as a line (after normalization) whose content ends with a
  **space + slash** (`" /"`) immediately before the line terminator, with nothing after that slash.
- Optional leading whitespace is allowed. For example, `"/echo  /"` or `"   /"` (space+slash with
  indentation) both qualify as continuation markers.

Formally, a line is a continuation marker if it matches:

- `^[ \t]*.*[ ]/[ ]*$` and the last two non-newline characters are `" /"`.

A special case of this is a line that, aside from leading whitespace, is exactly `" /"`; this
represents a **blank payload line** in continuation mode.

#### 4.2.2 First command line with continuation

For a command’s first line:

- If it ends with `" /"` (continuation marker):
  - The parser **strips** the final `" /"` from that line’s content.
  - The remaining content (which might be empty after the header) is appended to
    `arguments.payload`, followed by `"\n"` if you treat it as part of the payload body.
  - `arguments.mode` is set to `"continuation"`.
  - The parser moves to `accumulating`.

- If it does **not** end with `" /"` and has no fence opener:
  - The parser treats the command as a single-line command (4.1).

#### 4.2.3 Lines in `accumulating` state

In `accumulating`, for each subsequent line:

Let `line` be the current line content (without the trailing `\n`):

- **Continuation marker line** (matches the pattern, ends in `" /"` with no content after it):
  - This represents a **blank payload line**.
  - The parser appends `"\n"` to `arguments.payload`.
  - The parser remains in `accumulating`.

- **Empty line** (`line == ""` after normalization):
  - This is a **true blank line** with no continuation marker.
  - The parser **finalizes the command** and transitions to `idle`.
  - This blank line is **not** appended to `arguments.payload`.
  - Subsequent lines are parsed independently (either as text or new commands).

- **Any other non-empty line** (not a marker):
  - The parser appends `line + "\n"` to `arguments.payload`.
  - The parser remains in `accumulating`.

#### 4.2.4 Bare slash is not continuation

- A bare `/` at the end of a line (with no preceding space as `" /"`) is **not** a continuation
  marker.

Behavior:

- If the line’s first non-whitespace character is `/`, it is processed via command detection (either
  a new command or invalid command name).
- If the `/` appears elsewhere in the line, it is just part of the literal content.
- The parser does not introduce any special handling for this edge case; users are encouraged to use
  fenced blocks when they need more complex or ambiguous multi-line payloads.

**Implication:** examples like:

```text
/echo / 
ooga booga 
/
testing 123
```

do **not** cause `testing 123` to be included in `/echo`’s payload, because the line with just `/`
is not a `" /"` marker and either starts a new command or is treated as content depending on
position.

### 4.3 Fenced block mode

Fenced blocks allow attaching a raw, multi-line payload to a command, similar to markdown code
fences.

#### 4.3.1 Fence openers

Two ways to enter fence mode:

1. **Inline fence on the command line**

   ````text
   /<command-name> <arguments-prefix> ```[lang]
   ````

   - In `<arguments-prefix>`, the first occurrence of three or more consecutive backticks (```…) is
     treated as the fence opener.
   - Text before the opener is kept in `arguments.header`.
   - The parser records:
     - Fence marker length (number of backticks).
     - Optional language identifier (e.g., `jsonl`).
   - The parser enters `inFence` state.

2. **Fence on the next line after continuation**

   ````text
   /<command-name> <arguments-prefix> / 
   ```[lang]
   <payload>
   ````
   ```
   - First line ends with `" /"` → command enters `continuation` mode, but the next line is recognized as a fence opener.  
   - The parser transitions to `inFence` for this command.  
   - Subsequent lines until the fence close are payload lines for this command.
   ```

#### 4.3.2 Fence mode semantics

While in `inFence`:

- All lines (including blank lines) are appended to `arguments.payload` with `"\n"` separators.
- Continuation markers (`" /"`) inside a fence are ignored as syntax and treated as literal content.
- The parser looks for a **closing fence**:

  - After trimming leading/trailing whitespace, a line is a closing fence if:
    - It consists solely of backticks.
    - The number of backticks is **greater than or equal** to the opener’s count.

- The closing fence line itself is **not** appended to `arguments.payload`.
- Once the closing fence is found:
  - `arguments.mode = "fence"`.
  - `arguments.fence_lang` is the captured language (`lang`) or `null`.
  - The command is finalized, and the parser transitions back to `idle`.

## 5. Multiple commands and text blocks

- The parser walks the normalized input line by line.
- In `idle`, when it encounters a command line, it starts a new command according to the rules
  above.
- After a command is finalized (single-line, continuation termination, or fence close), the parser
  returns to `idle` and continues scanning for the next command.
- Non-command text segments between commands can optionally be represented as `text_blocks`:

  - Each `text_block` has:
    - `range.start_line`, `range.end_line` (inclusive, zero-based or one-based by convention).
    - `content`: the exact text for that block, with internal `\n` separators reflecting the
      normalized input.

## 6. JSON output schematics (summary)

A parser run produces a JSON object:

````json
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
```[]

### 6.1 Command

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
```[]

- `id`: unique identifier for this command instance.  
- `name`: command name (without the leading `/`).  
- `raw`: exact source slice for this command (header + argument lines).  
- `range`: inclusive line range that the command covers.  
- `arguments`:
  - `header`: header arguments from the command line before any fence opener.  
  - `mode`: `"single-line" | "continuation" | "fence"`.  
  - `fence_lang`: language tag or `null`.  
  - `payload`: final assembled argument string with `\n` between logical payload lines.  
- `children`: reserved for future hierarchical structures (may be empty).

### 6.2 TextBlock

Each `text_blocks[i]` is:

```json
{
  "id": "text-1",
  "range": {
    "start_line": 0,
    "end_line": 9
  },
  "content": "arbitrary text\n..."
}
````
