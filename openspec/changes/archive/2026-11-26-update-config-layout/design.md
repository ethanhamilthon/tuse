## Final shape

```lua
return {
  ui = {
    lines = { tool = -1, output = 1, diff = 12, reasoning = -1, strings = 1, input = 10, list = 16 },
    input = "boxed",
    space = { padding = 0, steps = 1, messages = 1 },
    theme = { accent = "#FFB86C", code = { keyword = "accent" } },
  },
  history = { hide = { "usage" } },
  open = { md = function(path) end },
}
```

Every key is optional. Unknown keys and wrong types fail with a path such as `ui.lines.diff: expected an integer >= -1`.

## Lines

`-1` is unlimited and `0` hides the item, for `tool`, `output`, `diff`, `reasoning`. `strings` is the number of lines of a multiline Lua string shown in a tool call: `0` shows only `…`, `N` shows the first N lines followed by `…` when more exist, `-1` shows all. Default 1 matches the old `fold_strings = true`. `input` is `-1` or `>= 1`, default 10. `list` is `>= 1`, default 16.

## Space

`padding` is the side padding in columns, default 0. `steps` is the blank rows between tool calls and reasoning blocks, default 1. `messages` is the blank rows above and below user and agent messages, default 1.

## Theme

| Token | Default | Used for |
| --- | --- | --- |
| `bg` | `#121212` | Background. |
| `accent` | `#BDDEAB` | Selection, atoms, accents. |
| `fg` | `#DEDEDE` | Agent messages. |
| `user` | `fg` | User message text. |
| `user_bg` | `#1E1E1E` | User message background. |
| `muted` | `#929292` | Secondary text, notices, tool output, reasoning, status bar. |
| `input` | `user` | Text in the input. |
| `input_bg` | `#333333` | Input background or frame. |
| `border` | `#333333` | Other borders: pickers, panels, rules, diff headers. |
| `icons` | `accent` | Prompt marker, spinner, star, bar mark. |
| `error` | `#E27373` | Error text. |
| `code` | table | Lua tokens and diff lines. |
| `markdown` | table | Markdown markup. |

`code` tokens: the fifteen existing Lua tokens (`keyword`, `storage`, `constant`, `operator`, `punctuation`, `number`, `string`, `escape`, `comment`, `function`, `builtin`, `method`, `field`, `parameter`, `variable`) plus `added` and `removed` for diff lines. Literal defaults replace the old links to `err`, `ok`, `info`, `dim`: `keyword` and `storage` `#E27373`, `string` and `added` `#94B979`, `escape`, `function`, `builtin`, `method` `#97BEDC`, `operator` and `parameter` `#BDBDBD`, `removed` `#E27373`; the rest keep their links to `accent`, `muted`, or `none`.

`markdown` tokens: `heading` (`#97BEDC`), `link` (`accent`), `code` (inline code, `#94B979`), `quote` (`muted`), `list` (`muted`).

A value is a color or the name of a top-level color token (`bg`, `accent`, `fg`, `user`, `user_bg`, `muted`, `input`, `input_bg`, `border`, `icons`, `error`). Links follow the target when it changes. A token cannot link to itself, and cycles fail validation. `diff` context lines use `muted`, `diff` headers use `border`, and hovered links use `fg`.

## Migration

No silent compatibility. Old keys fail loading: `settings` fails with `settings: moved to ui, history, and open`, top-level `theme` with `theme: moved to ui.theme`. Removed color names in `ui.theme` fail as unknown keys.
