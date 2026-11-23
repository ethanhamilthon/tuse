#!/bin/sh
set -eu

tag="${1:?usage: sh release/check-tag.sh TAG}"
version="$(cargo metadata --locked --no-deps --format-version 1 | python3 -c \
  'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "tuse"))')"
test "v$version" = "$tag" || { echo "tag $tag != Cargo version v$version" >&2; exit 1; }
grep -Fx "## $version" release/CHANGELOG.md >/dev/null
test -s release/main.lua
