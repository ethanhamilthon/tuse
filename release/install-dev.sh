#!/bin/sh
set -eu

clean=0
case "$#:${1-}" in
  0:) ;;
  1:--clean) clean=1 ;;
  *) echo 'Usage: sh release/install-dev.sh [--clean]' >&2; exit 1 ;;
esac

case "${HOME-}" in
  /*) ;;
  *) echo 'tusedev: HOME must be an absolute path.' >&2; exit 1 ;;
esac

CDPATH= cd "$(dirname "$0")/.."
bin_dir="${BIN_DIR:-$HOME/.local/bin}"
config_dir="$HOME/.config/tusedev"
launcher="$bin_dir/tusedev"
binary="${CARGO_TARGET_DIR:-target}/release/tuse"

cargo build --locked --release -p tuse
version="$("$binary" version)"
if [ "$clean" -eq 1 ]; then
  printf 'Resetting %s and %s\n' "$HOME/.tusedev" "$config_dir"
  rm -rf -- "$HOME/.tusedev" "$config_dir"
fi
mkdir -p "$bin_dir" "$config_dir"
config="$config_dir/init.lua"
if [ ! -e "$config" ] && [ ! -L "$config" ]; then
  (umask 077; set -C; printf '%s\n' 'return {}' > "$config")
fi
install -m 755 "$binary" "$bin_dir/.tusedev-bin"
install -m 755 release/tusedev.sh "$launcher"

printf 'Installed %s to %s\n' "$version" "$launcher"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) printf 'Add %s to your PATH.\n' "$bin_dir" ;;
esac
