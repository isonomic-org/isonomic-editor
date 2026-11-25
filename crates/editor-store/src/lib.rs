//! The replicated document.
//!
//! A [`Store`] owns one replicated document per page and keeps an
//! [`editor_model::Document`] snapshot beside it. All change goes
//! through [`Store::apply`]; undo is local-only; `export` and `import`
//! move bytes that any transport can carry.
//!
//! This is the only crate that names the replication library.

mod anchor;
mod convert;
mod read;
mod store;
mod write;

pub use anchor::Anchor;
pub use editor_model::Normalizer;
pub use store::{Store, StoreError, Undone};
