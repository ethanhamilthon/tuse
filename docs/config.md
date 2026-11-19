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
| `settings.max_output` | Bytes of a tool result kept; longer output is cut in the middle. Default 16384. Was `max_bash_output`. |
| `settings.tool_timeout` | Seconds before a `python` call is stopped. Default 120. The agent can pass its own `timeout`. Was `bash_timeout`. |
| `settings.hook_timeout` | Seconds before a hook is stopped. Default 30. |
| `settings.max_image_bytes` | Largest image `read` sends to the model. Default 5242880. |
| `settings.retries` | Retries for 429, 5xx and network errors, after 2, 5, 10, 20 and 30 s. Default 5. |
| `systems` | Texts appended to the system prompt. |
| `prompts` | `name: text`. `#name` in the input expands to the text. |
| `prelude` | List of Python snippets, joined and run once before the agent's REPL starts and before every hook. |
| `hooks` | `name: {events: [...], run: python code}`, see below. |
| `settings.theme` | Name of the active theme (empty or `default`: the built-in one). `/theme` changes it. A top-level `theme` still works. |
| `themes` | `name: {extend: base, token: value, ...}`, see below. |

## Hooks

A hook is Python code started on an event; Tuse does not wait for it (except for `session_end`,
which gets up to `hook_timeout` before Tuse exits). Events: `session_start`, `session_end`,
`turn_start`, `turn_end`, `tool_start`, `tool_end`, `error`.

Every call runs in a **fresh interpreter** with the same access as the agent's `python` tool:
`sh`, `ask`, `show`, `task_*`, files, the whole standard library of the sandbox, `import` of `.py`
files from the config folder, and the `prelude`. Nothing is kept between calls; keep state in
files. A hook stops after `hook_timeout` seconds. An error shows as the notice `hook NAME: ...`;
`print()` output is dropped. Tool calls a hook makes do not fire `tool_*` events. Hooks run concurrently, so two hooks of
successive events can finish in any order.

Names defined in the hook (`None` when they do not apply, `cancelled` is `False`):

| Name | Value |
| --- | --- |
| `event`, `hook` | The event and the hook's name. |
| `session`, `cwd`, `model`, `config` | Session ID, working directory, `provider/model`, path of the config file. |
| `tool`, `input` | For `tool_start` and `tool_end`: `python`, `read`, `write` or `edit`, and the code or the path. |
| `output`, `code` | For `tool_end`: the result (cut to `max_output`) and `0`, `1` (error) or `None` (cancelled). |
| `error` | For `error`: the text. |
| `cancelled` | For `turn_end`: the turn was cancelled. |

```yaml
hooks:
  notify:
    events: [turn_end]
    run: |
      if not cancelled:
          sh(f"osascript -e 'display notification \"done\" with title \"{model}\"'")
  log_edits:
    events: [tool_end]
    run: |
      if tool == "edit":
          with open("/tmp/edits.log", "a") as f:
              f.write(input + "\n")
```

To run a script, call it: `run: sh("~/scripts/x.sh")`. Longer code goes into a `.py` file next to
the config and `run: import notify`.

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
  `tool`, `tool_output`, `tool_error`, `diff_add`, `diff_del`, `error`, `notice`, `heading`, `code`,
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
- Lines shown in the chat: `tool_line` (the tool's input; was `bash_line`), `output_line`,
  `diff_line` (the diff of `edit`), `reasoning_line`. `0` hides the block, `-1` removes the limit.
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

`~/.tuse/<id>/` (or `$TUSE_HOME/<id>/`) holds `session.jsonl`, `meta.json`, `lock`
and `tasks/<id>.log`. A session can be open in one process only, and a process holds one session.
