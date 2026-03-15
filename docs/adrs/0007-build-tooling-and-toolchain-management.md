---
number: 7
title: Build Tooling and Toolchain Management
date: 2026-03-14
status: accepted
---

# 7. Build Tooling and Toolchain Management

Date: 2026-03-14

## Status

Accepted

## Context

The slash project is a polyglot monorepo containing Rust (core parser), WebAssembly components, and
multiple language-specific SDKs. Managing this requires a build system that can orchestrate a
directed acyclic graph (DAG) of tasks across different language ecosystems. We also need tooling to
physically enforce the architectural boundaries established in ADR 6, both between projects (macro)
and within projects (micro).

Furthermore, we must prevent "works on my machine" issues by guaranteeing that all contributors and
CI runners use the exact same toolchain versions, while preventing our monorepo orchestrator from
getting into a tug-of-war with workspace-aware tools like Cargo and pnpm.

## Decision

We will use Moon as our primary task orchestrator and monorepo manager, alongside its companion tool
`proto` for toolchain management. Moon will wrap, rather than replace, the native tools for each
language ecosystem.

### Boundary Enforcement (Macro vs Micro)

Tooling will explicitly enforce the architectural constraints defined in ADR 6 at two levels:

1. **Macro-Boundaries (Project Level):** We use Moon's `tagRelationships` in `.moon/workspace.yml`.
   Projects are tagged (`core`, `wasm`, `sdk`, `cli`, `pkg`, `docs`). If an `sdk` project attempts
   to depend on a `cli` project, Moon will fail the build graph generation. This physically prevents
   architectural regressions across the monorepo.

2. **Micro-Boundaries (File Level):** Within individual projects (like the `parser/` crate), we use
   ecosystem-specific tools configured as Moon lint tasks to enforce Clean Architecture:
   - **Rust:** We use `clippy.toml` restrictions (`disallowed-methods`, `disallowed-types`) to
     prevent Infrastructure IO and WASM types from leaking into the `src/domain/` and
     `src/application/` modules. We may adopt `tach` in the future for stricter folder-to-folder DAG
     enforcement.
   - **TypeScript:** We use `eslint-plugin-boundaries` to prevent illegal cross-folder imports.

### Toolchain Pinning (Proto)

All toolchain versions are strictly pinned at the repository root:

- `.prototools` acts as the single source of truth for exact version numbers (`rust = "1.85.0"`).
- `.moon/toolchains.yml` handles configuration behavior (e.g., auto-installing the `wasm32-wasip1`
  target) but explicitly omits version numbers to prevent configuration drift.

### Virtual Workspaces and Task Scoping

To support shared lockfiles and efficient caching, we use root-level virtual workspaces (a root
`Cargo.toml` and `pnpm-workspace.yaml`).

**The Task Scoping Rule:** To prevent Cargo and pnpm from attempting to build the entire monorepo
when invoked inside a specific project, all inherited Moon tasks must use scope flags injected with
Moon's `$project` variable (e.g., `cargo build -p $project` instead of bare `cargo build`).

### Editor Configuration & Validation

All Moon configuration files will utilize remote JSON `$schema` headers. To prevent the VS Code YAML
extension from reporting false-positive validation errors, contributors must use the
`moon: Append YAML schemas configuration to settings` command provided by the official Moon VS Code
extension.

## Consequences

- Complete reproducibility: anyone cloning the repo will automatically download the correct
  compilers and toolchains via `proto`.
- Clean Architecture is structurally enforced: Moon tags prevent illegal cross-project imports, and
  Clippy/ESLint prevent illegal intra-project imports.
- By using `$project` variables with scope flags (`-p`), Moon and Cargo cooperate peacefully rather
  than fighting over workspace control.
- The use of `$schema` headers combined with the VS Code extension provides immediate IDE validation
  and prevents silent configuration errors.
