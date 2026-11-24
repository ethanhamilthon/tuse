# Plugins

A plugin is a Lua file (in a local directory or Git checkout) that returns a config table or a function accepting options and returning one. Tuse validates every enabled plugin individually and merges all plugins underneath the user's config. For a step-by-step guide to building and publishing plugins, see [plugin-authoring.md](plugin-authoring.md).

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
| `ref` | string | unset | Tag, branch, or commit SHA synced on reload (and shown in `/config` and offline startup hints). A 40-char hex SHA (or 7–39-char hex string not matching any tag or remote branch) already at `HEAD` is treated as an immutable pin and skips fetching on reload; tags, branches, and omitted `ref` fetch on every reload. Multiple enabled entries from the same repository cannot specify conflicting `ref` values. |
| `dir` | string | unset | Local folder path (`~/` expanded, relative paths resolved against the config file's directory). Re-read directly from disk on every reload with no Git or network commands. |
| `path` | string | unset | Subfolder inside the repository (appended to any subpath in `[1]`). Rejects any `..` path segment, though it is not a filesystem confinement boundary (absolute paths and symlink targets are not restricted). |
| `enabled` | boolean | `true` | `false` skips loading, checkout checks, URL/ref-conflict checks, Git sync, merging, and listing; the entry's own schema, keys, `[1]` spec, and `path` syntax are still validated. |

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
2. Enabled plugin entries are parsed and checked for conflicting normalized URLs (stripping trailing `/` and `.git` while preserving scheme/host/user/path) and conflicting non-nil `ref` values on any shared checkout root before any Git command runs.
3. On reload (`/reload`, JSON-RPC `config.reload`, or Lua `reload()`), each unique enabled Git repository is synchronized at most once before loading. Startup, `/new`, `/resume`, `session.new`, `session.resume`, and `tuse models` stay offline and read existing checkouts on disk.
4. Each enabled plugin is loaded and validated strictly in list order (`plugin <name>: <path>: <message>`). Plugins cannot declare `plugins` or unknown keys.
5. Plugins merge in list order, then the user config merges on top:
   - `systems` and `models`: concatenated, plugins first, then user config. For `models`, the last definition for a given `name` wins.
   - `prompts`, `functions`, `hooks`, `commands`, `open`: merged by key. If two plugins define the same key (including `functions` entries set to either a function or `false`), loading fails (`plugins A and B both define <section>.<key>`). The user config silently overrides any plugin entry (including disabling a plugin's configured function with `false`).
   - `settings` and `theme`: merged deeply table-by-table; later plugins override earlier plugins on overlapping keys, and user config wins over all plugins.
6. The final merged config is validated before `close` hooks fire or the reload commits.

`/config` lists each enabled plugin with its name, optional `@ref`, 7-char commit (or `local`), and clickable directory path. JSON-RPC `config.get` returns them under `plugins` as `{ name, dir, ref?, commit? }`.

## Installing and Updating Checkouts (Reload Sync)

Initial startup (`tuse`, `tuse json`), session switches (`/new`, `/resume`, `session.new`, `session.resume`), and `tuse models` are **offline** and read existing checkouts on disk without touching the network. Reloading (`/reload`, JSON-RPC `config.reload`, or Lua `reload()`) automatically synchronizes every enabled Git plugin before loading and validating it:

- **Shared-repo deduplication**: multiple enabled entries pointing to subpaths of one repository share one sync per reload. If one entry sets `ref` and another omits it, the non-nil `ref` applies to that repository; conflicting `ref`s or conflicting normalized URLs fail before Git runs.
- **Cross-process locking & non-interactive Git**: every repository sync holds an exclusive advisory lock file under `~/.tuse/plugins/.locks/` and runs `git` non-interactively (stdin closed, `GIT_TERMINAL_PROMPT=0`, `GCM_INTERACTIVE=never`, `GIT_ASKPASS=/usr/bin/true` if unset, `GIT_SSH_COMMAND="ssh -o BatchMode=yes"` if unset) with a 60-second per-repository timeout (killing the Git process group on timeout).
- **Missing checkout (install)**: clones into a temporary staging directory under `~/.tuse/plugins/`, resolves `ref` (or the remote's default branch), checks out the target commit in detached `HEAD`, and atomically renames the staging directory into the checkout root. Any failure deletes the staging directory so no partial checkout is left behind.
- **Existing checkout (update)**:
  - Verifies the directory is a valid Git repository whose `origin` remote URL matches the entry's normalized URL (never overwrites `origin`).
  - **Immutable commit pin**: if `ref` is a 40-char hex SHA (or 7–39-char hex string matching no tag or remote branch) and `HEAD` already resolves to that commit, skips fetching and leaves the checkout untouched.
  - Otherwise fetches with pruning (`git fetch --prune --prune-tags --tags --force origin`, plus `git remote set-head origin -a` when `ref` is omitted so default-branch renames such as `master` -> `main` are picked up).
  - Resolves target commit identically on clone and reload: when `ref` is omitted, uses `refs/remotes/origin/HEAD`; when `ref` is set, checks exact tag `refs/tags/<ref>`, then exact remote branch `refs/remotes/origin/<ref>`, then commit SHA (7–40 hex chars), else fails with `plugin <name>: ref '<ref>' not found`.
  - If `HEAD` already equals the target commit, leaves the working tree untouched.
- **Local-work safety**: before moving `HEAD` to a different commit on an existing checkout, Tuse checks for a clean working tree (no staged, unstaged, or untracked non-ignored files), no in-progress Git operation (merge, rebase, cherry-pick, revert, bisect), and verifies that current `HEAD` (branch or detached) is reachable from `refs/remotes/origin/*` or `refs/tags/*`. Checkout runs in detached `HEAD` without `--force` (so colliding ignored files are never overwritten). If any safety check fails, the working tree is left untouched and reload fails naming the plugin.
- **Removed plugins & failure rollback**: removing a plugin from config unloads it on reload without deleting its checkout under `~/.tuse/plugins/`. If any plugin fails sync, load, or validation, the reloading session performs in-memory rollback (keeping the previous working config active) without reverting already-synced sibling repositories on disk.
- **Offline bootstrap**: if you want a new Git plugin present before first launch, clone it manually (`git clone <url> ~/.tuse/plugins/...`) or add it to `plugins` while running and run `/reload`. For local development, use `{ "name", dir = "~/code/my-plugin" }`.

## Bundled Default Plugin: `tuse/default`

`"tuse/default"` is a reserved built-in plugin embedded directly in the `tuse` binary (source under `plugins/default/`). It loads completely offline without Git or network access and exposes two optional parts:

- **`theme`** (default `true`) — applies Tuse's curated 10-token color palette ([theme.md](theme.md#bundled-default-theme-tusedefault)); user `theme` overrides still win token-by-token.
- **`subs`** (default `false`) — registers the `/subs` command wrapping CLIProxyAPI as a loopback-only local provider ([subs.md](subs.md)). All setup, login, proxy, and provider actions are explicit user opt-ins; loading or reloading never performs side effects.

```lua
return {
  plugins = {
    {
      "tuse/default",
      opts = {
        theme = true,
        subs = false,
      },
    },
  },
}
```

- Because `opts.theme` defaults to `true`, a `subs`-only configuration must set `theme = false` explicitly (`opts = { theme = false, subs = true }`).
- `ref`, `dir`, and `path` are rejected on `"tuse/default"` during validation (`plugin tuse/default: bundled plugin does not accept ref, dir, or path`).
- Unknown keys or invalid values inside `opts` or `opts.subs` fail config validation with diagnostics naming `plugin tuse/default` and the full key path.
- Capability probe: `tuse models -c <temp_config>` with `{ plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }` exits `0` on binaries that bundle `"tuse/default"` and non-zero on older binaries.

## Shipped Repository Plugin: `welcome`

`ethanhamilthon/tuse/plugins/welcome` ships in the Tuse Git repository (`plugins/welcome`), not embedded in the binary. Add `"ethanhamilthon/tuse/plugins/welcome"` to `plugins` and run `/reload` to install it automatically, or clone it beforehand for offline startup (or point `dir` at a local checkout):

```sh
git clone https://github.com/ethanhamilthon/tuse ~/.tuse/plugins/ethanhamilthon/tuse
```

Behavior:

- Activates only in TUI mode (`t.session().mode == "tui"`); returns `{}` in `json` mode.
- Registers a `welcome` hook on `start`: when `e.is_new` is `true`, renders an unpinned `show` block containing an ASCII title banner, active model (display name and effort when not `"default"`), working directory, Git branch and status (2-second timeout), local date/time, config path, and `tuse --version` (2-second timeout).
- Options:
  - `title` (string): banner text; defaults to the capitalized basename of `cwd` (or `"tuse"`).
  - `block` (string): `show` block name; defaults to `"welcome"`.
