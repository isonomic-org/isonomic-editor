// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use editor_model::{
    BlockDraft, BlockId, BlockKind, Content, Document, IdSource, Intent, Op, Props, Selection,
    Transaction, kinds,
};

use crate::build::{Builder, content, delete_range, ordered};

/// Pastes a fragment at the selection. One childless text block goes
/// in as text; anything else goes in as blocks after the current one,
/// with fresh IDs.
pub fn insert_fragment(
    doc: &Document,
    selection: &Selection,
    fragment: &Document,
    ids: &mut dyn IdSource,
) -> Option<Transaction> {
    let (start, end) = ordered(doc, selection)?;
    let mut build = Builder::new(doc);
    delete_range(&mut build, start, end)?;
    let target = start.block;
    let single = match fragment.root() {
        [only] => fragment.get(*only).filter(|block| block.children.is_empty()),
        _ => None,
    };
    if let Some(block) = single.filter(|block| block.content.has_text()) {
        let with = block.content.to_inline();
        let caret = Selection::caret(target, start.offset + with.len());
        build.push(Op::ReplaceText { id: target, range: start.offset..start.offset, with })?;
        return Some(Transaction { ops: build.ops, selection: Some(caret), intent: Intent::Paste });
    }
    let parent = build.doc.parent(target);
    let base = build.doc.index_in_parent(target)? + 1;
    let mut fresh: HashMap<BlockId, BlockId> = HashMap::new();
    let mut caret = None;
    for old in fragment.flat() {
        let block = fragment.get(old)?;
        let id = ids.next_id();
        fresh.insert(old, id);
        let draft = BlockDraft {
            id,
            kind: block.kind.clone(),
            props: block.props.clone(),
            tunes: block.tunes.clone(),
            text: block.content.to_inline(),
        };
        let index = fragment.index_in_parent(old)?;
        let op = match fragment.parent(old).and_then(|p| fresh.get(&p)) {
            Some(parent) => Op::InsertBlock { parent: Some(*parent), index, block: draft },
            None => Op::InsertBlock { parent, index: base + index, block: draft },
        };
        build.push(op)?;
        if block.content.has_text() {
            caret = Some(Selection::caret(id, block.content.len()));
        }
    }
    let empty = build.doc.get(target).is_some_and(|block| {
        block.kind.is(kinds::PARAGRAPH) && block.content.is_empty() && block.children.is_empty()
    });
    if empty && !fresh.is_empty() {
        build.push(Op::RemoveBlock { id: target })?;
    }
    if build.ops.is_empty() {
        return None;
    }
    let selection = caret.or_else(|| build.doc.contains(target).then(|| selection.clone()));
    Some(Transaction { ops: build.ops, selection, intent: Intent::Paste })
}

/// Inserts a new block of `kind` at the caret, as a slash menu would:
/// an empty paragraph is converted in place, otherwise the block goes
/// after the current one. The caret lands in the new block when it
/// has text, or in a paragraph after it when it has none.
pub fn insert_block(
    doc: &Document,
    selection: &Selection,
    kind: &str,
    props: &Props,
    ids: &mut dyn IdSource,
) -> Option<Transaction> {
    let at = selection.head()?;
    let current = doc.get(at.block)?;
    let mut build = Builder::new(doc);
    let reuse = current.kind.is(kinds::PARAGRAPH)
        && current.content.is_empty()
        && current.children.is_empty();
    let id = if reuse {
        for (key, value) in props {
            build.push(Op::SetProp {
                id: at.block,
                key: key.clone(),
                value: Some(value.clone()),
            })?;
        }
        build.push(Op::SetKind { id: at.block, kind: BlockKind::new(kind) })?;
        at.block
    } else {
        let id = ids.next_id();
        let mut block = BlockDraft::new(id, kind);
        block.props = props.clone();
        let parent = doc.parent(at.block);
        let index = doc.index_in_parent(at.block)? + 1;
        build.push(Op::InsertBlock { parent, index, block })?;
        id
    };
    let caret = if content(&build.doc, id).is_some_and(Content::has_text) {
        id
    } else {
        build.paragraph_after(id, ids)?
    };
    Some(Transaction {
        ops: build.ops,
        selection: Some(Selection::caret(caret, 0)),
        intent: Intent::Command,
    })
}
