// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use editor_model::{ATOM_CHAR, Document};

/// The document's text, one block per line, children indented.
pub fn to_plain_text(doc: &Document) -> String {
    let mut out = String::new();
    for id in doc.flat() {
        let Some(block) = doc.get(id) else { continue };
        if !block.content.has_text() {
            continue;
        }
        for _ in 1..doc.depth(id) {
            out.push_str("  ");
        }
        out.extend(block.content.text().chars().filter(|ch| *ch != ATOM_CHAR));
        out.push('\n');
    }
    out
}
