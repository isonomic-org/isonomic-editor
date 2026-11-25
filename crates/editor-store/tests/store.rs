use std::sync::Arc;

use editor_model::*;
use editor_store::Store;

fn registry() -> Arc<Registry> {
    let mut registry = Registry::new();
    registry.register(BlockSchema::new(kinds::PARAGRAPH, ContentKind::Inline, ChildPolicy::Any));
    registry.register(BlockSchema::new(kinds::CODE, ContentKind::Plain, ChildPolicy::None));
    registry.register(BlockSchema::new(kinds::DIVIDER, ContentKind::None, ChildPolicy::None));
    Arc::new(registry)
}

fn para(id: BlockId, text: &str) -> BlockDraft {
    BlockDraft::new(id, kinds::PARAGRAPH).text(InlineText::plain(text))
}

fn tx(ops: Vec<Op>) -> Transaction {
    Transaction { ops, selection: None, intent: Intent::Command }
}

fn add(store: &mut Store, parent: Option<BlockId>, index: usize, text: &str) -> BlockId {
    let id = store.ids().next_id();
    store.apply(&tx(vec![Op::InsertBlock { parent, index, block: para(id, text) }]), None).unwrap();
    id
}

fn replace(id: BlockId, range: std::ops::Range<usize>, text: &str) -> Op {
    Op::ReplaceText { id, range, with: InlineText::plain(text) }
}

/// The snapshot must equal what a fresh replica reads from the bytes.
fn assert_matches_bytes(store: &Store) {
    let reopened = Store::open(registry(), 99, &store.export_snapshot().unwrap()).unwrap();
    let (a, b) = (store.snapshot(), reopened.snapshot());
    assert_eq!(a.flat(), b.flat());
    for id in a.flat() {
        assert_eq!(a.get(id), b.get(id), "block {id}");
        assert_eq!(a.parent(id), b.parent(id));
    }
}

fn sync(a: &mut Store, b: &mut Store) {
    let (to_b, to_a) =
        (a.export_since(&b.version()).unwrap(), b.export_since(&a.version()).unwrap());
    b.import(&to_b).unwrap();
    a.import(&to_a).unwrap();
}

fn text(store: &Store, id: BlockId) -> String {
    store.snapshot().get(id).unwrap().content.text().to_owned()
}

#[test]
fn every_primitive_matches_the_replicated_bytes() {
    let mut store = Store::new(registry(), 1);
    let a = add(&mut store, None, 0, "alpha");
    let b = add(&mut store, None, 1, "beta");
    let c = add(&mut store, Some(a), 0, "child");
    assert_matches_bytes(&store);

    let link = Mark::Link("https://example.org".into());
    let atom = Atom { kind: "mention".into(), attrs: Props::from([("user".into(), "u1".into())]) };
    store
        .apply(
            &tx(vec![
                Op::AddMark { id: a, range: 0..3, mark: Mark::Bold },
                Op::AddMark { id: a, range: 2..5, mark: link },
                Op::ReplaceText {
                    id: b,
                    range: 4..4,
                    with: InlineText::atom(atom, Marks::default()),
                },
                Op::SetProp { id: b, key: "level".into(), value: Some(PropValue::Int(2)) },
                Op::SetTune { id: b, key: "align".into(), value: Some("center".into()) },
                Op::MoveBlock { id: c, parent: None, index: 0 },
            ]),
            None,
        )
        .unwrap();
    assert_matches_bytes(&store);
    assert_eq!(store.snapshot().root(), [c, a, b]);

    store
        .apply(
            &tx(vec![
                Op::RemoveMark { id: a, range: 0..5, kind: MarkKind::Bold },
                Op::SetProp { id: b, key: "level".into(), value: None },
                Op::SetKind { id: a, kind: BlockKind::new(kinds::CODE) },
                Op::MoveBlock { id: b, parent: Some(c), index: 0 },
                Op::RemoveBlock { id: c },
            ]),
            None,
        )
        .unwrap();
    assert_matches_bytes(&store);
    let doc = store.snapshot();
    assert_eq!(doc.len(), 1);
    assert_eq!(doc.get(a).unwrap().content, Content::Plain("alpha".into()));
}

#[test]
fn inserted_text_takes_exactly_the_marks_it_was_given() {
    let mut store = Store::new(registry(), 1);
    let a = add(&mut store, None, 0, "ab");
    store.apply(&tx(vec![Op::AddMark { id: a, range: 0..2, mark: Mark::Bold }]), None).unwrap();
    // Typed at the bold run's trailing edge, but asked for as plain.
    store.apply(&tx(vec![replace(a, 2..2, "c")]), None).unwrap();
    let doc = store.snapshot();
    let Content::Inline(inline) = &doc.get(a).unwrap().content else { panic!() };
    assert!(inline.range_has(0..2, MarkKind::Bold));
    assert!(!inline.range_has(2..3, MarkKind::Bold));
    assert_matches_bytes(&store);
}

#[test]
fn a_failed_transaction_changes_nothing() {
    let mut store = Store::new(registry(), 1);
    let a = add(&mut store, None, 0, "alpha");
    let bad = tx(vec![replace(a, 5..5, "!"), Op::RemoveBlock { id: BlockId(42) }]);
    assert!(store.apply(&bad, None).is_err());
    assert_eq!(text(&store, a), "alpha");
    assert_matches_bytes(&store);
    assert!(store.undo().is_some());
    assert!(store.snapshot().is_empty());
}

