-- A few full themes. Use: g.ui = g.ui or {}; g.ui.theme = require("themes").gruvbox
local function theme(c)
  return {
    bg = c.bg, fg = c.fg, accent = c.accent, muted = c.muted, border = c.border,
    user_bg = c.surface, input_bg = c.border, error = c.err,
    code = {
      keyword = c.err, storage = c.err, string = c.ok, escape = c.info,
      ["function"] = c.info, builtin = c.info, method = c.info,
      operator = c.dim, parameter = c.dim, added = c.ok, removed = c.err,
    },
    markdown = { heading = c.info, code = c.ok },
  }
end

return {
  light = theme {
    bg = "#FAFAFA", surface = "#EDEDED", fg = "#1F1F1F", dim = "#4A4A4A", muted = "#6E6E6E",
    border = "#D0D0D0", accent = "#2F7D32", ok = "#2F7D32", err = "#C62828", info = "#1565C0",
  },
  gruvbox = theme {
    bg = "#282828", surface = "#3C3836", fg = "#EBDBB2", dim = "#D5C4A1", muted = "#A89984",
    border = "#504945", accent = "#FABD2F", ok = "#B8BB26", err = "#FB4934", info = "#83A598",
  },
  solarized = theme {
    bg = "#002B36", surface = "#073642", fg = "#EEE8D5", dim = "#93A1A1", muted = "#839496",
    border = "#2A4A53", accent = "#B58900", ok = "#859900", err = "#DC322F", info = "#268BD2",
  },
  terminal = theme {
    bg = "none", surface = "0", fg = "white", dim = "grey", muted = "dark_grey",
    border = "dark_grey", accent = "cyan", ok = "green", err = "red", info = "blue",
  },
}
