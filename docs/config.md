# Config

Tuse reads one YAML file: `--config PATH`, else `./tuse.yaml`, else
`~/.config/tuse/main.yaml`. `/reload` rebuilds it from its files.

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
| `theme` | Name of the active theme (empty or `default`: the built-in one). `/theme` changes it. |
| `themes` | `name: {extend: base, token: value, ...}`, see below. |

## Hooks

Hooks are commands started on events. Tuse does not wait for them. Events:
`session_start`, `session_end`, `turn_start`, `turn_end`, `tool_start`, `tool_end`,
`error`. A relative `run` path is resolved against the file that defines it.

Environment: `TUSE_EVENT`, `TUSE_HOOK`, `TUSE_SESSION`, `TUSE_CWD`, `TUSE_MODEL`,
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
- Spacing: `pad_x`, `message_gap`, `list_indent`, `code_indent`, `picker_pad_x`.
- Limits: `picker_max_height` (8), `input_max_height`, `scroll_step`, `file_picker_max`,
  `frame_ms`.
- Lines shown in the chat: `bash_line`, `output_line`, `reasoning_line`. `0` hides the
  block, `-1` removes the limit.
- Symbols: `user_prefix`, `tool_prefix`, `reasoning_prefix`, `notice_prefix`,
  `error_prefix`, `bullet`, `quote_prefix`, `rule_char`, `table_sep`, `scroll_mark`,
  `label_sep`, `picker_selected_prefix`, `more`, `spinner` (a list of frames).

## Files

`~/.tuse/<id>/` (or `$TUSE_HOME/<id>/`) holds `session.jsonl`, `meta.json`, `lock`,
`sock`, `bin/` (the `bins` shims) and `tasks/<id>.log`. A session can be open in one
process only, and a process holds one session.
