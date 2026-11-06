-- Core: the namespace, safe-call helpers, the event dispatcher, timers and RynaDB setup.
--
-- Everything here is written so that a missing or changed client API can never break login:
-- uncertain functions are looked up with ns.Fn and called through ns.Try (pcall), and every
-- event handler and timer runs inside pcall with errors counted and reported once.

local addonName, ns = ...

ns.NAME = addonName
ns.VERSION = "0.1.0"
ns.SCHEMA = 1
ns.FLAVOR = "wowf"
ns.COMBAT_LOG_MODES = { instances = true, always = true, never = true }

-- Returns the global function `a`, or `a.b` when b is given, or nil when it doesn't exist.
function ns.Fn(a, b)
  local v = _G[a]
  if b ~= nil then
    if type(v) ~= "table" then return nil end
    v = v[b]
  end
  if type(v) == "function" then return v end
  return nil
end

local function passResults(ok, ...)
  if ok then return ... end
  return nil
end

-- Calls fn(...) under pcall. Returns its results, or a single nil when fn is missing or errors.
function ns.Try(fn, ...)
  if type(fn) ~= "function" then return nil end
  return passResults(pcall(fn, ...))
end

-- The 12.x engine can hand addons "secret" values in some contexts; never key or compare them.
function ns.IsSecret(v)
  local f = ns.Fn("issecretvalue")
  if not f then return false end
  local ok, secret = pcall(f, v)
  return ok and secret and true or false
end

function ns.NonEmpty(s)
  if type(s) == "string" and s ~= "" and not ns.IsSecret(s) then return s end
  return nil
end

function ns.Now()
  local f = ns.Fn("GetServerTime")
  return (f and ns.Try(f)) or time()
end

function ns.Print(msg)
  local frame = _G.DEFAULT_CHAT_FRAME
  if frame and frame.AddMessage then
    frame:AddMessage("|cff7fb2ffRyna|r: " .. tostring(msg))
  end
end

-- A chat line without the "Ryna:" prefix, for lines that already name the addon.
function ns.PrintRaw(msg)
  local frame = _G.DEFAULT_CHAT_FRAME
  if frame and frame.AddMessage then frame:AddMessage(tostring(msg)) end
end

-- Debug lines (RynaDB.settings.debug, off by default): one dimmed "Ryna debug: ..." line in chat
-- for each thing the addon does. The format is applied under pcall with nil, boolean and table
-- arguments passed through tostring, so a bad format or argument prints the raw format instead
-- of raising an error.
local function debugText(fmt, ...)
  local n = select("#", ...)
  if n == 0 then return tostring(fmt) end
  local args = { ... }
  for i = 1, n do
    local t = type(args[i])
    if t ~= "number" and t ~= "string" then args[i] = tostring(args[i]) end
  end
  return string.format(tostring(fmt), unpack(args, 1, n))
end

local function debugLine(fmt, ...)
  local ok, msg = pcall(debugText, fmt, ...)
  if ok then return msg end
  ok, msg = pcall(tostring, fmt)
  return ok and msg or "?"
end

local function debugEmit(msg)
  ns.PrintRaw("|cff7fb2ffRyna debug|r: |cffa0a0a0" .. msg .. "|r")
end

function ns.Debugging()
  return ns.Setting("debug") and true or false
end

function ns.Debug(fmt, ...)
  if not ns.Setting("debug") then return end
  debugEmit(debugLine(fmt, ...))
end

-- For scans that busy events re-run (quest log, reputations): a line identical to the last one
-- under the same key within five minutes isn't printed again.
local lastDebug = {}
local REPEAT_SECONDS = 300
function ns.DebugChanged(key, fmt, ...)
  if not ns.Setting("debug") then return end
  local msg = debugLine(fmt, ...)
  local now = GetTime()
  local last = lastDebug[key]
  if last and last.msg == msg and now - last.at < REPEAT_SECONDS then return end
  lastDebug[key] = { msg = msg, at = now }
  debugEmit(msg)
end

-- The debug setting, from the panel or /ryna debug.
function ns.SetDebug(on)
  if not ns.db then return end
  ns.db.settings.debug = on and true or false
  if on then
    ns.Debug("on")
  else
    ns.Print("debug messages off.")
  end
end

