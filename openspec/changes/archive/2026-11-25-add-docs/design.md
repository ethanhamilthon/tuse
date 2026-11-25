# Design

## Topics
Fixed list, in this order (same order on the site):

| topic | page | content |
|---|---|---|
| readme | README.md | written by the maintainer; summary fixed as "Overview, install, and quick start" |
| usage | docs/usage.md | install/update, TUI (keys, pickers, steering, `#` prompts, `@` files, images), CLI subcommands, slash commands, custom commands |
| connect | docs/connect.md | `tuse connect`/`disconnect`/`providers`, the four API types, keys from env/stdin, models and discovery, reasoning effort, `/models`, cost and usage |
| config | docs/config.md | config lookup and env vars, `tuse init`, `require` resolution, every schema key, `functions` overrides/disable, `systems`, `prompts`, `commands`, theme and `settings.ui`, `/reload`, `/config` |
| lua | docs/lua.md | VM and limits, the `lua` tool contract, every host function, tasks, hooks, `require("tuse")`, then **What the agent sees** |
| session | docs/session.md | SQLite store, locks, resume, the agent loop, cancel/steer, compaction, `session()` fields, trace |
| json | docs/json.md | `tuse json`, handshake, every method, events, UI requests |

`build.rs` has this list hard-coded. It fails if a listed page is missing or if `docs/` contains any other `.md` file. A topic's summary is the first paragraph after the page's `# Title`, so pages stay plain Markdown without front matter.

`docs()` with no topic prints `- <topic> — <summary>` lines, then the `Call docs("<topic>")` hint. Lookup stays trimmed and case-insensitive and still accepts a `.md` suffix.

## Prompt in code, mirrored in docs
- `docs/system-prompt.md` turns into `crates/core/src/prompt.md`. The block and function-line format stays the same, so `prompt.rs` only needs its `include_str!` path changed and the `docs-doc`/`tool-doc` blocks dropped. `docs-only` lines stay, because `docs("lua")` lists them under Functions.
- `docs/lua.md` ends with:

  ```
  ## What the agent sees
  <short note: this is the default; disabled functions drop out; `systems` strings follow>
  ### System prompt
  ```text
  ...system_prompt(Config::default(), false)...
  ```
  ### Tool description
  ```text
  ...tools(default)[0].description...
  ```
  ```
- A unit test rebuilds this tail from the code and compares it with the file. If they differ, the test prints the expected text so it can be pasted in. The site shows the file as it is, so it is always 1-1 with the agent's prompt.
- At runtime `docs("lua")` cuts the page at `## What the agent sees` and adds the same tail rendered with the session's disabled functions. The "Functions" reference in the middle of `lua.md` is hand-written prose. Disabled functions are reported by the `Disabled in this session: ...` notice; the prose itself is not filtered.

## Site
- `book.toml` at the root: title `Tuse`, `src = "target/book-src"`, output `target/book`, no search index tweaks, default theme.
- `make site` copies `README.md` → `target/book-src/README.md` and `docs/*.md` → `target/book-src/`, writes a `SUMMARY.md` (`[Tuse](README.md)` and then the six pages in topic order, titles taken from each `# Title`), and runs `mdbook build`. Nothing is generated inside the tracked tree.
- Cross-page links in docs use `usage.md#...`. These work on the site, and the agent can read them as topic names.
- The Pages workflow installs a pinned mdBook release binary, runs `make site`, and uploads `target/book`.
