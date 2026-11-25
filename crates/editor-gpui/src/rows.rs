// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! One frame's rows. The view lists every block in document order; the
//! list asks for the rows it is about to show, and only those blocks
//! are turned into elements.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use editor_model::{BlockId, Document, kinds};
use ui::base::text::editor_host::{FlowPosition, RenderContext};
use ui::{AnyElement, App, Hsla, IntoElement as _, ParentElement as _, Styled as _, Window, div};

use crate::nodes::{PLACEHOLDER, node_for};

/// How far each level of nesting is indented, in rems.
const INDENT_REMS: f32 = 1.5;

/// A stable number per block: its leaf key and its element ID.
#[derive(Default)]
pub struct Keys {
    map: HashMap<BlockId, u64>,
    next: u64,
}

impl Keys {
    /// The block's key, made on first use.
    pub fn of(&mut self, id: BlockId) -> u64 {
        *self.map.entry(id).or_insert_with(|| {
            self.next += 1;
            self.next
        })
    }

    pub fn get(&self, id: BlockId) -> Option<u64> {
        self.map.get(&id).copied()
    }
}

pub type SharedKeys = Rc<RefCell<Keys>>;

/// The rows asked for this frame, by index: each one's key and block.
pub type Drawn = Rc<RefCell<BTreeMap<usize, (u64, BlockId)>>>;

/// Everything a row needs to draw itself.
pub struct Rows {
    pub doc: Document,
    pub order: Rc<Vec<BlockId>>,
    pub keys: SharedKeys,
    pub highlight: Hsla,
    /// The color of a hint in an empty block.
    pub muted: Hsla,
    /// The block the caret is in, when the selection is a caret.
    pub caret_in: Option<BlockId>,
    pub drawn: Drawn,
    /// Places in runs of numbered items, worked out once per frame.
    pub numbers: RefCell<HashMap<BlockId, u32>>,
}

impl Rows {
    fn numbered(&self, id: BlockId) -> bool {
        self.doc.get(id).is_some_and(|block| block.kind.is(kinds::NUMBERED))
    }

    /// A numbered item's place in its run of numbered siblings, from
    /// one. Anything else is zero.
    fn number(&self, id: BlockId) -> u32 {
        if !self.numbered(id) {
            return 0;
        }
        if let Some(number) = self.numbers.borrow().get(&id) {
            return *number;
        }
        let siblings = self.doc.children(self.doc.parent(id));
        let at = self.doc.index_in_parent(id).unwrap_or(0).min(siblings.len());
        // Walk back to the start of the run, or to a sibling already
        // numbered this frame, then number forward from there.
        let mut numbers = self.numbers.borrow_mut();
        let mut start = at;
        let mut before = 0;
        while start > 0 && self.numbered(siblings[start - 1]) {
            if let Some(known) = numbers.get(&siblings[start - 1]) {
                before = *known;
                break;
            }
            start -= 1;
        }
        for (offset, sibling) in siblings[start..=at.min(siblings.len() - 1)].iter().enumerate() {
            numbers.insert(*sibling, before + offset as u32 + 1);
        }
        numbers.get(&id).copied().unwrap_or(1)
    }

    /// The element for the row at `ix`.
    pub fn row(&self, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let block = self.order.get(ix).and_then(|id| Some((*id, self.doc.get(*id)?)));
        let Some((id, block)) = block else { return div().into_any_element() };
        let key = self.keys.borrow_mut().of(id);
        self.drawn.borrow_mut().insert(ix, (key, id));
        // An empty paragraph with the caret in it says what can be typed.
        let hint = (self.caret_in == Some(id)
            && block.kind.is(kinds::PARAGRAPH)
            && block.content.is_empty())
        .then_some(PLACEHOLDER);
        let node = node_for(&self.doc, block, key, self.number(id), self.highlight, hint);
        // Spacing depends on what comes before, so build that too.
        let before = ix
            .checked_sub(1)
            .and_then(|at| self.order.get(at))
            .and_then(|id| self.doc.get(*id))
            .map(|block| node_for(&self.doc, block, 0, 0, self.highlight, None));
        let last = ix + 1 == self.order.len();
        let flow = match &before {
            Some(before) => FlowPosition::at(&[before, &node], 1, last),
            None => FlowPosition::at(&[&node], 0, last),
        };
        let depth = self.doc.depth(id).saturating_sub(1);
        let mut rcx = RenderContext::from_app(cx);
        if let Some(hint) = hint {
            rcx = rcx.with_foreground_ranges(key, vec![(0..hint.len(), self.muted)]);
        }
        div()
            .pl(window.rem_size() * INDENT_REMS * depth as f32)
            .child(node.render_with_id(("editor-block", key as usize), flow, &rcx, window, cx))
            .into_any_element()
    }
}
