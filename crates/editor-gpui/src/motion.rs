// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Moving the selection by what is on screen: visual lines and the
//! pointer. Where a block is not drawn, movement falls back to the
//! document's own order.

use editor_model::{BlockId, kinds};
use ui::base::text::editor_host::Caret;
use ui::{Context, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Window, point};

use crate::view::EditorView;

/// How far left of a to-do's text its checkbox reaches, in rems.
const CHECKBOX_REMS: f32 = 2.;
/// The most lines one frame scrolls while a drag holds the pointer
/// past the top or the bottom.
const MOST_LINES_A_FRAME: f32 = 6.;

impl EditorView {
    /// Moves the head to the start or end of its visual line. Without
    /// geometry for the block it moves to the block's edge.
    pub(crate) fn move_to_line_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(head) = self.state.read(cx).head() else { return };
        let aim = {
            let frame = self.frame.borrow();
            let key = self.keys.borrow().get(head.block);
            key.and_then(|key| frame.leaves.get(&key)).map(|leaf| {
                let at = leaf.position_for_caret(Caret::new(head.offset.min(leaf.len())));
                let bounds = leaf.bounds();
                let x = if end { bounds.origin.x + bounds.size.width } else { bounds.origin.x };
                (point(x, at.y + leaf.line_height() * 0.5), at.y)
            })
        };
        let found = aim.and_then(|(aim, line)| Some((self.position_at(aim, cx)?, line)));
        let Some((mut to, line)) = found.filter(|(to, _)| to.block == head.block) else {
            self.state.update(cx, |state, cx| state.move_to_edge(end, extend, cx));
            return;
        };
        // Where a line wraps, its end is also the next line's start and
        // the caret draws there: stay on this line, one character back.
        if end && self.caret_point(to).is_some_and(|at| at.y > line) {
            let text = self.state.read(cx).text_of(to.block);
            to.offset = text[..to.offset].char_indices().next_back().map_or(0, |(at, _)| at);
        }
        self.state.update(cx, |state, cx| state.move_head(to, extend, cx));
    }

    /// Moves the head one visual line up or down, keeping its column.
    pub(crate) fn move_vertically(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(head) = self.state.read(cx).head() else { return };
        let target = {
            let frame = self.frame.borrow();
            let key = self.keys.borrow().get(head.block);
            key.and_then(|key| Some((key, frame.leaves.get(&key)?))).and_then(|(key, leaf)| {
                let at = leaf.position_for_caret(Caret::new(head.offset.min(leaf.len())));
                let line = leaf.line_height();
                let bounds = leaf.bounds();
                let y = if down { at.y + line * 1.5 } else { at.y - line * 0.5 };
                if y >= bounds.origin.y && y < bounds.origin.y + bounds.size.height {
                    return Some(point(at.x, y));
                }
                // Leave this block: aim at the nearest line of the
                // next drawn text leaf in that direction.
                let here = frame.order.iter().position(|(k, _)| *k == key)?;
                let mut others: Box<dyn Iterator<Item = &(u64, BlockId)>> = if down {
                    Box::new(frame.order[here + 1..].iter())
                } else {
                    Box::new(frame.order[..here].iter().rev())
                };
                others.find_map(|(key, _)| frame.leaves.get(key)).map(|next| {
                    let bounds = next.bounds();
                    let half = next.line_height() * 0.5;
                    let y = if down {
                        bounds.origin.y + half
                    } else {
                        bounds.origin.y + bounds.size.height - half
                    };
                    point(at.x, y)
                })
            })
        };
        match target.and_then(|target| self.position_at(target, cx)) {
            Some(to) => self.state.update(cx, |state, cx| state.move_head(to, extend, cx)),
            // Nothing drawn that way, or the head itself is out of
            // view: step to the neighbouring block instead.
            None => self.state.update(cx, |state, cx| state.move_by_block(down, extend, cx)),
        }
    }

    /// A to-do whose checkbox, which sits just left of its text, is
    /// under the point.
    fn checkbox_at(
        &self,
        at: Point<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<BlockId> {
        let frame = self.frame.borrow();
        let doc = self.state.read(cx).snapshot();
        let reach = window.rem_size() * CHECKBOX_REMS;
        frame.order.iter().find_map(|(key, id)| {
            let leaf = frame.leaves.get(key)?;
            let bounds = leaf.bounds();
            let beside = at.x < bounds.origin.x && at.x >= bounds.origin.x - reach;
            let level = at.y >= bounds.origin.y && at.y < bounds.origin.y + leaf.line_height();
            let todo = doc.get(*id).is_some_and(|block| block.kind.is(kinds::TODO));
            (beside && level && todo).then_some(*id)
        })
    }

    pub(crate) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if let Some(id) = self.checkbox_at(event.position, window, cx) {
            return self.state.update(cx, |state, cx| state.toggle_todo(id, cx));
        }
        let Some(to) = self.position_at(event.position, cx) else { return };
        let extend = event.modifiers.shift;
        self.state.update(cx, |state, cx| match event.click_count {
            2 => state.select_word_at(to, cx),
            3 => state.select_block_at(to, cx),
            _ => state.move_head(to, extend, cx),
        });
        // A second or third click selects a word or a block, and a
        // drag after it would only undo that.
        self.dragging = event.click_count == 1;
    }

    pub(crate) fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dragging || event.pressed_button != Some(MouseButton::Left) {
            return self.end_drag();
        }
        if let Some(to) = self.position_at(event.position, cx) {
            self.state.update(cx, |state, cx| state.move_head(to, true, cx));
        }
        let view = self.list.viewport_bounds();
        let past =
            event.position.y < view.origin.y || event.position.y > view.origin.y + view.size.height;
        self.drag_past = past.then_some(event.position);
        if past {
            cx.notify();
        }
    }

    pub(crate) fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_past = None;
    }

    /// While a drag holds the pointer above or below the rows, scrolls
    /// that way, further the further out the pointer is, and extends
    /// the selection to the text now under it. Called once a frame.
    pub(crate) fn drag_past_edge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.drag_past else { return };
        let view = self.list.viewport_bounds();
        let (top, bottom) = (view.origin.y, view.origin.y + view.size.height);
        let line = window.line_height();
        let most = line * MOST_LINES_A_FRAME;
        let by = if at.y < top {
            (at.y - top - line).max(-most)
        } else if at.y > bottom {
            (at.y - bottom + line).min(most)
        } else {
            self.drag_past = None;
            return;
        };
        // The text under the pointer as of the last frame, which drew
        // what the step before this one scrolled to.
        let edge = point(at.x, at.y.clamp(top + line * 0.5, bottom - line * 0.5));
        if let Some(to) = self.position_at(edge, cx) {
            self.state.update(cx, |state, cx| state.move_head(to, true, cx));
        }
        self.list.scroll_by(by);
        window.request_animation_frame();
    }
}
