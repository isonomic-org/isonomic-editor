// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The editor view.
//!
//! [`EditorState`] holds one document, its selection and its undo
//! history, and turns key presses into transactions. [`EditorView`]
//! draws the document through the component library's text seam and
//! routes the pointer, the keyboard and platform text input to the
//! state. Handlers change the model; render builds a frame from a
//! snapshot; paint only reads.

// Derives and attributes from the component library expand to paths
// under this name.
extern crate ui as gpui;

mod actions;
mod input;
mod menu;
mod motion;
mod nodes;
mod rows;
mod state;
mod surface;
mod units;
mod view;

pub use actions::*;
pub use menu::InsertMenu;
pub use state::{EditorEvent, EditorState};
pub use view::EditorView;

/// Registers the editor's key bindings. Call once at start-up.
pub fn init(cx: &mut ui::App) {
    actions::bind_keys(cx);
}
