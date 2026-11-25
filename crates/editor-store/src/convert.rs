//! Conversions between model values and replicated values.

use std::collections::{BTreeMap, HashMap};

use editor_model::{ATOM_CHAR, Atom, InlineText, MarkKind, Marks, PropValue, Props, Run};
use loro::{LoroValue, TextDelta};

pub const ATOM_KEY: &str = "atom";

pub fn to_value(value: &PropValue) -> LoroValue {
    match value {
        PropValue::Null => LoroValue::Null,
        PropValue::Bool(v) => LoroValue::from(*v),
        PropValue::Int(v) => LoroValue::from(*v),
        PropValue::Float(v) => LoroValue::from(*v),
        PropValue::Text(v) => LoroValue::from(v.as_str()),
        PropValue::List(items) => LoroValue::from(items.iter().map(to_value).collect::<Vec<_>>()),
        PropValue::Map(map) => {
            let map: HashMap<String, LoroValue> =
                map.iter().map(|(key, value)| (key.clone(), to_value(value))).collect();
            LoroValue::from(map)
        }
    }
}

pub fn from_value(value: &LoroValue) -> PropValue {
    match value {
        LoroValue::Bool(v) => PropValue::Bool(*v),
        LoroValue::I64(v) => PropValue::Int(*v),
        LoroValue::Double(v) => PropValue::Float(*v),
        LoroValue::String(v) => PropValue::Text(v.to_string()),
        LoroValue::List(items) => PropValue::List(items.iter().map(from_value).collect()),
        LoroValue::Map(map) => PropValue::Map(props(map.iter())),
        _ => PropValue::Null,
    }
}

pub fn props<'a>(entries: impl Iterator<Item = (&'a String, &'a LoroValue)>) -> Props {
    let mut out = BTreeMap::new();
    for (key, value) in entries {
        out.insert(key.clone(), from_value(value));
    }
    out
}

pub fn atom_value(atom: &Atom) -> LoroValue {
    let mut map = BTreeMap::new();
    map.insert("kind".to_owned(), PropValue::Text(atom.kind.clone()));
    map.insert("attrs".to_owned(), PropValue::Map(atom.attrs.clone()));
    to_value(&PropValue::Map(map))
}

fn atom_from(value: &LoroValue) -> Option<Atom> {
    let PropValue::Map(mut map) = from_value(value) else {
        return None;
    };
    let kind = map.remove("kind")?.as_text()?.to_owned();
    let attrs = match map.remove("attrs") {
        Some(PropValue::Map(attrs)) => attrs,
        _ => Props::new(),
    };
    Some(Atom { kind, attrs })
}

/// Rich text from the replicated text's delta.
pub fn inline_from(delta: &[TextDelta]) -> InlineText {
    let mut text = String::new();
    let mut runs = Vec::new();
    for item in delta {
        let TextDelta::Insert { insert, attributes } = item else {
            continue;
        };
        let mut marks = Marks::default();
        let mut atom = None;
        for (key, value) in attributes.iter().flatten() {
            if key == ATOM_KEY {
                atom = atom_from(value);
            } else if let Some(kind) = MarkKind::from_key(key) {
                match value {
                    LoroValue::Bool(true) if !kind.has_value() => {
                        marks.set(&editor_model::Mark::new(kind, None));
                    }
                    LoroValue::String(v) if kind.has_value() => {
                        marks.set(&editor_model::Mark::new(kind, Some(v)));
                    }
                    _ => {}
                }
            }
        }
        text.push_str(insert);
        match atom {
            Some(atom) if !insert.is_empty() && insert.chars().all(|ch| ch == ATOM_CHAR) => {
                for _ in insert.chars() {
                    runs.push(Run {
                        len: ATOM_CHAR.len_utf8(),
                        marks: marks.clone(),
                        atom: Some(atom.clone()),
                    });
                }
            }
            _ => runs.push(Run { len: insert.len(), marks, atom: None }),
        }
    }
    InlineText::from_runs(text.clone(), runs).unwrap_or_else(|_| InlineText::plain(&text))
}