-- Text helpers for debug lines: "1g 20s 5c", "3h 12m", "A, B, C and 4 more".
function ns.MoneyText(copper)
  if type(copper) ~= "number" then return "?" end
  local sign = copper < 0 and "-" or ""
  copper = math.abs(math.floor(copper))
  local parts = {}
  local g, s, c = math.floor(copper / 10000), math.floor(copper / 100) % 100, copper % 100
  if g > 0 then parts[#parts + 1] = g .. "g" end
  if s > 0 then parts[#parts + 1] = s .. "s" end
  if c > 0 or #parts == 0 then parts[#parts + 1] = c .. "c" end
  return sign .. table.concat(parts, " ")
end

function ns.DurationText(seconds)
  if type(seconds) ~= "number" then return "unknown" end
  local m = math.floor(seconds / 60)
  if m < 1 then return "under a minute" end
  local d, h = math.floor(m / 1440), math.floor(m / 60) % 24
  if d > 0 then return string.format("%dd %dh", d, h) end
  if h > 0 then return string.format("%dh %dm", h, m % 60) end
  return m .. "m"
end

function ns.ListText(list, max)
  max = max or 6
  local shown = {}
  for i = 1, math.min(#list, max) do shown[i] = tostring(list[i]) end
  local out = table.concat(shown, ", ")
  if #list > max then out = out .. " and " .. (#list - max) .. " more" end
  return out
end

-- Errors: counted per label, printed once per label, the last few kept for /ryna status.
ns.errors = { count = 0, seen = {}, last = nil }

function ns.ReportError(label, err)
  local e = ns.errors
  e.count = e.count + 1
  e.last = tostring(label) .. ": " .. tostring(err)
  if not e.seen[label] then
    e.seen[label] = true
    ns.Print("an error in " .. tostring(label) .. " was caught (reported once): " .. tostring(err))
  end
end

function ns.Run(label, fn, ...)
  local ok, err = pcall(fn, ...)
  if not ok then ns.ReportError(label, err) end
end

function ns.After(delay, label, fn)
  if _G.C_Timer and _G.C_Timer.After then
    _G.C_Timer.After(delay, function() ns.Run(label, fn) end)
  else
    ns.Run(label, fn)
  end
end

-- Coalesces bursts: the first call schedules fn after `delay`; calls until it runs are dropped.
local pending = {}
function ns.Debounce(key, delay, fn)
  if pending[key] then return end
  pending[key] = true
  ns.After(delay, key, function()
    pending[key] = nil
    fn()
  end)
end

-- The event dispatcher: one frame, a list of handlers per event, each called as fn(event, ...).
local frame = CreateFrame("Frame")
local handlers = {}

function ns.On(event, fn)
  local list = handlers[event]
  if not list then
    -- Registering an event this client doesn't have raises an error; that event is just skipped.
    local ok = pcall(frame.RegisterEvent, frame, event)
    if not ok then return false end
    list = {}
    handlers[event] = list
  end
  list[#list + 1] = fn
  return true
end

frame:SetScript("OnEvent", function(_, event, ...)
  local list = handlers[event]
  if not list then return end
  for i = 1, #list do
    local ok, err = pcall(list[i], event, ...)
    if not ok then ns.ReportError(event, err) end
  end
end)

-- Settings (RynaDB.settings). The client may not restore SavedVariables, so every value has a
-- default that is applied whenever it is missing or the wrong type.
--   combatLog       "instances" | "always" | "never"
--   chatSightings   record a `seen` event for who speaks in chat
--   census          take part in the census at all: when off, no `seen` events from any source
--   groupMembers    record the other players in my group (needs census)
--   debug           print what the addon is doing to chat (ns.Debug)
--   minimap         { shown = true, angle = <degrees> } for the minimap button
ns.SETTING_DEFAULTS = { chatSightings = true, census = true, groupMembers = true, debug = false }
ns.MINIMAP_DEFAULT_ANGLE = 225

function ns.ApplySettingDefaults(settings)
  if not ns.COMBAT_LOG_MODES[settings.combatLog] then settings.combatLog = "instances" end
  for key, default in pairs(ns.SETTING_DEFAULTS) do
    if type(settings[key]) ~= "boolean" then settings[key] = default end
  end
  if type(settings.minimap) ~= "table" then settings.minimap = {} end
  local mm = settings.minimap
  if type(mm.shown) ~= "boolean" then mm.shown = true end
  if type(mm.angle) ~= "number" then mm.angle = ns.MINIMAP_DEFAULT_ANGLE end
end

-- A boolean setting, its default when RynaDB isn't set up yet.
function ns.Setting(key)
  local settings = ns.db and ns.db.settings
  local v = settings and settings[key]
  if type(v) == "boolean" then return v end
  return ns.SETTING_DEFAULTS[key]
end

-- RynaDB: rebuilt around whatever the client restored (possibly nothing). Only `characters`,
-- `guilds` and `journal` are carried over; everything in them is refreshed from the live game.
function ns.InitDB()
  local db = _G.RynaDB
  if type(db) ~= "table" or db.schema ~= ns.SCHEMA then
    local settings = type(db) == "table" and type(db.settings) == "table" and db.settings or nil
    db = { settings = settings }
  end
  db.schema = ns.SCHEMA
  db.addon = ns.VERSION
  db.flavor = ns.FLAVOR
  if type(db.characters) ~= "table" then db.characters = {} end
  if type(db.guilds) ~= "table" then db.guilds = {} end
  if type(db.journal) ~= "table" then db.journal = {} end
  if type(db.settings) ~= "table" then db.settings = {} end
  ns.ApplySettingDefaults(db.settings)
  RynaDB = db
  ns.db = db
end

-- What the client reports about itself and the player's name parts, for checking the TOC's
-- Interface number and how Forever splits "First-Last" (it is overwritten every login).
function ns.RecordClient(nameParts)
  local version, build, buildDate, interface = ns.Try(ns.Fn("GetBuildInfo"))
  ns.db.client = {
    version = version, build = build, date = buildDate, interface = interface,
    unitName = nameParts and nameParts.unitName,
    unitFullName = nameParts and nameParts.unitFullName,
    unitFullNameRealm = nameParts and nameParts.unitFullNameRealm,
    realm = nameParts and nameParts.realm,
    realmName = nameParts and nameParts.realmName,
  }
end
