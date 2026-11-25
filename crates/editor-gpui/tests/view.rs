// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The view in a headless window: real layout, real key bindings and
//! real pointer events, with no screen.

extern crate ui as gpui;

mod common;

use common::{blocks, keys, markdown, open, selection, settle, type_text};
use editor_gpui::EditorView;
use editor_model::{Position, Selection};
use ui::{
    Entity, EntityInputHandler as _, Modifiers, ScrollDelta, ScrollEvent, TestAppContext,
    VisualTestContext, point, px,
};

#[ui::test]
fn typing_builds_a_document(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "");
    type_text(cx, "# Title");
    keys(cx, "enter");
    type_text(cx, "Hello ");
    keys(cx, "secondary-b");
    type_text(cx, "bold");
    keys(cx, "secondary-b");
    type_text(cx, " world");
    keys(cx, "enter");
    type_text(cx, "- item");
    keys(cx, "enter");
    type_text(cx, "nested");
    keys(cx, "tab");
    assert_eq!(markdown(&view, cx), "# Title\n\nHello **bold** world\n\n- item\n  - nested\n");
    keys(cx, "secondary-z");
    assert_eq!(markdown(&view, cx), "# Title\n\nHello **bold** world\n\n- item\n- nested\n");
    keys(cx, "secondary-shift-z");
    keys(cx, "backspace backspace");
    assert!(markdown(&view, cx).ends_with("  - nest\n"));
}

#[ui::test]
fn every_text_block_is_drawn_with_geometry(cx: &mut TestAppContext) {
    let (view, cx) =
        open(cx, "# Title\n\nBody text\n\n- one\n  - two\n\n---\n\n```\ncode\n```\n\n");
    let ids = blocks(&view, cx);
    let bounds: Vec<_> =
        view.read_with(cx, |view, _| ids.iter().map(|id| view.leaf_bounds(*id)).collect());
    // Title, body, two list items and code are text; the divider is not.
    assert_eq!(view.read_with(cx, |view, _| view.drawn_leaves()), 5);
    assert!(bounds[4].is_none(), "a divider has no text leaf");
    let text: Vec<_> = bounds.iter().flatten().collect();
    for pair in text.windows(2) {
        assert!(pair[0].origin.y < pair[1].origin.y, "blocks stack downwards");
    }
    assert!(bounds[3].unwrap().origin.x > bounds[2].unwrap().origin.x, "a nested item is indented");
    assert!(bounds[0].unwrap().size.height > bounds[1].unwrap().size.height, "a heading is taller");
    let caret = view.read_with(cx, |view, _| view.caret_point(Position::new(ids[0], 0)));
    assert!(caret.is_some());
}

#[ui::test]
fn the_pointer_places_and_extends_the_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "first block\n\nsecond block\n");
    let ids = blocks(&view, cx);
    let at =
        |view: &Entity<EditorView>, cx: &mut VisualTestContext, block: usize, offset: usize| {
            let position = Position::new(ids[block], offset);
            view.read_with(cx, |view, _| view.caret_point(position)).unwrap()
                + point(px(1.), px(4.))
        };
    let target = at(&view, cx, 1, 6);
    cx.simulate_click(target, Modifiers::none());
    settle(cx);
    assert_eq!(selection(&view, cx), Selection::caret(ids[1], 6));
    let target = at(&view, cx, 0, 2);
    cx.simulate_click(target, Modifiers::shift());
    settle(cx);
    let expected =
        Selection::Text { anchor: Position::new(ids[1], 6), head: Position::new(ids[0], 2) };
    assert_eq!(selection(&view, cx), expected);
    type_text(cx, "X");
    assert_eq!(markdown(&view, cx), "fiX block\n");
}

#[ui::test]
fn arrow_keys_move_within_and_across_blocks(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "ab\n\ncdé\n");
    let ids = blocks(&view, cx);
    keys(cx, "right right");
    assert_eq!(selection(&view, cx), Selection::caret(ids[0], 2));
    keys(cx, "right");
    assert_eq!(
        selection(&view, cx),
        Selection::caret(ids[1], 0),
        "right at the end enters the next block"
    );
    keys(cx, "end");
    assert_eq!(selection(&view, cx), Selection::caret(ids[1], 4));
    keys(cx, "shift-left");
    assert_eq!(selection(&view, cx), Selection::range(ids[1], 4, 2), "a grapheme, not a byte");
    keys(cx, "up");
    assert_eq!(selection(&view, cx).head().unwrap().block, ids[0], "up leaves the block");
    keys(cx, "down");
    assert_eq!(selection(&view, cx).head().unwrap().block, ids[1]);
    keys(cx, "secondary-a");
    assert_eq!(selection(&view, cx), Selection::range(ids[1], 0, 4));
    keys(cx, "backspace");
    assert_eq!(markdown(&view, cx), "ab\n");
}

