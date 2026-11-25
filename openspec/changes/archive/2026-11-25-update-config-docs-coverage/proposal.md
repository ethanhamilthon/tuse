## Why
`docs("config")` is meant to be the full reference for the config, but only the `require("tuse")` functions are checked against the code. A review found gaps: no naming rules for prompts, commands, and `open` extensions, the color aliases `gray` and `dark_gray` are missing, and nothing keeps new keys, tokens, or events from going undocumented.

## What Changes
- Document the missing rules in `docs/config.md`: names of prompts, commands, and `open` extensions, the `gray` and `dark_gray` aliases, and that a hook needs at least one known event and a `run`.
- Extend the coverage tests so that `cargo test` fails when `docs("config")` lacks a top-level key, a `settings.ui` key, a theme token, or a `theme.lua` token, when `docs("lua")` lacks a hook event, or when `docs("session")` lacks a history role.

## Impact
- Specs: docs
- Code: `docs/config.md`, `crates/core/tests/contract.rs`
