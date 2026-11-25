# Add docs

## Why
The old `docs/` (22 pages), `site/` (custom Python generator) and `AGENTS.md` were removed. The build still needs `docs/index.md` and `docs/system-prompt.md`, and the Pages workflow still calls `site/build.py`. We want one small set of pages that both the website and the agent (`docs()`) read word for word, with the system prompt shown exactly as the agent gets it.

## What Changes
- New `docs/` with exactly six pages: `usage.md`, `connect.md`, `config.md`, `lua.md`, `session.md`, `json.md`. No `index.md`.
- `docs()` lists `readme` (the root `README.md`) plus these six topics, and `docs("<topic>")` returns the page. The old topics (`functions`, `tool`, `commands`, `hooks`, ...) are gone. **BREAKING** for scripts that call old topics.
- The system prompt template moves out of `docs/` back into the code (`crates/core/src/prompt.md`, loaded with `include_str!`). The placeholder mechanism stays as it is.
- `docs/lua.md` ends with a section `## What the agent sees`. It holds the full default system prompt and the `lua` tool description, verbatim. A test makes sure it matches what `system_prompt` and `tools` produce for the default config. At runtime `docs("lua")` renders this section for the session's own config, so disabled functions are left out.
- The site is built with mdBook: `README.md` is the first page, then the six pages. `book.toml` lives at the repo root, and `make site` puts the book source together under `target/`. The `site/` folder is not brought back.
- The Pages workflow installs mdBook and runs `make site`.
- Tests that read old topics move to the new ones: `functions`/`tool` → `lua`, `commands` → `usage`.

## Impact
- Specs: `docs` (new), `docs-site` (new).
- Code: `crates/core/build.rs`, `crates/core/src/docs.rs`, `crates/core/src/prompt.rs`, new `crates/core/src/prompt.md`, `crates/cli/src/main.rs`, tests in `core`, `tui`, `cli`, `rpc`.
- Repo: `docs/*.md`, `book.toml`, `Makefile`, `.github/workflows/pages.yml`. `AGENTS.md` and `site/` stay deleted.
- The README is written by the maintainer; it becomes the `readme` topic and the first site page.
