// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use ui::{App, KeyBinding};

/// The key context the editor view sets; bindings apply only inside.
pub const CONTEXT: &str = "IsonomicEditor";

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

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("alt-backspace", BackspaceWord, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("alt-delete", DeleteWord, context),
        KeyBinding::new("enter", Newline, context),
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
        KeyBinding::new(secondary!("a"), SelectAll, context),
        KeyBinding::new(secondary!("b"), ToggleBold, context),
        KeyBinding::new(secondary!("i"), ToggleItalic, context),
        KeyBinding::new(secondary!("u"), ToggleUnderline, context),
        KeyBinding::new(secondary!("shift-x"), ToggleStrike, context),
        KeyBinding::new(secondary!("e"), ToggleCode, context),
        KeyBinding::new(secondary!("c"), Copy, context),
        KeyBinding::new(secondary!("x"), Cut, context),
        KeyBinding::new(secondary!("v"), Paste, context),
    ]);
}
