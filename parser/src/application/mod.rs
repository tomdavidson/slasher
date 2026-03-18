mod command_accumulate;
mod command_finalize;
mod document_parse;
mod line_classify;
mod text_collect;

pub use document_parse::parse_document;

#[cfg(test)]
mod tests;
