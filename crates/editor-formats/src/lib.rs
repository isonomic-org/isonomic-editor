// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Document formats: canonical JSON, Markdown, HTML and plain text.
//!
//! Canonical JSON (`isonomic.doc/1`) is the clean structured form:
//! a tree of blocks with formatting as data. It is deterministic, so
//! two equal documents serialize to equal bytes.

mod html;
mod json;
mod markdown;
mod markdown_in;
mod plain;

use std::fmt;

pub use html::to_html;
pub use json::{FORMAT, block_to_json, from_json, to_json, to_json_string};
pub use markdown::to_markdown;
pub use markdown_in::from_markdown;
pub use plain::to_plain_text;

/// Why input could not be read as a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// The input is not well-formed.
    Syntax(String),
    /// The input is well-formed but is not a document of a version
    /// this build reads.
    Unsupported(String),
    /// The input is larger than the import limit.
    TooLarge,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(why) => write!(f, "malformed document: {why}"),
            Self::Unsupported(what) => write!(f, "unsupported document: {what}"),
            Self::TooLarge => f.write_str("document is larger than the import limit"),
        }
    }
}

impl std::error::Error for FormatError {}
