# TODO 0.5.1

## 1. Topic-based `docs(topic=None)` and split `docs/*.md`

Split `docs/agent.md` and `docs/config.md` into focused topic files so each topic fits under the default 4 KB tool output limit, and make `docs()` return a table of contents while `docs(topic)` returns the requested topic.

### 1.1. Split `docs/*.md` into topic files
Replace the two large files (`docs/agent.md`, `docs/config.md`) with smaller, focused markdown files (each < 4 KB):
- `docs/tool.md` (`tool`) — Python REPL tool, limits, timeout, imports.
- `docs/functions.md` (`functions`) — Built-in Python functions (`read`, `write`, `edit`, `grep`, `sh`, `ask`, `show`, `task_*`, `docs`).
- `docs/commands.md` (`commands`) — Built-in slash commands, CLI flags, and custom user commands.
- `docs/env.md` (`env`) — Environment variables and config file lookup.
- `docs/config.md` (`config`) — Config overview, merging/expansion, keys, compaction, and files.
- `docs/hooks.md` (`hooks`) — Hooks (`session_start`, `session_end`, `turn_start`, `turn_end`, `tool_start`, `tool_end`, `error`) and `prelude`.
- `docs/theme.md` (`theme`) — 10-color `themes` and `settings.ui`.

### 1.2. `docs(topic=None)` in `crates/core/src/docs.rs` & `crates/core/src/host.rs`
- Define a static table of topics `(name, summary, content)` embedded via `include_str!`.
- Change `crates/core/src/docs.rs` signature to `pub fn docs(topic: Option<&str>) -> String`:
  - `None` (or empty): return a formatted list of available topics (`<name> — <summary>`) with a hint to call `docs("<topic>")`.
  - `Some(name)` matching a topic (case-insensitive / trimmed): return that topic's markdown content.
  - `Some(unknown)`: return an error string (`Unknown docs topic "<unknown>". Available topics: ...`) with the topic list (do not raise an exception).
- In `crates/core/src/host.rs`, parse optional `a.opt_str(0, "topic")?` and pass it to `crate::docs::docs(topic.as_deref())`.

### 1.3. Prompts, docs & references
- Update `crates/core/prompts/system.md`: document `docs(topic=None)` (call `docs()` for topics, `docs("<topic>")` to read one).
- Update `README.md` and `release/main.yaml` links to `docs/` if needed.
- Add unit tests in `crates/core/src/docs.rs` covering `docs(None)`, valid topic lookup, and unknown topic response.

## 2. Reuse HTTP connection pool (`ureq::Agent`) across steps

Currently `crates/core/src/turn.rs` calls `Client::new(&conn.base_url, &conn.api_key)` on every `step()`, which creates a brand-new `ureq::Agent` and forces a fresh TCP + TLS handshake on every tool round-trip.

- **`crates/api/src/client.rs`**:
  - Add `pub fn default_agent() -> ureq::Agent` (with the existing 30s connect / 600s read timeouts) and `Client::with_agent(agent: ureq::Agent, base_url: &str, api_key: &str) -> Self`. Keep `Client::new` delegating to `with_agent(default_agent(), ...)`.
- **`crates/core/src/session.rs` & `crates/core/src/turn.rs`**:
  - Store a single `http: ureq::Agent` on `Session` (`ureq::Agent` is `Clone + Send + Sync` and shares its connection pool across clones).
  - In `turn.rs`, construct the client with `Client::with_agent(self.http.clone(), &conn.base_url, &conn.api_key)` so HTTP keep-alive and TLS sessions are reused across all steps in a session (even when switching models/providers).

## 3. Reduce Python interpreter thread stack size

In `crates/py/src/runtime.rs`, `Python::run` spawns a scoped OS thread with `const STACK: usize = 128 * 1024 * 1024` (128 MB) on every invocation (including every hook and prelude).

- Lower `STACK` in `crates/py/src/runtime.rs` from `128 * 1024 * 1024` to `16 * 1024 * 1024` (16 MB — plenty for deep recursion while reserving 8x less virtual address space per call).

## 4. TUI performance & rendering optimizations

