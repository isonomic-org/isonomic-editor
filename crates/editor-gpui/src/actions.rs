// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use ui::{App, KeyBinding};

/// The key context the editor view sets; bindings apply only inside.
pub const CONTEXT: &str = "IsonomicEditor";
/// The key context of the search bar.
pub const FIND_CONTEXT: &str = "IsonomicEditorFind";
/// The key context of the link bar.
pub const LINK_CONTEXT: &str = "IsonomicEditorLink";

ui::actions!(
    editor,
    [
        Backspace,
        BackspaceWord,
        Delete,
        DeleteWord,
        Newline,
        Indent,
        Outdent,
        Undo,
        Redo,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        MoveToStart,
        MoveToEnd,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectToStart,
        SelectToEnd,
        SelectAll,
        Cancel,
        Find,
        FindNext,
        FindPrevious,
        EditLink,
        MoveWordLeft,
        MoveWordRight,
        SelectWordLeft,
        SelectWordRight,
        MoveToDocumentStart,
        MoveToDocumentEnd,
        SelectToDocumentStart,
        SelectToDocumentEnd,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleStrike,
        ToggleCode,
        Copy,
        Cut,
        Paste,
    ]
);

/// The platform's command modifier with a key: Command on macOS,
/// Control elsewhere.
#[cfg(target_os = "macos")]
macro_rules! secondary {
    ($key:literal) => {
        concat!("cmd-", $key)
    };
}

#[cfg(not(target_os = "macos"))]
macro_rules! secondary {
    ($key:literal) => {
        concat!("ctrl-", $key)
    };
}

/// The key that moves by a word with an arrow: Option on macOS,
/// Control elsewhere.
#[cfg(target_os = "macos")]
macro_rules! by_word {
    ($key:literal) => {
        concat!("alt-", $key)
    };
}

#[cfg(not(target_os = "macos"))]
macro_rules! by_word {
    ($key:literal) => {
        concat!("ctrl-", $key)
    };
}

/// The keys for the start and end of the document: Command with Up and
/// Down on macOS, Control with Home and End elsewhere.
#[cfg(target_os = "macos")]
const DOCUMENT_EDGES: [&str; 4] = ["cmd-up", "cmd-down", "cmd-shift-up", "cmd-shift-down"];
#[cfg(not(target_os = "macos"))]
const DOCUMENT_EDGES: [&str; 4] = ["ctrl-home", "ctrl-end", "ctrl-shift-home", "ctrl-shift-end"];

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("alt-backspace", BackspaceWord, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("alt-delete", DeleteWord, context),
        KeyBinding::new(by_word!("backspace"), BackspaceWord, context),
        KeyBinding::new(by_word!("delete"), DeleteWord, context),
        KeyBinding::new("enter", Newline, context),
        KeyBinding::new("escape", Cancel, context),
        KeyBinding::new("tab", Indent, context),
        KeyBinding::new("shift-tab", Outdent, context),
        KeyBinding::new(secondary!("z"), Undo, context),
        KeyBinding::new(secondary!("shift-z"), Redo, context),
        KeyBinding::new("left", MoveLeft, context),
        KeyBinding::new("right", MoveRight, context),
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("home", MoveToStart, context),
        KeyBinding::new("end", MoveToEnd, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("shift-home", SelectToStart, context),
        KeyBinding::new("shift-end", SelectToEnd, context),
        KeyBinding::new(by_word!("left"), MoveWordLeft, context),
        KeyBinding::new(by_word!("right"), MoveWordRight, context),
        KeyBinding::new(by_word!("shift-left"), SelectWordLeft, context),
        KeyBinding::new(by_word!("shift-right"), SelectWordRight, context),
        KeyBinding::new(DOCUMENT_EDGES[0], MoveToDocumentStart, context),
        KeyBinding::new(DOCUMENT_EDGES[1], MoveToDocumentEnd, context),
        KeyBinding::new(DOCUMENT_EDGES[2], SelectToDocumentStart, context),
        KeyBinding::new(DOCUMENT_EDGES[3], SelectToDocumentEnd, context),
        KeyBinding::new(secondary!("a"), SelectAll, context),
        KeyBinding::new(secondary!("f"), Find, context),
        KeyBinding::new(secondary!("k"), EditLink, context),
        KeyBinding::new(secondary!("g"), FindNext, context),
        KeyBinding::new(secondary!("shift-g"), FindPrevious, context),
        KeyBinding::new(secondary!("b"), ToggleBold, context),
        KeyBinding::new(secondary!("i"), ToggleItalic, context),
        KeyBinding::new(secondary!("u"), ToggleUnderline, context),
        KeyBinding::new(secondary!("shift-x"), ToggleStrike, context),
        KeyBinding::new(secondary!("e"), ToggleCode, context),
        KeyBinding::new(secondary!("c"), Copy, context),
        KeyBinding::new(secondary!("x"), Cut, context),
        KeyBinding::new(secondary!("v"), Paste, context),
    ]);
    cx.bind_keys([KeyBinding::new("escape", Cancel, Some(LINK_CONTEXT))]);
    let find = Some(FIND_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, find),
        KeyBinding::new(secondary!("g"), FindNext, find),
        KeyBinding::new(secondary!("shift-g"), FindPrevious, find),
    ]);
}
