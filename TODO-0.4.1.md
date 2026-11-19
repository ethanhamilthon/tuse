# TODO — 0.4.1

## 1. `grep` as a host function

The agent searches with `sh("grep ...")` or `re` in `python`. A dedicated function is cheaper in tokens and does not depend on what is installed. After section 13 there is one tool, so `grep` is a **host function of `python`**, not a tool. Do section 13 first.

- [x] `grep(pattern, path=".", glob=None, ignore_case=False, context=0, limit=100)` in `crates/core/src/tools/grep.rs` (a host function, registered in `host.rs` with the others). `pattern` is a regex, `path` a file or a folder, `glob` a file filter (`*.rs`), `context` the lines around a match, `limit` the number of matches.
- [x] It returns a string, one `path:line: text` per match, grouped by file, with a last line when `limit` cut the result. It respects `.gitignore`, skips binary files and `.git`.
  - Decide how: a crate (`ignore` + `regex`) or a call to `rg`/`grep`. Prefer a crate, so it works without external tools.
- [x] It checks `ctx.stopped()` while walking (Esc and the call deadline must stop a search over a big tree).
- [x] Errors are Python exceptions: a bad regex is `ValueError`, a missing path is `FileNotFoundError`.
- [x] A test: a match with context, `glob`, `limit` cut, no match, a bad regex.
- [x] Docs: the function list in `README.md`, `docs/agent.md` and the prompt.

## 2. Remove all migrations

Nothing from before 0.4 is kept: no old names, no old formats, no notes.

- [x] `crates/config`: drop `alias = "max_bash_output"`, `alias = "bash_timeout"`, `alias = "bash_line"`, the legacy top-level `theme` (only `settings.theme`) and the `bins` field with its "removed in 0.4" handling.
- [x] `crates/core/src/prompt.rs`: drop `{max_bash_output}` and `{bash_timeout}` in `fill`.
- [x] `crates/core/src/record.rs`: drop `bash_name` (the default tool name) and `alias = "command"`; `trace.rs` `role_of` has no `bash` fallback; `compaction.rs` and `crates/api/src/stream.rs` tests stop using `bash` as a tool name; `tui` stops matching `"bash"`.
  - This breaks sessions written before 0.4 (they have no tool name). Decide: such a session fails with a clear error, or is skipped in the list.
- [x] Tests that use old names (`config/src/tests.rs`: `bash_timeout`, `max_bash_output`, `bash_line`) use the new ones.
- [x] Docs: remove "Was `max_bash_output`", "Was `bash_timeout`", "was `bash_line`", "A top-level `theme` still works" from `docs/config.md`, and any other "old/legacy/removed" text in `README.md` and `docs/`.
- [x] `release/CHANGELOG.md`: the 0.4.0 "Migration" paragraph and the "Old sessions open as before" line stay as history; 0.4.1 has no migration notes.

## 3. Remove `default.yaml`; the tool docs become a Markdown file

Today `release/default.yaml` holds the system prompt and the default settings, and is replaced on every `tuse update`. The prompt about the tools belongs to the code, so it is embedded in the binary and always matches it.

- [x] A Markdown file with the system prompt, e.g. `crates/core/prompts/system.md` (tools, functions in `python`, config), embedded with `include_str!` like `docs.rs` does. The `python` tool description is a section of it; update it for `grep`.
  - `{max_output}` and `{tool_timeout}` are filled as now.
  - The "Environment" and "Project rules (AGENTS.md)" parts (`#{{pwd}}`, `#{{uname -sm}}`, `#{{cat AGENTS.md}}`) are built in code or stay as `#{{cmd}}` expansion in the file. Decide.
  - `systems` in the user's config still work: the built-in prompt comes first, the `systems` entries follow it.
