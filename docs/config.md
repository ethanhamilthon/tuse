# Config

Tuse reads one YAML file: `--config PATH`, else `./tuse.yaml`, else `$TUSE_GLOBAL_CONFIG`,
else `~/.config/tuse/main.yaml`. The system prompt and the default settings are built into
the binary, so the file holds only what you change. `tuse reset` installs a fresh `main.yaml`;
`tuse update` never touches it. `/reload` rebuilds everything from its files.

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
| `settings.max_output` | Bytes of a tool result kept; longer output is cut in the middle. The agent is told the number. Default 4096. |
| `settings.tool_timeout` | Seconds before a `python` call is stopped (a running `sh` goes on as a task; time spent in `ask` does not count). Default 10. The agent can pass its own `timeout`. |
| `settings.hook_timeout` | Seconds before a hook is stopped. Default 30. |
| `settings.max_image_bytes` | Largest image `read` sends to the model. Default 5242880. |
| `settings.retries` | Retries for 429, 5xx and network errors, after 2, 5, 10, 20 and 30 s. Default 5. |
| `systems` | Texts appended to the built-in system prompt (after the environment and `AGENTS.md`). |
| `prompts` | `name: text`. `#name` in the input expands to the text. |
| `prelude` | List of Python snippets, joined and run once before the agent's REPL starts and before every hook. |
| `hooks` | `name: {events: [...], run: python code}`, see below. |
| `settings.theme` | Name of the active theme (empty or `default`: the built-in one). `/theme` changes it. |
| `settings.ui` | UI behavior and layout preferences (`input_design`, `motion`, line limits, spacing), see below. |
| `themes` | `name: {extend: base, bg: ..., ...}`, 10 base colors, see below. |

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
| `tool`, `input`, `title` | For `tool_start` and `tool_end`: `python`, its code and the `title` the agent gave it. |
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
  log_calls:
    events: [tool_end]
    run: |
      with open("/tmp/calls.log", "a") as f:
          f.write(title + "\n")
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
theme (or `default`, the built-in Vercel Dark palette) and overrides only the colors you write.

A theme has 10 color tokens. Colors take `#rrggbb`, `#rgb`, `0`-`255`, a name (`red`,
`dark_grey`, `bright_blue`, ...), `accent`, or `none` for the terminal default:

- `bg` (`#0A0A0A`): main background and selected list item foreground.
- `surface` (`#1A1A1A`): elevated background (user messages, filled input, lists and panels).
- `fg` (`#EDEDED`): primary text and code blocks.
- `dim` (`#A0A0A0`): secondary text (tool output, quotes, list details, notices).
- `muted` (`#676767`): subtle text (reasoning, syntax comments, motion gradient start).
- `border` (`#2E2E2E`): horizontal rules, box frames, separators, `/usage` keys.
- `accent` (`#EDEDED`): primary highlight (status, bar, list title, selected list bg, motion gradient end).
- `ok` (`#00AC3A`): additions and success (`diff` `+`, inline `code`, strings, `/usage` cost).
- `err` (`#F32E40`): errors and deletions (`diff` `-`, failed calls, keywords).
- `info` (`#52A8FF`): tools, headings, links, functions.

## UI settings (`settings.ui`)

UI behavior and layout live under `settings.ui` so switching `/theme` keeps them intact:

- `input_design`: `lines` (default), `box`, `fill` or `bar`.
- `motion`: `true` (default) or `false`. While the agent works, a gradient from `muted` to `accent`
  runs along the rules, frame borders, filled edges or bar.
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

## Files

`~/.tuse/<id>/` (or `$TUSE_HOME/<id>/`) holds `session.jsonl`, `meta.json`, `lock`
and `tasks/<id>.log`. A session can be open in one process only, and a process holds one session.
