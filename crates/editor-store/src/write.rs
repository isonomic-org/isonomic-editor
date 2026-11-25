//! Applying one primitive operation to the replicated document.

use std::collections::HashMap;
use std::ops::Range;

use editor_model::{BlockId, InlineText, Mark, MarkKind, Op, PropValue};
use loro::{LoroMap, LoroResult, LoroText, LoroTree, TreeParentId};

use crate::convert::{self, ATOM_KEY};
use crate::read::{Handles, PROP, TUNE, text_of};

fn parent_of(handles: &HashMap<BlockId, Handles>, parent: Option<BlockId>) -> TreeParentId {
    match parent.and_then(|id| handles.get(&id)) {
        Some(handles) => TreeParentId::Node(handles.node),
        None => TreeParentId::Root,
    }
}

fn set(map: &LoroMap, prefix: &str, key: &str, value: &Option<PropValue>) -> LoroResult<()> {
    let key = format!("{prefix}{key}");
    match value {
        Some(value) => map.insert(&key, convert::to_value(value)),
        None => map.delete(&key),
    }
}

fn mark(text: &LoroText, range: Range<usize>, mark: &Mark) -> LoroResult<()> {
    let key = mark.kind().key();
    match mark.value() {
        Some(value) => text.mark_utf8(range, key, value),
        None => text.mark_utf8(range, key, true),
    }
}

/// Mark removal takes character indices; ours are bytes.
fn unmark(text: &LoroText, range: Range<usize>, key: &str) -> LoroResult<()> {
    let content = text.to_string();
    let chars = |byte: usize| content.get(..byte).map_or(0, |head| head.chars().count());
    text.unmark(chars(range.start)..chars(range.end), key)
}

/// Inserts rich text at `at` and makes its marks exactly `with`'s,
/// whatever the neighbouring text would have lent it.
fn insert_rich(text: &LoroText, at: usize, with: &InlineText) -> LoroResult<()> {
    if with.is_empty() {
        return Ok(());
    }
    text.insert_utf8(at, with.text())?;
    let inherited = text_of(text).slice(at..at + with.len());
    for (span, run) in with.spans() {
        let range = at + span.start..at + span.end;
        let had = inherited.slice(span.clone());
        for kind in MarkKind::ALL {
            let want = run.marks.get(kind);
            let differs = had.runs().iter().any(|run| run.marks.get(kind) != want);
            match want {
                Some(want) if differs => mark(text, range.clone(), &want)?,
                None if differs => unmark(text, range.clone(), kind.key())?,
                _ => {}
            }
        }
        match &run.atom {
            Some(atom) => text.mark_utf8(range, ATOM_KEY, convert::atom_value(atom))?,
            None if had.runs().iter().any(|run| run.atom.is_some()) => {
                unmark(text, range, ATOM_KEY)?;
            }
            None => {}
        }
    }
    Ok(())
}

/// Brings the replicated text in line with a block type's content:
/// `keep` is the text the model kept after the type changed.
fn reshape(text: &LoroText, keep: &InlineText) -> LoroResult<()> {
    if text_of(text) == *keep {
        return Ok(());
    }
    let len = text.len_utf8();
    if len > 0 {
        text.delete_utf8(0, len)?;
    }
    insert_rich(text, 0, keep)
}

/// Applies `op`. The model has already accepted it, so `handles`
/// holds every block it names except one being inserted.
pub fn apply(
    tree: &LoroTree,
    handles: &mut HashMap<BlockId, Handles>,
    op: &Op,
    kept_text: Option<InlineText>,
) -> LoroResult<()> {
    let of = |id: &BlockId| handles.get(id).cloned();
    match op {
        Op::InsertBlock { parent, index, block } => {
            let node = tree.create_at(parent_of(handles, *parent), *index)?;
            let meta = tree.get_meta(node)?;
            meta.insert("id", block.id.to_string())?;
            meta.insert("kind", block.kind.as_str())?;
            let text = meta.insert_container("text", LoroText::new())?;
            for (key, value) in &block.props {
                set(&meta, PROP, key, &Some(value.clone()))?;
            }
            for (key, value) in &block.tunes {
                set(&meta, TUNE, key, &Some(value.clone()))?;
            }
            insert_rich(&text, 0, &kept_text.unwrap_or_default())?;
            handles.insert(block.id, Handles { node, meta, text });
        }
        Op::RemoveBlock { id } => {
            if let Some(block) = handles.remove(id) {
                tree.delete(block.node)?;
            }
        }
        Op::MoveBlock { id, parent, index } => {
            if let Some(block) = of(id) {
                tree.mov_to(block.node, parent_of(handles, *parent), *index)?;
            }
        }
        Op::SetKind { id, kind } => {
            if let Some(block) = of(id) {
                block.meta.insert("kind", kind.as_str())?;
                reshape(&block.text, &kept_text.unwrap_or_default())?;
            }
        }
        Op::SetProp { id, key, value } => {
            if let Some(block) = of(id) {
                set(&block.meta, PROP, key, value)?;
            }
        }
        Op::SetTune { id, key, value } => {
            if let Some(block) = of(id) {
                set(&block.meta, TUNE, key, value)?;
            }
        }
        Op::ReplaceText { id, range, with } => {
            if let Some(block) = of(id) {
                if range.end > range.start {
                    block.text.delete_utf8(range.start, range.end - range.start)?;
                }
                insert_rich(&block.text, range.start, with)?;
            }
        }
        Op::AddMark { id, range, mark: add } => {
            if let Some(block) = of(id)
                && range.end > range.start
            {
                mark(&block.text, range.clone(), add)?;
            }
        }
        Op::RemoveMark { id, range, kind } => {
            if let Some(block) = of(id)
                && range.end > range.start
            {
                unmark(&block.text, range.clone(), kind.key())?;
            }
        }
    }
    Ok(())
}
