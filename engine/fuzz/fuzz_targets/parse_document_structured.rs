//! Fuzz target: parse_document_structured
//!
//! Generates structured inputs that exercise specific grammar productions
//! from the Syntax RFC (Appendix A) rather than relying on raw byte
//! mutation alone. This reaches fence mode, line joining, and mixed
//! document states faster than unstructured fuzzing.

#![no_main]

#[cfg(feature = "libfuzzer")]
use libfuzzer_sys::fuzz_target;

#[cfg(feature = "libafl")]
use libafl_libfuzzer::fuzz_target;

use solidus_engine::{parse_document, ArgumentMode};

const MAX_FRAGMENTS: usize = 20;
const MAX_JOIN_PARTS: usize = 5;
const MAX_BODY_LINES: usize = 20;

// ---------------------------------------------------------------------------
// Domain types for grammar-aware generation
// ---------------------------------------------------------------------------

#[derive(arbitrary::Arbitrary, Debug)]
struct FuzzDoc {
    fragments: Vec<Fragment>,
}

#[derive(arbitrary::Arbitrary, Debug)]
enum Fragment {
    Text(TextLine),
    SingleLineCmd(CmdName, Payload),
    /// A fenced command with opener, body, and closer.
    FencedCmd(CmdName, Header, FenceLang, FenceBody),
    UnclosedFence(CmdName, Header, FenceBody),
    JoinedCmd(CmdName, Vec<Payload>),
    InvalidSlash(InvalidSlashKind),
    Blank,
}

#[derive(arbitrary::Arbitrary, Debug)]
struct CmdName {
    raw: Vec<u8>,
}

#[derive(arbitrary::Arbitrary, Debug)]
struct TextLine {
    content: String,
}

#[derive(arbitrary::Arbitrary, Debug)]
struct Payload {
    text: String,
}

#[derive(arbitrary::Arbitrary, Debug)]
struct Header {
    text: String,
}

#[derive(arbitrary::Arbitrary, Debug)]
struct FenceLang {
    lang: Option<String>,
}

#[derive(arbitrary::Arbitrary, Debug)]
struct FenceBody {
    lines: Vec<String>,
}

#[derive(arbitrary::Arbitrary, Debug)]
enum InvalidSlashKind {
    BareSlash,
    NumericAfterSlash,
    Capitalized,
    TrailingHyphen,
}

// ---------------------------------------------------------------------------
// Rendering: each type -> String fragment
// ---------------------------------------------------------------------------

fn sanitize(s: &str) -> String {
    s.replace('\n', " ").replace('\r', " ")
}

fn sanitize_no_backticks(s: &str) -> String {
    sanitize(s).replace('`', "'")
}

fn render_cmd_name(raw: &[u8]) -> String {
    if raw.is_empty() {
        return "cmd".to_string();
    }

    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789-";
    let first = (b'a' + (raw[0] % 26)) as char;

    let mut name: String = std::iter::once(first)
        .chain(raw[1..].iter().map(|&b| alphabet[(b as usize) % alphabet.len()] as char))
        .collect();

    while name.ends_with('-') {
        name.pop();
    }

    if name.is_empty() { "cmd".to_string() } else { name }
}

fn render_text_line(content: &str) -> String {
    let s = sanitize(content);
    if s.starts_with('/') {
        format!(" {s}")
    } else if s.is_empty() {
        "some text".to_string()
    } else {
        s
    }
}

fn render_fence_lang(lang: &Option<String>) -> String {
    let Some(l) = lang else { return String::new() };

    let clean: String = l.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(10)
        .collect();

    if clean.is_empty() { String::new() } else { format!(" {clean}") }
}

