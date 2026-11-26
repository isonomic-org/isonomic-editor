// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding text, in a headless window.

extern crate ui as gpui;

mod common;

use common::{keys, markdown, open, selection, type_text};
use editor_gpui::EditorView;
use editor_model::Selection;
use ui::{Entity, VisualTestContext};

const TEXT: &str =
    "# The plan\n\nThe first thing, then the second.\n\n- Another item\n- nothing here\n";

/// The current match's place and how many there are.
fn found(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> Option<(Option<usize>, usize)> {
    view.read_with(cx, |view, cx| {
        let find = view.state().read(cx).found()?;
        Some((find.current(), find.matches().len()))
    })
}

fn selected(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, cx| {
        let state = view.state().read(cx);
        let doc = state.snapshot();
        match state.selection() {
            Selection::Text { anchor, head } if anchor.block == head.block => {
                let text = doc.get(head.block).unwrap().content.text();
                text[anchor.offset.min(head.offset)..anchor.offset.max(head.offset)].to_owned()
            }
            _ => String::new(),
        }
    })
}

#[ui::test]
fn the_search_bar_finds_and_steps_through_matches(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, TEXT);
    let before = markdown(&view, cx);
    assert_eq!(found(&view, cx), None);

    // Opened, the bar has the keyboard: typing looks for the text and
    // changes nothing in the document.
    keys(cx, "secondary-f");
    assert_eq!(found(&view, cx), Some((None, 0)));
    type_text(cx, "the");
    assert_eq!(
        found(&view, cx),
        Some((Some(0), 5)),
        "case is ignored; \"then\" and \"Another\" count"
    );
    assert_eq!(selected(&view, cx), "The");
    assert_eq!(markdown(&view, cx), before);

    // Enter goes on, Shift-Enter back, and both go round at the ends.
    keys(cx, "enter");
    assert_eq!(found(&view, cx), Some((Some(1), 5)));
    keys(cx, "shift-enter shift-enter");
    assert_eq!(found(&view, cx), Some((Some(4), 5)));
    assert_eq!(selected(&view, cx), "the");
    keys(cx, "secondary-g");
    assert_eq!(found(&view, cx), Some((Some(0), 5)));
    keys(cx, "secondary-shift-g");
    assert_eq!(found(&view, cx), Some((Some(4), 5)));

    // Keys the editor would act on do nothing to the document from here.
    keys(cx, "secondary-b backspace");
    assert_eq!(markdown(&view, cx), before);
    assert_eq!(found(&view, cx), Some((Some(5), 7)), "\"th\" is in \"thing\" and \"nothing\" too");

    // Text that is nowhere leaves the selection where it was.
    let kept = selection(&view, cx);
    type_text(cx, "zzz");
    assert_eq!(found(&view, cx), Some((None, 0)));
    assert_eq!(selection(&view, cx), kept);

    // Escape closes the bar and the keyboard is the text's again.
    keys(cx, "escape");
    assert_eq!(found(&view, cx), None);
    type_text(cx, "X");
    assert_ne!(markdown(&view, cx), before);
}

#[ui::test]
fn matches_follow_edits_and_the_selection_seeds_the_search(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, TEXT);
    // With a word selected, the bar opens looking for it.
    keys(cx, "document-start shift-right shift-right shift-right");
    assert_eq!(selected(&view, cx), "The");
    keys(cx, "secondary-f");
    assert_eq!(found(&view, cx), Some((Some(0), 5)));

    // Back in the text with the search still open, an edit is searched again.
    view.update_in(cx, |view, window, cx| {
        let focus = ui::Focusable::focus_handle(view, cx);
        window.focus(&focus, cx);
    });
    common::settle(cx);
    type_text(cx, "A");
    assert_eq!(found(&view, cx), Some((None, 4)), "the match typed over is gone");
    keys(cx, "secondary-g");
    assert_eq!(found(&view, cx), Some((Some(0), 4)));
    assert_eq!(selected(&view, cx), "The");
    keys(cx, "escape");
    assert_eq!(found(&view, cx), None);
}
