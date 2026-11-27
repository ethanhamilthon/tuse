#!/bin/sh
set -eu

if [ "$#" -ne 0 ]; then
  echo "tuse: unknown option $1" >&2
  exit 1
fi

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
trap 'exit 130' INT
trap 'exit 143' TERM

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

version="$("$bin_dir/tuse" version)"
echo "Installed $version to $bin_dir/tuse"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) echo "Add $bin_dir to your PATH." ;;
esac
choose_plugins() {
  printf '\nOptional official plugins (press Enter to skip):\n'
  for plugin in login themes status rename; do
    case "$plugin" in
      login) description='subscription sign-in, limits, and API keys' ;;
      themes) description='31 color themes' ;;
      status) description='terminal and multiplexer status' ;;
      rename) description='automatic session titles and /rename' ;;
    esac
    if [ -f "$HOME/.tuse/plugins/$plugin/tuse-plugin.json" ]; then
      printf '  %s is already installed.\n' "$plugin"
      continue
    fi
    printf '  Install %s (%s)? [y/N] ' "$plugin" "$description"
    IFS= read -r answer <&3 || return 0
    case "$answer" in
      y|Y|yes|YES|Yes)
        if ! "$bin_dir/tuse" plugins add "$repo/plugins/$plugin"; then
          printf 'tuse: could not install %s; the Tuse binary is installed.\n' "$plugin" >&2
        fi
        ;;
    esac
  done
}

if [ -t 0 ]; then
  choose_plugins 3<&0
elif [ -t 1 ]; then
  if (exec 3</dev/tty) 2>/dev/null; then
    choose_plugins 3</dev/tty
  fi
fi
