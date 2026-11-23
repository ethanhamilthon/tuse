#!/bin/sh
set -eu

target="${1:?usage: sh release/package.sh TARGET ARCHIVE}"
archive="${2:?usage: sh release/package.sh TARGET ARCHIVE}"
bin="${CARGO_TARGET_DIR:-target}/$target/release/tuse"
case "$target" in
  aarch64-apple-darwin) format='Mach-O.*arm64' ;;
  x86_64-apple-darwin) format='Mach-O.*x86_64' ;;
  aarch64-unknown-linux-gnu) format='ELF.*ARM aarch64' ;;
  x86_64-unknown-linux-gnu) format='ELF.*x86-64' ;;
  *) echo "Unsupported target: $target" >&2; exit 1 ;;
esac
file -b "$bin" | grep -E "$format"
tar -C "$(dirname "$bin")" -czf "$archive" tuse
