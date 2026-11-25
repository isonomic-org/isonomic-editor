//! Fixture documents with golden outputs. Set `UPDATE_GOLDEN=1` to
//! rewrite the golden files after an intended format change.

use std::path::PathBuf;
use std::sync::Arc;

use editor_blocks::standard_registry;
use editor_formats::*;
use editor_model::SeqIds;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(name)
}

fn golden(name: &str, actual: &str) {
    let path = corpus(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(actual, expected, "{name} differs; see the note at the top of this file");
}

#[test]
fn the_tour_matches_its_golden_outputs() {
    let source = std::fs::read_to_string(corpus("tour.md")).unwrap();
    let doc = from_markdown(&source, Arc::new(standard_registry()), &mut SeqIds::new(1));
    assert_eq!(to_markdown(&doc), source, "the tour is already in canonical Markdown");
    golden("tour.json", &to_json_string(&doc));
    golden("tour.html", &to_html(&doc));
    golden("tour.txt", &to_plain_text(&doc));
}
