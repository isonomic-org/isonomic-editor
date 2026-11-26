// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Saving a document as bytes and opening it again.
//!
//! Reading a long document back takes many frames' worth of time, so
//! a host that is already drawing opens with [`EditorState::load`],
//! which reads on another thread and hands the state over when it is
//! whole.

use std::sync::Arc;

use editor_blocks::{normalizer, standard_registry};
use editor_store::{Store, StoreError};
use ui::{App, AppContext as _, Entity, Task};

use crate::EditorState;

impl EditorState {
    /// The document and its history as bytes, for [`EditorState::open`]
    /// and [`EditorState::load`] to read back.
    pub fn save(&self) -> Result<Vec<u8>, StoreError> {
        self.store.export_snapshot()
    }

    /// Opens a saved document on the calling thread. `replica` must be
    /// unique among the replicas that will ever edit it.
    pub fn open(bytes: &[u8], replica: u64) -> Result<Self, StoreError> {
        Ok(Self::over(read(bytes, replica)?))
    }

    /// Opens a saved document without holding up the thread that draws:
    /// the bytes are read in the background, and the state is made once
    /// they are.
    pub fn load(
        bytes: Vec<u8>,
        replica: u64,
        cx: &mut App,
    ) -> Task<Result<Entity<Self>, StoreError>> {
        let reading = cx.background_spawn(async move { read(&bytes, replica) });
        cx.spawn(async move |cx| {
            let store = reading.await?;
            Ok(cx.new(|_| Self::over(store)))
        })
    }
}

fn read(bytes: &[u8], replica: u64) -> Result<Store, StoreError> {
    let mut store = Store::open(Arc::new(standard_registry()), replica, bytes)?;
    store.set_normalizer(normalizer());
    Ok(store)
}
