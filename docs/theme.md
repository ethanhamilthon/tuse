# Theme & UI Settings

## Theme

The config has one theme, `theme`; it is not selectable in the UI. It starts from the built-in
dark palette and overrides only the colors you write.

The theme has 10 color tokens. Colors take `#rrggbb`, `#rgb`, `0`-`255`, a name (`red`,
`dark_grey`, `bright_blue`, ...), `accent`, or `none` for the terminal default:

- `bg` (`#121212`): main background, including lists and panels.
- `surface` (`#1E1E1E`): elevated background (user messages, shown as a block without `›`); lists and panels do not use it.
- `fg` (`#DEDEDE`): primary text and code blocks.
- `dim` (`#BDBDBD`): secondary text (quotes, list details, notices, the scroll mark).
- `muted` (`#929292`): subtle text (tool output, reasoning, syntax comments, inactive list items, status bars).
- `border` (`#333333`): horizontal rules, box frames, separators, `/session` keys, cut marks (`…`, `… N more lines`).
- `accent` (`#BDDEAB`): primary highlight (spinner and time, used context, flash notices, bar, picked `#`/`@`/image tokens in the input, selected list item).
- `ok` (`#94B979`): additions and success (`diff` `+`, inline `code`, strings, `/session` cost).
- `err` (`#E27373`): errors and deletions (`diff` `-`, failed calls, keywords).
- `info` (`#97BEDC`): tools, headings, links, functions.

```lua
return {
  theme = { accent = "#FFB86C", info = "bright_blue" },
}
```

## UI settings (`settings.ui`)

UI behavior and layout live under `settings.ui`, apart from the colors:

- `fold_strings`: `true` (default) or `false`. Folds multiline Lua long strings (`[[...]]`) in chat
  tool calls to a single line (`[[first line…]]`). `/trace` always shows the full code.
- Lines shown in the chat (`0` hides the block, `-1` removes the limit): `tool_line` (`-1`),
  `output_line` (`1`), `diff_line` (`12`), `reasoning_line` (`-1`).
- Layout and scrolling: `pad_x` (`0`), `message_gap` (`1`), `scroll_step` (`3`),
  `list_max_height` (`16`), `input_max_height` (`-1`).

### Input design

The input is framed by two rules with status labels: working (spinner, elapsed time and the current
action (`reading PATH...`, `writing PATH...`, `editing PATH...`, `searching PATTERN...`, `running CMD...`; each stays at least 100 ms), or `steering…` while a queued message waits) and the model (model, effort,
context, cost).

Colors of the two bars: the spinner and time are `accent`, the action is `muted`. In the model
bar everything is `muted` except the used context (`x` of `x/y`), which is `accent`. A notice
flashed in the working place (a copy, an error) is `accent`; the scroll mark `↑N` is `dim`.
Each of the two labels takes at most half the width and is cut with `…`.
