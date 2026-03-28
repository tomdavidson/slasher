# Solidus

**The gold standard for slash command parsing.**

Solidus parses `/commands` in UTF-8 text. It implements the Slash Command
Syntax v1.1.0 specification with a pure Rust engine: no IO, no serialization,
no unsafe, no global state. `parse_document` is a total function. It accepts
any input and always returns a valid result.

Run the CLI with `riff`. Import the engine in Rust. Load the WASM module in
JavaScript. Same spec, same output, every time.
