local rt = ...
local t = require("tuse")

local PINNED_VERSION = "v8.1.2"
local DEFAULT_RELEASE_URL = "https://github.com/router-for-me/CLIProxyAPI/releases/download/v8.1.2"
local MANIFEST = {
    ["darwin/arm64"] = {
        asset = "CLIProxyAPI_8.1.2_darwin_aarch64.tar.gz",
        sha256 = "95d746c8d6cb418a88cb79937ca90194388faa707c30e3ec53e7d41f469cd6ec",
    },
    ["darwin/amd64"] = {
        asset = "CLIProxyAPI_8.1.2_darwin_amd64.tar.gz",
        sha256 = "3e4ae8dc26676ada1db8b5f8e95bf097670e7aefb0bf6c3f5a3340c00cb42f0e",
    },
    ["linux/arm64"] = {
        asset = "CLIProxyAPI_8.1.2_linux_aarch64.tar.gz",
        sha256 = "41245fa2af02d68c69376afc6360af49b58c3b2368d8cec1d61f0e43de14d580",
    },
    ["linux/amd64"] = {
        asset = "CLIProxyAPI_8.1.2_linux_amd64.tar.gz",
        sha256 = "851e41c9a087b60e6586fec0c6be821b61921e8da0e84b9490ef542042db0756",
    },
}

local function endpoint_of(cfg)
    local h = cfg.host == "::1" and "[::1]" or cfg.host
    return ("http://%s:%d/v1"):format(h, cfg.port)
end

local function dir_of(cfg)
    return rt.expand_home(cfg.config_dir):gsub("/+$", "")
end

local function resolve_binary(cfg)
    if cfg.binary then
        local p = rt.expand_home(cfg.binary)
        return rt.is_exec(p) and p or nil
    end
    local installed = dir_of(cfg) .. "/bin/cli-proxy-api"
    if rt.is_exec(installed) then
        return installed
    end
    return rt.which("cli-proxy-api")
end

local function read_key(cfg)
    local raw = rt.read_file(dir_of(cfg) .. "/api.key")
    if not raw then return nil end
    local k = raw:gsub("%s+$", "")
    return k ~= "" and k or nil
end

local function secrets_of(cfg)
    local k = read_key(cfg)
    return k and { k } or {}
end

local function show_subs(cfg, md)
    t.show("subs", rt.redact(md, secrets_of(cfg)), false)
end

local function fail(cfg, msg)
    local clean = rt.redact(msg, secrets_of(cfg))
    rt.set_last_error(clean)
    show_subs(cfg, "**subs error:** " .. clean)
end

local function probe(cfg)
    local key = read_key(cfg)
    local headers = key and { Authorization = "Bearer " .. key } or nil
    local res = rt.probe_get(endpoint_of(cfg) .. "/models", headers, 2)
    if not res.ok then
        return { reachable = false, err = rt.redact(res.err or "unreachable", secrets_of(cfg)) }
    end
    if res.status == 200 then
        local ok, parsed = pcall(t.json.decode, res.body or "")
        if ok and type(parsed) == "table" then
            local count = type(parsed.data) == "table" and #parsed.data or 0
            return { reachable = true, ok = true, status = 200, models = count }
        end
        return { reachable = true, ok = false, status = 200, err = "invalid JSON from /v1/models" }
    end
    return {
        reachable = true,
        ok = false,
        status = res.status,
        err = ("HTTP %d"):format(res.status or 0),
    }
end

local function detect_version(bin)
    for _, flag in ipairs({ "--help", "--version" }) do
        local r = rt.exec({ bin, flag }, { timeout = 3 })
        local out = r.out or ""
        local ver = out:match("CLIProxyAPI%s+Version:%s*([^,%s]+)")
            or out:match("v?(%d+%.%d+%.%d[%w%.%-]*)")
        if ver then
            return ver:sub(1, 1) == "v" and ver or ("v" .. ver)
        end
    end
    return "unknown"
end

