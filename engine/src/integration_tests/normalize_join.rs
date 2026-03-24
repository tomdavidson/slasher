//! normalize + join composition tests.
//!
//! Verify that line-ending normalization (§2.1) feeds correctly into
//! backslash line joining (§2.2) regardless of CR/CRLF/LF style.

use crate::{join::LineJoiner, normalize::normalize};

#[test]
fn crlf_continuation_joins_same_as_lf() {
    // §2.1 rules 1-2: CRLF and bare CR both normalize to LF before any other processing.
    // §2.2: line joining runs after normalization, so a backslash before a CRLF boundary
    // must produce the same logical line as the equivalent LF-only input.
    let crlf = "/deploy production\\\r\n --region us-west-2";
    let lf = "/deploy production\\\n --region us-west-2";

    let crlf_lines: Vec<String> = normalize(crlf).split('\n').map(|s| s.to_string()).collect();
    let lf_lines: Vec<String> = normalize(lf).split('\n').map(|s| s.to_string()).collect();

    let crlf_ll = LineJoiner::new(crlf_lines).next_logical().unwrap();
    let lf_ll = LineJoiner::new(lf_lines).next_logical().unwrap();

    assert_eq!(crlf_ll.text, lf_ll.text);
    assert_eq!(crlf_ll.first_physical, lf_ll.first_physical);
    assert_eq!(crlf_ll.last_physical, lf_ll.last_physical);
}

#[test]
fn bare_cr_continuation_joins_same_as_lf() {
    // §2.1 rule 2: remaining bare CR characters are replaced with LF after CRLF removal.
    // §2.2: joining is agnostic to the original line-ending style.
    let cr = "/deploy production\\\r --region us-west-2";
    let lf = "/deploy production\\\n --region us-west-2";

    let cr_lines: Vec<String> = normalize(cr).split('\n').map(|s| s.to_string()).collect();
    let lf_lines: Vec<String> = normalize(lf).split('\n').map(|s| s.to_string()).collect();

    let cr_ll = LineJoiner::new(cr_lines).next_logical().unwrap();
    let lf_ll = LineJoiner::new(lf_lines).next_logical().unwrap();

    assert_eq!(cr_ll.text, lf_ll.text);
}

#[test]
fn mixed_crlf_multi_line_join_matches_lf() {
    // §2.1: normalization applies to all line endings uniformly.
    // §2.2 step 4: joining repeats while the accumulated line still ends with `\`,
    // so three physical lines collapse into one logical line regardless of ending style.
    // §2.2.1: last_physical must be the zero-based index of the last consumed physical line.
    let crlf = "/mcp call_tool read_file\\\r\n --path src/index.ts\\\r\n --format json";
    let lf = "/mcp call_tool read_file\\\n --path src/index.ts\\\n --format json";

    let crlf_lines: Vec<String> = normalize(crlf).split('\n').map(|s| s.to_string()).collect();
    let lf_lines: Vec<String> = normalize(lf).split('\n').map(|s| s.to_string()).collect();

    let crlf_ll = LineJoiner::new(crlf_lines).next_logical().unwrap();
    let lf_ll = LineJoiner::new(lf_lines).next_logical().unwrap();

    assert_eq!(crlf_ll.text, lf_ll.text);
    assert_eq!(crlf_ll.first_physical, 0);
    assert_eq!(crlf_ll.last_physical, 2);
}
