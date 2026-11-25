// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pointer, the word and document keys and the caret's blink, in a
//! headless window.

extern crate ui as gpui;

mod common;

use common::{blocks, keys, markdown, open, selection, settle};
use editor_gpui::EditorView;
use editor_model::{Position, Selection};
use ui::{
    Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    TestAppContext, VisualTestContext, point, px,
};

fn press(cx: &mut VisualTestContext, at: Point<Pixels>, clicks: usize) {
    let button = MouseButton::Left;
    for click_count in 1..=clicks {
        cx.simulate_event(MouseDownEvent {
            position: at,
            button,
            click_count,
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent { position: at, button, click_count, ..Default::default() });
    }
    settle(cx);
}

fn caret(view: &Entity<EditorView>, cx: &mut VisualTestContext, at: Position) -> Point<Pixels> {
    view.read_with(cx, |view, _| view.caret_point(at)).unwrap()
}

#[ui::test]
fn clicks_select_words_and_blocks_and_tick_to_dos(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "alpha beta, gamma\n\n- [ ] buy milk\n");
    let ids = blocks(&view, cx);
    let in_beta = caret(&view, cx, Position::new(ids[0], 8)) + point(px(1.), px(6.));
    press(cx, in_beta, 2);
    assert_eq!(selection(&view, cx), Selection::range(ids[0], 6, 10), "a double click, a word");
    press(cx, in_beta, 3);
    assert_eq!(selection(&view, cx), Selection::range(ids[0], 0, 17), "a triple click, the block");

    // The checkbox sits left of the to-do's text; its text does not tick it.
    let text = caret(&view, cx, Position::new(ids[1], 0));
    press(cx, text + point(px(-14.), px(8.)), 1);
    assert!(markdown(&view, cx).ends_with("- [x] buy milk\n"));
    press(cx, text + point(px(-14.), px(8.)), 1);
    assert!(markdown(&view, cx).ends_with("- [ ] buy milk\n"));
    press(cx, text + point(px(10.), px(8.)), 1);
    assert!(markdown(&view, cx).ends_with("- [ ] buy milk\n"));
    assert_eq!(selection(&view, cx).head().unwrap().block, ids[1]);
}

#[ui::test]
fn keys_move_by_word_and_to_the_document_edges(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "alpha beta\n\ngamma delta\n");
    let ids = blocks(&view, cx);
    keys(cx, "word-right word-right");
    assert_eq!(selection(&view, cx), Selection::caret(ids[0], 10));
    keys(cx, "word-right");
    assert_eq!(selection(&view, cx), Selection::caret(ids[1], 0), "past the end, the next block");
    keys(cx, "word-right word-left");
    assert_eq!(selection(&view, cx), Selection::caret(ids[1], 0));
    keys(cx, "word-shift-right");
    assert_eq!(selection(&view, cx), Selection::range(ids[1], 0, 5));
    keys(cx, "document-end");
    assert_eq!(selection(&view, cx), Selection::caret(ids[1], 11));
    keys(cx, "word-backspace");
    assert!(!markdown(&view, cx).contains("delta"), "the word key removes a word");
    keys(cx, "secondary-z");
    keys(cx, "document-start");
    assert_eq!(selection(&view, cx), Selection::caret(ids[0], 0));
}

#[ui::test]
fn a_drag_held_past_the_bottom_keeps_scrolling(cx: &mut TestAppContext) {
    let long: String = (0..400).map(|n| format!("paragraph {n}\n\n")).collect();
    let (view, cx) = open(cx, &long);
    let ids = blocks(&view, cx);
    let index = |view: &Entity<EditorView>, cx: &mut VisualTestContext| {
        let head = selection(view, cx).head().unwrap();
        ids.iter().position(|id| *id == head.block).unwrap()
    };
    let from = caret(&view, cx, Position::new(ids[1], 0)) + point(px(2.), px(6.));
    let left = MouseButton::Left;
    cx.simulate_event(MouseDownEvent {
        position: from,
        button: left,
        click_count: 1,
        ..Default::default()
    });
    let window = cx.update(|window, _| window.viewport_size());
    let below = point(from.x + px(30.), window.height + px(30.));
    cx.simulate_event(MouseMoveEvent {
        position: below,
        pressed_button: Some(left),
        ..Default::default()
    });
    settle(cx);
    let first = index(&view, cx);
    for _ in 0..6 {
        settle(cx);
    }
    let later = index(&view, cx);
    assert!(later > first + 3, "the selection keeps growing: {first} then {later}");
    let Selection::Text { anchor, .. } = selection(&view, cx) else { panic!("a text selection") };
    assert_eq!(anchor, Position::new(ids[1], 0), "the anchor never moved");
    assert!(view.read_with(cx, |view, _| view.caret_point(Position::new(ids[0], 0))).is_none());

    cx.simulate_event(MouseUpEvent { position: below, button: left, ..Default::default() });
    settle(cx);
    let stopped = index(&view, cx);
    settle(cx);
    settle(cx);
    assert_eq!(index(&view, cx), stopped, "letting go stops it");
}

