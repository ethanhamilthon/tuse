# Planned: First-Run Setup in the Installer (TODO #11)

Status: Planned (not yet implemented). Depends on the bundled default plugin design in [07-default-plugin-theme-subs.md](07-default-plugin-theme-subs.md) (`theme` and `subs`).

## Goal

Extend `release/install.sh` so that after installing the `tuse` binary, interactive first-time installations can optionally enable parts of the bundled default plugin (`theme` and `subs`) and write `~/.config/tuse/init.lua`, while preserving existing global configs, gating setup on the installed binary's capabilities, and keeping a clean non-interactive binary-only path for CI and headless environments.

## Target behavior

### Stage 1: Install the binary first

Unchanged from current installer behavior:

1. Detect OS (`darwin`/`linux`) and CPU (`arm64`/`amd64`).
2. Resolve download URL (`TUSE_RELEASE_URL` > `TUSE_VERSION` > latest).
3. Download `tuse_<os>_<arch>.tar.gz` and `checksums.txt`, verify SHA-256, extract `tuse`, and install it with mode `0755` into `TUSE_BIN_DIR` (default `$HOME/.local/bin`).
4. Print `Installed <version> to <bin_dir>/tuse` and, if `bin_dir` is not in `PATH`, `Add <bin_dir> to your PATH.`.

**Hard barrier:** If any step in Stage 1 fails, exit immediately with a non-zero status. Do not prompt, do not create `~/.config/tuse/`, and do not write or modify any config file.

### Stage 2: Check existing global config, interactivity, and binary capability

Run only after Stage 1 succeeds:

1. **Preserve existing global configs:** Check whether `$HOME/.config/tuse.lua` OR `$HOME/.config/tuse/init.lua` already exists (`-e` or `-L`, so even a symlink or dangling symlink is treated as present).
   - If either exists, print `Keeping existing config: <path>`, do **not** prompt, do **not** modify either file, and exit 0.
2. **Non-interactive binary-only path:** Skip first-run prompts and do **not** create `~/.config/tuse/init.lua` when any of the following is true:
   - **Decision:** `TUSE_NO_SETUP` is set to a non-empty value other than `0` or `false`, OR `--no-setup` is passed to the script (e.g. `curl ... | sh -s -- --no-setup`).
   - **Decision:** `CI` environment variable is set to a non-empty value other than `0` or `false`.
   - No interactive terminal is available: stdin is not a TTY (`! [ -t 0 ]`) **and** `/dev/tty` cannot be opened for reading.
3. **Gate setup on installed binary capability (pinned older releases):**
   - Because `release/install.sh` can be invoked with `TUSE_VERSION` or `TUSE_RELEASE_URL` pointing to an older binary that does not recognize the reserved bundled plugin ID (`"tuse/default"` per [07-default-plugin-theme-subs.md](07-default-plugin-theme-subs.md)), the installer must verify capability offline before prompting.
   - **Decision:** Write a temporary probe file returning `{ plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }` and run `"$bin_dir/tuse" models --config "$probe"` with stdout/stderr suppressed (validates config offline without creating a session directory). If the probe exits non-zero, print `Installed tuse does not include the bundled default plugin; skipping first-run setup.`, do **not** prompt, do **not** write an incompatible config, and exit 0.
4. **Interactive input source (including piped `curl ... | sh`):**
   - **Decision:** Read answers from stdin when stdin is a TTY (`[ -t 0 ]`); otherwise open `/dev/tty` for prompt input (when piped as `curl -fsSL ... | sh`, stdin carries the script itself, so reading from `/dev/tty` avoids consuming script lines). Write prompts to `/dev/tty` when reading from `/dev/tty` (or stderr/stdout when reading from TTY stdin).

### Stage 3: Interactive plugin selection

When interactive, no global config exists, and the installed binary supports `"tuse/default"`:

