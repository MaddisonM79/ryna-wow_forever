-- Init: Automation registers with the core (Ryna.RegisterMod). Loaded last, so every handler exists.
-- At login it turns the game's auto-loot on when that's ticked; everything else acts on its events,
-- and only for the features that are ticked.

local _, ns = ...
if ns.off then return end

ns.API.RegisterMod({
  id = "automation",
  addon = ns.NAME,
  name = "Automation",
  version = ns.VERSION,
  onLogin = function()
    if ns.Settings.On("autoLootOn") then ns.Loot.SetAutoLoot(true) end
    local on = ns.Settings.Summary()
    ns.Debug("automation: %s", on or "nothing on")
  end,
})
