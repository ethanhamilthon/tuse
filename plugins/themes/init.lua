local t = require("tuse")
local path = assert(os.getenv("THIS_PATH")):match("^(.+)/[^/]+$")
local rows = require(path .. "/palettes")
local state = os.getenv("TUSE_THEME_STATE_FILE") or (t.session().home .. "/theme")
local themes, labels = {}, {}
local function color(hex) return "#" .. hex end
for _, p in ipairs(rows) do
  local name, bg, fg, accent, muted, surface, green, red, blue = table.unpack(p)
  labels[#labels + 1] = name
  themes[name] = {
    bg = color(bg), fg = color(fg), accent = color(accent), muted = color(muted),
    border = color(surface), user = "fg", user_bg = "border", input = "fg", input_bg = "bg",
    error = color(red), icons = "accent",
    code = {keyword = "accent", storage = "accent", constant = "accent", operator = "fg", punctuation = "muted",
      number = "accent", string = color(green), escape = color(blue), comment = "muted", ["function"] = color(blue),
      builtin = color(blue), method = color(blue), field = "fg", parameter = "fg", variable = "fg", added = color(green), removed = "error"},
    markdown = {heading = "accent", link = color(blue), code = color(green), quote = "muted", list = "accent"},
  }
end
themes["Lavender Mist"] = require(path .. "/lavender-mist")
labels[#labels + 1] = "Lavender Mist"
table.sort(labels)
local function selected()
  local f = io.open(state, "r")
  if not f then return end
  local name = f:read("l")
  f:close()
  return themes[name] and name or nil
end
local name = selected()
return {
  ui = name and {theme = themes[name]} or {},
  commands = { theme = { desc = "choose or apply a color theme", run = function(...)
    local choice
    if select("#", ...) > 0 then
      choice = table.concat({...}, " ")
      local wanted = choice:lower():gsub("%-", " ")
      if wanted == "reset" then
        choice = "Use configured colors"
      else
        choice = nil
        for _, label in ipairs(labels) do
          if label:lower():gsub("%-", " ") == wanted then choice = label; break end
        end
        if not choice then
          t.show("theme", "Unknown theme. Run `/theme` to see the list.")
          return
        end
      end
    else
      local options = {table.unpack(labels)}
      if name then options[#options + 1] = "Use configured colors" end
      choice = t.pick(options, "Choose a theme" .. (name and " · current: " .. name or ""))
      if not choice then return end
    end
    if choice == "Use configured colors" then
      if selected() then t.delete(state) end
      t.reload()
      t.show("theme", "Restored configured colors.")
    else
      t.write(state, choice .. "\n")
      t.reload()
      t.show("theme", "Selected **" .. choice .. "**. Configured color overrides still apply.")
    end
  end } },
}
