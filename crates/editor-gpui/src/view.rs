// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::ops::Range;

use editor_commands::selection_ranges;
use editor_model::{BlockId, Document, MarkKind, Position, Selection, kinds};
use ui::base::text::editor_host::{BlockNode, Caret, FlowPosition, RenderContext};
use ui::component::ActiveTheme as _;
use ui::{
    AnyElement, App, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement as _, Pixels, Point, Render, ScrollHandle, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, point, px,
};

use crate::actions::*;
use crate::nodes::node_for;
use crate::state::EditorState;
use crate::surface::{SharedFrame, Surface};

/// Draws an [`EditorState`] and routes input to it.
pub struct EditorView {
    pub(crate) state: Entity<EditorState>,
    pub(crate) focus: FocusHandle,
    pub(crate) frame: SharedFrame,
    /// A stable number per block: its leaf key and its element ID.
    pub(crate) keys: HashMap<BlockId, u64>,
    next_key: u64,
    dragging: bool,
    scroll: ScrollHandle,
    /// The head as of the last frame; a change scrolls it into view.
    last_head: Option<Position>,
    _observe: Subscription,
}

impl EditorView {
    pub fn new(state: Entity<EditorState>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            focus: cx.focus_handle(),
            frame: SharedFrame::default(),
            keys: HashMap::new(),
            next_key: 1,
            dragging: false,
            scroll: ScrollHandle::new(),
            last_head: None,
            _observe: observe,
        }
    }

    pub fn state(&self) -> &Entity<EditorState> {
        &self.state
    }

    fn key(&mut self, id: BlockId) -> u64 {
        *self.keys.entry(id).or_insert_with(|| {
            self.next_key += 1;
            self.next_key
        })
    }

    /// How many text leaves the last frame drew.
    pub fn drawn_leaves(&self) -> usize {
        self.frame.borrow().leaves.len()
    }

    /// Where a block's text was drawn in the last frame.
    pub fn leaf_bounds(&self, id: BlockId) -> Option<Bounds<Pixels>> {
        let frame = self.frame.borrow();
        Some(frame.leaves.get(self.keys.get(&id)?)?.bounds())
    }

    /// Where a caret at this position was, or would be, drawn.
    pub fn caret_point(&self, at: Position) -> Option<Point<Pixels>> {
        let frame = self.frame.borrow();
        let leaf = frame.leaves.get(self.keys.get(&at.block)?)?;
        Some(leaf.position_for_caret(Caret::new(at.offset.min(leaf.len()))))
    }

    /// The document position nearest a point in the window.
    pub fn position_at(&self, at: Point<Pixels>, cx: &App) -> Option<Position> {
        let frame = self.frame.borrow();
        let mut best: Option<(Pixels, u64, BlockId)> = None;
        for (key, id) in &frame.order {
            let Some(leaf) = frame.leaves.get(key) else { continue };
            let bounds = leaf.bounds();
            let (top, bottom) = (bounds.origin.y, bounds.origin.y + bounds.size.height);
            let distance = if at.y < top {
                top - at.y
            } else if at.y > bottom {
                at.y - bottom
            } else {
                px(0.)
            };
            if best.is_none_or(|(nearest, ..)| distance < nearest) {
                best = Some((distance, *key, *id));
            }
        }
        let (_, key, id) = best?;
        let index = frame.leaves.get(&key)?.index_for_position(at);
        let text = self.state.read(cx).text_of(id);
        let mut offset = index.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        Some(Position::new(id, offset))
    }

    /// Moves the head one visual line up or down, keeping its column.
    fn move_vertically(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(head) = self.state.read(cx).head() else { return };
        let target = {
            let frame = self.frame.borrow();
            let Some(key) = self.keys.get(&head.block) else { return };
            let Some(leaf) = frame.leaves.get(key) else { return };
            let at = leaf.position_for_caret(Caret::new(head.offset.min(leaf.len())));
            let line = leaf.line_height();
            let bounds = leaf.bounds();
            let y = if down { at.y + line * 1.5 } else { at.y - line * 0.5 };
            if y >= bounds.origin.y && y < bounds.origin.y + bounds.size.height {
                Some(point(at.x, y))
            } else {
                // Leave this block: aim at the nearest line of the
                // next drawn text leaf in that direction.
                let here = frame.order.iter().position(|(k, _)| k == key);
                let mut others: Box<dyn Iterator<Item = &(u64, BlockId)>> = match (here, down) {
                    (Some(here), true) => Box::new(frame.order[here + 1..].iter()),
                    (Some(here), false) => Box::new(frame.order[..here].iter().rev()),
                    (None, _) => Box::new(std::iter::empty()),
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
            }
        };
        if let Some(to) = target.and_then(|target| self.position_at(target, cx)) {
            self.state.update(cx, |state, cx| state.move_head(to, extend, cx));
        }
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if let Some(to) = self.position_at(event.position, cx) {
            self.dragging = true;
            let extend = event.modifiers.shift;
            self.state.update(cx, |state, cx| state.move_head(to, extend, cx));
        }
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.dragging || event.pressed_button != Some(MouseButton::Left) {
            self.dragging = false;
            return;
        }
        if let Some(to) = self.position_at(event.position, cx) {
            self.state.update(cx, |state, cx| state.move_head(to, true, cx));
        }
    }

    /// Selection highlights for the frame, by leaf key.
    fn decorations(
        &self,
        doc: &Document,
        selection: &Selection,
        color: Hsla,
    ) -> HashMap<u64, Vec<(Range<usize>, Hsla)>> {
        let mut out: HashMap<u64, Vec<(Range<usize>, Hsla)>> = HashMap::new();
        let mut add = |id: BlockId, range: Range<usize>| {
            if let Some(key) = self.keys.get(&id)
                && range.start < range.end
            {
                out.entry(*key).or_default().push((range, color));
            }
        };
        match selection {
            Selection::Text { .. } => {
                for (id, range) in selection_ranges(doc, selection) {
                    add(id, range);
                }
            }
            Selection::Blocks(blocks) => {
                for id in blocks.iter().flat_map(|id| doc.subtree(*id)) {
                    let len = doc.get(id).map_or(0, |block| block.content.len());
                    add(id, 0..len);
                }
            }
            Selection::Cells { .. } => {}
        }
        out
    }
}

