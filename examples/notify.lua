-- Desktop notification when the agent finishes a turn (macOS and Linux).
-- Use: require("notify").apply(g)
local t = require("tuse")

local M = {}

function M.send(title, body)
  if t.sh("command -v osascript").ok then
    t.sh("osascript -e \"display notification (system attribute \\\"B\\\") with title (system attribute \\\"T\\\")\"",
      { env = { T = title, B = body } })
  else
    t.sh("notify-send \"$T\" \"$B\"", { env = { T = title, B = body } })
  end
end

function M.apply(g)
  g.hooks = g.hooks or {}
  g.hooks.notify = {
    events = { "agent_message" },
    run = function(e)
      if e.status == "finished" then
        M.send("Tuse", (t.session().name ~= "" and t.session().name) or "done")
      end
    end,
  }
  return g
end

return M
