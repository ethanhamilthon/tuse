-- Puts a timeout on the agent's sh, refuses some commands, and removes delete.
-- Use: return { sandbox = require("sandbox") }
local t = require("tuse")

local deny = { "rm %-rf /", "git push %-%-force", "sudo " }

return {
  functions = {
    sh = function(cmd, opts)
      for _, p in ipairs(deny) do
        if cmd:find(p) then
          error("sandbox: refused `" .. cmd .. "`")
        end
      end
      opts = opts or {}
      opts.timeout = opts.timeout or 60
      return t.sh(cmd, opts)
    end,
    delete = false,
  },
}
