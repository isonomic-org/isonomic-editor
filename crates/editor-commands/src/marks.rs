use editor_model::{Content, Document, Intent, Mark, MarkKind, Op, Selection, Transaction};

use crate::build::{block_ranges, content};

/// Adds the mark to the selected text, or removes it when all of the
/// selection already has it. A caret selects nothing, so there is
/// nothing to toggle; pending marks are the view's concern.
pub fn toggle_mark(
    doc: &Document,
    selection: &Selection,
    kind: MarkKind,
    value: Option<&str>,
) -> Option<Transaction> {
    let ranges: Vec<_> = block_ranges(doc, selection)
        .into_iter()
        .filter(|(id, range)| {
            range.start < range.end && matches!(content(doc, *id), Some(Content::Inline(_)))
        })
        .collect();
    if ranges.is_empty() {
        return None;
    }
    let all = ranges.iter().all(|(id, range)| match content(doc, *id) {
        Some(Content::Inline(inline)) => inline.range_has(range.clone(), kind),
        _ => false,
    });
    let remove = all && (value.is_none() || !kind.has_value());
    let mut tx = Transaction::new(Intent::Command).select(selection.clone());
    for (id, range) in ranges {
        tx.ops.push(if remove {
            Op::RemoveMark { id, range, kind }
        } else {
            Op::AddMark { id, range, mark: Mark::new(kind, value) }
        });
    }
    Some(tx)
}

/// Links the selected text to `url`, or removes its link with `None`.
pub fn set_link(doc: &Document, selection: &Selection, url: Option<&str>) -> Option<Transaction> {
    match url {
        Some(url) => toggle_mark(doc, selection, MarkKind::Link, Some(url)),
        None => {
            let mut tx = Transaction::new(Intent::Command).select(selection.clone());
            for (id, range) in block_ranges(doc, selection) {
                if range.start < range.end && matches!(content(doc, id), Some(Content::Inline(_))) {
                    tx.ops.push(Op::RemoveMark { id, range, kind: MarkKind::Link });
                }
            }
            (!tx.ops.is_empty()).then_some(tx)
        }
    }
}
