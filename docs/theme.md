# Theme & UI Settings

## Theme

Themes live under `themes`, one key per theme. `extend: other` starts from another
theme (or `default`, the built-in Vercel Dark palette) and overrides only the colors you write.

A theme has 10 color tokens. Colors take `#rrggbb`, `#rgb`, `0`-`255`, a name (`red`,
`dark_grey`, `bright_blue`, ...), `accent`, or `none` for the terminal default:

- `bg` (`#0A0A0A`): main background, including lists and panels.
- `surface` (`#1A1A1A`): elevated background (user messages, filled input); lists and panels do not use it.
- `fg` (`#EDEDED`): primary text and code blocks.
- `dim` (`#A0A0A0`): secondary text (tool output, quotes, list details, notices).
- `muted` (`#676767`): subtle text (reasoning, syntax comments, motion gradient start).
- `border` (`#2E2E2E`): horizontal rules, box frames, separators, `/usage` keys, inactive list items.
- `accent` (`#EDEDED`): primary highlight (status, bar, list title, selected list item, motion gradient end).
- `ok` (`#00AC3A`): additions and success (`diff` `+`, inline `code`, strings, `/usage` cost).
- `err` (`#F32E40`): errors and deletions (`diff` `-`, failed calls, keywords).
- `info` (`#52A8FF`): tools, headings, links, functions.

## UI settings (`settings.ui`)

UI behavior and layout live under `settings.ui` so switching `/theme` keeps them intact:

- `input_design`: `lines` (default), `box`, `fill` or `bar`.
- `motion`: `true` (default) or `false`. While the agent works, a gradient from `muted` to `accent`
  runs along the rules, frame borders, filled edges or bar.
- `fold_strings`: `true` (default) or `false`. Folds multiline triple-quoted Python strings in chat
  tool calls to a single line (`"""first line…"""`). `/trace` always shows the full code.
- Lines shown in the chat (`0` hides the block, `-1` removes the limit): `tool_line` (`-1`),
  `output_line` (`1`), `diff_line` (`12`), `reasoning_line` (`-1`).
- Layout and scrolling: `pad_x` (`0`), `message_gap` (`1`), `scroll_step` (`3`),
  `list_max_height` (`16`), `input_max_height` (`-1`).

### Input design

Each design shows the input and two status bars: working (spinner, elapsed time and the current
tool call's `title`, or `steering…` while a queued message waits) and the model (model, effort,
context, cost).

- `lines` (default): a rule above and below; working sits on the top rule, the model on the bottom one.
- `box`: a frame around the input with the same two labels on its borders.
- `fill`: the input has `surface` background; one status row below, working on the left and the
  model on the right.
- `bar`: an `accent` vertical bar left of the input, a blank row, then the same status row.

`settings.ui.tab_status` (default `true`) sets the terminal tab title to `tuse · <session title>`
(OSC 2) and shows the state in the tab (OSC 9;4): working spins, a question pauses, an error is
red, done and idle clear it. Terminals without OSC 9;4 ignore it. On exit the old title comes back
(xterm title stack). In tmux, enable `set -g set-titles on` to pass the title through. Set it to
`false` to send neither.
