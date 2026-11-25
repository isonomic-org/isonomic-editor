// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

mod common;

use common::Session;
use editor_commands as cmd;
use editor_model::*;

#[test]
fn a_document_can_be_written_with_the_keyboard_alone() {
    let mut s = Session::new(1);
    s.type_text("# Plan");
    s.enter();
    s.type_text("Ship the editor.");
    s.enter();
    s.type_text("- one");
    s.enter();
    s.type_text("two");
    assert!(s.tab());
    s.enter();
    s.type_text("[] check it");
    s.enter();
    assert!(s.enter(), "an empty nested item lifts out");
    assert!(s.enter(), "an empty top-level item becomes a paragraph");
    s.type_text("> quoted");
    s.enter();
    s.type_text("---");
    s.type_text("```");
    s.type_text("let x = 1;");
    s.enter();
    s.type_text("x");
    let expected = "# Plan\n\nShip the editor.\n\n- one\n  - two\n  - [ ] check it\n\n> quoted\n\n---\n\n```\nlet x = 1;\nx\n```\n";
    assert_eq!(s.markdown(), expected);
    let doc = s.doc();
    let last = doc.get(*doc.root().last().unwrap()).unwrap();
    assert!(last.kind.is(kinds::CODE), "the document already ends in text");
}

#[test]
fn split_then_backspace_restores_the_block() {
    let mut s = Session::new(1);
    s.type_text("hello world");
    let id = s.block(0);
    s.selection = Selection::range(id, 0, 5);
    let doc = s.doc();
    assert!(s.run(cmd::toggle_mark(&doc, &s.selection, MarkKind::Bold, None)));
    let before = s.doc().get(id).unwrap().clone();
    s.selection = Selection::caret(id, 5);
    assert!(s.enter());
    assert_eq!((s.text(0), s.text(1)), ("hello".into(), " world".into()));
    assert!(s.backspace());
    let after = s.doc().get(id).unwrap().clone();
    assert_eq!(before.content, after.content);
    assert_eq!(s.selection, Selection::caret(id, 5));
    assert_eq!(s.markdown(), "**hello** world\n");
}

#[test]
fn backspace_at_the_start_unwraps_before_it_joins() {
    let mut s = Session::new(1);
    s.type_text("first");
    s.enter();
    s.type_text("- item");
    let item = s.block(1);
    s.selection = Selection::caret(item, 0);
    assert!(s.backspace());
    assert!(s.doc().get(item).unwrap().kind.is(kinds::PARAGRAPH));
    assert!(s.backspace());
    assert_eq!(s.markdown(), "firstitem\n");
    assert!(!s.backspace() || s.text(0) == "firsitem");
}

#[test]
fn deleting_across_blocks_keeps_what_lies_past_the_selection() {
    let mut s = Session::new(1);
    s.type_text("alpha");
    s.enter();
    s.type_text("- parent");
    s.enter();
    s.type_text("child one");
    s.tab();
    s.enter();
    s.type_text("child two");
    s.enter();
    s.type_text("grandchild");
    s.tab();
    // From inside "alpha" to inside "child one".
    let (first, last) = (s.block(0), s.block(2));
    s.selection = Selection::Text { anchor: Position::new(last, 6), head: Position::new(first, 2) };
    let doc = s.doc();
    assert!(s.run(cmd::delete_selection(&doc, &s.selection)));
    assert_eq!(s.markdown(), "alone\n\n- child two\n  - grandchild\n");
    assert_eq!(s.selection, Selection::caret(first, 2));
    assert!(s.undo());
    assert_eq!(s.doc().len(), 5);
}

#[test]
fn typing_over_a_selection_replaces_it() {
    let mut s = Session::new(1);
    s.type_text("one");
    s.enter();
    s.type_text("two");
    s.enter();
    s.type_text("three");
    s.selection = Selection::Text {
        anchor: Position::new(s.block(0), 1),
        head: Position::new(s.block(2), 4),
    };
    s.type_text("X");
    assert_eq!(s.markdown(), "oXe\n");
}

