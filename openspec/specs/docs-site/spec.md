# docs-site

## Purpose
The documentation website built from the same sources as `docs()`.

## Requirements

### Requirement: Documentation site from the same sources
The documentation site SHALL be built with mdBook from `README.md` and the seven `docs/` pages, with no copies kept in the repo. The README SHALL be the first page, followed by the seven pages in topic order, and there SHALL be no other pages.

#### Scenario: Examples page on the site
- **WHEN** `make site` runs
- **THEN** `Examples` is the last entry of `SUMMARY.md` and its body equals `docs("examples")`

#### Scenario: Building the site
- **WHEN** `make site` runs
- **THEN** it assembles the book source under `target/`, generates `SUMMARY.md` from the README and the page titles, and writes the HTML to `target/book` without changing tracked files

#### Scenario: Page parity
- **WHEN** a page is opened on the site
- **THEN** its body is the same Markdown that `docs("<topic>")` returns for the default config

### Requirement: Site deployment
The Pages workflow SHALL install a pinned mdBook, run `make site`, and deploy `target/book` on pushes to `main`. Pull requests SHALL only build.

#### Scenario: Pull request
- **WHEN** a pull request changes `docs/`
- **THEN** the workflow builds the site and fails on build errors, but does not deploy

### Requirement: Shared Tuse screenshot
The docs site SHALL use the same screenshot asset as the repository README.

#### Scenario: Building the screenshot into the site
- **WHEN** `make site` builds the README page
- **THEN** the screenshot it references is copied to the corresponding path inside `target/book`
