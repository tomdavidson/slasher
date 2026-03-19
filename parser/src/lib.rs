mod application;
mod domain;

// Domain types
pub use domain::{
    ArgumentMode, Command, CommandArguments, LineRange, ParseResult, SPEC_VERSION, TextBlock,
    Warning,
};

// Engine entry point
pub use application::parse_document;
