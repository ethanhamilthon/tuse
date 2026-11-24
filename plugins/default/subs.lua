local rt, U, store = ...
local t = require("tuse")

local function do_status(cfg)
    local dir, bin, pr, ep = U.dir_of(cfg), U.resolve_bin(cfg), U.probe(cfg), U.endpoint(cfg)
    local bin_line = "not installed"
    if bin then
        local ver = U.detect_version(bin)
        bin_line = ("`%s` (%s, %s)"):format(bin, ver, ver == U.PINNED_VERSION and "matches pinned" or ("pinned: " .. U.PINNED_VERSION))
    end
    local cfg_path, files = dir .. "/config.yaml", U.list_files(dir .. "/auth")
    local auth_line = #files == 0 and "0 accounts" or ("%d file(s): %s"):format(#files, table.concat(files, ", "))
    local tid, proxy_line = (rt.state("owner")), "stopped"
    if tid then
        local st = U.task_state(tid)
        if st == "running" or st == "failed" then
            proxy_line = ("%s (%stask `%s`)"):format(st, st == "running" and "managed, " or "", tid)
        end
    elseif pr.ok then
        proxy_line = "running (external)"
    end
    local ep_line = pr.ok and ("`%s` (reachable, %d models)"):format(ep, pr.models) or ("`%s` (%s)"):format(ep, pr.err or "unreachable")
    local prov, perr = U.find_provider(cfg.provider)
    local prov_line = perr and ("error: " .. perr)
        or (prov and ("connected as `%s` (%s)"):format(cfg.provider, prov.base_url == ep and "matches endpoint" or ("points to `" .. prov.base_url .. "`")) or "not connected")
    local lines = {
        "### subs status", "",
        "- **Binary:** " .. bin_line,
        ("- **Config:** `%s` (%s) · auth: %s"):format(cfg_path, U.is_file(cfg_path) and "present" or "missing", auth_line),
        "- **Proxy:** " .. proxy_line,
        "- **Endpoint:** " .. ep_line,
        "- **Tuse provider:** " .. prov_line,
    }
    if store.last_error then lines[#lines + 1] = "- **Last error:** " .. store.last_error end
    U.show(cfg, table.concat(lines, "\n"))
end

local function install_binary(cfg)
    local label = "Install CLIProxyAPI " .. U.PINNED_VERSION
    if t.pick({ label, "Cancel" }) ~= label then return false, nil end
    local entry = U.MANIFEST[rt.platform]
    if not entry then return false, ("unsupported platform `%s`; install CLIProxyAPI manually and set `opts.subs.binary`"):format(rt.platform) end
    local dir = U.dir_of(cfg)
    local stage = dir .. "/.stage-" .. U.random_hex(8)
    U.mkdir(dir, "700"); U.mkdir(stage, "700")
    local base = os.getenv("TUSE_SUBS_RELEASE_URL")
    base = (base and base ~= "" and base or U.DEFAULT_RELEASE_URL):gsub("/+$", "")
    local archive = stage .. "/" .. entry.asset
    local dl = rt.http(base .. "/" .. entry.asset, archive, 120)
    if not dl.ok then U.rm(stage); return false, ("download failed: %s; or set `opts.subs.binary`"):format(dl.err or "network error") end
    local got = U.sha256(archive)
    if got ~= entry.sha256 then
        U.rm(stage)
        return false, ("SHA-256 mismatch for %s (expected %s, got %s); refusing to install. Set `opts.subs.binary` for a custom build."):format(entry.asset, entry.sha256, got)
    end
    local ex = U.exec({ "tar", "-xzf", archive, "-C", stage, "cli-proxy-api" }, { timeout = 30 })
    local extracted = stage .. "/cli-proxy-api"
    if not ex.ok or not U.is_file(extracted) then U.rm(stage); return false, "failed to extract `cli-proxy-api` from release archive" end
    U.chmod(extracted, "755"); U.mkdir(dir .. "/bin", "700")
    assert(os.rename(extracted, dir .. "/bin/cli-proxy-api"))
    U.rm(stage)
    return true, nil
end

local function do_setup(cfg)
    local bin = U.resolve_bin(cfg)
    if not bin then
        local ok, err = install_binary(cfg)
        if err then U.fail(cfg, err); return end
        if not ok then return end
        bin = U.resolve_bin(cfg)
    end
    local dir, cfg_path = U.dir_of(cfg), U.dir_of(cfg) .. "/config.yaml"
    if not U.is_file(cfg_path) then
        local label = "Create local config in " .. dir
        if t.pick({ label, "Cancel" }) ~= label then return end
        U.write_config(cfg)
    else
        local c = t.pick({ "Keep existing config", "Reset config (rotates local API key)", "Cancel" })
        if c == "Reset config (rotates local API key)" then U.write_config(cfg)
        elseif c ~= "Keep existing config" then return end
    end
    store.last_error = nil
    U.show(cfg, ("### subs setup complete\n\n- **Binary:** `%s`\n- **Config:** `%s`\n- **Next steps:** run `/subs` -> `login`, then `start`, then `connect`."):format(bin or "installed", cfg_path))
end

local LOGIN_FLAGS = { ["OpenAI / Codex"] = "--codex-login", ["Anthropic / Claude"] = "--claude-login", ["Google / Gemini"] = "--antigravity-login" }

local function do_login(cfg)
    local bin, dir = U.resolve_bin(cfg), U.dir_of(cfg)
    local cfg_path = dir .. "/config.yaml"
    if not bin or not U.is_file(cfg_path) then U.fail(cfg, "binary or `config.yaml` is missing; run `/subs` -> `setup` first"); return end
    local provider = t.pick({ "OpenAI / Codex", "Anthropic / Claude", "Google / Gemini", "Cancel" })
    local flag = provider and LOGIN_FLAGS[provider]
    if not flag then return end
    U.show(cfg, ("### subs login (%s)\n\nStarting OAuth login…"):format(provider))
    local proc = rt.spawn({ bin, "--config", cfg_path, "--no-browser", flag }, { timeout = 300 })
    local shown_url, shown_code, last_err_line
    while true do
        local ev = rt.next(proc)
        if ev.line then
            local raw = ev.line
            local url = raw:match("(https://%S+)")
            if url then shown_url = U.redact(url:gsub("[.,;)>'\"%]]+$", ""), U.secrets(cfg)) end
            local dcode = raw:lower():find("code", 1, true)
                and raw:match("(%f[%w][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9]+%-[A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9]+%f[%W])")
            if dcode then shown_code = dcode end
            if shown_url or shown_code then
                local lines = { ("### subs login (%s)"):format(provider), "" }
                if shown_url then lines[#lines + 1] = "- **Open URL:** " .. shown_url end
                if shown_code then lines[#lines + 1] = "- **Device code:** `" .. shown_code .. "`" end
                lines[#lines + 1] = "- Waiting for authentication to finish (Esc cancels)…"
                U.show(cfg, table.concat(lines, "\n"))
            end
            if raw:lower():find("error", 1, true) or raw:lower():find("fail", 1, true) then last_err_line = raw end
        end
        if ev.exited then
            if ev.timed_out then U.fail(cfg, "login timed out after 5 minutes")
            elseif ev.code == 0 then
                U.tighten_dir(dir .. "/auth"); store.last_error = nil
                U.show(cfg, ("### subs login (%s)\n\nLogin complete. Credentials saved in `%s/auth` (mode 0600)."):format(provider, dir))
            else
                U.fail(cfg, ("login failed (exit %s)%s"):format(tostring(ev.code), last_err_line and (": " .. last_err_line) or ""))
            end
            return
        end
    end
end

local function do_start(cfg)
    local bin, dir, ep = U.resolve_bin(cfg), U.dir_of(cfg), U.endpoint(cfg)
    local cfg_path = dir .. "/config.yaml"
    if not bin or not U.is_file(cfg_path) then U.fail(cfg, "binary or `config.yaml` is missing; run `/subs` -> `setup` first"); return end
    local owner_tid, existing = (rt.state("owner")), U.probe(cfg)
    if owner_tid and U.task_state(owner_tid) == "running" and existing.ok then
        U.show(cfg, ("Proxy is already running (managed, task `%s`) at `%s`."):format(owner_tid, ep)); return
    end
    if existing.ok then U.show(cfg, ("An externally managed proxy is already listening at `%s`."):format(ep)); return
    elseif existing.reachable then U.fail(cfg, ("port %d is already in use by another service (%s); change `opts.subs.port`"):format(cfg.port, existing.err or "conflict")); return end
    local owner_id = U.random_hex(16)
    local task_id = t.spawn(function()
        local _, cur = rt.state("owner")
        if cur ~= owner_id then return "subs-proxy stopped" end
        local proc = rt.spawn({ bin, "--config", cfg_path }, {})
        if not rt.state("bind_proc", owner_id, proc) then proc:kill(); return "subs-proxy stopped" end
        while true do
            local ev = rt.next(proc)
            if ev.line then print(U.redact(ev.line, U.secrets(cfg))) end
            if ev.exited then return ev.code == nil and "subs-proxy stopped" or ("subs-proxy exited with code %d"):format(ev.code) end
        end
    end, "subs-proxy")
    rt.state("set_owner", owner_id, task_id, U.target_key(cfg, bin))
    for _ = 1, 100 do
        t.sleep(0.1)
        local st = U.task_state(task_id)
        if st ~= "running" then
            rt.state("kill_owner")
            U.fail(cfg, ("proxy exited before becoming ready (task `%s` %s)"):format(task_id, tostring(st))); return
        end
        if U.probe(cfg).ok then
            store.last_error = nil
            U.show(cfg, ("Proxy started (managed, task `%s`) at `%s`."):format(task_id, ep)); return
        end
    end
    local tid = (rt.state("kill_owner"))
    if tid then pcall(t.task_stop, tid) end
    U.fail(cfg, ("proxy did not become ready at `%s` within 10s"):format(ep))
end

local function do_stop(cfg)
    local tid = (rt.state("kill_owner"))
    if tid then pcall(t.task_stop, tid); U.show(cfg, ("Stopped managed proxy (task `%s`)."):format(tid))
    elseif U.probe(cfg).ok then U.show(cfg, ("Proxy at `%s` is externally managed; not stopping it."):format(U.endpoint(cfg)))
    else U.show(cfg, "No managed proxy is running.") end
end

local function do_connect(cfg)
    local key, ep = U.read_key(cfg), U.endpoint(cfg)
    if not key then U.fail(cfg, "local API key is missing; run `/subs` -> `setup` first"); return end
    if not U.probe(cfg).ok then U.fail(cfg, ("proxy is not reachable at `%s`; run `/subs` -> `start` first"):format(ep)); return end
    local existing, perr = U.find_provider(cfg.provider)
    if perr then U.fail(cfg, perr); return end
    local argv = { rt.tuse_bin, "connect", "--base", ep, "--key-stdin", "--name", cfg.provider }
    if existing then
        if existing.base_url ~= ep then
            local c = ("Replace existing provider '%s' (%s)"):format(cfg.provider, existing.base_url)
            if t.pick({ c, "Cancel" }) ~= c then return end
        end
        argv[#argv + 1] = "--replace"
    end
    local res = U.exec(argv, { stdin = key, timeout = 30 })
    if not res.ok then U.fail(cfg, res.out ~= "" and res.out or "tuse connect failed"); return end
    U.reload_after(cfg, ("Saved provider `%s` in `~/.tuse/tuse.db`"):format(cfg.provider),
        ("Connected provider `%s` (`%s`) and reloaded session."):format(cfg.provider, ep))
end

local function do_disconnect(cfg)
    local existing, perr = U.find_provider(cfg.provider)
    if perr then U.fail(cfg, perr); return end
    if not existing then U.show(cfg, ("Provider `%s` is not connected."):format(cfg.provider)); return end
    if existing.base_url ~= U.endpoint(cfg) then
        local c = ("Disconnect '%s' (%s)"):format(cfg.provider, existing.base_url)
        if t.pick({ c, "Cancel" }) ~= c then return end
    end
    local res = U.exec({ rt.tuse_bin, "disconnect", cfg.provider }, { timeout = 15 })
    if not res.ok then U.fail(cfg, res.out ~= "" and res.out or "tuse disconnect failed"); return end
    U.reload_after(cfg, ("Removed provider `%s` from `~/.tuse/tuse.db`"):format(cfg.provider),
        ("Disconnected provider `%s` and reloaded session."):format(cfg.provider))
end

local ACTIONS = { status = do_status, setup = do_setup, login = do_login, start = do_start, stop = do_stop, connect = do_connect, disconnect = do_disconnect }

return {
    command = function(cfg)
        rt.state("set_next", U.target_key(cfg, U.resolve_bin(cfg)))
        return function()
            local action = t.pick({ "status", "setup", "login", "start", "stop", "connect", "disconnect" })
            if action and ACTIONS[action] then ACTIONS[action](cfg) end
        end
    end,
}