- [x] Default settings come from `Settings::default()` (check that it has `compact_at: 0`, `max_output: 16384`, `tool_timeout: 120`, `hook_timeout: 30`, `max_image_bytes: 5242880`, `retries: 5`), so there is no file to load them from.
- [x] Delete `release/default.yaml`. `release/main.yaml` loses `include: [default.yaml]`.
- [x] `tuse update` replaces only the binary; `tuse reset` installs only `main.yaml` (`crates/cli/src/update.rs`, `main.rs` help text).
- [x] Also remove it from: `Makefile` (`make install`), `.github/workflows/release.yml` (`cp release/default.yaml ...`), `release/install.sh` if it installs it, `tuse.yaml` (the dev config includes it).
- [x] The test `shipped_main_includes_default` becomes a test that the built-in prompt contains "running inside Tuse" and the settings defaults.
- [x] Docs: `README.md`, `docs/agent.md`, `docs/config.md` and the prompt text itself ("`default.yaml` next to it is replaced on every update").

## 4. New defaults

All values live in `Settings::default()` and `Theme::default()` (`crates/config`). Update the defaults quoted in `docs/config.md` and `README.md`.

- [x] `settings.max_output`: 16384 -> **4096** (4 KB).
- [x] `settings.tool_timeout`: 120 -> **10** s.
- [x] The system prompt tells the agent both limits, filled from the config (`{max_output}`, `{tool_timeout}`), so a changed setting changes the text.
  - Check that the `python` tool schema says the same: the timeout in the `timeout` description is already dynamic; add the output limit to the description.
  - `{max_output}` is a number of bytes; print it readable (`4 KB`) or say "bytes" in the text. Keep one way in the prompt and the schema.
- [x] `input_max_height`: 10 -> **-1** (no limit). It is a `u16` now and cannot hold `-1`: make it `i32` and use `limit()` like the `*_line` tokens (`draw.rs:23`, keep at least one row). The input still cannot grow past the screen.
- [x] `tool_line`: 3 -> **-1**.
- [x] `output_line`: 3 -> **1**.
- [x] `reasoning_line`: 2 -> **-1**.
- [x] `pad_x`, `input_pad_x`, `panel_pad_x`: 1 -> **0**.
  - Check the layouts that count on the padding (`lines`, `box`, `fill`, `bar`, pickers, panels): text must not touch the frame's border in `box` and `fill`.
- [x] `tool_prefix`: `$ ` -> **`* `**. `$` came from `bash`; the tools are `python`, `read`, `write`, `edit` and `grep` now. `tool_view.rs` takes the width from the token, so nothing else changes.
- [x] `spinner`: braille dots -> **`◐ ◓ ◑ ◒`** (a circle). With 4 frames instead of 10 the cycle is shorter: check the step in `status.rs` (`app.tick`, `frame_ms`) and keep the speed pleasant.
- [x] `notice_prefix`: `· ` -> **`✓ `** (a check mark). It is used by notices in the chat (`render.rs`) and by inline notices (`inline.rs`, `Tone::Notice`).
- [x] A test: `Theme::default()` and `Settings::default()` hold these values (one test; they are what the docs promise).

## 5. Syntax highlighting for Python code

The code of a `python` call is shown in one color (`tool`). Highlight it.

- [x] A small hand-written Python tokenizer in `crates/tui` (no new dependency; Tuse has none for this): keywords, strings (with prefixes `f`, `r`, `b`, triple quotes), comments, numbers, decorators, and the names of the host functions of `python` (`sh`, `read`, `write`, `edit`, `grep`, `ask`, `show`, `task_run`, ...). It must not fail on broken or half-written code (a call is shown while it streams), and one token never spans lines in a way that breaks line clipping and the `tool_line` limit.
- [x] New theme tokens (colors, so every theme can change them): `syntax_keyword`, `syntax_string`, `syntax_comment`, `syntax_number`, `syntax_function`; the rest of the code keeps `tool`. Defaults from the Vercel Dark palette already in `Theme::default()`.
- [x] `tool_view.rs`: the input of a `python` call is drawn with these colors; a failed call keeps `tool_error` for the whole input. The `*` prefix, `pad` and the `tool_line` limit work as before.
- [x] Also use it for fenced blocks marked `python` / `py` in the assistant's Markdown (`markdown.rs`, now one `code_block` color). Other languages stay as they are.
- [x] A test: the tokenizer on a snippet (keyword, string, comment, number) and on an unterminated string.
- [x] Docs: the new tokens in `docs/config.md`.