#[test]
fn typing_coalesces_into_one_undo_step() {
    let mut store = Store::new(registry(), 1);
    let a = add(&mut store, None, 0, "");
    let mut caret = Selection::caret(a, 0);
    for (index, ch) in ["h", "e", "y"].into_iter().enumerate() {
        let after = Selection::caret(a, index + 1);
        let step = Transaction {
            ops: vec![replace(a, index..index, ch)],
            selection: Some(after.clone()),
            intent: Intent::Typing,
        };
        store.apply(&step, Some(&caret)).unwrap();
        caret = after;
    }
    assert_eq!(text(&store, a), "hey");
    let undone = store.undo().unwrap();
    assert_eq!(text(&store, a), "");
    assert_eq!(undone.selection, Some(Selection::caret(a, 0)));
    let redone = store.redo().unwrap();
    assert_eq!(text(&store, a), "hey");
    assert_eq!(redone.selection, Some(Selection::caret(a, 3)));
    assert!(store.undo().is_some() && store.undo().is_some());
    assert!(store.snapshot().is_empty() && !store.can_undo());
}

#[test]
fn undo_is_local_only() {
    let mut one = Store::new(registry(), 1);
    let a = add(&mut one, None, 0, "shared");
    let mut two = Store::open(registry(), 2, &one.export_snapshot().unwrap()).unwrap();
    one.apply(&tx(vec![replace(a, 6..6, " one")]), None).unwrap();
    two.apply(&tx(vec![replace(a, 0..0, "two ")]), None).unwrap();
    sync(&mut one, &mut two);
    assert_eq!(text(&one, a), "two shared one");
    one.undo().unwrap();
    assert_eq!(text(&one, a), "two shared");
    sync(&mut one, &mut two);
    assert_eq!(text(&two, a), "two shared");
}

#[test]
fn concurrent_edits_converge() {
    let mut one = Store::new(registry(), 1);
    let a = add(&mut one, None, 0, "a");
    let b = add(&mut one, None, 1, "b");
    let mut two = Store::open(registry(), 2, &one.export_snapshot().unwrap()).unwrap();
    let c = add(&mut one, Some(a), 0, "from one");
    one.apply(&tx(vec![Op::MoveBlock { id: b, parent: Some(a), index: 0 }]), None).unwrap();
    two.apply(&tx(vec![Op::MoveBlock { id: a, parent: Some(b), index: 0 }]), None).unwrap();
    two.apply(&tx(vec![Op::AddMark { id: a, range: 0..1, mark: Mark::Italic }]), None).unwrap();
    let changes = two.import(&one.export_since(&two.version()).unwrap()).unwrap();
    assert!(changes.added.contains(&c));
    sync(&mut one, &mut two);
    let (x, y) = (one.snapshot(), two.snapshot());
    assert_eq!(x.flat(), y.flat());
    assert_eq!(x.len(), 3);
    for id in x.flat() {
        assert_eq!(x.get(id), y.get(id));
    }
    assert_matches_bytes(&one);
}

#[test]
fn anchors_follow_edits() {
    let mut one = Store::new(registry(), 1);
    let a = add(&mut one, None, 0, "héllo world");
    let anchor = one.anchor(Position::new(a, 7)).unwrap();
    let mut two = Store::open(registry(), 2, &one.export_snapshot().unwrap()).unwrap();
    two.apply(&tx(vec![replace(a, 0..0, "ünï ")]), None).unwrap();
    one.apply(&tx(vec![replace(a, 12..12, "!")]), None).unwrap();
    sync(&mut one, &mut two);
    assert_eq!(text(&one, a), "ünï héllo world!");
    assert_eq!(one.resolve(&anchor).unwrap().offset, 7 + "ünï ".len());
    one.apply(&tx(vec![Op::RemoveBlock { id: a }]), None).unwrap();
    assert!(one.resolve(&anchor).is_none());
}

#[test]
fn the_normalizer_runs_inside_the_transaction() {
    let mut store = Store::new(registry(), 1);
    store.set_normalizer(Box::new(|doc, _, ids| {
        if doc.is_empty() {
            vec![Op::InsertBlock { parent: None, index: 0, block: para(ids.next_id(), "") }]
        } else {
            Vec::new()
        }
    }));
    let a = add(&mut store, None, 0, "only");
    store.apply(&tx(vec![Op::RemoveBlock { id: a }]), None).unwrap();
    assert_eq!(store.snapshot().len(), 1);
    store.undo().unwrap();
    assert_eq!(store.snapshot().root(), [a]);
}

#[test]
fn limits_degrade_on_load() {
    let mut big = Store::new(registry(), 1);
    for index in 0..5 {
        add(&mut big, None, index, "x");
    }
    let limits = Limits { blocks: 3, ..Limits::default() };
    let mut small = Store::with_limits(registry(), 2, limits);
    small.import(&big.export_snapshot().unwrap()).unwrap();
    assert_eq!(small.snapshot().len(), 3);
    assert!(Store::open(registry(), 3, b"not a document").is_err());
}
