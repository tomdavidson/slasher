/// Non-fatal conditions detected during parsing.
///
/// Warnings are collected in `ParseResult.warnings` rather than
/// causing the parse to fail. The parser is intentionally permissive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseWarning {
    UnclosedFence { start_line: usize },
    UnclosedContinuation { start_line: usize },
}
