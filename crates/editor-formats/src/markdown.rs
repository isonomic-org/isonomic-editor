use editor_model::{ATOM_CHAR, Block, BlockId, Content, Document, InlineText, PropValue, kinds};

use crate::json::{block_to_json, prop_text};

/// The info-string prefix of a fenced block that carries a block
/// with no Markdown form, as canonical JSON.
pub(crate) const FENCE_TAG: &str = "isonomic:";

fn escape(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '\\' | '`' | '*' | '_' | '[' | ']' | '~' | '<' | '=' => {
                out.push('\\');
                out.push(ch);
            }
            ATOM_CHAR => {}
            other => out.push(other),
        }
    }
}

pub(crate) fn inline(text: &InlineText) -> String {
    let mut out = String::new();
    for (span, run) in text.spans() {
        let marks = &run.marks;
        let wraps: [(bool, &str, &str); 5] = [
            (marks.bold, "**", "**"),
            (marks.italic, "*", "*"),
            (marks.strike, "~~", "~~"),
            (marks.underline, "<u>", "</u>"),
            (marks.highlight.is_some(), "==", "=="),
        ];
        if marks.link.is_some() {
            out.push('[');
        }
        for (_, open, _) in wraps.iter().filter(|(on, ..)| *on) {
            out.push_str(open);
        }
        match (&run.atom, marks.code) {
            (Some(atom), _) => {
                let label =
                    atom.attrs.get("label").map(prop_text).unwrap_or_else(|| atom.kind.clone());
                escape(&format!("@{label}"), &mut out);
            }
            (None, true) => {
                out.push('`');
                out.push_str(&text.text()[span].replace('`', "'"));
                out.push('`');
            }
            (None, false) => escape(&text.text()[span], &mut out),
        }
        for (_, _, close) in wraps.iter().rev().filter(|(on, ..)| *on) {
            out.push_str(close);
        }
        if let Some(url) = &marks.link {
            out.push_str(&format!("]({})", url.replace(')', "%29")));
        }
    }
    out
}

fn fence_for(body: &str) -> String {
    let longest = body.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

fn write_block(doc: &Document, block: &Block, number: usize, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    let text = match &block.content {
        Content::Inline(text) => inline(text),
        Content::Plain(text) => text.clone(),
        Content::None => String::new(),
    };
    let int = |key: &str| block.props.get(key).and_then(PropValue::as_int);
    let flag = |key: &str| block.props.get(key).and_then(PropValue::as_bool).unwrap_or(false);
    let known = doc.registry().is_known(&block.kind);
    match block.kind.as_str() {
        kinds::PARAGRAPH if known => out.push_str(&format!("{pad}{text}\n\n")),
        kinds::HEADING if known => {
            let level = int("level").unwrap_or(1).clamp(1, 3) as usize;
            out.push_str(&format!("{pad}{} {text}\n\n", "#".repeat(level)));
        }
        kinds::BULLETED if known => out.push_str(&format!("{pad}- {text}\n")),
        kinds::NUMBERED if known => out.push_str(&format!("{pad}{number}. {text}\n")),
        kinds::TODO if known => {
            let mark = if flag("checked") { "x" } else { " " };
            out.push_str(&format!("{pad}- [{mark}] {text}\n"));
        }
        kinds::QUOTE if known => {
            for line in text.split('\n') {
                out.push_str(&format!("{pad}> {line}\n"));
            }
            out.push('\n');
        }
        kinds::DIVIDER if known => out.push_str(&format!("{pad}---\n\n")),
        kinds::CODE if known => {
            let fence = fence_for(&text);
            let language = block.props.get("language").map(prop_text).unwrap_or_default();
            out.push_str(&format!("{pad}{fence}{language}\n"));
            for line in text.split('\n') {
                out.push_str(&format!("{pad}{line}\n"));
            }
            out.push_str(&format!("{pad}{fence}\n\n"));
        }
        other => {
            // No Markdown form: the whole block, children included,
            // travels as canonical JSON in a tagged fence.
            let body = serde_json::to_string(&block_to_json(doc, block)).unwrap_or_default();
            let fence = fence_for(&body);
            out.push_str(&format!("{pad}{fence}{FENCE_TAG}{other}\n{pad}{body}\n{pad}{fence}\n\n"));
            return;
        }
    }
    write_blocks(doc, &block.children, indent + 2, out);
}

fn is_list(block: &Block) -> bool {
    [kinds::BULLETED, kinds::NUMBERED, kinds::TODO].iter().any(|kind| block.kind.is(kind))
}

fn write_blocks(doc: &Document, ids: &[BlockId], indent: usize, out: &mut String) {
    let mut number = 0;
    let mut in_list = false;
    let mut previous: Option<&Block> = None;
    for block in ids.iter().filter_map(|id| doc.get(*id)) {
        number = if block.kind.is(kinds::NUMBERED) { number + 1 } else { 0 };
        // A blank line ends a list, and at the top level separates
        // lists of different kinds.
        let changed = previous.is_some_and(|before| before.kind != block.kind);
        if in_list && (!is_list(block) || (indent == 0 && changed)) {
            out.push('\n');
        }
        in_list = is_list(block);
        previous = Some(block);
        write_block(doc, block, number, indent, out);
    }
    if in_list && indent == 0 {
        out.push('\n');
    }
}

/// Markdown for the document. Blocks with no Markdown form are
/// written as tagged fenced blocks so they survive a round trip.
pub fn to_markdown(doc: &Document) -> String {
    let mut out = String::new();
    write_blocks(doc, doc.root(), 0, &mut out);
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out.push('\n');
    out
}
