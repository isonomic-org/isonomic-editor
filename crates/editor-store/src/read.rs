// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Building a model snapshot from the replicated document.

use std::collections::HashMap;
use std::sync::Arc;

use editor_model::{
    BlockDraft, BlockId, BlockKind, Changes, Document, InlineText, Limits, Op, Props, Registry,
};
use loro::{
    Container, ContainerTrait as _, LoroMap, LoroText, LoroTree, LoroValue, TreeID, TreeParentId,
    ValueOrContainer,
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

type Fields = (BlockId, Option<String>, Props, Props);

/// A block's ID, type, properties and tunes, from its map's values.
fn fields(meta: &LoroMap) -> Option<Fields> {
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

/// One block's current type, properties, tunes and text.
pub fn block_of(
    handles: &Handles,
    registry: &Registry,
) -> Option<(BlockKind, Props, Props, InlineText)> {
    let (_, kind, props, tunes) = fields(&handles.meta)?;
    let kind = kind.map_or_else(|| registry.default_kind(), |kind| BlockKind::new(&kind));
    Some((kind, props, tunes, text_of(&handles.text)))
}

/// Reads the whole tree. Nodes that are malformed, duplicated or past
/// a limit are skipped with everything under them; nothing panics.
///
/// The library keeps each block's two objects compressed, in the order
/// of their identifiers, and decodes one the first time it is read. A
/// long document read in document order jumps about in that store and
/// decodes the same stretches again and again, so the maps are read in
/// the store's order, then the texts, and the snapshot is put together
/// in document order afterwards.
pub fn materialize(
    tree: &LoroTree,
    registry: &Arc<Registry>,
    limits: &Limits,
) -> (Document, HashMap<BlockId, Handles>) {
    // Document order: each node with the place of its parent in the list.
    let mut nodes: Vec<(Option<usize>, TreeID)> = Vec::new();
    let roots = tree.children(TreeParentId::Root).unwrap_or_default();
    let mut stack: Vec<(Option<usize>, TreeID)> =
        roots.into_iter().rev().map(|node| (None, node)).collect();
    while let Some((parent, node)) = stack.pop() {
        let place = nodes.len();
        nodes.push((parent, node));
        let children = tree.children(TreeParentId::Node(node)).unwrap_or_default();
        stack.extend(children.into_iter().rev().map(|child| (Some(place), child)));
    }

    let metas: Vec<Option<LoroMap>> =
        nodes.iter().map(|(_, node)| tree.get_meta(*node).ok()).collect();
    let mut read: Vec<Option<(Fields, LoroText)>> = Vec::new();
    read.resize_with(nodes.len(), || None);
    for place in in_store_order(&metas, |meta| meta.id().to_bytes()) {
        let Some(meta) = &metas[place] else { continue };
        read[place] = fields(meta).zip(child_text(meta));
    }
    let mut texts: Vec<Option<InlineText>> = Vec::new();
    texts.resize_with(nodes.len(), || None);
    for place in in_store_order(&read, |(_, text)| text.id().to_bytes()) {
        texts[place] = read[place].as_ref().map(|(_, text)| text_of(text));
    }

    let mut doc = Document::with_limits(registry.clone(), limits.clone());
    let mut handles = HashMap::with_capacity(nodes.len());
    // The block each place became, for its children to hang off; a
    // block that was refused takes everything under it with it.
    let mut placed: Vec<Option<BlockId>> = vec![None; nodes.len()];
    for (place, (parent, node)) in nodes.iter().enumerate() {
        let parent = match parent {
            Some(parent) => match placed[*parent] {
                Some(id) => Some(id),
                None => continue,
            },
            None => None,
        };
        let (Some(((id, kind, props, tunes), text)), Some(inline), Some(meta)) =
            (read[place].take(), texts[place].take(), metas[place].clone())
        else {
            continue;
        };
        let kind = kind.unwrap_or_else(|| registry.default_kind().to_string());
        let draft = BlockDraft { id, kind: BlockKind::new(&kind), props, tunes, text: inline };
        let index = doc.children(parent).len();
        if doc.apply(&Op::InsertBlock { parent, index, block: draft }).is_err() {
            continue;
        }
        placed[place] = Some(id);
        handles.insert(id, Handles { node: *node, meta, text });
    }
    (doc, handles)
}

/// The places of the items that are there, in the order the library
/// stores what `key` names.
fn in_store_order<T>(items: &[Option<T>], key: impl Fn(&T) -> Vec<u8>) -> Vec<usize> {
    let mut order: Vec<(Vec<u8>, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(place, item)| item.as_ref().map(|item| (key(item), place)))
        .collect();
    order.sort_unstable();
    order.into_iter().map(|(_, place)| place).collect()
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
