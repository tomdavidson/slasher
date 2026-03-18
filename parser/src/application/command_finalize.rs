use super::command_accumulate::PendingCommand;
use crate::domain::{ArgumentMode, Command, CommandArguments, LineRange, ParseWarning};

#[derive(Debug)]
pub struct FinalizedCommand {
    pub command: Command,
    pub warnings: Vec<ParseWarning>,
}

pub fn finalize_command(pending: PendingCommand) -> FinalizedCommand {
    let mut warnings = Vec::new();

    match pending.mode {
        ArgumentMode::Fence if pending.is_open => {
            warnings.push(ParseWarning::UnclosedFence {
                start_line: pending.start_line,
            });
        }
        ArgumentMode::Continuation if pending.is_open => {
            warnings.push(ParseWarning::UnclosedContinuation {
                start_line: pending.start_line,
            });
        }
        _ => {}
    }

    let payload = pending.payload_lines.join("\n");

    let command = Command {
        name: pending.name,
        raw: pending.raw_header,
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
            _ => panic!("expected command header"),
        }
    }

    // --- Warning tests ---

    #[test]
    fn unclosed_fence_at_eof_warns() {
        let cmd = start_command(header_from("/cmd ```rust"), 0);
        let (cmd, _) = accept_line(cmd, 1, "line1");
        let (cmd, _) = accept_line(cmd, 2, "line2");

        let result = finalize_command(cmd);

        assert_eq!(result.warnings.len(), 1);
        assert!(matches!(
            result.warnings[0],
            ParseWarning::UnclosedFence { start_line: 0 }
        ));
    }

    #[test]
    fn unclosed_continuation_at_eof_warns() {
        let cmd = start_command(header_from("/cmd first \\"), 0);

        let result = finalize_command(cmd);

        assert_eq!(result.warnings.len(), 1);
        assert!(matches!(
            result.warnings[0],
            ParseWarning::UnclosedContinuation { start_line: 0 }
        ));
    }

    #[test]
    fn closed_fence_has_no_warning() {
        let cmd = start_command(header_from("/cmd ```"), 0);
        let (cmd, _) = accept_line(cmd, 1, "content");
        let (cmd, _) = accept_line(cmd, 2, "```");

        let result = finalize_command(cmd);

        assert!(result.warnings.is_empty());
    }

    #[test]
    fn closed_continuation_has_no_warning() {
        let cmd = start_command(header_from("/cmd first \\"), 0);
        let (cmd, _) = accept_line(cmd, 1, "last line");

        let result = finalize_command(cmd);

        assert!(result.warnings.is_empty());
    }

    #[test]
    fn single_line_has_no_warning() {
        let cmd = start_command(header_from("/help"), 0);

        let result = finalize_command(cmd);

        assert!(result.warnings.is_empty());
    }

    // --- Finalized command structure ---

    #[test]
    fn finalized_command_has_correct_name() {
        let cmd = start_command(header_from("/deploy production"), 0);

        let result = finalize_command(cmd);

        assert_eq!(result.command.name, "deploy");
    }

    #[test]
    fn finalized_command_has_correct_range() {
        let cmd = start_command(header_from("/cmd ```"), 0);
        let (cmd, _) = accept_line(cmd, 1, "body");
        let (cmd, _) = accept_line(cmd, 2, "```");

        let result = finalize_command(cmd);

        assert_eq!(result.command.range.start_line, 0);
        assert_eq!(result.command.range.end_line, 2);
    }

    #[test]
    fn finalized_fence_payload_is_joined_lines() {
        let cmd = start_command(header_from("/cmd ```"), 0);
        let (cmd, _) = accept_line(cmd, 1, "line one");
        let (cmd, _) = accept_line(cmd, 2, "line two");
        let (cmd, _) = accept_line(cmd, 3, "```");

        let result = finalize_command(cmd);

        assert_eq!(result.command.arguments.payload, "line one\nline two");
        assert_eq!(result.command.arguments.mode, ArgumentMode::Fence);
    }

    #[test]
    fn finalized_continuation_payload_is_joined_lines() {
        let cmd = start_command(header_from("/cmd first \\"), 0);
        let (cmd, _) = accept_line(cmd, 1, "second \\");
        let (cmd, _) = accept_line(cmd, 2, "third");

        let result = finalize_command(cmd);

        assert_eq!(result.command.arguments.payload, "first\nsecond\nthird");
        assert_eq!(result.command.arguments.mode, ArgumentMode::Continuation);
    }

    #[test]
    fn finalized_single_line_payload_matches_header() {
        let cmd = start_command(header_from("/hello world"), 0);

        let result = finalize_command(cmd);

        assert_eq!(result.command.arguments.payload, "world");
        assert_eq!(result.command.arguments.mode, ArgumentMode::SingleLine);
    }

    #[test]
    fn finalized_fence_captures_language() {
        let cmd = start_command(header_from("/code ```rust"), 0);
        let (cmd, _) = accept_line(cmd, 1, "fn main() {}");
        let (cmd, _) = accept_line(cmd, 2, "```");

        let result = finalize_command(cmd);

        assert_eq!(
            result.command.arguments.fence_lang,
            Some("rust".to_string())
        );
    }

    use proptest::prelude::*;

    fn valid_command_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9\\-]{0,15}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
    }

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn finalized_name_matches_pending_name(name in valid_command_name()) {
            let input = format!("/{name} arg");
            let cmd = start_command(header_from(&input), 0);
            let result = finalize_command(cmd);
            prop_assert_eq!(result.command.name, name);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn closed_commands_never_warn(
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9]{1,30}", 1..8)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0);
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
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9]{1,20}", 1..5)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });

            let result = finalize_command(cmd);
            prop_assert_eq!(result.warnings.len(), 1);
            let is_unclosed_fence = matches!(result.warnings[0], ParseWarning::UnclosedFence { .. });
            prop_assert!(is_unclosed_fence);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn payload_is_payload_lines_joined(
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9]{1,20}", 1..8)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });
            let (cmd, _) = accept_line(cmd, body_lines.len() + 1, "```");

            let result = finalize_command(cmd);
            prop_assert_eq!(result.command.arguments.payload, body_lines.join("\n"));
        }
    }
}
