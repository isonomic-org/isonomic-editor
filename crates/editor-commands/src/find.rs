// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding text in a document.

use std::ops::Range;

use editor_model::{BlockId, Document, next_grapheme, prev_grapheme};

/// Every place `query` occurs, in document order, ignoring case. A
/// match lies inside one block and never overlaps the one before it.
/// Its ends are grapheme boundaries, so it can be selected as it is.
pub fn find_text(doc: &Document, query: &str) -> Vec<(BlockId, Range<usize>)> {
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let mut found = Vec::new();
    if needle.is_empty() {
        return found;
    }
    for id in doc.flat() {
        let Some(block) = doc.get(id).filter(|block| block.content.has_text()) else { continue };
        let text = block.content.text();
        let mut from = 0;
        while let Some(range) = next_match(text, from, &needle) {
            from = range.end;
            found.push((id, range));
        }
    }
    found
}

fn next_match(text: &str, from: usize, needle: &[char]) -> Option<Range<usize>> {
    text.get(from..)?.char_indices().find_map(|(at, _)| {
        let start = from + at;
        let end = match_at(text, start, needle)?;
        (is_boundary(text, start) && is_boundary(text, end)).then_some(start..end)
    })
}

/// Where a match starting at `start` ends. A character whose lower
/// case is several characters has to match all of them.
fn match_at(text: &str, start: usize, needle: &[char]) -> Option<usize> {
    let mut matched = 0;
    for (at, character) in text.get(start..)?.char_indices() {
        for lower in character.to_lowercase() {
            if needle.get(matched) != Some(&lower) {
                return None;
            }
            matched += 1;
        }
        if matched == needle.len() {
            return Some(start + at + character.len_utf8());
        }
    }
    None
}

fn is_boundary(text: &str, offset: usize) -> bool {
    offset == 0
        || offset == text.len()
        || next_grapheme(text, prev_grapheme(text, offset)) == offset
}
