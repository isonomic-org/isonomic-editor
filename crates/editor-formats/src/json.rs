// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use editor_model::{
    ATOM_CHAR, Atom, Block, BlockDraft, BlockId, BlockKind, Content, Document, InlineText, Limits,
    Mark, MarkKind, Marks, Op, PropValue, Props, Registry,
};
use serde_json::{Map, Value, json};

use crate::FormatError;

/// The format tag every canonical document carries.
pub const FORMAT: &str = "isonomic.doc/1";

fn props_json(props: &Props) -> Value {
    serde_json::to_value(props).unwrap_or_else(|_| json!({}))
}

fn inline_json(inline: &InlineText) -> Value {
    let mut marks: Vec<Value> = Vec::new();
    for kind in MarkKind::ALL {
        let mut open: Option<(usize, usize, Mark)> = None;
        for (span, run) in inline.spans() {
            let here = run.marks.get(kind);
            match (&mut open, here) {
                (Some((_, end, mark)), Some(next)) if *mark == next && *end == span.start => {
                    *end = span.end;
                }
                (slot, here) => {
                    if let Some(done) = slot.take() {
                        marks.push(mark_json(done));
                    }
                    *slot = here.map(|mark| (span.start, span.end, mark));
                }
            }
        }
        if let Some(done) = open {
            marks.push(mark_json(done));
        }
    }
    let atoms: Vec<Value> = inline
        .spans()
        .filter_map(|(span, run)| {
            let atom = run.atom.as_ref()?;
            Some(json!({ "at": span.start, "kind": atom.kind, "attrs": props_json(&atom.attrs) }))
        })
        .collect();
    let mut out = Map::new();
    out.insert("text".into(), Value::from(inline.text()));
    if !marks.is_empty() {
        out.insert("marks".into(), Value::Array(marks));
    }
    if !atoms.is_empty() {
        out.insert("atoms".into(), Value::Array(atoms));
    }
    Value::Object(out)
}

fn mark_json((start, end, mark): (usize, usize, Mark)) -> Value {
    let mut out = json!({ "start": start, "end": end, "type": mark.kind().key() });
    if let Some(value) = mark.value() {
        out["value"] = Value::from(value);
    }
    out
}

/// One block and everything under it.
pub fn block_to_json(doc: &Document, block: &Block) -> Value {
    let content = match &block.content {
        Content::Inline(inline) => inline_json(inline),
        Content::Plain(text) => json!({ "text": text }),
        Content::None => Value::Null,
    };
    let children: Vec<Value> = block
        .children
        .iter()
        .filter_map(|id| doc.get(*id))
        .map(|child| block_to_json(doc, child))
        .collect();
    json!({
        "id": block.id.to_string(),
        "type": block.kind.as_str(),
        "props": props_json(&block.props),
        "tunes": props_json(&block.tunes),
        "content": content,
        "children": children,
    })
}

pub fn to_json(doc: &Document) -> Value {
    let blocks: Vec<Value> = doc
        .root()
        .iter()
        .filter_map(|id| doc.get(*id))
        .map(|block| block_to_json(doc, block))
        .collect();
    json!({ "format": FORMAT, "blocks": blocks })
}

pub fn to_json_string(doc: &Document) -> String {
    serde_json::to_string_pretty(&to_json(doc)).unwrap_or_default()
}

fn props_from(value: Option<&Value>) -> Props {
    value.and_then(|value| serde_json::from_value(value.clone()).ok()).unwrap_or_default()
}

fn inline_from(value: &Value) -> InlineText {
    let text = value.get("text").and_then(Value::as_str).unwrap_or_default();
    let mut inline = InlineText::plain(text);
    for atom in value.get("atoms").and_then(Value::as_array).into_iter().flatten() {
        let at = atom.get("at").and_then(Value::as_u64).unwrap_or(u64::MAX) as usize;
        let kind = atom.get("kind").and_then(Value::as_str).unwrap_or_default();
        let end = at.saturating_add(ATOM_CHAR.len_utf8());
        if kind.is_empty() || text.get(at..end) != Some(ATOM_CHAR.to_string().as_str()) {
            continue;
        }
        let atom = Atom { kind: kind.to_owned(), attrs: props_from(atom.get("attrs")) };
        let _ = inline.replace(at..end, &InlineText::atom(atom, Marks::default()));
    }
    for mark in value.get("marks").and_then(Value::as_array).into_iter().flatten() {
        let number = |key: &str| mark.get(key).and_then(Value::as_u64).map(|n| n as usize);
        let kind = mark.get("type").and_then(Value::as_str).and_then(MarkKind::from_key);
        let (Some(start), Some(end), Some(kind)) = (number("start"), number("end"), kind) else {
            continue;
        };
        let value = mark.get("value").and_then(Value::as_str);
        if kind.has_value() && value.is_none() {
            continue;
        }
        let _ = inline.add_mark(start..end, &Mark::new(kind, value));
    }
    inline
}

/// Reads one block object into a draft, without its children.
pub(crate) fn draft_from(value: &Value) -> Result<BlockDraft, FormatError> {
    let field = |key: &str| value.get(key).and_then(Value::as_str);
    let id: BlockId = field("id")
        .and_then(|id| id.parse().ok())
        .ok_or_else(|| FormatError::Syntax("a block has no valid id".into()))?;
    let kind = field("type").ok_or_else(|| FormatError::Syntax("a block has no type".into()))?;
    Ok(BlockDraft {
        id,
        kind: BlockKind::new(kind),
        props: props_from(value.get("props")),
        tunes: props_from(value.get("tunes")),
        text: value.get("content").map(inline_from).unwrap_or_default(),
    })
}

/// Reads a canonical document. Malformed input is an error; a block
/// that repeats an ID or passes a limit is skipped with its children.
pub fn from_json(
    text: &str,
    registry: Arc<Registry>,
    limits: Limits,
) -> Result<Document, FormatError> {
    if text.len() > limits.paste_bytes {
        return Err(FormatError::TooLarge);
    }
    let root: Value =
        serde_json::from_str(text).map_err(|error| FormatError::Syntax(error.to_string()))?;
    match root.get("format").and_then(Value::as_str) {
        Some(FORMAT) => {}
        other => {
            return Err(FormatError::Unsupported(other.unwrap_or("no format").to_owned()));
        }
    }
    let blocks = root
        .get("blocks")
        .and_then(Value::as_array)
        .ok_or_else(|| FormatError::Syntax("no blocks".into()))?;
    let mut doc = Document::with_limits(registry, limits);
    let mut stack: Vec<(Option<BlockId>, &Value)> =
        blocks.iter().rev().map(|block| (None, block)).collect();
    while let Some((parent, value)) = stack.pop() {
        let block = draft_from(value)?;
        let id = block.id;
        let index = doc.children(parent).len();
        if doc.apply(&Op::InsertBlock { parent, index, block }).is_err() {
            continue;
        }
        let children = value.get("children").and_then(Value::as_array);
        stack.extend(children.into_iter().flatten().rev().map(|child| (Some(id), child)));
    }
    Ok(doc)
}

/// A property as the value a fenced Markdown block carries.
pub(crate) fn prop_text(value: &PropValue) -> String {
    match value {
        PropValue::Text(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
