// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Turning model blocks into the component library's block nodes.

use editor_model::{Block, Content, Document, InlineText, PropValue, kinds};
use ui::base::text::editor_host::{
    BlockNode, CodeBlock, InlineNode, LinkMark, Paragraph, Span, TextMark,
};
use ui::{Hsla, SharedString};

/// What an empty text leaf shows, so it still has a line to place the
/// caret on. Zero width; not part of the document.
pub const EMPTY: &str = "\u{200B}";

fn paragraph(text: &InlineText, key: u64, highlight: Hsla) -> Paragraph {
    let mut out = Paragraph::default();
    if text.is_empty() {
        out.push(InlineNode::new(EMPTY));
        return out.with_leaf_key(key);
    }
    let marks = text
        .spans()
        .filter(|(_, run)| !run.marks.is_plain())
        .map(|(range, run)| {
            let marks = &run.marks;
            let mark = TextMark {
                bold: marks.bold,
                italic: marks.italic,
                strikethrough: marks.strike,
                underline: marks.underline,
                code: marks.code,
                highlight: marks.highlight.as_ref().map(|_| highlight),
                link: marks.link.as_ref().map(|url| LinkMark {
                    url: SharedString::from(url.clone()),
                    identifier: None,
                    title: None,
                }),
            };
            (range, mark)
        })
        .collect();
    out.push(InlineNode::new(text.text().to_owned()).marks(marks));
    out.with_leaf_key(key)
}

fn int(block: &Block, key: &str) -> Option<i64> {
    block.props.get(key).and_then(PropValue::as_int)
}

/// The node that draws one block. `number` is the block's place in a
/// run of numbered items. Blocks of unknown types draw as text.
pub fn node_for(
    doc: &Document,
    block: &Block,
    key: u64,
    number: u32,
    highlight: Hsla,
) -> BlockNode {
    let text = match &block.content {
        Content::Inline(text) => text.clone(),
        Content::Plain(text) => InlineText::plain(text),
        Content::None => InlineText::new(),
    };
    let leaf = || BlockNode::Paragraph(paragraph(&text, key, highlight));
    let item = |ordered: bool, checked: Option<bool>| BlockNode::List {
        children: vec![BlockNode::ListItem {
            children: vec![leaf()],
            spread: false,
            checked,
            task_marker: None,
            span: None,
        }],
        ordered,
        start: ordered.then_some(number),
        span: None,
    };
    if !doc.registry().is_known(&block.kind) {
        return leaf();
    }
    match block.kind.as_str() {
        kinds::HEADING => BlockNode::Heading {
            level: int(block, "level").unwrap_or(1).clamp(1, 3) as u8,
            children: paragraph(&text, key, highlight),
            span: None,
        },
        kinds::BULLETED => item(false, None),
        kinds::NUMBERED => item(true, None),
        kinds::TODO => {
            let checked = block.props.get("checked").and_then(PropValue::as_bool).unwrap_or(false);
            item(false, Some(checked))
        }
        kinds::QUOTE => BlockNode::Blockquote { children: vec![leaf()], span: None },
        kinds::DIVIDER => BlockNode::HorizontalRule { span: None },
        kinds::CODE => {
            let language = block.props.get("language").and_then(PropValue::as_text);
            let language =
                language.filter(|name| !name.is_empty()).map(|name| name.to_owned().into());
            let code = if text.is_empty() { EMPTY.to_owned() } else { text.text().to_owned() };
            BlockNode::CodeBlock(
                CodeBlock::new(code.into(), language, None::<Span>).with_leaf_key(key),
            )
        }
        _ => leaf(),
    }
}
