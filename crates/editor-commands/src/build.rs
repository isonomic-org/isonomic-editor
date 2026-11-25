// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers: selection geometry and a sequential op builder.

use std::ops::Range;

use editor_model::{
    BlockDraft, BlockId, Content, Document, IdSource, InlineText, Op, Position, Selection, kinds,
};

/// Builds a list of operations against a scratch copy of the
/// document, so each operation sees the effect of the ones before it.
pub struct Builder {
    pub doc: Document,
    pub ops: Vec<Op>,
}

impl Builder {
    pub fn new(doc: &Document) -> Self {
        Self { doc: doc.clone(), ops: Vec::new() }
    }

    pub fn push(&mut self, op: Op) -> Option<()> {
        self.doc.apply(&op).ok()?;
        self.ops.push(op);
        Some(())
    }

    /// Moves `id` to sit right after `after`, as its sibling.
    pub fn move_after(&mut self, id: BlockId, after: BlockId) -> Option<()> {
        let parent = self.doc.parent(after);
        let mut index = self.doc.index_in_parent(after)? + 1;
        if self.doc.parent(id) == parent && self.doc.index_in_parent(id)? < index {
            index -= 1;
        }
        self.push(Op::MoveBlock { id, parent, index })
    }

    /// Lifts the children of `id` out, in order, to follow it.
    pub fn lift_children(&mut self, id: BlockId) -> Option<()> {
        let children = self.doc.children(Some(id)).to_vec();
        for child in children.into_iter().rev() {
            self.move_after(child, id)?;
        }
        Some(())
    }

    /// Inserts an empty paragraph after `after` and returns its ID.
    pub fn paragraph_after(&mut self, after: BlockId, ids: &mut dyn IdSource) -> Option<BlockId> {
        let id = ids.next_id();
        let parent = self.doc.parent(after);
        let index = self.doc.index_in_parent(after)? + 1;
        self.push(Op::InsertBlock { parent, index, block: BlockDraft::new(id, kinds::PARAGRAPH) })?;
        Some(id)
    }
}

pub fn content(doc: &Document, id: BlockId) -> Option<&Content> {
    doc.get(id).map(|block| &block.content)
}

/// Whether a position names existing text on a character boundary.
pub fn valid(doc: &Document, at: Position) -> bool {
    content(doc, at.block)
        .is_some_and(|content| content.has_text() && content.text().is_char_boundary(at.offset))
}

/// A text selection's ends in document order.
pub fn ordered(doc: &Document, selection: &Selection) -> Option<(Position, Position)> {
    let Selection::Text { anchor, head } = selection else {
        return None;
    };
    if !valid(doc, *anchor) || !valid(doc, *head) {
        return None;
    }
    let forward = if anchor.block == head.block {
        anchor.offset <= head.offset
    } else {
        doc.path(anchor.block)? <= doc.path(head.block)?
    };
    Some(if forward { (*anchor, *head) } else { (*head, *anchor) })
}

/// The text ranges a selection covers, block by block.
pub fn block_ranges(doc: &Document, selection: &Selection) -> Vec<(BlockId, Range<usize>)> {
    let Some((start, end)) = ordered(doc, selection) else {
        return Vec::new();
    };
    if start.block == end.block {
        return vec![(start.block, start.offset..end.offset)];
    }
    let mut out = Vec::new();
    let mut at = Some(start.block);
    while let Some(id) = at {
        let len = content(doc, id).map_or(0, Content::len);
        let range = match id {
            id if id == start.block => start.offset..len,
            id if id == end.block => 0..end.offset,
            _ => 0..len,
        };
        out.push((id, range));
        if id == end.block {
            break;
        }
        at = doc.next_in_order(id);
    }
    out
}

/// The blocks a command acts on: the selected blocks, minus any whose
/// ancestor is also selected, in document order.
pub fn targets(doc: &Document, selection: &Selection) -> Vec<BlockId> {
    let mut all: Vec<BlockId> = match selection {
        Selection::Blocks(ids) => ids.iter().copied().filter(|id| doc.contains(*id)).collect(),
        Selection::Text { .. } => {
            block_ranges(doc, selection).into_iter().map(|(id, _)| id).collect()
        }
        Selection::Cells { .. } => Vec::new(),
    };
    all.sort_by_key(|id| doc.path(*id));
    all.dedup();
    let keep = |id: &BlockId| !all.iter().any(|other| doc.is_ancestor(*other, *id));
    all.iter().copied().filter(keep).collect()
}

/// Deletes the text between two ordered positions, joining the last
/// block's remaining text onto the first.
pub fn delete_range(build: &mut Builder, start: Position, end: Position) -> Option<()> {
    if start.block == end.block {
        if start.offset == end.offset {
            return Some(());
        }
        let with = InlineText::new();
        return build.push(Op::ReplaceText {
            id: start.block,
            range: start.offset..end.offset,
            with,
        });
    }
    let flat = build.doc.flat();
    let first = flat.iter().position(|id| *id == start.block)?;
    let last = flat.iter().position(|id| *id == end.block)?;
    let doomed = flat.get(first + 1..=last)?.to_vec();
    let tail = content(&build.doc, end.block)?.to_inline();
    let tail = tail.slice(end.offset..tail.len());
    let len = content(&build.doc, start.block)?.len();
    build.push(Op::ReplaceText { id: start.block, range: start.offset..len, with: tail })?;
    // Blocks under a removed block that lie past the selection survive:
    // they move out to follow the topmost removed ancestor.
    for id in doomed.iter().rev() {
        let survivors: Vec<BlockId> = build
            .doc
            .children(Some(*id))
            .iter()
            .copied()
            .filter(|child| !doomed.contains(child))
            .collect();
        let mut top = *id;
        while let Some(parent) = build.doc.parent(top).filter(|parent| doomed.contains(parent)) {
            top = parent;
        }
        for child in survivors.into_iter().rev() {
            build.move_after(child, top)?;
        }
    }
    for id in doomed {
        if build.doc.contains(id) {
            build.push(Op::RemoveBlock { id })?;
        }
    }
    Some(())
}
