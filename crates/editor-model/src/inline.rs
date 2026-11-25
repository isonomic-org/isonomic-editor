// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

use crate::{Mark, MarkKind, Marks, ModelError, Props};

/// The placeholder character an inline atom occupies in the text.
pub const ATOM_CHAR: char = '\u{FFFC}';

/// An inline object, such as a mention or a date. It occupies one
/// placeholder character and carries its own attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct Atom {
    pub kind: String,
    pub attrs: Props,
}

/// A stretch of text with one set of marks, or one atom.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub len: usize,
    pub marks: Marks,
    pub atom: Option<Atom>,
}

/// Rich text: flat UTF-8 text plus runs that cover it exactly.
///
/// Invariants: no empty run, no two adjacent runs with equal marks
/// (atoms excepted), one run per atom.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineText {
    text: String,
    runs: Vec<Run>,
}

impl InlineText {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plain(text: &str) -> Self {
        Self::styled(text, Marks::default())
    }

    pub fn styled(text: &str, marks: Marks) -> Self {
        let mut out = Self::default();
        if !text.is_empty() {
            out.text.push_str(text);
            out.runs.push(Run { len: text.len(), marks, atom: None });
        }
        out
    }

    pub fn atom(atom: Atom, marks: Marks) -> Self {
        Self {
            text: ATOM_CHAR.to_string(),
            runs: vec![Run { len: ATOM_CHAR.len_utf8(), marks, atom: Some(atom) }],
        }
    }

    /// Builds rich text from parts, checking that the runs fit.
    pub fn from_runs(text: String, runs: Vec<Run>) -> Result<Self, ModelError> {
        let mut at = 0;
        for run in &runs {
            let end = at + run.len;
            if end > text.len() || !text.is_char_boundary(end) {
                return Err(ModelError::BadRuns);
            }
            if run.atom.is_some() && !text[at..end].chars().eq([ATOM_CHAR]) {
                return Err(ModelError::BadRuns);
            }
            at = end;
        }
        if at != text.len() {
            return Err(ModelError::BadRuns);
        }
        let mut out = Self { text, runs };
        out.normalize();
        Ok(out)
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Runs with the byte range each one covers.
    pub fn spans(&self) -> impl Iterator<Item = (Range<usize>, &Run)> {
        let mut at = 0;
        self.runs.iter().map(move |run| {
            let range = at..at + run.len;
            at = range.end;
            (range, run)
        })
    }

    pub fn append(&mut self, other: &InlineText) {
        self.text.push_str(&other.text);
        self.runs.extend(other.runs.iter().cloned());
        self.normalize();
    }

    /// A copy of the given byte range. Out-of-range input is clamped.
    pub fn slice(&self, range: Range<usize>) -> InlineText {
        let end = floor_boundary(&self.text, range.end.min(self.text.len()));
        let start = floor_boundary(&self.text, range.start.min(end));
        let mut copy = self.clone();
        let from = copy.split_at(start);
        let to = copy.split_at(end);
        InlineText { text: self.text[start..end].to_owned(), runs: copy.runs[from..to].to_vec() }
    }

    /// Replaces a byte range with other rich text.
    pub fn replace(&mut self, range: Range<usize>, with: &InlineText) -> Result<(), ModelError> {
        self.check(&range)?;
        let from = self.split_at(range.start);
        let to = self.split_at(range.end);
        self.runs.splice(from..to, with.runs.iter().cloned());
        self.text.replace_range(range, &with.text);
        self.normalize();
        Ok(())
    }

    pub fn add_mark(&mut self, range: Range<usize>, mark: &Mark) -> Result<(), ModelError> {
        self.edit_marks(range, |marks| marks.set(mark))
    }

    pub fn remove_mark(&mut self, range: Range<usize>, kind: MarkKind) -> Result<(), ModelError> {
        self.edit_marks(range, |marks| marks.clear(kind))
    }

    /// The marks text typed at `offset` should take: those of the run
    /// the caret is inside, or the expanding marks of the run it
    /// follows.
    pub fn marks_at(&self, offset: usize) -> Marks {
        for (range, run) in self.spans() {
            if offset > range.start && offset < range.end {
                return run.marks.clone();
            }
            if offset == range.end && offset > 0 {
                return run.marks.expanding();
            }
        }
        Marks::default()
    }

    /// Whether every character in a non-empty range has the mark.
    pub fn range_has(&self, range: Range<usize>, kind: MarkKind) -> bool {
        if range.start >= range.end {
            return false;
        }
        self.spans()
            .filter(|(span, _)| span.start < range.end && span.end > range.start)
            .all(|(_, run)| run.marks.has(kind))
    }

    pub fn is_boundary(&self, offset: usize) -> bool {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return false;
        }
        GraphemeCursor::new(offset, self.text.len(), true)
            .is_boundary(&self.text, 0)
            .unwrap_or(false)
    }

