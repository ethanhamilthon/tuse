#!/bin/sh
# Installs the tuse binary; on a first install also sets up the config (`tuse reset`) and runs `tuse connect`.
#   curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
# TUSE_VERSION=0.3.0 picks a version, TUSE_BIN_DIR changes the binary folder (default ~/.local/bin).
# TUSE_GLOBAL_CONFIG points to the global config file (default ~/.config/tuse/main.yaml).
set -eu

repo="ethanhamilthon/tuse"
bin_dir="${TUSE_BIN_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Darwin) os=darwin ;;
  Linux) os=linux ;;
  *) echo "tuse: unsupported OS $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  arm64 | aarch64) arch=arm64 ;;
  x86_64 | amd64) arch=amd64 ;;
  *) echo "tuse: unsupported CPU $(uname -m)" >&2; exit 1 ;;
esac

if [ -n "${TUSE_RELEASE_URL:-}" ]; then
  base="$TUSE_RELEASE_URL"
elif [ -n "${TUSE_VERSION:-}" ]; then
  base="https://github.com/$repo/releases/download/v${TUSE_VERSION#v}"
else
  base="https://github.com/$repo/releases/latest/download"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
name="tuse_${os}_${arch}.tar.gz"

echo "Downloading $name"
curl -fsSL "$base/$name" -o "$tmp/$name"
curl -fsSL "$base/checksums.txt" -o "$tmp/checksums.txt"

want="$(grep " $name\$" "$tmp/checksums.txt" | cut -d' ' -f1)"
if command -v sha256sum >/dev/null 2>&1; then
  got="$(sha256sum "$tmp/$name" | cut -d' ' -f1)"
else
  got="$(shasum -a 256 "$tmp/$name" | cut -d' ' -f1)"
fi
if [ -z "$want" ] || [ "$want" != "$got" ]; then
  echo "tuse: checksum mismatch for $name" >&2
  exit 1
fi

tar -xzf "$tmp/$name" -C "$tmp" tuse
mkdir -p "$bin_dir"
install -m 755 "$tmp/tuse" "$bin_dir/tuse"

echo "Installed $("$bin_dir/tuse" version) to $bin_dir/tuse"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) echo "Add $bin_dir to your PATH." ;;
esac

# A first install: no TUSE_GLOBAL_CONFIG and no ~/.config/tuse yet.
if [ -z "${TUSE_GLOBAL_CONFIG:-}" ] && [ ! -d "$HOME/.config/tuse" ]; then
  TUSE_RELEASE_URL="$base" "$bin_dir/tuse" reset
fi

# The installer is piped from curl, so the prompts read the terminal.
if (: < /dev/tty) 2>/dev/null; then
  "$bin_dir/tuse" connect < /dev/tty || true
else
  echo "Next: tuse connect"
fi
