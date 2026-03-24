use crate::ArgumentMode;

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
/// The state machine uses this to recognise the matching closer (RFC §5.2.3).
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
    s.as_bytes()
        .chunk_by(|a, b| a == b)
        .scan(0usize, |offset, chunk| {
            let start = *offset;
            *offset += chunk.len();
            Some((start, chunk))
        })
        .find(|(_, chunk)| chunk.first() == Some(&b'`') && chunk.len() >= 3)
        .map(|(start, chunk)| (start, chunk.len()))
}

fn is_wsp(c: char) -> bool { c == ' ' || c == '\t' }

fn try_parse_command(line: &str) -> Option<CommandHeader> {
    let trimmed = line.trim_start_matches(is_wsp);
    if !trimmed.starts_with('/') {
        return None;
    }

    let without_slash = &trimmed[1..];

    let mut parts = without_slash.splitn(2, is_wsp);
    let name_raw = parts.next().filter(|n| !n.is_empty())?;

    // RFC §4.1: [a-z]([a-z0-9-]*[a-z0-9])?
    if !name_raw.starts_with(|c: char| c.is_ascii_lowercase()) {
        return None;
    }

    if !name_raw.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return None;
    }

    // RFC §4.1: "Multi-character names MUST NOT end with a hyphen."
    // Engine Spec §15.1: trailing hyphen prohibition added in v0.5.0.
    if name_raw.len() > 1 && name_raw.ends_with('-') {
        return None;
    }

    let name = name_raw.to_string();
    let rest = parts.next().unwrap_or("").trim_start_matches(is_wsp);

    // RFC §5.2.1: detect first occurrence of 3+ backticks anywhere in the args.
    if let Some((fence_start, fence_count)) = find_fence_opener(rest) {
        let header_text = rest[..fence_start].trim_end().to_string();
        let after_ticks = &rest[fence_start + fence_count..];
        let after_trimmed = after_ticks.trim_matches(is_wsp);
        let fence_lang = if !after_trimmed.is_empty()
            && !after_trimmed.contains(is_wsp)
        {
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

    // RFC §5.1: single-line mode — no fence opener present.
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
    use crate::test_helper::valid_command_name;

    // =========================================================================
    // find_fence_opener — internal helper
    // =========================================================================

    // --- Boundary: minimum backtick threshold ---

    #[test]
    fn find_fence_opener_below_threshold_returns_none() {
        // RFC §5.2.1: opener requires three or more consecutive backticks.
        assert!(find_fence_opener("").is_none());
        assert!(find_fence_opener("hello world").is_none());
        assert!(find_fence_opener("`").is_none());
        assert!(find_fence_opener("``").is_none());
    }

    #[test]
    fn find_fence_opener_exactly_three() {
        // RFC §5.2.1: three consecutive backticks form a valid opener.
        assert_eq!(find_fence_opener("```"), Some((0, 3)));
    }

    #[test]
    fn find_fence_opener_four_backticks() {
        // RFC §5.2.1: variable-length fence (three or more). Four ticks
        // produce count 4, not two separate runs.
        assert_eq!(find_fence_opener("````"), Some((0, 4)));
    }

    // --- Position and first-occurrence ---

    #[test]
    fn find_fence_opener_offset_after_prefix() {
        // RFC §5.2.1: "first occurrence" — offset reflects actual position.
        assert_eq!(find_fence_opener("call_tool write_file ```"), Some((21, 3)));
    }

    #[test]
    fn find_fence_opener_returns_first_run_only() {
        // RFC §5.2.1: "first occurrence" — second qualifying run is ignored.
        assert_eq!(find_fence_opener("``` and ```"), Some((0, 3)));
    }

    // --- Non-qualifying patterns ---

    #[test]
    fn find_fence_opener_non_adjacent_pairs_do_not_combine() {
        // RFC §5.2.1: two separate runs of two do not combine.
        assert!(find_fence_opener("`` ``").is_none());
    }

    #[test]
    fn find_fence_opener_tildes_rejected() {
        // RFC §5.2: "Only backtick (`) fences are recognized. Tilde (~)
        // fences MUST NOT be treated as fence openers."
        assert!(find_fence_opener("~~~").is_none());
    }

    // =========================================================================
    // try_parse_command — command name validation
    // =========================================================================

    // --- Lines that are not commands ---

    #[test]
    fn no_slash_returns_none() {
        // RFC §4.2: command line requires first non-WSP char to be "/".
        assert!(try_parse_command("hello").is_none());
    }

    #[test]
    fn bare_slash_returns_none() {
        // RFC §4.5: a bare "/" is an invalid slash line.
        assert!(try_parse_command("/").is_none());
    }

    #[test]
    fn slash_then_space_returns_none() {
        // RFC §4.5: "/ space" — no command name follows the slash.
        assert!(try_parse_command("/ ").is_none());
    }

    #[test]
    fn uppercase_start_returns_none() {
        // RFC §4.1: name must begin with LCALPHA; RFC §4.5: "/Hello" is invalid.
        assert!(try_parse_command("/Hello").is_none());
    }

    #[test]
    fn digit_start_returns_none() {
        // RFC §4.1: name must begin with LCALPHA; RFC §4.5: "/123" is invalid.
        assert!(try_parse_command("/123").is_none());
    }

    #[test]
    fn underscore_in_name_returns_none() {
        // RFC §4.1: pattern allows only [a-z0-9-], not underscores.
        assert!(try_parse_command("/cmd_foo").is_none());
    }

    #[test]
    fn trailing_hyphen_returns_none() {
        // RFC §4.1: "Multi-character names MUST NOT end with a hyphen."
        // RFC §4.5: "/cmd-" is listed as an invalid slash line example.
        // Engine Spec §8 step 3: names ending with hyphen -> Text.
        assert!(try_parse_command("/cmd-").is_none());
    }

    #[test]
    fn trailing_hyphen_with_args_returns_none() {
        // RFC §4.1: trailing hyphen prohibition applies even with args after.
        assert!(try_parse_command("/cmd- args").is_none());
    }

    // --- Valid command names ---

    #[test]
    fn single_letter_name() {
        // RFC §4.1: "A single lowercase letter is a valid command name."
        let h = try_parse_command("/x").unwrap();
        assert_eq!(h.name, "x");
    }

    #[test]
    fn hyphenated_name() {
        // RFC §4.1: hyphens allowed between alphanumerics.
        let h = try_parse_command("/call-tool args").unwrap();
        assert_eq!(h.name, "call-tool");
    }

    #[test]
    fn name_with_digits() {
        // RFC §4.1: digits allowed after the initial letter.
        let h = try_parse_command("/v2").unwrap();
        assert_eq!(h.name, "v2");
    }

    // =========================================================================
    // try_parse_command — leading whitespace and raw field
    // =========================================================================

    #[test]
    fn leading_whitespace_stripped_for_detection() {
        // RFC §4.2: "first non-whitespace character is /"
        let h = try_parse_command("   /cmd arg").unwrap();
        assert_eq!(h.name, "cmd");
    }

    #[test]
    fn raw_preserves_original_line() {
        // RFC §7.1 / Engine Spec §3.2: raw is the exact source text as it
        // appeared in the normalized input, including leading whitespace.
        let h = try_parse_command("  /cmd arg").unwrap();
        assert_eq!(h.raw, "  /cmd arg");
    }

    // =========================================================================
    // try_parse_command — single-line mode
    // =========================================================================

    #[test]
    fn no_args_single_line() {
        // RFC §4.3: "The arguments portion may be empty."
        // RFC §5.1: mode is single-line when no fence opener present.
        let h = try_parse_command("/help").unwrap();
        assert_eq!(h.name, "help");
        assert_eq!(h.header_text, "");
        assert_eq!(h.mode, ArgumentMode::SingleLine);
        assert_eq!(h.fence_lang, None);
        assert_eq!(h.fence_backtick_count, 0);
    }

    #[test]
    fn single_line_header_is_full_args() {
        // RFC §4.4: in single-line mode "header and payload contain the same
        // string (the full arguments text)".
        // RFC §4.3: separator whitespace is not included in arguments.
        let h = try_parse_command("/deploy production --region us-west-2").unwrap();
        assert_eq!(h.header_text, "production --region us-west-2");
        assert_eq!(h.mode, ArgumentMode::SingleLine);
        assert_eq!(h.fence_backtick_count, 0);
    }

    #[test]
    fn tildes_treated_as_single_line_content() {
        // RFC §5.2: tildes are not fence openers, so the line stays single-line.
        let h = try_parse_command("/cmd ~~~").unwrap();
        assert_eq!(h.mode, ArgumentMode::SingleLine);
        assert_eq!(h.header_text, "~~~");
    }

    // =========================================================================
    // try_parse_command — fence mode
    // =========================================================================

    #[test]
    fn fence_at_start_of_args_empty_header() {
        // RFC §5.2.1: "Text before the backtick run … becomes the header."
        // When backticks are first, header is empty.
        let h = try_parse_command("/cmd ```json").unwrap();
        assert_eq!(h.header_text, "");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, Some("json".to_string()));
        assert_eq!(h.fence_backtick_count, 3);
    }

    #[test]
    fn fence_with_preceding_header() {
        // RFC §5.2.1: text before backtick run, trimmed trailing WSP, is header.
        // RFC §4.4: header is the dispatch/routing portion.
        let h = try_parse_command("/mcp call_tool write_file ```json").unwrap();
        assert_eq!(h.header_text, "call_tool write_file");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, Some("json".to_string()));
        assert_eq!(h.fence_backtick_count, 3);
    }

    #[test]
    fn fence_without_lang() {
        // RFC §5.2.1: fence_lang is absent when nothing follows the backticks.
        let h = try_parse_command("/cmd ```").unwrap();
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, None);
    }

    #[test]
    fn fence_lang_none_when_multiple_tokens() {
        // RFC §5.2.1: "single token (no internal whitespace)" — multiple
        // tokens disqualify the language identifier.
        let h = try_parse_command("/cmd ``` foo bar").unwrap();
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, None);
    }

    #[test]
    fn fence_four_backticks() {
        // RFC §5.2.1: variable-length fence. Count must match the run length
        // so the closer check (RFC §5.2.3) can match correctly.
        let h = try_parse_command("/cmd ````json").unwrap();
        assert_eq!(h.fence_backtick_count, 4);
        assert_eq!(h.mode, ArgumentMode::Fence);
    }

    #[test]
    fn fence_opener_mid_args() {
        // RFC §5.2.1: "first occurrence of three or more consecutive backtick
        // characters" — opener does not need to be at the start of args.
        let h = try_parse_command("/mcp call_tool write_file -c```json").unwrap();
        assert_eq!(h.header_text, "call_tool write_file -c");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, Some("json".to_string()));
    }

    // =========================================================================
    // classify_line — public API (composition of try_parse_command)
    // =========================================================================

    #[test]
    fn text_line_produces_text() {
        // RFC §4.2/§6.3: line whose first non-WSP is not "/" -> text.
        assert_eq!(classify_line("hello world"), LineKind::Text);
    }

    #[test]
    fn valid_command_produces_command() {
        // RFC §4.2: valid command line -> LineKind::Command.
        assert!(matches!(classify_line("/cmd args"), LineKind::Command(_)));
    }

    #[test]
    fn invalid_slash_lines_produce_text() {
        // RFC §4.5: invalid slash lines are text. Engine Spec §8 step 3.
        assert_eq!(classify_line("/Hello"), LineKind::Text);
        assert_eq!(classify_line("/123"), LineKind::Text);
        assert_eq!(classify_line("/"), LineKind::Text);
        assert_eq!(classify_line("/ "), LineKind::Text);
        assert_eq!(classify_line("/cmd-"), LineKind::Text);
    }


    #[test]
