-- cmux: tab title follows the session, and a notification arrives when the agent is done.
-- Use: require("cmux").apply(g)
local t = require("tuse")

local M = {}

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

function M.apply(g)
  g.hooks = g.hooks or {}
  g.hooks.cmux = {
    events = { "user_message", "agent_message" },
    run = function(e)
      run("[ -n \"$CMUX_SURFACE_ID\" ] && cmux rename-tab \"$N\"", { N = t.session().name or "Tuse" })
      if e.event == "user_message" then
        run("[ -n \"$CMUX_SURFACE_ID\" ] && cmux workspace status set working", { S = "working" })
      elseif e.event == "agent_message" and e.status ~= "working" then
        run("[ -n \"$CMUX_SURFACE_ID\" ] && cmux notify --title Tuse --body \"$S\" && cmux workspace status set auto", { S = e.status })
      end
    end,
  }
  return g
end

return M
