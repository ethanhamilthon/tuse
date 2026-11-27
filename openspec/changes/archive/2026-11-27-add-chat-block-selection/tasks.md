## 1. Rendering
- [x] 1.1 Render `Entry::Markdown` through `markdown::rows` so its rows carry copy data; shift the copy columns by the marker and inset widths, and keep the marker out of the copied text.
- [x] 1.2 Do the same for `Entry::UiResult` (`pick` and `ask` results).
- [x] 1.3 Keep the other entries as decorations without copy data.

## 2. Selection
- [x] 2.1 Treat each of these entries as one target by its entry id, for pinned and unpinned `show()` blocks, so drags stop at the block boundary.
- [x] 2.2 Make sure a `show()` update with the same name keeps working and clears a stale selection.

## 3. Tests
- [x] 3.1 Select and copy a multi-line `show()` block: no marker, no padding, no wrap newlines.
- [x] 3.2 Drag from a `show()` block into a message stays in the block; drag from a message into a `show()` block stays in the message.
- [x] 3.3 Select and copy a `pick` result.
- [x] 3.4 Reasoning, Lua blocks, and events still do not select.
