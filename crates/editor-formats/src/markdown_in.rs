use std::sync::Arc;

use editor_model::{
    BlockDraft, BlockId, Document, IdSource, InlineText, Mark, Marks, Op, PropValue, Registry,
    kinds,
};
use serde_json::Value;

use crate::json::draft_from;
use crate::markdown::FENCE_TAG;

const DELIMITERS: [(&str, &str, Option<Mark>); 6] = [
    ("**", "**", Some(Mark::Bold)),
    ("__", "__", Some(Mark::Bold)),
    ("~~", "~~", Some(Mark::Strike)),
    ("<u>", "</u>", Some(Mark::Underline)),
    ("*", "*", Some(Mark::Italic)),
    ("_", "_", Some(Mark::Italic)),
];

/// The byte index of the next unescaped `close` in `text`.
fn find_close(text: &str, close: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(found) = text[from..].find(close) {
        let at = from + found;
        let escaped = text[..at].chars().rev().take_while(|ch| *ch == '\\').count() % 2 == 1;
        let doubled = close == "*" && text[at..].starts_with("**");
        if !escaped && !doubled && at > 0 {
            return Some(at);
        }
        from = at + if doubled { 2 } else { close.len() };
        if from >= text.len() {
            break;
        }
    }
    None
}

/// Parses inline Markdown. A delimiter with no partner is literal.
pub(crate) fn parse_inline(source: &str, marks: &Marks, out: &mut InlineText) {
    let mut plain = String::new();
    let mut rest = source;
    let flush = |plain: &mut String, out: &mut InlineText| {
        out.append(&InlineText::styled(plain, marks.clone()));
        plain.clear();
    };
    'scan: while let Some(ch) = rest.chars().next() {
        if ch == '\\' {
            let mut chars = rest.chars();
            chars.next();
            match chars.next() {
                Some(next) if next.is_ascii_punctuation() => plain.push(next),
                Some(next) => plain.extend(['\\', next]),
                None => plain.push('\\'),
            }
            rest = chars.as_str();
            continue;
        }
        if ch == '`'
            && let Some(end) = rest[1..].find('`')
        {
            flush(&mut plain, out);
            out.append(&InlineText::styled(&rest[1..1 + end], marks.clone().with(Mark::Code)));
            rest = &rest[end + 2..];
            continue;
        }
        if ch == '['
            && let Some(mid) = find_close(rest, "](")
            && let Some(end) = rest[mid..].find(')')
        {
            flush(&mut plain, out);
            let url = rest[mid + 2..mid + end].replace("%29", ")");
            parse_inline(&rest[1..mid], &marks.clone().with(Mark::Link(url)), out);
            rest = &rest[mid + end + 1..];
            continue;
        }
        if rest.starts_with("==")
            && let Some(end) = find_close(&rest[2..], "==")
        {
            flush(&mut plain, out);
            let mark = Mark::Highlight("default".into());
            parse_inline(&rest[2..2 + end], &marks.clone().with(mark), out);
            rest = &rest[end + 4..];
            continue;
        }
        for (open, close, mark) in &DELIMITERS {
            if rest.starts_with(open)
                && !rest[open.len()..].starts_with(char::is_whitespace)
                && let Some(end) = find_close(&rest[open.len()..], close)
                && let Some(mark) = mark
            {
                flush(&mut plain, out);
                let inner = &rest[open.len()..open.len() + end];
                parse_inline(inner, &marks.clone().with(mark.clone()), out);
                rest = &rest[open.len() + end + close.len()..];
                continue 'scan;
            }
        }
        plain.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    flush(&mut plain, out);
}

fn rich(source: &str) -> InlineText {
    let mut out = InlineText::new();
    parse_inline(source.trim(), &Marks::default(), &mut out);
    out
}

/// A list line: its indent, its block type, a checkbox state and the
/// item text.
fn list_item(line: &str) -> Option<(usize, &'static str, Option<bool>, &str)> {
    let body = line.trim_start_matches(' ');
    let indent = line.len() - body.len();
    for (prefix, checked) in [("- [ ] ", false), ("- [x] ", true), ("- [X] ", true)] {
        if let Some(text) = body.strip_prefix(prefix) {
            return Some((indent, kinds::TODO, Some(checked), text));
        }
    }
    for bullet in ["- ", "* ", "+ "] {
        if let Some(text) = body.strip_prefix(bullet) {
            return Some((indent, kinds::BULLETED, None, text));
        }
    }
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    let text = body.get(digits..)?.strip_prefix(". ")?;
    (digits > 0 && digits < 10).then_some((indent, kinds::NUMBERED, None, text))
}

struct Reader<'a> {
    doc: Document,
    ids: &'a mut dyn IdSource,
    /// Open list items: (indent, block).
    lists: Vec<(usize, BlockId)>,
}

