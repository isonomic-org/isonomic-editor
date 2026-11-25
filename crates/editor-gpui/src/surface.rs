// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The element that wraps the drawn blocks. It collects each text
//! leaf's geometry while the blocks prepaint, registers the platform
//! text input handler, and paints the caret.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use editor_model::BlockId;
use ui::base::text::editor_host::{Caret, LeafGeometry, collect_leaf_geometry};
use ui::{
    AnyElement, App, Bounds, DispatchPhase, Element, ElementId, ElementInputHandler, Entity,
    FocusHandle, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, ListOffset,
    ListState, MouseMoveEvent, MouseUpEvent, Pixels, Window, fill, px, size,
};

use crate::rows::Drawn;
use crate::view::EditorView;

/// How wide the caret is drawn.
const CARET_WIDTH: Pixels = px(2.);

/// What the last frame drew: each text leaf's geometry by key, and
/// the rows it asked for, in visual order, with the blocks they stand
/// for. A row just outside the view is asked for but has no geometry.
#[derive(Default)]
pub struct Frame {
    pub leaves: HashMap<u64, LeafGeometry>,
    pub order: Vec<(u64, BlockId)>,
}

pub type SharedFrame = Rc<RefCell<Frame>>;

/// A row's height before it has been measured, in line heights.
const ROW_ESTIMATE: f32 = 1.5;

/// The selection's moving end, for this frame.
#[derive(Clone, Copy)]
pub struct Head {
    /// Its leaf and byte offset.
    pub key: u64,
    pub offset: usize,
    /// Its row, when this frame should bring it into view.
    pub row: Option<usize>,
    /// Whether the selection is a caret, which is drawn.
    pub caret: bool,
}

pub struct Surface {
    pub child: Option<AnyElement>,
    pub decorations: HashMap<u64, Vec<(Range<usize>, Hsla)>>,
    pub frame: SharedFrame,
    /// The rows the list asks for while it lays out.
    pub drawn: Drawn,
    pub head: Option<Head>,
    /// Whether the caret is in the shown half of its blink.
    pub caret_shown: bool,
    pub caret_color: Hsla,
    pub focus: FocusHandle,
    pub view: Entity<EditorView>,
    /// The list of rows.
    pub list: ListState,
    /// Whether to bring the head into view this frame, whether this
    /// frame is already a second try at that, and where to ask for one.
    pub reveal: bool,
    pub retry: bool,
    pub again: Rc<Cell<bool>>,
    /// The list width that row heights were last estimated for.
    pub estimated: Rc<Cell<Pixels>>,
}

impl IntoElement for Surface {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Surface {
    type RequestLayoutState = Option<AnyElement>;
    type PrepaintState = Option<Bounds<Pixels>>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Option<AnyElement>) {
        match self.child.take() {
            Some(mut child) => (child.request_layout(window, cx), Some(child)),
            None => (window.request_layout(Default::default(), [], cx), None),
        }
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut Option<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let decorations = std::mem::take(&mut self.decorations);
        let (_, leaves) = collect_leaf_geometry(decorations, || {
            if let Some(child) = child {
                child.prepaint(window, cx);
            }
        });
        let head = self.head.and_then(|head| {
            let leaf = leaves.get(&head.key)?;
            let at = leaf.position_for_caret(Caret::new(head.offset.min(leaf.len())));
            Some(Bounds::new(at, size(CARET_WIDTH, leaf.caret_height())))
        });
        {
            let mut frame = self.frame.borrow_mut();
            frame.leaves = leaves;
            frame.order = self.drawn.borrow().values().copied().collect();
        }
        // The list forgets every height when its width changes. Rows
        // it has not measured get an estimate, so the scrollbar spans
        // the whole document from the first frame.
        let view = self.list.viewport_bounds();
        if self.estimated.get() != view.size.width {
            self.estimated.set(view.size.width);
            let estimate = window.line_height() * ROW_ESTIMATE;
            let _ = self.list.clone().with_uniform_item_height(estimate);
        }
        match (self.reveal, head, self.head.and_then(|head| head.row)) {
            (true, Some(head), _) => {
                let (top, bottom) = (head.origin.y, head.origin.y + head.size.height);
                let (view_top, view_bottom) = (view.origin.y, view.origin.y + view.size.height);
                let down = if top < view_top {
                    top - view_top
                } else if bottom > view_bottom {
                    bottom - view_bottom
                } else {
                    px(0.)
                };
                if down != px(0.) && view.size.height > head.size.height {
                    self.list.scroll_by(down);
                    window.refresh();
                }
            }
            // The estimates put the head's row out of view: start the
            // view at that row, which cannot miss, and line up again.
            (true, None, Some(row)) if !self.retry => {
                self.list.scroll_to(ListOffset { item_ix: row, offset_in_item: px(0.) });
                self.again.set(true);
                window.refresh();
            }
            _ => {}
        }
        head.filter(|_| self.head.is_some_and(|head| head.caret))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Option<AnyElement>,
        caret: &mut Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = child {
            child.paint(window, cx);
        }
        // A drag follows the pointer wherever it goes, so the view
        // hears every move and release in the window.
        let view = self.view.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble {
                view.update(cx, |view, cx| view.mouse_move(event, window, cx));
            }
        });
        let view = self.view.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble {
                view.update(cx, |view, _| view.end_drag());
            }
        });
        let handler = ElementInputHandler::new(bounds, self.view.clone());
        window.handle_input(&self.focus, handler, cx);
        if let Some(caret) = caret
            && self.caret_shown
            && self.focus.is_focused(window)
        {
            window.paint_quad(fill(*caret, self.caret_color));
        }
    }
}
