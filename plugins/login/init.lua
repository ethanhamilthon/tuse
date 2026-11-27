local t = require("tuse")
local path = assert(os.getenv("THIS_PATH")):match("^(.+)/[^/]+$")
local state = os.getenv("TUSE_LOGIN_STATE_DIR") or (t.session().home .. "/login")
assert(state:sub(1, 1) == "/", "TUSE_LOGIN_STATE_DIR must be absolute")
local auth = state .. "/auth"
local base = "http://127.0.0.1:18317"
local go_base = "https://opencode.ai/inference/go/v1"
local subscriptions = {
  {"Codex", "codex", "-codex-login"},
  {"Claude", "claude", "-claude-login"},
  {"Antigravity", "antigravity", "-antigravity-login"},
  {"OpenCode Go", "opencode-go"},
  {"Kimi", "kimi", "-kimi-login"},
  {"Grok", "xai", "-xai-login"},
}

local function shell(action, timeout, arg, flag)
  return t.sh('exec bash "$SCRIPT" "$ACTION" "$ARG" "$FLAG"', {timeout = timeout, env = {
    SCRIPT = path .. "/proxy.sh", STATE = state, ACTION = action,
    AUTH_PATH = t.json.encode(auth), ARG = arg or "", FLAG = flag or "",
  }})
end

local function file(name)
  local f = io.open(name, "rb")
  if not f then return nil end
  local data = f:read("a")
  f:close()
  return data
end

local function decode(raw)
  local ok, value = pcall(t.json.decode, raw or "")
  if ok and type(value) == "table" then return value end
  return {}
end

local function account_files(dir)
  local files = {}
  for name in shell("files", 3, dir).out:gmatch("[^%z]+") do
    local kind = decode(file(name)).type
    if type(kind) == "string" then
      files[kind] = files[kind] or {}
      table.insert(files[kind], name)
    end
  end
  return files
end

local function show(text)
  t.show("login", "### Subscriptions\n\n" .. text)
end

local function check(r, message)
  if not r.ok then error(message, 0) end
  return r.out
end

local function ensure_proxy()
  if shell("binary", 3).ok then return true end
  show("CLIProxyAPI is not installed. Tuse can download a verified build and run it locally for subscription sign-in. It only listens on loopback, and your credentials stay on this machine.")
  if t.pick({"Install CLIProxyAPI", "Cancel"}, "Install CLIProxyAPI?") ~= "Install CLIProxyAPI" then
    show("Installation cancelled.")
    return false
  end
  show("Downloading CLIProxyAPI 8.1.2 and verifying its SHA-256 checksum…")
  check(shell("install", 110), "Couldn't install the verified CLIProxyAPI build. Check the network or install CLIProxyAPI on PATH and retry.")
  show("CLIProxyAPI is installed.")
  return true
end

local function reload()
  local ok = pcall(t.reload)
  if not ok then error("Account changes were saved, but Tuse couldn't reload. Check the configuration and run /reload.", 0) end
end

local function add(item)
  local label, id, flag = table.unpack(item)
  if not flag then
    if not os.getenv("OPENCODE_GO_API_KEY") then
      show("Set `OPENCODE_GO_API_KEY` before launching Tuse. Get the key from the [OpenCode Console](https://opencode.ai/console). Do not paste it into chat.")
      return
    end
    show("Checking the OpenCode Go subscription…")
    local result = decode(check(shell("go-check", 8), "OpenCode Go couldn't verify this key or subscription."))
    if type(result.usage) ~= "table" then error("OpenCode Go didn't return subscription usage.", 0) end
    check(shell("go-save", 3), "Couldn't save the OpenCode Go key.")
  else
    show("Opening the browser to sign in to **" .. label .. "**. Finish there to return to Tuse.")
    check(shell("prepare", 5), "Couldn't prepare private CLIProxyAPI settings.")
    local stage = check(shell("stage", 3), "Couldn't prepare a temporary sign-in directory."):gsub("%s+$", "")
    local ok, failure = pcall(function()
      t.write(stage .. "/config.yaml", 'host: "127.0.0.1"\nauth-dir: ' .. t.json.encode(stage .. "/auth") .. "\n")
      check(shell("oauth", 240, stage, flag), "Browser sign-in didn't complete. Run /login to try again.")
      local paths = account_files(stage .. "/auth")[id]
      if not paths then error("CLIProxyAPI didn't save a matching credential.", 0) end
      for _, name in ipairs(paths) do
        local dest = auth .. "/" .. name:match("[^/]+$")
        if file(dest) then error("An existing credential matches this account.", 0) end
        assert(os.rename(name, dest))
      end
      check(shell("enable", 3), "Couldn't enable the loopback proxy.")
    end)
    shell("discard", 4, stage)
    if not ok then error(failure, 0) end
    show("Signed in to **" .. label .. "**. Starting the loopback proxy…")
    check(shell("attach", 20), "Couldn't start the loopback proxy on port 18317. Check for a port conflict and restart Tuse.")
  end
  reload()
  show("**" .. label .. "** is connected. Use `/models` to select a model.")
