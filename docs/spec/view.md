# Isonomic Editor: Rendering, input and chrome

## B9. Rendering

**The text seam, as it is today** (Isonomic UI, `text::editor_host`).

- **In.** One node per text block (paragraph, heading or code block) carrying a host-chosen 64-bit leaf key; marks for bold, italic, underline, strikethrough, inline code, highlight color and link; foreground color ranges per leaf; background ranges per leaf for selection and decorations; a render context (style, code highlighter, image resolver, link handler); rendering under a stable host identity, which we set to the block ID.
- **Out.** For each keyed leaf laid out in the frame: bounds, line height, caret height, byte index for a point, position for a byte index, a caret for a point and a position for a caret with soft-wrap affinity, and the inline atom under a point. Indices are UTF-8 bytes; each inline atom counts as one placeholder character.
- **Not provided,** by its own statement: visual caret navigation, pagination, accessibility, native input. Those are the host's.

**Requests to Isonomic UI,** filed at M0, all additive. Nothing is patched locally.

| | Request | Needed for |
|---|---|---|
| R1 | The visual lines of a leaf (start and end byte of each) | Up, down, Home and End without probing points |
| R2 | Decoration styles beyond a background: wavy, dotted and solid underline, strike | Spell-check, suggestions, comments |
| R3 | A public constructor for a custom inline node. Runs, marks, images and custom atoms can already be built in code; the custom node type has private fields | Mentions, dates, inline equations |
| R4 | Visual caret movement in bidirectional text | Right-to-left languages |
| R5 | HTML and custom flavors on the clipboard, read and write | Rich paste from and to other apps |
| R6 | Exposing a host-owned leaf's text, selection and caret to assistive technology | Screen readers |

Until R1 lands, vertical movement probes the geometry one line height above or below the caret. Also to confirm in Spike 2: that the seam, the scrollbar interface and the grouped undo history are re-exported through the `ui` crate.

**Flat rows.** The view flattens the visible part of the tree into rows: one row per block, with its depth. A collapsed toggle hides its descendants' rows. Nesting is indentation plus gutter decoration: a quote bar or a callout background is drawn per row and joins across consecutive rows of the same container. Two kinds of block are *islands*, a single row that lays out its own children: columns and tables. Embeds (views, grids, canvases) are single rows hosting an element.

**Row index.** The toolkit's list state holds one balanced tree over the rows with each row's height. It answers offset to row and row to offset in O(log n), updates one row in O(log n), measures only the rows it is about to show, and keeps the scroll position as a row and a distance into it, so the first visible row stays put when heights above it settle. The view tells it which rows changed: the blocks between the longest unchanged start and end of the document order. The UI library's virtual list is not used; it takes every item size up front and rebuilds its sums on any change. Unmeasured rows use one estimated height (an estimate from text length and width is not built). Which of two blocks comes first is answered from their paths in the document, not from this tree. A tree owned by the view replaces the toolkit's only if collapsed toggles or row counts between positions need it.

**Projection cache.** Each block's node is cached by (block revision, width, font metrics). A keystroke rebuilds one leaf.

**Frame contract.** Handlers change the model. Render builds the rows for the visible range plus overscan and collects their geometry. Paint only reads.

**Block chrome** (bullets, checkboxes, callout frames, toggles, handles) is ordinary Isonomic UI inside the row.

**Selection painting.** Text selections are background ranges inside leaves; block selections are a halo around rows; remote carets and selections are decorations.

**Decorations** are ephemeral ranges or block marks that never enter the document: search hits, spell-check, remote presence, ghost text from an agent, comment highlights.

**Scrolling.** The owner passes the scroll handle. One scroll handle per viewport.

## B10. Input

- **Keyboard.** Platform key bindings through the UI library's key contexts; macOS first. Word and line motion and deletion match the UI library's inputs.
- **IME.** Through the framework's input handler. Composition is confined to one leaf, is one undo step, and reports bounds from leaf geometry. UTF-16 ranges are converted at this boundary only. Tested on macOS with Japanese, Chinese and Korean input methods before M1 closes.
- **Pointer.** Click, double-click (word), triple-click (block), Shift-click, drag selection across blocks with edge auto-scroll.
- **Drag and drop.** Every block, including nested and multi-selected blocks, drags by its handle. A drop indicator shows position and depth. One undo step. Escape cancels. Files dropped from outside go to `Host`.
- **Clipboard.** The framework clipboard carries text with attached metadata, images and file paths; it has no HTML flavor. Copy writes plain text in Markdown form, with the lossless typed payload attached as metadata on that text entry (IDs are regenerated on paste). Paste reads the typed payload when present, otherwise the text, detecting Markdown. Images paste as image blocks through `Host`. Paste-as-plain-text is a separate command. HTML in and out waits on R5; pasted HTML is then parsed into blocks through an allow-list.
- **Submit policy.** A compact preset can treat Enter as submit and a modified Enter as newline, for composers and comments.

## B11. Chrome

Slash menu (filterable, grouped, extensible); block handle with drag and a block menu (convert to, tunes, duplicate, delete, copy link to block); floating format toolbar on text selection; link editor popover; placeholder text on empty blocks; table grips for rows and columns; mention and page-link pickers fed by `Host`. All of it is built from Isonomic UI components and can be replaced or hidden per preset.

**Presets.** `Document` (full page), `Compact` (comments, composers: reduced block set, submit policy, auto-grow), `ReadOnly` (selectable, no editing).

## B15. Accessibility and languages

Accessibility in this framework is carried by elements: role, name and state. Each row is an element with a role and a name (heading with its level, list item with its position, to-do with its checked state, code, quote). Block moves and conversions are announced. Exposing the focused leaf's text, selection and caret needs R6. Right-to-left and bidirectional text are laid out by the UI library; visual caret movement needs R4. Spell-check is a decoration source supplied by the host and needs R2 for its underline. Roles and focus order start at M1; full coverage is an M6 exit criterion.

## B16. Performance budgets

Budgets are for the Intel test Mac, release build. The "measured" column is the store alone on a 2-core 2.1 GHz Linux machine; it shows how much of each budget the store uses, not that the budget is met. Two rows have now been measured whole on the Intel Mac, for the editor's share before the renderer (`docs/status.md`): typing is 2.9 ms a frame at 10,000 blocks, inside its budget; opening was first timed there as a Markdown import on the main thread, 0.77 s. Opening saved bytes now reads in the background (`EditorState::load`) and is about 250 ms to the first frame on the Intel Mac, 4 ms of it on the drawing thread (185 ms and 8 ms on the Linux machine), **which is off the main thread and still over its budget**.

| Measure | Budget | Store alone, measured |
|---|---|---|
| Keystroke to frame, 5,000-block document | p95 under 8 ms | under 0.01 ms |
| Blocks re-projected per keystroke | 1 leaf (counter test) | n/a |
| Open a 10,000-block document to first frame | under 150 ms, off the main thread | 148 ms |
| Open a 50,000-block document | under 1 s, behind a skeleton | 1.3 s at 49,000 blocks, **over its budget** |
| Scroll | 60 fps, no blank frames | n/a |
| Apply a remote update of 100 operations | under 4 ms | 1.1 ms |
| Memory, 10,000 blocks, no images | under 90 MB | 21 MB after a cold open |
| Background snapshot of 10,000 blocks | under 150 ms, never on the main thread | 62 ms |

CI enforces deterministic counters (blocks projected, leaves laid out, rows measured), not timings. Timings are recorded by `cargo xtask report` and written in `docs/status.md`.