## 6. The working indicator shows a status; `python` gets a `title`

The working indicator shows the spinner, the time and `N calls`. It shows a **status** instead: what the agent is doing now. The call counter leaves the indicator.

- [x] The core keeps one `status` string per running turn. `turn_info()` returns `(elapsed, status)` instead of `(elapsed, calls)`. The status is **not cleared when a call ends**: it stays until a newer one replaces it.
- [x] Where the status comes from, in this order:
  1. The turn starts: `thinking...`. The text is a theme token, `thinking_label` (default `thinking...`), so it can be replaced like `steering_label`.
  2. A `python` call starts: its `title` (below). A call without a title leaves the status as it was.
  3. A host function runs (`read`, `write`, `edit`, `grep`; section 13): a status with the argument, `reading PATH`, `writing PATH`, `editing PATH`, `searching PATTERN`. The texts are theme tokens with a `{}` for the argument (`read_label` = `reading {}`, `write_label`, `edit_label`, `grep_label`, and `sh_label`, `ask_label`, `docs_label` from below), so they are not hardcoded and can be translated.
  4. The next title or host function replaces it. The model's next turn (after the tool results) does not reset it to `thinking...`: the last status stays until a new one comes. A new user turn starts again from `thinking...`.
  - Labels of the other host functions: `sh` -> `running CMD` (the first line of the command, clipped; token `sh_label` = `running {}`), `ask` -> `asking user...` (`ask_label`), `docs` -> `reading docs...` (`docs_label`). `show` and `task_*` have no label and keep the current status.
  - A title is visible only while the code does not call a labelled function: the first `read(...)` in the call replaces it. That is intended (the more exact status wins).
- [x] New argument `title` in the `python` schema (`prompt.rs`): a short phrase about what this call does, e.g. `Running the tests`; one line, up to about 40 characters, in the user's language. It is required in the schema, but a call without it still runs (the status stays as it was).
- [x] `tui/status.rs` `working`: `{spinner} {time} · {status}`. The status is clipped to the width of the bar with `more`. `steering_label` still wins.
- [x] `Event::Status(String)` (or the status on `ToolStart`) so the TUI redraws at once; `Event::ToolStart` carries the title.
- [x] The counter of calls stays in `/usage` and in the `turn_end` record and hook (`st.calls`).
- [x] The title is part of the tool call arguments, so it stays in the history sent to the model; the chat shows the code, not the title. Hooks get it as the name `title` next to `input`.
- [x] Docs: `docs/agent.md` (the `python` arguments, the hook name `title`), the tokens `thinking_label`, `read_label`, `write_label`, `edit_label`, `grep_label`, `sh_label`, `ask_label`, `docs_label` in `docs/config.md`, and the prompt text about the `python` tool.
- [x] A test: the status is `thinking...` at the start, becomes the title at a `python` call, becomes `reading PATH` at a `read` and `running CMD` at an `sh`, and is still there after the call ends.

## 7. Mouse for the history, arrows for the input

Today `Up`/`Down` at the edge of the input and `PageUp`/`PageDown` scroll the chat (`keys.rs`). The history scrolls with the mouse only; the keyboard only moves in the input.

