-- herdr: reports the agent state on the pane and renames the pane after the session.
-- Use: return { herdr = require("herdr") }
local t = require("tuse")

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

return {
  hooks = {
    herdr = {
      events = { "user_message", "agent_message" },
      run = function(e)
        run("[ -n \"$HERDR_PANE_ID\" ] && herdr pane rename \"$HERDR_PANE_ID\" \"$N\"", { N = t.session().title or "Tuse" })
        if e.event == "user_message" then
          run("[ -n \"$HERDR_PANE_ID\" ] && herdr pane report-agent --source tuse --agent tuse --state working \"$HERDR_PANE_ID\"", { S = "working" })
        elseif e.event == "agent_message" and e.status ~= "working" then
          run("[ -n \"$HERDR_PANE_ID\" ] && herdr pane report-agent --source tuse --agent tuse --state idle --message \"$S\" \"$HERDR_PANE_ID\"", { S = e.status })
        end
      end,
    },
  },
}
