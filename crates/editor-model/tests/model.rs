use std::sync::Arc;

use editor_model::*;

fn registry() -> Arc<Registry> {
    let mut registry = Registry::new();
    registry.register(BlockSchema::new(kinds::PARAGRAPH, ContentKind::Inline, ChildPolicy::Any));
    registry.register(BlockSchema::new(kinds::CODE, ContentKind::Plain, ChildPolicy::None));
    registry.register(BlockSchema::new(kinds::DIVIDER, ContentKind::None, ChildPolicy::None));
    Arc::new(registry)
}

fn para(id: u128, text: &str) -> BlockDraft {
    BlockDraft::new(BlockId(id), kinds::PARAGRAPH).text(InlineText::plain(text))
}

fn insert(doc: &mut Document, parent: Option<u128>, index: usize, block: BlockDraft) {
    doc.apply(&Op::InsertBlock { parent: parent.map(BlockId), index, block }).unwrap();
}

#[test]
fn inline_runs_stay_normalized() {
    let mut text = InlineText::plain("hello world");
    text.add_mark(0..5, &Mark::Bold).unwrap();
    text.add_mark(3..8, &Mark::Italic).unwrap();
    assert_eq!(text.runs().len(), 4);
    text.remove_mark(0..11, MarkKind::Italic).unwrap();
    assert_eq!(text.runs().len(), 2);
    text.remove_mark(0..11, MarkKind::Bold).unwrap();
    assert_eq!(text, InlineText::plain("hello world"));
}

#[test]
fn inline_replace_and_slice() {
    let mut text = InlineText::plain("one three");
    text.replace(4..4, &InlineText::styled("two ", Marks::default().with(Mark::Bold))).unwrap();
    assert_eq!(text.text(), "one two three");
    assert!(text.range_has(4..8, MarkKind::Bold));
    assert!(!text.range_has(0..8, MarkKind::Bold));
    assert_eq!(text.slice(4..7).text(), "two");
    assert_eq!(text.replace(1..99, &InlineText::new()), Err(ModelError::BadRange));
    assert_eq!(text.text(), "one two three");
}

#[test]
fn typing_inherits_expanding_marks_only() {
    let mut text = InlineText::plain("ab");
    text.add_mark(0..2, &Mark::Bold).unwrap();
    text.add_mark(0..2, &Mark::Link("https://example.org".into())).unwrap();
    let at_end = text.marks_at(2);
    assert!(at_end.bold && at_end.link.is_none());
    assert!(text.marks_at(1).link.is_some());
    assert!(text.marks_at(0).is_plain());
}

#[test]
fn boundaries_respect_graphemes_and_words() {
    let text = "ae\u{301} 👩‍👩‍👧 end";
    assert_eq!(prev_grapheme(text, 4), 1);
    assert_eq!(next_grapheme(text, 5), 5 + "👩‍👩‍👧".len());
    assert_eq!(prev_word("alpha beta", 10), 6);
    assert_eq!(next_word("alpha beta", 0), 5);
    assert!(!InlineText::plain(text).is_boundary(2));
}

#[test]
fn atoms_are_one_run_each() {
    let atom = Atom { kind: "mention".into(), attrs: Props::new() };
    let mut text = InlineText::plain("hi ");
    text.append(&InlineText::atom(atom.clone(), Marks::default()));
    text.append(&InlineText::atom(atom, Marks::default()));
    assert_eq!(text.runs().len(), 3);
    assert!(
        InlineText::from_runs(
            "x".into(),
            vec![Run {
                len: 1,
                marks: Marks::default(),
                atom: Some(Atom { kind: "date".into(), attrs: Props::new() }),
            }]
        )
        .is_err()
    );
}