#[ui::test]
fn platform_text_input_composes_in_place(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "ab");
    let id = blocks(&view, cx)[0];
    keys(cx, "right");
    view.update_in(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "に", None, window, cx);
        assert_eq!(view.marked_text_range(window, cx), Some(1..2));
        view.replace_and_mark_text_in_range(None, "にほ", None, window, cx);
        assert_eq!(view.marked_text_range(window, cx), Some(1..3));
        view.replace_text_in_range(None, "日本", window, cx);
        assert_eq!(view.marked_text_range(window, cx), None);
        let selected = view.selected_text_range(false, window, cx).unwrap();
        assert_eq!(selected.range, 3..3, "counted in UTF-16 units");
        assert_eq!(view.text_for_range(1..3, &mut None, window, cx).as_deref(), Some("日本"));
    });
    settle(cx);
    assert_eq!(markdown(&view, cx), "a日本b\n");
    assert_eq!(selection(&view, cx), Selection::caret(id, 7));

    // To the person typing, the whole composition was one edit.
    keys(cx, "secondary-z");
    assert_eq!(markdown(&view, cx), "ab\n");
    keys(cx, "secondary-shift-z");
    assert_eq!(markdown(&view, cx), "a日本b\n");
    // And what is typed next is its own step again.
    keys(cx, "enter");
    type_text(cx, "x");
    keys(cx, "secondary-z");
    assert_eq!(markdown(&view, cx), "a日本\n\nb\n");
}

#[ui::test]
fn copy_cut_and_paste_carry_formatting(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "one **two** three\n\n- item\n  - nested\n\nlast\n");
    let ids = blocks(&view, cx);
    let select = |view: &Entity<EditorView>, cx: &mut VisualTestContext, selection: Selection| {
        view.update(cx, |view, cx| {
            view.state().update(cx, |state, cx| state.select(selection, cx))
        });
    };
    // Part of one block travels as marked text.
    select(&view, cx, Selection::range(ids[0], 4, 13));
    keys(cx, "secondary-c");
    select(&view, cx, Selection::caret(ids[3], 4));
    keys(cx, "secondary-v");
    assert!(markdown(&view, cx).ends_with("last**two** three\n"));
    keys(cx, "secondary-z");
    // Whole blocks travel with their type and nesting.
    let across =
        Selection::Text { anchor: Position::new(ids[1], 0), head: Position::new(ids[2], 6) };
    select(&view, cx, across);
    keys(cx, "secondary-x");
    assert_eq!(markdown(&view, cx), "one **two** three\n\n- \n\nlast\n");
    keys(cx, "secondary-z");
    select(&view, cx, Selection::caret(ids[3], 4));
    keys(cx, "secondary-v");
    assert_eq!(
        markdown(&view, cx),
        "one **two** three\n\n- item\n  - nested\n\nlast\n\n- item\n  - nested\n"
    );
}

#[ui::test]
fn the_caret_is_scrolled_into_view(cx: &mut TestAppContext) {
    let long: String = (0..400).map(|n| format!("paragraph {n}\n\n")).collect();
    let (view, cx) = open(cx, &long);
    let ids = blocks(&view, cx);
    let height = cx.update(|window, _| window.viewport_size().height);
    let caret_y = |view: &Entity<EditorView>, cx: &mut VisualTestContext, block: usize| {
        view.read_with(cx, |view, _| view.caret_point(Position::new(ids[block], 0))).map(|at| at.y)
    };
    assert_eq!(caret_y(&view, cx, 399), None, "the last block is far below and not drawn");
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| state.select(Selection::caret(ids[399], 0), cx));
    });
    settle(cx);
    settle(cx);
    let y = caret_y(&view, cx, 399).unwrap();
    assert!(y > px(0.) && y < height, "the caret is inside the window, at {y:?}");
    assert_eq!(caret_y(&view, cx, 0), None, "and the first block no longer is");
    keys(cx, "end enter");
    type_text(cx, "typed at the end");
    assert!(markdown(&view, cx).ends_with("typed at the end\n"));
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| state.select(Selection::caret(ids[0], 0), cx));
    });
    settle(cx);
    settle(cx);
    let y = caret_y(&view, cx, 0).unwrap();
    assert!(y >= px(0.) && y < height, "and back at the top, at {y:?}");
}