#[ui::test]
fn the_caret_blinks_and_any_change_shows_it_again(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "blink");
    let shown = |view: &Entity<EditorView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| view.caret_shown())
    };
    let interval = cx.update(|_, cx| {
        cx.global::<ui::base::Theme>().cursor_blink.interval().expect("the theme blinks")
    });
    keys(cx, "right");
    assert!(shown(&view, cx));
    cx.executor().advance_clock(interval);
    cx.run_until_parked();
    assert!(!shown(&view, cx), "hidden after one interval");
    cx.executor().advance_clock(interval);
    cx.run_until_parked();
    assert!(shown(&view, cx), "and shown after the next");
    cx.executor().advance_clock(interval);
    cx.run_until_parked();
    assert!(!shown(&view, cx));
    keys(cx, "right");
    assert!(shown(&view, cx), "moving the caret shows it at once");
}

#[ui::test]
fn a_click_past_the_end_of_a_line_lands_at_its_end(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "plain text\n\n1. numbered\n\n> quoted\n\n# Title\n");
    let ids = blocks(&view, cx);
    for (index, length) in [(0, 10), (1, 8), (2, 6), (3, 5)] {
        let end = caret(&view, cx, Position::new(ids[index], length));
        press(cx, end + point(px(200.), px(6.)), 1);
        assert_eq!(selection(&view, cx), Selection::caret(ids[index], length), "block {index}");
        let start = caret(&view, cx, Position::new(ids[index], 0));
        press(cx, point(px(1.), start.y + px(6.)), 1);
        assert_eq!(selection(&view, cx), Selection::caret(ids[index], 0), "block {index}");
    }
}

#[ui::test]
fn any_document_takes_every_key_and_click(cx: &mut TestAppContext) {
    use common::type_text;
    use ui::{ClipboardItem, EntityInputHandler as _};
    let every_key = "left right up down home end shift-left shift-right shift-up shift-down \
        shift-home shift-end word-left word-right word-shift-left word-shift-right \
        document-start document-end backspace delete alt-backspace alt-delete word-backspace \
        word-delete enter tab shift-tab escape secondary-a secondary-b secondary-i secondary-u \
        secondary-shift-x secondary-e secondary-c secondary-x secondary-v secondary-z \
        secondary-shift-z secondary-a secondary-a backspace secondary-z";
    let documents = [
        "",
        "---\n",
        "```\n\n```\n",
        "- [ ] \n  - \n    1. \n",
        "# \n\n> \n\n---\n\n---\n",
        "e\u{301}\u{301}👩‍👩‍👧\u{200D}\u{FEFF}\n\n\u{202E}abc\n",
    ];
    for document in documents {
        let (view, cx) = open(cx, document);
        // Each key twice over: once as the document stands, once after
        // the first pass has changed it.
        for _ in 0..2 {
            keys(cx, every_key);
            type_text(cx, "/");
            keys(cx, "down up enter");
            type_text(cx, "- ");
            type_text(cx, "```");
            type_text(cx, "---");
        }
        let window = cx.update(|window, _| window.viewport_size());
        let spots = [
            point(px(-10.), px(-10.)),
            point(px(3.), px(3.)),
            point(window.width - px(2.), px(40.)),
            point(px(200.), window.height + px(500.)),
        ];
        for at in spots {
            for clicks in 1..=3 {
                press(cx, at, clicks);
            }
        }
        // What an input method or a paste might send.
        cx.update(|window, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(
                "\u{0}\n\n\n```\n- [x] \t\r\n> ".into(),
            ));
            view.update(cx, |view, cx| {
                // Ranges that run backwards, as a confused caller might send.
                let backwards = |start: usize, end: usize| std::ops::Range { start, end };
                let (replace, select) = (backwards(50, 9), backwards(7, 3));
                view.replace_and_mark_text_in_range(Some(replace), "か", Some(select), window, cx);
                view.replace_text_in_range(Some(usize::MAX - 1..usize::MAX), "", window, cx);
                view.unmark_text(window, cx);
                let _ = view.text_for_range(backwards(4, 2), &mut None, window, cx);
                let _ = view.bounds_for_range(0..999, Default::default(), window, cx);
            });
        });
        keys(cx, "secondary-v secondary-v secondary-a secondary-a secondary-a delete");
        settle(cx);
        let ids = blocks(&view, cx);
        assert!(!ids.is_empty(), "a document always has a block: {document:?}");
        let _ = (markdown(&view, cx), selection(&view, cx));
    }
}
