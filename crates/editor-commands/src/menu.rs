// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The insert menu's rules: when typing the trigger opens it, what has
//! been typed after the trigger, and what choosing an entry does.

use editor_model::{
    BlockDraft, BlockId, BlockKind, ContentKind, Document, IdSource, InlineText, Intent, Op,
    Position, Props, Selection, Transaction,
};

use crate::build::Builder;

/// The character that opens the insert menu.
pub const MENU_TRIGGER: char = '/';

/// The longest query the menu follows before it gives up.
const LONGEST_QUERY: usize = 40;

fn caret(selection: &Selection) -> Option<Position> {
    if selection.is_caret() { selection.head() } else { None }
}

/// Call after a trigger character is typed. Returns where the trigger
/// sits if it opens the menu: in a block of rich text, at its start or
/// after white space.
pub fn menu_trigger(doc: &Document, selection: &Selection) -> Option<Position> {
    let at = caret(selection)?;
    let block = doc.get(at.block)?;
    if doc.registry().content(&block.kind) != ContentKind::Inline {
        return None;
    }
    let before = block.content.text().get(..at.offset)?;
    let mut typed = before.chars().rev();
    if typed.next() != Some(MENU_TRIGGER) {
        return None;
    }
    if typed.next().is_some_and(|c| !c.is_whitespace()) {
        return None;
    }
    Some(Position::new(at.block, at.offset - MENU_TRIGGER.len_utf8()))
}

/// What has been typed after the trigger at `from`, or `None` when the
/// menu should close: the caret left the text after the trigger, the
/// trigger was removed, or the query ran on too long.
pub fn menu_query(doc: &Document, selection: &Selection, from: Position) -> Option<String> {
    let at = caret(selection)?;
    let text = doc.get(from.block)?.content.text();
    let start = from.offset + MENU_TRIGGER.len_utf8();
    let trigger = text.get(from.offset..)?.starts_with(MENU_TRIGGER);
    if at.block != from.block || at.offset < start || !trigger {
        return None;
    }
    let query = text.get(start..at.offset)?;
    (query.chars().count() <= LONGEST_QUERY).then(|| query.to_owned())
}

/// Applies a menu choice: removes the trigger and the query, then
/// makes the block a `kind` with `props`. A kind that holds no text
/// takes the block's place when the block is otherwise empty, with a
/// paragraph after it for the caret; when the block has other text it
/// goes in after the block instead. One undo step.
pub fn choose_menu_entry(
    doc: &Document,
    selection: &Selection,
    from: Position,
    kind: &str,
    props: &Props,
    ids: &mut dyn IdSource,
) -> Option<Transaction> {
    let at = caret(selection)?;
    menu_query(doc, selection, from)?;
    let id: BlockId = from.block;
    let to = BlockKind::new(kind);
    let mut build = Builder::new(doc);
    build.push(Op::ReplaceText { id, range: from.offset..at.offset, with: InlineText::new() })?;
    let mut caret = Selection::caret(id, from.offset);
    if doc.registry().content(&to) == ContentKind::None {
        let block = build.doc.get(id)?;
        if block.content.is_empty() && build.doc.children(Some(id)).is_empty() {
            build.push(Op::SetKind { id, kind: to })?;
            caret = Selection::caret(build.paragraph_after(id, ids)?, 0);
        } else {
            let mut draft = BlockDraft::new(ids.next_id(), kind);
            draft.props = props.clone();
            let parent = build.doc.parent(id);
            let index = build.doc.index_in_parent(id)? + 1;
            build.push(Op::InsertBlock { parent, index, block: draft })?;
        }
    } else {
        let block = build.doc.get(id)?.clone();
        for key in block.props.keys().filter(|key| !props.contains_key(*key)) {
            build.push(Op::SetProp { id, key: key.clone(), value: None })?;
        }
        if !block.kind.is(kind) {
            build.push(Op::SetKind { id, kind: to })?;
        }
        for (key, value) in props {
            if block.props.get(key) != Some(value) {
                build.push(Op::SetProp { id, key: key.clone(), value: Some(value.clone()) })?;
            }
        }
    }
    Some(Transaction { ops: build.ops, selection: Some(caret), intent: Intent::Command })
}
