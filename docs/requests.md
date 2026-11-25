# Requests to other repositories

What Isonomic Editor needs from repositories it does not own. Each is written to be filed as it stands, with what building the view taught. **None has been filed yet.**

## To Isonomic UI: the text seam

The editor draws every block through the text seam (`base::text::editor_host`) and reads each leaf's geometry back. That works today for drawing, the caret, selection, pointer placement and input methods. These are the gaps.

### R1. The visual lines of a leaf

**Need.** For each leaf, the byte range of every visual line, so Up, Down, Home and End move by line without guessing.

**Today.** The editor probes: it asks the geometry for the index nearest a point one line above or below the caret, and for Home and End at the leaf's left and right edges. It works, with one wart: at a wrapped line's end the index is also the next line's start and the caret draws there, so End rests one character short of the wrap. A list of line ranges, or a caret affinity that the geometry honours at a wrap, removes the probing and the wart.

### R2. Decoration styles beyond a background

**Need.** Wavy, dotted and solid underlines and a strike, as decorations on a byte range of a leaf, each with a color.

**Why.** Spell-check, suggestions and comments. Today a decoration is a background color only, which the editor uses for the selection.

### R3. A public constructor for a custom inline node

**Need.** To build an inline node of a custom kind from code. Runs, marks, images and atoms can already be built; the custom node type has private fields.

**Why.** Mentions, dates and inline equations.

### R4. Visual caret movement in bidirectional text

**Need.** Left and Right that move the caret visually through mixed left-to-right and right-to-left text.

**Why.** Right-to-left languages. Blocks only the last milestone.

### R5. HTML and custom flavors on the clipboard

**Need.** To write and read an HTML flavor, and a custom flavor for a typed payload.

**Today.** The clipboard carries text with metadata, images and file paths. The editor copies Markdown and pastes it back, so structure and marks survive inside the editor and in anything that takes Markdown, and nothing richer reaches or comes from other apps.

**Shared with** Isonomic Grid's request G1.

### R6. A host-owned leaf, exposed to assistive technology

**Need.** For the leaf that holds the caret: its text, the selection and the caret, exposed through the accessibility tree.

**Why.** Screen readers. Blocks only the last milestone.

### R7. Geometry for an empty leaf (new)

**Found while building the view.** A leaf with no text reports no geometry, so there is nowhere to place a caret in an empty paragraph, and a click on it finds nothing. The editor draws a zero-width space in every empty text block to get a line. That character is not part of the document, but it is one more thing every offset calculation has to know about.

**Need.** An empty leaf that still reports one line: its bounds, its line height and a caret position at index zero.

## To the SDK

### S1. A documents interface

**Need.** In the SDK's app crate, behind the `editor` feature that already reserves an empty `editor` module: a way to open a document by ID and get its snapshot bytes, to send local updates and receive remote ones as they arrive, and to write a history checkpoint. The editor's store already imports and exports snapshots and updates in the replicated format, so the interface carries opaque bytes and never needs to know what is in them.

**Also to decide.** Where the adapter lives. The SDK is Apache-2.0 and this engine is GPL-3.0-or-later, so an adapter that links the engine is GPL. Recommendation: the adapter lives where both are already linked, or in a GPL crate of its own, and the SDK's own code stays free of engine dependencies beyond the feature-gated re-export.

**Label.** A shared-contract change: `needs-master`.

## To Isonomic Views

### V1. A host slot component

**Need.** A component in the views catalog whose content the host draws, so a view can contain something the views engine does not know how to render.

**Why.** A third-party block type defined through views is props-only today: its text lives in props and is edited with the views engine's own inputs. With a host slot, such a block can declare inline content and have the editor draw and edit it, with the same undo and sync as any other block.
