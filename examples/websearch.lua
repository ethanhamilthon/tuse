-- A websearch(query) agent function backed by a SearXNG instance.
-- Use: return { websearch = require("websearch")("http://localhost:8888") }
local t = require("tuse")

return function(base)
  return {
    systems = {
      "Call `websearch(query, n)` to search the web; it returns titles, URLs, and snippets.",
    },
    functions = {
      websearch = function(q, n)
        local r = t.sh("curl -fsS --get \"$BASE/search\" --data-urlencode \"q=$Q\" -d format=json",
          { env = { BASE = base, Q = q }, timeout = 30 })
        if not r.ok then
          error("websearch: " .. r.out)
        end
        local out = {}
        for i, it in ipairs(t.json.decode(r.out).results or {}) do
          if i > (n or 8) then
            break
          end
          out[#out + 1] = string.format("%d. %s\n   %s\n   %s", i, it.title or "", it.url or "", it.content or "")
        end
        return table.concat(out, "\n")
      end,
    },
  }
end
