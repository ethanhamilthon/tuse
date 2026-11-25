-- Shows the agent state and session title in the tmux window name.
-- Use: require("tmux").apply(g)
local t = require("tuse")

local M = {}

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

function M.apply(g)
  g.hooks = g.hooks or {}
  g.hooks.tmux = {
    events = { "user_message", "agent_message" },
    run = function(e)
      run("[ -n \"$TMUX\" ] && tmux rename-window \"$N\"", { N = t.session().name or "Tuse" })
      if e.event == "user_message" then
        run("[ -n \"$TMUX\" ] && tmux rename-window \"tuse: $S\"", { S = "working" })
      elseif e.event == "agent_message" and e.status ~= "working" then
        run("[ -n \"$TMUX\" ] && tmux rename-window \"tuse: $S\"", { S = e.status })
      end
    end,
  }
  return g
end

return M
