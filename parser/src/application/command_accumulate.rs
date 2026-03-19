use super::line_classify::CommandHeader;
use crate::domain::ArgumentMode;

/// Result of offering a physical line to an in-progress command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptResult {
    Consumed,
    Completed,
    Rejected,
}

/// In-progress command being assembled from physical lines.
///
/// v0.3.0: Continuation mode is no longer handled here. Multi-physical-line commands
/// that are not fenced are resolved entirely by line joining (§2.2) before this module runs.
#[derive(Debug, Clone)]
pub struct PendingCommand {
    pub id: usize,
    pub name: String,
    pub raw_header: String,
    pub header_text: String,
    pub mode: ArgumentMode,
    pub fence_lang: Option<String>,
    pub fence_backtick_count: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub payload_lines: Vec<String>,
    pub raw_lines: Vec<String>,
    pub is_open: bool,
}

/// Begin accumulating a new command from its parsed header.
///
/// §7: id is the caller-supplied sequential zero-based counter used to produce cmd-0, cmd-1, ...
/// §8.2: raw_lines is seeded with the header's raw text; physical lines are appended by accept_line.
pub fn start_command(header: CommandHeader, line_index: usize, id: usize) -> PendingCommand {
    // §5.1: single-line commands are fully resolved on their logical line; is_open = false.
    // §5.2.1: fence commands open immediately and stay open until a closer is found.
    let (payload_lines, is_open) = match &header.mode {
        ArgumentMode::SingleLine => {
            let lines = if header.header_text.is_empty() {
                vec![]
            } else {
                vec![header.header_text.clone()]
            };
            (lines, false)
        }
        ArgumentMode::Fence => (vec![], true),
    };

    PendingCommand {
        id,
        name: header.name,
        raw_header: header.raw.clone(),
        header_text: header.header_text,
        mode: header.mode,
        fence_lang: header.fence_lang,
        fence_backtick_count: header.fence_backtick_count,
        start_line: line_index,
        end_line: line_index,
        payload_lines,
        raw_lines: vec![header.raw],
        is_open,
    }
}

/// Offer one physical line to an open command.
///
/// §5.2.2: fence body lines are appended to payload_lines verbatim.
/// §5.2.4: a physical line that satisfies is_fence_closer closes the fence.
/// §8.2: every offered line, including the closer, is appended to raw_lines.
pub fn accept_line(
    cmd: PendingCommand,
    line_index: usize,
    line: &str,
) -> (PendingCommand, AcceptResult) {
    if !cmd.is_open {
        return (cmd, AcceptResult::Rejected);
    }

    match cmd.mode {
        ArgumentMode::Fence => accept_fence(cmd, line_index, line),
        // §4: only idle and in-fence states exist in v0.3.0; all other modes are closed.
        _ => (cmd, AcceptResult::Rejected),
    }
}

fn accept_fence(
    mut cmd: PendingCommand,
    line_index: usize,
    line: &str,
) -> (PendingCommand, AcceptResult) {
    // §8.2: raw accumulation happens unconditionally before the closer check.
    cmd.raw_lines.push(line.to_string());
    cmd.end_line = line_index;

    if is_fence_closer(line, cmd.fence_backtick_count) {
        // §5.2.4: the closing fence line is not included in the payload.
        cmd.is_open = false;
        (cmd, AcceptResult::Completed)
    } else {
        cmd.payload_lines.push(line.to_string());
        (cmd, AcceptResult::Consumed)
    }
}

