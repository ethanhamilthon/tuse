## 1. Config schema

- [x] 1.1 Replace `settings` with `ui` (`lines`, `input`, `space`, `theme`), `history.hide`, and top-level `open`; fail old `settings` and `theme` keys with `moved to` errors.
- [x] 1.2 Validate `ui.lines` (`tool`, `output`, `diff`, `reasoning`, `strings`, `input`, `list`) with the ranges and defaults of the design; `strings` replaces `fold_strings`.
- [x] 1.3 Validate `ui.input` as `boxed`, `lines`, `fill`, or `bar`; drop `classic`.
- [x] 1.4 Validate `ui.space` (`padding`, `steps`, `messages`); fix the wheel step at 2 lines and remove `scroll_step`.
- [x] 1.5 Remove `scope_models`; make `/models` list only configured `models` when set.
- [x] 1.6 Accept `commands` only as `{ args?, desc?, run }` and fail bare functions.

## 2. Theme

- [x] 2.1 Replace the theme struct with the new tokens, the `code` table (with `added` and `removed`), and the `markdown` table; resolve links to top-level color tokens and reject cycles.
- [x] 2.2 Switch every TUI color lookup (chat, tool view, diff, markdown, syntax, status, composer, pickers, panels, spinners) to the new tokens.
- [x] 2.3 Wire `ui.space.steps`, `ui.space.messages`, `ui.space.padding`, and `ui.lines.strings` into rendering.
- [x] 2.4 Expose the resolved `ui`, `history`, and `open` in `/config` and `config.get`.

## 3. Docs, examples, and tests

- [x] 3.1 Update `docs/config.md`, `docs/usage.md`, `docs/lua.md`, `docs/json.md`, and the system prompt bullet about the UI.
- [x] 3.2 Port `examples/*.lua`, `examples/themes.lua`, `examples/project.lua`, the repo `tuse.lua`, and any function-form commands to the new layout.
- [x] 3.3 Cover old-key errors, defaults, `strings` folding, theme links and cycles, `/models` filtering, and command shapes; update the docs coverage test to the new keys.
