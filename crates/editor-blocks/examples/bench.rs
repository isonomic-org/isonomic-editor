// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rough timings for the headless path. Run with
//! `cargo run --release -p editor-blocks --example bench -- 10000`.

use std::sync::Arc;
use std::time::Instant;

use editor_blocks::{normalizer, standard_registry};
use editor_commands as cmd;
use editor_model::*;
use editor_store::Store;

fn main() {
    let blocks: usize = std::env::args().nth(1).and_then(|n| n.parse().ok()).unwrap_or(10_000);
    let registry = Arc::new(standard_registry());
    let mut store = Store::new(registry.clone(), 1);
    store.set_normalizer(normalizer());

    let started = Instant::now();
    let mut tx = Transaction::new(Intent::Command);
    let mut ids = Vec::with_capacity(blocks);
    for index in 0..blocks {
        let id = store.ids().next_id();
        ids.push(id);
        let text = InlineText::plain("The quick brown fox jumps over the lazy dog.");
        let block = BlockDraft::new(id, kinds::PARAGRAPH).text(text);
        tx.ops.push(Op::InsertBlock { parent: None, index, block });
    }
    store.apply(&tx, None).expect("build");
    println!("build {blocks} blocks: {:?}", started.elapsed());

    let target = ids[blocks / 2];
    let keys = 2_000;
    let mut selection = Selection::caret(target, 0);
    let started = Instant::now();
    for _ in 0..keys {
        let doc = store.snapshot();
        let tx = cmd::insert_text(&doc, &selection, "x", None).expect("command");
        store.apply(&tx, Some(&selection)).expect("apply");
        selection = tx.selection.expect("caret");
    }
    println!("keystroke, a reader holding the last snapshot: {:?} each", started.elapsed() / keys);

    let doc = store.snapshot();
    let started = Instant::now();
    let tx = cmd::split_block(&doc, &selection, store.ids()).expect("split");
    store.apply(&tx, Some(&selection)).expect("apply");
    println!("split a block: {:?}", started.elapsed());

    let started = Instant::now();
    store.undo().expect("undo");
    println!("undo a split (re-reads the document): {:?}", started.elapsed());

    let started = Instant::now();
    store.undo().expect("undo");
    println!("undo a run of typing (refreshes one block): {:?}", started.elapsed());
    store.redo().expect("redo");

    let started = Instant::now();
    let bytes = store.export_snapshot().expect("export");
    println!("export snapshot ({} KiB): {:?}", bytes.len() / 1024, started.elapsed());

    let started = Instant::now();
    let reopened = Store::open(registry, 2, &bytes).expect("open");
    println!("open: {:?} ({} blocks)", started.elapsed(), reopened.snapshot().len());

    let started = Instant::now();
    let json = editor_formats::to_json_string(&store.snapshot());
    let markdown = editor_formats::to_markdown(&store.snapshot());
    println!(
        "to JSON ({} KiB) and Markdown ({} KiB): {:?}",
        json.len() / 1024,
        markdown.len() / 1024,
        started.elapsed()
    );
}
