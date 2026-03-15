use std::collections::HashMap;

use super::errors::ParseWarning;

/// Inclusive line range (zero-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRange {
    pub start_line: usize,
    pub end_line: usize,
}

/// How the argument payload was assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentMode {
    SingleLine,
    Continuation,
    Fence,
}

/// Parsed arguments for a single command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandArguments {
    pub header: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
    pub payload: String,
}

/// A single parsed slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub raw: String,
    pub range: LineRange,
    pub arguments: CommandArguments,
}

/// A contiguous block of non-command text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    pub range: LineRange,
    pub content: String,
}

/// Metadata provided by the caller, merged into the output envelope.
///
/// `extra` holds additional key-value pairs beyond the known fields.
/// Values are strings; richer types are a serialization concern
/// handled at the application boundary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParserContext {
    pub source: Option<String>,
    pub timestamp: Option<String>,
    pub user: Option<String>,
    pub session_id: Option<String>,
    pub extra: HashMap<String, String>,
}

/// Top-level parse result.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub version: String,
    pub context: ParserContext,
    pub commands: Vec<Command>,
    pub text_blocks: Vec<TextBlock>,
    pub warnings: Vec<ParseWarning>,
}

pub const SPEC_VERSION: &str = "v1";
