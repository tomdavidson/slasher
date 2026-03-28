---
title: Parser Engine v0.5.0
description: >
  The Rust library crate that implements every syntax rule defined in
  Slash Command Syntax v1.1.0.
---

The engine is a Rust library crate that implements every syntax rule defined
in the Slash Command Syntax v1.1.0. It consumes a UTF-8 string and produces
Rust domain types through a single public function: `parse_document`. The
engine performs no IO, carries no serialization dependencies, uses no unsafe
code, and maintains no global state. It is designed to be wrapped by SDKs
that handle JSON serialization, WASM bindings, and WASI runtime integration.

*Full engine specification content coming soon.*
