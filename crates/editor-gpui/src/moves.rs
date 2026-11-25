// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Moving the selection through the document itself: by character, by
//! word, by block and to the edges. Movement that depends on what is
//! on screen (visual lines, the pointer) is the view's.

use editor_model::{
    BlockId, Document, Position, Selection, next_grapheme, next_word, prev_grapheme, prev_word,
    word_range,
};
use ui::Context;

use crate::state::EditorState;

impl EditorState {
    /// The nearest block with text before or after `id` in document
    /// order.
    fn neighbour(doc: &Document, id: BlockId, forward: bool) -> Option<BlockId> {
        let mut at = id;
        loop {
            at = if forward { doc.next_in_order(at)? } else { doc.prev_in_order(at)? };
            if doc.get(at).is_some_and(|block| block.content.has_text()) {
                return Some(at);
            }
        }
    }

    /// Moves one grapheme left or right, crossing into the next block
    /// at an edge. Without `extend`, a range collapses to its end in
    /// that direction.
    pub fn move_horizontally(&mut self, forward: bool, extend: bool, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        let Selection::Text { anchor, head } = self.selection.clone() else { return };
        if !extend && anchor != head {
            let forward_order = doc.path(anchor.block) < doc.path(head.block)
                || (anchor.block == head.block && anchor.offset <= head.offset);
            let to = if forward == forward_order { head } else { anchor };
            return self.move_head(to, false, cx);
        }
        let text = self.text_of(head.block);
        let to = if forward && head.offset < text.len() {
            Position::new(head.block, next_grapheme(&text, head.offset))
        } else if !forward && head.offset > 0 {
            Position::new(head.block, prev_grapheme(&text, head.offset))
        } else {
            match Self::neighbour(&doc, head.block, forward) {
                Some(next) if forward => Position::new(next, 0),
                Some(prev) => Position::new(prev, self.text_of(prev).len()),
                None => return,
            }
        };
        self.move_head(to, extend, cx);
    }

    /// Moves the head to the start of the next block with text, or
    /// the end of the previous one. At the document's edge it moves to
    /// the edge of the block it is in.
    pub fn move_by_block(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(head) = self.head() else { return };
        let doc = self.snapshot();
        let mut at = head.block;
        let to = loop {
            let next = if down { doc.next_in_order(at) } else { doc.prev_in_order(at) };
            let Some(next) = next else {
                let offset = if down { self.text_of(head.block).len() } else { 0 };
                break Position::new(head.block, offset);
            };
            at = next;
            if doc
                .get(at)
                .is_some_and(|block| !matches!(block.content, editor_model::Content::None))
            {
                let offset = if down { 0 } else { self.text_of(at).len() };
                break Position::new(at, offset);
            }
        };
        self.move_head(to, extend, cx);
    }

    /// Moves to the start or the end of the block's text.
    pub fn move_to_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(head) = self.head() else { return };
        let offset = if end { self.text_of(head.block).len() } else { 0 };
        self.move_head(Position::new(head.block, offset), extend, cx);
    }

    /// Moves to the start of the word before the head or the end of
    /// the word after it, crossing into the next block at an edge.
    pub fn move_by_word(&mut self, forward: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(head) = self.head() else { return };
        let text = self.text_of(head.block);
        let at = head.offset.min(text.len());
        let to = if forward { next_word(&text, at) } else { prev_word(&text, at) };
        if to == at {
            return self.move_horizontally(forward, extend, cx);
        }
        self.move_head(Position::new(head.block, to), extend, cx);
    }

    /// Moves to the start of the first block with text, or the end of
    /// the last one.
    pub fn move_to_document_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        let order = doc.flat();
        let has_text = |id: &&BlockId| doc.get(**id).is_some_and(|block| block.content.has_text());
        let edge =
            if end { order.iter().rev().find(has_text) } else { order.iter().find(has_text) };
        let Some(id) = edge.copied() else { return };
        let offset = if end { self.text_of(id).len() } else { 0 };
        self.move_head(Position::new(id, offset), extend, cx);
    }

    /// Selects the word at a position, or the white space or
    /// punctuation between words there.
    pub fn select_word_at(&mut self, at: Position, cx: &mut Context<Self>) {
        let range = word_range(&self.text_of(at.block), at.offset);
        self.select(Selection::range(at.block, range.start, range.end), cx);
    }

    /// Selects all the text of the block at a position.
    pub fn select_block_at(&mut self, at: Position, cx: &mut Context<Self>) {
        let end = self.text_of(at.block).len();
        self.select(Selection::range(at.block, 0, end), cx);
    }
}
