-- Shows the session title and agent state in the zellij pane name.
-- Use: return { zellij = require("zellij") }
local t = require("tuse")

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

return {
  hooks = {
    zellij = {
      events = { "user_message", "agent_message" },
      run = function(e)
        local title = t.session().title or "Tuse"
        if e.event == "user_message" then
          run("[ -n \"$ZELLIJ\" ] && zellij action rename-pane \"$N: $S\"", { N = title, S = "working" })
        elseif e.event == "agent_message" and e.status ~= "working" then
          run("[ -n \"$ZELLIJ\" ] && zellij action rename-pane \"$N: $S\"", { N = title, S = e.status })
        end
      end,
    },
  },
}