#[ui::test]
fn only_the_rows_in_view_are_drawn(cx: &mut TestAppContext) {
    let long: String = (0..5_000).map(|n| format!("{}. item {n}\n", n + 1)).collect();
    let (view, cx) = open(cx, &long);
    let ids = blocks(&view, cx);
    assert_eq!(ids.len(), 5_000);
    let drawn = view.read_with(cx, |view, _| view.drawn_leaves());
    assert!(drawn > 5 && drawn < 200, "a screenful of rows, not {drawn}");

    // Walking down past the bottom keeps the caret in the window, one
    // block at a time.
    let height = cx.update(|window, _| window.viewport_size().height);
    for _ in 0..120 {
        keys(cx, "down");
    }
    let head = selection(&view, cx).head().unwrap();
    assert_eq!(head.block, ids[120]);
    settle(cx);
    let y = view.read_with(cx, |view, _| view.caret_point(head)).unwrap().y;
    assert!(y > px(0.) && y < height, "the caret is inside the window, at {y:?}");
    assert!(view.read_with(cx, |view, _| view.drawn_leaves()) < 200);

    // The wheel scrolls the rows; what scrolled away is no longer drawn.
    let before = view.read_with(cx, |view, _| view.caret_point(head)).unwrap().y;
    cx.simulate_event(ScrollEvent {
        position: point(px(100.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-90.))),
        ..Default::default()
    });
    settle(cx);
    let after = view.read_with(cx, |view, _| view.caret_point(head)).unwrap().y;
    assert_eq!(before - after, px(90.), "the row moved up by what the wheel asked");

    // Editing far from the top changes only what it should.
    type_text(cx, "x");
    keys(cx, "enter");
    assert_eq!(blocks(&view, cx).len(), 5_001);
    keys(cx, "secondary-z secondary-z");
    assert_eq!(blocks(&view, cx), ids);
    for _ in 0..120 {
        keys(cx, "up");
    }
    assert_eq!(selection(&view, cx).head().unwrap().block, ids[0]);
}

#[ui::test]
fn home_and_end_stop_at_the_visual_line(cx: &mut TestAppContext) {
    let words = "wrap ".repeat(400);
    let (view, cx) = open(cx, words.trim_end());
    let id = blocks(&view, cx)[0];
    let length = words.trim_end().len();
    let caret = |view: &Entity<EditorView>, cx: &mut VisualTestContext| {
        let head = selection(view, cx).head().unwrap();
        (head.offset, view.read_with(cx, |view, _| view.caret_point(head)).unwrap())
    };
    let (_, top) = caret(&view, cx);

    keys(cx, "end");
    let (first_end, at) = caret(&view, cx);
    assert!(first_end > 0 && first_end < length, "the first line's end, not the block's");
    assert_eq!(at.y, top.y, "the caret stays on its line");
    keys(cx, "end");
    assert_eq!(caret(&view, cx).0, first_end, "end twice stays put");

    keys(cx, "down end");
    let (second_end, low) = caret(&view, cx);
    assert!(second_end > first_end && second_end < length && low.y > top.y);
    keys(cx, "home");
    let (second_start, start) = caret(&view, cx);
    assert!(second_start > first_end && second_start < second_end);
    assert_eq!(start.y, low.y);
    keys(cx, "shift-end");
    assert_eq!(selection(&view, cx), Selection::range(id, second_start, second_end));
}

#[ui::test]
fn the_insert_menu_opens_filters_and_converts(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "");
    let menu = |view: &Entity<EditorView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| {
            let menu = view.state().read(cx).insert_menu().cloned()?;
            Some((menu.entries.iter().map(|entry| entry.title).collect::<Vec<_>>(), menu.selected))
        })
    };
    type_text(cx, "/");
    let (all, selected) = menu(&view, cx).unwrap();
    assert!(all.len() >= 8 && selected == 0, "every block type is offered: {all:?}");

    // Typing narrows it; Down and Up step through and wrap around.
    type_text(cx, "h");
    type_text(cx, "e");
    assert_eq!(menu(&view, cx).unwrap().0, ["Heading 1", "Heading 2", "Heading 3"]);
    keys(cx, "down down down up");
    assert_eq!(menu(&view, cx).unwrap().1, 2);
    let before = selection(&view, cx);
    assert_eq!(before.head().unwrap().offset, 3, "the arrows moved the menu, not the caret");

    // Enter applies the choice instead of splitting the block.
    keys(cx, "enter");
    assert_eq!(menu(&view, cx), None);
    type_text(cx, "Title");
    assert_eq!(markdown(&view, cx), "### Title\n");
    assert_eq!(blocks(&view, cx).len(), 1);

    // Escape closes it and leaves the text; so does typing past any match.
    keys(cx, "enter");
    type_text(cx, "/");
    assert!(menu(&view, cx).is_some());
    keys(cx, "escape");
    assert_eq!(menu(&view, cx), None);
    type_text(cx, "q");
    assert_eq!(menu(&view, cx), None, "it stays closed once dismissed");
    keys(cx, "backspace backspace");
    type_text(cx, "/");
    type_text(cx, "z");
    type_text(cx, "z");
    assert_eq!(menu(&view, cx), None, "nothing matches, so it closes");
    assert!(markdown(&view, cx).ends_with("/zz\n"));

    // A click on an entry chooses it.
    keys(cx, "backspace backspace backspace");
    type_text(cx, "/");
    type_text(cx, "quo");
    settle(cx);
    let head = selection(&view, cx).head().unwrap();
    let caret = view.read_with(cx, |view, _| view.caret_point(head)).unwrap();
    cx.simulate_click(caret + point(px(20.), px(70.)), Modifiers::none());
    settle(cx);
    assert_eq!(menu(&view, cx), None);
    type_text(cx, "said");
    assert!(markdown(&view, cx).ends_with("> said\n"), "{}", markdown(&view, cx));
}

