-- Mods: the mod API, the one global this addon defines for other addons (`Ryna`), and the registry of
-- the mods that use it. A mod is its own addon folder (`Ryna_<Name>`, `## Dependencies: Ryna`) that
-- the companion app installs beside this one (docs/ryna-agent-cores-mods.md section 6). Chronicle,
-- everything recorded about a character, is the first (mods/wow/chronicle). A mod reaches the core
-- only through this table, never through the core's private namespace, so the core can change
-- inside without breaking one. The surface (API 1):
--
--   Ryna.API, Ryna.VERSION            the API's number (1) and the core addon's version
--   Ryna.RegisterMod(spec)            { id, addon, name, version, onLogin, onLogout, onDefaults } -> true
--                                     onLogin runs at PLAYER_LOGIN after RynaDB is set up and the
--                                     journal session has started; onLogout at PLAYER_LOGOUT;
--                                     onDefaults after the panel's Defaults button reset the settings
--   Ryna.Mods()                       the registered mods, in the order they registered
--   Ryna.On(event, fn)                the event dispatcher: fn(event, ...) under pcall
--   Ryna.DB()                         RynaDB (nil before the client loads it). A mod writes only
--                                     under its own key (RynaDB[<mod id>]) or in the character
--                                     record it keeps; Chronicle, the first mod, keeps the keys
--                                     the companion app reads (characters, guilds, client)
--   Ryna.Journal.Add(type, fields, quiet), .Session(), .Seq(), .Count()
--   Ryna.Settings.Get(key), .Set(key, value), .All(), .AddDefaults(defaults), .COMBAT_LOG_MODES
--   Ryna.Panel.AddTab{ key, label, order, build(frame), layout(stack, settings) }
--   Ryna.Panel.AddStatusLines(order, fn)   fn() -> lines for the Status tab, at `order`
--   Ryna.Panel.Refresh(), .RefreshIfShown(), .Widgets (text, button, link, checkbox, toggle, WIDTH)
--   Ryna.Slash.Add{ word, run(arg), help, order }   a /ryna <word> command and its help line
--   Ryna.Slash.AddStatusLines(order, fn)   fn() -> lines for /ryna status, at `order`
--   Ryna.L                            the core's strings (a mod's own L falls back to it)
--   Ryna.Try, .Fn, .IsSecret, .NonEmpty, .Now, .Locale, .Print, .PrintRaw, .Debug, .DebugChanged,
--   .Debugging, .Plural, .N, .Run, .After, .Debounce, .ReportError, .CanonicalName, .MoneyText,
--   .DurationText, .ListText, .Ago, .Plain   the helpers (Core.lua, Status.lua)
--
-- Order numbers for the shared lists: the core's own Status tab lines are at 30 (journal), 35 (the
-- combat log) and 50 (the companion app), its /ryna status lines at 10 (versions), 30 (journal), 40
-- (the combat log) and 50 (the rest), its tabs at 10 (Status), 40 (Messages), 50 (Mods) and 60
-- (Advanced), its commands at 10 (status), 20 (combatlog) and 40 (debug).

local _, ns = ...

local R = {}
Ryna = R

R.API = 1
R.VERSION = ns.VERSION
R.L = ns.L

-- The helpers, as they are in Core.lua and Status.lua.
for _, name in ipairs({ "Try", "Fn", "IsSecret", "NonEmpty", "Now", "Locale", "Print", "PrintRaw", "Debug",
  "DebugChanged", "Debugging", "Plural", "N", "Run", "After", "Debounce", "ReportError", "CanonicalName",
  "MoneyText", "DurationText", "ListText", "On" }) do
  R[name] = ns[name]
end
R.Ago = ns.Status.Ago
R.Plain = ns.Status.Plain

function R.DB()
  return ns.db
end

-- Mods ------------------------------------------------------------------------------------------

ns.mods, ns.modsById, ns.modsByAddon = {}, {}, {}

function R.RegisterMod(spec)
  if type(spec) ~= "table" or type(spec.id) ~= "string" or spec.id == "" or ns.modsById[spec.id] then
    return false
  end
  local mod = {
    id = spec.id,
    addon = type(spec.addon) == "string" and spec.addon or nil,
    name = type(spec.name) == "string" and spec.name or spec.id,
    version = type(spec.version) == "string" and spec.version or nil,
    onLogin = spec.onLogin, onLogout = spec.onLogout, onDefaults = spec.onDefaults,
  }
  ns.mods[#ns.mods + 1] = mod
  ns.modsById[mod.id] = mod
  if mod.addon then ns.modsByAddon[mod.addon] = mod end
  ns.Debug("mods: %s %s registered", mod.id, tostring(mod.version))
  return true
end

function R.Mods()
  local out = {}
  for i, m in ipairs(ns.mods) do out[i] = { id = m.id, addon = m.addon, name = m.name, version = m.version } end
  return out
end

-- Each registered mod's hook (onLogin, onLogout, onDefaults), each under its own pcall.
function ns.CallMods(hook)
  for _, m in ipairs(ns.mods) do
    if type(m[hook]) == "function" then ns.Run(m.id .. " " .. hook, m[hook]) end
  end
end

-- The journal -----------------------------------------------------------------------------------

R.Journal = {
  Add = function(eventType, fields, quiet) return ns.Journal.Add(eventType, fields, quiet) end,
  Session = function() return ns.Journal.session end,
  Seq = function() return ns.Journal.seq or 0 end,
  Count = function() return ns.Journal.Count() end,
}

-- Settings --------------------------------------------------------------------------------------

R.Settings = {
  COMBAT_LOG_MODES = ns.COMBAT_LOG_MODES,
  Get = function(key) return ns.Setting(key) end,
  Set = function(key, value)
    if not ns.db then return end
    ns.db.settings[key] = value
  end,
  All = function() return ns.db and ns.db.settings end,
  -- A mod's own boolean settings and their defaults, applied now (when RynaDB is set up) and at
  -- every reset.
  AddDefaults = function(defaults)
    if type(defaults) ~= "table" then return end
    for key, value in pairs(defaults) do
      if type(value) == "boolean" and ns.SETTING_DEFAULTS[key] == nil then ns.SETTING_DEFAULTS[key] = value end
    end
    if ns.db then ns.ApplySettingDefaults(ns.db.settings) end
  end,
}

-- Ordered lines from several sources: each source is fn() -> a list of lines, run under pcall.
function ns.OrderedLines(list)
  table.sort(list, function(a, b)
    if a.order ~= b.order then return a.order < b.order end
    return a.seq < b.seq
  end)
  local out = {}
  for _, source in ipairs(list) do
    local ok, lines = pcall(source.fn)
    if ok and type(lines) == "table" then
      for _, line in ipairs(lines) do out[#out + 1] = line end
    elseif not ok then
      ns.ReportError("status lines", lines)
    end
  end
  return out
end

local function adder(list)
  return function(order, fn)
    if type(fn) ~= "function" then return end
    list[#list + 1] = { order = tonumber(order) or 100, fn = fn, seq = #list + 1 }
  end
end

-- The panel -------------------------------------------------------------------------------------

R.Panel = {
  AddTab = function(spec) return ns.Panel.AddTab(spec) end,
  AddStatusLines = adder(ns.Panel.statusSources),
  Refresh = function() return ns.Panel.Refresh() end,
  RefreshIfShown = function() return ns.Panel.RefreshIfShown() end,
  Widgets = ns.Panel.Widgets,
}

-- /ryna -----------------------------------------------------------------------------------------

R.Slash = {
  Add = function(spec) return ns.SlashAdd(spec) end,
  AddStatusLines = adder(ns.slashStatusSources),
}
