pub const SPEC_VERSION: &str = "0.5.0";

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
    pub id: String,
    pub name: String,
    pub raw: String,
    pub range: LineRange,
    pub arguments: CommandArguments,
}

/// A contiguous block of non-command text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    pub id: String,
    pub range: LineRange,
    pub content: String,
}

/// Top-level parse result.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub version: String,
    pub commands: Vec<Command>,
    pub textblocks: Vec<TextBlock>,
    pub warnings: Vec<Warning>,
}

/// Non-fatal conditions detected during parsing.
///
/// Warnings are collected in `ParseResult.warnings` rather than
/// causing the parse to fail. The parser is intentionally permissive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub wtype: String,
    pub start_line: Option<usize>,
    pub message: Option<String>,
}
