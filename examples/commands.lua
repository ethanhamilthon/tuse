-- A few custom slash commands: /diff, /test, /commit "message".
-- Use: return { commands = require("commands") }
local t = require("tuse")

return {
  commands = {
    diff = {
      desc = "show the git diff",
      run = function()
        t.show("diff", "```diff\n" .. t.sh("git diff --stat; git diff | head -200").out .. "\n```", false)
      end,
    },
    test = {
      desc = "run cargo test",
      run = function()
        t.show("test", "running…", true)
        local r = t.sh("cargo test --quiet 2>&1 | tail -20")
        t.show("test", (r.ok and "**passed**" or "**failed**") .. "\n\n```text\n" .. r.out .. "\n```", true)
      end,
    },
    commit = {
      desc = "commit all changes; /commit \"message\" skips the question",
      run = function(msg)
        msg = msg or t.ask("Commit message:")
        if msg and msg ~= "" then
          local r = t.sh("git commit -am \"$M\"", { env = { M = msg } })
          t.show("commit", "```text\n" .. r.out .. "\n```", false)
        end
      end,
    },
  },
}
