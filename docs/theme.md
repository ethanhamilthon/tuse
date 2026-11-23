# Theme & UI Settings

## Theme

The config has one theme, `theme`; it is not selectable in the UI. It starts from the built-in
dark palette and overrides only the colors you write.

The theme has 10 color tokens. Colors take `#rrggbb`, `#rgb`, `0`-`255`, a name (`red`,
`dark_grey`, `bright_blue`, ...), `accent`, or `none` for the terminal default:

- `bg` (`#121212`): main background, including lists and panels.
- `surface` (`#1E1E1E`): elevated background (user messages, filled input); lists and panels do not use it.
- `fg` (`#DEDEDE`): primary text and code blocks.
- `dim` (`#BDBDBD`): secondary text (tool output, quotes, list details, notices).
- `muted` (`#929292`): subtle text (reasoning, syntax comments, inactive list items).
- `border` (`#333333`): horizontal rules, box frames, separators, `/usage` keys.
- `accent` (`#BDDEAB`): primary highlight (status, bar, list title, selected list item).
- `ok` (`#94B979`): additions and success (`diff` `+`, inline `code`, strings, `/usage` cost).
- `err` (`#E27373`): errors and deletions (`diff` `-`, failed calls, keywords).
- `info` (`#97BEDC`): tools, headings, links, functions.

```lua
return {
  theme = { accent = "#FFB86C", info = "bright_blue" },
}
```

## UI settings (`settings.ui`)

UI behavior and layout live under `settings.ui`, apart from the colors:

- `input_design`: `lines` (default), `box`, `fill` or `bar`.
- `fold_strings`: `true` (default) or `false`. Folds multiline Lua long strings (`[[...]]`) in chat
  tool calls to a single line (`[[first line…]]`). `/trace` always shows the full code.
- Lines shown in the chat (`0` hides the block, `-1` removes the limit): `tool_line` (`-1`),
  `output_line` (`1`), `diff_line` (`12`), `reasoning_line` (`-1`).
- Layout and scrolling: `pad_x` (`0`), `message_gap` (`1`), `scroll_step` (`3`),
  `list_max_height` (`16`), `input_max_height` (`-1`).

### Input design

Each design shows the input and two status bars: working (spinner, elapsed time and the current
action (`reading PATH...`, `writing PATH...`, `editing PATH...`, `searching PATTERN...`, `running CMD...`; each stays at least 100 ms), or `steering…` while a queued message waits) and the model (model, effort,
context, cost).

- `lines` (default): a rule above and below; working sits on the top rule, the model on the bottom one.
- `box`: a frame around the input with the same two labels on its borders.
- `fill`: the input has `surface` background; one status row below, working on the left and the
  model on the right.
- `bar`: an `accent` vertical bar left of the input, a blank row, then the same status row.
