# Isonomic Editor: Milestones and risks

## B21. Milestones

| | Scope | Exit criteria |
|---|---|---|
| **M0 Foundations** | Workspace, CI, rules, xtask (deps, pins). **Spike 1:** repeat the store measurements on an Intel Mac. **Spike 2:** paragraph, heading and list rows drawn through the seam with caret, cross-block selection and IME on macOS. Requests filed: R1 to R6 (Isonomic UI), S1 (SDK), V1 (Isonomic Views). | Spike 1 numbers on the Mac are within twice the Linux numbers, or the fallback is chosen. Spike 2 passes. Counter harness exists. Results in `docs/status.md`. |
| **M1 Core editing** | Tier 1 blocks, marks, cross-block selection, undo, input rules, keymap, clipboard (typed payload and Markdown), JSON and Markdown, limits. | A 2,000-word document can be written with the keyboard alone. Property and convergence tests green. IME verified on macOS. |
| **M2 Block UX** | Slash menu and other typeahead triggers, handles, any-block drag and drop, block menu with tunes and conversion, format toolbar, links, callout, toggle, flat-row virtualization, find and replace. | Budgets in B16 met at 10,000 blocks. |
| **M3 Rich blocks** | Table (ordered rows and columns, keyed mergeable cells), image, file, bookmark, columns, equation, mention, date, table of contents, view block, grid block. | Each block has stories, screenshots, round-trip fixtures. Table convergence fuzzing green. |
| **M4 Extension API** | Freeze the five registries. Fill the SDK's `editor` module. Third-party blocks through Isonomic Views. Authoring guide. Importer for the reference product's format. | A first-party app adds a native block and a third-party manifest adds a declared block, both using only the SDK. API marked additive-only. |
| **M5 Agents and collaboration** | Engine replica through the documents interface (S1), document tools, streaming, suggestion mode, comments, synced blocks, presence, change feed, version history. | Two windows and one agent edit one document and converge. |
| **M6 Hardening** | Accessibility, bidirectional text, fuzzing, performance pass on Intel and Apple Silicon. | Screen-reader pass on macOS. No open crash from fuzzing. |

M1 and the first half of M2 are the "urgent" part: a working editor others can build on.

## B22. Risks

| Risk | Mitigation |
|---|---|
| The store behaves differently on an Intel Mac | The Linux run passed every check; the Mac run repeats it. Fallback plan behind the same transaction API; only `editor-store` changes. |
| Very large documents open slowly | Limit of 50,000 blocks, opening off the main thread, trimmed-history snapshots, and guidance to split into pages. |
| The documents interface (S1) is late | The window-only replica works through M4; agents and multi-device need S1. |
| A seam request (R1 to R6) is slow to land | Each has a stated interim: point probing for R1, background-only decorations for R2, plain and Markdown paste for R5. R3 blocks mentions (M3), R4 and R6 block only M6. |
| Third-party blocks through views are too limited | Props-only until V1; the store adapter gives them undo and sync for free; first-party apps keep the native path. |
| Normalization fights between peers | Deterministic, idempotent rules; convergence fuzzing from M1. |
| IME regressions on macOS | Manual script run on an Intel Mac at every milestone; one undo step per composition by construction. |