    fn check(&self, range: &Range<usize>) -> Result<(), ModelError> {
        let ok = range.start <= range.end
            && range.end <= self.text.len()
            && self.text.is_char_boundary(range.start)
            && self.text.is_char_boundary(range.end);
        if ok { Ok(()) } else { Err(ModelError::BadRange) }
    }

    fn edit_marks(
        &mut self,
        range: Range<usize>,
        edit: impl Fn(&mut Marks),
    ) -> Result<(), ModelError> {
        self.check(&range)?;
        let from = self.split_at(range.start);
        let to = self.split_at(range.end);
        for run in &mut self.runs[from..to] {
            edit(&mut run.marks);
        }
        self.normalize();
        Ok(())
    }

    /// Makes a run boundary at `offset`; returns the index of the run
    /// that starts there.
    fn split_at(&mut self, offset: usize) -> usize {
        let mut at = 0;
        for index in 0..self.runs.len() {
            if at == offset {
                return index;
            }
            let len = self.runs[index].len;
            if offset < at + len {
                let mut right = self.runs[index].clone();
                right.len = at + len - offset;
                self.runs[index].len = offset - at;
                self.runs.insert(index + 1, right);
                return index + 1;
            }
            at += len;
        }
        self.runs.len()
    }

    fn normalize(&mut self) {
        let mut out: Vec<Run> = Vec::with_capacity(self.runs.len());
        for run in self.runs.drain(..) {
            if run.len == 0 {
                continue;
            }
            if let Some(last) = out.last_mut()
                && last.atom.is_none()
                && run.atom.is_none()
                && last.marks == run.marks
            {
                last.len += run.len;
                continue;
            }
            out.push(run);
        }
        self.runs = out;
    }
}

fn floor_boundary(text: &str, mut offset: usize) -> usize {
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The grapheme boundary before `offset`, or 0.
pub fn prev_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset.min(text.len()));
    GraphemeCursor::new(offset, text.len(), true).prev_boundary(text, 0).ok().flatten().unwrap_or(0)
}

/// The grapheme boundary after `offset`, or the end.
pub fn next_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset.min(text.len()));
    GraphemeCursor::new(offset, text.len(), true)
        .next_boundary(text, 0)
        .ok()
        .flatten()
        .unwrap_or(text.len())
}

/// The start of the word before `offset`, or 0.
pub fn prev_word(text: &str, offset: usize) -> usize {
    text.unicode_word_indices()
        .map(|(start, _)| start)
        .take_while(|start| *start < offset)
        .last()
        .unwrap_or(0)
}

/// The end of the word after `offset`, or the end of the text.
pub fn next_word(text: &str, offset: usize) -> usize {
    text.unicode_word_indices()
        .map(|(start, word)| start + word.len())
        .find(|end| *end > offset)
        .unwrap_or(text.len())
}

/// The word that contains `offset` or ends at it, or else the run of
/// white space and punctuation between words there.
pub fn word_range(text: &str, offset: usize) -> Range<usize> {
    let offset = offset.min(text.len());
    let mut after_last = 0;
    for (start, word) in text.unicode_word_indices() {
        let end = start + word.len();
        if offset < start {
            return after_last..start;
        }
        if offset <= end {
            return start..end;
        }
        after_last = end;
    }
    after_last..text.len()
}