impl Reader<'_> {
    fn add(&mut self, parent: Option<BlockId>, block: BlockDraft) -> Option<BlockId> {
        let id = block.id;
        let index = self.doc.children(parent).len();
        self.doc.apply(&Op::InsertBlock { parent, index, block }).ok().map(|()| id)
    }

    fn top(&mut self, kind: &str, text: InlineText) -> Option<BlockId> {
        self.lists.clear();
        let draft = BlockDraft::new(self.ids.next_id(), kind).text(text);
        self.add(None, draft)
    }

    fn fenced(&mut self, info: &str, body: &str) {
        self.lists.clear();
        if info.starts_with(FENCE_TAG) {
            if let Ok(value) = serde_json::from_str::<Value>(body) {
                self.embedded(None, &value);
            }
            return;
        }
        let mut draft =
            BlockDraft::new(self.ids.next_id(), kinds::CODE).text(InlineText::plain(body));
        if !info.is_empty() {
            draft = draft.prop("language", info);
        }
        self.add(None, draft);
    }

    /// A block carried as canonical JSON. It keeps its ID unless that
    /// ID is already taken.
    fn embedded(&mut self, parent: Option<BlockId>, value: &Value) {
        let Ok(mut draft) = draft_from(value) else {
            return;
        };
        if self.doc.contains(draft.id) {
            draft.id = self.ids.next_id();
        }
        let Some(id) = self.add(parent, draft) else {
            return;
        };
        for child in value.get("children").and_then(Value::as_array).into_iter().flatten() {
            self.embedded(Some(id), child);
        }
    }
}

/// Reads Markdown into a document. Any text is accepted; what is not
/// recognized becomes paragraphs.
pub fn from_markdown(text: &str, registry: Arc<Registry>, ids: &mut dyn IdSource) -> Document {
    let mut reader = Reader { doc: Document::new(registry), ids, lists: Vec::new() };
    let mut paragraph: Vec<&str> = Vec::new();
    let mut quote: Vec<&str> = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        let quoted = trimmed.strip_prefix('>').map(|rest| rest.strip_prefix(' ').unwrap_or(rest));
        if quoted.is_none() && !quote.is_empty() {
            reader.top(kinds::QUOTE, rich(&quote.join("\n")));
            quote.clear();
        }
        let continues = !trimmed.is_empty()
            && quoted.is_none()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("```")
            && list_item(line).is_none();
        if !continues && !paragraph.is_empty() {
            reader.top(kinds::PARAGRAPH, rich(&paragraph.join(" ")));
            paragraph.clear();
        }
        if let Some(rest) = quoted {
            quote.push(rest);
        } else if trimmed.starts_with("```") {
            let ticks = trimmed.bytes().take_while(|b| *b == b'`').count();
            let (fence, info) = trimmed.split_at(ticks);
            let pad = line.len() - line.trim_start().len();
            let mut body: Vec<&str> = Vec::new();
            for inner in lines.by_ref() {
                if inner.trim() == fence {
                    break;
                }
                body.push(
                    inner.get(pad..).filter(|_| inner.len() >= pad).unwrap_or(inner.trim_start()),
                );
            }
            reader.fenced(info.trim(), &body.join("\n"));
        } else if matches!(trimmed, "---" | "***" | "___") {
            reader.top(kinds::DIVIDER, InlineText::new());
        } else if let Some((indent, kind, checked, item)) = list_item(line) {
            while reader.lists.last().is_some_and(|(open, _)| *open >= indent) {
                reader.lists.pop();
            }
            let parent = reader.lists.last().map(|(_, id)| *id);
            let mut draft = BlockDraft::new(reader.ids.next_id(), kind).text(rich(item));
            if let Some(checked) = checked {
                draft = draft.prop("checked", PropValue::Bool(checked));
            }
            if let Some(id) = reader.add(parent, draft) {
                reader.lists.push((indent, id));
            }
        } else if trimmed.starts_with('#') {
            let level = trimmed.bytes().take_while(|b| *b == b'#').count();
            match trimmed[level..].strip_prefix(' ') {
                Some(title) => {
                    let draft = BlockDraft::new(reader.ids.next_id(), kinds::HEADING)
                        .text(rich(title))
                        .prop("level", PropValue::Int(level.min(3) as i64));
                    reader.lists.clear();
                    reader.add(None, draft);
                }
                None => paragraph.push(trimmed),
            }
        } else if !trimmed.is_empty() {
            paragraph.push(trimmed);
        }
    }
    if !quote.is_empty() {
        reader.top(kinds::QUOTE, rich(&quote.join("\n")));
    }
    if !paragraph.is_empty() {
        reader.top(kinds::PARAGRAPH, rich(&paragraph.join(" ")));
    }
    reader.doc
}
