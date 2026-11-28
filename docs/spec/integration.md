# Isonomic Editor: Formats, host, agents, testing and integration

## B12. Formats

- **Canonical JSON** (`isonomic.doc/1`): a tree of `{id, type, props, tunes, content, children}` with inline content as text plus mark ranges. Versioned, deterministic, diff-friendly. This is the clean structured output, with formatting as data.
- **Replicated snapshot**: the store's binary export, lossless with history.
- **Markdown**: import and export with a defined mapping; blocks with no Markdown form export as fenced blocks tagged with their type so they round-trip.
- **HTML**: export for publishing and clipboard; import through the allow-list.
- **Importers**: the reference product's JSON block format at M4, so existing content moves over.

## B13. Host

```rust
pub trait Host {
    fn resolve_image(&self, src: &ImageRef) -> ImageLoad;
    fn store_file(&self, file: IncomingFile) -> BoxFuture<'static, Result<FileRef>>;
    fn open_link(&self, link: &Link);
    fn search_mentions(&self, query: &str, kind: MentionKind) -> BoxFuture<'static, Vec<MentionHit>>;
    fn embed(&self, kind: &str, block: BlockId) -> Option<EmbedHandle>;
}
```

The editor never opens a URL, reads a path or fetches anything itself. A host with no capabilities gives a fully working text editor that shows placeholders.

## B14. Agents

The headless crates are the agent's editor. Tools exposed through the harness at M5:

- `doc.read`: the document or a range as Markdown or JSON, with block IDs.
- `doc.edit`: a transaction of typed operations addressed by block ID (insert after, replace text, set props, move, delete).
- `doc.stream`: open an insertion anchor and append text or blocks as tokens arrive, while the user keeps editing elsewhere.

Agent edits carry the agent's actor identity, so they are attributed and undone separately. **Suggestion mode** records an agent's edits as suggestion marks and pending blocks the user accepts or rejects. Every tool call passes the identity library's check function like any other tool.

## B17. Testing

- **Property tests.** Any transaction followed by undo restores the document. Normalization is idempotent. Random command sequences never panic and never break invariants.
- **Convergence fuzzing.** Two and three peers apply random operations, exchange updates in random order, and end identical after normalization.
- **Golden round-trips** for JSON, Markdown and HTML over `corpus/`.
- **Headless view tests** for selection, IME and keyboard using the UI framework's test context.
- **Screenshots** of the corpus, light and dark, via `cargo xtask screenshots`. Every shipped screenshot is looked at.
- **Fuzz targets** for every parser and for `import`.

## B18. Integration

**Dependency directions.** The engines never depend on one another.

| Crate | Depends on |
|---|---|
| Isonomic UI | the framework |
| Isonomic Views, Isonomic Editor, Isonomic Grid | Isonomic UI only |
| Adapters (views-backed blocks, the grid block, the view block, an editor component for views) | the engines they join |
| The window and first-party apps | everything above, through the SDK app crate |

- **Isonomic UI.** Reached only through `ui`. Requests R1 to R6 go to `isonomic-ui/docs/requests.md`. Reused as is: inputs, popovers, menus, the code highlighter, the scrollbar. Not reused: the virtual list (B9).
- **Pin parity.** This repository's Isonomic UI pin must equal the SDK's. `cargo xtask pins` compares them and CI fails on a difference, so the window never links two copies.
- **SDK.** The SDK app crate already reserves an empty `editor` module behind its `editor` feature; M4 fills it. Two requests: S1, a documents interface (B5); and a decision on where the adapters live. The SDK and the engines are all GPL-3.0-or-later since 28 Nov 2026, so the licence no longer decides this. The recommendation is still to keep adapters in the window repository, or in a GPL crate of their own, and leave the SDK's own code free of engine dependencies beyond the feature-gated re-export. Both are shared-contract changes and take the `needs-master` label.
- **Isonomic Views.** A view block embeds a view. Request V1: a host slot component, so a view can contain content the host draws (B6). Later, an editor component gives artifacts rich text.
- **Isonomic Grid.** A grid block embeds a grid over a data source the host supplies. The editor's own table block stays a simple table; it does not use the grid.
- **Engine.** Documents are stored by the Capsule (B5).
- **Formats as interface.** Since 28 Nov 2026 every open-source Isonomic repository is GPL-3.0-or-later, interface layers included. The canonical document format and the block manifest are interfaces others should be able to implement. Open question for the owner: the licence their specifications are published under, beside the views document format.

## B19. Limits

Documents can come from other people. Each limit is enforced on load, paste, import and remote update. Exceeding one degrades (truncate, placeholder, or refuse the update) and never panics.

| Limit | Default |
|---|---|
| Blocks per document | 50,000 (a cold open measured 463 ms at this size; documents this large should be split into pages) |
| Nesting depth | 32 |
| Text per block | 1 MiB |
| Props per block | 64 KiB |
| Marks per block | 10,000 |
| Table size | 10,000 rows, 200 columns |
| Third-party view per block | The views engine's own limits on nodes, depth and render time |
| Paste or import size | 16 MiB |
| Normalization work per transaction | Bounded; the rest runs at idle |

Links open only for allow-listed schemes. Images and files resolve only through `Host`.

## B20. Change feed, comments, synced blocks

- **Change feed.** Every transaction's change summary is also a stream, keyed by block ID, for engine-side consumers: the full-text index, embeddings for semantic search, backlinks, the outline, word count. No consumer re-reads the whole document on an edit.
- **Comments.** Threads are stored beside the document, not in it. A thread holds anchors to a range or a block; the editor shows it as a decoration and a margin marker. Resolving or deleting a thread never edits the text.
- **Synced blocks.** A synced block's content is its own replicated document; a page holds a reference block with its ID. Editing in one place edits all. Permissions follow the referenced document.
- **Find and replace** runs over the snapshot off the main thread, shows hits as decorations, and replaces in one transaction.
- **Outline.** Derived from heading blocks through the change feed; it feeds the table-of-contents block and a sidebar the host may show.
