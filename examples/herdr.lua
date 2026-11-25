-- herdr: reports the agent state on the pane and renames the pane after the session.
-- Use: require("herdr").apply(g)
local t = require("tuse")

local M = {}

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

function M.apply(g)
  g.hooks = g.hooks or {}
  g.hooks.herdr = {
    events = { "user_message", "agent_message", "name_change" },
    run = function(e)
      if e.event == "user_message" then
        run("[ -n \"$HERDR_PANE_ID\" ] && herdr pane report-agent --source tuse --agent tuse --state working \"$HERDR_PANE_ID\"", { S = "working" })
      elseif e.event == "agent_message" and e.status ~= "working" then
        run("[ -n \"$HERDR_PANE_ID\" ] && herdr pane report-agent --source tuse --agent tuse --state idle --message \"$S\" \"$HERDR_PANE_ID\"", { S = e.status })
      elseif e.event == "name_change" then
        run("[ -n \"$HERDR_PANE_ID\" ] && herdr pane rename \"$HERDR_PANE_ID\" \"$N\"", { N = e.new })
      end
    end,
  }
  return g
end

return M
