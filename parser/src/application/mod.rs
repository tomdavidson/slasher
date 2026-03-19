// src/application/mod.rs
mod command_accumulate;
mod command_finalize;
mod document_parse;
mod line_classify;
mod line_join;
mod normalize;
mod text_collect;

pub use document_parse::parse_document;

#[cfg(test)]
pub mod tests;