fn render_fence_body(lines: &[String]) -> String {
    lines.iter()
        .take(MAX_BODY_LINES)
        .map(|l| sanitize(l))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_invalid_slash(kind: &InvalidSlashKind) -> String {
    match kind {
        InvalidSlashKind::BareSlash => "/".to_string(),
        InvalidSlashKind::NumericAfterSlash => "/123".to_string(),
        InvalidSlashKind::Capitalized => "/Hello".to_string(),
        InvalidSlashKind::TrailingHyphen => "/cmd-".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Fragment -> line(s)
// ---------------------------------------------------------------------------

fn render_fragment(frag: &Fragment) -> Vec<String> {
    match frag {
        Fragment::Text(t) => vec![render_text_line(&t.content)],

        Fragment::SingleLineCmd(name, payload) => {
            let n = render_cmd_name(&name.raw);
            let p = sanitize_no_backticks(&payload.text);
            if p.is_empty() {
                vec![format!("/{n}")]
            } else {
                vec![format!("/{n} {p}")]
            }
        }

        Fragment::FencedCmd(name, header, lang, body) => {
            let n = render_cmd_name(&name.raw);
            let h = sanitize_no_backticks(&header.text);
            let hdr = if h.is_empty() { String::new() } else { format!("{h} ") };
            let l = render_fence_lang(&lang.lang);
            let b = render_fence_body(&body.lines);

            let mut lines = vec![format!("/{n} {hdr}```{l}")];
            if !b.is_empty() {
                lines.push(b);
            }
            lines.push("```".to_string());
            lines
        }

        Fragment::UnclosedFence(name, header, body) => {
            let n = render_cmd_name(&name.raw);
            let h = sanitize_no_backticks(&header.text);
            let hdr = if h.is_empty() { String::new() } else { format!("{h} ") };
            let b = render_fence_body(&body.lines);

            let mut lines = vec![format!("/{n} {hdr}```")];
            if !b.is_empty() {
                lines.push(b);
            }
            lines
        }

        Fragment::JoinedCmd(name, parts) => {
            let n = render_cmd_name(&name.raw);
            let rendered: Vec<String> = parts.iter()
                .take(MAX_JOIN_PARTS)
                .map(|p| sanitize_no_backticks(&p.text))
                .collect();

            if rendered.is_empty() {
                return vec![format!("/{n}")];
            }
            if rendered.len() == 1 {
                return vec![format!("/{n} {}", rendered[0])];
            }

            let last = rendered.len() - 1;
            rendered.iter().enumerate().map(|(i, part)| {
                match i {
                    0 => format!("/{n} {part}\\"),
                    _ if i == last => part.clone(),
                    _ => format!("{part}\\"),
                }
            }).collect()
        }

        Fragment::InvalidSlash(kind) => vec![render_invalid_slash(kind)],

        Fragment::Blank => vec![String::new()],
    }
}

fn render_doc(doc: &FuzzDoc) -> String {
    doc.fragments.iter()
        .take(MAX_FRAGMENTS)
        .flat_map(render_fragment)
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

fn assert_ids_sequential(result: &solidus_engine::ParseResult) {
    for (i, cmd) in result.commands.iter().enumerate() {
        assert_eq!(cmd.id, format!("cmd-{i}"));
    }
    for (i, tb) in result.textblocks.iter().enumerate() {
        assert_eq!(tb.id, format!("text-{i}"));
    }
}

fn assert_argument_modes(result: &solidus_engine::ParseResult) {
    for cmd in &result.commands {
        assert!(
            cmd.arguments.mode == ArgumentMode::SingleLine
                || cmd.arguments.mode == ArgumentMode::Fence
        );
    }
}

fn assert_unclosed_fence_warning(doc: &FuzzDoc, result: &solidus_engine::ParseResult) {
    let last_is_unclosed = doc.fragments.iter()
        .take(MAX_FRAGMENTS)
        .last()
        .is_some_and(|f| matches!(f, Fragment::UnclosedFence(..)));

    if last_is_unclosed {
        assert!(
            result.warnings.iter().any(|w| w.wtype == "unclosed_fence"),
            "last fragment is UnclosedFence but no unclosed_fence warning emitted"
        );
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fuzz_target!(|doc: FuzzDoc| {
    let input = render_doc(&doc);
    let result = parse_document(&input);

    assert!(!result.version.is_empty());
    assert_ids_sequential(&result);
    assert_argument_modes(&result);
    assert_unclosed_fence_warning(&doc, &result);

    // §12.1: determinism.
    let result2 = parse_document(&input);
    assert_eq!(result, result2);
});
