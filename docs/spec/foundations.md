# Isonomic Editor: Scope, rules and layout

## B1. Scope

**In scope.** A reusable engine: document model, store, commands, formats, standard blocks, and a GPUI view with its chrome (slash menu, block handles, format toolbar).

**Out of scope.** The Docs app (pages, sidebar, sharing). Code editing beyond a code block (that is the code editor in Isonomic UI). Network transport (the SDK carries sync bytes). The grid, canvas and views engines, which embed as blocks.

## B2. Rules

1. **No panic on any input.** Every document, paste, remote update and keystroke yields a valid document.
2. **Block IDs are stable and never reused.** They survive reload, sync, copy within a document, and conversion between types.
3. **Headless crates stay headless.** No UI framework, network, file or process access. `cargo xtask deps` enforces an allow-list. They must build for WebAssembly.
4. **Only `ui` names the component library; only `editor-store` names the replication library.**
5. **One model, one selection, one undo history per document view.** No block owns private editable state.
6. **The view never mutates during paint.** Handlers change the model, render builds an immutable frame, paint reads it.
7. **Tokens only.** No raw colors, sizes or durations.
8. **The host denies by default.** Images, files, links and lookups go through `Host`.
9. **Hermetic tests.** No network, no real timers.
10. **Small files.** Every file under 20 KB.
11. **Names.** No other product, repository or account name anywhere in the repository except `THIRD_PARTY_NOTICES.md`.
12. **A block's replicated sub-objects are created with the block,** in the same operation. Where creation is lazy by nature (table cells), the object is created as a mergeable one. Nothing else is created lazily.
13. **Third-party code never runs natively.** A third-party block is data plus a view.
14. **Every limit in B19 is enforced** on load, paste, import and remote update.
16. **Opening, saving and searching a document never run on the main thread.**

## B3. Repository layout

| Crate | Role | Depends on |
|---|---|---|
| `editor-model` | IDs, block tree snapshot, inline text, marks, positions, selections, schema registry. | nothing here |
| `editor-store` | The replicated document: transactions, undo, anchors, history, export/import, presence. | model |
| `editor-commands` | Editing commands, input rules, normalization. Pure functions from (snapshot, selection) to a transaction. | model |
| `editor-formats` | Canonical JSON, Markdown, HTML, plain text, clipboard payloads, importers. | model |
| `editor-blocks` | Headless specs for the standard block set. | model, commands, formats |
| `editor-gpui` | `EditorState` entity, `EditorView` element, IME, keymap, chrome, drag and drop, virtualization, block views. | all above, ui |
| `ui` | Re-export of Isonomic UI. | isonomic-ui |
| `editor-playground` | Demo app and stories. | editor-gpui |
| `xtask` | deps check, screenshots, performance report, fixture checks. | |

`default-members` are the headless crates and `xtask`, so `cargo test` on Linux needs no GPU. `corpus/` holds fixture documents with golden outputs.
