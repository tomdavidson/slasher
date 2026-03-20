---
number: 6
title: Repository Structure and Project Boundaries
date: 2026-03-14
status: accepted
---

# 6. Repository Structure and Project Boundaries

Date: 2026-03-14

## Status

Accepted

## Context

The slash project encompasses a Rust parser core, WASM build targets, language-specific SDKs, a CLI
tool (riff), distribution packaging, a documentation website, and a formal spec.

Deep directory nesting (e.g., `core/parser/`, `sdk/slash-python/`, `cli/riff/`) adds navigation
friction. A flat layout with a strict naming convention communicates category membership and
dependency tiers without requiring a nested filesystem hierarchy.

Naming: "slash" is the format and project name (80s rock reference). "riff" is the CLI tool (guitar
riff; it reads stdin, writes stdout, stays simple). All SDK packages are prefixed `slash-` for
discoverability in package registries. All CLI packaging projects are prefixed `riff-`.

Because this is a polyglot monorepo, we need strict architectural constraints to ensure SDKs don't
accidentally import the CLI, and that WASM builds don't depend on downstream consumers. The
mechanisms for enforcing these constraints (e.g., build system tooling) will be handled in a
separate tooling ADR.

## Decision

The repository uses a flat, single-level project layout. Logical grouping and dependency enforcement
are handled via project metadata and tags, not directory depth.

### Project Layout

```text
wit/                 core    WIT interface definitions (single source of truth)
parser/              core    Rust library, internal to repo, not published
wasm-javascript/     wasm    wasm-bindgen module for JS/TS runtimes
wasm-wasi/           wasm    WASI component for polyglot SDK consumption
slash-web/           sdk     Browser runtime SDK (depends on wasm-javascript)
slash-javascript/    sdk     ESM server-side SDK (depends on wasm-javascript)
slash-python/        sdk     Python SDK (depends on wasm-wasi)
slash-ruby/          sdk     Ruby SDK (depends on wasm-wasi)
slash-php/           sdk     PHP SDK (depends on wasm-wasi)
slash-elixir/        sdk     Elixir SDK (depends on wasm-wasi)
slash-ocaml/         sdk     OCaml SDK (depends on wasm-wasi)
slash-haskell/       sdk     Haskell SDK (depends on wasm-wasi)
slash-dart/          sdk     Dart SDK (depends on wasm-wasi)
slash-java/          sdk     Java SDK (depends on wasm-wasi)
slash-go/            sdk     Go SDK (depends on wasm-wasi)
slash-zig/           sdk     Zig SDK (native FFI or wasm-wasi)
slash-rust/          sdk     Rust SDK, thin published crate wrapping parser
riff-cli/            cli     CLI binary (depends on slash-rust)
riff-deb/            pkg     Debian package
riff-rpm/            pkg     RPM package
riff-oci/            pkg     OCI container image
riff-proto/          pkg     Proto toolchain plugin
website/             docs    Static site for documentation and promotion
docs/                docs    Formal specifications
docs/adrs/           docs    Architecture Decision Records
```

### Module Boundaries

Projects are grouped into six distinct tags/tiers to define architectural boundaries. Build tooling
must enforce that dependencies only flow downward through these layers:

- `core`: the internal parser library and WIT definitions. Depends on nothing.
- `wasm`: WASM build targets that compile `parser`. Can only depend on `core`.
- `sdk`: language-specific packages wrapping a WASM module (or native FFI). Can only depend on
  `wasm` and `core`.
- `cli`: the riff binary. Can only depend on `sdk` and `core`.
- `pkg`: distribution packaging for riff. Can only depend on `cli`.
- `docs`: website, specifications, and ADRs. Can only depend on `docs`.

### Dependency Flow and The WIT Contract

```text
wit    -> parser
parser -> wasm-javascript -> slash-javascript, slash-web
parser -> wasm-wasi       -> slash-python, slash-ruby, slash-php, ...
parser -> slash-rust      -> riff-cli -> riff-deb, riff-rpm, riff-oci, riff-proto
docs   -> website
```

The `wit/` directory contains the WebAssembly Interface Types definitions, acting as the single
source of truth for the parser's API surface. Language SDKs generate types and thin wrappers based
on this contract.

### Internal vs Published Code

`parser/` is an internal Rust crate. It is consumed only by `wasm-javascript`, `wasm-wasi`, and
`slash-rust` within this repo. External Rust consumers use the `slash-rust` crate, which provides
the stable public API.

To keep the repository root clean, all fuzzing, profiling, and testing infrastructure for the core
engine must live entirely within the `parser/` directory, avoiding root-level artifact clutter.

## Consequences

- Every project is one `cd` from root. No navigating through intermediate grouping directories.
- Prefix conventions (`slash-*`, `riff-*`, `wasm-*`) make filesystem navigation and glob-based
  targeting self-documenting.
- The `parser` crate stays internal, giving freedom to change its API without semver concerns. Only
  `slash-rust` carries the public contract.
- Adding a new language SDK requires only a single new root directory; no restructuring is needed.
- Clear structural boundaries physically prevent architectural regressions (e.g., an SDK cannot
  accidentally depend on the CLI).
- The specification and ADRs live in `docs/` alongside the code. The website project pulls from
  `docs/` at build time, ensuring documentation is version-controlled with the codebase.
