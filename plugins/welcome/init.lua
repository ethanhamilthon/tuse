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

local function saved_session(id)
    -- `launch` also covers --resume. Tuse opens the lock for saved sessions;
    -- fresh sessions don't get one until the first message.
    local home = os.getenv("TUSE_HOME") or (os.getenv("HOME") .. "/.tuse")
    local lock = io.open(home .. "/" .. id .. "/lock", "r")
    if not lock then return false end
    lock:close()
    return true
end

local function inline_code(text)
    text = (text or "?"):gsub("%c", " ")
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

local function hook(opts)
    return {
        events = { "process_start" },
        run = function(e)
            if e.reason ~= "new" and (e.reason ~= "launch" or saved_session(e.session)) then
                return
            end
            local weekdays = { "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat" }
            local now = os.time()
            local started = weekdays[os.date("*t", now).wday] .. os.date(" %d.%m, %H:%M", now)
            t.show_markdown(opts.block or "welcome", ([[
```
%s
```

- **Model:** %s
- **CWD:** %s
- **Git:** %s
- **Date & time:** %s
- **Config:** %s
- **Version:** %s
]]):format(table.concat(banner.render(opts.title or project_name(e.cwd)), "\n"), inline_code(e.model), inline_code(e.cwd), git_status(e.cwd), inline_code(started), inline_code(e.config), inline_code(tuse_version())), false)
        end,
    }
end

-- opts.title: banner text (default: the project folder); opts.block: show_markdown name.
return function(opts)
    if os.getenv("TUSE_MODE") ~= "tui" then return {} end
    return { hooks = { welcome = hook(opts) } }
end
