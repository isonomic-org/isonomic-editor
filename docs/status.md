# Status

Newest first.

## 2026-11-25: version 0.1, the headless engine

**What exists.** Five headless crates, tested, with no GPU or window needed.

| Crate | State |
|---|---|
| `editor-model` | Block tree snapshot with shared storage (an edit copies one of 1,024 pieces, not the table), inline text with marks and atoms, positions and selections, the nine primitive operations, the schema registry, limits. |
| `editor-store` | One replicated document per page. Atomic transactions, normalization inside the transaction, local-only undo and redo with typing coalesced by intent, anchors, export and import by version. A block's text object is created with the block; properties and tunes are plain values on the block. |
| `editor-commands` | Insert text, delete (character, word, selection, blocks), split, join, indent, outdent, move, toggle mark, set link, convert, to-do, select all, paste a fragment, insert from the menu, twelve input rules, normalization. |
| `editor-formats` | Canonical JSON `isonomic.doc/1` both ways, Markdown both ways, HTML and plain text out. Blocks with no Markdown form travel as tagged fenced blocks. |
| `editor-blocks` | Paragraph, heading 1 to 3, bulleted and numbered lists, to-do, quote, divider, code; the insert menu search. |

**Tests.** 36, all hermetic: model invariants, every primitive checked against the bytes a fresh replica reads, atomic failure, undo steps, two-replica convergence, anchors across remote edits, a keyboard-only writing session, cross-block deletes, format round trips, hostile input, golden files in `corpus/`, and a seeded random test where two replicas edit concurrently and must converge to the same valid document.

**Measured** (release build, 2-core 2.1 GHz Linux machine, `cargo run --release -p editor-blocks --example bench`):

| | 10,000 blocks | 49,000 blocks |
|---|---|---|
| Keystroke: command plus transaction, a reader holding the last snapshot | 27 µs | 93 µs |
| Split a block | 0.24 ms | 0.46 ms |
| Undo (re-reads the whole document) | 65 ms | 387 ms |
| Open from a snapshot | 0.27 s | 1.86 s |

**Not done, in the order it should be done.**

1. **Nothing is drawn yet.** `editor-gpui`, `ui` and the playground do not exist. The component library was confirmed to compile on Linux at revision `834fc6a5`; the text seam has not been exercised. This is milestone 1's first task and the spec's second spike.
2. **Undo, redo and import re-read the whole document.** Correct, but 65 ms at 10,000 blocks. The fix is to refresh only the blocks the replication library reports as changed, and fall back to a full read when the tree's shape changed.
3. **Opening is slower than the spike measured** (0.27 s against 41 ms at 10,000 blocks). The spike's blocks carried less. Opening must stay off the main thread; the cause needs a profile before milestone 2.
4. **Nothing has run on macOS.** All numbers above are from Linux.
5. Limits not yet enforced: property size in bytes, table size (tables are milestone 3), normalization work per transaction.
6. HTML import, the importer for the reference format, history checkpoints and compaction, presence, and the change feed as a stream are later milestones.
7. Requests to other repositories (R1 to R6, S1, V1 in the spec) are not filed.

**Decisions made while building, to confirm.**

- Properties and tunes are plain values on the block's replicated map (keys `p:name`, `t:name`), last writer wins per key. The spec said separate objects created with the block; plain values are simpler, merge the same way, and cut open time by a third.
- Heading is one block type with a `level` property, not three types.
- IDs are the replica number in the high 64 bits and a counter in the low 64, so the headless crates need no random source. The embedder supplies a unique replica number.
- Undo coalescing counts commits per step and undoes that many, so it needs no timers.
- An input rule also fires in a list item when it turns it into another kind of list item.
