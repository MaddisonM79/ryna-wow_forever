-- Status: what the panel, the minimap tooltip, the login line and /ryna status say about the
-- recording and the companion app. The app's side comes from RynaAgentStatus (AgentStatus.lua),
-- which may be nil or hold any subset of its fields, so every read here checks its type.

local _, ns = ...

local St = {}
ns.Status = St

local STALE_SECONDS = 24 * 3600
St.MODE_LABELS = {
  instances = "In dungeons and raids only",
  always = "Always",
  never = "Never",
}
St.MODE_ORDER = { "instances", "always", "never" }

-- Text that came from outside the game is shown as typed: "|" would start a colour or texture code.
function St.Plain(s)
  if type(s) ~= "string" then return nil end
  return (s:gsub("|", "||"))
end

-- The companion app's status table, or nil.
function St.Agent()
  local a = _G.RynaAgentStatus
  if type(a) == "table" then return a end
  return nil
end

local function field(t, key, kind)
  if type(t) ~= "table" then return nil end
  local v = t[key]
  if type(v) == kind then return v end
  return nil
end
St.Field = field

-- Dotted integers: "0.10.0" > "0.9.3". Missing parts count as 0; nil when either isn't a version.
function St.CompareVersions(a, b)
  if type(a) ~= "string" or type(b) ~= "string" then return nil end
  local pa, pb = {}, {}
  for n in a:gmatch("%d+") do pa[#pa + 1] = tonumber(n) end
  for n in b:gmatch("%d+") do pb[#pb + 1] = tonumber(n) end
  if #pa == 0 or #pb == 0 then return nil end
  for i = 1, math.max(#pa, #pb) do
    local x, y = pa[i] or 0, pb[i] or 0
    if x ~= y then return x < y and -1 or 1 end
  end
  return 0
end

function St.IsNewer(candidate, current)
  return St.CompareVersions(candidate, current) == 1
end

local function plural(n, word)
  return n .. " " .. word .. (n == 1 and "" or "s")
end

-- "just now", "12 minutes ago", "3 hours ago", "2 days ago".
function St.Ago(t)
  if type(t) ~= "number" then return nil end
  local d = ns.Now() - t
  if d < 60 then return "just now" end
  if d < 3600 then return plural(math.floor(d / 60), "minute") .. " ago" end
  if d < 86400 then return plural(math.floor(d / 3600), "hour") .. " ago" end
  return plural(math.floor(d / 86400), "day") .. " ago"
end

-- One line about the companion app: "Synced 12 minutes ago as <name>." or the sign-in hint.
function St.SyncLine()
  local a = St.Agent()
  local lastPush = field(a, "lastPush", "number")
  local as = St.Plain(field(a, "as", "string"))
  if lastPush then
    local line = "Synced " .. St.Ago(lastPush) .. (as and (" as " .. as) or "") .. "."
    if field(a, "signedIn", "boolean") == false then
      line = line .. " Signed out now: sign in from the Ryna companion app."
    end
    return line
  end
  return "Not synced yet. Sign in from the Ryna companion app."
end

-- What AgentStatus.lua held at login, for the debug line: when, as whom, signed in, messages.
function St.DebugLine()
  local a = St.Agent()
  if not a then return "empty (nothing synced yet)" end
  local lastPush = field(a, "lastPush", "number")
  local signedIn = field(a, "signedIn", "boolean")
  local list = field(a, "messages", "table")
  return string.format("synced %s as %s, %s, %d messages, app %s",
    lastPush and St.Ago(lastPush) or "never", St.Plain(field(a, "as", "string")) or "nobody",
    signedIn == nil and "sign-in unknown" or (signedIn and "signed in" or "signed out"),
    list and #list or 0, St.Plain(field(a, "agent", "string")) or "version unknown")
end

-- True when the app hasn't pushed in the last 24 hours (or never has, as far as the file says).
function St.IsStale()
  local lastPush = field(St.Agent(), "lastPush", "number")
  return not lastPush or ns.Now() - lastPush > STALE_SECONDS
end

-- The "Update available" lines, possibly none.
function St.UpdateLines()
  local a = St.Agent()
  local latest = field(a, "latest", "table")
  local out = {}
  local addon = field(latest, "addon", "string")
  if St.IsNewer(addon, ns.VERSION) then
    out[#out + 1] = "Update available: Ryna addon " .. St.Plain(addon) .. " (you have " .. ns.VERSION .. ")."
  end
  local agent, have = field(latest, "agent", "string"), field(a, "agent", "string")
  if St.IsNewer(agent, have) then
    out[#out + 1] = "Update available: companion app " .. St.Plain(agent) .. " (you have " .. St.Plain(have) .. ")."
  end
  return out
end

-- Up to `limit` messages, newest first: { at, level, text }, with anything malformed skipped.
function St.Messages(limit)
  local list = field(St.Agent(), "messages", "table")
  local out = {}
  if not list then return out end
  for i = 1, #list do
    local m = list[i]
    local text = St.Plain(field(m, "text", "string"))
    if text then
      local level = field(m, "level", "string")
      if level ~= "warn" and level ~= "error" then level = "info" end
      out[#out + 1] = { at = field(m, "at", "number"), level = level, text = text, order = i }
    end
  end
  table.sort(out, function(x, y)
    if (x.at or 0) ~= (y.at or 0) then return (x.at or 0) > (y.at or 0) end
    return x.order > y.order
  end)
  for i = #out, (limit or 10) + 1, -1 do out[i] = nil end
  return out
end

-- Counts for "what has been recorded this session".
function St.Counts()
  local rec, db = ns.rec, ns.db
  local c = { items = 0, gear = 0, active = 0, completed = 0, roster = 0, journal = 0, session = 0 }
  if db then c.journal = #db.journal end
  c.session = ns.Journal.seq or 0
  if not rec then return c end
  for _, row in ipairs(rec.containers or {}) do
    if type(row) == "table" then c.items = c.items + (tonumber(row.count) or 1) end
  end
  c.gear = #(rec.gear or {})
  local q = rec.quests or {}
  c.active, c.completed = #(q.active or {}), #(q.completed or {})
  local guild = rec.guild and db and db.guilds[rec.guild.guid]
  if type(guild) == "table" and type(guild.roster) == "table" then c.roster = #guild.roster end
  return c
end

-- What the addon did about logging: it never asks the client (Zone.lua), so "on" means Ryna
-- turned it on, and "not turned on by Ryna" covers both off and a /combatlog typed by hand.
function St.LoggingText()
  local on = ns.Zone.IsLogging()
  if on == nil then return "unavailable on this client" end
  return on and "on (turned on by Ryna)" or "not turned on by Ryna"
end
