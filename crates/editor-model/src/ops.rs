// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use crate::{
    BlockDraft, BlockId, BlockKind, Document, IdSource, InlineText, Mark, MarkKind, PropValue,
    Selection,
};

/// Produces the repairs a document needs after a change. It must be
/// deterministic and return nothing for a valid document.
pub type Normalizer = Box<dyn FnMut(&Document, &Changes, &mut dyn IdSource) -> Vec<Op> + Send>;

/// The primitive operations. Every edit is a list of these.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Inserts a block under `parent` (the root when `None`).
    InsertBlock {
        parent: Option<BlockId>,
        index: usize,
        block: BlockDraft,
    },
    /// Removes a block and everything under it.
    RemoveBlock {
        id: BlockId,
    },
    /// Moves a block. `index` counts the destination's children after
    /// the block has left its old place.
    MoveBlock {
        id: BlockId,
        parent: Option<BlockId>,
        index: usize,
    },
    SetKind {
        id: BlockId,
        kind: BlockKind,
    },
    /// Sets a property, or clears it with `None`.
    SetProp {
        id: BlockId,
        key: String,
        value: Option<PropValue>,
    },
    /// Sets a tune, or clears it with `None`.
    SetTune {
        id: BlockId,
        key: String,
        value: Option<PropValue>,
    },
    ReplaceText {
        id: BlockId,
        range: Range<usize>,
        with: InlineText,
    },
    AddMark {
        id: BlockId,
        range: Range<usize>,
        mark: Mark,
    },
    RemoveMark {
        id: BlockId,
        range: Range<usize>,
        kind: MarkKind,
    },
}

impl Op {
    /// The block the operation edits, creates, moves or removes.
    pub fn target(&self) -> BlockId {
        match self {
            Self::InsertBlock { block, .. } => block.id,
            Self::RemoveBlock { id }
            | Self::MoveBlock { id, .. }
            | Self::SetKind { id, .. }
            | Self::SetProp { id, .. }
            | Self::SetTune { id, .. }
            | Self::ReplaceText { id, .. }
            | Self::AddMark { id, .. }
            | Self::RemoveMark { id, .. } => *id,
        }
    }
}

/// What a transaction is for. Undo groups steps by intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    Typing,
    Delete,
    Paste,
    Command,
    InputRule,
    Agent,
}

/// An atomic edit: operations plus the selection to show afterwards.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub ops: Vec<Op>,
    pub selection: Option<Selection>,
    pub intent: Intent,
}

impl Transaction {
    pub fn new(intent: Intent) -> Self {
        Self { ops: Vec::new(), selection: None, intent }
    }

    pub fn op(mut self, op: Op) -> Self {
        self.ops.push(op);
        self
    }

    pub fn select(mut self, selection: Selection) -> Self {
        self.selection = Some(selection);
        self
    }
}

/// What one transaction, undo or remote update changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub added: Vec<BlockId>,
    pub removed: Vec<BlockId>,
    pub moved: Vec<BlockId>,
    /// Blocks whose type, properties, tunes or text changed.
    pub changed: Vec<BlockId>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.moved.is_empty()
            && self.changed.is_empty()
    }

    /// Records one applied operation.
    pub fn record(&mut self, op: &Op) {
        let id = op.target();
        let list = match op {
            Op::InsertBlock { .. } => &mut self.added,
            Op::RemoveBlock { .. } => &mut self.removed,
            Op::MoveBlock { .. } => &mut self.moved,
            _ => &mut self.changed,
        };
        if !list.contains(&id) {
            list.push(id);
        }
    }

    /// Every block still present that the change touched.
    pub fn touched(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.added.iter().chain(&self.moved).chain(&self.changed).copied()
    }
}
