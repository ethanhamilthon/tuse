## Why
`settings.open` only handles local files by extension, and URLs always go to the OS opener. Folders, name patterns, a catch-all, and URLs cannot be handled from the config.

## What Changes
- `settings.open` accepts masks (`*.test.lua`, `.env*`), the catch-all `*`, `@dir` for folders, and `@url` for http(s) and mailto links, besides extensions.
- Lookup order for a file: extension, then the most specific mask, then `*`, then the OS opener. A folder uses `@dir`, then `*`.
- Docs updated.

## Impact
- Specs: config
- Code: `config/names.rs`, `core/session/lua/open.rs`, `tui/ui/screen/config_view.rs`, tests
- Docs: `docs/config.md`, `docs/usage.md`
