-- Skills are reusable prompts. This loads every skills/<name>/SKILL.md next to this file as #<name>.
-- Use: local skills = require("skills"); g.prompts = skills.load(g.prompts)
local t = require("tuse")

local M = {}

function M.load(prompts, dir)
  prompts = prompts or {}
  dir = dir or ((t.session().configpath or ""):match("^(.*)/") or t.session().cwd) .. "/skills"
  local list = t.sh("ls " .. string.format("%q", dir) .. " 2>/dev/null")
  for name in list.out:gmatch("[^\n]+") do
    local ok, body = pcall(t.read, dir .. "/" .. name .. "/SKILL.md")
    if ok then
      prompts[name] = body
    end
  end
  return prompts
end

return M
