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
  `bash -c command`, run at load and on `/reload`. The Python code in `hooks.*.run` and
  `functions.*.run` is left as is (read the environment there with `os.environ` or `sh`).

## Keys

| Key | Meaning |
| --- | --- |
| `model` | Default model, `name` or `provider/name`. |
| `provider` | The `connect` entry for `model` when it has no `provider/` prefix (default: the first one). |
| `effort` | `default` (sends no `reasoning_effort`), `low`, `medium`, `high`, `xhigh`, `max`. |
| `connect` | List of `{name, base_url, api_key}` for OpenAI-compatible endpoints. |
| `models` | List of `{name: provider/model, display, window, compact_at, prices}`. `prices` are dollars per 1M tokens: `in`, `out`, `cache` (cached reads) and `cache_write`. |
| `settings.compact_at` | Context size that triggers compaction (0 turns it off). Unset: 60% of the model's `window` (off when the window is unknown). A model's `compact_at` wins. |
| `settings.max_output` | Bytes of a tool result kept; longer output is cut in the middle. The agent is told the number. Default 4096. |
| `settings.tool_timeout` | Seconds before a `python` call is stopped (a running `sh` goes on as a task; time spent in `ask` does not count). Default 10. The agent can pass its own `timeout`. |
| `settings.hook_timeout` | Seconds before a hook is stopped. Default 30. |
| `settings.max_image_bytes` | Largest image `read` sends to the model. Default 5242880. |
| `settings.retries` | Retries for 429, 5xx and network errors, after 2, 5, 10, 20 and 30 s. Default 5. |
| `systems` | Texts appended to the built-in system prompt. |
| `prompts` | `name: text`. `#name` picked in the input sends the text along ([prompts.md](prompts.md)). |
| `functions` | Named Python helpers with `args`, `description` and `run`. Available to the agent, hooks and tasks. |
| `hooks` | `name: {events: [...], run: python code}`, see `docs("hooks")`. |
| `settings.theme` | Name of the active theme (empty or `default`: the built-in one). `/theme` changes it. |
| `settings.ui` | UI behavior and layout preferences, see `docs("theme")`. |
| `themes` | `name: {extend: base, bg: ..., ...}`, 10 base colors, see `docs("theme")`. |

## Compaction

Records stay intact; a compaction only moves two boundaries that every later request
applies. The latest quarter of `compact_at` is sent unchanged. The quarter before it
keeps tool calls but cuts their outputs to the first line. Everything older keeps
only user and assistant text.

## Files

`~/.tuse/<id>/` (or `$TUSE_HOME/<id>/`) holds `session.jsonl`, `meta.json`, `lock`
and `tasks/<id>.log`. A session can be open in one process only, and a process holds one session.