fn fence_lang_with_nbsp_is_valid_token() {
    // RFC Appendix A: NONWSP = %x21-7E / %x80-10FFFF — U+00A0 (NBSP)
    // is above %x7E in codepoint but falls in %x80-10FFFF, so it is
    // a valid non-whitespace character per the ABNF. A lang-id
    // consisting of a single token containing NBSP must be accepted.
    // NOTE: will FAIL until fence_lang extraction uses is_wsp instead
    // of char::is_whitespace for the single-token check.
    let h = try_parse_command("/cmd ``` nb\u{00A0}sp").unwrap();
    assert_eq!(h.fence_lang, Some("nb\u{00A0}sp".to_string()));
}

#[test]
fn fence_lang_with_em_space_is_valid_token() {
    // RFC Appendix A: NONWSP includes %x80-10FFFF. U+2003 (EM SPACE)
    // is not whitespace per spec, so it does not split a lang-id token.
    // NOTE: will FAIL until fence_lang extraction uses is_wsp instead
    // of char::is_whitespace for the single-token check.
    let h = try_parse_command("/cmd ``` em\u{2003}sp").unwrap();
    assert_eq!(h.fence_lang, Some("em\u{2003}sp".to_string()));
}

#[test]
fn fence_lang_split_by_space_is_none() {
    // RFC §5.2.1: lang-id must be "a single token (no internal
    // whitespace)." SP (U+0020) is WSP per ABNF, so two words
    // separated by SP means no valid lang-id.
    let h = try_parse_command("/cmd ``` two words").unwrap();
    assert_eq!(h.fence_lang, None);
}