end

local quota_urls = {
  codex = "https://chatgpt.com/backend-api/wham/usage",
  claude = "https://api.anthropic.com/api/oauth/usage",
  kimi = "https://api.kimi.com/coding/v1/usages",
  antigravity = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
  xai = "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
}

local function object(value)
  return type(value) == "table" and value or {}
end

local function remaining(used)
  used = tonumber(used)
  if used and used >= 0 then return math.max(0, 100 - used) end
end

local function amounts(raw)
  local limit, left = tonumber(raw.limit or raw.total), tonumber(raw.remaining or raw.available)
  if limit and limit > 0 and left then return math.min(100, math.max(0, 100 * left / limit)) end
  return tonumber(raw.remaining_percent) or remaining(raw.used_percent)
end

local function quota(id, account, credential)
  local headers = {Authorization = "Bearer $TOKEN$", Accept = "application/json", ["Content-Type"] = "application/json"}
  local req = {auth_index = account.auth_index, method = "GET", url = quota_urls[id], header = headers}
  if id == "codex" then
    headers["ChatGPT-Account-Id"] = credential.account_id
  elseif id == "claude" then
    headers["anthropic-beta"] = "oauth-2025-04-20"
  elseif id == "antigravity" then
    if type(credential.project_id) ~= "string" or credential.project_id == "" then return nil end
    req.method, req.data = "POST", t.json.encode({project = credential.project_id})
  elseif id == "xai" then
    headers["x-xai-token-auth"] = "xai-grok-cli"
    headers["x-userid"] = credential.user_id
  end
  local r = shell("quota", 10, t.json.encode(req))
  if not r.ok then return nil end
  local response = decode(r.out)
  if response.status_code ~= 200 then return nil end
  return type(response.body) == "string" and decode(response.body) or object(response.body)
end

local function parse_quota(id, data)
  local short, week, extra
  if id == "opencode-go" then
    local usage = object(data.usage)
    short, week = remaining(object(usage.rolling).percent), remaining(object(usage.weekly).percent)
  elseif id == "codex" then
    local limits = object(data.rate_limit)
    for _, key in ipairs({"primary_window", "secondary_window"}) do
      local window = object(limits[key])
      if window.limit_window_seconds == 18000 then short = remaining(window.used_percent)
      elseif window.limit_window_seconds == 604800 then week = remaining(window.used_percent) end
    end
  elseif id == "claude" then
    short = remaining(object(data.five_hour).utilization)
    week = remaining(object(data.seven_day).utilization)
  elseif id == "kimi" then
    week = amounts(object(data.usage))
    for _, value in ipairs(object(data.limits)) do
      local item = object(value)
      local window = object(item.window)
      local duration = tonumber(window.duration)
      local unit = tostring(window.timeUnit or window.time_unit or ""):upper()
      if duration then
        if unit:find("HOUR", 1, true) then duration = duration * 60
        elseif unit:find("SECOND", 1, true) then duration = duration / 60
        elseif unit:find("DAY", 1, true) then duration = duration * 1440 end
        if duration == 300 then short = amounts(object(item.detail or item)) end
      end
    end
  elseif id == "antigravity" then
    for _, group in ipairs(object(data.groups)) do
      for _, value in ipairs(object(object(group).buckets)) do
        local bucket = object(value)
        local window = tostring(bucket.window or bucket.displayName or ""):lower()
        local fraction = tonumber(bucket.remainingFraction or bucket.remaining_fraction)
        if fraction and fraction >= 0 and fraction <= 1 then
          local percent = fraction * 100
          if window == "5h" or window:find("5%s*hours?") then short = math.min(short or 100, percent)
          elseif window == "7d" or window:find("week", 1, true) then week = math.min(week or 100, percent) end
        end
      end
    end
    if short or week then extra = " (lowest across quota groups)" end
  elseif id == "xai" then
    local config = object(data.config or data)
    local period = object(config.currentPeriod or config.current_period)
    local kind = tostring(period.type or config.periodType or config.period_type or ""):lower()
    local left = remaining(config.creditUsagePercent or config.credit_usage_percent or config.usagePercent or config.usage_percent or config.usedPercent or config.used_percent)
    if kind:find("week", 1, true) or kind == "7d" then week = left
    elseif kind == "5h" then short = left
    elseif left then extra = (" · billing period: %.0f%% left"):format(left) end
  end
  return short, week, extra or ""
