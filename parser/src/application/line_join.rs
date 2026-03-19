/// POSIX-style backslash line joining.
///
/// Consumes physical lines and produces logical lines. A physical line ending
/// with `\` is joined with the next physical line, separated by a single space,
/// and the backslash is removed. Joining repeats while the accumulated line
/// still ends with `\`. At EOF, a trailing `\` is silently removed.
///
/// Fence immunity is enforced by the caller: when the state machine enters
/// `InFence`, it calls `next_physical` directly instead of `next_logical`,
/// bypassing the joiner for those lines.
pub struct LogicalLine {
    pub text: String,
    pub first_physical: usize,
    pub last_physical: usize,
}

pub struct LineJoiner {
    lines: Vec<String>,
    cursor: usize,
}

/// Consume the next logical line from `lines` starting at `*cursor`,
/// joining any trailing-backslash continuations.
fn consume_logical(lines: &[String], cursor: &mut usize) -> Option<LogicalLine> {
    if *cursor >= lines.len() {
        return None;
    }

    let first_physical = *cursor;
    let mut text = lines[*cursor].clone();
    *cursor += 1;

    while text.ends_with('\\') {
        text.truncate(text.len() - 1);

        if *cursor >= lines.len() {
            // Trailing backslash at EOF: silently removed, line stands alone.
            break;
        }

        text.push(' ');
        text.push_str(&lines[*cursor]);
        *cursor += 1;
    }

    Some(LogicalLine {
        text,
        first_physical,
        last_physical: *cursor - 1,
    })
}

/// Consume the next raw physical line from `lines` at `*cursor`,
/// bypassing join logic entirely.
fn consume_physical(lines: &[String], cursor: &mut usize) -> Option<(usize, String)> {
    if *cursor >= lines.len() {
        return None;
    }
    let idx = *cursor;
    let line = lines[*cursor].clone();
    *cursor += 1;
    Some((idx, line))
}

