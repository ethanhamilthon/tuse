## MODIFIED Requirements

### Requirement: Documentation site from the same sources
The documentation site SHALL be built with mdBook from `README.md` and the seven `docs/` pages, with no copies kept in the repo. The README SHALL be the first page, followed by the seven pages in topic order, and there SHALL be no other pages.

#### Scenario: Examples page on the site
- **WHEN** `make site` runs
- **THEN** `Examples` is the last entry of `SUMMARY.md` and its body equals `docs("examples")`
