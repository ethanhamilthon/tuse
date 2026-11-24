local rt, store = ...
local t = require("tuse")

local U = {
    PINNED_VERSION = "v8.1.2",
    DEFAULT_RELEASE_URL = "https://github.com/router-for-me/CLIProxyAPI/releases/download/v8.1.2",
    MANIFEST = {
        ["darwin/arm64"] = { asset = "CLIProxyAPI_8.1.2_darwin_aarch64.tar.gz", sha256 = "95d746c8d6cb418a88cb79937ca90194388faa707c30e3ec53e7d41f469cd6ec" },
        ["darwin/amd64"] = { asset = "CLIProxyAPI_8.1.2_darwin_amd64.tar.gz", sha256 = "3e4ae8dc26676ada1db8b5f8e95bf097670e7aefb0bf6c3f5a3340c00cb42f0e" },
        ["linux/arm64"] = { asset = "CLIProxyAPI_8.1.2_linux_aarch64.tar.gz", sha256 = "41245fa2af02d68c69376afc6360af49b58c3b2368d8cec1d61f0e43de14d580" },
        ["linux/amd64"] = { asset = "CLIProxyAPI_8.1.2_linux_amd64.tar.gz", sha256 = "851e41c9a087b60e6586fec0c6be821b61921e8da0e84b9490ef542042db0756" },
    },
}

local function ci(w) return (w:gsub("%a", function(c) return "[" .. c:lower() .. c:upper() .. "]" end)) end
local SECRET_KEYS = { "x%-api%-key", "x%-goog%-api%-key", "api[-_]key", "access_token", "refresh_token", "id_token", "client_secret", "set%-cookie", "cookie" }

