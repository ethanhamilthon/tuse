# Planned: Publish Docs on GitHub Pages (TODO #4)

Status: **Planned** (not yet implemented). Shares one GitHub Pages site and build/deploy pipeline with [14-landing-page.md](14-landing-page.md).

## Goal

Publish Tuse's Markdown documentation from `docs/` directly to GitHub Pages without maintaining a second copy of the Markdown files, serving the docs index at `/docs/` on the same site whose root `/` hosts the product landing page ([14-landing-page.md](14-landing-page.md)). Validate the build and all internal links on every pull request and deploy automatically only from the default branch.

## Shared Site URL Structure

All URLs are relative to the normalized site base path `BASE_URL` (leading and trailing `/`: `/tuse/` for `https://ethanhamilthon.github.io/tuse/`, or `/` if a custom domain is later configured):

| Built URL | Source |
| --- | --- |
| `<base>` | Product landing page (see [14-landing-page.md](14-landing-page.md)) |
| `<base>docs/` | Rendered from `docs/index.md` (`docs/index.html`) |
| `<base>docs/<name>.html` | Rendered directly from `docs/<name>.md` |
| `<base>docs/planned/<name>.html` | Rendered directly from `docs/planned/<name>.md` |
| `<base>assets/...` | Copied from `site/assets/...` (shared CSS, icons, demo media) |

- **Decision:** Render `docs/planned/*.md` under `<base>docs/planned/` alongside baseline docs (with a visual "Planned" badge and breadcrumb) rather than excluding them, so every relative link in `docs/index.md` resolves identically on GitHub and on GitHub Pages with zero special-case link stripping.
- **Decision (redirects when published plans move):** Maintain a lightweight `site/redirects.json` mapping retired paths to their new canonical targets (emitting static HTML `<meta http-equiv="refresh">` + `<link rel="canonical">` redirect stubs). Whenever a published planned document moves to a baseline path—such as [10-setup-guide.md](10-setup-guide.md) -> `docs/setup.html` or [10-plugin-authoring-guide.md](10-plugin-authoring-guide.md) -> `docs/plugin-authoring.html`—add an entry to `site/redirects.json` so existing external bookmarks never 404.
- **Decision:** Emit clean static HTML files (`index.html`, `docs/index.html`, `docs/<name>.html`, `docs/planned/<name>.html`) with zero client-side SPA routing so direct URLs, curl checks, browser navigation, and `#heading` anchors work without JavaScript.

## Single Builder, Asset Copy & Link Rewrite Contract

- **Decision (one concrete builder):** Use a single minimal Python 3 static site generator in `site/build.py` (with a pinned GFM Markdown renderer such as `markdown-it-py` + `mdit-py-plugins` locked in `site/requirements.txt` with hashes, kept outside the Rust workspace and 15,000-LOC Rust crates budget) plus a `--check` verification mode in the same tool (`python3 site/build.py --base /tuse/ --out dist --check`).
- **Source & asset copying**:
  1. Reads `docs/**/*.md` directly at build time. No generated HTML or duplicated Markdown is committed to `main` or to a `gh-pages` branch.
  2. Copies `site/assets/**` verbatim to `<out>/assets/**`.
  3. Copies any non-Markdown static files under `docs/**` (e.g., diagrams/images if added later) verbatim to `<out>/docs/**` preserving relative paths.
  4. Renders `site/landing.html` (or template) to `<out>/index.html` and every `docs/**/*.md` to `<out>/docs/**/*.html` using shared HTML layout templates in `site/templates/`.
- **URL & link rewriting**:
  - Layout templates use `{{base}}` (normalized `BASE_URL`) for site-wide links and assets (`{{base}}`, `{{base}}docs/`, `{{base}}assets/style.css`, etc.); root-absolute `/docs/` or `/assets/` literals are forbidden in templates.
  - Canonical Markdown files in `docs/` remain authored with standard relative `.md` links (for example, `[config.md](config.md)`, `[theme.md](theme.md#theme)`, `[02-api-types.md](planned/02-api-types.md)`, `[../providers.md](../providers.md)`), so browsing `docs/` directly on GitHub always works.
  - During Markdown-to-HTML rendering, relative `.md` links rewrite to built HTML targets while preserving `#fragment` anchors:
    - `index.md` / `./index.md` -> `./` (and `../index.md` from `docs/planned/` -> `../`)
    - `<path>.md` -> `<path>.html`
    - `<path>.md#frag` -> `<path>.html#frag`
  - External `http(s)://` and `mailto:` links pass through unchanged.
  - Relative links escaping `docs/` (such as `../README.md` or `../AGENTS.md`) are forbidden in canonical `docs/` Markdown (detected and failed by `--check`); reference repository-root files using full GitHub URLs if needed.
  - Headings generate deterministic GitHub-compatible lowercase hyphenated `id` slugs (with `-1`, `-2` suffixes on duplicate headings) so `#fragment` anchors resolve identically on GitHub and GitHub Pages.
- **Docs layout**:
  - Uses Tuse's 10-token dark palette (`#121212` bg, `#1E1E1E` surface, `#DEDEDE` fg, `#BDBDBD` dim, `#929292` muted, `#333333` border, `#BDDEAB` accent, `#94B979` ok, `#E27373` err, `#97BEDC` info), sharing one stylesheet with the landing page.
  - Top bar rendered by the Pages layout template links Home (`{{base}}`), Docs (`{{base}}docs/`), Releases (`https://github.com/ethanhamilthon/tuse/releases`), and GitHub (`https://github.com/ethanhamilthon/tuse`).
  - Docs navigation sidebar built from `docs/index.md` collapses cleanly on mobile viewports.

## Build-Time Checks vs. Live URL Checks

