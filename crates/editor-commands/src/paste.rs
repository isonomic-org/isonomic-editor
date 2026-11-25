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

/// What the selection holds, as a document of its own: the selected
/// blocks, with the first and last cut down to the selected text.
/// Blocks keep their nesting where their parent is selected too.
pub fn copy_fragment(doc: &Document, selection: &Selection) -> Option<Document> {
    let ranges: Vec<(BlockId, std::ops::Range<usize>)> = match selection {
        Selection::Text { .. } => crate::build::block_ranges(doc, selection),
        Selection::Blocks(blocks) => blocks
            .iter()
            .flat_map(|id| doc.subtree(*id))
            .map(|id| (id, 0..doc.get(id).map_or(0, |block| block.content.len())))
            .collect(),
        Selection::Cells { .. } => Vec::new(),
    };
    if ranges.iter().all(|(_, range)| range.is_empty())
        && !matches!(selection, Selection::Blocks(_))
    {
        return None;
    }
    let mut out = Document::with_limits(doc.registry().clone(), doc.limits().clone());
    let whole_blocks = ranges.len() > 1 || matches!(selection, Selection::Blocks(_));
    for (id, range) in ranges {
        let block = doc.get(id)?;
        let text = block.content.to_inline();
        let whole = range.start == 0 && range.end == text.len();
        // Part of one block is just text; it does not carry the
        // block's type along.
        let plain = !whole_blocks && !whole;
        let draft = BlockDraft {
            id,
            kind: if plain { BlockKind::new(kinds::PARAGRAPH) } else { block.kind.clone() },
            props: if plain { Props::new() } else { block.props.clone() },
            tunes: if plain { Props::new() } else { block.tunes.clone() },
            text: text.slice(range),
        };
        let parent = doc.parent(id).filter(|parent| out.contains(*parent));
        let index = out.children(parent).len();
        out.apply(&Op::InsertBlock { parent, index, block: draft }).ok()?;
    }
    (!out.is_empty()).then_some(out)
}