#[test]
fn marks_toggle_across_blocks() {
    let mut s = Session::new(1);
    s.type_text("one");
    s.enter();
    s.type_text("two");
    let all = Selection::Text {
        anchor: Position::new(s.block(0), 1),
        head: Position::new(s.block(1), 2),
    };
    s.selection = all.clone();
    let doc = s.doc();
    assert!(s.run(cmd::toggle_mark(&doc, &all, MarkKind::Italic, None)));
    assert_eq!(s.markdown(), "o*ne*\n\n*tw*o\n");
    let doc = s.doc();
    assert!(s.run(cmd::set_link(&doc, &all, Some("https://example.org"))));
    let doc = s.doc();
    assert!(s.run(cmd::toggle_mark(&doc, &all, MarkKind::Italic, None)));
    assert_eq!(s.markdown(), "o[ne](https://example.org)\n\n[tw](https://example.org)o\n");
    let doc = s.doc();
    assert!(s.run(cmd::set_link(&doc, &all, None)));
    assert_eq!(s.markdown(), "one\n\ntwo\n");
    assert!(
        cmd::toggle_mark(&s.doc(), &Selection::caret(s.block(0), 1), MarkKind::Bold, None)
            .is_none()
    );
}

#[test]
fn undo_walks_back_step_by_step_and_redo_returns() {
    let mut s = Session::new(1);
    let mut states = vec![s.markdown()];
    s.type_text("hello");
    states.push(s.markdown());
    s.enter();
    states.push(s.markdown());
    s.type_text("# ");
    states.push(s.markdown());
    s.type_text("Title");
    states.push(s.markdown());
    for expected in states.iter().rev().skip(1) {
        // The input rule is its own step: undo once more to reach the
        // typed prefix, which is not a recorded state here.
        assert!(s.undo());
        if s.markdown() != *expected {
            assert!(s.undo());
        }
        assert_eq!(s.markdown(), *expected);
    }
    while s.redo() {}
    assert_eq!(s.markdown(), *states.last().unwrap());
}

#[test]
fn structure_commands_respect_the_schema() {
    let mut s = Session::new(1);
    s.type_text("# Heading");
    s.enter();
    s.type_text("body");
    assert!(!s.tab(), "a heading holds no children");
    s.enter();
    s.type_text("second");
    assert!(s.tab());
    let (heading, body, second) = (s.block(0), s.block(1), s.block(2));
    assert_eq!(s.doc().parent(second), Some(body));
    // Turning the parent into a heading lifts its child out.
    s.selection = Selection::caret(body, 0);
    let doc = s.doc();
    let props = Props::from([("level".to_owned(), PropValue::Int(2))]);
    assert!(s.run(cmd::convert_block(&doc, &s.selection, kinds::HEADING, &props)));
    assert_eq!(s.doc().root(), [heading, body, second]);
    let doc = s.doc();
    assert!(s.run(cmd::move_up(&doc, &Selection::caret(second, 0))));
    assert_eq!(s.doc().root(), [heading, second, body]);
    let doc = s.doc();
    assert!(cmd::move_up(&doc, &Selection::caret(heading, 0)).is_none());
    // A document never ends in a block without text.
    let doc = s.doc();
    assert!(s.run(cmd::convert_block(
        &doc,
        &Selection::caret(body, 0),
        kinds::DIVIDER,
        &Props::new()
    )));
    let doc = s.doc();
    assert_eq!(doc.len(), 4);
    assert!(doc.get(*doc.root().last().unwrap()).unwrap().kind.is(kinds::PARAGRAPH));
}

#[test]
fn select_all_grows_and_blocks_delete() {
    let mut s = Session::new(1);
    s.type_text("one");
    s.enter();
    s.type_text("two");
    let (a, b) = (s.block(0), s.block(1));
    let step1 = cmd::select_all(&s.doc(), &Selection::caret(b, 1)).unwrap();
    assert_eq!(step1, Selection::range(b, 0, 3));
    let step2 = cmd::select_all(&s.doc(), &step1).unwrap();
    assert_eq!(step2, Selection::Blocks(vec![b]));
    let step3 = cmd::select_all(&s.doc(), &step2).unwrap();
    assert_eq!(step3, Selection::Blocks(vec![a, b]));
    let doc = s.doc();
    assert!(s.run(cmd::delete_selection(&doc, &step3)));
    assert_eq!(s.doc().len(), 1, "the normalizer leaves one empty paragraph");
}

