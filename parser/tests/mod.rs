// use super::*;
// use crate::domain::{ArgumentMode, ParserContext};

// mod proptest;

// // --- Test helpers ---

// fn default_context() -> ParserContext { ParserContext::default() }

// fn parse(input: &str) -> SlashParseResult { parse_to_domain(input, default_context()) }

// fn assert_single_command(input: &str) -> Command {
//     let result = parse(input);
//     assert_eq!(result.commands.len(), 1, "expected exactly 1 command");
//     result.commands.into_iter().next().unwrap()
// }

// fn assert_no_commands(input: &str) {
//     let result = parse(input);
//     assert!(result.commands.is_empty(), "expected no commands, got {}", result.commands.len());
// }