#[test]
fn fence_lang_split_by_tab_is_none() {
    // RFC §5.2.1 + Appendix A: HTAB (U+0009) is WSP. A tab between
    // tokens means the string is not a single token.
    let h = try_parse_command("/cmd ``` two\twords").unwrap();
    assert_eq!(h.fence_lang, None);
}


    // =========================================================================
    // Property tests
    // =========================================================================

    use proptest::prelude::*;

    fn arbitrary_line() -> impl Strategy<Value = String> {
        prop_oneof![
            "[a-zA-Z0-9 !.,]{0,80}",
            valid_command_name().prop_flat_map(|name| {
                "[a-zA-Z0-9 ]{0,40}".prop_map(move |args| format!("/{name} {args}"))
            }),
            valid_command_name().prop_flat_map(|name| {
                (1usize..5, "[a-zA-Z0-9 ]{0,40}")
                    .prop_map(move |(spaces, args)| format!("{}/{} {}", " ".repeat(spaces), name, args))
            }),
        ]
    }

    // =========================================================================
    // try_parse_command — trailing hyphen edge cases
    // RFC §4.1: "Multi-character names MUST NOT end with a hyphen."
    // =========================================================================

    #[test]
    fn single_hyphen_after_letter_returns_none() {
        // RFC §4.1: "a-" is two characters ending with hyphen, invalid.
        assert!(try_parse_command("/a-").is_none());
    }

    #[test]
    fn trailing_hyphen_with_fence_returns_none() {
        // RFC §4.1: trailing hyphen prohibition applies even when args
        // contain a fence opener. The name is invalid before args are parsed.
        assert!(try_parse_command("/cmd- ```json").is_none());
    }

    #[test]
    fn internal_hyphens_valid() {
        // RFC §4.1: hyphens between alphanumerics are fine. Only trailing
        // hyphens are prohibited.
        let h = try_parse_command("/a-b-c args").unwrap();
        assert_eq!(h.name, "a-b-c");
    }

    // =========================================================================
    // try_parse_command — special characters after slash
    // RFC §4.5: invalid slash lines are text.
    // =========================================================================

    #[test]
    fn slash_exclamation_returns_none() {
        // RFC §4.5: "/foo!bar" does not match [a-z0-9-].
        assert!(try_parse_command("/foo!bar").is_none());
    }

    #[test]
    fn slash_at_sign_returns_none() {
        // RFC §4.5: "/@cmd" starts with non-LCALPHA after slash.
        assert!(try_parse_command("/@cmd").is_none());
    }

    #[test]
    fn slash_dot_returns_none() {
        // RFC §4.5: "/cmd.sub" contains '.', not in [a-z0-9-].
        assert!(try_parse_command("/cmd.sub").is_none());
    }

    #[test]
    fn slash_emoji_returns_none() {
        // RFC §4.1: command name must be ASCII lowercase. Non-ASCII -> invalid.
        assert!(try_parse_command("/🚀").is_none());
    }

    // =========================================================================
    // try_parse_command — leading whitespace: tab handling
    // RFC §4.2 + Engine Spec §6: WSP is SP (U+0020) and HTAB (U+0009) only.
    // =========================================================================

    #[test]
    fn leading_tab_stripped_for_detection() {
        // RFC §4.2: "first non-whitespace character is /"
        // Engine Spec §6: HTAB is valid leading whitespace.
        let h = try_parse_command("\t/cmd arg").unwrap();
        assert_eq!(h.name, "cmd");
        assert_eq!(h.header_text, "arg");
    }

    #[test]
    fn leading_mixed_space_and_tab() {
        // Engine Spec §6: both SP and HTAB are valid leading whitespace.
        let h = try_parse_command(" \t /cmd arg").unwrap();
        assert_eq!(h.name, "cmd");
    }

    #[test]
    fn raw_preserves_leading_tab() {
        // RFC §7.1: raw is the exact source text including leading whitespace.
        let h = try_parse_command("\t/cmd arg").unwrap();
        assert_eq!(h.raw, "\t/cmd arg");
    }

    // =========================================================================
    // try_parse_command — exotic Unicode whitespace must NOT be treated as WSP
    // Engine Spec §6: "The engine MUST NOT use Rust's char::is_whitespace()"
    // NOTE: These tests assert the CORRECT spec behavior. The current
    // implementation uses trim_start() which strips Unicode WSP, so these
    // will FAIL until the code is updated to use is_wsp().
    // =========================================================================

    #[test]
    fn non_breaking_space_leading_is_not_stripped() {
        // Engine Spec §6: U+00A0 (NO-BREAK SPACE) is NOT whitespace per spec.
        // A line starting with NBSP then "/" is not a command because the first
        // non-WSP character is NBSP, not "/".
        assert_eq!(classify_line("\u{00A0}/cmd arg"), LineKind::Text);
    }

    #[test]
    fn em_space_leading_is_not_stripped() {
        // Engine Spec §6: U+2003 (EM SPACE) is NOT whitespace per spec.
        assert_eq!(classify_line("\u{2003}/cmd arg"), LineKind::Text);
    }

    // =========================================================================
    // try_parse_command — name/args separator whitespace
    // RFC §4.3 + Engine Spec §6: separator is SP or HTAB only.
    // =========================================================================

    #[test]
    fn tab_separates_name_from_args() {
        // Engine Spec §6: HTAB is valid separator between name and arguments.
        let h = try_parse_command("/cmd\targ1 arg2").unwrap();
        assert_eq!(h.name, "cmd");
        assert_eq!(h.header_text, "arg1 arg2");
    }

    #[test]
    fn non_breaking_space_does_not_separate_name() {
        // Engine Spec §6: U+00A0 is NOT a valid separator. It becomes part of
        // the command name, which then fails validation (non-ASCII).
        // NOTE: will FAIL until code replaces char::is_whitespace with is_wsp().
        assert!(try_parse_command("/cmd\u{00A0}arg").is_none());
    }

    // =========================================================================
    // try_parse_command — fence opener touching header text (no space)
    // RFC §5.2.1: "first occurrence of three or more consecutive backtick
    // characters" in the arguments portion.
    // =========================================================================

    #[test]
    fn fence_opener_directly_touching_alpha_header() {
        // RFC §5.2.1: backticks do not need to be space-separated from header.
        // "header```json" — header is "header", fence opens.
        let h = try_parse_command("/cmd header```json").unwrap();
        assert_eq!(h.header_text, "header");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, Some("json".to_string()));
        assert_eq!(h.fence_backtick_count, 3);
    }

    #[test]
    fn fence_opener_touching_numeric_header() {
        // RFC §5.2.1: first occurrence rule applies regardless of preceding chars.
        let h = try_parse_command("/cmd 123```").unwrap();
        assert_eq!(h.header_text, "123");
        assert_eq!(h.mode, ArgumentMode::Fence);
        assert_eq!(h.fence_lang, None);
    }

    // =========================================================================
    // try_parse_command — fence language identifier edge cases
    // RFC §5.2.1: "single token (no internal whitespace)"
    // =========================================================================

    #[test]
    fn fence_lang_with_digits() {
        // RFC §5.2.1: lang-id is any NONWSP token. Digits are valid.
        let h = try_parse_command("/cmd ```es2024").unwrap();
        assert_eq!(h.fence_lang, Some("es2024".to_string()));
    }

    #[test]
    fn fence_lang_with_hyphen() {
        // RFC §5.2.1: lang-id can contain hyphens (e.g., "utf-8").
        let h = try_parse_command("/cmd ```utf-8").unwrap();
        assert_eq!(h.fence_lang, Some("utf-8".to_string()));
    }

    #[test]
    fn fence_lang_with_dots() {
        // RFC §5.2.1: lang-id is any single non-whitespace token.
        let h = try_parse_command("/cmd ```.gitignore").unwrap();
        assert_eq!(h.fence_lang, Some(".gitignore".to_string()));
    }

    #[test]
    fn fence_lang_empty_after_trailing_whitespace() {
        // RFC §5.2.1: "trimmed of leading whitespace, if non-empty" — if only
        // whitespace follows the backticks, fence_lang is None.
        let h = try_parse_command("/cmd ```   ").unwrap();
        assert_eq!(h.fence_lang, None);
    }

    // =========================================================================
    // classify_line — empty and whitespace-only lines
    // RFC §6.3: non-command lines are text.
    // =========================================================================

    #[test]
    fn empty_line_is_text() {
        // RFC §6.3: blank lines are text lines.
        assert_eq!(classify_line(""), LineKind::Text);
    }

    #[test]
    fn whitespace_only_line_is_text() {
        // RFC §6.3: whitespace-only lines are text (no "/" present).
        assert_eq!(classify_line("   "), LineKind::Text);
        assert_eq!(classify_line("\t\t"), LineKind::Text);
    }

    proptest! {
        // RFC §8.2 item 4: parser is a total function. classify_line must
        // never panic on any input.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn classify_never_panics(line in "[\\x00-\\x7F]{0,200}") {
            let _ = classify_line(&line);
        }

        // RFC §4.2 + §4.1: "/" + valid name -> always Command.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn valid_name_always_produces_command(
            name in valid_command_name(),
            args in "[a-z0-9 ]{0,40}"
        ) {
            let input = format!("/{name} {args}");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.name, name),
                LineKind::Text => panic!("expected Command for input: {input}"),
            }
        }

        // RFC §4.2: first non-WSP must be "/" for a command.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn text_without_slash_is_never_command(line in "[a-zA-Z0-9 !.,]{1,80}") {
            prop_assert!(matches!(classify_line(&line), LineKind::Text));
        }

        // RFC §7.1 / Engine Spec §3.2: raw preserves original input.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn raw_preserves_original_input(line in arbitrary_line()) {
            if let LineKind::Command(h) = classify_line(&line) {
                prop_assert_eq!(h.raw, line);
            }
        }

        // RFC §5.2.1: 3+ backticks in args -> fence mode.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fence_mode_iff_three_or_more_backticks(
            name in valid_command_name(),
            lang in "[a-z]{0,10}"
        ) {
            let input = format!("/{name} ```{lang}");
            match classify_line(&input) {
                LineKind::Command(h) => prop_assert_eq!(h.mode, ArgumentMode::Fence),
                _ => panic!("expected Command"),
            }
        }

        // RFC §5.1: single-line -> fence_backtick_count == 0.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn single_line_fence_count_is_zero(
            name in valid_command_name(),
            args in "[a-zA-Z0-9 ]{0,40}"
        ) {
            let input = format!("/{name} {args}");
            let LineKind::Command(h) = classify_line(&input) else { return Ok(()); };
            prop_assert_eq!(h.mode, ArgumentMode::SingleLine);
            prop_assert_eq!(h.fence_backtick_count, 0);
        }

        // RFC §5.2.1: backtick run length is recorded as fence marker length.
        // RFC §5.2.3: closer must have >= this many backticks.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fence_backtick_count_matches_opener_length(
            name in valid_command_name(),
            extra in 0usize..5
        ) {
            let ticks = "`".repeat(3 + extra);
            let input = format!("/{name} {ticks}json");
            if let LineKind::Command(h) = classify_line(&input) {
                prop_assert_eq!(h.fence_backtick_count, 3 + extra);
            }
        }
    // =========================================================================
    // Property tests — trailing hyphen rejection
    // =========================================================================


       // RFC §4.1: names ending with hyphen are always rejected.
       #[test]
       #[cfg_attr(feature = "tdd", ignore)]
       fn trailing_hyphen_always_rejected(
           prefix in "[a-z][a-z0-9]{0,10}",
           args in "[a-zA-Z0-9 ]{0,40}"
       ) {
           let input = format!("/{prefix}- {args}");
           prop_assert!(matches!(classify_line(&input), LineKind::Text));
       }
    }
}