end

local function show_usage(active)
  local rows, accounts = {}, {}
  if file(state .. "/enabled") then
    local result = shell("auth-files", 7)
    if result.ok then
      for _, value in ipairs(object(decode(result.out).files)) do
        local entry = object(value)
        if type(entry.name) == "string" and type(entry.auth_index) == "string" then accounts[entry.name] = entry end
      end
    end
  end
  local function left(value, data)
    if value then return ("%.0f%% left"):format(value) end
    return data and "not reported" or "unavailable"
  end
  local function row(label, id, data)
    local short, week, extra = parse_quota(id, object(data))
    rows[#rows + 1] = ("- **%s:** 5h: %s · 7d: %s%s"):format(label, left(short, data), left(week, data), extra)
  end
  for _, item in ipairs(subscriptions) do
    local label, id = item[1], item[2]
    if id == "opencode-go" and active[id] then
      local result = shell("go-usage", 8)
      row(label, id, result.ok and decode(result.out) or nil)
    elseif active[id] then
      for i, name in ipairs(active[id]) do
        local account = accounts[name:match("[^/]+$")]
        local title = #active[id] == 1 and label or ("%s #%d"):format(label, i)
        row(title, id, account and quota(id, account, decode(file(name))))
      end
    end
  end
  local text = #rows > 0 and table.concat(rows, "\n") or "No subscriptions connected. Use `/login` to add one."
  t.show(("login-usage-%016x"):format(math.random(0)), "### Subscription usage\n\n" .. text)
end

local function choose(active, connected, title)
  local options, by_label = {}, {}
  for _, item in ipairs(subscriptions) do
    if (active[item[2]] ~= nil) == connected then
      options[#options + 1] = item[1]
      by_label[item[1]] = item
    end
  end
  if #options == 0 then
    show(connected and "No subscriptions connected." or "All available subscriptions are already connected.")
    return
  end
  return by_label[t.pick(options, title)]
end

local function connect_api_key()
  local name = t.ask("Provider name (for example, `openrouter`):")
  if not name then return end
  name = name:match("^%s*(.-)%s*$")
  if not name:match("^[a-zA-Z0-9][a-zA-Z0-9_-]*$") then error("Use only letters, digits, hyphens, and underscores in the provider name.", 0) end
  local url = t.ask("Provider base URL, including `/v1` if the API requires it:")
  if not url then return end
  url = url:match("^%s*(.-)%s*$")
  if not url:match("^https://[^/%s?#@]+") or url:find("[%s?#@]") then error("Enter an HTTPS base URL without credentials, query parameters, or a fragment.", 0) end
  local api = t.pick({"openai-chat", "openai-responses", "anthropic-messages", "google-gemini"}, "Which API does this provider speak?")
  if not api then return end
  local env_option, private_option = "Use an environment variable", "Enter the key in a private dialog"
  local platform = t.sh("uname -s", {timeout = 2}).out
  local private = platform:match("^Darwin") and t.session().mode == "tui"
  local method = private and t.pick({private_option, env_option}, "How do you want to enter the API key?") or (not private and env_option)
  if not method then return end
  local options = {PROVIDER = name, BASE = url, API = api}
  local cmd, timeout
  if method == private_option then
    show("Enter the key in the private system dialog. It will not go into chat history.")
    cmd = [==[key=$(/usr/bin/osascript -e 'text returned of (display dialog "Enter the provider API key" default answer "" with hidden answer buttons {"Cancel", "Connect"} default button "Connect" cancel button "Cancel" with title "Tuse API key")' 2>/dev/null) || exit 2
      [ -n "$key" ] || exit 2
      printf '%s' "$key" | tuse connect --name "$PROVIDER" --base "$BASE" --api "$API" --key-stdin]==]
    timeout = 180
  else
    local var = t.ask("Name of the environment variable with the key (for example, `OPENROUTER_API_KEY`). **Do not paste the key**: Tuse saves answers in chat history.")
    if not var then return end
    var = var:match("^%s*(.-)%s*$")
    if not var:match("^[A-Z_][A-Z0-9_]*$") then error("Enter the variable name, not its value.", 0) end
    local key = os.getenv(var)
    if not key or key == "" then error("That variable is not set. Set it before launching Tuse, then try again.", 0) end
    show("Checking the provider and fetching its model list…")
    options.KEY_VAR = var
    cmd, timeout = [[exec tuse connect --name "$PROVIDER" --base "$BASE" --api "$API" --key "\$$KEY_VAR"]], 30
  end
  local connected = t.sh(cmd, {env = options, timeout = timeout})
  if method == private_option and connected.code == 2 then return end
  if not connected.ok then error("Couldn't connect. Check the endpoint, key, and whether the name is already in use.", 0) end
  reload()
  show("**" .. name .. "** is connected. Use `/models` to select a model.")
end

local subscription_args, providers_by_arg = {}, {}
for _, item in ipairs(subscriptions) do
  local arg = item[2] == "xai" and "grok" or item[2]
  subscription_args[#subscription_args + 1] = arg
  providers_by_arg[arg] = item
end

local function active_subscriptions()
  local active = account_files(auth)
  if file(state .. "/opencode-go.key") then active["opencode-go"] = true end
  return active
end

local function login(arg)
  local active = active_subscriptions()
  local item = providers_by_arg[arg] or choose(active, false, "Choose a subscription to connect")
  if not item then return end
  local label, id, flag = table.unpack(item)
  if active[id] then return show("**" .. label .. "** is already connected.") end
  if not flag or ensure_proxy() then return add(item) end
end

local function logout(arg)
  local active = active_subscriptions()
  local item = providers_by_arg[arg] or choose(active, true, "Which subscription should Tuse remove?")
  if not item then return end
  local label, id = table.unpack(item)
  if not active[id] then return show("**" .. label .. "** is not connected.") end
  local remove = "Remove " .. label
  if t.pick({remove, "Cancel"}, "Remove this subscription from Tuse? It will not cancel your plan.") ~= remove then return end
  if id == "opencode-go" then
    assert(os.remove(state .. "/opencode-go.key"))
  else
    for _, name in ipairs(active[id]) do assert(os.remove(name)) end
    if not next(account_files(auth)) then os.remove(state .. "/enabled") end
  end
  reload()
  show("**" .. label .. "** was removed from Tuse.")
end

local function report_error(fn, ...)
  local ok, message = pcall(fn, ...)
  if not ok then show("**Error:** " .. tostring(message)) end
end

local function refresh_models(provider, action)
  local result = shell(action, 8)
  if not result.ok then return false end
  local data = decode(result.out).data
  if type(data) ~= "table" then return false end
  local ids, seen = {}, {}
  for _, model in ipairs(data) do
    local id = type(model) == "table" and model.id
    if type(id) == "string" and id ~= "" and not id:find("[%c%s]") and not seen[id] then
      seen[id] = true
      ids[#ids + 1] = id
    end
  end
  table.sort(ids)
  local cache = state .. "/" .. provider .. ".models.json"
  local saved = t.json.encode(ids)
  if saved == file(cache) then return false end
  t.write(cache, saved)
  return true
end

local providers, models = {}, {}
local function load_models(provider)
  for _, id in ipairs(decode(file(state .. "/" .. provider .. ".models.json"))) do
    if type(id) == "string" then models[#models + 1] = {name = provider .. "/" .. id} end
  end
end
local key = file(state .. "/proxy.key")
if key and file(state .. "/enabled") then
  providers["login-proxy"] = {base = base .. "/v1", key = key, api = "openai-chat"}
end
local go_key = file(state .. "/opencode-go.key")
if go_key then providers["opencode-go"] = {base = go_base, key = go_key, api = "openai-chat"} end

for _, provider in ipairs({"login-proxy", "opencode-go"}) do
  if providers[provider] then load_models(provider) end
end

return {
  providers = providers,
  models = models,
  commands = {
    login = { desc = "connect a subscription", args = subscription_args, run = function(arg) report_error(login, arg) end },
    logout = { desc = "remove a subscription", args = subscription_args, run = function(arg) report_error(logout, arg) end },
    usage = { desc = "show remaining subscription limits", run = function()
      report_error(function() show_usage(active_subscriptions()) end)
    end },
    ["connect-api"] = { desc = "connect a provider using an API key", run = function() report_error(connect_api_key) end },
  },
  hooks = { login_start = { events = {"config_load"}, run = function()
    local changed = false
    if file(state .. "/enabled") then
      if shell("attach", 20).ok then
        changed = refresh_models("login-proxy", "models")
      else
        show("Couldn't start the subscription proxy on port 18317. Check for a port conflict.")
      end
    end
    if file(state .. "/opencode-go.key") then
      changed = refresh_models("opencode-go", "go-models") or changed
    end
    if changed then t.reload() end
  end } },
}
