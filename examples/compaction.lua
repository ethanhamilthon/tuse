-- Summarizes older history once context crosses threshold of context_window.
-- Use: return { compaction = require("compaction")(0.8, 20) }
local t = require("tuse")

return function(threshold, keep)
  threshold = threshold or 0.8
  keep = keep or 20
  return {
    hooks = {
      compaction = {
        events = { "agent_message" },
        run = function(e)
          local s = t.session()
          if e.status ~= "finished" or not s.context_window or s.context < s.context_window * threshold then return end
          local h = s.history
          local first = h[1] and h[1].role == "system" and 2 or 1
          local cutoff = #h - keep + 1
          while cutoff > first and h[cutoff].role ~= "user" do cutoff = cutoff - 1 end
          if cutoff <= first then return end
          local older = {}
          for i = first, cutoff - 1 do
            local r = h[i]
            local content = r.text or r.output
            if content and (r.role == "user" or r.role == "agent" or r.role == "notification" or r.role == "lua") then
              older[#older + 1] = r.role .. ": " .. content
            end
          end
          if #older == 0 then return end
          local summary = t.llm("Summarize this conversation for the next model request. Preserve decisions, constraints, and unfinished work.\n\n" .. table.concat(older, "\n\n"))
          local history = {}
          if first == 2 then history[1] = h[1] end
          history[#history + 1] = { role = "notification", text = "Earlier conversation summary:\n" .. summary }
          local latest = t.session().history
          for i = cutoff, #latest do history[#history + 1] = latest[i] end
          t.set_session{ history = history }
        end,
      },
    },
  }
end
