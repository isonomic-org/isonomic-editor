# Status

Newest first.

## 2026-11-26: keys for whole blocks

- Option-Shift-Up and Option-Shift-Down move the selected blocks, with what is nested in them, among their siblings. Command-Option-0 to 8 turn them into text, a heading of level 1 to 3, a to-do, a bulleted or numbered item, a quote or code (`docs/keys.md`). The commands were already in the headless half; the view now answers them, as actions a host can also put in a menu.
- A test moves a block to both ends and a nested list past its neighbor, undoes, and takes one block through every type and back to text with its text and caret kept. Both backends.

## 2026-11-26: links

- Command-K opens a bar that asks where the selected text should point. With a caret it takes the link the caret is in, showing its address, or else the word under the caret. Enter sets the link, an empty address removes it, Escape leaves things as they were, and the change is one undo step (`docs/keys.md`).
- **A plain click on a link used to open it,** whatever it pointed at: the text seam opens links itself unless it is told otherwise. A click now only places the caret. Command-click opens the link, and only if it is a web or mail address; a bare domain is opened as a web address, and `file:`, `javascript:` and the like are not opened at all. `editor_gpui::openable` is the rule.
- Tests: the bar links a word, changes the address, cancels, links a selection, undoes and unlinks; a click without the modifier opens nothing, with it opens a web address and not a script address. Both backends.

Not built: showing a link's address when the pointer rests on it, and pasting an address over a selection to link it.

## 2026-11-26: find

- Command-F opens a search bar over the top right of the text. Typing looks for the text in every block, ignoring case, selects the first match at or after the selection and brings it into view; Enter and Command-G go on, Shift-Enter and Command-Shift-G go back, round at the ends; Escape closes it. The bar shows "2 of 5", and every match is shaded while it is open (`docs/keys.md`).
- The search itself is a pure function, `editor_commands::find_text`. A match stays inside one block, matches do not overlap, and both ends are grapheme boundaries, so a letter is not matched apart from a combining mark that follows it.
- The bar sits beside the text in the view, not inside it, so keys typed in it are not also the editor's. A test types, steps, presses the editor's own shortcuts in the bar and checks the document is untouched; another edits with the search open and sees the matches counted again. Both run on both backends.
- Tried in a real window on Linux: the bar opens, counts, and the match is selected and in view. Not tried on the Mac.

Not built: replace, whole-word and exact-case options.

## 2026-11-26: opening a saved document, off the drawing thread

- `EditorState::save` gives the document and its history as bytes. `EditorState::open` reads them back on the calling thread, and `EditorState::load` reads them in the background and hands the state over when it is whole, so a window that is already showing does not stop while a long document opens.
- Empty, truncated, corrupted and unrelated bytes are refused with an error by both.
- `frames` now measures what the budget is about, opening saved bytes. It had been timing a Markdown import, which is slower and a different thing. On the two-core Linux machine, 10,000 blocks, three runs:

| | time | budget |
|---|---|---|
| Open saved bytes to the first frame | 185 ms: 177 ms reading in the background, 8 ms drawing | under 150 ms, off the main thread |
| Read the same document from Markdown and save it | 0.56 to 0.60 s | none |

**Opening no longer holds the drawing thread: it takes one 8 ms frame of it. It is still over its budget, by about a quarter on this machine.** Nearly all of the reading is the replication library decoding two stored objects for each block the first time they are read (about 105 ms), and building the tree's order (about 45 ms); the editor's own part of it is a few milliseconds. `docs/spec/view.md` had "41 ms" for the store alone and "463 ms" for 50,000 blocks; the store's `bench` gave 168 ms and 1.7 s today.
- The store now reads the blocks' stored objects in the order the library keeps them and puts the snapshot together in document order afterwards. A long document read in document order made the library decode the same compressed stretches repeatedly. `bench` after the change: 148 ms at 10,000 blocks and 1.3 s at 49,000, about half of the second being the library building the order of a tree with that many siblings. The table in the specification has these numbers, and marks the 50,000-block budget (under 1 s) as missed.

What would close the gap is showing the first screen from the blocks at the top while the rest are read. It is not built. Not measured on the Mac.

## 2026-11-26: builds on the optional faster backend

- `editor-gpui` has a `fast` feature that builds the view on the component library's optional faster backend. It is off by default, and a host turns it on for its whole build or not at all, so the grid has the same feature.
- All 61 tests pass on both backends. The shared CI job builds with every feature, which is the faster backend; the headless job builds the default one, so every pull request is checked on both.
- The lock file holds the faster backend's packages at the version the component library was built against. Later versions add a blanket implementation the library does not compile with at its pinned revision.
- `frames` on the two-core Linux machine, 10,000 blocks: typing is 2.7 ms a frame on either backend, and moving the caret 2.9 ms on the default and 2.6 ms on the faster one. The editor draws about twenty rows a frame, so there is little for a faster painter to save. Not measured on the Mac.

Earlier entries: [`status-2026-11-25.md`](status-2026-11-25.md).