### 4.1. Visible-window chat slicing (`crates/tui/src/chat.rs`, `crates/tui/src/draw.rs`)
Currently `Chat::lines(width, p)` clones every `Line` (and every `Span` + `String`) for the entire session history on every frame, even though `draw()` only displays `chat_h` (~40) lines.
- Replace `Chat::lines` with a visible-window helper, e.g. `Chat::window(&mut self, width: usize, p: &Palette, scroll: usize, height: usize) -> (Vec<Line>, usize)` (returns the visible lines and the clamped `scroll`):
  - Ensure each entry's cache is populated for `width`.
  - Compute total rendered line count (including `p.ui.message_gap` between non-empty entries) without cloning.
  - Clamp `scroll` to `total.saturating_sub(height)`, compute `start..end`, and clone only the lines (and gaps) that intersect `start..end`.
- Update `crates/tui/src/draw.rs` to call `app.chat.window(...)`.

### 4.2. Throttle streaming redraws (`crates/tui/src/lib.rs`)
Currently every incoming `Event::Text` / `Event::Reasoning` triggers a full `draw()` (re-parsing the growing assistant Markdown on every chunk).
- Track `last_draw: Instant` in `main_loop`.
- When `dirty` is set solely by streaming events (`Event::Text` / `Event::Reasoning`), wait until `last_draw.elapsed() >= Duration::from_millis(style::FRAME_MS)` before calling `draw()` (user input events still draw immediately so typing/scrolling stays snappy).
- Drain all queued `app.events.try_recv()` before drawing so bursts of chunks coalesce into a single re-parse/render.

### 4.3. Line-level diff & minimal ANSI style transitions (`crates/tui/src/draw.rs`, `crates/tui/src/text.rs`)
Currently `draw()` rewrites all `h` terminal rows on every frame and `put()` emits `SetAttribute(Reset)` + `ResetColor` + `SetForegroundColor` + `SetBackgroundColor` before every single `Span`.
- Derive `PartialEq, Eq` on `Style` and `Span` (`crates/tui/src/style.rs`, `crates/tui/src/text.rs`).
- Store previous frame state (e.g. `prev_screen: Vec<Line>`, `prev_size: (usize, usize)`, `prev_palette_gen: u64` in a small `Renderer` struct in `draw.rs` or on `App`):
  - On size or palette change, clear `prev_screen` to force a full redraw.
  - Otherwise, for each row `y`, skip terminal output if `prev_screen.get(y) == Some(&screen[y])`.
  - Save `screen` into `prev_screen` after flushing.
- Within a drawn row, track `cur: Option<Style>` (resolved against `base`) and emit ANSI attribute/color changes only when a span's effective style differs from `cur`; emit a single `SetAttribute(Reset)` + `ResetColor` at the end of the row.

### 4.4. Event loop: skip unhandled events & cache terminal size (`crates/tui/src/lib.rs`, `crates/tui/src/mouse.rs`)
Currently `main_loop` sets `dirty = true` unconditionally after any `TermEvent` (including `MouseEventKind::Moved`, `MouseEventKind::Up`, `FocusGained`) and calls `terminal::size()` twice per iteration.
- Change `App::mouse(&mut self, m: MouseEvent) -> bool` to return `true` only when the event was actually handled (`ScrollUp`, `ScrollDown`, `Down(Left)`).
- Cache `(w, h)` from `terminal::size()` once before the loop and update it on `TermEvent::Resize(nw, nh)` (removing both per-iteration `terminal::size()` calls).
- Set `dirty = true` only for `TermEvent::Key(_)`, `TermEvent::Paste(_)`, `TermEvent::Resize(_, _)`, and handled `TermEvent::Mouse(_)` events.

## 5. Invalidate chat render cache on `/theme` change

Confirmed bug in `crates/tui/src/pickers.rs`: `App::set_theme` updates `self.palette = Palette::from_config(...)`, but does not invalidate `self.chat`'s per-entry `(width, Vec<Line>)` cache — so existing messages remain rendered in the old theme's colors until the terminal is resized.

- Add `Chat::invalidate(&mut self)` in `crates/tui/src/chat.rs` that clears all cached `Option<(usize, Vec<Line>)>` entries (`for (_, c) in &mut self.entries { *c = None; }`).
- Track `palette_gen: u64` on `App` (incremented on `/theme` and `/reload`) so the diffing renderer also forces a full screen redraw.
- Call `self.chat.invalidate()` and bump `self.palette_gen` in `App::set_theme` (`crates/tui/src/pickers.rs`) and on `/reload` (`crates/tui/src/commands.rs`).

## 6. Keep working indicator `title` until the next `title` arrives

Currently `crates/core/src/tool.rs` clears `st.status` and emits `Event::Status("")` immediately when each tool call finishes, causing the `title` in the working indicator to flash briefly and disappear on fast tool calls.