function U.redact(text, extra)
    local out = text
    for _, s in ipairs(extra or {}) do
        local tr = s:match("^%s*(.-)%s*$")
        if tr ~= "" then out = out:gsub(tr:gsub("([%^%$%(%)%%%.%[%]%*%+%-%?])", "%%%1"), "[REDACTED]") end
    end
    out = out:gsub("(" .. ci("authorization") .. "%s*:%s*" .. ci("bearer") .. "%s+)%S+", "%1[REDACTED]")
    out = out:gsub("(%f[%w]" .. ci("bearer") .. "%s+)([A-Za-z0-9%._~%+/=-]+)", function(p, v) return #v >= 8 and (p .. "[REDACTED]") or (p .. v) end)
    for _, k in ipairs(SECRET_KEYS) do out = out:gsub('("?' .. ci(k) .. '"?%s*[:=]%s*["\']?)[^%s"\',;}{]+', "%1[REDACTED]") end
    out = out:gsub('([?&]' .. ci("code") .. '=)[^&%s"\']+', "%1[REDACTED]")
    return (out:gsub("(%f[%w])(eyJ[A-Za-z0-9_%-]+%.[A-Za-z0-9_%-]+%.?[A-Za-z0-9_%-]*)", function(b, tok)
        local a, c = tok:match("^eyJ([^%.]+)%.([^%.]+)")
        return (a and #a >= 6 and c and #c >= 6) and (b .. "[REDACTED]") or (b .. tok)
    end))
end

function U.expand_home(p)
    local h = os.getenv("HOME") or ""
    return p == "~" and h or (p:sub(1, 2) == "~/" and (h .. "/" .. p:sub(3)) or p)
end

function U.endpoint(cfg) return ("http://%s:%d/v1"):format(cfg.host == "::1" and "[::1]" or cfg.host, cfg.port) end
function U.dir_of(cfg) return (U.expand_home(cfg.config_dir):gsub("/+$", "")) end
function U.target_key(cfg, bin) return table.concat({ cfg.host, tostring(cfg.port), U.dir_of(cfg), bin or "" }, "\0") end

function U.random_hex(n)
    local f = assert(io.open("/dev/urandom", "rb"))
    local raw = f:read(n); f:close()
    return (raw:gsub(".", function(c) return ("%02x"):format(c:byte()) end))
end

function U.exec(argv, opts)
    local p, lines = rt.spawn(argv, opts), {}
    while true do
        local ev = rt.next(p)
        if ev.line then lines[#lines + 1] = ev.line end
        if ev.exited then return { ok = ev.code == 0 and not ev.timed_out, code = ev.code, timed_out = ev.timed_out, out = table.concat(lines, "\n") } end
    end
end

function U.mkdir(dir, mode) assert(U.exec({ "mkdir", "-p", dir }).ok and U.exec({ "chmod", mode, dir }).ok) end
function U.chmod(p, mode) assert(U.exec({ "chmod", mode, p }).ok) end
function U.rm(p) U.exec({ "rm", "-rf", p }) end
function U.is_file(p) return U.exec({ "sh", "-c", 'test -f "$1" && ! test -L "$1"', "_", p }).ok end
function U.tighten_dir(dir) U.exec({ "sh", "-c", 'test -d "$1" && find "$1" -maxdepth 1 -type f -exec chmod 600 {} +', "_", dir }) end

function U.sha256(p)
    local r = U.exec({ "sh", "-c", 'if command -v sha256sum >/dev/null 2>&1; then sha256sum -- "$1"; else shasum -a 256 -- "$1"; fi', "_", p }, { timeout = 30 })
    local hex = r.ok and r.out:match("^(%x+)")
    return hex and hex:lower() or error(p .. ": sha256 failed")
end

function U.list_files(dir)
    local r = U.exec({ "sh", "-c", 'test -d "$1" && find "$1" -maxdepth 1 -type f -exec basename {} \\; | sort', "_", dir })
    local out = {}
    if r.ok then for l in r.out:gmatch("[^\n]+") do out[#out + 1] = l end end
    return out
end

function U.write_secret(path, content)
    local dir, base = path:match("^(.*)/([^/]+)$")
    local tmp = (dir and (dir .. "/.") or ".") .. (base or "subs") .. "." .. U.random_hex(8) .. ".tmp"
    local r = U.exec({ "sh", "-c", 'umask 077 && cat > "$1"', "_", tmp }, { stdin = content })
    if not r.ok then os.remove(tmp); error(path .. ": write failed") end
    local ok, err = os.rename(tmp, path)
    if not ok then os.remove(tmp); error(err) end
end

function U.resolve_bin(cfg)
    if cfg.binary then local p = U.expand_home(cfg.binary); return rt.is_exec(p) and p or nil end
    local installed = U.dir_of(cfg) .. "/bin/cli-proxy-api"
    if rt.is_exec(installed) then return installed end
    for d in (os.getenv("PATH") or ""):gmatch("[^:]+") do
        if rt.is_exec(d .. "/cli-proxy-api") then return d .. "/cli-proxy-api" end
    end
    return nil
end

function U.read_key(cfg)
    local f = io.open(U.dir_of(cfg) .. "/api.key", "rb")
    if not f then return nil end
    local k = (f:read("a") or ""):gsub("%s+$", ""); f:close()
    return k ~= "" and k or nil
end

function U.secrets(cfg) local k = U.read_key(cfg); return k and { k } or {} end
function U.show(cfg, md) t.show("subs", U.redact(md, U.secrets(cfg)), false) end
function U.fail(cfg, msg)
    local clean = U.redact(msg, U.secrets(cfg))
    store.last_error = clean
    U.show(cfg, "**subs error:** " .. clean)
end

function U.probe(cfg)
    local key = U.read_key(cfg)
    local res = rt.http(U.endpoint(cfg) .. "/models", key and { Authorization = "Bearer " .. key } or nil, 2)
    if not res.ok then return { reachable = false, err = U.redact(res.err or "unreachable", U.secrets(cfg)) } end
    if res.status == 200 then
        local ok, parsed = pcall(t.json.decode, res.body or "")
        if ok and type(parsed) == "table" then
            return { reachable = true, ok = true, status = 200, models = type(parsed.data) == "table" and #parsed.data or 0 }
        end
        return { reachable = true, ok = false, status = 200, err = "invalid JSON from /v1/models" }
    end
    return { reachable = true, ok = false, status = res.status, err = ("HTTP %d"):format(res.status or 0) }
end

function U.detect_version(bin)
    for _, flag in ipairs({ "--help", "--version" }) do
        local out = U.exec({ bin, flag }, { timeout = 3 }).out or ""
        local ver = out:match("CLIProxyAPI%s+Version:%s*([^,%s]+)") or out:match("v?(%d+%.%d+%.%d[%w%.%-]*)")
        if ver then return ver:sub(1, 1) == "v" and ver or ("v" .. ver) end
    end
    return "unknown"
end

function U.find_provider(name)
    local r = U.exec({ rt.tuse_bin, "providers", "--json" }, { timeout = 10 })
    if not r.ok then return nil, r.out ~= "" and r.out or "failed to list providers" end
    local ok, rows = pcall(t.json.decode, r.out)
    if not ok or type(rows) ~= "table" then return nil, "invalid JSON from `tuse providers --json`" end
    for _, row in ipairs(rows) do if row.name == name then return row, nil end end
    return nil, nil
end

function U.task_state(id) local ok, info = pcall(t.task_get, id, 1); return ok and type(info) == "table" and info.status or nil end

function U.write_config(cfg)
    local dir, auth = U.dir_of(cfg), U.dir_of(cfg) .. "/auth"
    local esc = function(s) return (s:gsub("\\", "\\\\"):gsub('"', '\\"')) end
    U.mkdir(dir, "700"); U.mkdir(auth, "700")
    local key = U.random_hex(32)
    U.write_secret(dir .. "/api.key", key .. "\n")
    U.write_secret(dir .. "/config.yaml", ('host: "%s"\nport: %d\nauth-dir: "%s"\napi-keys:\n  - "%s"\nremote-management:\n  allow-remote: false\n  disable-control-panel: true\n  secret-key: ""\n'):format(esc(cfg.host), cfg.port, esc(auth), key))
end

function U.reload_after(cfg, db_msg, ok_msg)
    local ok, rerr = pcall(t.reload)
    if not ok then
        local clean = U.redact(tostring(rerr), U.secrets(cfg))
        store.last_error = clean
        U.show(cfg, ("%s, but config reload failed: %s. Fix the config/plugin issue and run `/reload` to activate it in this session."):format(db_msg, clean))
        return
    end
    store.last_error = nil
    U.show(cfg, ok_msg)
end

return U
