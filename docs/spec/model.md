# Isonomic Editor: Model, store, extensions and commands

## B4. Document model

A document is a tree of blocks under a root.

```rust
pub struct Block {
    pub id: BlockId,            // 128-bit, persistent, opaque
    pub kind: BlockKind,        // interned name: "paragraph", "acme.kanban"
    pub props: Props,           // typed key-value, per the block's schema
    pub tunes: Tunes,           // cross-cutting settings, keyed by tune name
    pub content: Content,       // Inline(InlineText) | Plain(Text) | None
    pub children: Vec<BlockId>, // ordered
}
pub struct InlineText { /* text + mark runs + inline atoms */ }
```

- **Content kinds.** `Inline` is rich text. `Plain` is unformatted text (code). `None` is a block with no text of its own (divider, image, embed).
- **Children.** Each block type declares a child policy: none, any, or a fixed set of types. Nesting is always children; there are no list containers. A list item is a block; a nested item is its child. Numbering is computed at render.
- **Inline text.** Flat UTF-8 text plus mark ranges. Inline atoms (mention, inline equation, date, page link) occupy one placeholder character and carry attributes. Invariants: no empty or equal-adjacent runs, one run per atom, runs cover the text exactly.
- **Marks.** Bold, italic, underline, strikethrough, inline code, link, highlight, text color at M1. Comment anchors and suggestion marks at M5. Each mark declares whether typing at its trailing edge extends it (bold does, link and code do not) and which marks it excludes.
- **Tables.** A table block holds an ordered list of row IDs, an ordered list of column IDs, column settings (width, alignment, header), and a map of cells keyed by row ID and column ID. A cell is inline text, created as a mergeable object the first time anyone types in it. A missing cell is an empty cell. Inserting, moving or deleting a row or column edits one list; concurrent row and column edits merge with no repair step.
- **Snapshots.** Readers get an immutable, cheaply cloned snapshot with a revision per block. An edit bumps revisions only on the path from the edited block to the root.
- **Prop migration.** A block type's prop schema is versioned. Migrations are pure and deterministic and run when props are read; they are never written back on load, so opening an old document on two machines produces no edits.

## B5. Store, transactions, undo, sync