/// §5.2.4: A physical line is a closing fence if, after trimming leading and trailing
/// whitespace, it consists solely of backtick characters and the count is >= the opener's count.
fn is_fence_closer(line: &str, opener_count: usize) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.chars().all(|c| c == '`')
        && trimmed.len() >= opener_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::line_classify::{LineKind, classify_line};

    fn header_from(line: &str) -> CommandHeader {
        match classify_line(line) {
            LineKind::Command(h) => h,
            _ => panic!("expected command header from: {line:?}"),
        }
    }

    // --- start_command ---

    #[test]
    fn single_line_is_immediately_closed() {
        // §5.1: a single-line command is finalized on its logical line; no further lines consumed.
        let cmd = start_command(header_from("/help"), 0, 0);
        assert!(!cmd.is_open);
    }

    #[test]
    fn fence_opens_with_is_open_true() {
        // §5.2.3: fence lifetime begins at the opener and extends until a valid closer or EOF.
        let cmd = start_command(header_from("/code ```rust"), 0, 0);
        assert!(cmd.is_open);
    }

    #[test]
    fn id_is_stored_on_pending_command() {
        // §7: commands are assigned sequential zero-based IDs in encounter order (cmd-0, cmd-1, ...).
        let cmd = start_command(header_from("/deploy production"), 3, 7);
        assert_eq!(cmd.id, 7);
    }

    #[test]
    fn raw_lines_initialized_with_opener() {
        // §8.2: raw includes the command line itself; accumulation starts at start_command.
        let cmd = start_command(header_from("/mcp call_tool ```json"), 0, 0);
        assert_eq!(cmd.raw_lines, vec!["/mcp call_tool ```json"]);
    }

    #[test]
    fn single_line_payload_initialized_from_header_text() {
        // §5.1: in single-line mode header and payload contain the same string.
        let cmd = start_command(header_from("/deploy production --region us-west-2"), 0, 0);
        assert_eq!(cmd.payload_lines, vec!["production --region us-west-2"]);
    }

    #[test]
    fn single_line_no_args_has_empty_payload() {
        // §5.1 (implied): a command with no arguments produces an empty payload_lines.
        let cmd = start_command(header_from("/ping"), 0, 0);
        assert!(cmd.payload_lines.is_empty());
    }

    #[test]
    fn fence_payload_lines_initially_empty() {
        // §5.2.2: the fence body begins empty; content is appended as physical lines arrive.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        assert!(cmd.payload_lines.is_empty());
    }

    #[test]
    fn fence_backtick_count_carried_from_header() {
        // §5.2.1: the opener's backtick run length is recorded for use by the closer check (§5.2.4).
        let cmd = start_command(header_from("/cmd ```rust"), 0, 0);
        assert_eq!(cmd.fence_backtick_count, 3);
    }

    // --- accept_line: fence body ---

    #[test]
    fn fence_body_line_is_consumed() {
        // §5.2.2: physical lines that are not a closer are appended to the payload verbatim.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "fn main() {}");
        assert_eq!(res, AcceptResult::Consumed);
        assert!(cmd.is_open);
        assert_eq!(cmd.payload_lines, vec!["fn main() {}"]);
    }

    #[test]
    fn fence_body_line_added_to_raw_lines() {
        // §8.2: for fenced commands raw includes the command line, all body lines, and the closer.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "first");
        let (cmd, _) = accept_line(cmd, 2, "second");
        assert_eq!(cmd.raw_lines, vec!["/cmd ```", "first", "second"]);
    }

    #[test]
    fn fence_body_preserves_lines_verbatim() {
        // §5.2.2: content is appended verbatim; no parsing rules including line joining apply.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let line = r"  leading spaces and trailing backslash\";
        let (cmd, _) = accept_line(cmd, 1, line);
        assert_eq!(cmd.payload_lines, vec![line]);
    }

    #[test]
    fn blank_line_inside_fence_is_payload() {
        // §5.2.3: inside a fence, blank lines are literal payload content.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "");
        assert_eq!(res, AcceptResult::Consumed);
        assert_eq!(cmd.payload_lines, vec![""]);
    }

    #[test]
    fn command_line_inside_fence_is_payload() {
        // §5.2.3: inside a fence, command triggers are literal payload; no state change.
        let cmd = start_command(header_from("/outer ```"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "/inner arg");
        assert_eq!(res, AcceptResult::Consumed);
        assert!(cmd.is_open);
    }

    // --- accept_line: fence closer ---

    #[test]
    fn fence_closer_completes_command() {
        // §5.2.4: a valid closer transitions the accumulator to closed and returns Completed.
        let cmd = start_command(header_from("/cmd ```rust"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "fn main() {}");
        let (cmd, res) = accept_line(cmd, 2, "```");
        assert_eq!(res, AcceptResult::Completed);
        assert!(!cmd.is_open);
    }

    #[test]
    fn fence_closer_added_to_raw_lines() {
        // §8.2: raw includes the closing fence line if present.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "content");
        let (cmd, _) = accept_line(cmd, 2, "```");
        assert_eq!(cmd.raw_lines, vec!["/cmd ```", "content", "```"]);
    }

    #[test]
    fn fence_closer_not_in_payload() {
        // §5.2.4: the closing fence line is not included in the payload.
        let cmd = start_command(header_from("/cmd ```rust"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "fn main() {}");
        let (cmd, _) = accept_line(cmd, 2, "```");
        assert_eq!(cmd.payload_lines, vec!["fn main() {}"]);
    }

    #[test]
    fn fence_closer_with_more_backticks_than_opener() {
        // §5.2.4: closer count >= opener count; four backticks close a triple-backtick fence.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "````");
        assert_eq!(res, AcceptResult::Completed);
        assert!(!cmd.is_open);
    }

    #[test]
    fn fewer_backticks_than_opener_is_not_a_closer() {
        // §5.2.4: closer count must be >= opener count; two backticks cannot close a triple fence.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "``");
        assert_eq!(res, AcceptResult::Consumed);
        assert!(cmd.is_open);
    }

    #[test]
    fn line_with_backticks_and_trailing_text_is_not_a_closer() {
        // §5.2.4: after trimming whitespace the line must consist solely of backtick characters.
        // A line like "```rust" or "``` trailing" is body content, not a closer.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "```rust");
        assert_eq!(res, AcceptResult::Consumed);
        assert!(cmd.is_open);
    }

    #[test]
    fn fence_closer_with_leading_whitespace_is_valid() {
        // §5.2.4: closer is checked after trimming leading and trailing whitespace.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "   ```   ");
        assert_eq!(res, AcceptResult::Completed);
        assert!(!cmd.is_open);
    }

    #[test]
    fn closed_fence_rejects_subsequent_lines() {
        // §5.2.3: once the closing fence is found the command is finalized; no further lines accepted.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "content");
        let (cmd, _) = accept_line(cmd, 2, "```");
        let (cmd, res) = accept_line(cmd, 3, "after");
        assert_eq!(res, AcceptResult::Rejected);
        assert_eq!(cmd.payload_lines, vec!["content"]);
    }

    // --- accept_line: single-line ---

    #[test]
    fn single_line_rejects_any_subsequent_line() {
        // §5.1: single-line commands are finalized on their logical line; accept_line is a no-op.
        let cmd = start_command(header_from("/help"), 0, 0);
        let (cmd, res) = accept_line(cmd, 1, "anything");
        assert_eq!(res, AcceptResult::Rejected);
        assert!(cmd.payload_lines == vec![""] || cmd.payload_lines.is_empty());
    }

    // --- range tracking ---

    #[test]
    fn end_line_advances_with_each_accepted_line() {
        // §2.2.1: range.endLine is the zero-based index of the last physical line consumed.
        let cmd = start_command(header_from("/cmd ```"), 0, 0);
        let (cmd, _) = accept_line(cmd, 1, "line one");
        let (cmd, _) = accept_line(cmd, 2, "line two");
        let (cmd, _) = accept_line(cmd, 3, "```");
        assert_eq!(cmd.start_line, 0);
        assert_eq!(cmd.end_line, 3);
    }

    // --- Property tests ---

    use proptest::prelude::*;

    fn valid_command_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9\\-]{0,15}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
    }

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn raw_lines_count_equals_lines_consumed(
            // §8.2: raw accumulates opener + every body line + closer (if present).
            name in valid_command_name(),
            body_lines in prop::collection::vec("[a-zA-Z0-9 ]{1,30}", 0..8)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0, 0);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });
            let (cmd, _) = accept_line(cmd, body_lines.len() + 1, "```");
            prop_assert_eq!(cmd.raw_lines.len(), body_lines.len() + 2);
        }

 #[test]
