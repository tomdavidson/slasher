/// Serialization module — converts domain types to JSON via DTOs.
///
/// Serde derives live here, not on domain types. This module is the
/// boundary between the pure domain and the JSON output contract.
use serde::Serialize;

use crate::domain::{
    ArgumentMode, Command, CommandArguments, CommandRange, ParserContext, SlashParseResult, TextBlock,
};

#[derive(Serialize)]
struct SlashParseResultDto {
    version: String,
    context: ContextDto,
    commands: Vec<CommandDto>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    text_blocks: Vec<TextBlockDto>,
}

#[derive(Serialize)]
struct ContextDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct CommandDto {
    id: String,
    name: String,
    raw: String,
    range: RangeDto,
    arguments: ArgumentsDto,
    children: Vec<CommandDto>,
}

#[derive(Serialize)]
struct RangeDto {
    start_line: usize,
    end_line: usize,
}

#[derive(Serialize)]
struct ArgumentsDto {
    #[serde(skip_serializing_if = "String::is_empty")]
    header: String,
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fence_lang: Option<String>,
    payload: String,
}

#[derive(Serialize)]
struct TextBlockDto {
    id: String,
    range: RangeDto,
    content: String,
}

// --- From impls: domain → DTO ---

impl From<&CommandRange> for RangeDto {
    fn from(range: &CommandRange) -> Self {
        Self { start_line: range.start_line, end_line: range.end_line }
    }
}

impl From<&ArgumentMode> for &'static str {
    fn from(mode: &ArgumentMode) -> Self {
        match mode {
            ArgumentMode::SingleLine => "single-line",
            ArgumentMode::Continuation => "continuation",
            ArgumentMode::Fence => "fence",
        }
    }
}

impl From<&CommandArguments> for ArgumentsDto {
    fn from(args: &CommandArguments) -> Self {
        Self {
            header: args.header.clone(),
            mode: <&str>::from(&args.mode).to_string(),
            fence_lang: args.fence_lang.clone(),
            payload: args.payload.clone(),
        }
    }
}

impl From<&Command> for CommandDto {
    fn from(cmd: &Command) -> Self {
        Self {
            id: cmd.id.clone(),
            name: cmd.name.clone(),
            raw: cmd.raw.clone(),
            range: RangeDto::from(&cmd.range),
            arguments: ArgumentsDto::from(&cmd.arguments),
            children: Vec::new(),
        }
    }
}

impl From<&TextBlock> for TextBlockDto {
    fn from(block: &TextBlock) -> Self {
        Self { id: block.id.clone(), range: RangeDto::from(&block.range), content: block.content.clone() }
    }
}

impl From<&ParserContext> for ContextDto {
    fn from(ctx: &ParserContext) -> Self {
        Self {
            source: ctx.source.clone(),
            timestamp: ctx.timestamp.clone(),
            user: ctx.user.clone(),
            session_id: ctx.session_id.clone(),
            extra: ctx.extra.clone(),
        }
    }
}

impl From<&SlashParseResult> for SlashParseResultDto {
    fn from(result: &SlashParseResult) -> Self {
        Self {
            version: result.version.clone(),
            context: ContextDto::from(&result.context),
            commands: result.commands.iter().map(CommandDto::from).collect(),
            text_blocks: result.text_blocks.iter().map(TextBlockDto::from).collect(),
        }
    }
}

/// Serialize a `SlashParseResult` to a JSON string.
pub fn to_json(result: &SlashParseResult) -> Result<String, serde_json::Error> {
    let dto = SlashParseResultDto::from(result);
    serde_json::to_string(&dto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ParserContext;

    #[test]
    fn to_json_empty_result_produces_valid_json() {
        let result = SlashParseResult {
            version: "0.1.0".to_string(),
            context: ParserContext::default(),
            commands: Vec::new(),
            text_blocks: Vec::new(),
        };
        let json = to_json(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["version"], "0.1.0");
        assert!(parsed["commands"].as_array().unwrap().is_empty());
        // text_blocks should be omitted when empty
        assert!(parsed.get("text_blocks").is_none());
    }

    #[test]
    fn to_json_serializes_populated_context_correctly() {
        let extra_json = serde_json::json!({ "feature_flag": true });
        let context = ParserContext {
            source: Some("API".to_string()),
            timestamp: Some("2026-03-13T00:00:00Z".to_string()),
            user: Some("tom".to_string()),
            session_id: Some("session-123".to_string()),
            extra: Some(extra_json),
        };

        let result = SlashParseResult {
            version: "0.1.0".to_string(),
            context,
            commands: Vec::new(),
            text_blocks: Vec::new(),
        };

        let json = to_json(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["context"]["source"], "API");
        assert_eq!(parsed["context"]["timestamp"], "2026-03-13T00:00:00Z");
        assert_eq!(parsed["context"]["user"], "tom");
        assert_eq!(parsed["context"]["session_id"], "session-123");
        assert_eq!(parsed["context"]["extra"]["feature_flag"], true);
    }
}