#[test]
fn tree_operations_and_order() {
    let mut doc = Document::new(registry());
    insert(&mut doc, None, 0, para(1, "a"));
    insert(&mut doc, None, 1, para(2, "b"));
    insert(&mut doc, Some(1), 0, para(3, "a1"));
    insert(&mut doc, Some(3), 0, para(4, "a1x"));
    let ids = |list: Vec<BlockId>| list.into_iter().map(|id| id.0).collect::<Vec<_>>();
    assert_eq!(ids(doc.flat()), [1, 3, 4, 2]);
    assert_eq!(doc.prev_in_order(BlockId(2)), Some(BlockId(4)));
    assert_eq!(doc.next_in_order(BlockId(4)), Some(BlockId(2)));
    assert!(doc.path(BlockId(4)) < doc.path(BlockId(2)));
    assert_eq!(doc.depth(BlockId(4)), 3);

    let cycle = Op::MoveBlock { id: BlockId(1), parent: Some(BlockId(4)), index: 0 };
    assert_eq!(doc.apply(&cycle), Err(ModelError::Cycle));
    doc.apply(&Op::MoveBlock { id: BlockId(3), parent: None, index: 2 }).unwrap();
    assert_eq!(ids(doc.flat()), [1, 2, 3, 4]);
    doc.apply(&Op::RemoveBlock { id: BlockId(3) }).unwrap();
    assert_eq!(doc.len(), 2);
    assert!(!doc.contains(BlockId(4)));
}

#[test]
fn snapshots_are_isolated_and_revisions_follow_the_path() {
    let mut doc = Document::new(registry());
    insert(&mut doc, None, 0, para(1, "a"));
    insert(&mut doc, Some(1), 0, para(2, "b"));
    insert(&mut doc, None, 1, para(3, "c"));
    let before = doc.clone();
    let (r1, r3) = (doc.rev(BlockId(1)), doc.rev(BlockId(3)));
    let edit = Op::ReplaceText { id: BlockId(2), range: 1..1, with: InlineText::plain("!") };
    doc.apply(&edit).unwrap();
    assert_eq!(before.get(BlockId(2)).unwrap().content.text(), "b");
    assert_eq!(doc.get(BlockId(2)).unwrap().content.text(), "b!");
    assert!(doc.rev(BlockId(1)) > r1);
    assert_eq!(doc.rev(BlockId(3)), r3);
}

#[test]
fn content_follows_the_block_type() {
    let mut doc = Document::new(registry());
    let bold = InlineText::styled("code", Marks::default().with(Mark::Bold));
    insert(&mut doc, None, 0, BlockDraft::new(BlockId(1), kinds::PARAGRAPH).text(bold));
    doc.apply(&Op::SetKind { id: BlockId(1), kind: BlockKind::new(kinds::CODE) }).unwrap();
    assert_eq!(doc.get(BlockId(1)).unwrap().content, Content::Plain("code".into()));
    let mark = Op::AddMark { id: BlockId(1), range: 0..1, mark: Mark::Bold };
    assert_eq!(doc.apply(&mark), Err(ModelError::NoText));
    doc.apply(&Op::SetKind { id: BlockId(1), kind: BlockKind::new(kinds::DIVIDER) }).unwrap();
    assert_eq!(doc.get(BlockId(1)).unwrap().content, Content::None);
    doc.apply(&Op::SetKind { id: BlockId(1), kind: BlockKind::new("acme.unknown") }).unwrap();
    assert!(matches!(doc.get(BlockId(1)).unwrap().content, Content::Inline(_)));
}

#[test]
fn limits_refuse_instead_of_panicking() {
    let limits = Limits { blocks: 2, depth: 2, text_bytes: 8, ..Limits::default() };
    let mut doc = Document::with_limits(registry(), limits);
    insert(&mut doc, None, 0, para(1, "a"));
    insert(&mut doc, Some(1), 0, para(2, "b"));
    let third = Op::InsertBlock { parent: None, index: 0, block: para(3, "c") };
    assert_eq!(doc.apply(&third), Err(ModelError::Limit("blocks")));
    let long = Op::ReplaceText { id: BlockId(1), range: 0..0, with: InlineText::plain("12345678") };
    assert_eq!(doc.apply(&long), Err(ModelError::Limit("text per block")));
    assert_eq!(
        doc.apply(&Op::RemoveBlock { id: BlockId(9) }),
        Err(ModelError::UnknownBlock(BlockId(9)))
    );
}

#[test]
fn insert_ops_rebuild_the_document() {
    let mut doc = Document::new(registry());
    insert(&mut doc, None, 0, para(1, "a"));
    insert(&mut doc, Some(1), 0, para(2, "b"));
    insert(&mut doc, None, 1, para(3, "c"));
    let mut copy = Document::new(registry());
    for op in doc.insert_ops() {
        copy.apply(&op).unwrap();
    }
    assert_eq!(copy.flat(), doc.flat());
    assert_eq!(copy.get(BlockId(2)), doc.get(BlockId(2)));
}
