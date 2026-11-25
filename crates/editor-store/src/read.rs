// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Building a model snapshot from the replicated document.

use std::collections::HashMap;
use std::sync::Arc;

use editor_model::{
    BlockDraft, BlockId, BlockKind, Changes, Document, InlineText, Limits, Op, Props, Registry,
};
use loro::{
    Container, LoroMap, LoroText, LoroTree, LoroValue, TreeID, TreeParentId, ValueOrContainer,
};

use crate::convert;

/// Key prefixes in a block's replicated map. Properties and tunes
/// are plain values under prefixed keys, so a block owns exactly one
/// nested object: its text.
pub const PROP: &str = "p:";
pub const TUNE: &str = "t:";

/// The replicated objects behind one block.
#[derive(Clone)]
pub struct Handles {
    pub node: TreeID,
    pub meta: LoroMap,
    pub text: LoroText,
}

pub fn text_of(text: &LoroText) -> InlineText {
    convert::inline_from(&text.to_delta())
}

fn child_text(meta: &LoroMap) -> Option<LoroText> {
    match meta.get("text")? {
        ValueOrContainer::Container(Container::Text(text)) => Some(text),
        _ => None,
    }
}

/// A block's ID, type, properties and tunes, from its map's values.
fn fields(meta: &LoroMap) -> Option<(BlockId, Option<String>, Props, Props)> {
    let LoroValue::Map(map) = meta.get_value() else { return None };
    let mut id = None;
    let mut kind = None;
    let (mut props, mut tunes) = (Props::new(), Props::new());
    for (key, value) in map.iter() {
        if let Some(name) = key.strip_prefix(PROP) {
            props.insert(name.to_owned(), convert::from_value(value));
        } else if let Some(name) = key.strip_prefix(TUNE) {
            tunes.insert(name.to_owned(), convert::from_value(value));
        } else if let LoroValue::String(text) = value {
            match key.as_str() {
                "id" => id = text.parse::<BlockId>().ok(),
                "kind" => kind = Some(text.to_string()),
                _ => {}
            }
        }
    }
    Some((id?, kind, props, tunes))
}

/// Reads the whole tree. Nodes that are malformed, duplicated or past
/// a limit are skipped with everything under them; nothing panics.
pub fn materialize(
    tree: &LoroTree,
    registry: &Arc<Registry>,
    limits: &Limits,
) -> (Document, HashMap<BlockId, Handles>) {
    let mut doc = Document::with_limits(registry.clone(), limits.clone());
    let mut handles = HashMap::new();
    let roots = tree.children(TreeParentId::Root).unwrap_or_default();
    let mut stack: Vec<(Option<BlockId>, TreeID)> =
        roots.into_iter().rev().map(|node| (None, node)).collect();
    while let Some((parent, node)) = stack.pop() {
        let Ok(meta) = tree.get_meta(node) else { continue };
        let (Some((id, kind, props, tunes)), Some(text)) = (fields(&meta), child_text(&meta))
        else {
            continue;
        };
        let kind = kind.unwrap_or_else(|| registry.default_kind().to_string());
        let draft =
            BlockDraft { id, kind: BlockKind::new(&kind), props, tunes, text: text_of(&text) };
        let index = doc.children(parent).len();
        if doc.apply(&Op::InsertBlock { parent, index, block: draft }).is_err() {
            continue;
        }
        handles.insert(id, Handles { node, meta, text });
        let children = tree.children(TreeParentId::Node(node)).unwrap_or_default();
        stack.extend(children.into_iter().rev().map(|child| (Some(id), child)));
    }
    (doc, handles)
}

/// What differs between two snapshots.
pub fn diff(old: &Document, new: &Document) -> Changes {
    let mut changes = Changes::default();
    for id in new.flat() {
        match (old.get(id), new.get(id)) {
            (None, _) => changes.added.push(id),
            (Some(before), Some(after)) => {
                if old.parent(id) != new.parent(id) {
                    changes.moved.push(id);
                }
                if before != after {
                    changes.changed.push(id);
                }
            }
            _ => {}
        }
    }
    changes.removed = old.flat().into_iter().filter(|id| !new.contains(*id)).collect();
    changes
}
