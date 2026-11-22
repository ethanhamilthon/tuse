-- Loaded into every VM before the config. `tuse` holds the internals.
local tuse, SCHED = ...
local raw_resume, raw_yield, status = coroutine.resume, coroutine.yield, coroutine.status
local isyieldable = coroutine.isyieldable

-- A coroutine of the user that calls `sh` or `sleep` passes the wait on to the scheduler.
local function pass(co, ok, ...)
  if ok and (...) == SCHED and status(co) == "suspended" then
    return pass(co, raw_resume(co, raw_yield(...)))
  end
  return ok, ...
end
function coroutine.resume(co, ...)
  return pass(co, raw_resume(co, ...))
end
function coroutine.wrap(f)
  local co = coroutine.create(f)
  return function(...)
    local function check(ok, ...)
      if not ok then error((...), 2) end
      return ...
    end
    return check(coroutine.resume(co, ...))
  end
end

-- Waits for the scheduler; blocks the VM where Lua cannot yield (the config, a C callback).
local function wait(kind, arg)
  local ok, v
  if isyieldable() then
    ok, v = raw_yield(SCHED, kind, arg)
  else
    ok, v = tuse.block(kind, arg)
  end
  if not ok then error(v, 3) end
  return v
end

local sh_mt = {}
sh_mt.__index = sh_mt
sh_mt.__name = "sh"
function sh_mt:__tostring()
  local note = ""
  if self.timed_out then note = "[timed out]"
  elseif self.code == nil then note = "[killed by signal]"
  elseif self.code ~= 0 then note = "[exit code " .. self.code .. "]" end
  if note == "" then return self.out end
  if self.out ~= "" and self.out:sub(-1) ~= "\n" then return self.out .. "\n" .. note end
  return self.out .. note
end
function sh_mt:lines()
  local t = {}
  for l in (self.out .. "\n"):gmatch("(.-)\r?\n") do t[#t + 1] = l end
  if t[#t] == "" then t[#t] = nil end
  return t
end
tuse.sh_mt = sh_mt

function sh(cmd, opts)
  return wait("sh", tuse.sh_start(cmd, opts))
end

function sleep(secs)
  wait("sleep", secs or 0)
end

local function key_order(a, b)
  local ta, tb = type(a), type(b)
  if ta ~= tb then return ta < tb end
  if ta == "number" or ta == "string" then return a < b end
  return tostring(a) < tostring(b)
end

-- A readable form of any value: tables nest, strings are quoted inside tables.
function inspect(v, depth, seen)
  depth, seen = depth or 0, seen or {}
  local t = type(v)
  if t == "string" then return depth == 0 and v or string.format("%q", v) end
  if t ~= "table" then return tostring(v) end
  local mt = getmetatable(v)
  if mt and mt.__tostring then return tostring(v) end
  if seen[v] then return "<cycle>" end
  if depth > 6 then return "{...}" end
  seen[v] = true
  local parts, n = {}, #v
  for i = 1, n do parts[#parts + 1] = inspect(v[i], depth + 1, seen) end
  local keys = {}
  for k in pairs(v) do
    if not (math.type(k) == "integer" and k >= 1 and k <= n) then keys[#keys + 1] = k end
  end
  table.sort(keys, key_order)
  for _, k in ipairs(keys) do
    local name = type(k) == "string" and k:match("^[%a_][%w_]*$") and k or "[" .. inspect(k, depth + 1, seen) .. "]"
    parts[#parts + 1] = name .. " = " .. inspect(v[k], depth + 1, seen)
  end
  seen[v] = nil
  return "{" .. table.concat(parts, ", ") .. "}"
end

-- What the agent sees for the values a call returns.
function tuse.render(...)
  local out = {}
  for i = 1, select("#", ...) do
    local v = select(i, ...)
    if v ~= nil then out[#out + 1] = inspect(v) end
  end
  return table.concat(out, "\t")
end

-- A Rust function raises its error as a plain string, so `pcall` gets just the message.
local function clean(e)
  if type(e) ~= "userdata" then return e end
  local s = tostring(e):gsub("^runtime error: ", "")
  return (s:gsub("\nstack traceback:.*$", ""))
end
local function unwrap(ok, ...)
  if ok then return ... end
  error(clean((...)), 2)
end
function tuse.wrap(f)
  return function(...) return unwrap(pcall(f, ...)) end
end
tuse.sh_start = tuse.wrap(tuse.sh_start)

os.exit = nil