1. Present the two bundled default plugin options from [07-default-plugin-theme-subs.md](07-default-plugin-theme-subs.md), with clear skip options:
   - **Decision (prompt flow & defaults):**
     - Prompt 1: `Enable default theme plugin (theme)? [Y/n]` (default: `yes` on empty Enter; accepts `y`/`yes` or `n`/`no`, case-insensitive; reprompts on unrecognized input).
     - Prompt 2: `Enable CLIProxyAPI provider plugin (subs)? [y/N]` (default: `no` on empty Enter, matching TODO #7's requirement that `subs` is an explicit opt-in).
   - Answering `n` to both skips all plugins and proceeds to creating a minimal config.
   - **No eager side effects:** Selecting `subs` only writes its plugin configuration into `~/.config/tuse/init.lua`. Per TODO #7, the installer must **not** install CLIProxyAPI, start any background proxy process, or trigger login flows during installation.

### Stage 4: Atomic, no-clobber creation of `~/.config/tuse/init.lua` (final step only)

Only after prompts complete successfully:

1. **Generated content:**
   - **None selected (`theme = false`, `subs = false`):** Minimal valid config:
     ```lua
     return {}
     ```
   - **One or both selected:** Valid Lua config using the single reserved offline plugin ID `"tuse/default"` from [07-default-plugin-theme-subs.md](07-default-plugin-theme-subs.md), always writing **both** booleans explicitly (in particular, `subs`-only **must** include `theme = false` because `opts.theme` defaults to `true` when omitted):
     ```lua
     return {
       plugins = {
         { "tuse/default", opts = { theme = true, subs = false } },
       },
     }
     ```
     (substituting the selected `true`/`false` values for `theme` and `subs`). Because `"tuse/default"` is bundled into the binary, the generated config loads offline on the first run of `tuse` without requiring `git clone`.
2. **Atomic and no-clobber publication (including concurrent config creation while prompting):**
   - **Decision:** Never overwrite an existing config file with `mv`. Instead:
     1. Re-check whether `$HOME/.config/tuse.lua` or `$HOME/.config/tuse/init.lua` appeared while the user was answering prompts (`-e` or `-L`); if either exists, print `Keeping existing config: <path>` and exit 0 without modifying it.
     2. Ensure `$HOME/.config/tuse` exists (`mkdir -p`) and write the generated content to a uniquely named temporary file in `$HOME/.config/tuse` (cleaned up on exit/signal via `trap`).
     3. Atomically publish without clobbering using `ln "$tmp_cfg" "$HOME/.config/tuse/init.lua"` (atomic `EEXIST` failure on POSIX if `init.lua` was created concurrently) and remove `$tmp_cfg`. If `ln` fails because `init.lua` already exists, remove `$tmp_cfg`, print `Keeping existing config: $HOME/.config/tuse/init.lua`, and exit 0. (If the filesystem does not support hard links, fall back to POSIX noclobber `set -C` creation guarded by re-checking existence.)
     4. If `$HOME/.config/tuse.lua` appeared in the narrow race right before `ln`, keep `$HOME/.config/tuse.lua` untouched and note that it takes precedence over `init.lua`.
3. Print `Created <HOME>/.config/tuse/init.lua` and concise next steps (`tuse connect ...`, or starting `tuse` and running `/subs` if `subs` was selected).

## Defaults

- `TUSE_BIN_DIR`: `$HOME/.local/bin` (unchanged).
- `TUSE_VERSION`: latest release (unchanged).
- `TUSE_RELEASE_URL`: latest GitHub release download URL (unchanged).
- `TUSE_NO_SETUP`: unset (interactive setup enabled when a terminal is available and `CI` is unset).
- Prompt defaults: `theme` = `Yes`, `subs` = `No`.

## Edge cases

- **Piped install (`curl ... | sh`):** Script body comes from stdin; interactive prompts read from `/dev/tty` so piped installation works interactively without hanging or eating script lines.
- **Re-running installer to upgrade:** Binary is updated in Stage 1, then Stage 2 detects the existing `~/.config/tuse.lua` or `~/.config/tuse/init.lua` and preserves it untouched without prompting.
- **Symlinked (including dangling symlink) existing config:** Treated as an existing config (`-e` or `-L`) and preserved untouched.
- **Pinned older release (`TUSE_VERSION=0.9.2`):** Binary installs normally, capability probe detects lack of `"tuse/default"`, setup is skipped cleanly without writing a broken config.
- **Concurrent config creation while prompting:** Re-check plus atomic no-clobber `ln` ensures neither `~/.config/tuse.lua` nor `~/.config/tuse/init.lua` is ever overwritten.
- **Interrupted prompt (`Ctrl+C` or EOF):**
  - **Decision:** On EOF during prompting, skip config creation, print `Skipped first-run setup.`, and exit 0 (leaving the installed binary intact and no `~/.config/tuse/init.lua` created). On `SIGINT`, clean up temp files without creating `~/.config/tuse/init.lua` and exit non-zero (130).
- **Unwritable `~/.config`:** Binary install already succeeded; failing to create `~/.config/tuse/init.lua` prints an error to stderr, removes any temp file, and exits non-zero.
- **POSIX `sh` compatibility:** Installer must remain valid POSIX `sh` (`sh -n release/install.sh` passes, works under `dash`, `ash`, and macOS `/bin/sh`).

## Error reporting

- Stage 1 errors: same stderr messages and exit status 1 as today (`tuse: unsupported OS ...`, `tuse: unsupported CPU ...`, `tuse: checksum mismatch for ...`, `curl`/`tar`/`install` failures).
- Unrecognized prompt answer: prints `Please answer y or n.` and re-prompts.
- Config write failure: prints `tuse: failed to write <path>: ...` to stderr and exits 1.

## Baseline docs to update before implementation

Before handing off implementation (docs-first gate), update and review:

- [../install.md](../install.md) — `Install with the shell script` and `Post-install setup` sections.
- [../env.md](../env.md) — add `TUSE_NO_SETUP` (and `CI` behavior) to the environment variables table.

## Acceptance tests

1. Interactive fresh install selecting both `theme` and `subs`: installs binary, writes valid `~/.config/tuse/init.lua` with `{ "tuse/default", opts = { theme = true, subs = true } }` (passes `tuse models` config validation), does not install/start CLIProxyAPI.
2. Interactive fresh install selecting `theme` only (`Enter`, `Enter` or `y`, `n`): writes valid `~/.config/tuse/init.lua` with `{ "tuse/default", opts = { theme = true, subs = false } }`.
3. Interactive fresh install selecting `subs` only (`n`, `y`): writes valid `~/.config/tuse/init.lua` **explicitly** setting `theme = false` and `subs = true` (`{ "tuse/default", opts = { theme = false, subs = true } }`; omitting `theme` fails this requirement because `theme` defaults to `true`).
4. Interactive fresh install skipping both (`n`, `n`): writes minimal valid `~/.config/tuse/init.lua` (`return {}`).
5. Piped execution (`cat release/install.sh | sh` under a PTY): installs binary and reads interactive answers from `/dev/tty` cleanly without corrupting script execution.
6. Existing `~/.config/tuse.lua` (regular file or symlink): installs binary, does not prompt, leaves `~/.config/tuse.lua` byte-for-byte identical, does not create `~/.config/tuse/init.lua`.
7. Existing `~/.config/tuse/init.lua` (regular file or symlink): installs binary, does not prompt, leaves `~/.config/tuse/init.lua` byte-for-byte identical.
8. Concurrent config creation while prompting: if `~/.config/tuse/init.lua` or `~/.config/tuse.lua` is created before final publication, the existing file is preserved untouched (no-clobber) and no stray temp file remains.
9. Pinned older binary without bundled `"tuse/default"` support: installs binary, capability probe detects incompatibility, skips plugin prompts, writes no broken config, exits 0.
10. Non-interactive paths (`TUSE_NO_SETUP=1`, `--no-setup`, `CI=true`, and closed stdin with no `/dev/tty`): installs binary, does not prompt, does not create `~/.config/tuse/init.lua`, exits 0.
11. Failed binary installation (bad checksum, missing release archive, or unsupported OS/CPU): exits non-zero, does not prompt, does not create `~/.config/tuse/` or `init.lua`.
12. Interrupted prompt (EOF or `SIGINT`): leaves installed binary intact, leaves no `~/.config/tuse/init.lua` or stray temp file.
13. `sh -n release/install.sh` succeeds.
