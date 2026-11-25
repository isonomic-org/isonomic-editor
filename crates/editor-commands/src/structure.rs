use editor_model::{
    BlockDraft, BlockId, BlockKind, Content, ContentKind, Document, IdSource, InlineText, Intent,
    Op, PropValue, Props, Selection, Transaction, kinds,
};

use crate::build::{Builder, content, delete_range, ordered, targets};

fn done(build: Builder, selection: Selection, intent: Intent) -> Option<Transaction> {
    (!build.ops.is_empty()).then_some(Transaction {
        ops: build.ops,
        selection: Some(selection),
        intent,
    })
}

/// Turns a block into a plain paragraph, dropping its properties.
fn to_paragraph(build: &mut Builder, id: BlockId) -> Option<()> {
    let keys: Vec<String> = build.doc.get(id)?.props.keys().cloned().collect();
    for key in keys {
        build.push(Op::SetProp { id, key, value: None })?;
    }
    build.push(Op::SetKind { id, kind: BlockKind::new(kinds::PARAGRAPH) })
}

fn lift(build: &mut Builder, id: BlockId) -> Option<()> {
    let parent = build.doc.parent(id)?;
    build.move_after(id, parent)
}

/// Backspace at the start of a block. In order: a styled block
/// becomes a paragraph, a nested block lifts one level, and a
/// top-level paragraph joins the block before it.
pub(crate) fn join_backward(doc: &Document, id: BlockId) -> Option<Transaction> {
    let block = doc.get(id)?;
    let mut build = Builder::new(doc);
    let here = Selection::caret(id, 0);
    if !block.kind.is(kinds::PARAGRAPH) {
        let inline = doc.registry().content(&block.kind) == ContentKind::Inline;
        if !inline && !block.content.is_empty() {
            return None;
        }
        to_paragraph(&mut build, id)?;
        return done(build, here, Intent::Delete);
    }
    if doc.parent(id).is_some() {
        lift(&mut build, id)?;
        return done(build, here, Intent::Delete);
    }
    let prev = doc.prev_in_order(id)?;
    let before = content(doc, prev)?;
    if !before.has_text() {
        build.push(Op::RemoveBlock { id: prev })?;
        return done(build, here, Intent::Delete);
    }
    let at = before.len();
    build.push(Op::ReplaceText { id: prev, range: at..at, with: block.content.to_inline() })?;
    build.lift_children(id)?;
    build.push(Op::RemoveBlock { id })?;
    done(build, Selection::caret(prev, at), Intent::Delete)
}

/// Delete at the end of a block: the next block joins onto it.
pub(crate) fn join_forward(doc: &Document, id: BlockId) -> Option<Transaction> {
    let next = doc.next_in_order(id)?;
    let at = content(doc, id)?.len();
    let after = content(doc, next)?;
    let mut build = Builder::new(doc);
    if after.has_text() {
        build.push(Op::ReplaceText { id, range: at..at, with: after.to_inline() })?;
        build.lift_children(next)?;
    }
    build.push(Op::RemoveBlock { id: next })?;
    done(build, Selection::caret(id, at), Intent::Delete)
}

/// Enter. Splits the block at the caret; in an empty styled or nested
/// block it leaves the container instead.
pub fn split_block(
    doc: &Document,
    selection: &Selection,
    ids: &mut dyn IdSource,
) -> Option<Transaction> {
    let (start, end) = ordered(doc, selection)?;
    let mut build = Builder::new(doc);
    delete_range(&mut build, start, end)?;
    let id = start.block;
    let block = build.doc.get(id)?.clone();
    let text = block.content.to_inline();
    let here = Selection::caret(id, 0);
    if let Content::Plain(_) = block.content {
        let with = InlineText::plain("\n");
        build.push(Op::ReplaceText { id, range: start.offset..start.offset, with })?;
        return done(build, Selection::caret(id, start.offset + 1), Intent::Typing);
    }
    let paragraph = block.kind.is(kinds::PARAGRAPH);
    if text.is_empty() && !paragraph {
        if build.doc.parent(id).is_some() {
            lift(&mut build, id)?;
        } else {
            to_paragraph(&mut build, id)?;
        }
        return done(build, here, Intent::Command);
    }
    if text.is_empty() && build.doc.parent(id).is_some() {
        lift(&mut build, id)?;
        return done(build, here, Intent::Command);
    }
    let parent = build.doc.parent(id);
    let index = build.doc.index_in_parent(id)?;
    let new = ids.next_id();
    let continues =
        [kinds::BULLETED, kinds::NUMBERED, kinds::TODO].iter().any(|k| block.kind.is(k));
    if start.offset == 0 && !text.is_empty() {
        // An empty block appears above; the text keeps its block.
        let kind = if continues { block.kind.as_str() } else { kinds::PARAGRAPH };
        build.push(Op::InsertBlock { parent, index, block: BlockDraft::new(new, kind) })?;
        return done(build, here, Intent::Command);
    }
    let tail = text.slice(start.offset..text.len());
    build.push(Op::ReplaceText { id, range: start.offset..text.len(), with: InlineText::new() })?;
    let kind = if continues { block.kind.as_str() } else { kinds::PARAGRAPH };
    let draft = BlockDraft::new(new, kind).text(tail);
    let has_children = !block.children.is_empty();
    let op = if has_children && build.doc.registry().allows_child(&block.kind, &draft.kind) {
        Op::InsertBlock { parent: Some(id), index: 0, block: draft }
    } else {
        Op::InsertBlock { parent, index: index + 1, block: draft }
    };
    build.push(op)?;
    done(build, Selection::caret(new, 0), Intent::Command)
}

