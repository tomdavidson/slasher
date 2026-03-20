use crate::{
    domain::{
        ArgumentMode, Command, CommandArguments, CommandRange, ParseError, ParserContext, SlashParseResult,
        TextBlock,
    },
    serialize,
};

/// Parser states — flat, no recursion.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Idle,
    Accumulating,
    InFence { marker_len: usize },
}

/// In-progress command being built.
struct CommandBuilder {
    name: String,
    header: String,
    raw_lines: Vec<String>,
    payload: String,
    mode: ArgumentMode,
    fence_lang: Option<String>,
    start_line: usize,
}

impl CommandBuilder {
    fn new(name: String, header: String, start_line: usize) -> Self {
        Self {
            name,
            header,
            raw_lines: Vec::new(),
            payload: String::new(),
            mode: ArgumentMode::SingleLine,
            fence_lang: None,
            start_line,
        }
    }

    fn finalize(self, end_line: usize, command_index: usize) -> Command {
        Command {
            id: format!("cmd-{command_index}"),
            name: self.name,
            raw: self.raw_lines.join("\n"),
            range: CommandRange { start_line: self.start_line, end_line },
            arguments: CommandArguments {
                header: self.header,
                mode: self.mode,
                fence_lang: self.fence_lang,
                payload: self.payload,
            },
        }
    }
}

/// Parse a command name from the text after the leading `/`.
/// Returns `(name, rest_of_line)` or `None` if no valid name.
fn parse_command_name(after_slash: &str) -> Option<(&str, &str)> {
    let bytes = after_slash.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return None;
    }
    let end = bytes
        .iter()
        .position(|&b| !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        .unwrap_or(bytes.len());

    // Name must start with lowercase letter — already checked above.
    let name = &after_slash[..end];
    let rest = &after_slash[end..];
    Some((name, rest))
}

/// Check if a line ends with the continuation marker ` /`.
fn has_continuation(line: &str) -> bool {
    line.ends_with(" /")
}

/// Strip the trailing ` /` continuation marker.
fn strip_continuation(line: &str) -> &str {
    if line.len() < 2 { line } else { &line[..line.len() - 2] }
}

/// Detect a fence opener (three or more backticks) in text.
/// Returns `(before_fence, marker_len, lang)` or `None`.
fn detect_fence_opener(text: &str) -> Option<(&str, usize, Option<&str>)> {
    let start = text.find("```")?;
    let before = &text[..start];
    let backtick_region = &text[start..];
    let marker_len = backtick_region.bytes().take_while(|&b| b == b'`').count();
    let after_backticks = &backtick_region[marker_len..];

    // Language identifier: up to first whitespace or end of string.
    let lang = after_backticks.split_whitespace().next();
    let lang = lang.filter(|l| !l.is_empty());

    Some((before, marker_len, lang))
}

/// Check if a line is a closing fence for the given marker length.
fn is_closing_fence(line: &str, marker_len: usize) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < marker_len {
        return false;
    }
    let backtick_count = trimmed.bytes().take_while(|&b| b == b'`').count();
    backtick_count >= marker_len && trimmed.len() == backtick_count
}

/// Detect if a line is a command line (first non-whitespace is `/`).
/// Returns the content after leading whitespace if it starts with `/`.
fn detect_command_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('/') { Some(trimmed) } else { None }
}

/// Parse slash commands from input text and return a JSON string.
///
/// Input: UTF-8 text. `\r\n` is normalized to `\n`.
/// Output: `Result<String, ParseError>` where the string is JSON
/// conforming to the `SlashParseResult` schema.
pub fn parse_slash_commands(input: &str, context: ParserContext) -> Result<String, ParseError> {
    let result = parse_to_domain(input, context);
    serialize::to_json(&result).map_err(|err| ParseError::SerializationError { message: err.to_string() })
}

