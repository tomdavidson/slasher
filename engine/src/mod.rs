mod classify;
mod fence;
mod join;
mod normalize;
mod parse;
mod single_line;
mod text;

pub use parse::parse_document;

#[cfg(test)]
pub mod tests;
