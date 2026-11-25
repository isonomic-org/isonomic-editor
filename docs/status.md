# Status

Newest first.

## 2026-11-25: the pointer, word and document keys, a blinking caret

- A double click selects a word (or the gap between words), a triple click the block's text. A click on a to-do's checkbox ticks or unticks it.
- A drag follows the pointer anywhere in the window. Held above or below the rows it scrolls that way, further the further out the pointer is, and the selection grows with every frame.
- Option with the arrows on macOS, Control elsewhere, moves and selects by word and crosses into the next block at an edge. Command with Up and Down on macOS, Control with Home and End elsewhere, go to the document's start and end.
- The caret blinks at the interval the theme gives, and any change to the selection or the text shows it at once. A theme that asks for a steady caret gets one.
- Movement through the document's own order moved out of the state file into `moves.rs`.
- The insert menu opens above the caret when there is no room for it below, and the view reports where it was drawn.
- Checked by hand in a real window on Linux (virtual display, real pointer and key events): ticking a to-do, a double click on a word, the insert menu typed and confirmed at the bottom of the window.

## 2026-11-25: the insert menu

- Typing `/` at the start of a block of text or after white space opens a menu of block types under the caret. What is typed next filters it (title starts, then keyword starts, then anywhere in the title); Up and Down step through and wrap; Enter or a click applies the choice; Escape, moving the caret away, deleting the trigger or typing past any match closes it.
- Choosing removes the trigger and the query and converts the block, as one undo step. A divider takes the block's place when the block is otherwise empty and goes in after it when it is not.
- The rules are headless in `editor-commands` (`menu_trigger`, `menu_query`, `choose_menu_entry`) and tested there; the view only draws the list. `docs/images/editor-menu.png` is a rendered frame.

## 2026-11-25: only the rows in view are drawn

- The view lists every block in document order in the toolkit's list and builds elements only for the rows the list is about to show. Rows not yet measured get an estimated height, so the scrollbar spans the whole document from the first frame. The standard scrollbar sits over the rows.
- Bringing the selection's head into view works for a row that is not drawn: the list is scrolled to it first and the caret is lined up once it has geometry. It now follows the head of a range selection too, not only a caret.
- Up and Down from a block with no drawn neighbour that way, or with the head out of view, step to the neighbouring block in document order.
- Numbered items are numbered from their run of siblings, worked out once per frame for the rows drawn.
- **Specification changed to match the code:** the row index is the toolkit's list state, not a tree owned by the view (`docs/spec/view.md`, B9).
- `cargo run --release -p editor-gpui --example frames` times the editor's share of a frame (snapshot, rows, layout, painting into a scene; not the renderer).

| 2-core 2.1 GHz Linux machine, 900 by 1200 window | before | now |
|---|---|---|
| Typing, one character a frame | 128 ms at 2,000 blocks | 3.1 ms at 10,000 blocks |
| Moving the caret, one step a frame | 133 ms at 2,000 blocks | 3.4 ms at 10,000 blocks |
| Open and draw the first frame, 10,000 blocks | not measured | 0.6 s |

Tested in a headless window with 5,000 numbered items: a screenful of rows is drawn; walking the caret 120 rows down keeps it in the window; the wheel moves the rows by exactly what it asks; an edit and its undo far from the top leave the rest alone.

## 2026-11-25: the editor draws and takes input

`ui` re-exports the component library at one pinned revision. `editor-gpui` adds:

- `EditorState`: one document, its selection, pending marks and composition range; every key press becomes a command's transaction.
- `EditorView`: draws the tier 1 blocks through the component library's text seam, one keyed leaf per block; paints the selection behind the text and the caret from the leaf geometry; places and drags the selection with the pointer; moves by grapheme, by block and by visual line; binds the editing keys; implements platform text input, composition included, counting UTF-16 units only at that boundary.

Tested in a headless window with real layout and real key dispatch: a document typed from the keyboard, geometry for every text block, pointer placement and shift-click, arrow keys across blocks, composition replacing in place.

**Not yet, and known rough edges.**

1. It has been rendered offscreen with the real renderer on Linux (software Vulkan); the frames are in `docs/images/`, made by the `screenshot` example. The `playground` example opens it in a window; it was run on Linux under a virtual display with software rendering and driven with real pointer and key events (click, type, shortcuts, wheel). On macOS (Intel, macOS 15) every test passes and the `screenshot` example renders its frame with the native renderer; the window itself has not been opened there, and nobody has used it by hand yet.
2. Only the rows in view are drawn; see the entry above. Each frame still walks the whole document once to list its blocks in order.
3. The view scrolls, brings the selection's head into view when it moves, and scrolls while a drag selects past the top or bottom. Up or Down into a block that is not drawn lands at that block's edge, not under the column the caret was in.
4. An empty text block draws a zero-width space so it has a line for the caret. Inline atoms draw as a placeholder character.
5. A composition is not yet one undo step. The caret's blink timer runs while the editor is not focused, where it only costs a repaint.
6. Copy, cut and paste work and carry Markdown; there is no typed clipboard payload yet. The insert menu works; it is not grouped and does not scroll, which a longer list of block types will need. No block handles, toolbar or drag and drop.
7. Home and End move to the edges of the visual line. At a wrapped line's end the caret rests one character short of the wrap.

## 2026-11-25: undo, redo and import refresh only what changed

- The store listens to the replicated document while an undo, redo or import runs, and refreshes only the blocks it reports. A change to the tree's shape still re-reads the whole document.
- Opening reads the bytes before anything listens to the document.
- A new test asserts that text, mark, property and type changes from outside a transaction cause no whole-document read.

| | 10,000 blocks | 49,000 blocks |
|---|---|---|
| Undo a run of 2,000 keystrokes | 27 ms | 25 ms |
| Undo a split (shape change) | 76 ms | 382 ms |
| Open from a snapshot | 0.22 s | 1.76 s |

Still open: shape changes should be applied from the reported tree changes instead of a full read, and opening spends about 70% of its time inside the replication library's import. The spike measured 41 ms for 10,000 blocks built in batches of 1,000 per commit with no text styles declared; which of those differences matters has not been measured.

## 2026-11-25: licence

The repository is licensed under GPL-3.0-or-later: LICENSE at the root, the licence field in every manifest, and the copyright and SPDX lines at the top of every Rust source file. SECURITY.md says how to report a vulnerability. No behaviour changed.

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
2. ~~Undo, redo and import re-read the whole document.~~ Done for everything but shape changes; see the entry above.
3. **Opening is slower than the spike measured** (0.27 s against 41 ms at 10,000 blocks). The spike's blocks carried less. Opening must stay off the main thread; the cause needs a profile before milestone 2.
4. **macOS runs the tests and renders a frame; nothing has been measured there.** All numbers above are from Linux.
5. Limits not yet enforced: property size in bytes, table size (tables are milestone 3), normalization work per transaction.
6. HTML import, the importer for the reference format, history checkpoints and compaction, presence, and the change feed as a stream are later milestones.
7. Requests to other repositories (R1 to R6, S1, V1 in the spec) are not filed.

**Decisions made while building, to confirm.**

- Properties and tunes are plain values on the block's replicated map (keys `p:name`, `t:name`), last writer wins per key. The spec said separate objects created with the block; plain values are simpler, merge the same way, and cut open time by a third.
- Heading is one block type with a `level` property, not three types.
- IDs are the replica number in the high 64 bits and a counter in the low 64, so the headless crates need no random source. The embedder supplies a unique replica number.
- Undo coalescing counts commits per step and undoes that many, so it needs no timers.
- An input rule also fires in a list item when it turns it into another kind of list item.