#[cfg_attr(feature = "tdd", ignore)]
fn payload_never_contains_closer(
    // §5.2.4: the closing fence line is never part of the payload.
    // The check mirrors is_fence_closer: non-empty after trim, all backticks.
    name in valid_command_name(),
    body_lines in prop::collection::vec("[a-zA-Z0-9 ]{1,30}", 0..8)
) {
    let input = format!("/{name} ```");
    let cmd = start_command(header_from(&input), 0, 0);
    let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
        let (next, _) = accept_line(cmd, i + 1, line);
        next
    });
    let (cmd, _) = accept_line(cmd, body_lines.len() + 1, "```");
let no_closer = !cmd.payload_lines.iter().any(|l| {
    let t = l.trim();
    !t.is_empty() && t.chars().all(|c| c == '`')
});
prop_assert!(no_closer);
}


        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn fence_closer_property_count_gte_opener(
            // §5.2.4: any line of N or more backticks (N >= opener) closes the fence.
            name in valid_command_name(),
            extra in 0usize..5
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0, 0);
            let closer = "`".repeat(3 + extra);
            let (cmd, res) = accept_line(cmd, 1, &closer);
            prop_assert_eq!(res, AcceptResult::Completed);
            prop_assert!(!cmd.is_open);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn id_is_preserved_through_accumulation(
            // §7: the id assigned at start_command is stable throughout accumulation.
            name in valid_command_name(),
            id in 0usize..1000,
            body_lines in prop::collection::vec("[a-zA-Z0-9]{1,20}", 0..5)
        ) {
            let input = format!("/{name} ```");
            let cmd = start_command(header_from(&input), 0, id);
            let cmd = body_lines.iter().enumerate().fold(cmd, |cmd, (i, line)| {
                let (next, _) = accept_line(cmd, i + 1, line);
                next
            });
            prop_assert_eq!(cmd.id, id);
        }
    }
}
