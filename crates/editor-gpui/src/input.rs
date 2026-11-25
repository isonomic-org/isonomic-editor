// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform text input for the editor view.

use std::ops::Range;

use editor_model::{BlockId, Position, Selection};
use ui::{
    App, Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, px, size,
};

use crate::units::{from_utf16, to_utf16};
use crate::view::EditorView;

impl EditorView {
    /// Replaces a range of the head's block (or the text being
    /// composed, or the selection) and returns where the new text
    /// starts.
    fn replace(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        cx: &mut Context<Self>,
    ) -> Option<(BlockId, usize)> {
        self.state.update(cx, |state, cx| {
            let head = state.head()?;
            let current = state.text_of(head.block);
            let composing =
                state.composing().filter(|(block, _)| *block == head.block).map(|(_, r)| r.clone());
            // Each update of a composition replaces the last: to the
            // person typing, all of them are one edit.
            state.store.join_next(composing.is_some());
            let range = range_utf16
                .map(|range| from_utf16(&current, range.start)..from_utf16(&current, range.end))
                .or(composing);
            if let Some(range) = range {
                state.select(Selection::range(head.block, range.start, range.end), cx);
            }
            if !text.is_empty() {
                state.insert_text(text, cx);
            } else if !state.selection().is_caret() {
                state.backspace(false, cx);
            }
            state.store.join_next(false);
            let caret = state.head()?;
            Some((caret.block, caret.offset.saturating_sub(text.len())))
        })
    }

    fn head_text(&self, cx: &App) -> Option<(Position, String)> {
        let state = self.state.read(cx);
        let head = state.head()?;
        Some((head, state.text_of(head.block)))
    }
}

/// Platform text input. Ranges count UTF-16 units within the text of
/// the block that holds the caret.
impl EntityInputHandler for EditorView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let (_, text) = self.head_text(cx)?;
        text.get(from_utf16(&text, range.start)..from_utf16(&text, range.end)).map(str::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (head, text) = self.head_text(cx)?;
        let anchor = match self.state.read(cx).selection() {
            Selection::Text { anchor, .. } if anchor.block == head.block => anchor.offset,
            _ => head.offset,
        };
        let (start, end) = (anchor.min(head.offset), anchor.max(head.offset));
        Some(UTF16Selection {
            range: to_utf16(&text, start)..to_utf16(&text, end),
            reversed: head.offset < anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let (head, text) = self.head_text(cx)?;
        let state = self.state.read(cx);
        let (_, range) = state.composing().filter(|(block, _)| *block == head.block)?;
        Some(to_utf16(&text, range.start)..to_utf16(&text, range.end))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| state.set_composing(None));
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text, cx);
        self.state.update(cx, |state, _| state.set_composing(None));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((block, start)) = self.replace(range, new_text, cx) else { return };
        self.state.update(cx, |state, cx| {
            let composing = (!new_text.is_empty()).then(|| (block, start..start + new_text.len()));
            state.set_composing(composing);
            if let Some(selected) = new_selected_range {
                let from = start + from_utf16(new_text, selected.start);
                let to = start + from_utf16(new_text, selected.end);
                state.select(Selection::range(block, from, to), cx);
            }
        });
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (head, text) = self.head_text(cx)?;
        let frame = self.frame.borrow();
        let leaf = frame.leaves.get(&self.keys.borrow().get(head.block)?)?;
        let start = from_utf16(&text, range_utf16.start).min(leaf.len());
        Some(Bounds::new(leaf.position_for_index(start), size(px(1.), leaf.line_height())))
    }

    fn character_index_for_point(
        &mut self,
        at: Point<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let (head, text) = self.head_text(cx)?;
        let hit = self.position_at(at, cx)?;
        (hit.block == head.block).then(|| to_utf16(&text, hit.offset))
    }
}
