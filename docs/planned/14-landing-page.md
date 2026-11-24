# Planned: Tuse Landing Page on GitHub Pages (TODO #14)

Status: **Planned** (not yet implemented). Hosted on the same GitHub Pages site and shared `site/build.py` + `.github/workflows/pages.yml` pipeline specified in [04-github-pages-docs.md](04-github-pages-docs.md).

## Goal

Provide a concise, fast-loading product homepage at the root of Tuse's GitHub Pages site (`<base>`, e.g., `https://ethanhamilthon.github.io/tuse/`) that explains Tuse at a glance, shows a real terminal demo or screenshot, and makes installation, documentation (`<base>docs/`), and the GitHub repository immediately accessible on both desktop and mobile.

## Site Placement & Shared Pipeline

- Root URL `<base>` serves the landing page (`index.html`), rendered from `site/landing.html` by `site/build.py`.
- `<base>docs/` serves the documentation index rendered from `docs/index.md` (see [04-github-pages-docs.md](04-github-pages-docs.md)).
- Built, offline link/asset-checked, and deployed by the same GitHub Actions workflow as [04-github-pages-docs.md](04-github-pages-docs.md) (PR build + check on all PRs; deploy only from `refs/heads/main`, including `workflow_dispatch`; requires manual GitHub repo setting **Pages -> Source = GitHub Actions**).
- Every navigation link and asset reference on the landing page uses `{{base}}` (normalized with leading and trailing `/`, e.g., `/tuse/` on project GitHub Pages or `/` on a custom domain).

## Page Structure & Content

Keep the page crisp, scannable, and faithful to Tuse's minimal aesthetic and actual current capabilities:

1. **Top Navigation Bar** (shared layout shell with `/docs/`):
   - Brand (`Tuse`) linking to `{{base}}`
   - Links to **Docs** (`{{base}}docs/`), **Releases** (`https://github.com/ethanhamilthon/tuse/releases`), and **GitHub** (`https://github.com/ethanhamilthon/tuse`)
2. **Hero Section**:
   - Headline and one-sentence summary: minimal AI coding agent for the terminal — written in Rust, centered on one `lua` tool in a persistent Lua 5.4 VM, with both a full-screen TUI and `tuse json` editor protocol.
   - Copyable one-line install block showing the exact current installer command:
     ```sh
     curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
     ```
     With a lightweight copy button (gracefully degrades if JavaScript is disabled).
   - Primary buttons: **Read the Docs** (`{{base}}docs/`) and **View on GitHub**.
3. **Terminal Demo / Screenshot**:
   - **Decision:** Include a crisp static terminal screenshot (SVG or compressed WebP/PNG, < 300 KB) or lightweight self-contained animated SVG/WebP capture (never a multi-megabyte GIF) stored in `site/assets/` (published at `{{base}}assets/...`) with descriptive `alt` text.
   - Must show real Tuse TUI behavior: a prompt, syntax-highlighted `lua` tool call, colored file diff, and bottom model/context status bar rendered in Tuse's palette.
4. **Core Capabilities (At a Glance)**:
   - **One tool: `lua`** — persistent Lua 5.4 VM with `read`, `write`, `edit`, `append`, `delete`, `grep`, `sh`, `sleep`, and `spawn` coroutines; diffs shown automatically; no output truncation.
   - **TUI and `tuse json`** — full-screen terminal UI with Markdown, pickers (`/` commands, `#` prompts, `@` files), and a JSON-RPC 2.0 stdio mode sharing the same SQLite sessions (`~/.tuse/tuse.db`).
   - **Lua config & plugins** — plain Lua tables for hooks, custom slash commands, helpers, file openers, 10-token theme, and Git/local plugins.
   - **Lean context** — small built-in prompt, 50/25/25 compaction, and automatic retries.
   - **Decision:** Omit stale benchmark tables from the homepage (or include only a concise summary tied to current verified runs) so the landing page does not go stale across releases.
5. **Quick Start (3 Steps)**:
   - Install binary -> Connect a provider (`tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai`) -> Launch `tuse` and choose a model with `/models`, linking to `{{base}}docs/` for provider, config, and plugin details.
6. **Footer**:
   - Direct links to Docs (`{{base}}docs/`), GitHub repository, Releases / Changelog, and MIT License.

## Design, Performance & Mobile Requirements

- **Palette**: aligns with Tuse's 10-token dark theme (`#121212` bg, `#1E1E1E` surface, `#DEDEDE` fg, `#BDBDBD` dim, `#929292` muted, `#333333` border, `#BDDEAB` accent, `#94B979` ok, `#E27373` err, `#97BEDC` info) and shares `{{base}}assets/style.css` with the `/docs/` pages.
- **Zero heavy runtime**: plain HTML + CSS, system font stack + monospace for terminal/code elements, at most a tiny inline/local script for copy-to-clipboard. No external font CDN or tracking scripts required.
- **Metadata**: `<title>`, meta description, favicon (`{{base}}assets/...`), and OpenGraph tags (using canonical full HTTPS URLs or `{{base}}`-resolved asset URLs).
- **Responsive / mobile layout** (verified down to `375px` viewport width):
  - No horizontal page scrolling (`overflow-x` contained cleanly).
  - Install command block scrolls horizontally inside its code box (or wraps cleanly) without stretching the viewport.
  - Feature grid stacks vertically into a single column.
  - Terminal demo scales proportionally within the viewport width without clipping text unreadably.
  - Navigation links wrap or collapse cleanly with comfortable tap targets.

## Error Handling & Edge Cases

- If JavaScript is disabled or clipboard API is unavailable, the install command remains plain selectable text and the copy button hides or fails gracefully without errors.
- Root-absolute URLs (`/docs/`, `/assets/...`) are forbidden in templates so hosting under a repository subpath (`/tuse/`) never returns 404 for CSS, demo media, or the Docs link.
- **Do not add a `../` product-home link in `docs/index.md`:** on GitHub, `../` inside `docs/index.md` points to the repository root rather than the landing page. Navigation from `/docs/` back to the landing page (`{{base}}`) lives exclusively in the shared Pages top-bar template rendered by `site/build.py`.

## Baseline Docs to Update Before Implementation (Handoff Gate)

- Canonical `docs/index.md` needs no `../` homepage link (the Pages top bar handles navigation between `<base>` and `<base>docs/`).
- Once #4 and #14 are live on GitHub Pages, proceed to [13-readme-rewrite.md](13-readme-rewrite.md) to add the live landing page and docs URLs to `README.md`.

## Acceptance Tests

1. **Root vs `/docs/` routing**: built site serves the landing page at `<out>/index.html` (`<base>`) and the docs index at `<out>/docs/index.html` (`<base>docs/`) in one artifact.
2. **Build-time base-path & asset check**: `site/build.py --check` verifies all internal links (`<base>docs/`) and all local assets (CSS, favicon, terminal demo asset) resolve under both `--base /tuse/` and `--base /` with zero root-absolute `/docs/` or `/assets/` leaks.
3. **Command accuracy**: install command and `tuse connect` quick-start example match current CLI behavior and `release/install.sh`.
4. **Mobile layout verification**: render landing page at desktop (`1280px`) and mobile (`375px`) viewports and confirm zero horizontal overflow, readable install block, responsive demo image, and clickable navigation/CTAs.
5. **No-JS resilience**: page renders completely and all links and install commands remain usable with JavaScript disabled.
6. **Live post-deploy verification**: after deploying `main`, verify HTTP 200 on the live `<site>/`, CSS, demo asset, and navigation from `<site>/` -> `<site>/docs/` -> `<site>/`.
