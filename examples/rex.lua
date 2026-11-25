-- Rex: renames the current block after the session and shows the agent state in its label.
-- Use: require("rex").apply(g)
local t = require("tuse")

local M = {}

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

function M.apply(g)
  g.hooks = g.hooks or {}
  g.hooks.rex = {
    events = { "user_message", "agent_message", "name_change" },
    run = function(e)
      if e.event == "user_message" then
        run("[ -n \"$REX_BLOCK\" ] && rex block rename \"$REX_BLOCK\" \"tuse: $S\"", { S = "working" })
      elseif e.event == "agent_message" and e.status ~= "working" then
        run("[ -n \"$REX_BLOCK\" ] && rex block rename \"$REX_BLOCK\" \"tuse: $S\"", { S = e.status })
      elseif e.event == "name_change" then
        run("[ -n \"$REX_BLOCK\" ] && rex block rename \"$REX_BLOCK\" \"$N\"", { N = e.new })
      end
    end,
  }
  return g
end

return M
