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

#[derive(Debug, Clone, PartialEq, Eq)]
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

        // text.push(' ');
        text.push_str(&lines[*cursor]);
        *cursor += 1;
    }

    Some(LogicalLine { text, first_physical, last_physical: *cursor - 1 })
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
    pub fn new(lines: Vec<String>) -> Self { Self { lines, cursor: 0 } }

    pub fn next_logical(&mut self) -> Option<LogicalLine> { consume_logical(&self.lines, &mut self.cursor) }

    /// Used by the state machine when inside a fenced block.
    pub fn next_physical(&mut self) -> Option<(usize, String)> {
        consume_physical(&self.lines, &mut self.cursor)
    }

    // Implemented but does not look like it's needed by the pipeline.
    // TODO: remove if still dead when parser is finished
    #[allow(dead_code)]
    pub fn is_exhausted(&self) -> bool { self.cursor >= self.lines.len() }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn lines(xs: &[&str]) -> Vec<String> { xs.iter().map(|s| s.to_string()).collect() }

    fn joiner(xs: &[&str]) -> LineJoiner { LineJoiner::new(lines(xs)) }

    /// Build a string ending with a backslash without embedding `\\` in literals.
    fn bsl(s: &str) -> String {
        let mut r = s.to_string();
        r.push('\\');
        r
    }

    // =========================================================================
    // consume_logical — no joining (pass-through)
    // RFC §3.2: "Lines that do not end with '\' are left unchanged."
    // =========================================================================

    #[test]
    fn consume_logical_empty_returns_none() {
        // Structural: no input -> None, cursor unchanged.
        let ls = lines(&[]);
        let mut cursor = 0;
        assert!(consume_logical(&ls, &mut cursor).is_none());
        assert_eq!(cursor, 0);
    }

    #[test]
    fn consume_logical_no_backslash_passes_through() {
        // RFC §3.2: line without trailing '\' is unchanged.
        let ls = lines(&["/echo hello"]);
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "/echo hello");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 0);
        assert_eq!(cursor, 1);
    }

    // =========================================================================
    // consume_logical — joining (backslash continuation)
    // RFC §3.2 steps 1-3 / Engine Spec §7.1
    // =========================================================================

    #[test]
    fn consume_logical_joins_two_lines() {
        // Engine Spec §7.1: remove trailing '\', concatenate directly with
        // the next physical line. No separator character is inserted.
        let ls = vec![bsl("a"), "b".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "ab");
        assert_eq!(ll.last_physical, 1);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn consume_logical_chains_three_lines() {
        // RFC §3.2 step 4: "If the result still ends with '\', repeat."
        let ls = vec![bsl("a"), bsl("b"), "c".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "abc");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 2);
        assert_eq!(cursor, 3);
    }

    // =========================================================================
    // consume_logical — trailing backslash at EOF
    // RFC §3.2: "the trailing '\' is removed and the line stands alone."
    // =========================================================================

    #[test]
    fn consume_logical_trailing_backslash_at_eof_removed() {
        // RFC §3.2: final line ends with '\', no subsequent line -> '\' removed.
        let ls = vec![bsl("a")];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "a");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 0);
    }

    // =========================================================================
    // consume_logical — cursor advancement
    // Engine Spec §7.3 / RFC §3.3
    // =========================================================================

    #[test]
    fn consume_logical_advances_cursor_past_joined_lines() {
        // RFC §3.3: each logical line maps to one or more physical lines.
        // Cursor must advance past all consumed physical lines.
        let ls = vec![bsl("x"), "y".to_string(), "z".to_string()];
        let mut cursor = 0;
        consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(cursor, 2);
        let ll2 = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll2.text, "z");
    }

    // =========================================================================
    // consume_physical — raw line access (fence body support)
    // RFC §3.2 / §5.2.2 / Engine Spec §7.2
    // =========================================================================

    #[test]
    fn consume_physical_empty_returns_none() {
        // Structural: no input -> None, cursor unchanged.
        let ls = lines(&[]);
        let mut cursor = 0;
        assert!(consume_physical(&ls, &mut cursor).is_none());
        assert_eq!(cursor, 0);
    }

    #[test]
    fn consume_physical_returns_raw_line_including_backslash() {
        // RFC §5.2.2: "preserving their original content including any
        // trailing backslashes." Engine Spec §7.2: next_physical bypasses
        // join logic.
        let ls = vec![bsl("line one"), "line two".to_string()];
        let mut cursor = 0;
        let (idx, line) = consume_physical(&ls, &mut cursor).unwrap();
        assert_eq!(idx, 0);
        assert!(line.ends_with('\\'));
        assert_eq!(cursor, 1);
    }

    #[test]
    fn consume_physical_does_not_join() {
        // RFC §5.2.2: "Fence body lines are never subject to line joining."
        let ls = vec![bsl("a"), "b".to_string()];
        let mut cursor = 0;
        let (_, line) = consume_physical(&ls, &mut cursor).unwrap();
        assert!(line.ends_with('\\'));
        let (_, line2) = consume_physical(&ls, &mut cursor).unwrap();
        assert_eq!(line2, "b");
    }

    // =========================================================================
    // LineJoiner — delegation and shared cursor
    // Engine Spec §5.3 / Engine Spec §7.2
    // =========================================================================

    #[test]
    fn next_logical_delegates() {
        // Structural: next_logical delegates to consume_logical.
        let mut j = joiner(&["/echo hello"]);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/echo hello");
        assert!(j.is_exhausted());
    }

    #[test]
    fn next_physical_delegates() {
        // Structural: next_physical delegates to consume_physical.
        let input = vec![bsl("line one"), "line two".to_string()];
        let mut j = LineJoiner::new(input);
        let (idx, line) = j.next_physical().unwrap();
        assert_eq!(idx, 0);
        assert!(line.ends_with('\\'));
    }

    #[test]
    fn interleaving_logical_and_physical_shares_cursor() {
        // Engine Spec §5.3: idle state uses next_logical, in-fence uses
        // next_physical. Both share a cursor so no lines are skipped at
        // the transition boundary.
        let input = vec![bsl("/cmd a"), " b".to_string(), "fence body".to_string()];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/cmd a b");
        assert_eq!(ll.last_physical, 1);
        let (idx, line) = j.next_physical().unwrap();
        assert_eq!(idx, 2);
        assert_eq!(line, "fence body");
        assert!(j.is_exhausted());
    }

    #[test]
    fn is_exhausted_tracks_cursor() {
        // Structural: is_exhausted reflects cursor vs line count.
        let mut j = joiner(&["a", "b"]);
        assert!(!j.is_exhausted());
        j.next_logical();
        assert!(!j.is_exhausted());
        j.next_logical();
        assert!(j.is_exhausted());
    }

    // =========================================================================
    // Spec examples from RFC Appendix B
    // =========================================================================

    #[test]
    fn appendix_b2_three_physical_lines_join() {
        // RFC Appendix B.2: three physical lines joined into one logical line.
        // Engine Spec §7.3: first_physical and last_physical track the range.
        let input = vec![
            bsl("/mcp call_tool read_file"),
            bsl("  --path src/index.ts"),
            "  --format json".to_string(),
        ];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/mcp call_tool read_file  --path src/index.ts  --format json");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 2);
    }

    #[test]
    fn trailing_backslash_at_eof_preserves_preceding_space() {
        // RFC §3.2: backslash removed but preceding content (including space)
        // is preserved.
        let input = vec![bsl("/echo hello ")];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/echo hello ");
    }

    #[test]
    fn fence_closer_with_trailing_backslash_joins_in_idle() {
        // RFC §3.2: "The join marker is any '\' immediately before the line
        // boundary, regardless of what precedes it."
        // NOTE: this test exercises the joiner in isolation. In the real
        // pipeline the state machine would call next_physical for this line
        // (since it's inside a fence), so the join would not occur.
        let input = vec![bsl("```"), "next line".to_string()];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "```next line");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 1);
    }

    // =========================================================================
    // consume_logical — no separator insertion (POSIX semantics)
    // Engine Spec §7.1: "No separator character is inserted."
    // =========================================================================

    #[test]
    fn consume_logical_does_not_insert_separator_space() {
        // Engine Spec §7.1: joining concatenates directly with no separator.
        // This test guards against regression to the v0.3.0 space-insertion
        // behavior (Engine Spec §16.1).
        let ls = vec![bsl("x"), "y".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text.len(), 2);
        assert_eq!(ll.text.as_bytes(), b"xy");
    }

    // =========================================================================
    // consume_logical — multi-backslash content preservation
    // RFC §3.2: "The join marker is any '\' immediately before the line
    // boundary, regardless of what precedes it."
    // =========================================================================

    #[test]
    fn consume_logical_double_backslash_preserves_content_backslash() {
        // RFC §3.2: only the final '\' (immediately before LF) is the join
        // marker. A preceding '\' is literal content and must be preserved.
        // Input: "foo\\" (two backslashes) + "bar"
        // After removing the final '\' and joining: "foo\bar"
        let mut line = "foo\\".to_string();
        line.push('\\'); // "foo\\" — two trailing backslashes
        let ls = vec![line, "bar".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "foo\\bar");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 1);
    }

    #[test]
    fn consume_logical_triple_backslash_joins_and_preserves_two() {
        // RFC §3.2: three trailing backslashes. The final one is the join
        // marker; after removal and joining, the result still ends with '\'
        // so another join occurs.
        // Input: "a\\\" + "b" → after first join: "a\\" + "b" → "a\\b"
        //   wait — let's be precise:
        //   Physical line 0: "a\\\" (three chars after 'a': \, \, \)
        //   Step 1: ends with '\', remove it → "a\\"
        //   Step 2: "a\\" still ends with '\', remove it → "a\"  — wait,
        //   no. After removing the last '\' and concatenating line 1:
        //   "a\\" + "b" = "a\\b". Then check: does "a\\b" end with '\'? No.
        //   So the result is "a\\b".
        //
        // Actually: "a\\\" is 4 bytes: a, \, \, \.
        //   ends_with('\') → true. Truncate last → "a\\"
        //   Concat next line "b" → "a\\b"
        //   ends_with('\') on "a\\b"? No → done.
        let mut line = "a".to_string();
        line.push('\\');
        line.push('\\');
        line.push('\\');
        let ls = vec![line, "b".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "a\\\\b");
        assert_eq!(ll.last_physical, 1);
    }

    // =========================================================================
    // consume_logical — backslash-only line
    // RFC §3.2: empty remainder joins directly with the next physical line.
    // =========================================================================

    #[test]
    fn consume_logical_backslash_only_line_joins_with_next() {
        // RFC §3.2: a line containing only '\' has empty remainder after
        // removing the join marker. Direct concatenation (no separator per
        // Engine Spec §7.1) produces the next line's content unchanged.
        let ls = vec![bsl(""), "hello".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "hello");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 1);
    }

    #[test]
    fn consume_logical_backslash_only_at_eof() {
        // RFC §3.2: backslash-only line at EOF. The '\' is removed; the line
        // stands alone as an empty string.
        let ls = vec![bsl("")];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 0);
    }

    // =========================================================================
    // consume_logical — whitespace-only continuation line
    // RFC §3.2: no trimming of continuation lines; all content is preserved.
    // =========================================================================

    #[test]
    fn consume_logical_whitespace_only_continuation_preserves_spaces() {
        // RFC §3.2: the continuation line is concatenated verbatim. No
        // trimming occurs. Engine Spec §7.1: no separator inserted.
        let ls = vec![bsl("cmd"), "   ".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "cmd   ");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 1);
    }

    #[test]
    fn consume_logical_tab_continuation_preserved() {
        // RFC §3.2 + Engine Spec §6: HTAB is valid whitespace. The joiner
        // must not strip or replace it.
        let ls = vec![bsl("cmd"), "\targ".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "cmd\targ");
    }

    // =========================================================================
    // consume_logical — trailing whitespace before backslash
    // Engine Spec §7.1: content before the '\' is preserved as-is.
    // =========================================================================

    #[test]
    fn consume_logical_trailing_space_before_backslash_preserved() {
        // Engine Spec §7.1: the joiner removes only the final '\'. Any
        // trailing space before it is part of the content and is preserved.
        let ls = vec![bsl("cmd "), "arg".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "cmd arg");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 1);
    }

    // =========================================================================
    // consume_logical — empty lines in join chains
    // RFC §3.2: joining behavior does not depend on line content.
    // =========================================================================

    #[test]
    fn consume_logical_empty_continuation_line() {
        // RFC §3.2: empty continuation line produces no additional content.
        // Engine Spec §7.1: no separator, so result is just the first line's
        // content (without the backslash).
        let ls = vec![bsl("prefix"), "".to_string()];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "prefix");
        assert_eq!(ll.last_physical, 1);
    }

    // =========================================================================
    // consume_logical — chained EOF backslash after join
    // RFC §3.2 step 4: repeat if result still ends with '\'.
    // =========================================================================

    #[test]
    fn consume_logical_mid_chain_eof_removes_backslash() {
        // RFC §3.2: two physical lines both end with '\', but there is no
        // third line. The first join succeeds; the second '\' hits EOF and
        // is silently removed.
        let ls = vec![bsl("a"), bsl("b")];
        let mut cursor = 0;
        let ll = consume_logical(&ls, &mut cursor).unwrap();
        assert_eq!(ll.text, "ab");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 1);
    }

    // =========================================================================
    // LineJoiner — interleaving modes with shared cursor
    // Engine Spec §5.3: idle uses next_logical, in-fence uses next_physical.
    // =========================================================================

    #[test]
    fn physical_then_logical_shares_cursor() {
        // Engine Spec §5.3 / §7.2: after consuming physical lines (fence body),
        // switching back to next_logical resumes at the correct position.
        let input = vec!["fence body".to_string(), "```".to_string(), bsl("/cmd arg"), "rest".to_string()];
        let mut j = LineJoiner::new(input);
        // Simulate in-fence: consume two physical lines.
        let (idx0, _) = j.next_physical().unwrap();
        assert_eq!(idx0, 0);
        let (idx1, _) = j.next_physical().unwrap();
        assert_eq!(idx1, 1);
        // Transition to idle: next_logical joins lines 2-3.
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/cmd argrest");
        assert_eq!(ll.first_physical, 2);
        assert_eq!(ll.last_physical, 3);
        assert!(j.is_exhausted());
    }

    // =========================================================================
    // RFC Appendix B.2 — updated assertion for no-separator semantics
    // Engine Spec §7.1 + RFC Appendix B.2
    // =========================================================================

    #[test]
    fn appendix_b2_with_leading_whitespace_on_continuations() {
        // RFC Appendix B.2 shows continuation lines indented with spaces.
        // After joining (no separator), the leading spaces on continuation
        // lines provide the visual separation.
        // Input:
        //   "/deploy production \"   (trailing space before \)
        //   "  --region us-west-2 \"  (two leading spaces, trailing space before \)
        //   "  --canary"             (two leading spaces)
        // Result: "production " + "  --region..." + "  --canary"
        let input = vec![bsl("/deploy production "), bsl("  --region us-west-2 "), "  --canary".to_string()];
        let mut j = LineJoiner::new(input);
        let ll = j.next_logical().unwrap();
        assert_eq!(ll.text, "/deploy production   --region us-west-2   --canary");
        assert_eq!(ll.first_physical, 0);
        assert_eq!(ll.last_physical, 2);
    }

    // =========================================================================
    // Property tests
    // =========================================================================

    proptest! {
        // RFC §3.2: joining only merges lines, never creates new ones.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn logical_count_lte_physical_count(
            ls in prop::collection::vec("[a-zA-Z0-9 ]{0,40}", 0..20)
        ) {
            let count = ls.len();
            let mut cursor = 0;
            let mut logical_count = 0;
            while consume_logical(&ls, &mut cursor).is_some() {
                logical_count += 1;
            }
            prop_assert!(logical_count <= count);
        }

        // RFC §3.2: lines without '\' pass through unchanged.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn clean_lines_pass_through_unchanged(
            ls in prop::collection::vec("[a-zA-Z0-9 !.,]{1,40}", 1..10)
        ) {
            let expected = ls.clone();
            let mut cursor = 0;
            for expected_text in expected {
                let ll = consume_logical(&ls, &mut cursor).unwrap();
                prop_assert_eq!(ll.text, expected_text);
            }
        }

        // RFC §3.3: logical lines must partition physical lines without gaps.
        // Engine Spec §7.3: first_physical/last_physical cover the range.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn ranges_cover_all_physical_lines(
            ls in prop::collection::vec("[a-zA-Z0-9 ]{0,40}", 1..20)
        ) {
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

        // Structural: is_exhausted true after consuming all lines.
        #[test]
        #[cfg_attr(feature = "tdd", ignore)]
        fn exhausted_after_consuming_all_logical_lines(
            ls in prop::collection::vec("[a-zA-Z0-9 ]{0,40}", 0..20)
        ) {
            let mut j = LineJoiner::new(ls);
            while j.next_logical().is_some() {}
            prop_assert!(j.is_exhausted());
        }
    }
}
