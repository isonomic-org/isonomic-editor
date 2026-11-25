// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

mod common;

use std::sync::Arc;

use common::Session;
use editor_blocks::standard_registry;
use editor_commands as cmd;
use editor_formats::*;
use editor_model::*;

const SAMPLE: &str = "# Title\n\nA **bold** and *italic* word, `code`, ~~gone~~ and a [link](https://example.org).\n\n- one\n  - nested \\*literal\\*\n- two\n\n1. first\n2. second\n\n- [x] done\n- [ ] open\n\n> quoted\n> lines\n\n---\n\n```rust\nfn main() {}\n\n  indented\n```\n\nlast\n";

fn registry() -> Arc<Registry> {
    Arc::new(standard_registry())
}

fn same(a: &Document, b: &Document) {
    assert_eq!(a.flat(), b.flat());
    for id in a.flat() {
        assert_eq!(a.get(id), b.get(id));
    }
}

#[test]
fn markdown_round_trips() {
    let doc = from_markdown(SAMPLE, registry(), &mut SeqIds::new(7));
    assert_eq!(to_markdown(&doc), SAMPLE);
    assert_eq!(doc.root().len(), 12);
    let heading = doc.get(doc.root()[0]).unwrap();
    assert_eq!(heading.props.get("level"), Some(&PropValue::Int(1)));
}

#[test]
fn canonical_json_round_trips_and_is_deterministic() {
    let doc = from_markdown(SAMPLE, registry(), &mut SeqIds::new(7));
    let text = to_json_string(&doc);
    let back = from_json(&text, registry(), Limits::default()).unwrap();
    same(&doc, &back);
    assert_eq!(to_json_string(&back), text);
    assert!(text.contains("\"format\": \"isonomic.doc/1\""));
}

#[test]
fn unknown_blocks_and_atoms_survive_every_format() {
    let mut doc = Document::new(registry());
    let atom =
        Atom { kind: "mention".into(), attrs: Props::from([("label".into(), "Ada".into())]) };
    let mut text = InlineText::plain("hi ");
    text.append(&InlineText::atom(atom, Marks::default()));
    let poll = BlockDraft::new(BlockId(1), "acme.poll").text(text).prop("question", "Why?");
    doc.apply(&Op::InsertBlock { parent: None, index: 0, block: poll }).unwrap();
    let child = BlockDraft::new(BlockId(2), kinds::PARAGRAPH).text(InlineText::plain("inside"));
    doc.apply(&Op::InsertBlock { parent: Some(BlockId(1)), index: 0, block: child }).unwrap();

    let back = from_json(&to_json_string(&doc), registry(), Limits::default()).unwrap();
    same(&doc, &back);
    let markdown = to_markdown(&doc);
    assert!(markdown.starts_with("```isonomic:acme.poll\n"));
    let again = from_markdown(&markdown, registry(), &mut SeqIds::new(7));
    same(&doc, &again);
}

#[test]
fn hostile_input_is_refused_or_degraded_never_a_panic() {
    let limits = Limits::default;
    assert!(matches!(from_json("{", registry(), limits()), Err(FormatError::Syntax(_))));
    assert!(matches!(
        from_json("{\"format\":\"other/9\",\"blocks\":[]}", registry(), limits()),
        Err(FormatError::Unsupported(_))
    ));
    let tiny = Limits { paste_bytes: 4, ..Limits::default() };
    assert_eq!(from_json("{\"format\":1}", registry(), tiny).unwrap_err(), FormatError::TooLarge);
    // A repeated ID and out-of-range marks: the first block wins and
    // the bad marks are dropped.
    let odd = r#"{"format":"isonomic.doc/1","blocks":[
        {"id":"1","type":"paragraph","content":{"text":"héllo","marks":[
            {"start":0,"end":99,"type":"bold"},{"start":2,"end":3,"type":"bold"},
            {"start":0,"end":1,"type":"link"},{"start":0,"end":1,"type":"nope"}]}},
        {"id":"1","type":"paragraph","content":{"text":"twin"}}]}"#;
    let doc = from_json(odd, registry(), limits()).unwrap();
    assert_eq!(doc.len(), 1);
    assert_eq!(doc.get(BlockId(1)).unwrap().content, Content::Inline(InlineText::plain("héllo")));
    for garbage in ["***", "[x](", "`", "- [ ]", "> ", "```\nopen", "1. ", "\\", "==a", "<u>b", "_"]
    {
        let doc = from_markdown(garbage, registry(), &mut SeqIds::new(1));
        let _ = (to_markdown(&doc), to_html(&doc), to_plain_text(&doc));
    }
}

#[test]
fn html_and_plain_text_export() {
    let source = "# T\n\n- a <b>\n  - [link](javascript:alert) and [ok](https://example.org)\n\n```\n1 < 2\n```\n";
    let doc = from_markdown(source, registry(), &mut SeqIds::new(7));
    let html = to_html(&doc);
    assert!(html.contains("<h1>T</h1>"));
    assert!(html.contains("<li>a &lt;b&gt;"));
    assert!(!html.contains("javascript:"));
    assert!(html.contains("<a href=\"https://example.org\">ok</a>"));
    assert!(html.contains("<pre><code>1 &lt; 2</code></pre>"));
    assert_eq!(to_plain_text(&doc), "T\na <b>\n  link and ok\n1 < 2\n");
}

#[test]
fn pasting_markdown_inserts_text_or_blocks() {
    let mut s = Session::new(1);
    s.type_text("start  end");
    let id = s.block(0);
    s.selection = Selection::caret(id, 6);
    let word = from_markdown("**big** ", registry(), &mut SeqIds::new(50));
    let doc = s.doc();
    let tx = cmd::insert_fragment(&doc, &s.selection, &word, s.store.ids());
    assert!(s.run(tx));
    assert_eq!(s.markdown(), "start **big** end\n");
    let blocks = from_markdown("- a\n  - b\n\ntail", registry(), &mut SeqIds::new(51));
    let doc = s.doc();
    let tx = cmd::insert_fragment(&doc, &s.selection, &blocks, s.store.ids());
    assert!(s.run(tx));
    assert_eq!(s.markdown(), "start **big** end\n\n- a\n  - b\n\ntail\n");
    assert_eq!(s.selection, Selection::caret(s.block(3), 4));
    assert!(s.undo());
    assert_eq!(s.markdown(), "start **big** end\n");
}
