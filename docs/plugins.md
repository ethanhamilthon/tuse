# Plugins

A plugin is a Lua file in a git checkout that returns a config table. Tuse validates each plugin
individually and merges plugins under your own config.

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

Plugins load and validate in list order, then your config goes on top.

- `systems` and `models` are concatenated, plugins first. The last definition of a model wins.
- `prompts`, `functions`, `hooks`, `commands`, and `open` merge by name. If two plugins use the
  same name, that is an error. Your config silently overrides a plugin.
- `settings` and `theme` merge deeply, with your values winning.
- Plugins cannot set `plugins`, unknown keys, or removed keys (`model`, `provider`, `effort`).

`/config` lists the loaded plugins with their `ref`, commit (or `local`) and folder; `config.get`
in `tuse json` returns them as `plugins`.

## Installing

Tuse never touches the network: startup reads plugins from checkouts on disk, and a missing
checkout is an error that shows the `git clone` command to run. Manage checkouts with Git and
declare them in `tuse.lua`.

A GitHub entry `owner/repo[/path]` lives in `~/.tuse/plugins/owner/repo`; a git URL lives in
`~/.tuse/plugins/HOST/OWNER/REPO`.

```sh
git clone https://github.com/owner/repo ~/.tuse/plugins/owner/repo   # install
git -C ~/.tuse/plugins/owner/repo checkout v1.2.0                     # pin to the entry's ref
git -C ~/.tuse/plugins/owner/repo pull --ff-only                      # update
rm -rf ~/.tuse/plugins/owner/repo                                     # remove the checkout
```

Then add (or remove) the entry in `plugins` in your `tuse.lua` and run `/reload`. For a plugin you
develop locally, skip the clone and use `{ "name", dir = "~/code/my-plugin" }`.

## Bundled: welcome

`ethanhamilthon/tuse/plugins/welcome` listens to the `start` hook (when `e.is_new` is true) and
reads `require("tuse").session()` to show a banner with the model, folder, git state, date, config
path, and version in new TUI sessions. Its options are `title` (the banner text, default: the
project folder name) and `block` (the `show` name, default `welcome`).
