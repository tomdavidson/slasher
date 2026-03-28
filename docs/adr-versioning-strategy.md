---
status: accepted
date: 2026-03-21
decision-makers: Tom Davidson
---

# Versioning Strategy for Slash Command Parser Document Suite and Crates

## Context and Problem Statement

The Slash Command Parser project has three specification documents and (will have) three or more Rust crates. Each artifact evolves at a different rate and serves a different audience. How should versions be assigned across these artifacts, and what is the starting version for each?

Specific questions:

1. What version does each artifact start at?
2. Do crate versions track their spec, or are they independent?
3. What goes in the JSON envelope's "version" field?
4. How do consumers know which spec a given output conforms to?

## Decision Drivers

* Consumers need to validate output against a specific schema version
* The engine crate has existed since v0.1.0 and is currently at v0.3.0; restarting at 0.1.0 would be confusing
* The SDK and WASM/WASI crates do not exist yet; they start fresh
* Semantic versioning (SemVer) is required for crates.io publication
* The syntax RFC is the only document consumers interact with directly (via the JSON schema)
* Specs and crates change at different rates; a whitespace-only spec fix should not force a crate release

## Considered Options

* Option 1: Unified version (all artifacts share one version number)
* Option 2: Independent versions per artifact with explicit mapping table
* Option 3: Spec-tracks-crate (each crate version matches its spec)

## Decision Outcome

Chosen option: "Option 2: Independent versions per artifact with explicit mapping table", because it is the only option that allows each artifact to evolve at its own pace while maintaining a clear contract for consumers.

### Version Assignments

| Artifact          | Type          | Starting Version | Versioning Scheme     |
|-------------------|---------------|------------------|-----------------------|
| Syntax RFC        | Specification | 0.3.1            | Document version      |
| Engine Spec       | Specification | 0.4.0            | Document version      |
| SDK Spec          | Specification | 0.1.0            | Document version      |
| parser (crate)    | Rust crate    | 0.4.0            | SemVer, workspace     |
| parser-sdk (crate)| Rust crate    | 0.1.0            | SemVer, workspace     |
| parser-wasm       | Rust crate    | 0.1.0            | SemVer, workspace     |
| parser-wasi       | Rust crate    | 0.1.0            | SemVer, workspace     |

### Rationale for Starting Versions

**Syntax RFC at 0.3.1.** The syntax rules evolved through 0.1.0 and 0.2.0 in prior implementation specs. The v0.3.0 implementation spec was the first to fully codify the two-state parser, fence semantics, and backslash joining. The syntax RFC extracts and corrects those rules, making it 0.3.1 (a patch on the 0.3.x line). Going to 0.4.0 would overstate the delta; going back to 0.1.0 would discard history.

**Engine Spec at 0.4.0.** The engine spec replaces the v0.3.0 implementation spec. It incorporates four breaking changes (true POSIX joining, whitespace redefinition, warning type rename, version bump), warranting a minor version bump to 0.4.0.

**parser crate at 0.4.0.** The crate version tracks the engine spec version at this point because the crate and the spec are 1:1. The crate was at 0.3.0; the breaking changes bump it to 0.4.0. If the crate and spec later diverge in cadence, they may drift, but they start aligned.

**SDK artifacts at 0.1.0.** These are new. SemVer starts at 0.1.0 for initial development.

### The "version" Field in JSON Output

The "version" field in the serialized JSON envelope always contains the Syntax RFC version, currently "0.3.1". This is the contract consumers validate against. The SDK is responsible for this mapping.

The engine's SPEC_VERSION constant is "0.4.0" (matching the engine spec). The SDK maps this to the syntax RFC version at serialization time.

### Mapping Table

The SDK maintains a mapping from engine version to syntax RFC version:

| Engine SPEC_VERSION | Syntax RFC Version | JSON "version" Field |
|---------------------|--------------------|----------------------|
| "0.4.0"             | "0.3.1"            | "0.3.1"              |

When the syntax RFC is updated (e.g., to 0.3.2 or 0.4.0), the SDK mapping is updated and the SDK crate gets a version bump.

### Consequences

* Good, because each artifact can release independently without forcing cascading version bumps.
* Good, because consumers have a single, stable version identifier (the syntax RFC version) to validate against.
* Good, because the engine crate version history is continuous (0.1.0 -> 0.2.0 -> 0.3.0 -> 0.4.0), not restarted.
* Good, because new SDK crates start at 0.1.0 per SemVer convention for initial development.
* Neutral, because the mapping table adds a small maintenance burden in the SDK.
* Bad, because three different version numbers for "the parser" can confuse newcomers. Mitigated by documenting the mapping in the SDK README.

### Confirmation

Confirmed by checking:

1. The parser crate's Cargo.toml version is "0.4.0".
2. The SPEC_VERSION constant in domain/types.rs is "0.4.0".
3. The SDK's envelope serialization produces "version": "0.3.1".
4. The JSON output validates against the 0.3.1 schema.

## Pros and Cons of the Options

### Option 1: Unified version

All artifacts share one version number. When any artifact changes, all bump.

* Good, because there is only one version to remember.
* Bad, because a typo fix in the SDK spec forces a version bump on the syntax RFC and engine crate.
* Bad, because crate versions on crates.io would include meaningless bumps (no code change, just a version increment).
* Bad, because the engine crate would jump from 0.3.0 to some arbitrary unified number, breaking the continuous history.

### Option 2: Independent versions with mapping table

Each artifact has its own version. The SDK maintains a mapping from engine version to syntax RFC version.

* Good, because each artifact evolves at its own pace.
* Good, because the JSON "version" field is always the syntax RFC version, giving consumers a stable contract.
* Good, because the engine crate history is continuous.
* Neutral, because the mapping table must be maintained.
* Bad, because newcomers see three version numbers and need to understand the relationship.

### Option 3: Spec-tracks-crate

Each crate version is forced to match its corresponding spec version.

* Good, because crate version == spec version, reducing confusion.
* Bad, because a crate bugfix (no spec change) would require a spec version bump, or the crate couldn't publish a patch.
* Bad, because SemVer semantics on crates.io would be violated (a spec-only change with no code change would still bump the crate version).
* Bad, because SDK crate and SDK spec would need lockstep releases even when only one changed.

## More Information

This ADR should be revisited when:

* The project reaches 1.0.0 stability for any artifact.
* A second language implementation of the engine is created (the syntax RFC version becomes the cross-implementation contract).
* The mapping table grows beyond a simple 1:1 relationship.

Related documents:

* Syntax RFC v0.3.1: slash-parser-rfc-v0.3.1.txt
* Engine Spec v0.4.0: slash-parser-engine-spec-v0.4.0.txt
* SDK Spec v0.1.0: slash-parser-sdk-spec-v0.1.0.txt
