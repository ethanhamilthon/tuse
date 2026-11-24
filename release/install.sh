#!/bin/sh
set -eu

no_setup=0
for arg in "$@"; do
  case "$arg" in
    --no-setup) no_setup=1 ;;
    *) echo "tuse: unknown option $arg" >&2; exit 1 ;;
  esac
done

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
tmp_cfg=""
cleanup() {
  rm -rf "$tmp"
  if [ -n "$tmp_cfg" ]; then
    rm -f "$tmp_cfg"
  fi
}
trap cleanup EXIT
trap 'cleanup; trap - INT; exit 130' INT
trap 'cleanup; trap - TERM; exit 143' TERM

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

existing_config() {
  if [ -e "$HOME/.config/tuse.lua" ] || [ -L "$HOME/.config/tuse.lua" ]; then
    printf '%s\n' "$HOME/.config/tuse.lua"
  elif [ -e "$HOME/.config/tuse/init.lua" ] || [ -L "$HOME/.config/tuse/init.lua" ]; then
    printf '%s\n' "$HOME/.config/tuse/init.lua"
  fi
}

is_enabled_flag() {
  case "${1:-}" in
    "" | 0 | false | FALSE | False) return 1 ;;
    *) return 0 ;;
  esac
}

existing="$(existing_config)"
if [ -n "$existing" ]; then
  echo "Keeping existing config: $existing"
  exit 0
fi

if [ "$no_setup" -eq 1 ] || is_enabled_flag "${TUSE_NO_SETUP:-}" || is_enabled_flag "${CI:-}"; then
  exit 0
fi

if [ -t 0 ]; then
  exec 3<&0
  prompt_out=""
elif (exec 3</dev/tty) 2>/dev/null; then
  exec 3</dev/tty
  if (exec 4>/dev/tty) 2>/dev/null; then
    exec 4>/dev/tty
    prompt_out="tty"
  else
    prompt_out=""
  fi
else
  exit 0
fi

probe="$tmp/probe.lua"
printf '%s\n' 'return { plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }' > "$probe"
if ! "$bin_dir/tuse" models --config "$probe" >/dev/null 2>&1; then
  echo "Installed tuse does not include the bundled default plugin; skipping first-run setup."
  exit 0
fi

write_prompt() {
  if [ "$prompt_out" = "tty" ]; then
    printf '%s' "$1" >&4
  else
    printf '%s' "$1"
  fi
}

write_prompt_line() {
  if [ "$prompt_out" = "tty" ]; then
    printf '%s\n' "$1" >&4
  else
    printf '%s\n' "$1"
  fi
}

ask_yn() {
  question="$1"
  default="$2"
  while :; do
    write_prompt "$question "
    if ! IFS= read -r ans <&3; then
      write_prompt_line ""
      echo "Skipped first-run setup."
      exit 0
    fi
    ans="$(printf '%s' "$ans" | tr '\r\t' '  ' | tr '[:upper:]' '[:lower:]')"
    while [ "${ans# }" != "$ans" ]; do
      ans="${ans# }"
    done
    while [ "${ans% }" != "$ans" ]; do
      ans="${ans% }"
    done
    case "$ans" in
      "")
        answer="$default"
        return 0
        ;;
      y | yes)
        answer="true"
        return 0
        ;;
      n | no)
        answer="false"
        return 0
        ;;
      *)
        write_prompt_line "Please answer y or n."
        ;;
    esac
  done
}

ask_yn "Enable default theme plugin (theme)? [Y/n]" "true"
use_theme="$answer"
ask_yn "Enable CLIProxyAPI provider plugin (subs)? [y/N]" "false"
use_subs="$answer"

existing="$(existing_config)"
if [ -n "$existing" ]; then
  echo "Keeping existing config: $existing"
  exit 0
fi

cfg_dir="$HOME/.config/tuse"
cfg_path="$cfg_dir/init.lua"
if ! mkdir -p "$cfg_dir" 2>"$tmp/err"; then
  err="$(cat "$tmp/err" 2>/dev/null || true)"
  echo "tuse: failed to write $cfg_path: $err" >&2
  exit 1
fi

if ! tmp_cfg="$(mktemp "$cfg_dir/.init.lua.tmp.XXXXXX" 2>"$tmp/err")"; then
  err="$(cat "$tmp/err" 2>/dev/null || true)"
  echo "tuse: failed to write $cfg_path: $err" >&2
  exit 1
fi

if [ "$use_theme" = "false" ] && [ "$use_subs" = "false" ]; then
  printf 'return {}\n' > "$tmp_cfg" 2>"$tmp/err" || {
    err="$(cat "$tmp/err" 2>/dev/null || true)"
    echo "tuse: failed to write $cfg_path: $err" >&2
    exit 1
  }
else
  printf 'return {\n  plugins = {\n    { "tuse/default", opts = { theme = %s, subs = %s } },\n  },\n}\n' \
    "$use_theme" "$use_subs" > "$tmp_cfg" 2>"$tmp/err" || {
    err="$(cat "$tmp/err" 2>/dev/null || true)"
    echo "tuse: failed to write $cfg_path: $err" >&2
    exit 1
  }
fi

if ln "$tmp_cfg" "$cfg_path" 2>"$tmp/err"; then
  rm -f "$tmp_cfg"
  tmp_cfg=""
else
  if [ -e "$cfg_path" ] || [ -L "$cfg_path" ]; then
    rm -f "$tmp_cfg"
    tmp_cfg=""
    echo "Keeping existing config: $cfg_path"
    exit 0
  fi
  if (set -C; cat "$tmp_cfg" > "$cfg_path") 2>"$tmp/err2"; then
    rm -f "$tmp_cfg"
    tmp_cfg=""
  elif [ -e "$cfg_path" ] || [ -L "$cfg_path" ]; then
    rm -f "$tmp_cfg"
    tmp_cfg=""
    echo "Keeping existing config: $cfg_path"
    exit 0
  else
    err="$(cat "$tmp/err2" 2>/dev/null || cat "$tmp/err" 2>/dev/null || true)"
    echo "tuse: failed to write $cfg_path: $err" >&2
    exit 1
  fi
fi

echo "Created $cfg_path"
if [ -e "$HOME/.config/tuse.lua" ] || [ -L "$HOME/.config/tuse.lua" ]; then
  echo "Note: $HOME/.config/tuse.lua already exists and takes precedence over $cfg_path."
fi
if [ "$use_subs" = "true" ]; then
  echo "Next: start tuse and run /subs to set up CLIProxyAPI, or connect a provider with:"
  echo "  tuse connect --base https://api.openai.com/v1 --key '\$OPENAI_API_KEY' --name openai"
else
  echo "Next: connect a provider and start tuse:"
  echo "  tuse connect --base https://api.openai.com/v1 --key '\$OPENAI_API_KEY' --name openai"
fi
