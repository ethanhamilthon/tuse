# TODO 0.5.0

Breaking release (no config migration required).

## 1. Simplify UI tokens (`themes` and `settings.ui`)

Replace the 92-token flat `Theme` struct with a minimal 10-color theme + a dedicated `settings.ui` section for UI behavior/layout preferences. Move all symbols, prefixes, status labels, and technical/engine constants into code.

### 1.1. `Theme` — 10 base colors only (`crates/config/src/theme.rs`, `crates/tui/src/style.rs`)
Keep `themes.<name>` with `extend` support, containing only 10 color tokens (Vercel Dark defaults):
- `bg` (`#0A0A0A`) — main background, selected list item foreground
- `surface` (`#1A1A1A`) — elevated background (`user_bg`, `input_bg`, `list_bg`)
- `fg` (`#EDEDED`) — primary text, user/assistant text, code blocks
- `dim` (`#A0A0A0`) — secondary text (tool output, quotes, list details, notices)
- `muted` (`#676767`) — subtle text (reasoning, syntax comments, motion gradient start)
- `border` (`#2E2E2E`) — horizontal rules, box frames, separators
- `accent` (`#EDEDED`) — primary highlight (status, bar, list title, selected list bg, motion gradient end)
- `ok` (`#00AC3A`) — additions / success (`diff_add`, inline `code`, `syntax_string`)
- `err` (`#F32E40`) — errors / deletions (`error`, `tool_error`, `diff_del`, `syntax_keyword`)
- `info` (`#52A8FF`) — tools / links / headings (`tool`, `heading`, `link`, `syntax_function`)

`Palette` in `crates/tui/src/style.rs` parses these 10 colors once per reload and all TUI modules map directly to them.

### 1.2. `settings.ui` (`crates/config/src/types.rs`)
Move user-facing UI behavior & layout preferences out of `Theme` into `settings.ui` (so switching `/theme` does not overwrite them):
- `input_design`: `"lines"` | `"box"` | `"fill"` | `"bar"` (default `"lines"`)
- `motion`: `bool` (default `true`)
- `tool_line`: `i32` (default `-1`)
- `diff_line`: `i32` (default `12`)
- `output_line`: `i32` (default `1`)
- `reasoning_line`: `i32` (default `-1`)
- `scroll_step`: `u16` (default `3`)
- `pad_x`: `u16` (default `0`)
- `message_gap`: `u16` (default `1`)
- `list_max_height`: `usize` (default `16`)
- `input_max_height`: `i32` (default `-1`)

### 1.3. Move symbols, prefixes, labels, and internals into code (`crates/tui`)
Remove from config and define as Rust constants in `crates/tui`:
- All symbols & prefixes (`input_prefix`, `user_prefix`, `tool_prefix`, `reasoning_prefix`, `notice_prefix`, `error_prefix`, `list_selected_prefix`, `quote_prefix`, `bullet`, `rule_char`, `table_sep`, `scroll_mark`, `label_sep`, `more`, `mask_char`, `spinner`, `bar_char`, `box_chars`, `edge_top`, `edge_bottom`)
- All status texts (`steering_label`, `thinking_label`, `read_label`, `write_label`, `edit_label`, `grep_label`, `sh_label`, `ask_label`, `docs_label`, `ask_custom_label`)
- All internal spacing & engine knobs (`list_indent`, `code_indent`, `list_pad_x`, `input_pad_x`, `input_top_padding`, `input_bottom_padding`, `trace_role_width`, `file_picker_max`, `frame_ms`, `motion_period`, `motion_ms`)

### 1.4. Docs & rules
- Update `docs/config.md` to document the 10-color `themes` and `settings.ui`.
- Update `AGENTS.md` rule ("Every UI decision is a theme token...") to reflect the new split (10 color tokens + `settings.ui`; symbols/labels/internals live in code).

## 2. Working indicator: show only tool `title` (no `reading`/`asking` overrides)

Simplify the working indicator so it never overwrites the tool's `title` with per-function labels (`reading ...`, `writing ...`, `asking user...`, etc.) and shows only spinner + elapsed time while the model is thinking.

- **Remove `Host::status`**: delete `Host::status(&self, name, args)` from `crates/py` (`Host` trait and call in `crates/py/src/funcs.rs`) and remove `SessionHost::status` (and unused helpers like `last_line_of_command`) from `crates/core/src/host.rs`.
- **Turn start (`crates/core/src/control.rs`)**: initialize `st.status = String::new()` instead of `thinking_label` so the indicator shows only `<spinner> <elapsed>`.
- **Tool start & end (`crates/core/src/tool.rs`)**:
  - On tool start: `self.set_status(title.clone())` (shows `<spinner> <elapsed> · <title>` when `title` is non-empty).
  - On tool end: clear status back to `""` (`self.set_status(String::new())`) so between calls / while the model thinks, only `<spinner> <elapsed>` is shown.
- **Steering (`crates/tui/src/status.rs`)**: keep `"steering…"` unchanged when `app.steering.is_some()`.
- **Tests & docs**: update `crates/core/tests/agent_flow.rs` (status assertions) and `docs/config.md`.

## 3. `/trace`: Left/Right navigation in opened item & no boundary wrap

- **Navigate opened `/trace` items (`crates/tui/src/panel_keys.rs`, `crates/tui/src/commands.rs`)**:
  - When a `/trace` item is open in `Panel` (`self.trace_item.is_some()`) and `self.editor.is_empty()`:
    - `Left` opens the previous item (`at - 1`), clamped at `0` (no wrap to the end).
    - `Right` opens the next item (`at + 1`), clamped at `self.trace.len() - 1` (no wrap to the start).
  - Reset panel scroll to `0` when switching items; `Esc` returns to the `/trace` list at the newly active `trace_item`.
  - Panel title shows full position info: `Trace {at + 1}/{total} {role}` (e.g. `Trace 4/18 python`).
- **No wrap in `/trace` list (`crates/tui/src/picker.rs`)**:
  - In `Picker::moved`, when `self.kind == Kind::Trace`, clamp `self.selected` to `0..len - 1` instead of wrapping with `rem_euclid` (going up from the first item stays at `0`, going down from the last stays at the end).

## 4. Styled `/usage` panel

Add multi-color styling to the `/usage` panel using the 10-color `Palette` instead of a single flat text color.

- **`Panel` styled content (`crates/tui/src/panel.rs`)**: support pre-styled `Vec<Line>` (e.g. `Panel::styled(title, lines)`) alongside plain text panels so `/usage` can render colored spans per row.
- **Usage rows styling (`crates/tui/src/commands.rs` or `crates/tui/src/usage.rs`)**:
  - Row keys (`Session`, `Model`, `Context`, `Turns`, `Tokens`, `Cost`): `border` (fixed-width left column).
  - `Session`: session ID in `dim`.
  - `Model`: display name in `fg` (omit duplicate `({model})` when `display == model`), `, effort ` in `dim`, effort value in `info`.
  - `Context`: current context size in `accent` (bold); `/ {window} ({pct}%)` in `info` (or `dim` for punctuation); `, compacts at {n}` in `dim`.
  - `Turns`: labels (`tool calls`, `requests`) in `dim`, numbers in `fg`.
  - `Tokens`: labels (`in`, `cached`, `cache write`, `out`) in `dim`, numbers in `fg`.
  - `Cost`: dollar amount in `ok` (bold).