- [x] Today the wheel works through "alternate scroll" (`\x1b[?1007h` in `lib.rs` `setup`): the terminal turns the wheel into `Up`/`Down` keys, which is why those keys scroll the chat. Remove it (and `?1007l` in `restore`), otherwise the wheel would also move the cursor in the input.
- [x] Enable mouse capture (`EnableMouseCapture`) with the alternate screen and disable it on exit and on a panic (`lib.rs`, next to bracketed paste). Handle `TermEvent::Mouse` in the loop.
- [x] The wheel scrolls the chat by `scroll_step` lines per notch (the token stays, default 3). With a panel open (`/trace`, lists) the wheel scrolls the panel instead. Wheel events with a picker open do nothing.
- [x] Remove the chat scrolling from the keyboard: `PageUp`/`PageDown` and `Up`/`Down` at the first or last row of the input do nothing to the chat (at the edge the cursor stays). The `scroll_mark` (`↑`) still shows when the chat is scrolled up.
  - Panels keep their own keys (`panel_keys.rs`): they are lists and viewers, not the history. Decide whether to move them to the mouse too.
- [x] Moving in the input is by keys only: arrows, `Home`/`End`, `Ctrl`/`Alt` + arrows for words, `Ctrl+A`/`Ctrl+E`. `Up`/`Down` move by visual rows as now.
- [x] A test: `Up` on the first input row and `PageUp` leave `scroll` unchanged; a wheel event changes it by `scroll_step`.
- [x] Docs: the keys in `README.md` and `docs/agent.md`.
- [x] Note in the docs: with mouse capture the terminal's own text selection needs a modifier (`Shift` in most terminals, `Option` in iTerm2 and Terminal.app on macOS).

## 8. Click in the input to place the cursor

- [x] A left click on a text row of the input moves the cursor to the clicked cell. The click column is counted from the start of the text (`pad_x + frame.left`), by display width, so wide chars and atoms (`[Image]`, pasted labels) work. A click past the end of a row goes to the end of that row; a click on the frame, the status rows or the chat does nothing.
- [x] `draw.rs` already knows the screen rows of the visible editor part (`input_top`, `input_h`, `cy`) and `app.input_cols`. Save them in `App` on every draw (the clickable rectangle and `input_top`), so a mouse event can be mapped without redoing the layout.
- [x] `Editor::at(row, col, cols) -> usize` in `editor/layout.rs`, built on the same `placed` rows and `unit` widths as `vertical()`; the cursor never lands inside an atom or after a soft-wrapped row's last cell (same rule as `vertical()`).
- [x] After a click the token picker (`@` files, `/` commands) is refreshed like after a key, and a waiting steering message is not withdrawn: only the cursor moved, the text is the same.
- [x] A test: `Editor::at` on wrapped text, on a wide char and on an atom.
- [x] Docs: one line in `README.md`.

## 9. One design for every list

