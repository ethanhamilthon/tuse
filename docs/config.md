# Config

Tuse reads one YAML file: `--config PATH`, else `./tuse.yaml`, else `$TUSE_GLOBAL_CONFIG`,
else `~/.config/tuse/main.yaml`. The app knows no other file: the shipped `main.yaml`
loads `default.yaml` (system prompt and default settings) with `include`. `tuse reset`
installs both files, `tuse update` replaces `default.yaml`. `/reload` rebuilds everything
from its files.

## Merging and expansion

- `include` lists other files to load, with paths relative to the including file.
  Includes merge in order and the later one wins a conflict. The including file
  wins over all of its includes.
- Mappings merge key by key. Lists (`systems`, `connect`, `models`, hook `events`, ...)
  are concatenated. Any other value is replaced.
- In every string value, `$VAR` and `${VAR}` become environment variables (empty when
  unset) and `$$` is a literal `$`. `#{{command}}` becomes the trimmed output of
  `bash -c command`, run at load and on `/reload`.

## Keys

| Key | Meaning |
| --- | --- |
| `model` | Default model, `name` or `provider/name`. |
| `provider` | The `connect` entry for `model` when it has no `provider/` prefix (default: the first one). |
| `effort` | `default` (sends no `reasoning_effort`), `low`, `medium`, `high`, `xhigh`, `max`. |
| `connect` | List of `{name, base_url, api_key}` for OpenAI-compatible endpoints. |
| `models` | List of `{name: provider/model, display, window, compact_at, prices}`. `prices` are dollars per 1M tokens: `in`, `out`, `cache` (cached reads) and `cache_write`. |
| `settings.compact_at` | Context size that triggers compaction (0 turns it off). A model's `compact_at` wins. |
| `settings.max_bash_output` | Bytes of bash output kept; longer output is cut in the middle. Default 16384. |
| `settings.bash_timeout` | Seconds before a bash call is killed. Default 120. The agent can pass its own `timeout`. |
| `settings.retries` | Retries for 429, 5xx and network errors, after 2, 5, 10, 20 and 30 s. Default 5. |
| `systems` | Texts appended to the system prompt. |
| `prompts` | `name: text`. `#name` in the input expands to the text. |
| `bins` | `name: command`. Short aliases on the agent's PATH, also listed in the system prompt. |
| `hooks` | `name: {events: [...], run: command}`, see below. |
| `settings.theme` | Name of the active theme (empty or `default`: the built-in one). `/theme` changes it. A top-level `theme` still works. |
| `themes` | `name: {extend: base, token: value, ...}`, see below. |

## Hooks

Hooks are commands started on events. Tuse does not wait for them. Events:
`session_start`, `session_end`, `turn_start`, `turn_end`, `tool_start`, `tool_end`,
`error`. A relative `run` path is resolved against the file that defines it.

Environment: `TUSE_EVENT`, `TUSE_HOOK`, `TUSE_SESSION`, `TUSE_CWD`, `TUSE_MODEL`, `TUSE_CURRENT_CONFIG`,
plus `TUSE_TOOL_COMMAND` and `TUSE_TOOL_EXIT` for tool events, `TUSE_ERROR` for
`error`, and `TUSE_CANCELLED=1` for a cancelled `turn_end`.

## Compaction

Records stay intact; a compaction only moves two boundaries that every later request
applies. The latest quarter of `compact_at` is sent unchanged. The quarter before it
keeps tool calls but cuts their outputs to the first line. Everything older keeps
only user and assistant text.

## Theme

Themes live under `themes`, one key per theme. `extend: other` starts from another
theme (or `default`, the built-in one) and overrides only the tokens you write.

Every UI decision is a token. Colors take `#rrggbb`, `0`-`255`, a name (`red`,
`dark_grey`, `bright_blue`, ...), `accent`, or `none` for the terminal default.

- Colors: `accent`, `bg`, `fg`, `dim`, `user`, `user_bg`, `assistant`, `reasoning`,
  `tool`, `tool_output`, `tool_error`, `error`, `notice`, `heading`, `code`,
  `code_block`, `link`, `quote`, `rule`, `status`, `picker_fg`, `picker_bg`,
  `picker_selected_fg`, `picker_selected_bg`, `picker_detail`, `picker_title`.
- Input and panel colors: `input_bg` (the `fill` design), `bar` (the `bar` design),
  `panel_fg`, `panel_bg`, `panel_title`, and the gradient ends `motion_from`, `motion_to`
  (`#rrggbb`, so they can be blended).
- Spacing: `pad_x`, `message_gap`, `list_indent`, `code_indent`, `picker_pad_x`,
  `input_pad_x`, `input_top_padding`, `input_bottom_padding` (rows in `bar`, default `0`),
  `panel_pad_x`, `trace_role_width` (the role column of `/trace`, 9).
- Limits: `picker_max_height` (8), `input_max_height`, `panel_max_height` (16), `scroll_step`,
  `file_picker_max`, `frame_ms`, `motion_period` (cells per wave), `motion_ms` (ms per step).
- Lines shown in the chat: `bash_line`, `output_line`, `reasoning_line`. `0` hides the
  block, `-1` removes the limit.
- Symbols: `user_prefix`, `tool_prefix`, `reasoning_prefix`, `notice_prefix`,
  `error_prefix`, `bullet`, `quote_prefix`, `rule_char`, `table_sep`, `scroll_mark`,
  `label_sep`, `steering_label`, `picker_selected_prefix`, `more`, `spinner` (a list of frames),
  `input_prefix` (default `› `; `""` removes it), `bar_char`, `box_chars` (top left, top
  right, bottom left, bottom right, vertical), `edge_top`, `edge_bottom`.

### Input design

`input_design` picks how the input looks. Each design shows the input and two status bars:
working (spinner, time, tool calls; `steering_label` while a message you sent waits for the agent) and the model (model, effort, context, cost).

- `lines` (default): a rule above and below; working sits on the top rule, the model on the bottom one.
- `box`: a frame around the input with the same two labels on its borders.
- `fill`: the input has its own background (`input_bg`); one status row below, working on the
  left and the model on the right.
- `bar`: a colored vertical bar (`bar`) left of the input, a blank row, then the same status
  row. `input_top_padding` and `input_bottom_padding` add rows above and below the text; the bar
  runs through them.

While the agent works, a gradient from `motion_from` to `motion_to` runs along the rules, the
frame's borders, the edges of the filled input or the bar. `motion: false` turns it off.

## Files

`~/.tuse/<id>/` (or `$TUSE_HOME/<id>/`) holds `session.jsonl`, `meta.json`, `lock`,
`sock`, `bin/` (the `bins` shims) and `tasks/<id>.log`. A session can be open in one
process only, and a process holds one session.
