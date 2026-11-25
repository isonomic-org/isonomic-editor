// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use crate::{BlockKind, PropValue, kinds};

/// What kind of text a block carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentKind {
    /// Rich text: marks and inline atoms.
    Inline,
    /// Unformatted text, as in a code block.
    Plain,
    /// No text of its own.
    None,
}

/// Which children a block may hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChildPolicy {
    None,
    Any,
    Only(Vec<BlockKind>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropType {
    Bool,
    Int,
    Float,
    Text,
    List,
    Map,
    Any,
}

/// One declared property: its name, type and default.
#[derive(Clone, Debug, PartialEq)]
pub struct PropDef {
    pub name: String,
    pub ty: PropType,
    pub default: PropValue,
}

impl PropDef {
    pub fn new(name: &str, ty: PropType, default: PropValue) -> Self {
        Self { name: name.to_owned(), ty, default }
    }
}

/// A block type, as data.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockSchema {
    pub kind: BlockKind,
    pub content: ContentKind,
    pub children: ChildPolicy,
    pub props: Vec<PropDef>,
    pub version: u32,
}

impl BlockSchema {
    pub fn new(kind: &str, content: ContentKind, children: ChildPolicy) -> Self {
        Self { kind: BlockKind::new(kind), content, children, props: Vec::new(), version: 1 }
    }

    pub fn prop(mut self, def: PropDef) -> Self {
        self.props.push(def);
        self
    }
}

/// The known block types. A type missing from the registry is kept
/// intact and treated as rich text that accepts any child.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    blocks: HashMap<BlockKind, BlockSchema>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, schema: BlockSchema) {
        self.blocks.insert(schema.kind.clone(), schema);
    }

    pub fn get(&self, kind: &BlockKind) -> Option<&BlockSchema> {
        self.blocks.get(kind)
    }

    pub fn is_known(&self, kind: &BlockKind) -> bool {
        self.blocks.contains_key(kind)
    }

    pub fn content(&self, kind: &BlockKind) -> ContentKind {
        self.get(kind).map_or(ContentKind::Inline, |schema| schema.content)
    }

    pub fn allows_child(&self, parent: &BlockKind, child: &BlockKind) -> bool {
        match self.get(parent).map(|schema| &schema.children) {
            None | Some(ChildPolicy::Any) => true,
            Some(ChildPolicy::None) => false,
            Some(ChildPolicy::Only(allowed)) => allowed.contains(child),
        }
    }

    /// The declared default for a property, if the type declares it.
    pub fn default_prop(&self, kind: &BlockKind, name: &str) -> Option<&PropValue> {
        let schema = self.get(kind)?;
        schema.props.iter().find(|def| def.name == name).map(|def| &def.default)
    }

    /// The type new empty text blocks take.
    pub fn default_kind(&self) -> BlockKind {
        BlockKind::new(kinds::PARAGRAPH)
    }
}
