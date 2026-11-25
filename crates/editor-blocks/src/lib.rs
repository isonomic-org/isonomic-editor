// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The standard block set, as headless specs.
//!
//! A [`BlockSpec`] declares a block type: its schema and its entries
//! in the insert menu. How a block is drawn is the view crate's
//! concern and is not described here.

use editor_model::{
    BlockSchema, ChildPolicy, ContentKind, Normalizer, PropDef, PropType, PropValue, Props,
    Registry, kinds,
};

/// One way to insert a block from a menu.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    pub title: &'static str,
    pub keywords: &'static [&'static str],
    pub kind: &'static str,
    pub props: Props,
}

/// A block type.
pub trait BlockSpec: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn schema(&self) -> BlockSchema;
    fn menu(&self) -> Vec<MenuEntry> {
        Vec::new()
    }
}

struct Standard {
    name: &'static str,
    content: ContentKind,
    children: ChildPolicy,
    props: Vec<PropDef>,
    menu: Vec<MenuEntry>,
}

impl BlockSpec for Standard {
    fn name(&self) -> &'static str {
        self.name
    }

    fn schema(&self) -> BlockSchema {
        let mut schema = BlockSchema::new(self.name, self.content, self.children.clone());
        schema.props = self.props.clone();
        schema
    }

    fn menu(&self) -> Vec<MenuEntry> {
        self.menu.clone()
    }
}

fn entry(title: &'static str, keywords: &'static [&'static str], kind: &'static str) -> MenuEntry {
    MenuEntry { title, keywords, kind, props: Props::new() }
}

fn spec(
    name: &'static str,
    content: ContentKind,
    children: ChildPolicy,
    menu: Vec<MenuEntry>,
) -> Standard {
    Standard { name, content, children, props: Vec::new(), menu }
}

/// The tier 1 blocks: paragraph, headings, lists, to-do, quote,
/// divider and code.
pub fn standard() -> Vec<Box<dyn BlockSpec>> {
    use ChildPolicy::{Any, None as NoChildren};
    use ContentKind::{Inline, Plain};
    let heading = |level: i64, title: &'static str| MenuEntry {
        title,
        keywords: &["heading", "title", "h"],
        kind: kinds::HEADING,
        props: Props::from([("level".to_owned(), PropValue::Int(level))]),
    };
    let mut headings = spec(
        kinds::HEADING,
        Inline,
        NoChildren,
        vec![heading(1, "Heading 1"), heading(2, "Heading 2"), heading(3, "Heading 3")],
    );
    headings.props = vec![PropDef::new("level", PropType::Int, PropValue::Int(1))];
    let mut todo = spec(
        kinds::TODO,
        Inline,
        Any,
        vec![entry("To-do", &["todo", "task", "check"], kinds::TODO)],
    );
    todo.props = vec![PropDef::new("checked", PropType::Bool, PropValue::Bool(false))];
    let mut code = spec(
        kinds::CODE,
        Plain,
        NoChildren,
        vec![entry("Code", &["code", "snippet"], kinds::CODE)],
    );
    code.props = vec![PropDef::new("language", PropType::Text, PropValue::Text(String::new()))];
    let specs = vec![
        spec(
            kinds::PARAGRAPH,
            Inline,
            Any,
            vec![entry("Text", &["paragraph", "plain"], kinds::PARAGRAPH)],
        ),
        headings,
        spec(
            kinds::BULLETED,
            Inline,
            Any,
            vec![entry("Bulleted list", &["bullet", "ul"], kinds::BULLETED)],
        ),
        spec(
            kinds::NUMBERED,
            Inline,
            Any,
            vec![entry("Numbered list", &["number", "ol"], kinds::NUMBERED)],
        ),
        todo,
        spec(kinds::QUOTE, Inline, Any, vec![entry("Quote", &["quote", "cite"], kinds::QUOTE)]),
        spec(
            kinds::DIVIDER,
            ContentKind::None,
            NoChildren,
            vec![entry("Divider", &["divider", "rule", "line"], kinds::DIVIDER)],
        ),
        code,
    ];
    specs.into_iter().map(|spec| Box::new(spec) as Box<dyn BlockSpec>).collect()
}

/// A registry holding the given specs.
pub fn registry_of(specs: &[Box<dyn BlockSpec>]) -> Registry {
    let mut registry = Registry::new();
    for spec in specs {
        registry.register(spec.schema());
    }
    registry
}

/// A registry holding the standard blocks.
pub fn standard_registry() -> Registry {
    registry_of(&standard())
}

/// Menu entries that match what was typed after the trigger, best
/// first: title prefixes, then keyword prefixes, then substrings.
pub fn search_menu(specs: &[Box<dyn BlockSpec>], query: &str) -> Vec<MenuEntry> {
    let query = query.trim().to_lowercase();
    let mut ranked: Vec<(u8, usize, MenuEntry)> = Vec::new();
    let entries = specs.iter().flat_map(|spec| spec.menu());
    for (order, entry) in entries.enumerate() {
        let title = entry.title.to_lowercase();
        let rank = if query.is_empty() || title.starts_with(&query) {
            0
        } else if entry.keywords.iter().any(|word| word.starts_with(&query)) {
            1
        } else if title.contains(&query) {
            2
        } else {
            continue;
        };
        ranked.push((rank, order, entry));
    }
    ranked.sort_by_key(|(rank, order, _)| (*rank, *order));
    ranked.into_iter().map(|(_, _, entry)| entry).collect()
}

/// The normalization pass, in the shape a store takes.
pub fn normalizer() -> Normalizer {
    Box::new(editor_commands::normalize)
}
