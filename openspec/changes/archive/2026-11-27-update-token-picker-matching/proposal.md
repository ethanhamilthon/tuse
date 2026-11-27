## Why
Typing `#r` in the input opens a picker that lists `#imagegen`, `#openspec`, `#srt`, and `#use-figma` next to `#release`. They match only because the search runs over the description, or over the middle of the name. The same happens for `/`. The list should show only what really matches, with the best match first.

## What Changes
- The `/` and `#` pickers, and the arg pickers opened after `/name ` and `#name `, match only the start of the name, ignoring case. The `/` or `#` mark and the description are not searched.
- The `@` file picker matches a substring of the file or folder name, ignoring case. Names that start with the query come first.
- Within each group, `@` items are ordered by the last modified time, newest first. Without a query, `@` lists the newest first. `/` and `#` keep the list order inside each group.
- `files::list` sorts by the modified time (the created time when it is missing) before the size cap, so recent files are not cut off in a large folder.
- Searchable pickers (`/history`, `/resume`, `/fork`, `/models`, `/tasks`) keep searching the whole row.

## Impact
- Specs: `tui`.
- Implementation: `Picker::filter` and `token_picker.rs` in `crates/tui/src/ui`, `files::list` in `crates/tui/src/kit/files.rs`.
- Tests: the picker filter test is split into token and row search.
