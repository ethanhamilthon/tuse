# Planned: Rewrite `README.md` (TODO #13)

Status: **Planned** (not yet implemented). `README.md` is currently unchanged until this spec is executed.

**Prerequisite:** **Decision:** Land and deploy [04-github-pages-docs.md](04-github-pages-docs.md) and [14-landing-page.md](14-landing-page.md) (with GitHub Pages enabled on `main`) **before** implementing #13, so the landing page and published `/docs/` links in `README.md` are live immediately and never ship as dead links.

## Goal

Turn `README.md` into a concise front door for GitHub visitors—covering what Tuse is, why it is useful, how to install it, and how to start a first session—while leaving detailed setup, keybindings, config references, and protocol documentation in `docs/`.

## Target Content & Structure

Target length: roughly 60–100 lines, readable in under a minute.

1. **Heading & Summary (`# Tuse`)**:
   - Must begin with `# Tuse` (required by the built-in `docs("README.md")` topic contract).
   - One-to-two-sentence description of Tuse: minimal AI coding agent for the terminal written in Rust + Crossterm, centered on a single `lua` tool in a persistent Lua 5.4 VM, with both a TUI and JSON-RPC mode.
   - Compact link line directly under the summary:
     - Landing page (`https://ethanhamilthon.github.io/tuse/`)
     - Documentation (`https://ethanhamilthon.github.io/tuse/docs/` and local [docs/index.md](../index.md))
     - Releases (`https://github.com/ethanhamilthon/tuse/releases` and `release/CHANGELOG.md`)
2. **Why Tuse** (4–6 short bullets):
   - Single `lua` tool in a persistent Lua 5.4 VM (`read`, `write`, `edit`, `append`, `delete`, `grep`, `sh`, `sleep`, `spawn` background coroutines, live diffs, no output cap; one VM per session).
   - Full-screen TUI (Markdown, `/` commands, `#` prompts, `@` files, image paste) plus `tuse json` JSON-RPC 2.0 over stdio sharing SQLite sessions (`~/.tuse/tuse.db`).
   - Plain Lua config and plugins (hooks, custom slash commands, file openers, 10-token theme, live `/reload`).
   - Lean context with 50/25/25 compaction and automatic retries.
   - Single binary with built-in defaults (`tuse update` never touches your config).
3. **Install**:
   - One-line installer command (`curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh`).
   - One sentence mentioning `tuse update`, building from source (`cargo install --locked --git https://github.com/ethanhamilthon/tuse tuse` or `make install`), and linking to [../install.md](../install.md) and [../env.md](../env.md) for version pinning and custom install directories.
   - Note: if [11-installer-first-run.md](11-installer-first-run.md) lands before #13, update the install sentence to match the interactive/non-interactive installer behavior accurately.
4. **Quick Start**:
   - Step 1: Connect a provider (verify exact flags match current CLI—e.g. `tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai`; include `--api` note if [02-api-types.md](02-api-types.md) has already landed).
   - Step 2: Run `tuse`, pick a model and effort with `/models`, and start chatting.
   - One-line pointers to `tuse --resume ID` and `tuse json`.
5. **Documentation**:
   - Short pointer to the published docs site and local [docs/index.md](../index.md) as the single entry point to all guides and references (replacing the duplicated reference tables and config tutorials currently in `README.md`).
6. **Development & License**:
   - Brief pointer to `AGENTS.md`, `make check`, supported release platforms (macOS 11+, Linux glibc 2.35+), and `MIT` license.

## What to Remove from `README.md` (Now Owned by `docs/`)

Move/keep these details strictly in `docs/` rather than duplicating them in `README.md`:

- Full 18-row TUI keybindings table
- Exhaustive CLI flag / subcommand block and slash-command catalog
- Detailed `lua` host function descriptions and `#`/`@` hidden-context mechanics
- Large Lua configuration sample (`open`, `functions`, `hooks`)
- **Decision:** Remove the outdated 0.5.1 benchmark table from `README.md` (or move refreshed benchmarks to a dedicated document/landing section later) so `README.md` stays short and does not advertise stale versions or model names.

## Edge Cases & Compatibility

- `README.md` is embedded in the binary as the `docs("README.md")` / `docs("readme")` Lua documentation topic.
  - Keep `# Tuse` as the first line so existing topic contract tests keep passing.
  - Update the `README.md` topic summary text shown by `docs()` (currently mentions "hotkeys, config, benchmark (long; read in slices)") to reflect the concise overview.
- Verify every command, flag, slash command, path, and relative link in `README.md` against current binary behavior at implementation time (including fixing any stale claims such as per-process vs. per-session VM lifetime).

## Baseline Docs to Update Before Implementation (Handoff Gate)

Before handing off implementation (per TODO #3):

- Confirm [../index.md](../index.md) covers all topics moved out of `README.md` so no information is lost.
- Review the `README.md` rewrite diff and updated `docs()` topic summary for `README.md` together.

## Acceptance Tests

1. **Structure & conciseness**: `README.md` begins with `# Tuse`, stays concise (~60–100 lines), includes what Tuse is, why it is useful, install instructions, first-session quick start, and links to the landing page, docs, and releases, without duplicating full TUI hotkey tables or config/function references.
2. **Command accuracy**: every CLI command, flag, and slash command shown in `README.md` matches the current binary's `--help` and documented behavior.
3. **Link validity**: all local links (`docs/index.md`, `docs/install.md`, `docs/env.md`, `AGENTS.md`, `release/CHANGELOG.md`) exist in the repository, and the live external links (`https://ethanhamilthon.github.io/tuse/`, `https://ethanhamilthon.github.io/tuse/docs/`, `https://github.com/ethanhamilthon/tuse/releases`) return HTTP 200 (prerequisite #4/#14 deployed).
4. **Test suite**: `make check` passes (including the `docs("README.md")` topic test).
