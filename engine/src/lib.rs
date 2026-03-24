mod classify;
mod fence;
mod join;
mod normalize;
mod parse;
mod single_line;
mod text;
mod types;

// Public API
pub use parse::parse_document;
pub use types::{
    ArgumentMode, Command, CommandArguments, LineRange, ParseResult, SPEC_VERSION, TextBlock, Warning,
};

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod test_helper;