#[ui::test]
fn the_insert_menu_stays_inside_the_window(cx: &mut TestAppContext) {
    // Enough paragraphs to put the caret on the window's last lines.
    let long: String = (0..400).map(|n| format!("paragraph {n}\n\n")).collect();
    let (view, cx) = open(cx, &long);
    let ids = blocks(&view, cx);
    let height = cx.update(|window, _| window.viewport_size().height);
    let last_shown = ids
        .iter()
        .rev()
        .find(|id| {
            let at = view.read_with(cx, |view, _| view.caret_point(Position::new(**id, 0)));
            at.is_some_and(|at| at.y < height - px(30.))
        })
        .copied()
        .unwrap();
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| state.select(Selection::caret(last_shown, 0), cx));
    });
    settle(cx);
    type_text(cx, "/");
    settle(cx);
    let bounds = view.read_with(cx, |view, _| view.menu_bounds()).expect("the menu is drawn");
    let caret = view.read_with(cx, |view, _| view.caret_point(Position::new(last_shown, 0)));
    assert!(bounds.origin.y >= px(0.) && bounds.origin.y + bounds.size.height <= height);
    assert!(
        bounds.origin.y + bounds.size.height <= caret.unwrap().y + px(1.),
        "with no room below, it opens above the caret: {bounds:?}"
    );
}

#[ui::test]
fn an_empty_paragraph_with_the_caret_shows_a_hint(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, "");
    let id = blocks(&view, cx)[0];
    let caret = |view: &Entity<EditorView>, cx: &mut VisualTestContext, at| {
        view.read_with(cx, |view, _| view.caret_point(at)).unwrap()
    };
    // How wide the text drawn in a block is: from its start to the end
    // of whatever the leaf holds.
    let width = |view: &Entity<EditorView>, cx: &mut VisualTestContext, id| {
        let points = view.read_with(cx, |view, _| {
            (view.caret_point(Position::new(id, 0)), view.caret_point(Position::new(id, 999)))
        });
        points.1.unwrap().x - points.0.unwrap().x
    };
    // The hint is drawn, and it is not in the document.
    let left = caret(&view, cx, Position::new(id, 0)).x;
    assert!(width(&view, cx, id) > px(40.), "the hint is drawn");
    assert_eq!(markdown(&view, cx), "\n", "the document is still empty");
    assert_eq!(selection(&view, cx), Selection::caret(id, 0));

    // A click in the middle of the hint leaves the caret at the start,
    // and End has nowhere to go.
    let bounds = view.read_with(cx, |view, _| view.leaf_bounds(id)).unwrap();
    cx.simulate_click(bounds.origin + point(px(60.), px(8.)), Modifiers::none());
    settle(cx);
    keys(cx, "end shift-right");
    assert_eq!(selection(&view, cx), Selection::caret(id, 0));
    assert_eq!(caret(&view, cx, Position::new(id, 0)).x, left);

    // Typing replaces it; a second, empty paragraph without the caret shows nothing.
    type_text(cx, "a");
    assert_eq!(markdown(&view, cx), "a\n");
    keys(cx, "enter up");
    let second = blocks(&view, cx)[1];
    let without = width(&view, cx, second);
    keys(cx, "down");
    settle(cx);
    let with = width(&view, cx, second);
    assert!(with > without + px(40.), "the hint only with the caret: {without:?} then {with:?}");
    assert_eq!(markdown(&view, cx), "a\n");
}
