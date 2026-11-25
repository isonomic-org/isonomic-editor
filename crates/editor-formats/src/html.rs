use editor_model::{Block, Content, Document, InlineText, PropValue, kinds};

fn escape(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            editor_model::ATOM_CHAR => {}
            other => out.push(other),
        }
    }
}

/// Links are written only for schemes that are safe to follow.
fn safe_link(url: &str) -> bool {
    ["https://", "http://", "mailto:"].iter().any(|scheme| url.starts_with(scheme))
}

fn inline(text: &InlineText, out: &mut String) {
    for (span, run) in text.spans() {
        let marks = &run.marks;
        let link = marks.link.as_deref().filter(|url| safe_link(url));
        let tags: [(bool, &str); 6] = [
            (marks.bold, "strong"),
            (marks.italic, "em"),
            (marks.underline, "u"),
            (marks.strike, "s"),
            (marks.highlight.is_some(), "mark"),
            (marks.code, "code"),
        ];
        if let Some(url) = link {
            out.push_str("<a href=\"");
            escape(url, out);
            out.push_str("\">");
        }
        for (_, tag) in tags.iter().filter(|(on, _)| *on) {
            out.push_str(&format!("<{tag}>"));
        }
        match &run.atom {
            Some(atom) => escape(&format!("@{}", atom.kind), out),
            None => escape(&text.text()[span], out),
        }
        for (_, tag) in tags.iter().rev().filter(|(on, _)| *on) {
            out.push_str(&format!("</{tag}>"));
        }
        if link.is_some() {
            out.push_str("</a>");
        }
    }
}

fn list_tag(block: &Block) -> Option<&'static str> {
    match block.kind.as_str() {
        kinds::BULLETED | kinds::TODO => Some("ul"),
        kinds::NUMBERED => Some("ol"),
        _ => None,
    }
}

fn blocks(doc: &Document, ids: &[editor_model::BlockId], out: &mut String) {
    let mut open: Option<&'static str> = None;
    for block in ids.iter().filter_map(|id| doc.get(*id)) {
        let list = list_tag(block);
        if open != list {
            if let Some(tag) = open {
                out.push_str(&format!("</{tag}>\n"));
            }
            if let Some(tag) = list {
                out.push_str(&format!("<{tag}>\n"));
            }
            open = list;
        }
        let tag = match block.kind.as_str() {
            kinds::HEADING => {
                match block.props.get("level").and_then(PropValue::as_int).unwrap_or(1) {
                    1 => "h1",
                    2 => "h2",
                    _ => "h3",
                }
            }
            kinds::QUOTE => "blockquote",
            kinds::DIVIDER => {
                out.push_str("<hr>\n");
                continue;
            }
            kinds::CODE => "pre",
            _ if list.is_some() => "li",
            _ => "p",
        };
        out.push_str(&format!("<{tag}>"));
        if block.kind.is(kinds::TODO) {
            let checked = block.props.get("checked").and_then(PropValue::as_bool).unwrap_or(false);
            out.push_str("<input type=\"checkbox\" disabled");
            out.push_str(if checked { " checked> " } else { "> " });
        }
        match &block.content {
            Content::Inline(text) => inline(text, out),
            Content::Plain(text) => {
                out.push_str("<code>");
                escape(text, out);
                out.push_str("</code>");
            }
            Content::None => {}
        }
        if !block.children.is_empty() {
            out.push('\n');
            blocks(doc, &block.children, out);
        }
        out.push_str(&format!("</{tag}>\n"));
    }
    if let Some(tag) = open {
        out.push_str(&format!("</{tag}>\n"));
    }
}

/// HTML for publishing and the clipboard. Text is escaped and links
/// are limited to safe schemes.
pub fn to_html(doc: &Document) -> String {
    let mut out = String::new();
    blocks(doc, doc.root(), &mut out);
    out
}
