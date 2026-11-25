// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;
use std::sync::Arc;

use editor_blocks::{normalizer, standard_registry};
use editor_commands as cmd;
use editor_model::{
    BlockDraft, BlockId, Content, Document, IdSource as _, Intent, MarkKind, Marks, Op, Position,
    Selection, Transaction, kinds, next_grapheme, prev_grapheme,
};
use editor_store::Store;
use ui::{Context, EventEmitter};

use crate::menu::InsertMenu;

/// What an [`EditorState`] tells its observers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    /// The document changed.
    Edited,
    /// Only the selection moved.
    SelectionChanged,
}

/// One document being edited: its store, its selection and the marks
/// waiting for the next typed character.
pub struct EditorState {
    pub(crate) store: Store,
    pub(crate) selection: Selection,
    pending: Option<Marks>,
    /// Text the platform is still composing: block and byte range.
    composing: Option<(BlockId, Range<usize>)>,
    pub(crate) menu: Option<InsertMenu>,
}

impl EventEmitter<EditorEvent> for EditorState {}

impl EditorState {
    /// An empty document. `replica` must be unique among the replicas
    /// that will ever edit it.
    pub fn new(replica: u64, _cx: &mut Context<Self>) -> Self {
        let mut store = Store::new(Arc::new(standard_registry()), replica);
        store.set_normalizer(normalizer());
        let block = BlockDraft::new(store.ids().next_id(), kinds::PARAGRAPH);
        let first = Op::InsertBlock { parent: None, index: 0, block };
        let _ = store.apply(&Transaction::new(Intent::Command).op(first), None);
        Self::over(store)
    }

    /// A document read from Markdown.
    pub fn from_markdown(markdown: &str, replica: u64, _cx: &mut Context<Self>) -> Self {
        let registry = Arc::new(standard_registry());
        let mut store = Store::new(registry.clone(), replica);
        store.set_normalizer(normalizer());
        let doc = editor_formats::from_markdown(markdown, registry, store.ids());
        let tx = Transaction { ops: doc.insert_ops(), selection: None, intent: Intent::Command };
        let _ = store.apply(&tx, None);
        Self::over(store)
    }

    /// An editor over an existing store.
    pub fn over(store: Store) -> Self {
        let mut state = Self {
            store,
            selection: Selection::Blocks(Vec::new()),
            pending: None,
            composing: None,
            menu: None,
        };
        state.settle();
        state
    }

    pub fn snapshot(&self) -> Document {
        self.store.snapshot()
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn markdown(&self) -> String {
        editor_formats::to_markdown(&self.snapshot())
    }

    pub fn can_undo(&self) -> bool {
        self.store.can_undo()
    }

    /// The store, for sync and for agents. After changing it from
    /// outside, call [`EditorState::refresh`].
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// Call after the store changed from outside this state.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.settle();
        self.edited(cx);
    }

    pub(crate) fn composing(&self) -> Option<&(BlockId, Range<usize>)> {
        self.composing.as_ref()
    }

    pub(crate) fn set_composing(&mut self, composing: Option<(BlockId, Range<usize>)>) {
        self.composing = composing;
    }

    /// The caret or the moving end of the selection.
    pub fn head(&self) -> Option<Position> {
        self.selection.head()
    }

    pub(crate) fn text_of(&self, id: BlockId) -> String {
        self.snapshot().get(id).map(|block| block.content.text().to_owned()).unwrap_or_default()
    }

    fn valid(doc: &Document, at: &Position) -> bool {
        doc.get(at.block).is_some_and(|block| {
            block.content.has_text() && block.content.text().is_char_boundary(at.offset)
        })
    }

    /// Puts the selection back on existing text if a change removed
    /// what it pointed at.
    fn settle(&mut self) {
        let doc = self.snapshot();
        let ok = match &self.selection {
            Selection::Text { anchor, head } => {
                Self::valid(&doc, anchor) && Self::valid(&doc, head)
            }
            Selection::Blocks(blocks) => {
                !blocks.is_empty() && blocks.iter().all(|id| doc.contains(*id))
            }
            Selection::Cells { table, .. } => doc.contains(*table),
        };
        if !ok {
            let first = doc
                .flat()
                .into_iter()
                .find(|id| doc.get(*id).is_some_and(|block| block.content.has_text()));
            self.selection =
                first.map_or(Selection::Blocks(Vec::new()), |id| Selection::caret(id, 0));
        }
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        self.follow_menu();
        cx.emit(EditorEvent::Edited);
        cx.notify();
    }

