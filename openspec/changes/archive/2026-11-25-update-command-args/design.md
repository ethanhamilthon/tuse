## Context
`Config.commands` is a `Vec<String>` of names, and the function lives only in the Lua registry. Prompts keep `{ kind, body, args }` in `Config.prompts`. A prompt takes one value, picked from `args`; commands differ in that without `args` they take free words.

## Decisions
- `Config.commands` becomes a `BTreeMap<String, Command>` with `desc`, `args`, and `params`. A bare function stays valid and means `{ run = f }`.
- `params` is the number of parameters `run` declares, read from the function when the config loads, or `null` when `run` is variadic. It is exposed so clients can build input.
- Words are the text after `/name` split on whitespace; a word may be wrapped in `"…"` or `'…'` to hold spaces, and `\"` or `\'` inside the matching quotes is a literal quote. An unclosed quote is an error. The quotes are not part of the word.
- Without `args`, at most `params` words are accepted (any number if variadic), passed in order; missing ones are `nil`. With `args`, at most one word is accepted, it must be in `args`, and it is the first parameter.
- `Session::run_command(name, argv)` validates `argv` before the job starts, so the TUI and JSON-RPC fail the same way.
- Because words are accepted, `/name text` is always a command call for a known command; it is never sent to the agent as a message.
- Commands are not atoms in the editor: they run on Enter, so the arg picker only fills the text after `/name `.
