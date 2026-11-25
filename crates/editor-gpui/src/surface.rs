// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The element that wraps the drawn blocks. It collects each text
//! leaf's geometry while the blocks prepaint, registers the platform
//! text input handler, and paints the caret.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use editor_model::BlockId;
use ui::base::text::editor_host::{Caret, LeafGeometry, collect_leaf_geometry};
use ui::{
    AnyElement, App, Bounds, Element, ElementId, ElementInputHandler, Entity, FocusHandle,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels, Window, fill, px,
    size,
};

use crate::view::EditorView;

/// How wide the caret is drawn.
const CARET_WIDTH: Pixels = px(2.);

/// What the last frame drew: each text leaf's geometry by key, and
/// the keys in visual order with the blocks they stand for.
#[derive(Default)]
pub struct Frame {
    pub leaves: HashMap<u64, LeafGeometry>,
    pub order: Vec<(u64, BlockId)>,
}

pub type SharedFrame = Rc<RefCell<Frame>>;

pub struct Surface {
    pub child: Option<AnyElement>,
    pub decorations: HashMap<u64, Vec<(Range<usize>, Hsla)>>,
    pub frame: SharedFrame,
    /// The caret's leaf and byte offset, when the selection is a caret.
    pub caret: Option<(u64, usize)>,
    pub caret_color: Hsla,
    pub focus: FocusHandle,
    pub view: Entity<EditorView>,
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
        let caret = self.caret.and_then(|(key, offset)| {
            let leaf = leaves.get(&key)?;
            let at = leaf.position_for_caret(Caret::new(offset.min(leaf.len())));
            Some(Bounds::new(at, size(CARET_WIDTH, leaf.caret_height())))
        });
        self.frame.borrow_mut().leaves = leaves;
        caret
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
        let handler = ElementInputHandler::new(bounds, self.view.clone());
        window.handle_input(&self.focus, handler, cx);
        if let Some(caret) = caret
            && self.focus.is_focused(window)
        {
            window.paint_quad(fill(*caret, self.caret_color));
        }
    }
}
