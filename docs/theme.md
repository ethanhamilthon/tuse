# Theme & UI Settings

## Theme

The config defines one theme under `theme`; it is not selectable in the UI. It starts from the built-in dark palette and overrides only the tokens you specify. Invalid color values or unknown keys fail config validation.

### Color values

Color validation is case-insensitive (surrounding whitespace ignored):

- `#rrggbb` or `#rgb` (24-bit RGB hex; `#rgb` expands each nibble to two identical hex digits)
- `0`–`255` as a string (ANSI 256-color palette index, e.g. `"208"`)
- Named ANSI colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `grey`/`gray`, `dark_grey`/`dark_gray`, `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta`, `bright_cyan`, `bright_white`
- `"accent"` (inherits the resolved value of `theme.accent` — note that alias resolution during rendering is **case-sensitive** for lowercase `"accent"` despite case-insensitive validation, so `"ACCENT"` validates but resolves to the terminal default; setting `theme.accent = "accent"` likewise resolves to the terminal default)
- `"none"` (terminal default foreground/background)

### The 10 tokens

| Token | Default | Used for |
| --- | --- | --- |
| `bg` | `#121212` | Main background, including pickers, panels, and selected text foreground. |
| `surface` | `#1E1E1E` | Elevated background for user messages in chat (full-width block without `›`). Not used by pickers or panels. |
| `fg` | `#DEDEDE` | Primary text, plain panel text, code blocks, active input text, `/session` values. |
| `dim` | `#BDBDBD` | Secondary text: blockquotes, horizontal rules, table separators/headers in narrow mode, picker details and `no matches`, notices (`✓`), diff context lines, scroll mark (`↑N`), dimmed steering input, `/session` & `/config` secondary labels. |
| `muted` | `#929292` | Subtle text: tool output, reasoning (`∴`, italic), Lua syntax comments, inactive picker items, turn summary (`✓ 12s · ...`), and status bar labels. |
| `border` | `#333333` | Input rules, overlay top borders, disabled input `›`, `/session` & `/config` keys, diff file path headers, truncation marks (`…`, `… N more lines`). |
| `accent` | `#BDDEAB` | Primary highlight: active input `›`, spinner and elapsed time, steering wave, used context count, flash notices, selection background, picked `#`/`@`/image atoms in input, selected picker item, Markdown list markers, Lua numbers, `show` (`◆`) and `pick`/`ask` (`?`) markers, hovered clickable links (bold), `/config` headings. |
| `ok` | `#94B979` | Additions and success: diff `+` lines, inline Markdown `code`, Lua strings, `/session` cost (bold). |
| `err` | `#E27373` | Errors and deletions: diff `-` lines, error entries (`✗`), failed tool calls, Lua keywords. |
| `info` | `#97BEDC` | Tools, headings (bold), links (underlined by default, including Markdown links whose destination is not clickable), Lua host and called functions, `/session` mode/effort/window, `/config` hooks/commands/open/plugins. |

```lua
return {
  theme = { accent = "#FFB86C", info = "bright_blue" },
}
```

## UI settings (`settings.ui`)

UI behavior and layout live under `settings.ui`, separate from theme colors:

| Key | Default | Allowed values | Behavior |
| --- | --- | --- | --- |
| `fold_strings` | `true` | boolean | Folds multiline Lua long strings (`[[...]]`, `[=[...]=]`, including unclosed streaming strings) in chat tool calls to a single line (`[[first non-empty line…]]`). `/trace` always shows unfolded source. |
| `tool_line` | `-1` | `-1` or `>= 0` | Max wrapped lines of Lua tool source shown in chat (`-1` = unlimited, `0` = hide source, `N > 0` = first `N` lines + `…`). |
| `output_line` | `1` | `-1` or `>= 0` | Max lines of tool output shown in chat (`-1` = unlimited, `0` = hide output, `N > 0` = first `N` lines + `… M more lines`). |
| `diff_line` | `12` | `-1` or `>= 0` | Max diff body lines per modified file shown in chat (`-1` = all stored diff lines, `0` = hide the entire diff block, `N > 0` = first `N` lines + `… M more lines`). |
| `reasoning_line` | `-1` | `-1` or `>= 0` | Max wrapped lines of model reasoning shown in chat (`-1` = unlimited, `0` = hide reasoning, `N > 0` = **last** `N` wrapped lines). |
| `pad_x` | `0` | `0..=65535` | Horizontal margin in columns on both left and right edges of the screen. |
| `message_gap` | `1` | `0..=65535` | Blank lines inserted between non-empty chat entries. |
| `scroll_step` | `3` | `1..=65535` | Lines scrolled per mouse wheel tick or Up/Down key in an open panel. |
| `list_max_height` | `16` | `>= 1` | Maximum height in rows (including top border) for picker and panel overlays. |
| `input_max_height` | `-1` | `-1` or `>= 1` (`0` invalid) | Maximum visible rows of the input editor (`-1` = grow up to available screen height). |

## Input framing and status bars

The input area is framed by two `border`-colored horizontal rules (`─`):

- **Top rule:**
  - Left side: scroll mark `↑N ` in `dim` whenever the chat is scrolled up (`N` lines above the bottom).
  - Right side: if a flash notice is active (lasts 5 seconds, for example `Copied.`, `Config reloaded.`, or retry status), it renders in `accent` here. Otherwise, while a turn is running:
    - With a queued steering message: animated wave (`●····`) in `accent` followed by `steering…` in `muted`.
    - Without a queued steering message: spinner (`◐`, `◓`, `◑`, `◒`) and elapsed duration in `accent` (for example `◐ 0s`, `◓ 14s`, `◑ 1m5s`, `◒ 1h2m`).
- **Bottom rule:**
  - Right side: model bar showing `<display> · <effort>`, optionally ` · <used>/<window>` (omitted when both `<used>` and `<window>` are `0`; shows `/?` when `<window>` is `0` and `<used> > 0`), and optionally ` · $<cost>` (2 decimal places, omitted when pricing is unknown). Everything is `muted` except `<used>`, which is `accent`.

Each label area is capped at half the inner width (the working label shares the remaining half after the scroll mark) and clipped with `…` if wider.
