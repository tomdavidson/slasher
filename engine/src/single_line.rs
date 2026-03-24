use crate::{ArgumentMode, Command, CommandArguments, LineRange, classify::CommandHeader};

/// Finalize a single-line command. No accumulation needed.
/// Extracted from the single-line paths of start_command + finalize_command.
pub fn finalize_single_line(header: CommandHeader, raw: String, id: usize, range: LineRange) -> Command {
    let payload = header.header_text.clone();
    Command {
        id: format!("cmd-{}", id),
        name: header.name,
        raw,
        range,
        arguments: CommandArguments {
            header: header.header_text,
            mode: ArgumentMode::SingleLine,
            fence_lang: None,
            payload,
        },
    }
}

#[cfg(test)]
mod tests {

    use crate::{ArgumentMode, LineRange, classify::CommandHeader, single_line::finalize_single_line};

    fn make_header(name: &str, header_text: &str) -> CommandHeader {
        CommandHeader {
            raw: format!("/{name} {header_text}"),
            name: name.to_string(),
            header_text: header_text.to_string(),
            mode: ArgumentMode::SingleLine,
            fence_lang: None,
            fence_backtick_count: 0,
        }
    }

    // =========================================================================
    // Output field mapping
    // Engine Spec §9.3 / RFC §7.1 / RFC §6.5
    // =========================================================================

    #[test]
    fn id_and_name() {
        // RFC §6.5: sequential zero-based id "cmd-{n}".
        // Engine Spec §3.2: name from header.
        let h = make_header("deploy", "prod");
        let cmd = finalize_single_line(h, "/deploy prod".into(), 5, LineRange { start_line: 0, end_line: 0 });
        assert_eq!(cmd.id, "cmd-5");
        assert_eq!(cmd.name, "deploy");
    }

    #[test]
    fn raw_and_range_passed_through() {
        // RFC §7.1: raw source is "the exact source text from the normalized
        // input (before line joining)."
        // Engine Spec §3.6: LineRange inclusive on both ends.
        let h = make_header("cmd", "arg");
        let range = LineRange { start_line: 3, end_line: 5 };
        let cmd = finalize_single_line(h, "/cmd arg".into(), 0, range);
        assert_eq!(cmd.raw, "/cmd arg");
        assert_eq!(cmd.range.start_line, 3);
        assert_eq!(cmd.range.end_line, 5);
    }

    // =========================================================================
    // Single-line mode: header == payload
    // RFC §4.4 / RFC §5.1 / Engine Spec §3.3
    // =========================================================================

    #[test]
    fn payload_equals_header() {
        // RFC §4.4: "In single-line mode, the header and payload are identical
        // (the full arguments text)."
        // RFC §5.1: mode is "single-line", no fence language.
        let h = make_header("deploy", "production --region us-west-2");
        let cmd = finalize_single_line(
            h,
            "/deploy production --region us-west-2".into(),
            0,
            LineRange { start_line: 0, end_line: 0 },
        );
        assert_eq!(cmd.arguments.payload, "production --region us-west-2");
        assert_eq!(cmd.arguments.header, "production --region us-west-2");
        assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
        assert_eq!(cmd.arguments.fence_lang, None);
    }

    #[test]
    fn empty_args() {
        // RFC §4.3: "The arguments portion may be empty (command with no arguments)."
        // RFC §5.1: empty args -> header and payload are both empty strings.
        let h = make_header("ping", "");
        let cmd = finalize_single_line(h, "/ping".into(), 0, LineRange { start_line: 0, end_line: 0 });
        assert_eq!(cmd.arguments.payload, "");
        assert_eq!(cmd.arguments.header, "");
        assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
        assert_eq!(cmd.arguments.fence_lang, None);
    }

    // =========================================================================
    // Raw passthrough: leading whitespace and joined lines
    // RFC §7.1
    // =========================================================================

    #[test]
    fn raw_with_leading_whitespace_preserved() {
        // RFC §7.1: raw is the exact source text including leading whitespace.
        let h = make_header("cmd", "arg");
        let cmd = finalize_single_line(h, "  /cmd arg".into(), 0, LineRange { start_line: 0, end_line: 0 });
        assert_eq!(cmd.raw, "  /cmd arg");
    }

    #[test]
    fn raw_with_multi_physical_lines_preserved() {
        // RFC §7.1 / RFC Appendix B.2: caller builds raw from joined physical
        // lines. This function must pass it through unmodified.
        let h = make_header("deploy", "prod --region us-west-2");
        let raw = "/deploy prod \\\n --region us-west-2".to_string();
        let cmd = finalize_single_line(h, raw.clone(), 0, LineRange { start_line: 0, end_line: 1 });
        assert_eq!(cmd.raw, raw);
        assert_eq!(cmd.range.start_line, 0);
        assert_eq!(cmd.range.end_line, 1);
    }

    // =========================================================================
    // Property tests
    // =========================================================================

    use proptest::prelude::*;

    proptest! {
        // RFC §4.4: in single-line mode header and payload are always identical.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn payload_always_equals_header(
            name in "[a-z][a-z0-9]{0,10}",
            args in "[a-zA-Z0-9 ]{0,50}"
        ) {
            let h = make_header(&name, &args);
            let raw = format!("/{name} {args}");
            let cmd = finalize_single_line(h, raw, 0, LineRange { start_line: 0, end_line: 0 });
            prop_assert_eq!(&cmd.arguments.payload, &cmd.arguments.header);
        }

        // Engine Spec §3.3: fence_lang is always None for single-line commands.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fence_lang_always_none(
            name in "[a-z][a-z0-9]{0,10}",
            args in "[a-zA-Z0-9 ]{0,50}"
        ) {
            let h = make_header(&name, &args);
            let raw = format!("/{name} {args}");
            let cmd = finalize_single_line(h, raw, 0, LineRange { start_line: 0, end_line: 0 });
            prop_assert_eq!(cmd.arguments.fence_lang, None);
            prop_assert_eq!(cmd.arguments.mode, ArgumentMode::SingleLine);
        }
    }
}
