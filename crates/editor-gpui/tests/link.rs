// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Links, in a headless window.

extern crate ui as gpui;

mod common;

use common::{keys, markdown, open, type_text};
use editor_gpui::openable;

#[ui::test]
fn the_link_bar_links_changes_and_unlinks(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, "Read the manual first.\n");

    // A caret in a word links the word.
    keys(cx, "document-start right right right right right right");
    keys(cx, "secondary-k");
    type_text(cx, "example.org/manual");
    keys(cx, "enter");
    assert_eq!(markdown(&view, cx), "Read [the](example.org/manual) manual first.\n");

    // The keyboard is the text's again, and the linked word is selected.
    // A caret in a link opens the bar on that link, with its address.
    keys(cx, "left right");
    keys(cx, "secondary-k");
    keys(cx, "secondary-a");
    type_text(cx, "https://example.org/guide");
    keys(cx, "enter");
    assert_eq!(markdown(&view, cx), "Read [the](https://example.org/guide) manual first.\n");

    // Escape leaves the link alone.
    keys(cx, "secondary-k");
    keys(cx, "secondary-a");
    type_text(cx, "https://elsewhere.example");
    keys(cx, "escape");
    assert_eq!(markdown(&view, cx), "Read [the](https://example.org/guide) manual first.\n");

    // A selection across words links all of it; one step of undo
    // takes that back.
    keys(
        cx,
        "document-start shift-right shift-right shift-right shift-right shift-right shift-right shift-right shift-right",
    );
    keys(cx, "secondary-k");
    type_text(cx, "https://example.org/all");
    keys(cx, "enter");
    assert_eq!(markdown(&view, cx), "[Read the](https://example.org/all) manual first.\n");
    keys(cx, "secondary-z");
    assert_eq!(markdown(&view, cx), "Read [the](https://example.org/guide) manual first.\n");

    // An empty address takes the link away.
    keys(cx, "document-start right right right right right right");
    keys(cx, "secondary-k");
    keys(cx, "secondary-a backspace enter");
    assert_eq!(markdown(&view, cx), "Read the manual first.\n");

    // Where there is no word, there is nothing to link and no bar.
    keys(cx, "document-end secondary-k");
    type_text(cx, "!");
    assert_eq!(markdown(&view, cx), "Read the manual first.!\n");
}

#[test]
fn only_web_and_mail_addresses_open() {
    assert_eq!(openable("https://example.org/a"), Some("https://example.org/a".to_owned()));
    assert_eq!(openable(" HTTP://example.org "), Some("HTTP://example.org".to_owned()));
    assert_eq!(
        openable("mailto:someone@example.org"),
        Some("mailto:someone@example.org".to_owned())
    );
    assert_eq!(openable("example.org/manual"), Some("https://example.org/manual".to_owned()));
    for refused in ["", "file:///etc/passwd", "javascript:alert(1)", "notes", "a b.org", ".hidden"]
    {
        assert_eq!(openable(refused), None, "{refused:?}");
    }
}
