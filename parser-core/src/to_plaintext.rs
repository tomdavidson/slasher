use crate::domain::{ArgumentMode, SlashParseResult};

pub fn to_plaintext(result: &SlashParseResult) -> String {
    #[derive(Debug)]
    enum Item<'a> {
        Command(&'a crate::domain::Command),
        Text(&'a crate::domain::TextBlock),
    }

    let mut items: Vec<Item> = Vec::new();
    for cmd in &result.commands {
        items.push(Item::Command(cmd));
    }
    for txt in &result.text_blocks {
        items.push(Item::Text(txt));
    }

    items.sort_by_key(|item| match item {
        Item::Command(c) => c.range.start_line,
        Item::Text(t) => t.range.start_line,
    });

    let mut output = String::new();

    for (i, item) in items.iter().enumerate() {
        match item {
            Item::Command(c) => match c.arguments.mode {
                ArgumentMode::SingleLine => {
                    if c.arguments.payload.is_empty() {
                        output.push_str(&format!("/{} ", c.name));
                    } else {
                        output.push_str(&format!("/{} {}", c.name, c.arguments.payload));
                    }
                }
                ArgumentMode::Continuation => {
                    output.push_str(&format!(
                        "/{} \\
{}",
                        c.name, c.arguments.payload
                    ));
                }
                ArgumentMode::Fence => {
                    let lang = c.arguments.fence_lang.as_deref().unwrap_or("");
                    output.push_str(&format!(
                        "/{}
```{}
{}
```",
                        c.name, lang, c.arguments.payload
                    ));
                }
            },
            Item::Text(t) => {
                output.push_str(&t.content);
            }
        }

        if i < items.len() - 1 {
            output.push('\n');
        }
    }

    output
}
