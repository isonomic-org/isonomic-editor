use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{BlockId, InlineText, Props};

/// Names of the standard block types.
pub mod kinds {
    pub const PARAGRAPH: &str = "paragraph";
    pub const HEADING: &str = "heading";
    pub const BULLETED: &str = "bulleted_list";
    pub const NUMBERED: &str = "numbered_list";
    pub const TODO: &str = "todo";
    pub const QUOTE: &str = "quote";
    pub const DIVIDER: &str = "divider";
    pub const CODE: &str = "code";
}

/// A block type's name, such as `paragraph` or `acme.kanban`.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockKind(Arc<str>);

impl BlockKind {
    pub fn new(name: &str) -> Self {
        Self(Arc::from(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is(&self, name: &str) -> bool {
        &*self.0 == name
    }
}

impl fmt::Debug for BlockKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockKind({})", self.0)
    }
}

impl fmt::Display for BlockKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for BlockKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for BlockKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::new(&String::deserialize(d)?))
    }
}

/// A block's own text.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Inline(InlineText),
    Plain(String),
    None,
}

impl Content {
    /// The text as a plain string; empty when the block has none.
    pub fn text(&self) -> &str {
        match self {
            Self::Inline(inline) => inline.text(),
            Self::Plain(text) => text,
            Self::None => "",
        }
    }

    pub fn len(&self) -> usize {
        self.text().len()
    }

    pub fn is_empty(&self) -> bool {
        self.text().is_empty()
    }

    pub fn has_text(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// The text as rich text; plain text comes back unmarked.
    pub fn to_inline(&self) -> InlineText {
        match self {
            Self::Inline(inline) => inline.clone(),
            Self::Plain(text) => InlineText::plain(text),
            Self::None => InlineText::new(),
        }
    }
}

/// One block of a document snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
    pub props: Props,
    pub tunes: Props,
    pub content: Content,
    pub children: Vec<BlockId>,
}

/// A block to insert. Its text is always given as rich text; the
/// block type decides how it is kept.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockDraft {
    pub id: BlockId,
    pub kind: BlockKind,
    pub props: Props,
    pub tunes: Props,
    pub text: InlineText,
}

impl BlockDraft {
    pub fn new(id: BlockId, kind: &str) -> Self {
        Self {
            id,
            kind: BlockKind::new(kind),
            props: Props::new(),
            tunes: Props::new(),
            text: InlineText::new(),
        }
    }

    pub fn text(mut self, text: InlineText) -> Self {
        self.text = text;
        self
    }

    pub fn prop(mut self, key: &str, value: impl Into<crate::PropValue>) -> Self {
        self.props.insert(key.to_owned(), value.into());
        self
    }
}
