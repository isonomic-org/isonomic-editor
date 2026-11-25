// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A block's identity: 128 bits, persistent, opaque, never reused.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u128);

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl fmt::Debug for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockId({self})")
    }
}

impl FromStr for BlockId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        if s.is_empty() || s.len() > 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(());
        }
        u128::from_str_radix(s, 16).map(BlockId).map_err(|_| ())
    }
}

impl Serialize for BlockId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BlockId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(|()| serde::de::Error::custom("invalid block id"))
    }
}

/// A table row's identity within its table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RowId(pub u64);

/// A table column's identity within its table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ColumnId(pub u64);

/// Where new block IDs come from. Commands stay pure by taking one.
pub trait IdSource {
    fn next_id(&mut self) -> BlockId;
}

/// IDs made of a replica number and a counter. Unique as long as the
/// replica number is unique and the counter never goes backwards.
#[derive(Clone, Debug)]
pub struct SeqIds {
    replica: u64,
    next: u64,
}

impl SeqIds {
    pub fn new(replica: u64) -> Self {
        Self { replica, next: 1 }
    }

    pub fn replica(&self) -> u64 {
        self.replica
    }

    /// Makes sure no later ID collides with `seen`.
    pub fn observe(&mut self, seen: BlockId) {
        if (seen.0 >> 64) as u64 == self.replica {
            let low = seen.0 as u64;
            self.next = self.next.max(low.saturating_add(1));
        }
    }
}

impl IdSource for SeqIds {
    fn next_id(&mut self) -> BlockId {
        let id = BlockId(((self.replica as u128) << 64) | self.next as u128);
        self.next = self.next.saturating_add(1);
        id
    }
}
