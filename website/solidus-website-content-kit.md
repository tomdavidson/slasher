# Solidus Website Content Kit

## PERMANENT PROJECT DESCRIPTION

The gold standard for slash command parsing.

Use this line everywhere: repo description, npm package, crate metadata,
OpenGraph tags, the hero section. It does triple duty: references the
gold solidus coin, declares quality, and describes what the project
actually does.


## ROTATING TAGLINES

These rotate in the hero section or appear as subheads throughout the site.
Grouped by flavor.

### Typography / History

1.  Giving U+002F the spec it deserves.
2.  The most underspecified character on your keyboard. Until now.
3.  Seven centuries of history. One formal specification.
4.  Named for a Roman gold coin. Built for modern text.
5.  From 12th-century punctuation to UTF-8 parsing.
6.  The solidus has been working without a spec since 1150. We fixed that.
7.  One stroke. Formally defined.
8.  Ancient symbol. Modern parser.
9.  Every language has it. No one had specified it.
10. The character everyone uses and no one had defined.

### Technical / Dry Wit

11. Pure Rust. Pure function. Pure spec.
12. No IO. No unsafe. No exceptions. No, seriously.
13. A total function walks into a bar. It handles every input.
14. parse_document always returns. Always.
15. One entry point. Zero failure modes.
16. It cannot panic. We tried.
17. Spec-first. Vibes-never.
18. The parser that treats your arguments as opaque. As it should.
19. Deterministic output. Every time. Across every implementation.
20. Three specs. One character. Zero ambiguity.

### Light Rock Touch (where it lands naturally)

21. One riff. Every format.
22. Same spec, every stage.
23. Tuned to the spec. No improvisation.
24. Drop the needle on any input. Same output every time.


## HERO SECTION

### Headline
Solidus

### Permanent Subhead
The gold standard for slash command parsing.

### Body (2-3 sentences max)
Solidus is a formally specified parser for /command syntax in UTF-8 text.
The engine is pure Rust with a single entry point, parse_document, that
accepts any input and always returns a valid result. No IO. No unsafe.
No panic. No exceptions.

### Primary CTA
Read the Spec

### Secondary CTA
See Examples


## FEATURE CARDS (Landing Page)

### Card 1: Formally Specified
Slash commands appear in chat apps, developer tools, and AI agent
interfaces. Everyone implements them differently. Solidus defines the
syntax with an RFC-style specification, complete with ABNF grammar
and conformance rules. If it starts with / and follows the spec, the
parser handles it. If it doesn't, it's classified as text. No gray area.

### Card 2: Total Function
parse_document accepts any UTF-8 string and always returns a ParseResult.
There is no error type. There is no Result<_, E>. Malformed input
produces partial results and warnings, never a failure. The engine uses
no randomness, no floating point, no HashMap iteration. Identical input
produces identical output across invocations and across implementations.

### Card 3: Tested Relentlessly
Three layers of tests: unit tests at every module, property-based testing
with proptest generating random inputs against structural invariants, and
fuzz testing with libFuzzer feeding arbitrary bytes to assert the engine
never panics. Proptest regressions and fuzz crash inputs are committed as
permanent regression tests. The corpus only grows.

### Card 4: SDKs (Coming Soon)
The engine compiles to three targets: a Rust crate for native integration,
a WASM module with TypeScript declarations for JavaScript, and a WASI
binary for shell pipelines. Same spec. Same JSON envelope. Every target.


## SPEC PAGE INTROS

### Syntax RFC (/spec/syntax/)
#### Page Title: Slash Command Syntax v1.1.0

#### Intro Paragraph
This document defines how a conforming parser partitions UTF-8 text into
an ordered sequence of commands and text blocks. It specifies line
normalization, backslash continuation, command detection, single-line and
fenced multi-line argument modes, and document partitioning. The
specification is format-agnostic: it defines observable behavior without
prescribing architecture, serialization, or host bindings.

### Engine Spec (/spec/engine/)
#### Page Title: Parser Engine v0.5.0

#### Intro Paragraph
The engine is a Rust library crate that implements every syntax rule
defined in the Slash Command Syntax v1.1.0. It consumes a UTF-8 string
and produces Rust domain types through a single public function:
parse_document. The engine performs no IO, carries no serialization
dependencies, uses no unsafe code, and maintains no global state. It is
designed to be wrapped by SDKs that handle JSON serialization, WASM
bindings, and WASI runtime integration.


## EXAMPLES PAGE (/examples/)

