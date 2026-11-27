-- Names the session with a small model after the first message, and adds /rename.
-- Use: return { rename = require("rename")("openai/gpt-5-mini") }
local t = require("tuse")

local function title(text, model)
  local out = t.llm("Give a 3-6 word title for this request. Reply with the title only.\n\n" .. text, model, "low")
  return (out:gsub("[\r\n\"]", " "):gsub("^%s+", ""):gsub("%s+$", ""))
end

return function(model)
  return {
    hooks = {
      rename = {
        events = { "user_message" },
        run = function()
          local first
          for _, record in ipairs(t.session().history) do
            if record.role == "user" then
              if first then return end
              first = record.text
            end
          end
          if first then t.set_session{ title = title(first, model) } end
        end,
      },
    },
    commands = {
      rename = {
        desc = "set the session title",
        run = function()
          local name = t.ask("New session title:")
          if name and name ~= "" then
            t.set_session{ title = name }
          end
        end,
      },
    },
  }
end
