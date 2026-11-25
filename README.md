# Isonomic Editor

The block editor engine for Isonomic apps: a document model, a replicated store, editing commands, formats, the standard blocks, and (next) the editor view.

A document is a tree of blocks. Each block has a stable ID, a type, properties, and rich text made of UTF-8 text plus mark ranges. Every edit is a transaction of nine primitive operations, so undo, sync, agents and the view all share one path.

## Layout

| Crate | Role |
|---|---|
| `editor-model` | IDs, the block tree snapshot, inline text, marks, positions, selections, operations, the schema registry, limits. |
| `editor-store` | The replicated document: atomic transactions, local-only undo, anchors, export and import. |
| `editor-commands` | Editing commands, input rules and normalization, as pure functions. |
| `editor-formats` | Canonical JSON (`isonomic.doc/1`), Markdown, HTML and plain text. |
| `editor-blocks` | Specs for the standard blocks and the insert menu. |
| `xtask` | Repository checks. |

The view crate (`editor-gpui`), the `ui` re-export and the playground arrive with milestone 1; see `docs/status.md`.

## Use

```rust
use std::sync::Arc;
use editor_blocks::{normalizer, standard_registry};
use editor_commands as cmd;
use editor_model::*;
use editor_store::Store;

let mut store = Store::new(Arc::new(standard_registry()), replica_id);
store.set_normalizer(normalizer());

// A command reads a snapshot and returns a transaction, or `None`.
let doc = store.snapshot();
if let Some(tx) = cmd::insert_text(&doc, &selection, "Hello", None) {
    store.apply(&tx, Some(&selection))?;
}

// Sync is bytes in, bytes out; any transport can carry them.
let update = store.export_since(&their_version)?;
other.import(&update)?;
```

## Develop

```sh
cargo test            # headless crates; no GPU needed
cargo clippy --all-targets -- -D warnings
cargo xtask deps      # headless crates stay headless
cargo xtask sizes     # every file under 20 KB
```

The specification is in `docs/spec.md`. Working rules are in `CLAUDE.md`. Current state is in `docs/status.md`.