/// Tab: each selected block becomes the last child of the sibling
/// before it, where that sibling accepts it.
pub fn indent(doc: &Document, selection: &Selection) -> Option<Transaction> {
    let mut build = Builder::new(doc);
    for id in targets(doc, selection) {
        let Some(index) = build.doc.index_in_parent(id).filter(|index| *index > 0) else {
            continue;
        };
        let prev = build.doc.children(build.doc.parent(id))[index - 1];
        let (Some(host), Some(block)) = (build.doc.get(prev), build.doc.get(id)) else {
            continue;
        };
        if !build.doc.registry().allows_child(&host.kind, &block.kind) {
            continue;
        }
        let at = host.children.len();
        build.push(Op::MoveBlock { id, parent: Some(prev), index: at })?;
    }
    done(build, selection.clone(), Intent::Command)
}

/// Shift-Tab: each selected nested block moves out to follow its
/// parent.
pub fn outdent(doc: &Document, selection: &Selection) -> Option<Transaction> {
    let mut build = Builder::new(doc);
    for id in targets(doc, selection).into_iter().rev() {
        if build.doc.parent(id).is_some() {
            lift(&mut build, id)?;
        }
    }
    done(build, selection.clone(), Intent::Command)
}

fn shift(doc: &Document, selection: &Selection, down: bool) -> Option<Transaction> {
    let id = *targets(doc, selection).first()?;
    let index = doc.index_in_parent(id)?;
    let parent = doc.parent(id);
    let to = if down { index + 1 } else { index.checked_sub(1)? };
    if to >= doc.children(parent).len() {
        return None;
    }
    let op = Op::MoveBlock { id, parent, index: to };
    Some(Transaction::new(Intent::Command).op(op).select(selection.clone()))
}

pub fn move_up(doc: &Document, selection: &Selection) -> Option<Transaction> {
    shift(doc, selection, false)
}

pub fn move_down(doc: &Document, selection: &Selection) -> Option<Transaction> {
    shift(doc, selection, true)
}

/// Changes the type of the selected blocks, replacing their
/// properties with `props`.
pub fn convert_block(
    doc: &Document,
    selection: &Selection,
    kind: &str,
    props: &Props,
) -> Option<Transaction> {
    let mut build = Builder::new(doc);
    for id in targets(doc, selection) {
        let block = doc.get(id)?;
        for key in block.props.keys().filter(|key| !props.contains_key(*key)) {
            build.push(Op::SetProp { id, key: key.clone(), value: None })?;
        }
        for (key, value) in props {
            if block.props.get(key) != Some(value) {
                build.push(Op::SetProp { id, key: key.clone(), value: Some(value.clone()) })?;
            }
        }
        if !block.kind.is(kind) {
            build.push(Op::SetKind { id, kind: BlockKind::new(kind) })?;
        }
    }
    let selection = match selection.head() {
        Some(head) if content(&build.doc, head.block).is_some_and(|c| c.len() < head.offset) => {
            Selection::caret(head.block, 0)
        }
        _ => selection.clone(),
    };
    done(build, selection, Intent::Command)
}

/// Checks or unchecks a to-do.
pub fn toggle_todo(doc: &Document, id: BlockId) -> Option<Transaction> {
    let block = doc.get(id)?;
    if !block.kind.is(kinds::TODO) {
        return None;
    }
    let checked = block.props.get("checked").and_then(PropValue::as_bool).unwrap_or(false);
    let op = Op::SetProp { id, key: "checked".into(), value: Some(PropValue::Bool(!checked)) };
    Some(Transaction::new(Intent::Command).op(op))
}

/// Select-all grows in steps: the block's text, then the block, then
/// every top-level block.
pub fn select_all(doc: &Document, selection: &Selection) -> Option<Selection> {
    match selection {
        Selection::Text { .. } => {
            let (start, end) = ordered(doc, selection)?;
            let len = content(doc, end.block)?.len();
            if start.block == end.block && (start.offset > 0 || end.offset < len) {
                Some(Selection::range(start.block, 0, len))
            } else {
                Some(Selection::Blocks(targets(doc, selection)))
            }
        }
        _ => Some(Selection::Blocks(doc.root().to_vec())),
    }
}