impl LineJoiner {
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines, cursor: 0 }
    }

    pub fn next_logical(&mut self) -> Option<LogicalLine> {
        consume_logical(&self.lines, &mut self.cursor)
    }

    /// Used by the state machine when inside a fenced block.
    pub fn next_physical(&mut self) -> Option<(usize, String)> {
        consume_physical(&self.lines, &mut self.cursor)
    }

    pub fn is_exhausted(&self) -> bool {
        self.cursor >= self.lines.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn lines(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    fn joiner(xs: &[&str]) -> LineJoiner {
        LineJoiner::new(lines(xs))
    }

    /// Build a string ending with a backslash without embedding `\\` in literals.
    fn bsl(s: &str) -> String {
        let mut r = s.to_string();
        r.push('\\');
        r
    }

    // --- consume_logical (private free function) ---

    #[test]
    fn consume_logical_empty_returns_none() {
        // Structural: cursor must not advance and None must be returned on empty input.
        let ls = lines(&[]);
        let mut cursor = 0;
        assert!(consume_logical(&ls, &mut cursor).is_none());
        assert_eq!(cursor, 0);
    }

    #[test]
    fn consume_logical_no_backslash_passes_through() {
        // §2.2: "Lines that do not end with `\` are left unchanged."
        let ls = lines(&["/echo hello"]);
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "/echo hello");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 0);
        assert_eq!(cursor, 1);
    }

    #[test]
    fn consume_logical_joins_two_lines() {
        // §2.2 steps 1-3: remove trailing backslash, concatenate with next physical
        // line separated by a single space.
        let ls = vec![bsl("a"), "b".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "a b");
        assert_eq!(ll.last_physical, 1);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn consume_logical_trailing_backslash_at_eof_removed() {
        // §2.2.2: "If the final physical line ends with a backslash and there is no
        // subsequent line to join with, the trailing backslash is removed and the
        // line stands alone."
        let ls = vec![bsl("a")];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "a");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 0);
    }

    #[test]
    fn consume_logical_advances_cursor_past_consumed_lines() {
        // §2.2.1: the cursor must advance past all physical lines consumed by a
        // single logical line so that subsequent calls produce the next logical line.
        let ls = vec![bsl("x"), "y".to_string(), "z".to_string()];
        let mut cursor = 0;
        consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(cursor, 2);
        let ll2 = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll2.text, "z");
    }

    // --- consume_physical (private free function) ---

    #[test]
    fn consume_physical_empty_returns_none() {
        // Structural: cursor must not advance and None must be returned on empty input.
        let ls = lines(&[]);
        let mut cursor = 0;
        assert!(consume_physical(&ls, &mut cursor).is_none());
        assert_eq!(cursor, 0);
    }

    #[test]
    fn consume_physical_returns_raw_line_with_backslash() {
        // §2.3: inside a fenced block "all physical lines are consumed verbatim",
        // including trailing backslashes that would otherwise be join markers.
        let ls = vec![bsl("line one"), "line two".to_string()];
        let mut cursor = 0;
        let (idx, line) = consume_physical(&ls, &mut cursor).unwrap();
        assert_eq!(idx, 0);
        assert!(line.ends_with('\\'));
        assert_eq!(cursor, 1);
    }

    #[test]
    fn consume_physical_does_not_join() {
        // §2.3: "A trailing `\` inside a fence is literal content, not a join marker."
        let ls = vec![bsl("a"), "b".to_string()];
        let mut cursor = 0;
        let (_, line) = consume_physical(&ls, &mut cursor).unwrap();
        assert!(line.ends_with('\\'));
        let (_, line2) = consume_physical(&ls, &mut cursor).unwrap();
        assert_eq!(line2, "b");
    }

    // --- LineJoiner impl (delegation + composition) ---

    #[test]
    fn next_logical_delegates_correctly() {
        // Structural: next_logical must delegate to consume_logical and advance state.
        let mut j = joiner(&["/echo hello"]);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/echo hello");
        assert!(j.is_exhausted());
    }

    #[test]
    fn next_physical_delegates_correctly() {
        // Structural: next_physical must delegate to consume_physical and advance state.
        let input = vec![bsl("line one"), "line two".to_string()];
        let mut j = LineJoiner::new(input);
        let (idx, line) = j.next_physical().unwrap();
        assert_eq!(idx, 0);
        assert!(line.ends_with('\\'));
    }

    #[test]
    fn interleaving_logical_and_physical_shares_cursor() {
        // §2.3: the state machine calls next_logical in idle state and next_physical
        // in fence state; both must share a single cursor so no lines are skipped
        // or double-consumed at the transition boundary.
        let input = vec![bsl("/cmd a"), "  b".to_string(), "fence body".to_string()];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/cmd a   b");
        assert_eq!(ll.last_physical, 1);
        let (idx, line) = j.next_physical().unwrap();
        assert_eq!(idx, 2);
        assert_eq!(line, "fence body");
        assert!(j.is_exhausted());
    }

    #[test]
    fn is_exhausted_false_when_lines_remain() {
        // Structural: is_exhausted must reflect the current cursor position accurately.
        let mut j = joiner(&["a", "b"]);
        assert!(!j.is_exhausted());
        j.next_logical();
        assert!(!j.is_exhausted());
        j.next_logical();
        assert!(j.is_exhausted());
    }

    // --- Spec examples ---

    #[test]
    fn spec_example_three_physical_lines_join_to_one() {
        // §2.2.3 example: three physical lines collapse into one logical line.
        // §2.2.1: first_physical and last_physical cover the full physical range.
        let input = vec![
            bsl("/mcp call_tool read_file"),
            bsl("  --path src/index.ts"),
            "  --format json".to_string(),
        ];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(
            ll.text,
            "/mcp call_tool read_file   --path src/index.ts   --format json"
        );
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 2);
    }

    #[test]
    fn spec_example_trailing_backslash_at_eof_space_preserved() {
        // §2.2.2 example: "/echo hello \" -> "/echo hello " — the backslash is
        // removed but the space before it is part of the content and stays.
        let input = vec![bsl("/echo hello ")];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/echo hello ");
    }

    #[test]
    fn fence_closer_line_with_trailing_backslash_joins_normally() {
        // §2.2: "The join marker is any backslash character immediately before the
        // physical line terminator, regardless of what precedes it. This includes
        // lines that serve other syntactic roles, such as a closing fence line
        // followed by a trailing backslash."
        // §5.2.4: the fence close is detected by the state machine, not the joiner;
        // the joiner has no special case here.
        let input = vec![bsl("```"), "next line".to_string()];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "``` next line");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 1);
    }

    // --- Property tests ---

    proptest! {
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn logical_count_lte_physical_count(
            ls in prop::collection::vec("[a-zA-Z0-9 ]{0,40}", 0..20)
        ) {
            // §2.2: joining only reduces or maintains line count, never increases it.
            let count = ls.len();
            let mut cursor = 0;
            let mut logical_count = 0;
            while consume_logical(&ls, &mut cursor).is_some() {
                logical_count += 1;
            }
            prop_assert!(logical_count <= count);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn clean_lines_pass_through_unchanged(
            ls in prop::collection::vec("[a-zA-Z0-9 !.,]{1,40}", 1..10)
        ) {
            // §2.2: "Lines that do not end with `\` are left unchanged."
            let expected = ls.clone();
            let mut cursor = 0;
            for expected_text in expected {
                let ll = consume_logical(&ls, &mut cursor).unwrap();
                prop_assert_eq!(ll.text, expected_text);
            }
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn ranges_cover_all_physical_lines(
            ls in prop::collection::vec("[a-zA-Z0-9 ]{0,40}", 1..20)
        ) {
            // §2.2.1: logical lines must partition the physical line sequence without
            // gaps or overlaps — every physical line belongs to exactly one logical line.
            let count = ls.len();
            let mut cursor = 0;
            let mut next_expected = 0usize;
            while let Some(ll) = consume_logical(&ls, &mut cursor) {
                prop_assert_eq!(ll.first_physical, next_expected);
                prop_assert!(ll.first_physical <= ll.last_physical);
                next_expected = ll.last_physical + 1;
            }
            prop_assert_eq!(next_expected, count);
        }

        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn exhausted_after_consuming_all_logical_lines(
            ls in prop::collection::vec("[a-zA-Z0-9 ]{0,40}", 0..20)
        ) {
            // Structural: is_exhausted must be true once all lines have been consumed.
            let mut j = LineJoiner::new(ls);
            while j.next_logical().is_some() {}
            prop_assert!(j.is_exhausted());
        }
    }
}
