// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The document model: blocks, inline text, positions, selections,
//! primitive operations and the schema registry.
//!
//! This crate is headless. It names no UI framework and no replication
//! library, and it never panics on input: every fallible call returns
//! a [`ModelError`].

mod block;
mod doc;
mod error;
mod ids;
mod inline;
mod limits;
mod marks;
mod ops;
mod position;
mod schema;
mod value;

pub use block::{Block, BlockDraft, BlockKind, Content, kinds};
pub use doc::Document;
pub use error::ModelError;
pub use ids::{BlockId, ColumnId, IdSource, RowId, SeqIds};
pub use inline::{
    ATOM_CHAR, Atom, InlineText, Run, next_grapheme, next_word, prev_grapheme, prev_word,
    word_range,
};
pub use limits::Limits;
pub use marks::{Mark, MarkKind, Marks};
pub use ops::{Changes, Intent, Normalizer, Op, Transaction};
pub use position::{Affinity, Position, Selection};
pub use schema::{BlockSchema, ChildPolicy, ContentKind, PropDef, PropType, Registry};
pub use value::{PropValue, Props};
