-- Init: the login sequence. Loaded after every core module, before any mod (a mod depends on this
-- addon, so the client loads it after), and a mod's own login work is its onLogin (Mods.lua).
--
-- PLAYER_LOGIN sets up RynaDB, prunes the journal and starts this session's journal, then runs each
-- mod's onLogin in the order they registered (Chronicle builds the character record and writes the
-- session's `login` event there). The heavier scans are the mods' own, deferred past the first
-- PLAYER_ENTERING_WORLD.

local addonName, ns = ...
local L = ns.L

ns.On("ADDON_LOADED", function(_, name)
  if name == addonName then ns.InitDB() end
end)

ns.On("PLAYER_LOGIN", function()
  if not ns.db then ns.InitDB() end
  ns.Journal.Prune()
  ns.Journal.StartSession(UnitGUID("player"))
  ns.CallMods("onLogin")
  if ns.Setting("chatNotices") then
    ns.PrintRaw(string.format("|cff7fb2ffRyna %s|r: %s", ns.VERSION, L["recording. /ryna for settings."]))
  end
  ns.Debug("login: %d mods (%s)", #ns.mods, ns.ListText((function()
    local ids = {}
    for i, m in ipairs(ns.mods) do ids[i] = m.id end
    return ids
  end)()))
  if ns.Setting("chatNotices") and ns.Status.IsStale() then
    ns.Print(L["the companion app hasn't synced recently."])
  end
  ns.Debug("companion app file: %s", ns.Status.DebugLine())
  ns.Run("advanced combat logging", ns.CombatLog.ApplyAdvancedLogging)
end)

-- The UI is a second PLAYER_LOGIN handler, after the one above, with each part under its own
-- pcall: a client without the Settings API or a minimap still records, and an error above doesn't
-- take the panel with it.
ns.On("PLAYER_LOGIN", function()
  if not ns.db then ns.InitDB() end
  ns.Run("panel", ns.Panel.Register)
  ns.Run("minimap", ns.Minimap.Create)
  -- A frame, never chat, and only when the API said this version is no longer supported.
  ns.Run("version notice", ns.Notice.ShowAtLogin)
end)

ns.On("PLAYER_LOGOUT", function()
  ns.CallMods("onLogout")
end)

-- When the client blocks or forbids something this addon (or one of its mods) did (the "blocked
-- from an action only available to the Blizzard UI" dialog), say which call it was, in chat and in
-- RynaDB.blocked, so it can be found without a taint log.
local function noteBlocked(event, addon, fn)
  local what = tostring(fn)
  if addon == ns.NAME or ns.modsByAddon[addon] then
    ns.db.blocked = { event = event, fn = what, at = ns.Now(), addon = ns.VERSION }
  end
  -- Printed for every addon, not only this one: the client may blame a different name than the folder's.
  ns.PrintRaw(string.format("|cffff6060Ryna|r: the client %s a call to %s by %s (Ryna %s). Please report this line.",
    event == "ADDON_ACTION_FORBIDDEN" and "forbade" or "blocked", what, tostring(addon), ns.VERSION))
end
ns.On("ADDON_ACTION_FORBIDDEN", noteBlocked)
ns.On("ADDON_ACTION_BLOCKED", noteBlocked)
