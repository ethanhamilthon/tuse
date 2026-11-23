-- Merges plugin tables (in order) and then the user's config on top.
local list, user = ...
local maps = { prompts = true, functions = true, hooks = true, commands = true }
local deep = { settings = true, theme = true }
local lists = { systems = true, models = true }
local denied = { model = true, provider = true, effort = true, plugins = true }

local function merge(dst, src)
    for k, v in pairs(src) do
        if type(v) == "table" and type(dst[k]) == "table" then
            merge(dst[k], v)
        else
            dst[k] = type(v) == "table" and merge({}, v) or v
        end
    end
    return dst
end

local out, owner = {}, {}
local function put(k, v, from)
    if type(v) ~= "table" or (from and denied[k]) then
        if from then
            error(("plugin %s: `%s` %s"):format(from, k,
                denied[k] and "is not allowed in a plugin" or "must be a table"), 0)
        end
        out[k] = v
        return
    end
    if lists[k] then
        out[k] = out[k] or {}
        for _, x in ipairs(v) do out[k][#out[k] + 1] = x end
    elseif maps[k] then
        out[k] = out[k] or {}
        for n, x in pairs(v) do
            local id = k .. "." .. tostring(n)
            if from and owner[id] then
                error(("plugins %s and %s both define %s"):format(owner[id], from, id), 0)
            end
            owner[id] = from
            out[k][n] = x
        end
    elseif deep[k] then
        out[k] = merge(type(out[k]) == "table" and out[k] or {}, v)
    elseif from then
        error(("plugin %s: unknown key `%s`"):format(from, k), 0)
    else
        out[k] = v
    end
end

for _, p in ipairs(list) do
    for k, v in pairs(p.config) do put(k, v, p.name) end
end
for k, v in pairs(user) do
    if k ~= "plugins" then put(k, v, nil) end
end
return out
