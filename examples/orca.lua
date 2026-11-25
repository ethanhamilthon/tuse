-- Orca: posts progress as a worktree comment and marks the workspace unread when the agent is done.
-- Use: require("orca").apply(g)
local t = require("tuse")

local M = {}

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

function M.apply(g)
  g.hooks = g.hooks or {}
  g.hooks.orca = {
    events = { "user_message", "agent_message" },
    run = function(e)
      run("command -v orca && orca worktree set --worktree active --comment \"$N\" --json", { N = t.session().name or "Tuse" })
      if e.event == "user_message" then
        run("command -v orca && orca worktree set --worktree active --comment \"tuse: $S\" --json", { S = "working" })
      elseif e.event == "agent_message" and e.status ~= "working" then
        run("command -v orca && orca worktree set --worktree active --comment \"tuse: $S\" --unread --json", { S = e.status })
      end
    end,
  }
  return g
end

return M
