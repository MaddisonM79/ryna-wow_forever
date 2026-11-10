-- Status: what the panel, the minimap tooltip, the login line and /ryna status say about the
-- companion app (the recording's own lines are Chronicle's, in its Status.lua). The app's side comes
-- from RynaAgentStatus (AgentStatus.lua), which may be nil or hold any subset of its fields, so every
-- read here checks its type.

local _, ns = ...
local L = ns.L
local N = ns.N

local St = {}
ns.Status = St

local STALE_SECONDS = 24 * 3600

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

-- "just now", "12 minutes ago", "3 hours ago", "2 days ago".
function St.Ago(t)
  if type(t) ~= "number" then return nil end
  local d = ns.Now() - t
  if d < 60 then return L["just now"] end
  if d < 3600 then return ns.Plural(math.floor(d / 60), L["%d minute ago"], L["%d minutes ago"]) end
  if d < 86400 then return ns.Plural(math.floor(d / 3600), L["%d hour ago"], L["%d hours ago"]) end
  return ns.Plural(math.floor(d / 86400), L["%d day ago"], L["%d days ago"])
end

-- One line about the companion app: "Synced 12 minutes ago as <name>." or the sign-in hint.
function St.SyncLine()
  local a = St.Agent()
  local lastPush = field(a, "lastPush", "number")
  local as = St.Plain(field(a, "as", "string"))
  if lastPush then
    local line
    if as then
      line = string.format(L["Synced %s as %s."], St.Ago(lastPush), as)
    else
      line = string.format(L["Synced %s."], St.Ago(lastPush))
    end
    if field(a, "signedIn", "boolean") == false then
      line = line .. " " .. L["Signed out now: sign in from the Ryna companion app."]
    end
    return line
  end
  return L["Not synced yet. Sign in from the Ryna companion app."]
end

-- What AgentStatus.lua held at login, for the debug line: when, as whom, signed in, messages.
-- Debug text stays English; only St.Ago's "12 minutes ago" comes out in the client's language.
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

-- When the companion app last wrote AgentStatus.lua, as near as the file says: it holds no time of
-- its own, so this is the newest time in it (the last upload, an account's save or upload, a
-- message). Nil when the file is empty or has none.
function St.FileTime()
  local a = St.Agent()
  local newest = field(a, "lastPush", "number")
  local function consider(t, key)
    local v = field(t, key, "number")
    if v and (not newest or v > newest) then newest = v end
  end
  local accounts = field(a, "accounts", "table")
  for i = 1, accounts and #accounts or 0 do
    consider(accounts[i], "savedAt")
    consider(accounts[i], "pushedAt")
  end
  local messages = field(a, "messages", "table")
  for i = 1, messages and #messages or 0 do consider(messages[i], "at") end
  return newest
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
    out[#out + 1] = string.format(L["Update available: Ryna addon %s (you have %s)."], St.Plain(addon), ns.VERSION)
  end
  local agent, have = field(latest, "agent", "string"), field(a, "agent", "string")
  if St.IsNewer(agent, have) then
    out[#out + 1] = string.format(L["Update available: companion app %s (you have %s)."], St.Plain(agent),
      St.Plain(have))
  end
  return out
end

-- Supported versions (docs/ryna-agent-cores-mods.md 9.1): what the API last said about this
-- version, as the companion app wrote it (RynaAgentStatus.support). Only `reduced` and `unsupported`
-- have words here; `behind` is the "Update available" lines above, and anything else, or nothing, is
-- current. The sentences are Maddison's, the same in the app, the game and the portal; the app writes
-- them in English, so they're compared in English and shown through L.
local SENTENCES = {
  reduced = N("This version of Ryna is getting old. Your characters still sync, but what you add to "
    .. "everyone's census and prices is paused until you update."),
  unsupported = N("This version of Ryna is no longer supported, so nothing syncs to ryna.io. Update to "
    .. "carry on."),
}
St.SUPPORT_SENTENCES = SENTENCES

local SUPPORT_COLOR = { reduced = "|cffffb000", unsupported = "|cffff4040" }

-- { band, sentence (English), note } for a reduced or unsupported band, else nil.
function St.Support()
  local sup = field(St.Agent(), "support", "table")
  local band = field(sup, "band", "string")
  local sentence = band and SENTENCES[band]
  if not sentence then return nil end
  return { band = band, sentence = sentence, note = St.Plain(field(sup, "note", "string")) }
end

-- The Status tab's first lines: the band's sentence, coloured, with how to update, then the note.
-- None when current, behind or unknown.
function St.SupportLines()
  local sup = St.Support()
  if not sup then return {} end
  local lines = { SUPPORT_COLOR[sup.band] .. L[sup.sentence] .. " " .. L["Open the Ryna app to update."] .. "|r" }
  if sup.note then lines[2] = SUPPORT_COLOR[sup.band] .. sup.note .. "|r" end
  return lines
end

-- Up to `limit` messages, newest first: { at, level, text }, with anything malformed skipped. The
-- app also writes the band's sentence as a message (for addons that don't read `support`); it is
-- skipped here while St.SupportLines says it, so it isn't shown twice.
function St.Messages(limit)
  local list = field(St.Agent(), "messages", "table")
  local out = {}
  if not list then return out end
  local sup = St.Support()
  local said = sup and sup.sentence
  for i = 1, #list do
    local m = list[i]
    local raw = field(m, "text", "string")
    local text = raw ~= said and St.Plain(raw) or nil
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