local function list_providers()
    local r = rt.exec({ rt.tuse_bin(), "providers", "--json" }, { timeout = 10 })
    if not r.ok then
        return nil, r.out ~= "" and r.out or "failed to list providers"
    end
    local ok, rows = pcall(t.json.decode, r.out)
    if not ok or type(rows) ~= "table" then
        return nil, "invalid JSON from `tuse providers --json`"
    end
    return rows, nil
end

local function find_provider(name)
    local rows, err = list_providers()
    if not rows then return nil, err end
    for _, row in ipairs(rows) do
        if row.name == name then
            return row, nil
        end
    end
    return nil, nil
end

local function task_state(id)
    local ok, info = pcall(t.task_get, id, 1)
    if not ok or type(info) ~= "table" then return nil end
    return info.status
end

local function yaml_escape(s)
    return (s:gsub("\\", "\\\\"):gsub('"', '\\"'))
end

local function write_config(cfg)
    local dir = dir_of(cfg)
    local auth_dir = dir .. "/auth"
    rt.ensure_dir(dir, 448)
    rt.ensure_dir(auth_dir, 448)
    local key = rt.random_hex(32)
    rt.write_secret(dir .. "/api.key", key .. "\n", 384)
    local yaml = table.concat({
        ('host: "%s"'):format(yaml_escape(cfg.host)),
        ("port: %d"):format(cfg.port),
        ('auth-dir: "%s"'):format(yaml_escape(auth_dir)),
        "api-keys:",
        ('  - "%s"'):format(key),
        "remote-management:",
        "  allow-remote: false",
        "  disable-control-panel: true",
        '  secret-key: ""',
        "",
    }, "\n")
    rt.write_secret(dir .. "/config.yaml", yaml, 384)
end

