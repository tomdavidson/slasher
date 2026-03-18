use crate::domain::ArgumentMode;

/// Raw classification of a single input line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    Command(CommandHeader),
    Text,
}

/// Extracted header fields from a line that starts a slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHeader {
    pub raw: String,
    pub name: String,
    pub header_text: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
}

/// Classify a single line as either a command header or plain text.
pub fn classify_line(line: &str) -> LineKind {
    match try_parse_command(line) {
        Some(header) => LineKind::Command(header),
        None => LineKind::Text,
    }
}

fn try_parse_command(line: &str) -> Option<CommandHeader> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }

    let without_slash = &trimmed[1..];

    // Command name is the first contiguous non-whitespace run.
    let mut parts = without_slash.splitn(2, char::is_whitespace);
    let name_raw = parts.next().filter(|n| !n.is_empty())?;

    // Command must begin with a lowercase ASCII letter and may contain lowercase letters, digits, and hyphens
    if !name_raw.starts_with(|c: char| c.is_ascii_lowercase()) {
        return None;
    }

    if !name_raw
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return None;
    }
    let name = name_raw.to_string();
    let rest = parts.next().unwrap_or("").trim_start();

    // Fence mode: rest starts with ```
    if let Some(stripped) = rest.strip_prefix("```") {
        let after_ticks = stripped.trim();
        let lang = after_ticks
            .split_whitespace()
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        return Some(CommandHeader {
            raw: line.to_string(),
            name,
            header_text: rest.to_string(),
            mode: ArgumentMode::Fence,
            fence_lang: lang,
        });
    }

    // Continuation mode: rest ends with trailing backslash.
    if rest.ends_with('\\') {
        let header_text = rest.trim_end_matches('\\').trim_end().to_string();

        return Some(CommandHeader {
            raw: line.to_string(),
            name,
            header_text,
            mode: ArgumentMode::Continuation,
            fence_lang: None,
        });
    }

    // Single-line mode (default).
    Some(CommandHeader {
        raw: line.to_string(),
        name,
        header_text: rest.to_string(),
        mode: ArgumentMode::SingleLine,
        fence_lang: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_line() {
        assert_eq!(classify_line("hello world"), LineKind::Text);
    }

    #[test]
    fn bare_slash_is_text() {
        assert_eq!(classify_line("/ "), LineKind::Text);
    }

    #[test]
    fn simple_command() {
        let result = classify_line("/cmd some args");
        match result {
            LineKind::Command(h) => {
                assert_eq!(h.name, "cmd");
                assert_eq!(h.header_text, "some args");
                assert_eq!(h.mode, ArgumentMode::SingleLine);
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn fence_command_with_lang() {
        let result = classify_line("/code ```rust");
        match result {
            LineKind::Command(h) => {
                assert_eq!(h.name, "code");
                assert_eq!(h.mode, ArgumentMode::Fence);
                assert_eq!(h.fence_lang.as_deref(), Some("rust"));
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn continuation_command() {
        let result = classify_line("/cmd first part \\");
        match result {
            LineKind::Command(h) => {
                assert_eq!(h.name, "cmd");
                assert_eq!(h.header_text, "first part");
                assert_eq!(h.mode, ArgumentMode::Continuation);
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn command_with_no_args() {
        let result = classify_line("/help");
        match result {
            LineKind::Command(h) => {
                assert_eq!(h.name, "help");
                assert_eq!(h.header_text, "");
                assert_eq!(h.mode, ArgumentMode::SingleLine);
            }
            _ => panic!("expected Command"),
        }
    }
    #[test]
    fn slash_alone_no_space_is_text() {
        assert_eq!(classify_line("/"), LineKind::Text);
    }

    #[test]
    fn command_name_starting_with_uppercase_is_text() {
        assert_eq!(classify_line("/Hello"), LineKind::Text);
    }

    #[test]
    fn command_name_starting_with_digit_is_text() {
        assert_eq!(classify_line("/2fast"), LineKind::Text);
    }

    #[test]
    fn leading_spaces_still_detected_as_command() {
        match classify_line("   /cmd arg") {
            LineKind::Command(h) => assert_eq!(h.name, "cmd"),
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn fence_without_language() {
        match classify_line("/cmd ```") {
            LineKind::Command(h) => {
                assert_eq!(h.mode, ArgumentMode::Fence);
                assert_eq!(h.fence_lang, None);
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn raw_preserves_original_line() {
        match classify_line("  /cmd arg") {
            LineKind::Command(h) => assert_eq!(h.raw, "  /cmd arg"),
            _ => panic!("expected Command"),
        }
    }

    // --- Property tests ---

    use proptest::prelude::*;

    fn valid_command_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9\\-]{0,20}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
    }

    fn arbitrary_line() -> impl Strategy<Value = String> {
        prop_oneof![
            // plain text
            "[a-zA-Z0-9 !.,]{0,80}",
            // command-like
            valid_command_name().prop_flat_map(|name| {
                "[a-zA-Z0-9 ]{0,40}".prop_map(move |args| format!("/{name} {args}"))
            }),
            // leading whitespace + command
            valid_command_name().prop_flat_map(|name| {
                (1usize..5, "[a-zA-Z0-9 ]{0,40}").prop_map(move |(spaces, args)| {
                    format!("{}/{} {}", " ".repeat(spaces), name, args)
                })
            }),
        ]
    }

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn classify_never_panics(line in "[\\x00-\\x7F]{0,200}") {
            let _ = classify_line(&line);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn valid_name_always_produces_command(name in valid_command_name(), args in "[a-z0-9 ]{0,40}") {
            let input = format!("/{name} {args}");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.name, name),
                LineKind::Text => panic!("expected Command for input: {input}"),
            }
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn text_without_slash_is_never_command(line in "[a-zA-Z0-9 !.,]{1,80}") {
            prop_assert!(matches!(classify_line(&line), LineKind::Text));
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn raw_field_preserves_original_input(line in arbitrary_line()) {
            if let LineKind::Command(h) = classify_line(&line) {
                prop_assert_eq!(h.raw, line);
            }
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fence_mode_iff_backticks_present(name in valid_command_name(), lang in "[a-z]{0,10}") {
            let input = format!("/{name} ```{lang}");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.mode, ArgumentMode::Fence),
                _ => panic!("expected Command"),
            }
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn continuation_mode_iff_trailing_backslash(
            name in valid_command_name(),
            args in "[a-z0-9 ]{1,30}"
        ) {
            let input = format!("/{name} {args} \\");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.mode, ArgumentMode::Continuation),
                _ => panic!("expected Command"),
            }
        }
    }
}
