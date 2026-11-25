// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;

use crate::BlockId;

/// Why a model operation was refused. The document is unchanged when
/// an operation returns an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    UnknownBlock(BlockId),
    DuplicateBlock(BlockId),
    /// An index into a child list is past its end.
    BadIndex,
    /// A text range is out of bounds or not on a character boundary.
    BadRange,
    /// A block cannot be moved into itself or one of its descendants.
    Cycle,
    /// The block has no text of the kind the operation needs.
    NoText,
    /// Runs do not cover the text exactly, or an atom run is malformed.
    BadRuns,
    /// A limit from [`crate::Limits`] would be exceeded.
    Limit(&'static str),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBlock(id) => write!(f, "unknown block {id}"),
            Self::DuplicateBlock(id) => write!(f, "block {id} already exists"),
            Self::BadIndex => f.write_str("child index out of range"),
            Self::BadRange => f.write_str("text range out of bounds or off a character boundary"),
            Self::Cycle => f.write_str("a block cannot move into its own subtree"),
            Self::NoText => f.write_str("the block has no text for this operation"),
            Self::BadRuns => f.write_str("runs do not match the text"),
            Self::Limit(which) => write!(f, "limit exceeded: {which}"),
        }
    }
}

impl std::error::Error for ModelError {}