- **Transactions.** All change goes through `store.transact(origin, |tx| ...)`. A transaction is atomic, ends with the normalize pass (B8), bumps revisions, and emits one change summary (blocks added, removed, moved, changed; text ranges changed).
- **Primitive operations** (the whole set): insert block, remove block, move block, set kind, set prop or tune, replace text range, add or remove mark range. Commands compose these.
- **Origins and actors.** Every transaction records its actor (the user, a named agent, a remote peer) and an origin tag (typing, paste, command, agent, remote). Attribution and per-actor undo come from this.
- **Undo.** Local-only: undo reverts this actor's transactions and leaves others' edits in place. Coalescing is by intent (a typing run, one IME composition, one paste, one drag are one step each).
- **Anchors.** A position that must outlive an edit (selection ends, comment ranges, remote carets, an agent's insertion point) is an anchor resolved through the store, never a raw offset.
- **History.** Version checkpoints, checkout of an earlier version read-only, and diff between two versions.
- **Sync.** `export(since) -> bytes` and `import(bytes)`. The SDK moves the bytes.
- **Remote changes** arrive as the same change summary, so the view handles local and remote edits with one code path.
- **One replicated document per page.** Cross-block transactions, moves and a single undo history need one document. A synced block is its own replicated document, referenced by ID (B20).
- **Sub-objects are created with the block.** A block's text object is created in the operation that creates the block; two peers creating the same child object concurrently would otherwise keep only one. Properties and tunes are not objects: they are plain values on the block's own map, under prefixed keys, last writer wins per key. That merges the same way and keeps one nested object per block.
- **Inline atoms** are one placeholder character under a non-expanding mark that carries the atom's attributes.
- **Index units.** Our offsets are UTF-8 bytes. The library edits and marks in UTF-8 bytes directly; its cursors and mark removal take Unicode indices. Conversion happens inside `editor-store` and nowhere else.
- **Presence** (carets, selections, names) travels as SDK events that carry anchors. The store does not depend on the library's presence support.
- **Compaction.** Old history is trimmed at checkpoints with a shallow snapshot. A trimmed document can no longer merge edits older than the cut, so the engine compacts only when every known replica is past it.
- **Wire.** SDK frames are capped at 1 MiB. Updates larger than a frame are chunked; full snapshots move through the blob interface.
**Replicas.**

| Replica | Where | Role |
|---|---|---|
| Engine replica | The Capsule | The record. It persists the update log and snapshots, serves other devices, and is the replica agents edit. |
| Window replica | The app showing the document | What the editor renders and the user types into. It syncs with the engine replica. |
| Agent actor | In the engine | Each agent session writes as its own actor, so its edits are attributed and undone separately. |

A peer identity is one (device, process, actor). The window and the engine exchange updates by version: on connect each side sends what the other lacks, then streams. **The SDK has no interface for this yet.** Its events are delivered at most once, which is not enough alone. Request S1 asks the SDK and engine for a documents interface: open, a subscription of updates with catch-up by version, push, and snapshots through blobs. Until S1 lands (it is needed by M5), the window holds the only replica and saves through blobs: appended updates per transaction, and a full snapshot written in the background.

## B6. Extension API

**Five extension points.**

| Extension point | What it adds | Examples |
|---|---|---|
| **Block type** | A new kind of block, its props, its slash-menu entry, its conversions. | Callout, kanban embed, AI block |
| **Mark / inline atom** | Inline formatting or an inline object, with its toolbar entry and shortcut. | Highlight, mention, inline equation |
| **Tune** | A setting any block can carry, shown in the block menu. | Alignment, background color, indent, anchor |
| **Input rule** | Text typed at a position that triggers a transaction. Each firing is its own undo step. | `# ` to heading, `[] ` to to-do, `---` to divider |
| **Typeahead trigger** | A character that opens a filtered menu at the caret and inserts the choice. | `/` commands, `@` mentions, `[[` page links, `:` emoji |

Commands, key bindings, slash-menu items and paste handlers register the same way.

**Three tiers of author.**

| Tier | Who | Spec | View | Native code |
|---|---|---|---|---|
| Built-in | This repository | Rust `BlockSpec` | Rust `BlockView` | Yes |
| First-party | An Isonomic app crate compiled into the window | Rust `BlockSpec` | Rust `BlockView` | Yes: trusted code with a least-privilege token |
| Third-party | Any other app | A manifest: name, content kind, child policy, prop schema, Markdown mapping, all as data | An Isonomic View bound to the block's props | Never |

```rust
pub trait BlockSpec: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn content(&self) -> ContentKind;
    fn children(&self) -> ChildPolicy;
    fn props(&self) -> &PropSchema;                      // typed, defaulted, versioned
    fn normalize(&self, block: BlockRef<'_>, tx: &mut Tx) {}
    fn convert_from(&self, other: BlockRef<'_>) -> Option<BlockDraft> { None }
    fn to_markdown(&self, block: BlockRef<'_>, out: &mut MarkdownWriter);
}

// editor-gpui; built-in and first-party only
pub trait BlockView: 'static {
    fn render(&self, cx: &mut BlockCx<'_>) -> AnyElement;
    fn menu(&self, cx: &BlockCx<'_>) -> Vec<MenuItem> { Vec::new() }
}
```

A third-party block is declared, not coded:

```json
{
  "block": "acme.poll",
  "content": "none",
  "children": "none",
  "props": {
    "question": { "type": "text", "default": "" },
    "options":  { "type": "list<text>", "default": [] },
    "closed":   { "type": "bool", "default": false }
  },
  "markdown": "fenced",
  "view": "poll.view"
}
```

How it is wired, checked against the views specification:

- **Props are the view's shared state.** Isonomic Views keeps saved state behind a store trait meant for a collaborative back end. The adapter implements that trait over the block's props, so a view action that sets `shared.closed` is a block transaction, with undo and sync.
- **Read-only context** (the document title, the current user's name) reaches the view as host data. Everything else the host denies.
- **No inline text yet.** The views catalog is a fixed registry with no slot for content drawn by a host, so a third-party block is props-only: its text lives in props and is edited with the views engine's own inputs. Request V1 asks Isonomic Views for a host slot; when it lands, a third-party block may declare inline content.
- **Where the adapter lives.** This repository does not depend on the views engine. The editor exposes a view-factory interface, and the adapter is registered where both engines are already linked (B18).

Unknown block types in a loaded document are kept intact and shown as a generic placeholder block; nothing is dropped.

**Standard blocks.**

| Tier | Blocks | Milestone |
|---|---|---|
| 1 | Paragraph, heading (one type with a level of 1 to 3), bulleted list, numbered list, to-do, quote, divider, code | M1 |
| 2 | Callout, toggle, toggle heading, table, image, file, bookmark, columns, equation, table of contents, page link, mention, date | M2 to M3 |
| 3 | View block (Isonomic Views), grid block (Isonomic Grid), canvas block, synced block, button, template, AI block | M3 to M5 |

## B7. Positions and selection

```rust
pub struct Position { pub block: BlockId, pub offset: usize, pub affinity: Affinity }
pub enum Selection {
    Text { anchor: Position, head: Position },   // may span blocks
    Blocks(Vec<BlockId>),                         // whole blocks
    Cells { table: BlockId, rows: Vec<RowId>, columns: Vec<ColumnId> },   // by ID, survives reordering
}
```

Offsets are UTF-8 bytes on grapheme boundaries. Conversion to UTF-16 happens only at the IME boundary. Invalid positions return an error value. Selection ends are held as anchors, so remote edits move them correctly. Escape lifts a text selection to a block selection; arrow keys and Shift extend block selections.
**Islands.** A text selection never ends inside a table or a column layout it did not start in. Dragging or extending across an island selects the island whole, as a block. Inside a table, dragging across cells becomes a cell selection.

**Order.** Comparing two positions (which comes first) and counting rows between them are answered by the view's row index (B9), not by walking the tree.

## B8. Commands and normalization

Commands are pure: `fn(&Snapshot, &Selection, args) -> Option<Transaction>`. The set at M1: insert text, delete (character, word, line, selection), split block, join with previous or next, indent, outdent, move up or down, toggle mark, set link, convert block, toggle to-do, select all (text, then block, then document), paste.

**Normalization** runs at the end of every transaction and after every remote import. It is deterministic and a no-op on a valid document, so two peers never fight. Rules: the document has at least one block; the last block is a text block; child policies hold (an illegal child is lifted out, not deleted); a table's cell keys that name a deleted row or column are ignored, not removed; inline invariants hold; a block whose type is unknown is left untouched.

**Leaving containers.** Enter or Backspace in an empty nested block lifts it one level; in an empty heading, quote or list item at the top level it becomes a paragraph.