    /// Moves the selection without editing.
    pub fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        if self.selection != selection {
            self.selection = selection;
            self.settle();
            self.pending = None;
            self.follow_menu();
            cx.emit(EditorEvent::SelectionChanged);
            cx.notify();
        }
    }

    /// Moves the head to a position; the anchor follows unless
    /// `extend` is set.
    pub fn move_head(&mut self, to: Position, extend: bool, cx: &mut Context<Self>) {
        let anchor = match (&self.selection, extend) {
            (Selection::Text { anchor, .. }, true) => *anchor,
            _ => to,
        };
        self.select(Selection::Text { anchor, head: to }, cx);
    }

    /// Applies a command's transaction. `false` when it did not apply.
    pub(crate) fn run(&mut self, tx: Option<Transaction>, cx: &mut Context<Self>) -> bool {
        let Some(tx) = tx else { return false };
        if self.store.apply(&tx, Some(&self.selection)).is_err() {
            return false;
        }
        if let Some(selection) = tx.selection {
            self.selection = selection;
        }
        self.settle();
        self.edited(cx);
        true
    }

    /// Types text at the selection, then lets an input rule fire.
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        let marks = self.pending.take();
        if !self.run(cmd::insert_text(&doc, &self.selection, text, marks), cx) {
            return;
        }
        let doc = self.snapshot();
        let rule = cmd::input_rule(&doc, &self.selection, self.store.ids());
        self.run(rule, cx);
        self.open_menu_if_triggered(text, cx);
    }

    pub fn backspace(&mut self, word: bool, cx: &mut Context<Self>) {
        let unit = if word { cmd::Unit::Word } else { cmd::Unit::Grapheme };
        let doc = self.snapshot();
        self.run(cmd::delete_backward(&doc, &self.selection, unit), cx);
    }

    pub fn delete(&mut self, word: bool, cx: &mut Context<Self>) {
        let unit = if word { cmd::Unit::Word } else { cmd::Unit::Grapheme };
        let doc = self.snapshot();
        self.run(cmd::delete_forward(&doc, &self.selection, unit), cx);
    }

    pub fn newline(&mut self, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        let tx = cmd::split_block(&doc, &self.selection, self.store.ids());
        self.run(tx, cx);
    }

    pub fn indent(&mut self, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        self.run(cmd::indent(&doc, &self.selection), cx);
    }

    pub fn outdent(&mut self, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        self.run(cmd::outdent(&doc, &self.selection), cx);
    }

    /// Toggles a mark on the selection, or with a caret arms it for
    /// the next typed text.
    pub fn toggle_mark(&mut self, kind: MarkKind, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        if !self.selection.is_caret() {
            self.run(cmd::toggle_mark(&doc, &self.selection, kind, None), cx);
            return;
        }
        let Some(head) = self.head() else { return };
        let Some(Content::Inline(text)) = doc.get(head.block).map(|block| &block.content) else {
            return;
        };
        let mut marks = self.pending.take().unwrap_or_else(|| text.marks_at(head.offset));
        match marks.has(kind) {
            true => marks.clear(kind),
            false => marks.set(&editor_model::Mark::new(kind, None)),
        }
        self.pending = Some(marks);
        cx.notify();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(undone) = self.store.undo() {
            self.selection = undone.selection.unwrap_or_else(|| self.selection.clone());
            self.settle();
            self.edited(cx);
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(redone) = self.store.redo() {
            self.selection = redone.selection.unwrap_or_else(|| self.selection.clone());
            self.settle();
            self.edited(cx);
        }
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        if let Some(selection) = cmd::select_all(&doc, &self.selection) {
            self.select(selection, cx);
        }
    }

    /// The selection as Markdown, for the clipboard.
    pub fn copy(&self) -> Option<String> {
        let fragment = cmd::copy_fragment(&self.snapshot(), &self.selection)?;
        Some(editor_formats::to_markdown(&fragment))
    }

    /// Copies the selection and removes it.
    pub fn cut(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let copied = self.copy()?;
        let doc = self.snapshot();
        self.run(cmd::delete_selection(&doc, &self.selection), cx);
        Some(copied)
    }

    /// Pastes Markdown at the selection: one line goes in as text,
    /// anything more as blocks.
    pub fn paste(&mut self, markdown: &str, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        if markdown.len() > doc.limits().paste_bytes {
            return;
        }
        let fragment =
            editor_formats::from_markdown(markdown, doc.registry().clone(), self.store.ids());
        let tx = cmd::insert_fragment(&doc, &self.selection, &fragment, self.store.ids());
        self.run(tx, cx);
    }

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

    /// Moves to the start or the end of the block's text.
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

    pub fn move_to_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(head) = self.head() else { return };
        let offset = if end { self.text_of(head.block).len() } else { 0 };
        self.move_head(Position::new(head.block, offset), extend, cx);
    }
}
