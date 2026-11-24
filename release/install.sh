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
