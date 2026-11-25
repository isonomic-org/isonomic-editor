use editor_model::{
    Content, Document, InlineText, Intent, Marks, Op, Position, Selection, Transaction,
    next_grapheme, next_word, prev_grapheme, prev_word,
};

use crate::build::{Builder, content, delete_range, ordered, targets};
use crate::structure::{join_backward, join_forward};

/// How much a delete key removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Grapheme,
    Word,
}

/// Types `text` at the selection, replacing it if it is a range.
/// `marks` overrides the marks the text would inherit.
pub fn insert_text(
    doc: &Document,
    selection: &Selection,
    text: &str,
    marks: Option<Marks>,
) -> Option<Transaction> {
    let (start, end) = ordered(doc, selection)?;
    if text.is_empty() {
        return None;
    }
    let styled = |doc: &Document| match content(doc, start.block) {
        Some(Content::Inline(inline)) => {
            let marks = marks.clone().unwrap_or_else(|| inline.marks_at(start.offset));
            InlineText::styled(text, marks)
        }
        _ => InlineText::plain(text),
    };
    let caret = Selection::caret(start.block, start.offset + text.len());
    let mut tx = Transaction::new(Intent::Typing).select(caret);
    if start.block == end.block {
        // The hot path: one operation, no scratch document.
        let with = styled(doc);
        tx.ops.push(Op::ReplaceText { id: start.block, range: start.offset..end.offset, with });
        return Some(tx);
    }
    let mut build = Builder::new(doc);
    delete_range(&mut build, start, end)?;
    let with = styled(&build.doc);
    build.push(Op::ReplaceText { id: start.block, range: start.offset..start.offset, with })?;
    tx.ops = build.ops;
    Some(tx)
}

/// Deletes the selected text or the selected blocks.
pub fn delete_selection(doc: &Document, selection: &Selection) -> Option<Transaction> {
    if let Selection::Blocks(_) = selection {
        let blocks = targets(doc, selection);
        let first = *blocks.first()?;
        let before = doc
            .prev_in_order(first)
            .filter(|id| doc.get(*id).is_some_and(|b| b.content.has_text()));
        let mut tx = Transaction::new(Intent::Delete);
        tx.selection =
            before.map(|id| Selection::caret(id, content(doc, id).map_or(0, Content::len)));
        tx.ops = blocks.into_iter().map(|id| Op::RemoveBlock { id }).collect();
        return Some(tx);
    }
    let (start, end) = ordered(doc, selection)?;
    if start == end {
        return None;
    }
    let mut build = Builder::new(doc);
    delete_range(&mut build, start, end)?;
    let caret = Selection::caret(start.block, start.offset);
    Some(Transaction { ops: build.ops, selection: Some(caret), intent: Intent::Delete })
}

fn remove(at: Position, from: usize, to: usize) -> Option<Transaction> {
    let op = Op::ReplaceText { id: at.block, range: from..to, with: InlineText::new() };
    Some(Transaction::new(Intent::Delete).op(op).select(Selection::caret(at.block, from)))
}

/// Backspace: removes the selection, the unit before the caret, or at
/// the start of a block joins it with what comes before.
pub fn delete_backward(doc: &Document, selection: &Selection, unit: Unit) -> Option<Transaction> {
    if !selection.is_caret() {
        return delete_selection(doc, selection);
    }
    let at = selection.head()?;
    let text = content(doc, at.block)?.text();
    if at.offset == 0 || at.offset > text.len() {
        return join_backward(doc, at.block);
    }
    let from = match unit {
        Unit::Grapheme => prev_grapheme(text, at.offset),
        Unit::Word => prev_word(text, at.offset),
    };
    remove(at, from, at.offset)
}

/// Delete: removes the selection, the unit after the caret, or at the
/// end of a block joins the next block onto it.
pub fn delete_forward(doc: &Document, selection: &Selection, unit: Unit) -> Option<Transaction> {
    if !selection.is_caret() {
        return delete_selection(doc, selection);
    }
    let at = selection.head()?;
    let text = content(doc, at.block)?.text();
    if at.offset >= text.len() {
        return join_forward(doc, at.block);
    }
    let to = match unit {
        Unit::Grapheme => next_grapheme(text, at.offset),
        Unit::Word => next_word(text, at.offset),
    };
    remove(at, at.offset, to)
}
