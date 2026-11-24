# Installation & Updates

## Supported platforms

Prebuilt release binaries are published for:

| OS | Architecture | Release archive | Baseline requirement |
| --- | --- | --- | --- |
| macOS | Apple Silicon (`arm64` / `aarch64`) | `tuse_darwin_arm64.tar.gz` | macOS 11+ |
| macOS | Intel (`x86_64` / `amd64`) | `tuse_darwin_amd64.tar.gz` | macOS 11+ |
| Linux | `x86_64` / `amd64` | `tuse_linux_amd64.tar.gz` | glibc 2.35+ |
| Linux | `aarch64` / `arm64` | `tuse_linux_arm64.tar.gz` | glibc 2.35+ |

Each archive contains a single `tuse` binary. Every GitHub release also publishes `checksums.txt` with SHA-256 checksums for all archives.

## Install with the shell script

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
```

- **Requirements:** POSIX `sh`, `uname`, `mktemp`, `curl`, `grep`, `cut`, `tr`, `ln`, `sha256sum` or `shasum`, `tar`, `mkdir`, and `install`.
- **Stage 1 (binary install first):**
  - **Platform detection:** Maps `uname -s` (`Darwin` -> `darwin`, `Linux` -> `linux`) and `uname -m` (`arm64`/`aarch64` -> `arm64`, `x86_64`/`amd64` -> `amd64`). Exits 1 on unsupported platforms.
  - **Download URL precedence:**
    1. `TUSE_RELEASE_URL` (if non-empty) — takes precedence over `TUSE_VERSION`.
    2. `TUSE_VERSION` (if non-empty, optional leading `v` stripped) -> `https://github.com/ethanhamilthon/tuse/releases/download/v<VERSION>`.
    3. Default -> `https://github.com/ethanhamilthon/tuse/releases/latest/download`.
  - **Verification and placement:** Downloads `tuse_<os>_<arch>.tar.gz` and `checksums.txt` into a temporary directory (cleaned up on exit), verifies the archive's SHA-256 digest (aborting on missing entry or mismatch before touching the destination), extracts `tuse`, creates `TUSE_BIN_DIR` (default `$HOME/.local/bin`) if needed, installs with mode `0755`, prints `Installed <version> to <bin_dir>/tuse`, and prints `Add <bin_dir> to your PATH.` when `bin_dir` is absent from `PATH`.
  - If any step in Stage 1 fails, the installer exits non-zero immediately without prompting or creating `~/.config/tuse/`.
- **Stage 2 (first-run setup gating):**
  - **Preserve existing global configs:** If `$HOME/.config/tuse.lua` or `$HOME/.config/tuse/init.lua` already exists (regular file or symlink, including dangling symlinks), prints `Keeping existing config: <path>` and exits 0 without prompting or modifying any file.
  - **Non-interactive binary-only path:** Skips prompts and creates no config file when `--no-setup` is passed (for example `curl -fsSL ... | sh -s -- --no-setup`), when `TUSE_NO_SETUP` or `CI` is set to a non-empty value other than `0` or `false`, or when neither stdin is a TTY nor `/dev/tty` is readable.
  - **Binary capability probe:** Before prompting, runs `"$bin_dir/tuse" models --config <temp_probe>` offline with `{ plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }`. On older binaries that do not include the bundled `"tuse/default"` plugin, prints `Installed tuse does not include the bundled default plugin; skipping first-run setup.` and exits 0 without creating an incompatible config.
- **Stage 3 & 4 (interactive first-run setup & atomic config write):**
  - Reads prompt answers from stdin when stdin is a TTY, or from `/dev/tty` when piped (`curl ... | sh`).
  - Prompts to enable the bundled `"tuse/default"` plugin parts: `theme` (default `Y`) and `subs` (CLIProxyAPI wrapper, default `N`). Selecting `subs` only writes its plugin entry; the installer never installs or starts CLIProxyAPI or triggers logins.
  - On EOF during prompting, prints `Skipped first-run setup.` and exits 0 with the binary intact and no config created; on `SIGINT`, cleans up temporary files and exits 130.
  - Re-checks for existing global configs and publishes `$HOME/.config/tuse/init.lua` atomically without clobbering (`ln` with POSIX `set -C` fallback): writes `return {}` if both options were skipped, or `{ plugins = { { "tuse/default", opts = { theme = <bool>, subs = <bool> } } } }` with both booleans explicit when one or both were enabled. Prints `Created <HOME>/.config/tuse/init.lua` and concise next steps.

See [env.md](env.md) for environment variable details.

## Build from source

Requires the Rust toolchain pinned in `rust-toolchain.toml` (Rust edition 2024):

- From GitHub: `cargo install --locked --git https://github.com/ethanhamilthon/tuse tuse`
- From a local checkout: `make install` (builds `--locked --release -p tuse` and installs into `BIN_DIR`, defaulting to `$HOME/.local/bin`, with mode `0755`).

## Updating (`tuse update`)

Unlike the shell installer, `tuse update` performs downloading, SHA-256 verification, and gzip/tar extraction entirely in-process (requiring no external `curl`, `tar`, or `shasum` tools). It replaces the currently running executable in place without touching `~/.tuse/tuse.db` or any Lua config files.

1. Determines the base URL from `TUSE_RELEASE_URL` (if non-empty) or `https://github.com/ethanhamilthon/tuse/releases/latest/download`. `TUSE_VERSION` and `TUSE_BIN_DIR` are not read by `tuse update`.
2. Detects the current OS and CPU (`darwin`/`linux`, `arm64`/`amd64`), prints `Downloading <asset>`, and downloads `tuse_<os>_<arch>.tar.gz` and `checksums.txt`.
3. Verifies the SHA-256 checksum against `checksums.txt` and unpacks `tuse` into a temporary directory (cleaned up afterward).
4. Copies the unpacked binary to a sibling staging file next to the current executable, attempts best-effort `0755` permissions on Unix, and atomically renames the staging file over the current executable.
5. After replacement completes, runs the new binary with `--version` to print `<old_version> -> <new_version>` (falling back to `Updated tuse` if the version check fails or returns empty output).

## Post-install setup

Defaults and the built-in system prompt are compiled into the binary; no config file is required to run Tuse. On interactive first-time installations (when the installed binary supports `"tuse/default"`), the installer can optionally create `~/.config/tuse/init.lua` for you. To start talking to a model:

1. Connect a provider: `tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai` (or, if you enabled `subs`, launch `tuse` and run `/subs` to configure CLIProxyAPI).
2. Run `tuse` and select a model and effort with `/models`.
3. Optionally customize your global config (`~/.config/tuse.lua` or `~/.config/tuse/init.lua`) or create a project config (`tuse init`).
