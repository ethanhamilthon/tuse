# plugins

## Purpose
Installing, discovering, validating, and managing Lua plugins under `~/.tuse/plugins`.

## Requirements

### Requirement: Plugin directory discovery and manifest
Installed plugins SHALL live under `~/.tuse/plugins`. A directory under `~/.tuse/plugins` SHALL be treated as an installed plugin if and only if its root directory contains `tuse-plugin.json`. `tuse-plugin.json` MUST be valid JSON containing a required `version` string and an optional `entrypoint` string defaulting to `"init.lua"`. `entrypoint` MUST resolve to a file inside the plugin root directory and MUST NOT escape the plugin root via relative traversal or absolute paths.

#### Scenario: Directory without manifest ignored
- **WHEN** `~/.tuse/plugins/scratch/` exists without `tuse-plugin.json` in its root (even if a nested subdirectory contains `tuse-plugin.json`)
- **THEN** `scratch` is not discovered as an installed plugin

#### Scenario: Default and custom entrypoint
- **WHEN** `~/.tuse/plugins/alpha/tuse-plugin.json` contains `{"version": "1.0.0"}`
- **THEN** `alpha` resolves its entrypoint to `~/.tuse/plugins/alpha/init.lua`
- **WHEN** `~/.tuse/plugins/beta/tuse-plugin.json` contains `{"version": "1.0.0", "entrypoint": "src/main.lua"}`
- **THEN** `beta` resolves its entrypoint to `~/.tuse/plugins/beta/src/main.lua`

#### Scenario: Invalid manifest or escaping entrypoint
- **WHEN** `tuse-plugin.json` omits `version` or sets `"entrypoint": "../escape.lua"`
- **THEN** plugin validation fails with an error identifying the plugin and invalid field

### Requirement: Plugin sources from public GitHub repositories and subdirectories
Tuse SHALL support installing and updating plugins from public GitHub repositories and from subdirectories within public GitHub repositories. The target repository root or specified subdirectory MUST contain a valid `tuse-plugin.json` and its declared entrypoint file; otherwise installation MUST fail without leaving a partial directory in `~/.tuse/plugins`. Archive extraction MUST reject path traversal entries outside the plugin root.

#### Scenario: Install from repository root
- **WHEN** the user installs a public GitHub repository whose root contains a valid `tuse-plugin.json` and `init.lua`
- **THEN** the plugin files are installed into `~/.tuse/plugins/<name>` with `tuse-plugin.json` at the plugin root

#### Scenario: Install from repository subdirectory
- **WHEN** the user installs a plugin from a subdirectory of a public GitHub repository containing `tuse-plugin.json`
- **THEN** only that subdirectory's contents are installed into `~/.tuse/plugins/<name>` with `tuse-plugin.json` at the plugin root

#### Scenario: Missing manifest aborts cleanly
- **WHEN** the target GitHub repository or subdirectory does not contain `tuse-plugin.json`
- **THEN** installation fails with an error and `~/.tuse/plugins` is left unchanged

### Requirement: Plugin management CLI
Tuse SHALL provide the `tuse plugins` command with subcommands `add`, `list`, `delete`, `print`, and `update [repo]`:
- `tuse plugins add [--replace] <repo>` SHALL accept `owner/repo[/subdir][@ref]` or `https://github.com/owner/repo[/tree/<ref>/<subdir>]`, fetch the archive over HTTPS (`TUSE_PLUGINS_URL` override supported for tests), validate `tuse-plugin.json` and its entrypoint, derive `<name>` from `tuse-plugin.json` `name` else repo/subdir basename, reject collisions unless `--replace` is passed, install into `~/.tuse/plugins/<name>`, and record source metadata in `~/.tuse/plugins.json`.
- `tuse plugins list [--json]` SHALL list installed plugins discovered under `~/.tuse/plugins`.
- `tuse plugins delete <target>` SHALL remove the installed plugin directory from `~/.tuse/plugins` and its metadata entry, and fail if it is not installed.
- `tuse plugins print [--json] <target>` SHALL print the plugin's path, source metadata, and manifest details (`--json` for JSON output) and fail if it is not installed.
- `tuse plugins update [repo]` SHALL update the specified installed plugin when an argument is provided, or update all installed plugins when omitted.

#### Scenario: Add, list, and delete lifecycle
- **WHEN** the user runs `tuse plugins add <repo>`, then `tuse plugins list`, then `tuse plugins delete <target>`
- **THEN** `add` installs the plugin under `~/.tuse/plugins`, `list` displays it with its version, and `delete` removes its directory

#### Scenario: Update one or all plugins
- **WHEN** the user runs `tuse plugins update <target>`
- **THEN** only that installed plugin is updated from its source
- **WHEN** the user runs `tuse plugins update` with no argument
- **THEN** every installed plugin is updated from its recorded source

#### Scenario: Unknown plugin target fails
- **WHEN** the user runs `tuse plugins delete nonexistent` or `tuse plugins print nonexistent`
- **THEN** the CLI exits with status 1 and prints an error naming `nonexistent`
### Requirement: Official plugin catalog and installer selection
The repository SHALL publish `login`, `themes`, `status`, and `rename` as independent installable directories under `plugins/`, each with a manifest. The download installer SHALL offer each missing official plugin individually in an interactive terminal after installing the binary. It MUST require affirmative consent per plugin, MUST NOT replace existing plugins, and MUST NOT install plugins when run without an interactive terminal. Cancelling or declining a plugin MUST NOT undo the binary installation.

#### Scenario: Select only one plugin
- **WHEN** a user runs the download installer interactively, chooses `themes`, and declines the other plugins
- **THEN** the installer installs `themes` using `tuse plugins add` and does not install the other plugins

#### Scenario: Install without a terminal
- **WHEN** an unattended job runs the download installer without a terminal
- **THEN** the installer installs the binary without prompting or installing plugins

#### Scenario: Keep an existing plugin
- **WHEN** an official plugin is already installed
- **THEN** the installer skips it without replacing its contents