Today there are two looks. The pickers (`/models`, `/resume`, `/theme`, tasks, `ask`, `@` files, `/` commands) sit between the chat and the input and use `picker_*` tokens (title with a query, `picker_max_height`). `/trace` and the text views (`/usage`, a task's output, an opened trace item) are a panel over the bottom of the chat with `panel_*` tokens (a rule on top, title with `N/M`, `panel_max_height`). Decided: one new design and one set of tokens.

- [x] **The default look is the one `/trace` has now**: a title line with `N/M`, rows with the selected one filled in `picker_selected_*` colors, the whole list on the `panel_bg` background (`#000000`), `panel_max_height` (16) as the height, a rule (`rule`) on top that separates the list from the chat, and the selection mark `› `. The `list_*` defaults are what `/trace` uses now: `panel_*` for the list and the title, and the current `picker_selected_*` values for the selected row. The other defaults of `picker_*` (`picker_bg`, `picker_fg`, `picker_title`, `picker_detail`) are dropped.
- [x] A common list component in `crates/tui` (`list.rs`, replacing `picker_lines` in `draw.rs` and `panel_list.rs`): a rule on top, a title line (`title`, the query after ` › `, `N/M` on the right), the rows, the selected row filled. It is drawn **over the bottom of the chat**, like the panel now: the chat does not shrink, the input stays where it is.
- [x] It is used by all of them: `/models`, `/effort`, `/resume`, `/theme`, tasks, `ask`, `/trace`, and the token pickers (`@` files, `/` commands, prompts).
  - The token pickers keep filtering by the token under the cursor; the others keep their own query. The title line of a token picker can stay empty.
- [x] `/trace` gets the filter too: typing filters the rows by role and summary, like in `/models`. The columns (index, role, summary) are kept; the other lists keep label and detail.
- [x] The text views (`/usage`, a task's output, an opened trace item) use the same frame and tokens: a rule, a title with the scroll position `N/M`, the text.
- [x] One set of tokens, `list_*`, replaces `picker_*` and `panel_*` (no aliases, see section 2): `list_fg`, `list_bg`, `list_selected_fg`, `list_selected_bg`, `list_detail`, `list_title`, `list_pad_x`, `list_max_height`. `panel_pad_x` and `picker_pad_x` go; `list_pad_x` defaults to 0 as in section 4. The rule uses `rule`. Check `Theme::default()` and the themes in `themes.rs`.
- [x] `picker_selected_prefix` stays as the selection symbol, renamed `list_selected_prefix` for the same reason. `trace_role_width` stays.
- [x] Keys are the same everywhere: `Up`/`Down` move, `Enter` chooses (opens, in `/trace`), `Esc` closes (an opened item goes back to the list), typing filters. In a text view the wheel and `Up`/`Down` scroll (the wheel is section 7; the wheel in a list moves the selection).
- [x] Remove the dead code: `Panel` vs `Picker` split in `app.rs`, `panel_keys.rs`, `picker_rows`, the draw code for the picker between chat and input. Keep every file under 250 lines (`panel.rs` is 208 now).
- [x] A test: filtering `/trace` by a role keeps only those rows; the list is drawn over the chat and the input row does not move.
- [x] Docs: the tokens in `docs/config.md`.

## 10. `ask`: the user can always write an answer

Today a typed text is a filter. It becomes the answer only when it matches no option (`commands.rs`, `Kind::Ask`); if it matches one, Enter takes that option, so the user cannot answer with their own words.

- [x] An `ask` list always has a last row for a free answer, e.g. `Write your own answer…` (a theme token `ask_custom_label`, so the text is not hardcoded). It is there with any number of options, also with none (`options=[]` is a plain question).
- [x] Typing in an `ask` does not filter the options. The text goes to a visible answer line (the title line shows `question › text`, as the query does now), and Enter sends **the typed text** when there is some, otherwise the selected option. The options are only a quick choice.
  - Alternative to decide: the typed text still filters, and the `Write your own answer…` row stays at the bottom and sends the text as it is. Simpler to read, but needs a second keypress. The first variant is proposed.
- [x] An empty answer is not sent; Esc still dismisses (`answer(id, None)`; `ask()` raises `dismissed`; section 14 removes the non-blocking variant).
- [x] Multi-line answers are not needed: one line, no newline.
- [x] The answer returned to Python is the text as typed (not the option's label).
- [x] A test: a text that equals a prefix of an option is sent as typed; Enter on an empty line sends the selected option; with no options Enter sends the text.
- [x] Docs: `ask` in `docs/agent.md` and the prompt text ("the user can pick an option or write their own answer").

## 11. `task_run` runs Python code; `task_add` queues more code into it

Decided: a task is a **background Python REPL**. A function cannot be passed (Monty gives the host only a `<function f at 0xc>` string and a REPL lives on one thread), so `task_run` takes a string of code and runs it the way the agent's own REPL runs code.

- [x] `task_run(code, name=None)` replaces `task_run(cmd, name=None)`. It starts a **new interpreter on its own thread**: the same host functions (`sh`, `ask`, `show`, `task_*`, `docs`), the same `prelude`, the same filesystem access, the same `import` of config files. It does not see the variables of the agent's REPL. Shell commands are `task_run('sh("npm run dev")')`.
  - Same rules as the REPL: `print()` and the value of the last expression are written to the task log (`tasks/<id>.log`); an exception is written as a traceback.
  - No deadline (`tool_timeout` is for the `python` tool, not for tasks); `hook_timeout` does not apply either. `sh` inside a task has no timeout unless the code passes `timeout=`; if it does, section 12 does not apply there (no nested tasks): the process is killed.
- [x] The REPL of a task **stays alive after its code ends**: variables and functions stay, the status is `idle` (new status next to `running`, `exited`, `stopped`). `exited` now means the REPL was closed or crashed.
- [x] `task_add(id, code)` puts one more snippet **into the queue of that live REPL**, a steer for a background task: it runs after the current code ends, in the same interpreter (it sees the variables and functions of the earlier code), in the order of the calls. It returns at once with the position in the queue.
  - Output of each snippet goes to the same log, with a marker line before it so a reader can tell them apart.
  - An exception in one snippet does not close the REPL; the next one runs.
  - `task_add` on an `exited` or `stopped` task is an error with a clear text.
  - The queue is shown in `task_get` (`.queue`: how many snippets wait) and in `/tasks`.
- [x] `task_stop(id)` sets the cancel flag of the task's interpreter (like Esc for the REPL), drops the queue and closes the REPL. `Esc` of the user does **not** stop tasks, as now. Tasks are stopped at the end of the session and of `tuse -p`.
- [x] The object returned by `task_run`, `task_get`, `task_stop`, `task_list`: `id`, `name`, `code` (what was started; it was `cmd`), `status`, `exit` (was `code`, a name clash: the exit code of a REPL task is `1` after a crash and `0` after a normal close), `queue`, `output`. Decide the names before building.
- [x] The agent gets the `<notification>` when a snippet ends (the REPL goes `idle`), not only when the task exits, because the task does not exit by itself any more. The text has the task id and the last line of the output.
- [x] `crates/py`: a runtime for a long-lived interpreter that takes snippets from a channel and a cancel flag; `crates/core/src/tasks.rs` owns the thread, the queue, the log and the status. Keep the files under 250 lines (`tasks.rs` is ~190 now: split it).
- [x] A test: a task prints to its log; a variable set by the first snippet is visible to a snippet added with `task_add`; two added snippets run in order; an exception in one does not stop the next; `task_stop` ends a task running `while True: pass`.
- [x] Docs: `task_run` and `task_add` in `docs/agent.md` and in the prompt text, including that a task does not see the agent's variables.

## 12. `sh` does not die on a timeout; it becomes a task

Today a timeout kills the process group (`kill_group` in `crates/py/src/sh.rs`) and `sh` returns `timed_out=True`. Both timeouts apply: the `timeout` argument of `sh` and the deadline of the whole `python` call (`tool_timeout`, now 10 s). With the new 10 s default a normal build or test run would be killed. Instead it keeps running in the background.

After section 11 a task is a Python REPL, so a rescued process is a second kind of task: a **process task**. It has an id, a name, a log and a status, and it shows in the same lists, but it has no REPL: `task_add` on it is an error (`this task is a process, not a REPL`). Its `code` is the command line.

- [x] On either timeout `sh` does **not** kill the process. It hands the running process to the session's tasks (`Tasks::adopt(child, cmd, cwd, ...)` in `crates/core/src/tasks.rs`) and returns at once. The `Sh` object gets a new field `task` (the task id, `None` otherwise); `timed_out` stays `True`.
  - The output so far is in `.out`; the rest goes on in the task log. Today the reader thread fills an in-memory buffer; it must write to `tasks/<id>.log` from the start (or switch to it at the hand-over), so no output is lost and no second reader is needed.
  - The result text for the model says what happened, e.g. `[still running as task t3; use task_get("t3")]`.
  - The process task is stopped by `task_stop`, shown in `/tasks`, and the agent gets the `<notification>` when it exits (`exited`, with its exit code).
- [x] `tuse-py` knows nothing about tasks (lower layer): `Host` gets a method, e.g. `fn adopt(&self, child: Child, cmd: &str, cwd: &Path) -> Option<String>` (default `None`: the process is killed as now, so headless tests keep working). `crates/core` implements it with `Tasks`.
- [x] What still kills the process: **Esc / cancel** (the user said stop), the end of the session (tasks are stopped at exit, as now), a timeout in a **hook** (a hook must not leave tasks behind) and a `sh(..., timeout=)` inside a task's REPL (section 11: no nested tasks).
- [x] `tuse -p` (headless): process tasks are stopped at the end, like the others.
- [x] Docs: `sh` and the prompt text: "if a command runs longer than the timeout it keeps running as a task; its id is in `.task`". The timeout line of section 4 says the same.
- [x] A test: `sh("sleep 2; echo done", timeout=1)` returns `timed_out=True` with a task id, and the task ends with `done` in its log; Esc kills it.

## 13. One tool: `python`; `read`, `write` and `edit` become host functions

Today the model has four tools (`python`, `read`, `write`, `edit`; `grep` is coming). Decided: **one tool, `python`**, and the rest are host functions next to `sh`, `ask`, `show`, `task_*`. Then every call in the chat is Python code and looks like Python (`edit("path", old, new)`), there is one schema, one title (section 6), and one highlighter (section 5). The old plan of this section (show `edit("path")` for a separate tool) is dropped. Do this section before section 1.

- [x] Host functions (their code moves from `crates/core/src/tools/` to implementations behind `host.rs`; `host.rs` is ~190 lines, so put them in `host_fs.rs` and keep each file under 250 lines):
  - `read(path, offset=1, limit=2000)` returns the text with line numbers as a string. For an image (png, jpg, gif, webp) it returns a short text (`[image: name, 12 KB]`) and **attaches the image to the result of the call** (the `images` of `Record::Tool` already exist; `max_image_bytes` applies).
  - `write(path, content)` creates or replaces a file atomically, creates missing folders, returns a short confirmation.
  - `edit(path, old, new, replace_all=False)` replaces exact, unique text, keeps CRLF files working, returns a short confirmation (`edited path (1 replacement)`).
  - `grep(...)` from section 1.
  - Paths: absolute, `~/` or relative to the working directory, as now (`tools::resolve`).
- [x] Errors are Python exceptions, so the agent can catch them and the traceback says what happened: `old` not found or not unique is `ValueError`, a missing file is `FileNotFoundError`. Today the tools return an error result; there is no such thing for a host function.
- [x] **The diff is shown at the end of the call, only when there was an edit.** `edit` (and `write`, which replaces a file or creates one; decide: show it, it is the same code) does not return the diff to the model. It records it in the call; when the `python` call ends, the chat shows one diff block per changed file (`path`, then the colored lines, `diff_line` lines each), after the output.
  - New event `Event::Diff { id, path, diff }` (or the diffs on `ToolEnd`) and a `diffs` field on `Record::Tool` (empty by default, skipped when empty), so a resumed session and `/trace` show them too. The model gets the confirmations only: it already knows what it changed.
  - A change at the end of a long line is not visible when the line is clipped (`-` and `+` look the same). For a `-`/`+` pair cut a window around the first difference, with `more` on the cut side.
  - Tokens `diff_add`, `diff_del`, `diff_line` stay.
- [x] `read` and the 4 KB `max_output` (section 4): the text a call prints or returns is cut in the middle at `max_output`, and a page of 2000 lines never fits. Decide: `read` stops at `max_output` bytes (or `limit` lines, whichever comes first) and ends with `[cut at line N; use offset=N]`, instead of the middle cut.
- [x] `tools::NAMES` is `["python"]`; the schema in `prompt.rs` has one tool. The system prompt text (section 3) lists the functions: `sh`, `read`, `write`, `edit`, `grep`, `ask`, `show`, `task_*`, `docs`, and tells the agent to use them instead of `open`/`pathlib` for the files it changes (so the diff is shown), and that `edit` needs a `read` first.
- [x] `tool.rs` loses the branch for the other tools; `input_of`, `role_of` (`trace.rs`), `role_color` (`panel_list.rs`), `tool_view.rs` and `panel_list.rs` match only `python`. A tool name that is not `python` (a session written by 0.4.0 has `read`, `write`, `edit`) is shown generically as `name input`, with no special case (section 2: no migrations).
- [x] Hooks: `tool_start` / `tool_end` get `tool = "python"` always; `input` is the code. No new hook names. (An `edit`-specific hook is gone; say it in the changelog.)
- [x] The width of `tool_prefix` and the highlighting (section 5) apply to the code as it is; nothing is formatted into `name(args)` by the TUI any more.
- [x] A test: `edit` through `python` changes the file and records a diff; a failed `edit` raises; `read` of an image attaches it; `Record::Tool` with and without `diffs` round-trips.
- [x] Docs: the tool list in `README.md` and `docs/agent.md`, the host functions table, the changelog (a breaking change for hooks and for anything that looked at the tool name).

## 14. `ask` always blocks; the timeouts stop while it waits

Today `ask(question, options, wait=True)` can also return at once (`wait=False`) and deliver the answer later as a `<notification>`. Decided: `ask` **always blocks** until the user answers. A model that does not want to wait asks from a task (section 11: `task_run('ask(...)')`), and the answer is the task's output.

- [x] Remove `wait`: the signature is `ask(question, options=[])`. Remove the `wait=False` thread and the `<notification>` of an ask (`host.rs`); `ask` is one blocking call. Passing `wait` is a `TypeError` (section 2: no compatibility).
- [x] **While `ask` waits, the timeouts do not run.** Neither the deadline of the `python` call (`tool_timeout`) nor the `timeout` of a running `sh` counts the time the user needs. The user's time is not the agent's time.
  - `Ctx.deadline` is a fixed `Instant` today (`crates/py/src/ctx.rs`) and Monty gets `set_max_feed_duration(ctx.remaining())` once at the start of a call (`runtime.rs`, `drive`). Make the deadline movable (a `Cell`/atomic in `Ctx`, with `Ctx::pause()` returning a guard that adds the waited time to the deadline on drop), and give Monty the new remaining time after the call returns (`tracker_mut().set_max_feed_duration(...)` at the host boundary; **check that Monty counts from the start of the feed, so the already used time is not forgotten**, and fix it with a larger value if it does).
  - The wait is measured in `ask` in `host.rs` around `rx.recv...`, so nothing else has to know.
  - Esc still stops a waiting `ask` (`ctx.cancelled()`), as now; only the deadline is paused.
- [x] The status of the indicator while it waits is `asking user...` (section 6) and the spinner keeps going; the timer of the turn keeps counting (it is the user-facing time), only the deadline is paused.
- [x] In a task REPL (section 11) there is no deadline, so `ask` there simply blocks that task; the agent is not blocked. This is how to ask without blocking.
- [x] A test: a `python` call with a 1 s deadline that blocks 2 s in `ask` does not time out; the same without `ask` does; Esc during `ask` ends the call.
- [x] Docs: `ask` in `docs/agent.md` and the prompt text: "`ask` blocks until the user answers and does not count against the timeout; to ask without waiting, call it from `task_run`".

## 15. Remove two notices

The result of these two commands is already visible (the model in the status bar, the theme on the whole screen), so the chat line is noise.

- [x] `commands.rs:128`: remove `Model {name}, effort {v}.` after choosing the effort in `/models`. A failed save (`Can't save the model: ...`) stays as an error.
- [x] `pickers.rs:60`: remove `Theme {name}.` in `set_theme`. A failed save stays as an error.
- [x] Check that nothing else relies on them (tests in `keys_tests.rs`, docs).

## Release

- [x] `make check`, one commit per item.
- [x] Bump `version` in `Cargo.toml` and `release/VERSION` to 0.4.1, update `release/CHANGELOG.md`, tag `v0.4.1`.
