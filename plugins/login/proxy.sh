set -eu
umask 077
base=http://127.0.0.1:18317
ready() {
  curl -fsS --noproxy '*' --max-time 1 --config "$STATE/proxy.curl" "$base/v1/models" >/dev/null 2>&1
}
private_dir() {
  [ ! -L "$1" ] && mkdir -p "$1" && chmod 700 "$1"
}
setup() {
  private_dir "$STATE"
  private_dir "$STATE/auth"
  private_dir "$STATE/clients"
}
identity() {
  ps -p "$1" -o lstart= -o stat= | sed -n '/[^[:space:]]/p' | grep -vE ' Z[^ ]* *$'
}
bin() {
  if [ -x "$STATE/cli-proxy-api" ]; then printf '%s\n' "$STATE/cli-proxy-api"; else command -v cli-proxy-api; fi
}
install() {
  case "$(uname -s)/$(uname -m)" in
    Darwin/arm64) platform=darwin_aarch64; digest=95d746c8d6cb418a88cb79937ca90194388faa707c30e3ec53e7d41f469cd6ec ;;
    Darwin/x86_64) platform=darwin_amd64; digest=3e4ae8dc26676ada1db8b5f8e95bf097670e7aefb0bf6c3f5a3340c00cb42f0e ;;
    Linux/aarch64) platform=linux_aarch64; digest=41245fa2af02d68c69376afc6360af49b58c3b2368d8cec1d61f0e43de14d580 ;;
    Linux/x86_64) platform=linux_amd64; digest=851e41c9a087b60e6586fec0c6be821b61921e8da0e84b9490ef542042db0756 ;;
    *) exit 2 ;;
  esac
  tmp=$(mktemp -d "$STATE/install.XXXXXXXX")
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL --proto '=https' --max-time 100 --max-filesize 157286400 -o "$tmp/asset" "https://github.com/router-for-me/CLIProxyAPI/releases/download/v8.1.2/CLIProxyAPI_8.1.2_${platform}.tar.gz"
  actual=$(openssl dgst -sha256 "$tmp/asset"); [ "${actual##* }" = "$digest" ] || exit 3
  tar -xOzf "$tmp/asset" cli-proxy-api > "$tmp/cli-proxy-api"
  [ -s "$tmp/cli-proxy-api" ]; chmod 700 "$tmp/cli-proxy-api"
  mv -f "$tmp/cli-proxy-api" "$STATE/cli-proxy-api"
}
supervise() {
  child=''
  mkdir "$STATE/supervisor" 2>/dev/null || exit 0
  trap 'if [ -n "$child" ]; then kill "$child" 2>/dev/null || :; wait "$child" || :; fi; rm -f "$STATE/supervisor/pid"; rmdir "$STATE/supervisor"' EXIT
  trap 'exit 0' TERM INT HUP
  printf '%s\n' "$$" > "$STATE/supervisor/pid"
  local client owner stamp live
  while [ -f "$STATE/enabled" ]; do
    live=''
    for client in "$STATE/clients/"*; do
      [ -f "$client" ] || continue
      owner=${client##*/}
      case "$owner" in *[!0-9]*|'') continue ;; esac
      stamp=$(identity "$owner") || stamp=''
      if [ -n "$stamp" ] && [ "$stamp" = "$(cat "$client" 2>/dev/null)" ]; then live=1; else rm -f "$client"; fi
    done
    [ -n "$live" ] || break
    if [ -z "$child" ]; then
      "$(bin)" -config "$STATE/proxy.yaml" >/dev/null 2>&1 &
      child=$!
    else
      kill -0 "$child" 2>/dev/null || break
    fi
    sleep 1
  done
}
attach() {
  [ -f "$STATE/enabled" ] || return 0
  bin >/dev/null
  stamp=$(identity "$PPID")
  [ -n "$stamp" ]
  tmp=$(mktemp "$STATE/lease.XXXXXXXX")
  printf '%s\n' "$stamp" > "$tmp"
  mv -f "$tmp" "$STATE/clients/$PPID"
  for attempt in 0 1 2 3 4 5 6 7 8 9 10 11; do
    ready && return 0
    case "$attempt" in 0|4|8) bash "$0" supervise </dev/null >/dev/null 2>&1 & ;; esac
    sleep 0.35
  done
  return 1
}
case "$1" in
  binary) bin ;;
  setup) setup ;;
  prepare)
    setup
    mkdir "$STATE/preparing" || exit 1
    trap 'rmdir "$STATE/preparing"' EXIT
    if [ ! -f "$STATE/proxy.yaml" ]; then
      key=$(openssl rand -hex 32); management=$(openssl rand -hex 32)
      printf '%s' "$key" > "$STATE/proxy.key"
      printf 'header = "Authorization: Bearer %s"\n' "$key" > "$STATE/proxy.curl"
      printf 'header = "Authorization: Bearer %s"\n' "$management" > "$STATE/management.curl"
      printf 'host: "127.0.0.1"\nport: 18317\nauth-dir: %s\napi-keys: ["%s"]\nusage-statistics-enabled: true\nremote-management:\n  allow-remote: false\n  disable-control-panel: true\n  secret-key: "%s"\n' "$AUTH_PATH" "$key" "$management" > "$STATE/proxy.yaml.tmp"
      mv -f "$STATE/proxy.yaml.tmp" "$STATE/proxy.yaml"
    fi
    ;;
  stage) mktemp -d "$STATE/oauth.XXXXXXXX" ;;
  discard) case "$2" in "$STATE"/oauth.*) rm -rf "$2" ;; *) exit 1 ;; esac ;;
  enable) : > "$STATE/enabled" ;;

  install) setup; install ;;
  attach) attach ;;
  supervise) supervise ;;
  oauth) cd "$2"; "$(bin)" -config "$2/config.yaml" "$3" </dev/null >/dev/null 2>&1 ;;
  files) for f in "$2"/*.json; do [ ! -L "$f" ] && [ -f "$f" ] && printf '%s\0' "$f"; done; exit 0 ;;
  models) curl -fsS --noproxy '*' --max-time 5 --config "$STATE/proxy.curl" "$base/v1/models" ;;
  auth-files) curl -fsS --noproxy '*' --max-time 5 --config "$STATE/management.curl" "$base/v0/management/auth-files" ;;
  quota) printf '%s' "$2" | curl -fsS --noproxy '*' --max-time 8 --config "$STATE/management.curl" -H 'Content-Type: application/json' --data-binary @- "$base/v0/management/api-call" ;;
  go-check|go-usage|go-models)
    if [ "$1" = go-check ]; then key=${OPENCODE_GO_API_KEY:-}; else key=$(cat "$STATE/opencode-go.key"); fi
    case "$key" in *[!A-Za-z0-9_.~-]*|'') exit 1 ;; esac
    resource=usage; if [ "$1" = go-models ]; then resource=models; fi
    printf 'header = "Authorization: Bearer %s"\n' "$key" | curl -fsS --proto '=https' --max-time 6 --config - "https://opencode.ai/inference/go/v1/$resource"
    ;;
  go-save)
    setup
    key=${OPENCODE_GO_API_KEY:-}
    case "$key" in *[!A-Za-z0-9_.~-]*|'') exit 1 ;; esac
    tmp=$(mktemp "$STATE/go.XXXXXXXX"); printf '%s' "$key" > "$tmp"; mv -f "$tmp" "$STATE/opencode-go.key"
    ;;
  *) exit 1 ;;
esac
