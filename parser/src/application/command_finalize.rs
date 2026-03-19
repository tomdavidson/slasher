use super::command_accumulate::PendingCommand;
use crate::domain::{ArgumentMode, Command, CommandArguments, LineRange, Warning};

#[derive(Debug)]
pub struct FinalizedCommand {
    pub command: Command,
    pub warnings: Vec<Warning>,
}

/// Consume a PendingCommand and produce the finalized Command value plus any warnings.
///
/// §7: id is formatted as cmd-{n} using the zero-based encounter index from start_command.
/// §8.2: raw is the physical lines joined with newline separators (opener + body + closer).
/// §5.2.5: an unclosed fence produces a Warning rather than a hard parse failure.
pub fn finalize_command(pending: PendingCommand) -> FinalizedCommand {
    let mut warnings = Vec::new();

    // §5.2.5: if the parser reached EOF without finding a closing fence, emit one warning
    // and finalize with whatever payload was accumulated.
    if pending.mode == ArgumentMode::Fence && pending.is_open {
        warnings.push(Warning {
            wtype: "unclosed-fence".to_string(),
            start_line: Some(pending.start_line),
            message: Some(format!(
                "Fenced block opened at line {} was never closed.",
                pending.start_line
            )),
        });
    }

    // §8.2: raw is built from all accumulated physical lines joined with newline separators.
    // For single-line commands this is just the one header line.
    // For fenced commands this includes the opener, all body lines, and the closer if present.
    let raw = pending.raw_lines.join("\n");

    // §5.1: single-line payload equals header_text (initialised at start_command).
    // §5.2.2: fence payload is the verbatim body lines joined with newline separators.
    let payload = pending.payload_lines.join("\n");

    let command = Command {
        // §7: sequential zero-based id formatted as cmd-0, cmd-1, ...
        id: format!("cmd-{}", pending.id),
        name: pending.name,
        raw,
        range: LineRange {
            start_line: pending.start_line,
            end_line: pending.end_line,
        },
        arguments: CommandArguments {
            header: pending.header_text,
            mode: pending.mode,
            fence_lang: pending.fence_lang,
            payload,
        },
    };

    FinalizedCommand { command, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            command_accumulate::{accept_line, start_command},
            line_classify::{CommandHeader, LineKind, classify_line},
        },
        domain::ArgumentMode,
    };

    fn header_from(line: &str) -> CommandHeader {
        match classify_line(line) {
            LineKind::Command(h) => h,
            _ => panic!("expected command header from: {line:?}"),
        }
    }

    // --- id ---

    #[test]
    fn id_is_formatted_as_cmd_zero() {
        // §7: commands are assigned sequential zero-based IDs formatted as cmd-0, cmd-1, ...
        let cmd = start_command(header_from("/deploy staging"), 0, 0);
        let result = finalize_command(cmd);
        assert_eq!(result.command.id, "cmd-0");
    }

    #[test]
    fn id_reflects_encounter_index() {
        // §7: the id counter is caller-supplied; cmd-5 identifies the sixth command encountered.
        let cmd = start_command(header_from("/deploy staging"), 0, 5);
        let result = finalize_command(cmd);
        assert_eq!(result.command.id, "cmd-5");
    }

    // --- raw ---

    #[test]
    fn single_line_raw_is_header_line_only() {
        // §8.2: for a single-line command raw contains only the original command line.
        let cmd = start_command(header_from("/deploy production --region us-west-2"), 0, 0);
        let result = finalize_command(cmd);
        assert_eq!(result.command.raw, "/deploy production --region us-west-2");
    }

    #[test]
    fn fenced_raw_includes_opener_body_and_closer() {
        // §8.2: for fenced commands raw includes the command line, all body lines,
        // and the closing fence line, joined with newline separators.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "line one");
        let (cmd, _) = accept_line(cmd, 2, "line two");
        let (cmd, _) = accept_line(cmd, 3, "```");
        let result = finalize_command(cmd);
        assert_eq!(result.command.raw, "/cmd ```\nline one\nline two\n```");
    }

    #[test]
    fn unclosed_fence_raw_includes_opener_and_partial_body() {
        // §8.2: if the fence is unclosed raw still contains all accumulated physical lines;
        // there is simply no closer line to include.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "partial");
        let result = finalize_command(cmd);
        assert_eq!(result.command.raw, "/cmd ```\npartial");
    }

    // --- warnings ---

    #[test]
    fn unclosed_fence_emits_exactly_one_warning() {
        // §5.2.5: an unclosed fence produces exactly one warning; the parser does not fail.
        let cmd = start_command(header_from("/cmd ```rust"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "line1");
        let (cmd, _) = accept_line(cmd, 2, "line2");
        let result = finalize_command(cmd);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn unclosed_fence_warning_wtype_is_unclosed_fence() {
        // §5.2.5: the warning type string is "unclosed-fence".
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let result = finalize_command(cmd);
        assert_eq!(result.warnings[0].wtype, "unclosed-fence");
    }

    #[test]
    fn unclosed_fence_warning_start_line_is_opener_physical_line() {
        // §5.2.5: start_line is set to the fence opener's physical line number.
        let cmd = start_command(header_from("/cmd ```"), 4, 0);
        let result = finalize_command(cmd);
        assert_eq!(result.warnings[0].start_line, Some(4));
    }

    #[test]
    fn unclosed_fence_warning_message_is_present() {
        // §5.2.5 (implied): a human-readable message is present and non-empty.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let result = finalize_command(cmd);
        assert!(result.warnings[0].message.as_deref().map(|m| !m.is_empty()).unwrap_or(false));
    }

    #[test]
    fn unclosed_fence_warning_message_references_start_line() {
        // §5.2.5 (implied): the message should identify the offending line number for diagnostics.
        let cmd = start_command(header_from("/cmd ```"), 7, 0);
        let result = finalize_command(cmd);
        let msg = result.warnings[0].message.as_deref().unwrap_or("");
        assert!(msg.contains('7'));
    }

    #[test]
    fn closed_fence_has_no_warning() {
        // §5.2.3: once the closing fence is found the command is well-formed; no warning emitted.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "content");
        let (cmd, _) = accept_line(cmd, 2, "```");
        let result = finalize_command(cmd);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn single_line_has_no_warning() {
        // §5.1 (implied): a single-line command is always complete; no warning emitted.
        let cmd = start_command(header_from("/help"), 0, 0);
        let result = finalize_command(cmd);
        assert!(result.warnings.is_empty());
    }

    // --- payload ---

    #[test]
    fn single_line_payload_equals_header_text() {
        // §5.1: in single-line mode header and payload contain the same string.
        let cmd = start_command(header_from("/hello world"), 0, 0);
        let result = finalize_command(cmd);
        assert_eq!(result.command.arguments.payload, "world");
        assert_eq!(result.command.arguments.mode, ArgumentMode::SingleLine);
    }

    #[test]
    fn single_line_no_args_has_empty_payload() {
        // §5.1 (implied): a command with no arguments has an empty header and empty payload.
        let cmd = start_command(header_from("/ping"), 0, 0);
        let result = finalize_command(cmd);
        assert_eq!(result.command.arguments.payload, "");
        assert_eq!(result.command.arguments.header, "");
    }

    #[test]
    fn fence_payload_is_body_lines_joined() {
        // §5.2.2: fence payload is the verbatim body lines joined with newline separators.
        // §5.2.4: the closing fence line is not included in the payload.
        // Order matters: body accumulation (§5.2.2) must be verified against closer exclusion (§5.2.4).
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "line one");
        let (cmd, _) = accept_line(cmd, 2, "line two");
        let (cmd, _) = accept_line(cmd, 3, "```");
        let result = finalize_command(cmd);
        assert_eq!(result.command.arguments.payload, "line one\nline two");
        assert_eq!(result.command.arguments.mode, ArgumentMode::Fence);
    }

    #[test]
    fn empty_fence_body_has_empty_payload() {
        // §5.2.2 (implied): a fence with no body lines between opener and closer has empty payload.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "```");
        let result = finalize_command(cmd);
        assert_eq!(result.command.arguments.payload, "");
    }

    // --- range and metadata ---

    #[test]
    fn finalized_command_has_correct_range() {
        // §2.2.1: range covers zero-based physical line numbers from opener through closer.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "body");
        let (cmd, _) = accept_line(cmd, 2, "```");
        let result = finalize_command(cmd);
        assert_eq!(result.command.range.start_line, 0);
        assert_eq!(result.command.range.end_line, 2);
    }

    #[test]
    fn finalized_command_name_matches_header() {
        // §3.1 (implied): the command name is carried through the pipeline unchanged.
        let cmd = start_command(header_from("/deploy production"), 0, 0);
        let result = finalize_command(cmd);
        assert_eq!(result.command.name, "deploy");
    }

    #[test]
    fn fence_lang_preserved_in_arguments() {
        // §5.2.1: fence_lang is the language identifier from the opener; null if absent.
        let cmd = start_command(header_from("/code ```rust"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "fn main() {}");
        let (cmd, _) = accept_line(cmd, 2, "```");
        let result = finalize_command(cmd);
        assert_eq!(result.command.arguments.fence_lang, Some("rust".to_string()));
    }

    #[test]
    fn fence_without_lang_has_none_fence_lang() {
        // §5.2.1: fence_lang is None when no language identifier follows the backtick run.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "```");
        let result = finalize_command(cmd);
        assert_eq!(result.command.arguments.fence_lang, None);
    }

    // --- Property tests ---

    use proptest::prelude::*;

    fn valid_command_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9\\-]{0,15}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
    }

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn id_format_is_cmd_n(
            // §7: the id field always matches the pattern cmd-[0-9]+.
            name in valid_command_name(),
            n in 0usize..1000
        ) {
            let input = format!("/{name} arg");
            let cmd = start_command(header_from(&input), 0, n);
            let result = finalize_command(cmd);
            prop_assert_eq!(result.command.id, format!("cmd-{n}"));
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn finalized_name_matches_pending_name(name in valid_command_name()) {
            // §3.1 (implied): name survives the full start_command -> finalize_command pipeline.
            let input = format!("/{name} arg");
            let cmd = start_command(header_from(&input), 0, 0);
            let result = finalize_command(cmd);
            prop_assert_eq!(result.command.name, name);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn closed_fence_never_warns(
            // §5.2.3: a properly closed fence produces zero warnings regardless of body content.
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9 ]{1,30}", 1..8)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0, 0);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });
            let (cmd, _) = accept_line(cmd, body_lines.len() + 1, "```");
            let result = finalize_command(cmd);
            prop_assert!(result.warnings.is_empty());
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn unclosed_fence_always_warns(
            // §5.2.5: any fence that reaches EOF without a closer produces exactly one warning.
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9]{1,20}", 1..5)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0, 0);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });
            let result = finalize_command(cmd);
            prop_assert_eq!(result.warnings.len(), 1);
            prop_assert_eq!(&result.warnings[0].wtype, "unclosed-fence");
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fence_payload_equals_body_lines_joined(
            // §5.2.2: payload is exactly the body lines joined with "\n", verbatim.
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9 ]{1,20}", 1..8)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0, 0);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });
            let (cmd, _) = accept_line(cmd, body_lines.len() + 1, "```");
            let result = finalize_command(cmd);
            prop_assert_eq!(result.command.arguments.payload, body_lines.join("\n"));
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn raw_newline_count_equals_physical_lines_minus_one(
            // §8.2: raw joins all physical lines so its newline count equals consumed lines - 1.
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9]{1,20}", 0..6)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0, 0);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });
            let (cmd, _) = accept_line(cmd, body_lines.len() + 1, "```");
            let result = finalize_command(cmd);
            // opener + body + closer = body_lines.len() + 2 physical lines
            let expected_newlines = body_lines.len() + 1;
            prop_assert_eq!(
                result.command.raw.chars().filter(|&c| c == '\n').count(),
                expected_newlines
            );
        }
    }
}
