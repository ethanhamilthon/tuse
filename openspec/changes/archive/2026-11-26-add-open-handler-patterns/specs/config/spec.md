## MODIFIED Requirements

### Requirement: Open handlers under settings
`open` SHALL be read from `settings.open` (`key -> function(path)`). A key is an extension without a dot (`md`, case-insensitive), a mask with `*` or `?` matched against the file name (`*.test.lua`, `.env*`, case-insensitive), the catch-all `*`, `@dir` for folders, or `@url` for `http://`, `https://`, and `mailto:` links, which get the URL. Any other key MUST fail validation, and keys MUST NOT contain `/`, `\`, whitespace, or control characters. A file SHALL use the handler of its extension, else the most specific mask (the one with the most literal characters, ties in alphabetical order), else `*`, else the OS opener. A folder SHALL use `@dir`, else `*`, else the OS opener. A top-level `open` key MUST fail with `open: moved to settings.open`.

#### Scenario: Old key
- **WHEN** a config returns `{ open = { md = f } }`
- **THEN** loading fails with `tuse.lua: open: moved to settings.open`

#### Scenario: New key
- **WHEN** a config returns `{ settings = { open = { md = f } } }` and a `.md` path is clicked
- **THEN** `f` is called with the path

#### Scenario: Mask beats catch-all
- **WHEN** `open = { ["*.test.lua"] = a, ["*"] = b }` and `x.test.lua` is clicked
- **THEN** `a` is called, and clicking `y.txt` calls `b`

#### Scenario: Extension beats mask
- **WHEN** `open = { lua = a, ["*.test.lua"] = b }` and `x.test.lua` is clicked
- **THEN** `a` is called

#### Scenario: Folder
- **WHEN** `open = { ["@dir"] = f }` and an existing folder is clicked
- **THEN** `f` is called with the folder path

#### Scenario: URL
- **WHEN** `open = { ["@url"] = f }` and `https://example.com` is clicked
- **THEN** `f` is called with `https://example.com`, and a `javascript:` link is still refused

#### Scenario: Invalid key
- **WHEN** a config sets `open = { ["@file"] = f }` or `open = { [".md"] = f }`
- **THEN** loading fails with an error naming the key
