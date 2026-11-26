// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A document saved as bytes and opened again.

extern crate ui as gpui;

use editor_gpui::EditorState;
use ui::{AppContext as _, TestAppContext};

const TEXT: &str = "# Title\n\nSome **bold** text.\n\n- one\n- two\n";

#[ui::test]
async fn a_saved_document_opens_in_the_background(cx: &mut TestAppContext) {
    let (saved, before) = cx.update(|cx| {
        let state = cx.new(|cx| EditorState::from_markdown(TEXT, 1, cx));
        state.update(cx, |state, cx| state.insert_text("An edit. ", cx));
        let state = state.read(cx);
        (state.save().unwrap(), state.markdown())
    });

    let loading = cx.update(|cx| EditorState::load(saved.clone(), 2, cx));
    let state = loading.await.unwrap();
    assert_eq!(state.read_with(cx, |state, _| state.markdown()), before);

    // It edits and undoes like any other, and saves what it holds.
    state.update(cx, |state, cx| state.insert_text("More. ", cx));
    assert_ne!(state.read_with(cx, |state, _| state.markdown()), before);
    state.update(cx, |state, cx| state.undo(cx));
    assert_eq!(state.read_with(cx, |state, _| state.markdown()), before);
    let again = state.read_with(cx, |state, _| state.save().unwrap());
    let reopened = EditorState::open(&again, 3).unwrap();
    assert_eq!(reopened.markdown(), before);
}

#[ui::test]
async fn bytes_that_are_not_a_document_are_refused(cx: &mut TestAppContext) {
    let saved = cx
        .update(|cx| cx.new(|cx| EditorState::from_markdown(TEXT, 1, cx)).read(cx).save().unwrap());
    let mut cut = saved.clone();
    cut.truncate(saved.len() / 2);
    let mut flipped = saved.clone();
    for byte in flipped.iter_mut().skip(16).step_by(7) {
        *byte ^= 0x5a;
    }
    for bytes in [Vec::new(), b"not a document".to_vec(), cut, flipped] {
        assert!(EditorState::open(&bytes, 2).is_err());
        let loading = cx.update(|cx| EditorState::load(bytes, 3, cx));
        assert!(loading.await.is_err());
    }
}