/// Parse slash commands into domain types.
///
/// Returns structured domain types rather than a JSON string. Used by
/// consumers that want to work with typed data (e.g. the CLI).
pub fn parse_to_domain(input: &str, context: ParserContext) -> SlashParseResult {
    let normalized = input.replace("\r\n", "\n").replace("\r", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    let mut state = State::Idle;
    let mut commands: Vec<Command> = Vec::new();
    let mut text_blocks: Vec<TextBlock> = Vec::new();
    let mut current: Option<CommandBuilder> = None;

    // Track contiguous non-command text.
    let mut text_start: Option<usize> = None;
    let mut text_content = String::new();

    let mut line_index = 0;
    while line_index < lines.len() {
        let line = lines[line_index];

        match &state {
            State::Idle => {
                if let Some(trimmed) = detect_command_line(line) {
                    // Flush any pending text block.
                    if let Some(start) = text_start.take() {
                        // Remove trailing newline from text content.
                        if text_content.ends_with('\n') {
                            text_content.pop();
                        }
                        text_blocks.push(TextBlock {
                            id: format!("text-{}", text_blocks.len()),
                            range: CommandRange { start_line: start, end_line: line_index.saturating_sub(1) },
                            content: std::mem::take(&mut text_content),
                        });
                    }

                    // Parse command name from after the `/`.
                    let after_slash = &trimmed[1..];
                    if let Some((name, rest)) = parse_command_name(after_slash) {
                        let header = if rest.starts_with(char::is_whitespace) {
                            rest.trim_start().to_string()
                        } else {
                            String::new()
                        };

                        let mut builder = CommandBuilder::new(name.to_string(), header.clone(), line_index);
                        builder.raw_lines.push(trimmed.to_string());

                        // Check for inline fence opener in the header.
                        if let Some((before, marker_len, lang)) = detect_fence_opener(&header) {
                            builder.header = before.trim_end().to_string();
                            builder.fence_lang = lang.map(ToString::to_string);
                            builder.mode = ArgumentMode::Fence;
                            current = Some(builder);
                            state = State::InFence { marker_len };
                        } else if has_continuation(trimmed) {
                            // Full trimmed line has continuation marker; enter continuation mode.
                            // Compute the portion after `/name` as the first payload line.
                            let after_name = &trimmed[1 + name.len()..];
                            let body_with_marker = after_name.trim_start();
                            let body = strip_continuation(body_with_marker);
                            builder.payload.push_str(body);
                            builder.payload.push('\n');
                            builder.mode = ArgumentMode::Continuation;
                            current = Some(builder);
                            state = State::Accumulating;
                        } else {
                            // Single-line command.
                            builder.payload = header;
                            builder.mode = ArgumentMode::SingleLine;
                            commands.push(builder.finalize(line_index, commands.len()));
                        }
                    } else {
                        // `/` followed by invalid name — treat as text.
                        if text_start.is_none() {
                            text_start = Some(line_index);
                        }
                        text_content.push_str(line);
                        text_content.push('\n');
                    }
                } else {
                    // Non-command line in idle — accumulate as text.
                    if text_start.is_none() {
                        text_start = Some(line_index);
                    }
                    text_content.push_str(line);
                    text_content.push('\n');
                }
            }

            State::Accumulating => {
                if let Some(builder) = current.as_mut() {
                    builder.raw_lines.push(line.to_string());

                    // Check if this line is a fence opener.
                    if let Some((_, marker_len, lang)) = detect_fence_opener(line) {
                        builder.fence_lang = lang.map(ToString::to_string);
                        builder.mode = ArgumentMode::Fence;
                        state = State::InFence { marker_len };
                    } else if has_continuation(line) {
                        // Continuation marker line contributes a (possibly empty) payload line.
                        let stripped = strip_continuation(line);
                        builder.payload.push_str(stripped);
                        builder.payload.push('\n');
                        // Stay in Accumulating.
                    } else if line.is_empty() {
                        // True blank line ends continuation without being added to payload.
                        let cmd =
                            current.take().unwrap().finalize(line_index.saturating_sub(1), commands.len());
                        commands.push(cmd);
                        state = State::Idle;
                    } else {
                        // Normal continuation line — append and keep accumulating; EOF will finalize.
                        builder.payload.push_str(line);
                        builder.payload.push('\n');
                    }
                }
            }

            State::InFence { marker_len } => {
                let marker_len = *marker_len;
                if let Some(builder) = current.as_mut() {
                    builder.raw_lines.push(line.to_string());

                    if is_closing_fence(line, marker_len) {
                        // Closing fence — finalize command.
                        let cmd = current.take().unwrap().finalize(line_index, commands.len());
                        commands.push(cmd);
                        state = State::Idle;
                    } else {
                        // Inside fence — append verbatim.
                        builder.payload.push_str(line);
                        builder.payload.push('\n');
                    }
                }
            }
        }

        line_index += 1;
    }

    // Finalize any in-progress command (unclosed fence or continuation at EOF).
    if let Some(builder) = current.take() {
        let end = lines.len().saturating_sub(1);
        commands.push(builder.finalize(end, commands.len()));
    }

    // Flush any trailing text block.
    if let Some(start) = text_start.take() {
        if text_content.ends_with('\n') {
            text_content.pop();
        }
        text_blocks.push(TextBlock {
            id: format!("text-{}", text_blocks.len()),
            range: CommandRange { start_line: start, end_line: lines.len().saturating_sub(1) },
            content: text_content,
        });
    }

    SlashParseResult { version: "0.1.0".to_string(), context, commands, text_blocks }
}

#[cfg(test)]
mod tests;
