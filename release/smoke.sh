#!/bin/sh
set -eu

archive="${1:?usage: sh release/smoke.sh ARCHIVE TAG}"
tag="${2:?usage: sh release/smoke.sh ARCHIVE TAG}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
test "$(tar -tzf "$archive")" = tuse
tar -xzf "$archive" -C "$tmp"
test "$("$tmp/tuse" version)" = "tuse ${tag#v}"
cd "$tmp"
unset TUSE_GLOBAL_CONFIG
export HOME="$tmp" TUSE_HOME="$tmp/state"
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
  '{"jsonrpc":"2.0","id":2,"method":"shutdown"}' > request.jsonl
./tuse json < request.jsonl > response.jsonl
python3 -c 'import json, sys
with open("response.jsonl") as f:
    init, shutdown = map(json.loads, f)
assert init["id"] == 1 and init["result"]["tuse"] == sys.argv[1]
assert shutdown["id"] == 2 and shutdown["result"] is None
' "${tag#v}"
