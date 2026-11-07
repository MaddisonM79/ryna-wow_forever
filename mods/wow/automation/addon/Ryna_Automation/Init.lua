-- Init: Automation registers with the core (Ryna.RegisterMod). Loaded last, so every handler exists.
-- It has nothing to do at login or logout: its handlers only act on the NPC events, and only for the
-- features that are ticked.

local _, ns = ...
if ns.off then return end

ns.API.RegisterMod({
  id = "automation",
  addon = ns.NAME,
  name = "Automation",
  version = ns.VERSION,
  onLogin = function()
    local on = ns.Settings.Summary()
    ns.Debug("automation: %s", on or "nothing on")
  end,
})
