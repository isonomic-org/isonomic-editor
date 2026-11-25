// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use editor_model::{BlockDraft, BlockId, Changes, Document, IdSource, Op, kinds};

use crate::build::Builder;

fn misplaced(doc: &Document, id: BlockId) -> bool {
    let (Some(block), Some(parent)) = (doc.get(id), doc.parent(id).and_then(|p| doc.get(p))) else {
        return false;
    };
    !doc.registry().allows_child(&parent.kind, &block.kind)
}

/// The repairs a document needs after `changes`. Deterministic, and
/// empty for a valid document, so two replicas never fight.
///
/// Rules: a document has at least one block; its last block carries
/// text; a block its parent does not accept is lifted out, never
/// deleted. Blocks of unknown types are left alone.
pub fn normalize(doc: &Document, changes: &Changes, ids: &mut dyn IdSource) -> Vec<Op> {
    let mut suspects: Vec<BlockId> = Vec::new();
    for id in changes.touched() {
        suspects.push(id);
        suspects.extend(doc.children(Some(id)));
    }
    suspects.retain(|id| misplaced(doc, *id));
    suspects.sort_by_key(|id| doc.path(*id));
    suspects.dedup();
    let mut build = Builder::new(doc);
    for id in suspects.into_iter().rev() {
        let mut guard = doc.limits().depth;
        while misplaced(&build.doc, id) && guard > 0 {
            guard -= 1;
            let Some(parent) = build.doc.parent(id) else {
                break;
            };
            if build.move_after(id, parent).is_none() {
                break;
            }
        }
    }
    let last = build.doc.root().last().and_then(|id| build.doc.get(*id));
    if !last.is_some_and(|block| block.content.has_text()) {
        let index = build.doc.root().len();
        let block = BlockDraft::new(ids.next_id(), kinds::PARAGRAPH);
        let _ = build.push(Op::InsertBlock { parent: None, index, block });
    }
    build.ops
}
