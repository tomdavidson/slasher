//! Layer 2 integration tests: cross-module composition within the application layer.
//!
//! These tests exercise combinations of modules that no single file's tests can cover.
//! They use only the public API of each module.

mod normalize_join;
mod normalize_join_classify;
mod spec_examples;
