local t = require("tuse")
local function env(name)
  local value = os.getenv(name)
  return value ~= "" and value or nil
end
local ids = {herdr = env("HERDR_PANE_ID"), ["herdr-app"] = env("HC_TERMINAL_ID"), rex = env("REX_BLOCK"), cmux = env("CMUX_SURFACE_ID")}
local order = {"herdr", "herdr-app", "rex", "cmux"}
local state, last, previous, backend = "idle"
local waiting = 0
local alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
local function b64(s)
  local out = {}
  for i = 1, #s, 3 do
    local a, b, c = s:byte(i, i + 2)
    local bits = (a << 16) | ((b or 0) << 8) | (c or 0)
    for n = 0, 3 do
      local index = ((bits >> (18 - n * 6)) & 63) + 1
      out[#out + 1] = ((n == 2 and not b) or (n == 3 and not c)) and "=" or alphabet:sub(index, index)
    end
  end
  return table.concat(out)
end
local function detect()
  local count, first = 0
  for _, name in ipairs(order) do if ids[name] then count, first = count + 1, first or name end end
  if count < 2 then return first or "osc" end
  local paths = t.sh([[p=$PPID; for i in {1..24}; do
    ps -p "$p" -o comm=; p=$(ps -p "$p" -o ppid= | tr -d ' '); [ "${p:-0}" -gt 1 ] || break
  done]], {timeout = 2})
  for line in paths.out:lower():gmatch("[^\n]+") do
    local name = line:find("herdr.app", 1, true) and "herdr-app" or line:match("[/%s]hc$") and "herdr-app"
      or line:match("herdr%-app$") and "herdr-app" or line:match("herdr$") and "herdr"
      or line:match("rex$") and "rex" or line:match("cmux$") and "cmux"
    if name and ids[name] then return name end
  end
  return first
end

local function report(value)
  local s = t.session()
  if s.mode ~= "tui" then return end
  backend = backend or detect()
  local title = (s.title or "Tuse"):gsub("%c", " "):gsub("\194[\128-\159]", " ")
  if value == last and title == previous then return end
  local args = {ID = ids[backend] or "", TITLE = title, STATE = value}
  local function run(cmd) return t.sh(cmd, {env = args, timeout = 3}).ok end
  local ok
  if backend == "herdr" then
    args.NATIVE = ({working = "working", blocked = "blocked", clear = "unknown"})[value] or "idle"
    if title ~= previous and value ~= "clear" then run('herdr pane rename -- "$ID" "$TITLE"') end
    ok = run('exec herdr pane report-agent --source tuse --agent tuse --state "$NATIVE" --message "$STATE" -- "$ID"')
  elseif backend == "rex" then
    args.LABEL = (value == "clear" or value == "idle") and title or title .. " · " .. value
    ok = run('exec rex block rename "$ID" "$LABEL"')
  elseif backend == "cmux" then
    if title ~= previous and value ~= "clear" then run('cmux rename-tab --surface "$ID" -- "$TITLE"') end
    ok = run((value == "clear" or value == "idle") and 'exec cmux clear-status tuse' or 'exec cmux set-status tuse "$STATE"')
  else
    local bytes = "\27]7501;state=" .. value .. ":app=tuse"
    if value == "blocked" then bytes = bytes .. ":kind=question" end
    if value ~= "clear" then bytes = bytes .. ":title=" .. b64(title) end
    bytes = bytes .. "\27\\"
    if backend == "herdr-app" and title ~= previous and value ~= "clear" then bytes = bytes .. "\27]2;" .. title .. "\27\\" end
    local tty = io.open("/dev/tty", "w")
    if tty then tty:setvbuf("no"); ok = tty:write(bytes); tty:close() end
  end
  if ok then last, previous = value, title end
end
local function refresh()
  if waiting > 0 then return report("blocked") end
  if state == "idle" or state == "done" then
    for _, task in ipairs(t.task_list()) do if task.status == "running" then return report("working") end end
  end
  report(state)
end

local function interactive(fn)
  return function(...)
    waiting = waiting + 1
    refresh()
    local ok, result = pcall(fn, ...)
    waiting = math.max(0, waiting - 1)
    refresh()
    if not ok then error(result, 0) end
    return result
  end
end

return {
  functions = {ask = interactive(t.ask), pick = interactive(t.pick)},
  hooks = {status_terminal = {
    events = {"config_load", "session_created", "session_resumed", "session_close", "session_updated", "process_end",
      "user_message", "agent_message", "turn_error", "turn_stopped", "lua_start", "lua_end"},
    run = function(e)
      local s = t.session()
      if s.mode ~= "tui" then return end
      local event = e.event
      if event == "session_close" or event == "process_end" then report("clear"); return end
      if event == "user_message" then state = "working"
      elseif event == "agent_message" then state = ({working = "working", finished = "done", cancelled = "cancelled"})[e.status] or "idle"
      elseif event == "turn_error" then state = "error"
      elseif event == "turn_stopped" then waiting = 0; if state ~= "error" then state = "cancelled" end
      elseif event == "config_load" then state = s.busy and "working" or state
      elseif event == "session_created" or event == "session_resumed" then state = s.busy and "working" or "idle" end
      refresh()
    end,
  }},
}
