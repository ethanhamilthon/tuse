## Why
Text from `show()` blocks and from `pick` and `ask` results in the chat cannot be selected or copied. Scripts use `show()` to print tables, summaries, and links, and people need to copy them. The spec lists these blocks as not selectable, so this is a rule to change, not a bug.

## What Changes
- A `show()` block, pinned or not, and a `pick` or `ask` result in the chat each become one selection target, like a single user or agent message.
- A drag that starts in such a block stops at its boundary, and a drag that starts elsewhere does not add text from it.
- Copy returns the displayed text only. The marker icon on the left, the padding, and the layout whitespace are not copied.
- Reasoning, Lua tool blocks, event entries, built-in pickers, and layout decorations stay not selectable.

## Impact
- Specs: `tui`.
- Implementation: `Entry::Markdown` and `Entry::UiResult` rendering in `crates/tui/src/kit/rich/render.rs` and `crates/tui/src/ui/chat.rs` produce rows with copy data and the same column shift as messages; `ui_markdown` uses `markdown::rows`.
- Tests: one selection and copy test for a `show()` block and one for a `pick` result.
