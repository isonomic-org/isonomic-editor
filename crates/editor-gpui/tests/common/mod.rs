// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! An editor in a headless window, and the things a person does to it.
#![allow(dead_code)]

use editor_gpui::{EditorState, EditorView};
use editor_model::{BlockId, Selection};
use ui::{AppContext as _, Entity, Focusable as _, TestAppContext, VisualTestContext};

pub fn open<'a>(
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
pub fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

pub fn markdown(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, cx| view.state().read(cx).markdown())
}

pub fn selection(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> Selection {
    view.read_with(cx, |view, cx| view.state().read(cx).selection().clone())
}

pub fn blocks(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> Vec<BlockId> {
    view.read_with(cx, |view, cx| view.state().read(cx).snapshot().flat())
}

/// Presses keys. `secondary` stands for the platform's command
/// modifier, `word` for the one that moves by words, and
/// `document-start` and `document-end` for those keys.
pub fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    let mac = cfg!(target_os = "macos");
    let keystrokes = keystrokes
        .replace("secondary", if mac { "cmd" } else { "ctrl" })
        .replace("word", if mac { "alt" } else { "ctrl" })
        .replace("document-start", if mac { "cmd-up" } else { "ctrl-home" })
        .replace("document-end", if mac { "cmd-down" } else { "ctrl-end" });
    cx.simulate_keystrokes(&keystrokes);
    settle(cx);
}

pub fn type_text(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    settle(cx);
}
