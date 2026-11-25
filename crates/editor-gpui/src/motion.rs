// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Moving the selection by what is on screen: visual lines and the
//! pointer. Where a block is not drawn, movement falls back to the
//! document's own order.

use editor_model::BlockId;
use ui::base::text::editor_host::Caret;
use ui::{Context, MouseButton, MouseDownEvent, MouseMoveEvent, Window, point};

use crate::view::EditorView;

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

    pub(crate) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if let Some(to) = self.position_at(event.position, cx) {
            self.dragging = true;
            let extend = event.modifiers.shift;
            self.state.update(cx, |state, cx| state.move_head(to, extend, cx));
        }
    }

    pub(crate) fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dragging || event.pressed_button != Some(MouseButton::Left) {
            self.dragging = false;
            return;
        }
        if let Some(to) = self.position_at(event.position, cx) {
            self.state.update(cx, |state, cx| state.move_head(to, true, cx));
        }
    }
}
