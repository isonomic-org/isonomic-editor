//! Two replicas make random edits, exchange updates at random times,
//! and must end up with the same valid document. No input may panic.

mod common;

use common::{Session, sync};
use editor_commands as cmd;
use editor_model::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Moves the selection somewhere random but valid.
fn wander(s: &mut Session, rng: &mut Rng) {
    let doc = s.doc();
    let texts: Vec<BlockId> =
        doc.flat().into_iter().filter(|id| doc.get(*id).unwrap().content.has_text()).collect();
    let pick = |rng: &mut Rng| {
        let id = texts[rng.below(texts.len())];
        let text = doc.get(id).unwrap().content.text();
        let mut offset = rng.below(text.len() + 1);
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        Position::new(id, offset)
    };
    let head = pick(rng);
    let anchor = if rng.below(4) == 0 { pick(rng) } else { head };
    s.selection = Selection::Text { anchor, head };
}

fn act(s: &mut Session, rng: &mut Rng) {
    wander(s, rng);
    let doc = s.doc();
    let selection = s.selection.clone();
    match rng.below(14) {
        0..=4 => s.type_text(["a", "é", "- ", "# ", "👩‍👩‍👧", " ", "[] ", "---", "x"][rng.below(9)]),
        5 => drop(s.enter()),
        6 => drop(s.backspace()),
        7 => drop(s.delete()),
        8 => drop(s.tab()),
        9 => drop(s.shift_tab()),
        10 => {
            let kind = MarkKind::ALL[rng.below(5)];
            s.run(cmd::toggle_mark(&doc, &selection, kind, None));
        }
        11 => {
            let tx = if rng.below(2) == 0 {
                cmd::move_up(&doc, &selection)
            } else {
                cmd::move_down(&doc, &selection)
            };
            s.run(tx);
        }
        12 => {
            let kinds =
                [kinds::PARAGRAPH, kinds::QUOTE, kinds::CODE, kinds::DIVIDER, kinds::BULLETED];
            s.run(cmd::convert_block(&doc, &selection, kinds[rng.below(5)], &Props::new()));
        }
        _ => drop(if rng.below(2) == 0 { s.undo() } else { s.redo() }),
    }
}

fn assert_valid(doc: &Document) {
    assert!(!doc.is_empty());
    let last = doc.get(*doc.root().last().unwrap()).unwrap();
    assert!(last.content.has_text(), "a document ends in a text block");
    for id in doc.flat() {
        let block = doc.get(id).unwrap();
        if let Some(parent) = doc.parent(id) {
            let parent = doc.get(parent).unwrap();
            assert!(doc.registry().allows_child(&parent.kind, &block.kind), "child policy");
        }
        if let Content::Inline(inline) = &block.content {
            let rebuilt = InlineText::from_runs(inline.text().to_owned(), inline.runs().to_vec());
            assert_eq!(rebuilt.as_ref(), Ok(inline), "inline invariants");
        }
    }
}

#[test]
fn random_concurrent_editing_converges() {
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut a = Session::new(1);
        a.type_text("seed text");
        let mut b = Session::join(&a, 2);
        for step in 0..160 {
            if rng.below(2) == 0 {
                act(&mut a, &mut rng)
            } else {
                act(&mut b, &mut rng)
            }
            if step % 9 == 8 {
                sync(&mut a, &mut b);
            }
        }
        // Repairs after an import are edits too; a few rounds settle.
        for _ in 0..4 {
            sync(&mut a, &mut b);
        }
        let (x, y) = (a.doc(), b.doc());
        assert_eq!(a.markdown(), b.markdown(), "seed {seed}");
        assert_eq!(x.flat(), y.flat(), "seed {seed}");
        assert_valid(&x);
        let json = editor_formats::to_json_string(&x);
        let back =
            editor_formats::from_json(&json, x.registry().clone(), Limits::default()).unwrap();
        assert_eq!(editor_formats::to_json_string(&back), json);
    }
}
