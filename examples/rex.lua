-- Rex: renames the current block with the session title and agent state.
-- Use: return { rex = require("rex") }
local t = require("tuse")

local function run(cmd, env)
  t.sh(cmd .. " >/dev/null 2>&1", { env = env })
end

return {
  hooks = {
    rex = {
      events = { "user_message", "agent_message" },
      run = function(e)
        local title = t.session().title or "Tuse"
        if e.event == "user_message" then
          run("[ -n \"$REX_BLOCK\" ] && rex block rename \"$REX_BLOCK\" \"$N: $S\"", { N = title, S = "working" })
        elseif e.event == "agent_message" and e.status ~= "working" then
          run("[ -n \"$REX_BLOCK\" ] && rex block rename \"$REX_BLOCK\" \"$N: $S\"", { N = title, S = e.status })
        end
      end,
    },
  },
}
