-- Skills are reusable prompts. This loads every skills/<name>/SKILL.md as #<name>.
-- Use: return { skills = require("skills")() }
local t = require("tuse")

return function(dir)
  local prompts = {}
  dir = dir or ((t.session().config_path or ""):match("^(.*)/") or t.session().cwd) .. "/skills"
  local list = t.sh("ls " .. string.format("%q", dir) .. " 2>/dev/null")
  for name in list.out:gmatch("[^\n]+") do
    local ok, body = pcall(t.read, dir .. "/" .. name .. "/SKILL.md")
    if ok then
      prompts[name] = body
    end
  end
  return { prompts = prompts }
end
