# Isonomic Editor: working rules

Read `docs/spec.md` before changing behavior, and `docs/status.md` for where things stand.

## Rules

1. **No panic on any input.** Every document, paste, remote update and keystroke yields a valid document. Fallible calls return an error value.
2. **Block IDs are stable and never reused.**
3. **Headless crates stay headless.** `editor-model`, `editor-store`, `editor-commands`, `editor-formats` and `editor-blocks` name no UI framework and touch no network, file or process. `cargo xtask deps` enforces the allow-list.
4. **Only `ui` names the component library; only `editor-store` names the replication library.**
5. **All change goes through `Store::apply`.** Commands are pure functions that return a transaction. Nothing else mutates a document.
6. **A block's replicated sub-objects are created with the block,** in the same operation. Nothing is created lazily.
7. **Offsets are UTF-8 bytes on grapheme boundaries.** Conversion to other units happens inside `editor-store` and, later, at the IME boundary.
8. **Normalization is deterministic and a no-op on a valid document.**
9. **Limits are enforced on load, paste, import and remote update.** Exceeding one degrades; it never panics.
10. **Hermetic tests.** No network, no real timers, no randomness without a fixed seed.
11. **Small files.** Every file under 20 KB. `cargo xtask sizes` checks.
12. **Tokens only** in view code: no raw colors, sizes or durations.
13. **Names.** No other product, repository or account name anywhere in this repository except dependency manifests and `THIRD_PARTY_NOTICES.md`.
14. **The engines do not depend on each other.** Adapters between the editor and the grid, views or canvas live where both are already linked.

## Workflow

- One task per branch and pull request. Keep pull requests small enough to review.
- Before pushing: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo xtask deps`, `cargo xtask sizes`.
- A change in behavior comes with a test. A change in a format comes with updated golden files (`UPDATE_GOLDEN=1 cargo test -p editor-blocks --test corpus`).
- Update `docs/status.md` (newest first) and `docs/PROGRESS.md` in the same pull request.
- When code and specification disagree, fix one of them in the same pull request and say which.
- Changes to shared contracts (the SDK's wire and app-crate modules, the session event schema, the organization's shared configuration) are requests to those repositories, not edits made from here.
