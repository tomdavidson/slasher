use crate::types::*;

pub fn sanitize(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

pub fn sanitize_no_backticks(s: &str) -> String {
    sanitize(s).replace('`', "'")
}

pub fn render_cmd_name(raw: &[u8]) -> String {
    if raw.is_empty() {
        return "cmd".to_string();
    }

    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789-";
    let first = (b'a' + (raw[0] % 26)) as char;

    let mut name: String = std::iter::once(first)
        .chain(
            raw[1..]
                .iter()
                .map(|&b| alphabet[(b as usize) % alphabet.len()] as char),
        )
        .collect();

    while name.ends_with('-') {
        name.pop();
    }

    if name.is_empty() {
        "cmd".to_string()
    } else {
        name
    }
}

fn trailing_backslash_count(s: &str) -> usize {
    s.as_bytes().iter().rev().take_while(|&&b| b == b'\\').count()
}

fn is_join_marker(s: &str) -> bool {
    trailing_backslash_count(s) % 2 == 1
}


// pub fn render_text_line(content: &str) -> String {
//     let s = sanitize(content);

//     if s.starts_with('/') {
//         format!(" {s}")
//     } else if s.is_empty() {
//         "some text".to_string()
//     } else {
//         s
//     }
// }

pub fn render_text_line(content: &str) -> String {
    let s = sanitize_no_backticks(content);
    if s.trim_start().starts_with('/') {
        format!("x{s}")
    } else if s.is_empty() {
        "some text".to_string()
    } else {
        s
    }
}

pub fn render_fence_lang(lang: &Option<String>) -> String {
    let Some(l) = lang else {
        return String::new();
    };

    let clean: String = l
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(10)
        .collect();

    if clean.is_empty() {
        String::new()
    } else {
        format!(" {clean}")
    }
}

pub fn render_fence_body(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| {
            let s = sanitize(l);
            let trimmed = s.trim_matches(|c: char| c == ' ' || c == '\t');
            if !trimmed.is_empty() && trimmed.len() >= 3 && trimmed.chars().all(|c| c == '`') {
                format!("{s}x")
            } else {
                s
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_invalid_slash(kind: &InvalidSlashKind) -> String {
    match kind {
        InvalidSlashKind::BareSlash => "/".to_string(),
        InvalidSlashKind::NumericAfterSlash => "/123".to_string(),
        InvalidSlashKind::Capitalized => "/Hello".to_string(),
        InvalidSlashKind::TrailingHyphen => "/cmd-".to_string(),
    }
}

pub fn render_fragment(frag: &Fragment) -> Vec<String> {
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
            let hdr = if h.is_empty() {
                String::new()
            } else {
                format!("{h} ")
            };
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
            let hdr = if h.is_empty() {
                String::new()
            } else {
                format!("{h} ")
            };
            let b = render_fence_body(&body.lines);

            let mut lines = vec![format!("/{n} {hdr}```")];
            if !b.is_empty() {
                lines.push(b);
            }
            lines
        }

        Fragment::JoinedCmd(name, parts) => {
            let n = render_cmd_name(&name.raw);
            let rendered: Vec<String> = parts
                .iter()
                .map(|p| sanitize_no_backticks(&p.text))
                .collect();

            if rendered.is_empty() {
                return vec![format!("/{n}")];
            }
            if rendered.len() == 1 {
                return vec![format!("/{n} {}", rendered[0])];
            }

            let last = rendered.len() - 1;
            rendered
                .iter()
                .enumerate()
                .map(|(i, part)| match i {
                    0 => format!("/{n} {part}\\"),
                    _ if i == last => part.clone(),
                    _ => format!("{part}\\"),
                })
                .collect()
        }

        Fragment::InvalidSlash(kind) => vec![render_invalid_slash(kind)],

        Fragment::Blank => vec![String::new()],
    }
}

pub fn render_doc(doc: &FuzzDoc) -> String {
    let mut lines: Vec<String> = doc
        .fragments
        .iter()
        .take(MAX_FRAGMENTS)
        .flat_map(render_fragment)
        .collect();

    // Strip trailing join markers from non-final lines so that
    // backslash continuation cannot merge independently generated fragments.
    let last = lines.len().saturating_sub(1);
    for line in &mut lines[..last] {
        if is_join_marker(line) {
            line.truncate(line.len() - 1);
        }
    }

    lines.join("\n")
}