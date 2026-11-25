-- Puts a timeout on the agent's sh, refuses some commands, and removes delete.
-- Use: require("sandbox").apply(g)
local t = require("tuse")

local M = {}

M.deny = { "rm %-rf /", "git push %-%-force", "sudo " }

function M.apply(g)
  g.functions = g.functions or {}
  g.functions.sh = function(cmd, opts)
    for _, p in ipairs(M.deny) do
      if cmd:find(p) then
        error("sandbox: refused `" .. cmd .. "`")
      end
    end
    opts = opts or {}
    opts.timeout = opts.timeout or 60
    return t.sh(cmd, opts)
  end
  g.functions.delete = false
  return g
end

return M
