# Keys and pointer

"Command" below is Command on macOS and Control elsewhere. "Word key" is Option on macOS and Control elsewhere.

## Moving and selecting

Add Shift to any of these to select as it moves.

| Key | Does |
|---|---|
| Left, Right | One character; at a block's edge, into the next block |
| Up, Down | One line as it is drawn; from the last line, into the next block |
| Home, End | Start or end of the line as it is drawn |
| Word key with Left or Right | Start of the word before, or end of the word after |
| Command with Up or Down (macOS); Control with Home or End (elsewhere) | Start or end of the document |
| Command with A | Select the block's text; pressed again, the block, and then more each time up to everything |

## Editing

| Key | Does |
|---|---|
| Enter | Split the block; in an empty list item, lift it out, then turn it into a paragraph |
| Backspace, Delete | Remove a character; at a block's start, Backspace unwraps the block before it joins the one above |
| Word key with Backspace or Delete | Remove a word (Option does this everywhere) |
| Tab, Shift with Tab | Indent or outdent the block |
| Command with B, I, U | Bold, italic, underline |
| Command with Shift and X | Strike |
| Command with E | Code |
| Command with C, X, V | Copy, cut and paste; what is copied carries its structure and marks as Markdown |
| Command with Z, Shift with Command and Z | Undo and redo |

## Typed at the start of a paragraph

| Type | Makes |
|---|---|
| `# `, `## `, `### ` | A heading of level 1, 2 or 3 |
| `- ` or `* ` | A bulleted item |
| `1. ` | A numbered item |
| `[] ` or `[ ] `, `[x] ` | A to-do, unticked or ticked |
| `> ` | A quote |
| ` ``` ` | A code block |
| `---` | A divider |

## The insert menu

Type `/` at the start of a block of text, or after a space. Keep typing to narrow the list.

| Key | Does |
|---|---|
| Up, Down | Step through the list, around the ends |
| Enter, or a click on an entry | Turn the block into that type |
| Escape | Close the menu and keep what was typed |

## Find

| Key | What it does |
|---|---|
| Command-F (Control-F off macOS) | Open the search bar, looking for the selected text if the selection is inside one block |
| Typing in the bar | Look for the text, ignoring case, and select the first match at or after the selection |
| Enter, Command-G | Go to the next match, round to the first after the last |
| Shift-Enter, Command-Shift-G | Go to the match before |
| Escape | Close the bar; the last match stays selected |

Every match is shaded while the bar is open, and the bar says which one is selected and how many there are. Matches are looked for again after each edit.

## Pointer

| Gesture | Does |
|---|---|
| Click | Place the caret; with Shift, extend the selection to there |
| Drag | Select; held above or below the text, scroll that way |
| Double click | Select the word |
| Triple click | Select the block's text |
| Click a to-do's checkbox | Tick or untick it |
| Wheel, scrollbar | Scroll |
