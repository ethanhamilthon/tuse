-- Shows the agent state and session title in the zellij pane name.
-- Use: require("zellij").apply(g)
local t = require("tuse")

local M = {}

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

function M.apply(g)
  g.hooks = g.hooks or {}
  g.hooks.zellij = {
    events = { "user_message", "agent_message", "name_change" },
    run = function(e)
      if e.event == "user_message" then
        run("[ -n \"$ZELLIJ\" ] && zellij action rename-pane \"tuse: $S\"", { S = "working" })
      elseif e.event == "agent_message" and e.status ~= "working" then
        run("[ -n \"$ZELLIJ\" ] && zellij action rename-pane \"tuse: $S\"", { S = e.status })
      elseif e.event == "name_change" then
        run("[ -n \"$ZELLIJ\" ] && zellij action rename-pane \"$N\"", { N = e.new })
      end
    end,
  }
  return g
end

return M