local function do_status(cfg)
    local dir = dir_of(cfg)
    local bin = resolve_binary(cfg)
    local bin_line = "not installed"
    if bin then
        local ver = detect_version(bin)
        local tag = ver == PINNED_VERSION and "matches pinned" or ("pinned: " .. PINNED_VERSION)
        bin_line = ("`%s` (%s, %s)"):format(bin, ver, tag)
    end
    local cfg_path = dir .. "/config.yaml"
    local cfg_state = rt.exists(cfg_path) and "present" or "missing"
    local auth_files = rt.list_files(dir .. "/auth")
    local auth_line = #auth_files == 0 and "0 accounts"
        or ("%d file(s): %s"):format(#auth_files, table.concat(auth_files, ", "))
    local pr = probe(cfg)
    local owner = rt.get_owner()
    local proxy_line = "stopped"
    if owner then
        local st = task_state(owner.task_id)
        if st == "running" then
            proxy_line = ("running (managed, task `%s`)"):format(owner.task_id)
        elseif st == "failed" then
            proxy_line = ("failed (task `%s`)"):format(owner.task_id)
        end
    elseif pr.ok then
        proxy_line = "running (external)"
    end
    local ep = endpoint_of(cfg)
    local ep_line = pr.ok and ("`%s` (reachable, %d models)"):format(ep, pr.models)
        or ("`%s` (%s)"):format(ep, pr.err or "unreachable")
    local prov, perr = find_provider(cfg.provider)
    local prov_line = "not connected"
    if perr then
        prov_line = "error: " .. perr
    elseif prov then
        local match = prov.base_url == ep and "matches endpoint" or ("points to `" .. prov.base_url .. "`")
        prov_line = ("connected as `%s` (%s)"):format(cfg.provider, match)
    end
    local lines = {
        "### subs status",
        "",
        "- **Binary:** " .. bin_line,
        ("- **Config:** `%s` (%s) · auth: %s"):format(cfg_path, cfg_state, auth_line),
        "- **Proxy:** " .. proxy_line,
        "- **Endpoint:** " .. ep_line,
        "- **Tuse provider:** " .. prov_line,
    }
    local last_err = rt.get_last_error()
    if last_err then
        lines[#lines + 1] = "- **Last error:** " .. last_err
    end
    show_subs(cfg, table.concat(lines, "\n"))
end

local function install_binary(cfg)
    local choice = t.pick({ "Install CLIProxyAPI " .. PINNED_VERSION, "Cancel" })
    if choice ~= "Install CLIProxyAPI " .. PINNED_VERSION then
        return false, nil
    end
    local plat = rt.platform()
    local key = plat.os .. "/" .. plat.arch
    local entry = MANIFEST[key]
    if not entry then
        return false, ("unsupported platform `%s`; install CLIProxyAPI manually and set `opts.subs.binary`"):format(key)
    end
    local dir = dir_of(cfg)
    rt.ensure_dir(dir, 448)
    local stage = dir .. "/.stage-" .. rt.random_hex(8)
    rt.ensure_dir(stage, 448)
    local base = os.getenv("TUSE_SUBS_RELEASE_URL")
    if not base or base == "" then base = DEFAULT_RELEASE_URL end
    base = base:gsub("/+$", "")
    local archive = stage .. "/" .. entry.asset
    local dl = rt.download_file(base .. "/" .. entry.asset, archive, 120)
    if not dl.ok then
        rt.remove_all(stage)
        return false, ("download failed: %s; or set `opts.subs.binary`"):format(dl.err or "network error")
    end
    local got = rt.sha256_file(archive)
    if got ~= entry.sha256 then
        rt.remove_all(stage)
        return false, ("SHA-256 mismatch for %s (expected %s, got %s); refusing to install. Set `opts.subs.binary` for a custom build."):format(entry.asset, entry.sha256, got)
    end
    local ex = rt.exec({ "tar", "-xzf", archive, "-C", stage, "cli-proxy-api" }, { timeout = 30 })
    local extracted = stage .. "/cli-proxy-api"
    if not ex.ok or not rt.is_regular_file(extracted) then
        rt.remove_all(stage)
        return false, "failed to extract `cli-proxy-api` from release archive"
    end
    rt.chmod(extracted, 493)
    rt.ensure_dir(dir .. "/bin", 448)
    rt.rename(extracted, dir .. "/bin/cli-proxy-api")
    rt.remove_all(stage)
    return true, nil
end

local function do_setup(cfg)
    local bin = resolve_binary(cfg)
    if not bin then
        local ok, err = install_binary(cfg)
        if err then
            fail(cfg, err)
            return
        end
        if not ok then return end
        bin = resolve_binary(cfg)
    end
    local dir = dir_of(cfg)
    local cfg_path = dir .. "/config.yaml"
    if not rt.exists(cfg_path) then
        local c = t.pick({ "Create local config in " .. dir, "Cancel" })
        if c ~= "Create local config in " .. dir then
            return
        end
        write_config(cfg)
    else
        local c = t.pick({
            "Keep existing config",
            "Reset config (rotates local API key)",
            "Cancel",
        })
        if c == "Reset config (rotates local API key)" then
            write_config(cfg)
        elseif c ~= "Keep existing config" then
            return
        end
    end
    rt.set_last_error(nil)
    show_subs(cfg, table.concat({
        "### subs setup complete",
        "",
        ("- **Binary:** `%s`"):format(bin or "installed"),
        ("- **Config:** `%s`"):format(cfg_path),
        "- **Next steps:** run `/subs` -> `login`, then `start`, then `connect`.",
    }, "\n"))
end

local LOGIN_FLAGS = {
    ["OpenAI / Codex"] = "--codex-login",
    ["Anthropic / Claude"] = "--claude-login",
    ["Google / Gemini"] = "--antigravity-login",
}

local function do_login(cfg)
    local bin = resolve_binary(cfg)
    local dir = dir_of(cfg)
    local cfg_path = dir .. "/config.yaml"
    if not bin or not rt.exists(cfg_path) then
        fail(cfg, "binary or `config.yaml` is missing; run `/subs` -> `setup` first")
        return
    end
    local provider = t.pick({ "OpenAI / Codex", "Anthropic / Claude", "Google / Gemini", "Cancel" })
    local flag = provider and LOGIN_FLAGS[provider]
    if not flag then return end
    show_subs(cfg, ("### subs login (%s)\n\nStarting OAuth login…"):format(provider))
    local proc = rt.spawn_proc({ bin, "--config", cfg_path, "--no-browser", flag }, { timeout = 300 })
    local shown_url, shown_code
    local last_err_line
    while true do
        local ev = rt.next_proc(proc)
        if ev.line then
            local raw = ev.line
            local url = raw:match("(https://%S+)")
            if url then
                url = url:gsub("[.,;)>'\"%]]+$", "")
                local safe = rt.redact(url, secrets_of(cfg))
                if safe ~= shown_url then
                    shown_url = safe
                end
            end
            local dcode = raw:lower():find("code", 1, true)
                and raw:match("(%f[%w][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9]+%-[A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9]+%f[%W])")
            if dcode then shown_code = dcode end
            if shown_url or shown_code then
                local lines = { ("### subs login (%s)"):format(provider), "" }
                if shown_url then
                    lines[#lines + 1] = "- **Open URL:** " .. shown_url
                end
                if shown_code then
                    lines[#lines + 1] = "- **Device code:** `" .. shown_code .. "`"
                end
                lines[#lines + 1] = "- Waiting for authentication to finish (Esc cancels)…"
                show_subs(cfg, table.concat(lines, "\n"))
            end
            if raw:lower():find("error", 1, true) or raw:lower():find("fail", 1, true) then
                last_err_line = raw
            end
        end
        if ev.exited then
            if ev.timed_out then
                fail(cfg, "login timed out after 5 minutes")
            elseif ev.code == 0 then
                rt.tighten_dir_files(dir .. "/auth", 384)
                rt.set_last_error(nil)
                show_subs(cfg, ("### subs login (%s)\n\nLogin complete. Credentials saved in `%s/auth` (mode 0600)."):format(provider, dir))
            else
                fail(cfg, ("login failed (exit %s)%s"):format(
                    tostring(ev.code),
                    last_err_line and (": " .. last_err_line) or ""
                ))
            end
            return
        end
    end
end

local function do_start(cfg)
    local bin = resolve_binary(cfg)
    local dir = dir_of(cfg)
    local cfg_path = dir .. "/config.yaml"
    if not bin or not rt.exists(cfg_path) then
        fail(cfg, "binary or `config.yaml` is missing; run `/subs` -> `setup` first")
        return
    end
    local ep = endpoint_of(cfg)
    local owner = rt.get_owner()
    if owner and task_state(owner.task_id) == "running" and probe(cfg).ok then
        show_subs(cfg, ("Proxy is already running (managed, task `%s`) at `%s`."):format(owner.task_id, ep))
        return
    end
    local existing = probe(cfg)
    if existing.ok then
        show_subs(cfg, ("An externally managed proxy is already listening at `%s`."):format(ep))
        return
    elseif existing.reachable then
        fail(cfg, ("port %d is already in use by another service (%s); change `opts.subs.port`"):format(cfg.port, existing.err or "conflict"))
        return
    end
    local owner_id = rt.random_hex(16)
    local target = { host = cfg.host, port = cfg.port, config_dir = dir, binary = bin }
    local task_id = t.spawn(function()
        if not rt.owns(owner_id) then
            return "subs-proxy stopped"
        end
        local proc = rt.spawn_proc({ bin, "--config", cfg_path }, {})
        if not rt.set_owner_proc(owner_id, proc) then
            proc:kill()
            return "subs-proxy stopped"
        end
        while true do
            local ev = rt.next_proc(proc)
            if ev.line then
                print(rt.redact(ev.line, secrets_of(cfg)))
            end
            if ev.exited then
                if ev.code == nil then
                    return "subs-proxy stopped"
                end
                return ("subs-proxy exited with code %d"):format(ev.code)
            end
        end
    end, "subs-proxy")
    rt.set_owner({ owner_id = owner_id, task_id = task_id, target = target })
    for _ = 1, 100 do
        t.sleep(0.1)
        local st = task_state(task_id)
        if st ~= "running" then
            rt.kill_owner()
            fail(cfg, ("proxy exited before becoming ready (task `%s` %s)"):format(task_id, tostring(st)))
            return
        end
        if probe(cfg).ok then
            rt.set_last_error(nil)
            show_subs(cfg, ("Proxy started (managed, task `%s`) at `%s`."):format(task_id, ep))
            return
        end
    end
    local tid = rt.kill_owner()
    if tid then pcall(t.task_stop, tid) end
    fail(cfg, ("proxy did not become ready at `%s` within 10s"):format(ep))
end

local function do_stop(cfg)
    local tid = rt.kill_owner()
    if tid then
        pcall(t.task_stop, tid)
        show_subs(cfg, ("Stopped managed proxy (task `%s`)."):format(tid))
        return
    end
    if probe(cfg).ok then
        show_subs(cfg, ("Proxy at `%s` is externally managed; not stopping it."):format(endpoint_of(cfg)))
        return
    end
    show_subs(cfg, "No managed proxy is running.")
end

local function do_connect(cfg)
    local key = read_key(cfg)
    if not key then
        fail(cfg, "local API key is missing; run `/subs` -> `setup` first")
        return
    end
    local ep = endpoint_of(cfg)
    if not probe(cfg).ok then
        fail(cfg, ("proxy is not reachable at `%s`; run `/subs` -> `start` first"):format(ep))
        return
    end
    local existing, perr = find_provider(cfg.provider)
    if perr then
        fail(cfg, perr)
        return
    end
    local argv = { rt.tuse_bin(), "connect", "--base", ep, "--key-stdin", "--name", cfg.provider }
    if existing then
        if existing.base_url ~= ep then
            local confirm = ("Replace existing provider '%s' (%s)"):format(cfg.provider, existing.base_url)
            if t.pick({ confirm, "Cancel" }) ~= confirm then
                return
            end
        end
        argv[#argv + 1] = "--replace"
    end
    local res = rt.exec(argv, { stdin = key, timeout = 30 })
    if not res.ok then
        fail(cfg, res.out ~= "" and res.out or "tuse connect failed")
        return
    end
    local ok, rerr = pcall(t.reload)
    if not ok then
        local clean = rt.redact(tostring(rerr), secrets_of(cfg))
        rt.set_last_error(clean)
        show_subs(cfg, ("Saved provider `%s` in `~/.tuse/tuse.db`, but config reload failed: %s. Fix the config/plugin issue and run `/reload` to activate it in this session."):format(cfg.provider, clean))
        return
    end
    rt.set_last_error(nil)
    show_subs(cfg, ("Connected provider `%s` (`%s`) and reloaded session."):format(cfg.provider, ep))
end

local function do_disconnect(cfg)
    local ep = endpoint_of(cfg)
    local existing, perr = find_provider(cfg.provider)
    if perr then
        fail(cfg, perr)
        return
    end
    if not existing then
        show_subs(cfg, ("Provider `%s` is not connected."):format(cfg.provider))
        return
    end
    if existing.base_url ~= ep then
        local confirm = ("Disconnect '%s' (%s)"):format(cfg.provider, existing.base_url)
        if t.pick({ confirm, "Cancel" }) ~= confirm then
            return
        end
    end
    local res = rt.exec({ rt.tuse_bin(), "disconnect", cfg.provider }, { timeout = 15 })
    if not res.ok then
        fail(cfg, res.out ~= "" and res.out or "tuse disconnect failed")
        return
    end
    local ok, rerr = pcall(t.reload)
    if not ok then
        local clean = rt.redact(tostring(rerr), secrets_of(cfg))
        rt.set_last_error(clean)
        show_subs(cfg, ("Removed provider `%s` from `~/.tuse/tuse.db`, but config reload failed: %s. Fix the config/plugin issue and run `/reload` to activate it in this session."):format(cfg.provider, clean))
        return
    end
    rt.set_last_error(nil)
    show_subs(cfg, ("Disconnected provider `%s` and reloaded session."):format(cfg.provider))
end

local ACTIONS = {
    status = do_status,
    setup = do_setup,
    login = do_login,
    start = do_start,
    stop = do_stop,
    connect = do_connect,
    disconnect = do_disconnect,
}

return {
    command = function(cfg)
        rt.set_next_target({
            host = cfg.host,
            port = cfg.port,
            config_dir = dir_of(cfg),
            binary = resolve_binary(cfg) or "",
        })
        return function()
            local action = t.pick({ "status", "setup", "login", "start", "stop", "connect", "disconnect" })
            local fn = action and ACTIONS[action]
            if fn then fn(cfg) end
        end
    end,
}