Separate checks into two explicit stages:

1. **Build-time checks (CI on every PR and `main`, strictly offline, `--check`)**:
   - Every built internal `<a href>` resolves to an existing built HTML file, directory index, or redirect stub inside `<out>` under `BASE_URL`.
   - Every `#fragment` on internal links matches an actual `id` attribute in the target built HTML file.
   - Every local `<link href>`, `<script src>`, `<img src>`, and local OpenGraph asset resolves to an existing file in `<out>` under `BASE_URL`, and zero unprefixed root-absolute `/docs/` or `/assets/` paths exist when `BASE_URL=/tuse/`.
   - Every `.md` file under `docs/` is rendered and reachable from `<out>/docs/index.html` (no orphan unlinked pages).
   - No internal Markdown link escapes `docs/` (e.g., `../README.md`).
2. **Live post-deploy checks (after `main` deploys to GitHub Pages; never run as a flaky PR gate)**:
   - Verify HTTP 200 on the deployed `<site>/`, `<site>/docs/`, sample baseline and planned docs pages, and static assets (CSS, landing demo media), and spot-check external links (GitHub repo, Releases).

## GitHub Actions Workflow (PR Check + Default-Branch Deploy)

Add `.github/workflows/pages.yml` following the repository's existing CI security style (every action pinned by full 40-char commit SHA with `# vN` comment, minimal per-job `permissions`, explicit `timeout-minutes`):

- **Triggers**:
  - `pull_request` — **Decision:** run on **all** `pull_request` events with **no `paths:` filter** so configuring `build` as a GitHub Required Status Check never stalls PRs in a permanently Pending state (the static site build + offline check runs in seconds).
  - `push` to `main` (the default branch).
  - `workflow_dispatch` (manual re-run/re-deploy).
- **Job 1: `build` (runs on PRs, `push` to `main`, and `workflow_dispatch`)**:
  - Permissions: `contents: read`.
  - Installs pinned Python dependencies from `site/requirements.txt` (`--require-hashes`).
  - Runs `python3 site/build.py --base /tuse/ --out dist --check`.
  - Runs a second portability check `python3 site/build.py --base / --out dist-root --check` to verify root-base portability.
  - Uploads `dist` via `actions/upload-pages-artifact` **only** when `github.ref == 'refs/heads/main'` (covering both `push` to `main` and `workflow_dispatch` triggered on `main`).
- **Job 2: `deploy` (default branch ONLY)**:
  - Condition: `needs: build` AND `if: (github.event_name == 'push' || github.event_name == 'workflow_dispatch') && github.ref == 'refs/heads/main'` (manual `workflow_dispatch` from any non-default branch runs `build` + `--check` only and never deploys).
  - Permissions: `pages: write`, `id-token: write`.
  - Environment: `github-pages`.
  - Concurrency: group `pages`, `cancel-in-progress: false`.
  - Deploys via `actions/deploy-pages`.

## Manual Repository Prerequisites

Two one-time settings live in GitHub repository settings (cannot be enabled by workflow YAML alone):

1. **Required to publish:** **Settings -> Pages -> Build and deployment -> Source** -> select **GitHub Actions**.
2. **Recommended to block broken PR merges:** **Settings -> Branches / Rulesets -> `main`** -> add the Pages workflow's `build` job as a **Required status check** (because workflow YAML alone reports status checks without enforcing merge blocking).

## Error Reporting

- Any broken relative `.md` link, missing target file, missing `#anchor`, missing local asset, orphan unlinked doc page, escaping `../` link, or unprefixed `/docs/`/`/assets/` URL causes `--check` to exit non-zero printing the exact source file, offending URL/anchor, and reason.
- A failing `build` job prevents `deploy` from running on `main` (and blocks PR merges once configured as a required status check).

## Baseline Docs to Update Before Implementation (Handoff Gate)

Keep current baseline docs accurate now; before handing off implementation (per TODO #3), review:

- [../index.md](../index.md): confirm every baseline and planned doc is linked with valid relative paths and no escaping `../` links.
- Coordinate with [14-landing-page.md](14-landing-page.md) (shared `site/` builder, assets, and stylesheet) and [13-readme-rewrite.md](13-readme-rewrite.md) (live URLs added after first deploy).

## Acceptance Tests

1. **Single-source build**: `python3 site/build.py --base /tuse/ --out dist --check` renders `docs/index.md` -> `dist/docs/index.html`, every `docs/<page>.md` -> `dist/docs/<page>.html`, every `docs/planned/<spec>.md` -> `dist/docs/planned/<spec>.html`, and copies `site/assets/` -> `dist/assets/` without modifying `docs/`.
2. **Relative link & anchor rewriting**: sibling, nested (`planned/...`), parent (`../...`), and `#fragment` links resolve to valid built HTML targets and existing heading IDs under both `--base /tuse/` and `--base /`.
3. **Broken link / orphan / escape detection**: introducing a broken `.md` link, missing `#anchor`, missing local asset, unlinked orphan doc file, or escaping `../README.md` link causes `--check` to exit non-zero naming the source file and target.
4. **Redirect stubs**: adding an entry to `site/redirects.json` emits a valid HTML redirect at the old path that passes `--check`.
5. **Workflow gating**: PR events and non-`main` `workflow_dispatch` runs execute `build` + `--check` without deploying or requiring `pages: write`; only `push` or `workflow_dispatch` on `refs/heads/main` runs `deploy`; no `paths:` filter is set on `pull_request`.
6. **Live post-deploy verification**: after setting Pages source = GitHub Actions and deploying `main`, live HTTP checks confirm 200 OK on `<site>/`, `<site>/docs/`, sample baseline and planned pages, and static assets with zero 404s.
