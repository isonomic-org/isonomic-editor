// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A headless editing session: a store, a selection and the keys a
//! person would press.
#![allow(dead_code)]

use std::sync::Arc;

use editor_blocks::{normalizer, standard_registry};
use editor_commands as cmd;
use editor_model::*;
use editor_store::Store;

pub struct Session {
    pub store: Store,
    pub selection: Selection,
}

impl Session {
    pub fn new(replica: u64) -> Self {
        let mut store = Store::new(Arc::new(standard_registry()), replica);
        store.set_normalizer(normalizer());
        let first = store.ids().next_id();
        let block = BlockDraft::new(first, kinds::PARAGRAPH);
        let tx =
            Transaction::new(Intent::Command).op(Op::InsertBlock { parent: None, index: 0, block });
        store.apply(&tx, None).unwrap();
        Self { store, selection: Selection::caret(first, 0) }
    }

    pub fn join(other: &Session, replica: u64) -> Self {
        let bytes = other.store.export_snapshot().unwrap();
        let mut store = Store::open(Arc::new(standard_registry()), replica, &bytes).unwrap();
        store.set_normalizer(normalizer());
        let first = store.snapshot().root()[0];
        Self { store, selection: Selection::caret(first, 0) }
    }

    pub fn doc(&self) -> Document {
        self.store.snapshot()
    }

    /// Applies a command's transaction; `false` when it did not apply.
    pub fn run(&mut self, tx: Option<Transaction>) -> bool {
        let Some(tx) = tx else { return false };
        if self.store.apply(&tx, Some(&self.selection)).is_err() {
            return false;
        }
        if let Some(selection) = tx.selection {
            self.selection = selection;
        }
        true
    }

    pub fn type_text(&mut self, text: &str) {
        let mut buffer = [0; 4];
        for ch in text.chars() {
            let doc = self.doc();
            self.run(cmd::insert_text(&doc, &self.selection, ch.encode_utf8(&mut buffer), None));
            let doc = self.doc();
            let rule = cmd::input_rule(&doc, &self.selection, self.store.ids());
            self.run(rule);
        }
    }

    pub fn enter(&mut self) -> bool {
        let doc = self.doc();
        let tx = cmd::split_block(&doc, &self.selection, self.store.ids());
        self.run(tx)
    }

    pub fn backspace(&mut self) -> bool {
        let doc = self.doc();
        self.run(cmd::delete_backward(&doc, &self.selection, cmd::Unit::Grapheme))
    }

    pub fn delete(&mut self) -> bool {
        let doc = self.doc();
        self.run(cmd::delete_forward(&doc, &self.selection, cmd::Unit::Grapheme))
    }

    pub fn tab(&mut self) -> bool {
        let doc = self.doc();
        self.run(cmd::indent(&doc, &self.selection))
    }

    pub fn shift_tab(&mut self) -> bool {
        let doc = self.doc();
        self.run(cmd::outdent(&doc, &self.selection))
    }

    pub fn undo(&mut self) -> bool {
        match self.store.undo() {
            Some(undone) => {
                self.selection = undone.selection.unwrap_or_else(|| self.selection.clone());
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.store.redo() {
            Some(redone) => {
                self.selection = redone.selection.unwrap_or_else(|| self.selection.clone());
                true
            }
            None => false,
        }
    }

    pub fn markdown(&self) -> String {
        editor_formats::to_markdown(&self.doc())
    }

    pub fn block(&self, index: usize) -> BlockId {
        self.doc().flat()[index]
    }

    pub fn text(&self, index: usize) -> String {
        let doc = self.doc();
        doc.get(doc.flat()[index]).unwrap().content.text().to_owned()
    }

    /// Puts the caret back on existing text after a remote change.
    pub fn settle(&mut self) {
        let doc = self.doc();
        let ok = match &self.selection {
            Selection::Text { anchor, head } => [anchor, head].iter().all(|at| {
                doc.get(at.block).is_some_and(|block| {
                    block.content.has_text() && block.content.text().is_char_boundary(at.offset)
                })
            }),
            _ => false,
        };
        if !ok {
            let first = doc.flat().into_iter().find(|id| doc.get(*id).unwrap().content.has_text());
            self.selection = Selection::caret(first.unwrap(), 0);
        }
    }
}

pub fn sync(a: &mut Session, b: &mut Session) {
    let to_b = a.store.export_since(&b.store.version()).unwrap();
    let to_a = b.store.export_since(&a.store.version()).unwrap();
    b.store.import(&to_b).unwrap();
    a.store.import(&to_a).unwrap();
    // Repairs made after an import are local edits; exchange those too.
    let to_b = a.store.export_since(&b.store.version()).unwrap();
    let to_a = b.store.export_since(&a.store.version()).unwrap();
    b.store.import(&to_b).unwrap();
    a.store.import(&to_a).unwrap();
    a.settle();
    b.settle();
}
