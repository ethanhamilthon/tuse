# Plugins

A plugin is a Lua file (in a local directory or Git checkout) that returns a config table or a function accepting options and returning one. Tuse validates every enabled plugin individually and merges all plugins underneath the user's config.

## Declaring Plugins

Declare plugins in the top-level `plugins` list of your user config (plugins themselves cannot declare `plugins`):

```lua
return {
  plugins = {
    "owner/repo",                                   -- init.lua (else tuse.lua) at repo root
    "ethanhamilthon/tuse/plugins/welcome",          -- subfolder inside the repo
    { "owner/kit", opts = { name = "Yen" } },       -- options table passed to the plugin
    { "owner/pinned", ref = "v1.2.0" },             -- tag, branch, or commit metadata
    { "https://gitlab.com/me/stuff.git", path = "plugins/x" },
    { "git@github.com:me/private.git" },
    { "dev", dir = "~/code/my-plugin" },            -- local directory, no git layout
    { "owner/off", enabled = false },               -- skipped completely
  },
}
```

`plugins` must be a 1-based sequence (unexpected map keys are rejected). Each entry is a string shorthand or a table with these keys (unknown keys are rejected):

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `[1]` | string (required) | — | GitHub `owner/repo[/subpath]`, a Git URL (`://` or `git@`), or a display name when `dir` is set. |
| `opts` | table | `{}` | Options table passed to function plugins. |
| `ref` | string | unset | Tag, branch, or commit metadata shown in `/config` and error hints (not verified or enforced against the checkout on disk). Multiple enabled entries from the same repository cannot specify conflicting `ref` values. |
| `dir` | string | unset | Local folder path (`~/` expanded, relative paths resolved against the config file's directory). |
| `path` | string | unset | Subfolder inside the repository (appended to any subpath in `[1]`). Rejects any `..` path segment, though it is not a filesystem confinement boundary (absolute paths and symlink targets are not restricted). |
| `enabled` | boolean | `true` | `false` skips loading, checkout checks, ref-conflict checks, merging, and listing; the entry's own schema, keys, `[1]` spec, and `path` syntax are still validated. |

## Checkout Paths and Entrypoints

| Entry form | Checkout root | Subpath |
| --- | --- | --- |
| `"owner/repo"` | `~/.tuse/plugins/<owner>/<repo>` | none (or `path`) |
| `"owner/repo/sub/dir"` | `~/.tuse/plugins/<owner>/<repo>` | `sub/dir` (plus `path` if set) |
| `"https://host/owner/repo[.git]"` or `"git@host:owner/repo[.git]"` | `~/.tuse/plugins/<host>/<owner>/<repo>` | none (or `path`) |
| `{ "name", dir = "<local>" }` | `<local>` resolved from config file dir | none (or `path`) |

- GitHub shorthand requires at least `owner/repo` with no empty, `.`, or `..` segments; its clone URL is `https://github.com/<owner>/<repo>.git`.
- Inside the resolved plugin directory (`<root>[/<subpath>]`), Tuse looks for `init.lua` first, else `tuse.lua`.
- If a Git checkout root does not exist on disk, loading fails with an error showing the `git clone <url> <root>` command (plus `git -C <root> checkout <ref>` when `ref` is set). If the root exists (or `dir` is used) without `init.lua` or `tuse.lua`, loading fails with `no init.lua or tuse.lua in <dir>`.
- When the checkout root is a standard Git checkout, `/config` and JSON-RPC `config.get` report the 7-character short commit SHA of `HEAD` (otherwise `commit` is omitted and `/config` displays `local`).

## Writing a Plugin

A plugin executes in Tuse's file environment ([lua.md](lua.md)):

- Import host functions with `require("tuse")`; `t.session()` returns the pre-session snapshot (`mode`, `configpath`, `cwd`, `home`, with `id == nil`) at load time.
- Relative `require("./util")` resolves relative to the plugin file; `os.getenv("THIS_PATH")` returns the plugin file's path.
- Return a config table, `nil` (treated as `{}`), or a `function(opts)` that returns a config table (or `nil`). `opts` is always a table (`{}` when omitted). Returning any other type fails.

```lua
local util = require("./util") -- relative to this file

return function(opts)
  return {
    systems = { "User's name is " .. (opts.name or "friend") },
    commands = {
      hello = function()
        require("tuse").show("hello", util.greet(opts), false)
      end,
    },
  }
end
```

## Validation and Merging

1. The user's top-level config table (excluding `plugins`) is validated first.
2. Enabled plugin entries are parsed and checked for conflicting `ref` values on any shared repository root.
3. Each enabled plugin is loaded and validated strictly in list order (`plugin <name>: <path>: <message>`). Plugins cannot declare `plugins` or unknown keys.
4. Plugins merge in list order, then the user config merges on top:
   - `systems` and `models`: concatenated, plugins first, then user config. For `models`, the last definition for a given `name` wins.
   - `prompts`, `functions`, `hooks`, `commands`, `open`: merged by key. If two plugins define the same key, loading fails (`plugins A and B both define <section>.<key>`). The user config silently overrides any plugin entry.
   - `settings` and `theme`: merged deeply table-by-table; later plugins override earlier plugins on overlapping keys, and user config wins over all plugins.
5. The final merged config is validated before committing.

`/config` lists each enabled plugin with its name, optional `@ref`, 7-char commit (or `local`), and clickable directory path. JSON-RPC `config.get` returns them under `plugins` as `{ name, dir, ref?, commit? }`.

## Managing Checkouts

Tuse's plugin loader never touches the network: startup and `/reload` read checkouts already on disk (note that once loaded, plugin Lua code itself runs with full user permissions). `ref` is metadata only and is not verified against the checked-out commit. Manage Git checkouts manually and declare them in your config:

```sh
git clone https://github.com/owner/repo ~/.tuse/plugins/owner/repo   # install
git -C ~/.tuse/plugins/owner/repo checkout v1.2.0                     # match entry ref
git -C ~/.tuse/plugins/owner/repo pull --ff-only                      # update
rm -rf ~/.tuse/plugins/owner/repo                                     # remove checkout
```

Then update `plugins` in your config and run `/reload`. For local development, use `{ "name", dir = "~/code/my-plugin" }`.

## Shipped Repository Plugin: `welcome`

`ethanhamilthon/tuse/plugins/welcome` ships in the Tuse Git repository (`plugins/welcome`), not embedded in the binary. Clone the repository to `~/.tuse/plugins/ethanhamilthon/tuse` (or point `dir` at a local checkout):

```sh
git clone https://github.com/ethanhamilthon/tuse ~/.tuse/plugins/ethanhamilthon/tuse
```

Behavior:

- Activates only in TUI mode (`t.session().mode == "tui"`); returns `{}` in `json` mode.
- Registers a `welcome` hook on `start`: when `e.is_new` is `true`, renders an unpinned `show` block containing an ASCII title banner, active model (display name and effort when not `"default"`), working directory, Git branch and status (2-second timeout), local date/time, config path, and `tuse --version` (2-second timeout).
- Options:
  - `title` (string): banner text; defaults to the capitalized basename of `cwd` (or `"tuse"`).
  - `block` (string): `show` block name; defaults to `"welcome"`.
