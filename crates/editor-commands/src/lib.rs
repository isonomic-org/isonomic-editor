// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Editing commands, input rules and normalization.
//!
//! Every command is a pure function from a snapshot and a selection to
//! an optional [`editor_model::Transaction`]. `None` means the command
//! does not apply; nothing here mutates a document.

mod build;
mod find;
mod marks;
mod menu;
mod normalize;
mod paste;
mod rules;
mod structure;
mod text;

pub use build::block_ranges as selection_ranges;
pub use find::find_text;
pub use marks::{set_link, toggle_mark};
pub use menu::{MENU_TRIGGER, choose_menu_entry, menu_query, menu_trigger};
pub use normalize::normalize;
pub use paste::{copy_fragment, insert_block, insert_fragment};
pub use rules::input_rule;
pub use structure::{
    convert_block, indent, move_down, move_up, outdent, select_all, split_block, toggle_todo,
};
pub use text::{Unit, delete_backward, delete_forward, delete_selection, insert_text};
