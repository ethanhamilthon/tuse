local t = require("tuse")
local banner = require("./banner")

local function tuse_version()
    local r = t.sh("tuse --version", { timeout = 2 })
    return r.ok and (r.out:match("(%d[%w%.%-]*)") or r.out:gsub("%s+$", "")) or "?"
end

local function project_name(cwd)
    local name = (cwd or ""):gsub("/+$", ""):match("([^/]+)$") or "tuse"
    return (name:gsub("^%l", string.upper))
end

local function inline_code(text)
    text = (text or "(none)"):gsub("%c", " ")
    local fence = "`"
    while text:find(fence, 1, true) do fence = fence .. "`" end
    return fence .. " " .. text .. " " .. fence
end

local function git_status(cwd)
    local result = t.sh("git --no-optional-locks status --porcelain=v2 --branch --untracked-files=normal", {
        cwd = cwd,
        env = { LC_ALL = "C" },
        timeout = 2,
    })
    if not result.ok then
        return result.out:find("not a git repository", 1, true) and "not a repository" or "unavailable"
    end

    local head, oid, ahead, behind
    local changed = 0
    for _, line in ipairs(result:lines()) do
        local key, value = line:match("^# branch%.(%S+) (.+)$")
        if key == "head" then head = value end
        if key == "oid" then oid = value end
        if key == "ab" then ahead, behind = value:match("^%+(%d+) %-(%d+)$") end
        if line:match("^[12u?] ") then changed = changed + 1 end
    end
    if head == "(detached)" then head = "detached @ " .. (oid or "?"):sub(1, 8) end
    local state = changed == 0 and "clean" or ("changes: %d"):format(changed)
    if tonumber(ahead) and tonumber(ahead) > 0 then state = state .. " · ↑" .. ahead end
    if tonumber(behind) and tonumber(behind) > 0 then state = state .. " · ↓" .. behind end
    return inline_code(head) .. " · " .. state
end

local function model_label(s)
    local name = s.model_info and s.model_info.display or s.model
    if not name then return "(none)" end
    if s.effort and s.effort ~= "default" then
        return name .. " (" .. s.effort .. ")"
    end
    return name
end

local function hook(opts)
    return {
        events = { "start" },
        run = function(e)
            if not e.is_new then return end
            local s = t.session()
            local weekdays = { "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat" }
            local now = os.time()
            local started = weekdays[os.date("*t", now).wday] .. os.date(" %d.%m, %H:%M", now)
            t.show(opts.block or "welcome", ([[
```
%s
```

- **Model:** %s
- **CWD:** %s
- **Git:** %s
- **Date & time:** %s
- **Config:** %s
- **Version:** %s
]]):format(
                table.concat(banner.render(opts.title or project_name(s.cwd)), "\n"),
                inline_code(model_label(s)),
                inline_code(s.cwd),
                git_status(s.cwd),
                inline_code(started),
                inline_code(s.configpath),
                inline_code(tuse_version())
            ), false)
        end,
    }
end

-- opts.title: banner text (default: the project folder); opts.block: show name.
return function(opts)
    if t.session().mode ~= "tui" then return {} end
    return { hooks = { welcome = hook(opts) } }
end
