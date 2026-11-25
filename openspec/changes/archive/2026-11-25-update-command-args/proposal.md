## Why
A custom command is a bare function: it takes no arguments and has no description, so the `/` picker shows only its name. Prompts already have `{ args, run }`, and commands should read the same way.

## What Changes
- A `commands` value is a function (as today) or a table `{ args?, desc?, run }`, the same shape as a prompt plus `desc`.
- Words can be quoted (`/commit "fix bug"`).
- Without `args`, `/name one two` passes the typed words to `run` as positional parameters, as many as `run` declares.
- With `args`, only one word from `args` is accepted and passed as the first parameter.
- `desc` is shown next to `/name` in the `/` picker.
- In the TUI, picking a command with `args` opens the arg picker.
- JSON-RPC `commands.run` takes an optional `argv` list of words, and `commands.list` returns `{name, desc, args, params}` entries.
- `config.get` and `/config` show `desc` and `args` for each command.
- Docs and the `commands` example are updated.

## Impact
- Specs: config, tui
- Code: `crates/config/src/types.rs`, `crates/config/src/names.rs`, `crates/core/src/scripting/validate.rs`, `crates/core/src/session/lua/commands.rs`, `crates/rpc/src/methods.rs`, `crates/tui/src/ui/{commands,send,token_picker}.rs`, `crates/tui/src/ui/screen/config_view.rs`, `docs/config.md`, `docs/json.md`, `docs/usage.md`, `examples/commands.lua`
- Breaking: `commands.list` returns objects instead of names
