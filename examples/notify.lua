-- Desktop notification when the agent finishes a turn (macOS and Linux).
-- Use: return { notify = require("notify") }
local t = require("tuse")

local function send(title, body)
  if t.sh("command -v osascript").ok then
    t.sh("osascript -e \"display notification (system attribute \\\"B\\\") with title (system attribute \\\"T\\\")\"",
      { env = { T = title, B = body } })
  else
    t.sh("notify-send \"$T\" \"$B\"", { env = { T = title, B = body } })
  end
end

return {
  hooks = {
    notify = {
      events = { "agent_message" },
      run = function(e)
        if e.status == "finished" then
          send("Tuse", (t.session().title ~= "" and t.session().title) or "done")
        end
      end,
    },
  },
}