impl Focusable for EditorView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (doc, selection) = {
            let state = self.state.read(cx);
            (state.snapshot(), state.selection().clone())
        };
        let (selection_color, caret_color) = (cx.theme().selection, cx.theme().caret);
        let mut nodes: Vec<BlockNode> = Vec::new();
        let mut order = Vec::new();
        let mut depths = Vec::new();
        let mut numbers: HashMap<Option<BlockId>, u32> = HashMap::new();
        for id in doc.flat() {
            let Some(block) = doc.get(id) else { continue };
            let parent = doc.parent(id);
            let number = if block.kind.is(kinds::NUMBERED) {
                let count = numbers.entry(parent).or_insert(0);
                *count += 1;
                *count
            } else {
                numbers.remove(&parent);
                0
            };
            let key = self.key(id);
            nodes.push(node_for(&doc, block, key, number, selection_color));
            order.push((key, id));
            depths.push(doc.depth(id).saturating_sub(1));
        }
        let rcx = RenderContext::from_app(cx);
        let refs: Vec<&BlockNode> = nodes.iter().collect();
        let indent = window.rem_size() * 1.5;
        let rows: Vec<AnyElement> = refs
            .iter()
            .enumerate()
            .map(|(ix, node)| {
                let flow = FlowPosition::at(&refs, ix, ix + 1 == refs.len());
                let id = ("editor-block", order[ix].0 as usize);
                div()
                    .pl(indent * depths[ix] as f32)
                    .child(node.render_with_id(id, flow, &rcx, window, cx))
                    .into_any_element()
            })
            .collect();
        let caret = match &selection {
            Selection::Text { anchor, head } if anchor == head => {
                self.keys.get(&head.block).map(|key| (*key, head.offset))
            }
            _ => None,
        };
        let head = selection.head();
        let reveal = head != self.last_head;
        self.last_head = head;
        let decorations = self.decorations(&doc, &selection, selection_color);
        self.frame.borrow_mut().order = order;
        let surface = Surface {
            child: Some(div().flex().flex_col().children(rows).into_any_element()),
            decorations,
            frame: self.frame.clone(),
            caret,
            caret_color,
            focus: self.focus.clone(),
            view: cx.entity(),
            scroll: self.scroll.clone(),
            reveal,
        };
        div()
            .id("isonomic-editor")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_action(cx.listener(|this, _: &Backspace, _, cx| {
                this.edit(cx, |s, cx| s.backspace(false, cx))
            }))
            .on_action(cx.listener(|this, _: &BackspaceWord, _, cx| {
                this.edit(cx, |s, cx| s.backspace(true, cx))
            }))
            .on_action(
                cx.listener(|this, _: &Delete, _, cx| this.edit(cx, |s, cx| s.delete(false, cx))),
            )
            .on_action(
                cx.listener(|this, _: &DeleteWord, _, cx| {
                    this.edit(cx, |s, cx| s.delete(true, cx))
                }),
            )
            .on_action(cx.listener(|this, _: &Newline, _, cx| this.edit(cx, |s, cx| s.newline(cx))))
            .on_action(cx.listener(|this, _: &Indent, _, cx| this.edit(cx, |s, cx| s.indent(cx))))
            .on_action(cx.listener(|this, _: &Outdent, _, cx| this.edit(cx, |s, cx| s.outdent(cx))))
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.edit(cx, |s, cx| s.undo(cx))))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.edit(cx, |s, cx| s.redo(cx))))
            .on_action(
                cx.listener(|this, _: &SelectAll, _, cx| this.edit(cx, |s, cx| s.select_all(cx))),
            )
            .on_action(cx.listener(|this, _: &MoveLeft, _, cx| {
                this.edit(cx, |s, cx| s.move_horizontally(false, false, cx))
            }))
            .on_action(cx.listener(|this, _: &MoveRight, _, cx| {
                this.edit(cx, |s, cx| s.move_horizontally(true, false, cx))
            }))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| {
                this.edit(cx, |s, cx| s.move_horizontally(false, true, cx))
            }))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| {
                this.edit(cx, |s, cx| s.move_horizontally(true, true, cx))
            }))
            .on_action(cx.listener(|this, _: &MoveToStart, _, cx| {
                this.edit(cx, |s, cx| s.move_to_edge(false, false, cx))
            }))
            .on_action(cx.listener(|this, _: &MoveToEnd, _, cx| {
                this.edit(cx, |s, cx| s.move_to_edge(true, false, cx))
            }))
            .on_action(cx.listener(|this, _: &SelectToStart, _, cx| {
                this.edit(cx, |s, cx| s.move_to_edge(false, true, cx))
            }))
            .on_action(cx.listener(|this, _: &SelectToEnd, _, cx| {
                this.edit(cx, |s, cx| s.move_to_edge(true, true, cx))
            }))
            .on_action(
                cx.listener(|this, _: &MoveUp, _, cx| this.move_vertically(false, false, cx)),
            )
            .on_action(
                cx.listener(|this, _: &MoveDown, _, cx| this.move_vertically(true, false, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectUp, _, cx| this.move_vertically(false, true, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectDown, _, cx| this.move_vertically(true, true, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleBold, _, cx| {
                this.edit(cx, |s, cx| s.toggle_mark(MarkKind::Bold, cx))
            }))
            .on_action(cx.listener(|this, _: &ToggleItalic, _, cx| {
                this.edit(cx, |s, cx| s.toggle_mark(MarkKind::Italic, cx))
            }))
            .on_action(cx.listener(|this, _: &ToggleUnderline, _, cx| {
                this.edit(cx, |s, cx| s.toggle_mark(MarkKind::Underline, cx))
            }))
            .on_action(cx.listener(|this, _: &ToggleStrike, _, cx| {
                this.edit(cx, |s, cx| s.toggle_mark(MarkKind::Strike, cx))
            }))
            .on_action(cx.listener(|this, _: &ToggleCode, _, cx| {
                this.edit(cx, |s, cx| s.toggle_mark(MarkKind::Code, cx))
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if let Some(text) = this.state.read(cx).copy() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                if let Some(text) = this.state.update(cx, |state, cx| state.cut(cx)) {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    this.state.update(cx, |state, cx| state.paste(&text, cx));
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.dragging = false))
            .child(surface)
    }
}

impl EditorView {
    fn edit(
        &mut self,
        cx: &mut Context<Self>,
        run: impl FnOnce(&mut EditorState, &mut Context<EditorState>),
    ) {
        self.state.update(cx, run);
    }
}
