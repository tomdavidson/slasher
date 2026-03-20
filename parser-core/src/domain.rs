//! Domain types for the slash command parser.
//!
//! These types are pure data — no serde derives. Serialization
//! happens in the `serialize` module via DTO conversion.

/// Inclusive line range (0-based).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct CommandRange {
    pub start_line: usize,
    pub end_line: usize,
}

/// How the argument payload was assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum ArgumentMode {
    SingleLine,
    Continuation,
    Fence,
}

/// Parsed arguments for a single command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct CommandArguments {
    pub header: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
    pub payload: String,
}

/// A single parsed slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Command {
    pub id: String,
    #[cfg_attr(test, proptest(regex = "[a-z][a-z0-9-]*"))]
    pub name: String,
    pub raw: String,
    pub range: CommandRange,
    pub arguments: CommandArguments,
}

/// A contiguous block of non-command text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct TextBlock {
    pub id: String,
    pub range: CommandRange,
    pub content: String,
}

/// Optional context passed by the caller.
#[derive(Debug, Clone, Default)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct ParserContext {
    pub source: Option<String>,
    pub timestamp: Option<String>,
    pub user: Option<String>,
    pub session_id: Option<String>,
    #[cfg_attr(test, proptest(value = "None"))]
    pub extra: Option<serde_json::Value>,
}

/// Top-level parse result.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct SlashParseResult {
    pub version: String,
    pub context: ParserContext,
    pub commands: Vec<Command>,
    pub text_blocks: Vec<TextBlock>,
}

/// Typed parse errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("serialization failed: {message}")]
    SerializationError { message: String },
}