- **`crates/core/src/tool.rs`**: remove `st.status.clear();` and `self.emit(Event::Status(String::new()));` on tool end so the active `title` remains shown in the working indicator until a subsequent tool call sets a new non-empty `title` (or the turn ends; `crates/core/src/control.rs` still resets `st.status = String::new()` at the start of each new user turn).
- **`crates/core/tests/agent_flow.rs`**: update the status assertion from `["Testing the flow", ""]` to `["Testing the flow"]`.

## 7. Show `old -> new` version on `tuse update`

Currently `crates/cli/src/update.rs` prints only `Updated tuse` after installing the new binary.

- **`crates/cli/src/update.rs`**:
  - Capture `old = env!("CARGO_PKG_VERSION")`.
  - After `install(...)` replaces `exe`, run `<exe> --version` and parse the new version (`out.trim().strip_prefix("tuse ").unwrap_or(out.trim())`).
  - Print `{old} -> {new}` to stdout (e.g. `0.5.1 -> 0.5.2`); if running `--version` fails, fall back to `Updated tuse`.

## 8. Markdown & Python syntax highlighting in `/trace` panel

Currently `open_trace_item` in `crates/tui/src/commands.rs` opens every `/trace` entry with plain `Panel::new(...)`.

- **`crates/tui/src/panel.rs`**:
  - Extend `Body` with `Markdown(String)` and `Python(String)` variants (plus `Panel::markdown(title, text)` and `Panel::python(title, code)` constructors).
  - In `Panel::wrapped(width, p)`:
    - `Body::Markdown(text)` -> `crate::markdown::render(text, width, p)` (re-wraps cleanly on terminal resize).
    - `Body::Python(code)` -> `crate::syntax::highlight(&crate::render::clean(code), Style::fg(p.fg), p)` wrapped to `width` with `wrap(...)` (same highlighting as Python calls in chat).
  - Add `Panel::invalidate(&mut self)` (clears `self.cache`) and call it on `/theme` change if a panel is open.
- **`crates/tui/src/commands.rs` (`open_trace_item`)**:
  - `"user" | "agent" | "system"` -> `Panel::markdown(title, text.clone())`.
  - `"python"` -> `Panel::python(title, text.clone())`.
  - Other roles (`"thinking"`, `"output"`, `"notice"`, `"error"`, etc.) -> `Panel::new(title, text.clone())`.

## 9. `settings.ui.fold_strings`: fold multiline Python strings in chat

Add a `fold_strings: bool` setting (default `true`) to `settings.ui` that collapses multiline triple-quoted Python strings (`"""..."""`, `'''...'''`, including `r`/`f`/`b`/`u` prefixes) to a single line in chat tool calls, keeping `/trace` unfolded.

- **`crates/config/src/theme.rs`**:
  - Add `pub fold_strings: bool` to `UiSettings` with default `true`.
- **`crates/tui/src/syntax.rs` (or a small helper called from `crates/tui/src/tool_view.rs`)**:
  - Add `fold_multiline_strings(code: &str) -> String`:
    - Scans Python source (respecting comments, single-quoted strings, and escape sequences so quotes inside `# ...` or `'...'` are not misinterpreted).
    - When a triple-quoted string (`"""` or `'''`) spans multiple lines:
      - Keeps the opening quote (with any prefix) + the first non-empty line of its body (or the text on the opening line if non-empty) + `…` + the closing triple quote, preserving any code after the closing quote on the same line.
      - Examples:
        - `write("a.txt", """line 1\nline 2\nline 3""")` → `write("a.txt", """line 1…""")`
        - `write("a.txt", """\nline 1\nline 2\n""")` → `write("a.txt", """line 1…""")`
        - `edit("a.txt", """old 1\nold 2""", """new 1\nnew 2""")` → `edit("a.txt", """old 1…""", """new 1…""")`
      - If a triple-quoted string is unclosed (e.g. while streaming), keeps the first line + `…` without a closing quote.
    - Leaves single-line strings untouched.
- **`crates/tui/src/tool_view.rs`**:
  - In `source(...)`, when `p.ui.fold_strings` is `true`, pass the cleaned code through `fold_multiline_strings` before highlighting/wrapping (applies only in chat, not in `/trace`).
- **Docs & tests**:
  - Document `fold_strings` (default `true`) in `docs/theme.md` under `settings.ui`.
  - Add unit tests for `fold_multiline_strings` (basic multiline, leading newline, multiple strings in one call like `edit(...)`, escaped quotes, quotes inside comments, unclosed streaming string, single-line triple-quoted string left unchanged).
