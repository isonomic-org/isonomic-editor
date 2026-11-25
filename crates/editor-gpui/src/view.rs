// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use editor_commands::selection_ranges;
use editor_model::{BlockId, Document, MarkKind, Position, Selection};
use ui::base::Scrollbar;
use ui::base::text::editor_host::Caret;
use ui::component::ActiveTheme as _;
use ui::{
    App, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, ListAlignment, ListState, MouseButton,
    ParentElement as _, Pixels, Point, Render, Styled as _, Subscription, Window, div, list, px,
};

use crate::actions::*;
use crate::rows::{Drawn, Rows, SharedKeys};
use crate::state::EditorState;
use crate::surface::{Head, SharedFrame, Surface};

/// How far past the window's edges rows are measured ahead of being
/// scrolled to.
const OVERDRAW: f32 = 600.;
/// Above this many new rows at once, every row not yet measured is
/// given an estimated height again, so the scrollbar is about right
/// before they are seen.
const ESTIMATE_ABOVE: usize = 64;

/// Draws an [`EditorState`] and routes input to it.
pub struct EditorView {
    pub(crate) state: Entity<EditorState>,
    pub(crate) focus: FocusHandle,
    pub(crate) frame: SharedFrame,
    /// A stable number per block: its leaf key and its element ID.
    pub(crate) keys: SharedKeys,
    pub(crate) dragging: bool,
    /// The rows: every block in document order, of which the list
    /// draws the ones in view.
    pub(crate) list: ListState,
    listed: Rc<Vec<BlockId>>,
    drawn: Drawn,
    /// Set by the surface when the head's row turned out not to be
    /// drawn after all, to try once more on the next frame.
    reveal_again: Rc<Cell<bool>>,
    /// The list width that row heights were last estimated for.
    estimated: Rc<Cell<Pixels>>,
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
            keys: SharedKeys::default(),
            dragging: false,
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
            listed: Rc::default(),
            drawn: Drawn::default(),
            reveal_again: Rc::default(),
            estimated: Rc::default(),
            last_head: None,
            _observe: observe,
        }
    }

    pub fn state(&self) -> &Entity<EditorState> {
        &self.state
    }

    /// How many text leaves the last frame drew.
    pub fn drawn_leaves(&self) -> usize {
        self.frame.borrow().leaves.len()
    }

    /// Where a block's text was drawn in the last frame.
    pub fn leaf_bounds(&self, id: BlockId) -> Option<Bounds<Pixels>> {
        let frame = self.frame.borrow();
        Some(frame.leaves.get(&self.keys.borrow().get(id)?)?.bounds())
    }

    /// Where a caret at this position was, or would be, drawn.
    pub fn caret_point(&self, at: Position) -> Option<Point<Pixels>> {
        let frame = self.frame.borrow();
        let leaf = frame.leaves.get(&self.keys.borrow().get(at.block)?)?;
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

    /// Tells the list which rows changed since the last frame: the
    /// blocks between the longest unchanged start and end.
    fn list_rows(&mut self, order: &Rc<Vec<BlockId>>) {
        let old = &self.listed;
        if **old == **order {
            return;
        }
        let start = old.iter().zip(order.iter()).take_while(|(a, b)| a == b).count();
        let most = old.len().min(order.len()) - start;
        let end =
            old.iter().rev().zip(order.iter().rev()).take(most).take_while(|(a, b)| a == b).count();
        let added = order.len() - start - end;
        self.list.splice(start..old.len() - end, added);
        if added > ESTIMATE_ABOVE {
            // Estimate again after the next layout.
            self.estimated.take();
        }
        self.listed = order.clone();
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
            if range.start < range.end {
                let key = self.keys.borrow_mut().of(id);
                out.entry(key).or_default().push((range, color));
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (doc, selection) = {
            let state = self.state.read(cx);
            (state.snapshot(), state.selection().clone())
        };
        let (selection_color, caret_color) = (cx.theme().selection, cx.theme().caret);
        let order = Rc::new(doc.flat());
        self.list_rows(&order);
        // The head is where the caret goes and what scrolling follows.
        let head = selection.head();
        let retry = self.reveal_again.take();
        let reveal = head != self.last_head || retry;
        self.last_head = head;
        let head = head.map(|head| Head {
            key: self.keys.borrow_mut().of(head.block),
            offset: head.offset,
            row: if reveal { order.iter().position(|id| *id == head.block) } else { None },
            caret: selection.is_caret(),
        });
        if let Some(Head { key, row: Some(row), .. }) = head
            && !retry
            && !self.frame.borrow().leaves.contains_key(&key)
        {
            // A head in a row that is not drawn is brought into the
            // list's view first; the surface then lines up the caret.
            self.list.scroll_to_reveal_item(row);
        }
        let decorations = self.decorations(&doc, &selection, selection_color);
        self.drawn.borrow_mut().clear();
        let rows = Rows {
            doc,
            order,
            keys: self.keys.clone(),
            highlight: selection_color,
            drawn: self.drawn.clone(),
            numbers: Default::default(),
        };
        let rows = list(self.list.clone(), move |ix, window, cx| rows.row(ix, window, cx));
        let surface = Surface {
            child: Some(rows.size_full().into_any_element()),
            decorations,
            frame: self.frame.clone(),
            drawn: self.drawn.clone(),
            head,
            caret_color,
            focus: self.focus.clone(),
            view: cx.entity(),
            list: self.list.clone(),
            reveal,
            retry,
            again: self.reveal_again.clone(),
            estimated: self.estimated.clone(),
        };
        div()
            .id("isonomic-editor")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .size_full()
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
            .on_action(cx.listener(|this, _: &Newline, _, cx| {
                this.edit(cx, |s, cx| match s.insert_menu() {
                    Some(_) => s.choose_menu_entry(None, cx),
                    None => s.newline(cx),
                })
            }))
            .on_action(
                cx.listener(|this, _: &Cancel, _, cx| this.edit(cx, |s, cx| s.close_menu(cx))),
            )
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
            .on_action(
                cx.listener(|this, _: &MoveToStart, _, cx| {
                    this.move_to_line_edge(false, false, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &MoveToEnd, _, cx| this.move_to_line_edge(true, false, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectToStart, _, cx| {
                    this.move_to_line_edge(false, true, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &SelectToEnd, _, cx| this.move_to_line_edge(true, true, cx)),
            )
            .on_action(cx.listener(|this, _: &MoveUp, _, cx| this.move_or_step_menu(false, cx)))
            .on_action(cx.listener(|this, _: &MoveDown, _, cx| this.move_or_step_menu(true, cx)))
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
            .children(self.render_menu(cx))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(Scrollbar::width())
                    .child(Scrollbar::vertical(&self.list).viewport_from_layout()),
            )
    }
}

impl EditorView {
    /// Up and Down step through the insert menu while it is open.
    fn move_or_step_menu(&mut self, down: bool, cx: &mut Context<Self>) {
        if self.state.read(cx).insert_menu().is_some() {
            self.state.update(cx, |state, cx| state.step_menu(down, cx));
        } else {
            self.move_vertically(down, false, cx);
        }
    }

    fn edit(
        &mut self,
        cx: &mut Context<Self>,
        run: impl FnOnce(&mut EditorState, &mut Context<EditorState>),
    ) {
        self.state.update(cx, run);
    }
}
