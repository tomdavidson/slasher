use super::line_classify::{CommandHeader, LineKind, classify_line};
use crate::domain::ArgumentMode;

/// Result of offering a line to an in-progress command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptResult {
    Consumed,
    Completed,
    Rejected,
}

/// In-progress command being assembled from consecutive lines.
#[derive(Debug, Clone)]
pub struct PendingCommand {
    pub name: String,
    pub raw_header: String,
    pub header_text: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub payload_lines: Vec<String>,
    pub is_open: bool,
}

pub fn start_command(header: CommandHeader, line_index: usize) -> PendingCommand {
    let initial_lines = if header.header_text.is_empty() {
        vec![]
    } else {
        vec![header.header_text.clone()]
    };

    let (payload_lines, is_open) = match &header.mode {
        ArgumentMode::SingleLine => (initial_lines, false),
        ArgumentMode::Continuation => (initial_lines, true),
        ArgumentMode::Fence => (vec![], true),
    };

    PendingCommand {
        name: header.name,
        raw_header: header.raw,
        header_text: header.header_text,
        mode: header.mode,
        fence_lang: header.fence_lang,
        start_line: line_index,
        end_line: line_index,
        payload_lines,
        is_open,
    }
}

pub fn accept_line(
    cmd: PendingCommand,
    line_index: usize,
    line: &str,
) -> (PendingCommand, AcceptResult) {
    if !cmd.is_open {
        return (cmd, AcceptResult::Rejected);
    }

    match cmd.mode {
        ArgumentMode::SingleLine => (cmd, AcceptResult::Rejected),
        ArgumentMode::Continuation => accept_continuation(cmd, line_index, line),
        ArgumentMode::Fence => accept_fence(cmd, line_index, line),
    }
}

fn accept_continuation(
    mut cmd: PendingCommand,
    line_index: usize,
    line: &str,
) -> (PendingCommand, AcceptResult) {
    if line.trim().is_empty() {
        cmd.is_open = false;
        return (cmd, AcceptResult::Rejected);
    }

    if matches!(classify_line(line), LineKind::Command(_)) {
        cmd.is_open = false;
        return (cmd, AcceptResult::Rejected);
    }

    if line.trim_end().ends_with('\\') {
        let content = line.trim_end().trim_end_matches('\\').trim_end();
        cmd.payload_lines.push(content.to_string());
        cmd.end_line = line_index;
        (cmd, AcceptResult::Consumed)
    } else {
        cmd.payload_lines.push(line.to_string());
        cmd.end_line = line_index;
        cmd.is_open = false;
        (cmd, AcceptResult::Completed)
    }
}

fn accept_fence(
    mut cmd: PendingCommand,
    line_index: usize,
    line: &str,
) -> (PendingCommand, AcceptResult) {
    if line.trim_start().starts_with("```") {
        cmd.end_line = line_index;
        cmd.is_open = false;
        (cmd, AcceptResult::Completed)
    } else {
        cmd.end_line = line_index;
        cmd.payload_lines.push(line.to_string());
        (cmd, AcceptResult::Consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_from(line: &str) -> CommandHeader {
        match classify_line(line) {
            LineKind::Command(h) => h,
            _ => panic!("expected command header"),
        }
    }

    #[test]
    fn single_line_is_immediately_closed() {
        let cmd = start_command(header_from("/help"), 0);
        assert!(!cmd.is_open);
    }

    #[test]
    fn fence_stays_open_until_closing_ticks() {
        let cmd = start_command(header_from("/code ```rust"), 0);
        assert!(cmd.is_open);

        let (cmd, res) = accept_line(cmd, 1, "fn main() {}");
        assert_eq!(res, AcceptResult::Consumed);
        assert!(cmd.is_open);

        let (cmd, res) = accept_line(cmd, 2, "```");
        assert_eq!(res, AcceptResult::Completed);
        assert!(!cmd.is_open);
        assert_eq!(cmd.payload_lines, vec!["fn main() {}"]);
    }

    #[test]
    fn continuation_ends_on_line_without_backslash() {
        let cmd = start_command(header_from("/cmd first \\"), 0);
        assert!(cmd.is_open);

        let (cmd, res) = accept_line(cmd, 1, "second \\");
        assert_eq!(res, AcceptResult::Consumed);

        let (cmd, res) = accept_line(cmd, 2, "last line");
        assert_eq!(res, AcceptResult::Completed);
        assert!(!cmd.is_open);
    }

    #[test]
    fn continuation_rejects_blank_line() {
        let cmd = start_command(header_from("/cmd start \\"), 0);
        let (cmd, res) = accept_line(cmd, 1, "");
        assert_eq!(res, AcceptResult::Rejected);
        assert!(!cmd.is_open);
    }

    #[test]
    fn continuation_rejects_new_command() {
        let cmd = start_command(header_from("/cmd start \\"), 0);
        let (cmd, res) = accept_line(cmd, 1, "/other");
        assert_eq!(res, AcceptResult::Rejected);
        assert!(!cmd.is_open);
    }

    #[test]
    fn continuation_two_lines_produces_continuation_mode() {
        let header = header_from("/mcp call_tool read_file \\");
        let cmd = start_command(header, 0);

        assert_eq!(cmd.mode, ArgumentMode::Continuation);
        assert!(cmd.is_open);
    }

    #[test]
    fn continuation_header_strips_trailing_marker() {
        let header = header_from("/mcp call_tool read_file \\");
        let cmd = start_command(header, 0);

        assert_eq!(cmd.header_text, "call_tool read_file");
        assert_eq!(cmd.payload_lines, vec!["call_tool read_file"]);
    }

    #[test]
    fn continuation_payload_preserves_newlines() {
        let cmd = start_command(header_from("/mcp call_tool read_file \\"), 0);

        let line1 = r#"{"path": "src/index.ts"}"#;
        let (cmd, res) = accept_line(cmd, 1, line1);
        assert_eq!(res, AcceptResult::Completed);
        assert!(!cmd.is_open);

        assert_eq!(
            cmd.payload_lines,
            vec!["call_tool read_file".to_string(), line1.to_string()]
        );

        assert_eq!(cmd.start_line, 0);
        assert_eq!(cmd.end_line, 1);
    }

    #[test]
    fn continuation_three_lines() {
        let cmd = start_command(header_from("/cmd first \\"), 0);

        let (cmd, res) = accept_line(cmd, 1, "second \\");
        assert_eq!(res, AcceptResult::Consumed);
        let (cmd, res) = accept_line(cmd, 2, "third");
        assert_eq!(res, AcceptResult::Completed);
        assert!(!cmd.is_open);

        assert_eq!(
            cmd.payload_lines,
            vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string()
            ]
        );
    }

    #[test]
    fn continuation_range_spans_all_lines() {
        let cmd = start_command(header_from("/cmd first \\"), 0);

        let (cmd, _) = accept_line(cmd, 1, "second \\");
        let (cmd, _) = accept_line(cmd, 2, "third");

        assert_eq!(cmd.start_line, 0);
        assert_eq!(cmd.end_line, 2);
    }

    #[test]
    fn slash_at_end_without_space_is_not_continuation() {
        let header = header_from("/path /var/log/");
        let cmd = start_command(header, 0);

        assert_eq!(cmd.mode, ArgumentMode::SingleLine);
        assert!(!cmd.is_open);
        assert_eq!(cmd.payload_lines, vec!["/var/log/"]);
    }
}
