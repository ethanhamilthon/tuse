## Why
The config mixes UI keys under `settings`, scatters limits and spacing into flat `settings.ui` names, and has a theme of ten overlapping colors that cannot style code, Markdown, icons, or the input separately. Commands accept two shapes, and `scope_models` duplicates what `models` already says.

## What Changes
- **BREAKING** Remove `settings`. UI keys move to a top-level `ui` table: `ui.lines`, `ui.input`, `ui.space`, `ui.theme`. `hide_history` moves to `history.hide`. `settings.open` moves to a top-level `open`.
- **BREAKING** Remove the top-level `theme`. The theme moves to `ui.theme` with new tokens: `bg`, `accent`, `fg`, `user`, `user_bg`, `muted`, `input`, `input_bg`, `border`, `icons`, `error`, and the tables `code` and `markdown`. The colors `surface`, `dim`, `ok`, `err`, `info` are removed.
- **BREAKING** Replace `fold_strings` with `ui.lines.strings` (a line count). Rename `tool_line`, `output_line`, `diff_line`, `reasoning_line`, `input_max_height`, `list_max_height` to `ui.lines.tool`, `output`, `diff`, `reasoning`, `input`, `list`. `input` now defaults to 10.
- **BREAKING** Rename `input_style` to `ui.input` and drop the `classic` alias. Move `pad_x` and `message_gap` to `ui.space.padding` and `ui.space.messages`; add `ui.space.steps`.
- **BREAKING** Remove `scroll_step`. The mouse wheel always scrolls 2 lines per tick.
- **BREAKING** Remove `scope_models`. When `models` is set, `/models` lists only those models.
- **BREAKING** `commands` accept only the table `{ args?, desc?, run }`. The bare function form is removed.

## Impact
- Specs: `config`, `tui`, `docs`.
- Implementation: config schema and validation, theme resolution, TUI color lookups, spacing and folding, `/config` view, `/models`, command loading.
- Docs and examples: `docs/config.md`, `docs/usage.md`, `docs/lua.md`, the system prompt bullet about the UI, `examples/themes.lua`, `examples/project.lua`, and every example that uses `settings`, `theme`, or function commands.
- Old keys fail loading with a `moved to` error.
