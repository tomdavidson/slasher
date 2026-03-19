use crate::domain::ArgumentMode;

/// Raw classification of a single logical input line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    Command(CommandHeader),
    Text,
}

/// Extracted header fields from a line that opens a slash command.
///
/// `fence_backtick_count` is the length of the backtick run that opened the
/// fence (e.g. 3 for ` ``` `, 4 for ` ```` `). It is 0 for single-line commands.
/// The state machine uses this to recognise the matching closer (§5.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHeader {
    pub raw: String,
    pub name: String,
    pub header_text: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
    pub fence_backtick_count: usize,
}

/// Classify a single logical line as either a command header or plain text.
pub fn classify_line(line: &str) -> LineKind {
    match try_parse_command(line) {
        Some(header) => LineKind::Command(header),
        None => LineKind::Text,
    }
}

/// Find the first run of 3 or more consecutive backtick characters in `s`.
///
/// Returns `(start_byte_offset, backtick_count)`.
fn find_fence_opener(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'`' {
                i += 1;
            }
            let count = i - start;
            if count >= 3 {
                return Some((start, count));
            }
        } else {
            i += 1;
        }
    }
    None
}

fn try_parse_command(line: &str) -> Option<CommandHeader> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }

    let without_slash = &trimmed[1..];

    let mut parts = without_slash.splitn(2, char::is_whitespace);
    let name_raw = parts.next().filter(|n| !n.is_empty())?;

    // §3.1: [a-z][a-z0-9-]*
    if !name_raw.starts_with(|c: char| c.is_ascii_lowercase()) {
        return None;
    }
    if !name_raw
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return None;
    }

    let name = name_raw.to_string();
    let rest = parts.next().unwrap_or("").trim_start();

    // §5.2.1: detect first occurrence of 3+ backticks anywhere in the args.
    if let Some((fence_start, fence_count)) = find_fence_opener(rest) {
        let header_text = rest[..fence_start].trim_end().to_string();
        let after_ticks = &rest[fence_start + fence_count..];
        // "trimmed of leading whitespace, if non-empty and consisting of a
        // single token (no internal whitespace)"
        let after_trimmed = after_ticks.trim();
        let fence_lang =
            if !after_trimmed.is_empty() && !after_trimmed.contains(char::is_whitespace) {
                Some(after_trimmed.to_string())
            } else {
                None
            };

        return Some(CommandHeader {
            raw: line.to_string(),
            name,
            header_text,
            mode: ArgumentMode::Fence,
            fence_lang,
            fence_backtick_count: fence_count,
        });
    }

    // §5.1: single-line mode — no fence opener present.
    Some(CommandHeader {
        raw: line.to_string(),
        name,
        header_text: rest.to_string(),
        mode: ArgumentMode::SingleLine,
        fence_lang: None,
        fence_backtick_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- find_fence_opener ---

    #[test]
    fn find_fence_opener_empty_returns_none() {
        // Structural: no input, no opener.
        assert!(find_fence_opener("").is_none());
    }

    #[test]
    fn find_fence_opener_no_backticks_returns_none() {
        // Structural: absence of backticks cannot produce an opener.
        assert!(find_fence_opener("hello world").is_none());
    }

    #[test]
    fn find_fence_opener_one_backtick_returns_none() {
        // §5.2.1: opener requires "three or more consecutive backtick characters".
        assert!(find_fence_opener("`").is_none());
    }

    #[test]
    fn find_fence_opener_two_backticks_returns_none() {
        // §5.2.1: two consecutive backticks do not meet the three-or-more threshold.
        assert!(find_fence_opener("``").is_none());
    }

    #[test]
    fn find_fence_opener_three_backticks_at_start() {
        // §5.2.1: exactly three backticks form a valid opener at offset 0 with count 3.
        assert_eq!(find_fence_opener("```"), Some((0, 3)));
    }

    #[test]
    fn find_fence_opener_four_backticks_counted_correctly() {
        // §5.2.1: "variable-length backtick fence (three or more)" — four ticks
        // produce count 4, not two separate runs of two.
        assert_eq!(find_fence_opener("````"), Some((0, 4)));
    }

    #[test]
    fn find_fence_opener_backticks_after_prefix() {
        // §5.2.1: "first occurrence" — offset must reflect the actual position in args.
        assert_eq!(find_fence_opener("call_tool write_file ```"), Some((21, 3)));
    }

    #[test]
    fn find_fence_opener_returns_first_qualifying_run() {
        // §5.2.1: "first occurrence" — a second run of 3+ is ignored.
        assert_eq!(find_fence_opener("``` and ```"), Some((0, 3)));
    }

    #[test]
    fn find_fence_opener_two_runs_of_two_returns_none() {
        // §5.2.1: non-adjacent pairs do not combine into a qualifying run.
        assert!(find_fence_opener("`` ``").is_none());
    }

    #[test]
    fn find_fence_opener_tilde_is_not_backtick() {
        // §5.2: "Only backtick (`) fences are recognised. Tilde (~) fences are not supported."
        assert!(find_fence_opener("~~~").is_none());
    }

    // --- try_parse_command ---

    #[test]
    fn no_slash_returns_none() {
        // §3: "first non-whitespace character is `/`" is required for a command.
        assert!(try_parse_command("hello").is_none());
    }

    #[test]
    fn bare_slash_returns_none() {
        // §3.2: "a bare `/`" is an invalid slash line treated as text.
        assert!(try_parse_command("/").is_none());
    }

    #[test]
    fn slash_then_space_returns_none() {
        // §3.2: "/ space" — space after slash means no command name follows.
        assert!(try_parse_command("/ ").is_none());
    }

    #[test]
    fn uppercase_name_returns_none() {
        // §3.1: pattern is `[a-z][a-z0-9-]*`; uppercase start does not match.
        // §3.2: "/Hello" is an example of an invalid slash line.
        assert!(try_parse_command("/Hello").is_none());
    }

    #[test]
    fn digit_name_returns_none() {
        // §3.1: pattern requires lowercase letter start; digit start does not match.
        // §3.2: "/123" is an example of an invalid slash line.
        assert!(try_parse_command("/123").is_none());
    }

    #[test]
    fn underscore_in_name_returns_none() {
        // §3.1: pattern `[a-z][a-z0-9-]*` — hyphens allowed, underscores are not.
        assert!(try_parse_command("/cmd_foo").is_none());
    }

    #[test]
    fn hyphen_in_name_is_valid() {
        // §3.1: "followed by zero or more lowercase ASCII letters, ASCII digits, or hyphens".
        let h = try_parse_command("/call-tool args").unwrap();
        assert_eq!(h.name, "call-tool");
    }

    #[test]
    fn leading_whitespace_before_slash_is_ignored() {
        // §3: "first non-whitespace character is `/`" — leading spaces are stripped.
        let h = try_parse_command("   /cmd arg").unwrap();
        assert_eq!(h.name, "cmd");
    }

    #[test]
    fn raw_field_preserves_original_line_with_leading_whitespace() {
        // §8.2: `raw` must contain "the exact source text … as it appeared in the
        // normalized input". Leading whitespace is part of that text.
        let h = try_parse_command("  /cmd arg").unwrap();
        assert_eq!(h.raw, "  /cmd arg");
    }

    #[test]
    fn no_args_produces_empty_header_single_line() {
        // §3.3: "The arguments portion may be empty (command with no arguments)."
        // §5.1: mode is single-line, header is the empty arguments string.
        let h = try_parse_command("/help").unwrap();
        assert_eq!(h.name, "help");
        assert_eq!(h.header_text, "");
        assert_eq!(h.mode, ArgumentMode::SingleLine);
        assert_eq!(h.fence_lang, None);
        assert_eq!(h.fence_backtick_count, 0);
    }

    #[test]
    fn single_line_header_text_is_full_args() {
        // §3.4: in single-line mode "header and payload contain the same string
        // (the full arguments text)".
        // §3.3: "The whitespace between the command name and the arguments is
        // consumed as a separator and is not included in the arguments string."
        let h = try_parse_command("/deploy production --region us-west-2").unwrap();
        assert_eq!(h.header_text, "production --region us-west-2");
        assert_eq!(h.mode, ArgumentMode::SingleLine);
        assert_eq!(h.fence_backtick_count, 0);
    }

    #[test]
    fn fence_at_start_of_args_gives_empty_header() {
        // §5.2.1: "Text before the backtick run (trimmed of trailing whitespace)
        // becomes arguments.header." When the backtick run is first, header is empty.
        let h = try_parse_command("/cmd ```json").unwrap();
        assert_eq!(h.header_text, "");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, Some("json".to_string()));
        assert_eq!(h.fence_backtick_count, 3);
    }

    #[test]
    fn fence_with_preceding_header_text() {
        // §5.2.1: text before the backtick run, trimmed of trailing whitespace,
        // becomes header. §3.4: header serves as the dispatch/routing portion.
        let h = try_parse_command("/mcp call_tool write_file ```json").unwrap();
        assert_eq!(h.header_text, "call_tool write_file");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, Some("json".to_string()));
        assert_eq!(h.fence_backtick_count, 3);
    }

    #[test]
    fn fence_without_lang_gives_none() {
        // §5.2.1: fence_lang is the optional language identifier — None when absent.
        let h = try_parse_command("/cmd ```").unwrap();
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, None);
    }

    #[test]
    fn fence_lang_none_when_multiple_tokens_after_ticks() {
        // §5.2.1: "consisting of a single token (no internal whitespace)" —
        // multiple tokens disqualify the language identifier.
        let h = try_parse_command("/cmd ``` foo bar").unwrap();
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, None);
    }

    #[test]
    fn fence_backtick_count_four() {
        // §5.2.1: "variable-length backtick fence (three or more)" — count must
        // match the actual run length so the closer check can match correctly (§5.2.4).
        let h = try_parse_command("/cmd ````json").unwrap();
        assert_eq!(h.fence_backtick_count, 4);
        assert_eq!(h.mode, ArgumentMode::Fence);
    }

    #[test]
    fn tilde_fence_not_recognised() {
        // §5.2: "Only backtick (`) fences are recognised. Tilde (~) fences are not supported."
        let h = try_parse_command("/cmd ~~~").unwrap();
        assert_eq!(h.mode, ArgumentMode::SingleLine);
        assert_eq!(h.header_text, "~~~");
    }

    #[test]
    fn fence_opener_anywhere_in_args_is_detected() {
        // §5.2.1: "first occurrence of three or more consecutive backtick characters"
        // — the opener does not need to be at the start of the arguments.
        let h = try_parse_command("/mcp call_tool write_file -c```json").unwrap();
        assert_eq!(h.header_text, "call_tool write_file -c");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, Some("json".to_string()));
    }

    // --- classify_line (public API — composition only) ---

    #[test]
    fn plain_text_line_produces_text_kind() {
        // §3: lines whose first non-whitespace char is not `/` are not commands.
        assert_eq!(classify_line("hello world"), LineKind::Text);
    }

    #[test]
    fn valid_command_produces_command_kind() {
        // §3: valid command lines produce LineKind::Command.
        assert!(matches!(classify_line("/cmd args"), LineKind::Command(_)));
    }

    #[test]
    fn invalid_slash_lines_produce_text_kind() {
        // §3.2: invalid slash lines are treated as ordinary text in idle state.
        assert_eq!(classify_line("/Hello"), LineKind::Text);
        assert_eq!(classify_line("/123"), LineKind::Text);
        assert_eq!(classify_line("/"), LineKind::Text);
        assert_eq!(classify_line("/ "), LineKind::Text);
    }

    // --- Property tests ---

    use proptest::prelude::*;

    fn valid_command_name() -> impl Strategy<Value = String> {
        // §3.1: [a-z][a-z0-9-]*
        "[a-z][a-z0-9\\-]{0,20}".prop_map(|s| s)
    }

    fn arbitrary_line() -> impl Strategy<Value = String> {
        prop_oneof![
            "[a-zA-Z0-9 !.,]{0,80}",
            valid_command_name().prop_flat_map(|name| {
                "[a-zA-Z0-9 ]{0,40}".prop_map(move |args| format!("/{name} {args}"))
            }),
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
                // §8.1 (implied): "The parser is a total function: it always produces a
                // valid envelope for any input." classify_line must never panic.
                let _ = classify_line(&line);
            }

            #[test]
            #[cfg_attr(feature = "tdd", ignore)]
            fn valid_name_always_produces_command(
                name in valid_command_name(),
                args in "[a-z0-9 ]{0,40}"
            ) {
                // §3, §3.1: any line whose first non-whitespace is `/` followed by a
                // name matching `[a-z][a-z0-9-]*` must produce LineKind::Command.
                let input = format!("/{name} {args}");
                match classify_line(&input) {
                    LineKind::Command(h) => prop_assert_eq!(h.name, name),
                    LineKind::Text => panic!("expected Command for input: {input}"),
                }
            }

            #[test]
            #[cfg_attr(feature = "tdd", ignore)]
            fn text_without_slash_is_never_command(line in "[a-zA-Z0-9 !.,]{1,80}") {
                // §3: command detection requires the first non-whitespace char to be `/`.
                prop_assert!(matches!(classify_line(&line), LineKind::Text));
            }

            #[test]
            #[cfg_attr(feature = "tdd", ignore)]
            fn raw_field_preserves_original_input(line in arbitrary_line()) {
                // §8.2: raw must contain "the exact source text … as it appeared in
                // the normalized input".
                if let LineKind::Command(h) = classify_line(&line) {
                    prop_assert_eq!(h.raw, line);
                }
            }

            #[test]
            #[cfg_attr(feature = "tdd", ignore)]
            fn fence_mode_iff_three_or_more_backticks(
                name in valid_command_name(),
                lang in "[a-z]{0,10}"
            ) {
                // §5.2.1: the first occurrence of 3+ consecutive backticks in the args
                // triggers fence mode regardless of position.
                let input = format!("/{name} ```{lang}");
                match classify_line(&input) {
                    LineKind::Command(h) => prop_assert_eq!(h.mode, ArgumentMode::Fence),
                    _ => panic!("expected Command"),
                }
            }

     #[test]
    #[cfg_attr(feature = "tdd", ignore)]
    fn single_line_fence_count_is_zero(
        name in valid_command_name(),
        args in "[a-zA-Z0-9 ]{0,40}"
    ) {
        // §5.1: single-line mode has no fence opener, so fence_backtick_count
        // must be 0.
        let input = format!("/{name} {args}");
        let LineKind::Command(h) = classify_line(&input) else { return Ok(()); };
        prop_assert_eq!(h.mode, ArgumentMode::SingleLine);
        prop_assert_eq!(h.fence_backtick_count, 0);
    }


            #[test]
            #[cfg_attr(feature = "tdd", ignore)]
            fn fence_backtick_count_matches_opener_length(
                name in valid_command_name(),
                extra in 0usize..5
            ) {
                // §5.2.1: "The backtick run length is recorded as the fence marker length."
                // §5.2.4: the closer must have >= this many backticks.
                let ticks = "`".repeat(3 + extra);
                let input = format!("/{name} {ticks}json");
                if let LineKind::Command(h) = classify_line(&input) {
                    prop_assert_eq!(h.fence_backtick_count, 3 + extra);
                }
            }
        }
}
