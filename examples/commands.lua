-- A few custom slash commands: /diff, /test, /commit.
-- Use: require("commands").apply(g)
local t = require("tuse")

local M = {}

function M.apply(g)
  g.commands = g.commands or {}
  g.commands.diff = function()
    t.show("diff", "```diff\n" .. t.sh("git diff --stat; git diff | head -200").out .. "\n```", false)
  end
  g.commands.test = function()
    t.show("test", "running…", true)
    local r = t.sh("cargo test --quiet 2>&1 | tail -20")
    t.show("test", (r.ok and "**passed**" or "**failed**") .. "\n\n```text\n" .. r.out .. "\n```", true)
  end
  g.commands.commit = function()
    local msg = t.ask("Commit message:")
    if msg and msg ~= "" then
      local r = t.sh("git commit -am \"$M\"", { env = { M = msg } })
      t.show("commit", "```text\n" .. r.out .. "\n```", false)
    end
  end
  return g
end

return M
