// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The view in a headless window: real layout, real key bindings and
//! the real text input path, with no screen.

extern crate ui as gpui;

use editor_gpui::{EditorState, EditorView};
use editor_model::{BlockId, Position, Selection};
use ui::{
    AppContext as _, Entity, EntityInputHandler as _, Focusable as _, Modifiers, TestAppContext,
    VisualTestContext, point, px,
};

fn open<'a>(
    cx: &'a mut TestAppContext,
    markdown: &str,
) -> (Entity<EditorView>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        ui::init(cx);
        editor_gpui::init(cx);
    });
    let markdown = markdown.to_owned();
    let (view, vcx) = cx.add_window_view(move |window, cx| {
        let state = cx.new(|cx| EditorState::from_markdown(&markdown, 1, cx));
        let view = EditorView::new(state, cx);
        let focus = view.focus_handle(cx);
        window.focus(&focus, cx);
        view
    });
    settle(vcx);
    (view, vcx)
}

/// Draws a frame, so geometry and the input handler are current.
fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

fn markdown(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, cx| view.state().read(cx).markdown())
}

fn selection(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> Selection {
    view.read_with(cx, |view, cx| view.state().read(cx).selection().clone())
}

fn blocks(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> Vec<BlockId> {
    view.read_with(cx, |view, cx| view.state().read(cx).snapshot().flat())
}

/// Presses keys. `secondary` stands for the platform's command
/// modifier.
fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    let modifier = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.simulate_keystrokes(&keystrokes.replace("secondary", modifier));
    settle(cx);
}

fn type_text(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    settle(cx);
}

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
