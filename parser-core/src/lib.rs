pub mod domain;
pub mod parser;
pub mod serialize;
pub mod to_plaintext;

pub use domain::{
    ArgumentMode, Command, CommandArguments, CommandRange, ParseError, ParserContext, SlashParseResult,
    TextBlock,
};
pub use parser::{parse_slash_commands, parse_to_domain};