#[test]
fn the_insert_menu_finds_and_inserts_blocks() {
    let specs = editor_blocks::standard();
    let titles = |query: &str| -> Vec<&str> {
        editor_blocks::search_menu(&specs, query).iter().map(|entry| entry.title).collect()
    };
    assert_eq!(titles("head"), ["Heading 1", "Heading 2", "Heading 3"]);
    assert_eq!(titles("task"), ["To-do"]);
    assert_eq!(titles("").len(), 10);
    let mut s = Session::new(1);
    s.type_text("text");
    let entry = &editor_blocks::search_menu(&specs, "divider")[0];
    let doc = s.doc();
    let tx = cmd::insert_block(&doc, &s.selection, entry.kind, &entry.props, s.store.ids());
    assert!(s.run(tx));
    s.type_text("after");
    assert_eq!(s.markdown(), "text\n\n---\n\nafter\n");
}

#[test]
fn the_insert_menu_follows_what_is_typed_and_applies_a_choice() {
    use editor_blocks::{search_menu, standard};
    let specs = standard();
    let mut s = Session::new(1);
    s.type_text("Notes /");
    let doc = s.doc();
    let from = cmd::menu_trigger(&doc, &s.selection).expect("a trigger after a space opens it");
    assert_eq!(from.offset, 6);

    s.type_text("hea");
    let doc = s.doc();
    let query = cmd::menu_query(&doc, &s.selection, from).unwrap();
    assert_eq!(query, "hea");
    let entries = search_menu(&specs, &query);
    let titles: Vec<_> = entries.iter().map(|entry| entry.title).collect();
    assert_eq!(titles, ["Heading 1", "Heading 2", "Heading 3"]);

    // Choosing removes the trigger and query and converts the block,
    // in one step that one undo takes back.
    let second = &entries[1];
    let tx =
        cmd::choose_menu_entry(&doc, &s.selection, from, second.kind, &second.props, s.store.ids());
    assert!(s.run(tx));
    assert_eq!(s.markdown(), "## Notes\n");
    assert_eq!(s.doc().get(from.block).unwrap().content.text(), "Notes ");
    assert_eq!(s.selection, Selection::caret(from.block, 6));
    assert!(s.undo());
    assert_eq!(s.markdown(), "Notes /hea\n");
}

#[test]
fn the_insert_menu_opens_only_where_it_should() {
    let mut s = Session::new(1);
    s.type_text("a/");
    assert_eq!(cmd::menu_trigger(&s.doc(), &s.selection), None, "not in the middle of a word");
    s.type_text(" /");
    let from = cmd::menu_trigger(&s.doc(), &s.selection).unwrap();

    // It closes when the caret leaves the query or the trigger goes.
    let before = Selection::caret(from.block, from.offset);
    assert_eq!(cmd::menu_query(&s.doc(), &before, from), None);
    let range = Selection::range(from.block, 0, 2);
    assert_eq!(cmd::menu_query(&s.doc(), &range, from), None);
    assert_eq!(cmd::menu_query(&s.doc(), &s.selection, from), Some(String::new()));
    s.backspace();
    assert_eq!(cmd::menu_query(&s.doc(), &s.selection, from), None);

    // Positions that do not exist are answered, not trusted.
    let nowhere = Position::new(from.block, 999);
    assert_eq!(cmd::menu_query(&s.doc(), &s.selection, nowhere), None);
    let inside = {
        s.type_text("é/x");
        // Offset 4 is inside the two-byte character.
        Position::new(from.block, 4)
    };
    assert_eq!(cmd::menu_query(&s.doc(), &s.selection, inside), None);

    // Code holds plain text: no menu there.
    let mut code = Session::new(2);
    code.type_text("```");
    code.type_text("/");
    assert_eq!(cmd::menu_trigger(&code.doc(), &code.selection), None);
}

#[test]
fn a_divider_from_the_menu_replaces_an_empty_block_or_follows_a_full_one() {
    let mut s = Session::new(1);
    s.type_text("/div");
    let from = Position::new(s.selection.head().unwrap().block, 0);
    let tx = cmd::choose_menu_entry(
        &s.doc(),
        &s.selection,
        from,
        kinds::DIVIDER,
        &Props::new(),
        s.store.ids(),
    );
    assert!(s.run(tx));
    assert_eq!(s.markdown(), "---\n");
    s.type_text("after /");
    let block = s.selection.head().unwrap().block;
    let from = Position::new(block, 6);
    let tx = cmd::choose_menu_entry(
        &s.doc(),
        &s.selection,
        from,
        kinds::DIVIDER,
        &Props::new(),
        s.store.ids(),
    );
    assert!(s.run(tx));
    assert_eq!(s.markdown(), "---\n\nafter \n\n---\n");
    assert_eq!(s.selection, Selection::caret(block, 6), "the caret stays in the text");
}
