local theme_palette, subs = ...

local LOOPBACK = {
    ["127.0.0.1"] = true,
    ["::1"] = true,
    ["localhost"] = true,
}

local SUBS_KEYS = {
    provider = true,
    host = true,
    port = true,
    binary = true,
    config_dir = true,
}

local function parse_subs(v)
    if v == nil or v == false then
        return nil
    end
    local cfg = {
        provider = "subs",
        host = "127.0.0.1",
        port = 8317,
        binary = nil,
        config_dir = "~/.tuse/subs",
    }
    if v == true then
        return cfg
    end
    if type(v) ~= "table" then
        error("opts.subs: expected boolean or table, got " .. type(v), 0)
    end
    for k, val in pairs(v) do
        if type(k) ~= "string" or not SUBS_KEYS[k] then
            error("opts.subs." .. tostring(k) .. ": unknown key (allowed: provider, host, port, binary, config_dir)", 0)
        end
        if k == "provider" then
            if type(val) ~= "string" or val == "" or val:find("[/%s]") then
                error("opts.subs.provider: must be a non-empty string without `/` or whitespace", 0)
            end
            cfg.provider = val
        elseif k == "host" then
            if type(val) ~= "string" or not LOOPBACK[val] then
                error("opts.subs.host: must be loopback (`127.0.0.1`, `::1`, or `localhost`)", 0)
            end
            cfg.host = val
        elseif k == "port" then
            if type(val) ~= "number" or math.type(val) ~= "integer" or val < 1 or val > 65535 then
                error("opts.subs.port: must be an integer in 1..65535", 0)
            end
            cfg.port = val
        elseif k == "binary" then
            if type(val) ~= "string" or val:match("^%s*$") then
                error("opts.subs.binary: expected non-empty string", 0)
            end
            cfg.binary = val
        elseif k == "config_dir" then
            if type(val) ~= "string" or val:match("^%s*$") then
                error("opts.subs.config_dir: expected non-empty string", 0)
            end
            cfg.config_dir = val
        end
    end
    return cfg
end

return function(opts)
    if opts == nil then opts = {} end
    if type(opts) ~= "table" then
        error("opts: expected table, got " .. type(opts), 0)
    end
    for k in pairs(opts) do
        if k ~= "theme" and k ~= "subs" then
            error("opts." .. tostring(k) .. ": unknown key (allowed: theme, subs)", 0)
        end
    end
    local use_theme = true
    if opts.theme ~= nil then
        if type(opts.theme) ~= "boolean" then
            error("opts.theme: expected boolean, got " .. type(opts.theme), 0)
        end
        use_theme = opts.theme
    end
    local subs_cfg = parse_subs(opts.subs)
    local out = {}
    if use_theme then
        out.theme = theme_palette
    end
    if subs_cfg then
        out.commands = { subs = subs.command(subs_cfg) }
    end
    return out
end
