#!/bin/sh
set -eu

case "${1-}" in
  update|rollback)
    echo 'tusedev: run make install-dev to rebuild from your checkout.' >&2
    exit 1
    ;;
esac

export TUSE_HOME="${HOME:?HOME must be set}/.tusedev"
export TUSE_GLOBAL_CONFIG="$HOME/.config/tusedev/init.lua"
exec "$(dirname "$0")/.tusedev-bin" "$@"
