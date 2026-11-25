## ADDED Requirements

### Requirement: Documentation site from the same sources
The documentation site SHALL be built with mdBook from `README.md` and the six `docs/` pages, with no copies kept in the repo. The README SHALL be the first page, followed by the six pages in topic order, and there SHALL be no other pages.

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
