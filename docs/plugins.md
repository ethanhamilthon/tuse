# Plugins

A plugin is a Lua file in a git repository that returns a config table. Tuse merges the
tables from your plugins under your own config.

```lua
return {
  plugins = {
    "owner/repo",                                   -- init.lua (else tuse.lua) at the root
    "ethanhamilthon/tuse/plugins/welcome",          -- a folder inside the repository
    { "owner/kit", opts = { name = "Yen" } },       -- options for the plugin
    { "owner/pinned", ref = "v1.2.0" },             -- tag, branch or commit
    { "https://gitlab.com/me/stuff.git", path = "plugins/x" },
    { "dev", dir = "~/code/my-plugin" },            -- a local folder, no git
    { "owner/off", enabled = false },
  },
}
```

| Key | Meaning |
| --- | --- |
| `[1]` | `owner/repo[/path]` on GitHub, a git URL, or a name for `dir` |
| `opts` | Table passed to the plugin. Default `{}` |
| `ref` | Tag, branch or commit to check out. Plugins from one repository must agree on it |
| `dir` | Local folder, relative to the config file |
| `path` | Folder inside the repository, for git URLs |
| `enabled` | `false` skips the plugin |

## Writing a plugin

Return a table, or a function that takes `opts` (always a table) and returns one:

```lua
local util = require("./util")   -- relative to this file
return function(opts)
  return {
    systems = { "User's name is " .. (opts.name or "friend") },
    commands = { hello = function() print(util.greet(opts)) end },
  }
end
```

## Merging

Plugins load in list order, then your config goes on top.

- `systems` and `models` are concatenated, plugins first.
- `prompts`, `functions`, `hooks` and `commands` merge by name. If two plugins use the same
  name, that is an error. Your config silently overrides a plugin.
- `settings` and `theme` merge deeply, with your values winning.
- Plugins cannot set `model`, `provider`, `effort` or `plugins`.

## Installing

Clones live in `~/.tuse/plugins` (`$TUSE_HOME/plugins`), one per repository. Starting Tuse
never touches the network: a missing clone is an error that points to `tuse plugins install`.

```
tuse plugins                        list the plugins of the config and their commits
tuse plugins add NAME [--ref REF]   clone NAME and add it to the config
tuse plugins remove NAME            remove the entry (the clone stays until `clean`)
tuse plugins install                clone what is missing, check out each `ref`
tuse plugins update [NAME]          fetch and fast-forward the clones
tuse plugins clean                  delete clones no plugin uses
```

Every subcommand takes `-c/--config PATH`. Without it, the subcommand uses the config Tuse
would load: `./tuse.lua`, then `$TUSE_GLOBAL_CONFIG`, then `~/.config/tuse.lua`, then
`~/.config/tuse/init.lua`. `add` creates `~/.config/tuse/init.lua` when no config exists. When
the config does not return a literal table (for example `return g`), `add` prints the lines to
paste by hand.

## Bundled: welcome

`ethanhamilthon/tuse/plugins/welcome` shows a banner with the model, folder, git state, date,
config and version when a new TUI session starts. Its options are `title` (the banner text,
default: the project folder name) and `block` (the `show_markdown` name, default `welcome`).
