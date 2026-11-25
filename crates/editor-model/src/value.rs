use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A block property or tune value. Plain data only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<PropValue>),
    Map(BTreeMap<String, PropValue>),
}

impl PropValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(v) => Some(v),
            _ => None,
        }
    }
}

impl From<bool> for PropValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for PropValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<&str> for PropValue {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

impl From<String> for PropValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

/// Properties keyed by name. Ordered, so output is deterministic.
pub type Props = BTreeMap<String, PropValue>;