### Page Title: Set List

### Intro
Each example shows input text and the resulting parse output. These are
drawn from the specification's Appendix B. They cover single-line
commands, multi-line joining, fenced payloads, text block partitioning,
invalid slash lines, and edge cases like unclosed fences.


## SOUNDCHECK PAGE (/soundcheck/)

### Page Title: Soundcheck

### Opening Paragraph
The parser is a total function. It accepts any UTF-8 input and always
returns a valid result. It cannot panic, cannot return an error, and
cannot fail. The testing strategy is designed to prove that guarantee
holds under adversarial conditions.

### Section: Three-Layer Test Architecture
Tests are organized by scope. Layer 1: unit tests sit inside every module
file, exercising individual functions with direct calls. Layer 2:
integration tests span multiple modules within the application layer,
exercising cross-module composition through parse_document. Layer 3:
cross-layer tests cover the full public API, including the Appendix B
parsing examples from the specification.

### Section: Property-Based Testing
Randomly generated inputs validate structural invariants that must hold
for all possible documents: roundtrip fidelity, deterministic output, ID
sequencing, and line-range consistency. The engine uses proptest to
generate inputs at scale. When proptest finds a failing case, it
automatically shrinks it to the minimal reproducer and writes it to a
regression file. These files are committed to version control. Once a
failure class is found, it can never silently return.

### Section: Fuzz Testing
A cargo-fuzz harness feeds arbitrary bytes to parse_document and asserts
the engine never panics. Crash inputs are automatically minimized and
added as permanent regression tests. The fuzz corpus grows monotonically:
every interesting input discovered during a run is preserved for future
sessions. The parser's total function guarantee means the fuzz harness is
simple: if it doesn't panic, it passes.

### Section: What We Prove
- Deterministic: identical input always produces identical output. No
  randomness, no HashMap iteration order, no floating point arithmetic.
- Total: every input produces a valid ParseResult. There is no error path.
- Safe: no unsafe code anywhere in the engine.
- Faithful: parse, format, parse again yields a structurally equivalent
  result (the roundtrip fidelity invariant).
- Opaque: the parser never interprets argument content. Your payload is
  your business.


## 404 PAGE

### Headline
404

### Body
This page doesn't exist. The parser would classify it as text.

### CTA
Back to the set list


## FOOTER

### Left
Solidus. The gold standard for slash command parsing.

### Right
Named for U+002F SOLIDUS, the forward slash,
itself named for the Roman gold coin.


## META / SEO

### Site Title
Solidus: The Gold Standard for Slash Command Parsing

### Meta Description (155 chars max)
Solidus is a formally specified slash command parser. Pure Rust engine
with ABNF grammar, property testing, and fuzz testing. Spec-first. Always.

### OpenGraph Title
Solidus

### OpenGraph Description
The gold standard for slash command parsing. A formally specified, pure
Rust parser for /command syntax in UTF-8 text.


## README OPENER (for the GitHub repo)

# Solidus

**The gold standard for slash command parsing.**

Solidus parses `/commands` in UTF-8 text. It implements the
[Slash Command Syntax v1.1.0](link) specification with a pure Rust
engine: no IO, no serialization, no unsafe, no global state.
`parse_document` is a total function. It accepts any input and always
returns a valid result.

Run the CLI with `riff`. Import the engine in Rust. Load the WASM
module in JavaScript. Same spec, same output, every time.


## SIDEBAR LABELS

- Overview
- Set List (examples)
- The Spec
  - Syntax v1.1.0
  - Engine v0.5.0
- Soundcheck (testing)
- SDKs (badged "Coming Soon")
  - Rust
  - WASM / JavaScript
  - WASI


## SHORT-FORM DESCRIPTIONS (for different contexts)

### One Line (repo subtitle, crate description)
The gold standard for slash command parsing.

### Two Lines (npm package, README badge area)
A formally specified slash command parser for UTF-8 text.
Pure Rust. Total function. Spec-first.

### One Paragraph (blog post, announcement, Show HN)
Solidus is a formally specified parser for /command syntax. The Slash
Command Syntax v1.1.0 defines how commands and text blocks are
partitioned in UTF-8 text, with ABNF grammar and RFC-style conformance
rules. The engine is a pure Rust library: one entry point
(parse_document), zero failure modes, no IO, no unsafe, no global state.
Tested with property-based testing and fuzz testing. SDKs for Rust,
WASM/JavaScript, and WASI are in development.
