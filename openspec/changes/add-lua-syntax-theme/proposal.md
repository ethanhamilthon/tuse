## Why

Lua code is shown with five colors, and operators, punctuation, parameters, and table keys all look like plain text. Users cannot tell `=` from a function name, and cannot restyle Lua separately from the rest of the theme. User messages also share `fg` with the agent's text.

## What Changes

- Parse Lua with context instead of word lists: definitions, parameters and their uses, calls, methods, fields, table keys, standard and Tuse functions, escapes, numbers, and long strings and comments across lines.
- Add `theme.lua` with 15 tokens: `keyword`, `storage`, `constant`, `operator`, `punctuation`, `number`, `string`, `escape`, `comment`, `function`, `builtin`, `method`, `field`, `parameter`, `variable`.
- Add `theme.user` for the text of user messages and of the input. `fg` stays for the agent's text.
- Let `theme.lua.*` and `theme.user` link to another theme color by name, so defaults follow the base theme.

## Impact

- Specs: `config`, `tui`.
- Code: theme schema and validation, palette, Lua highlighter, `/config` view, `docs/config.md`.
- Compatibility: all new keys are optional and default to links onto the current colors; `user` defaults to `fg`. `config.get` gains `theme.user` and `theme.lua`.
