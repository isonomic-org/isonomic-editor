use editor_model::{
    Document, IdSource, InlineText, Intent, Op, PropValue, Selection, Transaction, kinds,
};

use crate::build::Builder;

/// A typed prefix, the block it turns a paragraph into, and a property
/// to set on it.
type Rule = (&'static str, &'static str, Option<(&'static str, PropValue)>);

const RULES: &[Rule] = &[
    ("# ", kinds::HEADING, Some(("level", PropValue::Int(1)))),
    ("## ", kinds::HEADING, Some(("level", PropValue::Int(2)))),
    ("### ", kinds::HEADING, Some(("level", PropValue::Int(3)))),
    ("- ", kinds::BULLETED, None),
    ("* ", kinds::BULLETED, None),
    ("1. ", kinds::NUMBERED, None),
    ("[] ", kinds::TODO, Some(("checked", PropValue::Bool(false)))),
    ("[ ] ", kinds::TODO, Some(("checked", PropValue::Bool(false)))),
    ("[x] ", kinds::TODO, Some(("checked", PropValue::Bool(true)))),
    ("> ", kinds::QUOTE, None),
    ("```", kinds::CODE, None),
    ("---", kinds::DIVIDER, None),
];

/// Call after text is typed. If the text before the caret is a known
/// prefix, returns the transaction that converts
/// the block. Each firing is its own undo step.
pub fn input_rule(
    doc: &Document,
    selection: &Selection,
    ids: &mut dyn IdSource,
) -> Option<Transaction> {
    if !selection.is_caret() {
        return None;
    }
    let at = selection.head()?;
    let block = doc.get(at.block)?;
    let text = block.content.text();
    let typed = text.get(..at.offset)?;
    let (prefix, kind, prop) = RULES.iter().find(|(prefix, ..)| *prefix == typed)?;
    // Rules fire in a paragraph; a list item may also turn into
    // another kind of list item.
    let lists = [kinds::BULLETED, kinds::NUMBERED, kinds::TODO];
    let relist = lists.iter().any(|list| block.kind.is(list)) && lists.contains(kind);
    if !(block.kind.is(kinds::PARAGRAPH) || relist) || block.kind.is(kind) {
        return None;
    }
    let id = at.block;
    let mut build = Builder::new(doc);
    let mut caret = Selection::caret(id, 0);
    if *kind == kinds::DIVIDER {
        if text.len() != prefix.len() {
            return None;
        }
        build.push(Op::SetKind { id, kind: editor_model::BlockKind::new(kind) })?;
        caret = Selection::caret(build.paragraph_after(id, ids)?, 0);
    } else {
        build.push(Op::ReplaceText { id, range: 0..prefix.len(), with: InlineText::new() })?;
        build.push(Op::SetKind { id, kind: editor_model::BlockKind::new(kind) })?;
        if let Some((key, value)) = prop {
            build.push(Op::SetProp { id, key: (*key).into(), value: Some(value.clone()) })?;
        }
    }
    Some(Transaction { ops: build.ops, selection: Some(caret), intent: Intent::InputRule })
}
