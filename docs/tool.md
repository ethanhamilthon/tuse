# The `lua` tool

The agent has a single tool, `lua`, taking one required argument: `code` (a string of Lua 5.4 source). See [functions.md](functions.md) for the canonical host function reference, [lua.md](lua.md) for full VM/scheduler details, and [agent-loop.md](agent-loop.md) for how tool calls fit into turns.

## Tool call contract

- **Arguments:** JSON object `{"code": "..."}`. Invalid JSON, a missing or non-string `code`, or calling any tool name other than `lua` returns an error tool result (`code = 1`) immediately without starting a Lua job or firing `lua_start`/`lua_end` hooks. Unknown tool names tell the model to call functions such as `read` or `sh` inside `lua`.
- **Compilation and return values:** Tuse first tries compiling `return <code>` so a lone expression (such as `x + 1` or `read("a.txt")`) returns its value automatically; if that does not parse, it compiles `<code>` as statements.
- **Output format:** Captured `print(...)` (tab-separated, newline-terminated) and `io.write(...)` (concatenated directly) come first, followed by any non-nil return values rendered with `inspect(value)` (tab-separated, separated from prior printed text by a newline when needed). Top-level returned strings render raw without quotes; tables with a `__tostring` metamethod (including `sh` results) use `tostring`; plain tables format with sorted keys, cycle detection (`<cycle>`), and a depth cap (root depth `0`; tables nested beyond depth `6` render as `{...}`).
- **Uncapped output:** Tuse does not truncate `lua` call output, `sh().out`, or task logs. Keep large outputs concise in your script (for example `out:sub(1, 2000)` or `grep` with `limit`).
- **Exit status:** Successful calls finish with `code = 0`, errors and timeouts with `code = 1`, and cancelled calls with `code = nil`.

## VM lifetime, timeouts, and scheduling

- **Persistent VM:** One Lua 5.4 VM runs on a dedicated thread for the lifetime of a session. Globals persist between `lua` calls (use `local` for scratch variables). Errors, timeouts, and cancellations do not destroy the VM or wipe globals. Starting `/new` or `/resume` creates a fresh VM.
- **120-second deadline:** Each `lua` tool call has a 120-second wall-clock deadline. Tight CPU loops are interrupted via an instruction hook (every 1,000 instructions) and return `[timed out]` or `[cancelled]`. Start long jobs with `spawn(fn, name)` (background tasks have no deadline).
- **Cooperative scheduler:** Agent tool calls, hooks, custom slash commands, `open` handlers, and background tasks all run as coroutines in the same VM. Waiting calls (`sh`, `sleep`, `pick`, `ask`, `reload`, `llm`) yield so other coroutines continue running; `sh` and `sleep` also yield through user-created `coroutine.resume` and `coroutine.wrap`.
- **Paused deadlines:** Interactive UI waits (`pick`, `ask`) and `reload()` pause the calling coroutine's remaining deadline while waiting and resume the remaining budget afterward. `llm()` does not pause the deadline.
- **Non-yieldable boundaries:** Inside C callbacks where Lua cannot yield (for example a `string.gsub` function), `sh` and `sleep` block the VM thread until finished (still respecting cancellation and deadlines), while `pick`, `ask`, `reload`, and `llm` raise `interactive wait cannot run here`.

## Standard library and modules

- **Loaded libraries:** `string`, `table`, `math`, `utf8`, `io`, `os` (with `os.exit = nil`), `coroutine`, `package`, `load`, `loadfile`, `dofile` (takes only a file path, no explicit environment argument), and `require`. The `debug` library is **not** available. Lua patterns are not regular expressions; use `grep` or `sh` for regexes.
- **Added globals:** `json.encode(value)`, `json.decode(text)` (maps JSON `null` to `nil`), and `inspect(value)`.
- **Modules:** `package.path` and `package.cpath` are cleared. `require` resolves `"tuse"` (host functions table), `"global"`, relative paths (`"name"`, `"a.b"`, `"./x"`, `"../x"`), `"~/..."`, absolute paths, and explicit `.lua` paths, cached by canonical path. An initial file or preload `require` returns `(value, canonical_path_or_preload)`; subsequent cached calls return only `(value)`. Required files run in the file environment (host globals hidden; import them via `require("tuse")`). See [config.md](config.md).
- Paths passed to file functions may be absolute, `~/...`, or relative to the session working directory.

## File, search, and shell behavior inside `lua`

Built-in host functions are Lua globals inside `lua`, not separate API tools. Automatic diff attachment and model image attachment apply **only** to agent `lua` tool calls (not hooks, custom commands, `open` handlers, or background tasks). See [functions.md](functions.md) for full signatures and options.

- **`read(path, opts?)`:** Rejects directories; returns `[binary file, N bytes]` when a NUL byte appears in the first 8,192 bytes; returns raw text by default or paginated numbered lines with `{lines = true, offset = 1, limit = 2000}` (lines cut at 2,000 chars with `… [line cut]`). On images (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`) up to `settings.max_image_bytes`, returns `[image: <path>, <KB> KB]` and attaches the image to the tool result so the model sees it.
- **`write(path, content)` / `append(path, text)` / `delete(path)` / `edit(path, old, new, {all = false})`:**
  - `write`, `append`, and `edit` write atomically (temp file in target directory, preserving existing file permissions, flushing to disk, then renaming). `write` and `append` create missing parent folders and reject directories; `append` decodes existing file contents with lossy UTF-8 before appending; `delete` removes a single file (never directories).
  - `edit` requires UTF-8 text, exact match (unique unless `all = true`, listing up to 10 line numbers on ambiguity), and preserves CRLF line endings when `old` contains no `\r`.
  - **Diff capture:** Each file mutation in an agent `lua` call attaches a diff to the tool result (shown in TUI chat and `/trace`, not returned in the Lua string). Diffs include 2 context lines, up to 5 hunks for `edit` (`[N more replacements]`), up to 200 lines for whole-file creation/deletion (`[N more lines]`), and up to 200 removed lines **and** 200 added lines per changed span for `write` overwrites and `append` (`[N more changed lines]`). Binary deletions record `- [binary file, N bytes]`.
- **`grep(pattern, opts?)`:** Searches files with regular expressions (or `literal = true`), respecting `.gitignore` and skipping `.git` and binary files. Formats matches as `path:line: text` and context lines as `path:line- text` (`--` between separated context blocks), clipping lines at 300 characters with `…`. Default limit is 100 matches.
- **`sh(cmd, opts?)`:** Runs `bash -c <cmd>` with stdin closed, stdout and stderr merged, in its own process group. Timeout clamps to the calling coroutine's remaining deadline; timeout, cancellation, or drop kills the entire process group with `SIGKILL`. Returns `{out, code, ok, timed_out}` with `:lines()` and `__tostring`. Shell state does not persist between calls.
