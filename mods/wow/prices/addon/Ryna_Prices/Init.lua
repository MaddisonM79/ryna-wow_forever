-- Init: Prices registers with the core (Ryna.RegisterMod). Loaded last, so every handler exists. At
-- login it prunes the auction store (Auction.lua) and hooks the item tooltips (Tooltip.lua); the
-- settings decide, at each tooltip, what shows.

local _, ns = ...
if ns.off then return end

ns.API.RegisterMod({
  id = "prices",
  addon = ns.NAME,
  name = "Prices",
  version = ns.VERSION,
  onLogin = function()
    ns.Run("prices auction prune", ns.Auction.Prune)
    ns.Run("prices tooltips", ns.Tooltip.Register)
    ns.Debug("prices: %s", ns.Settings.Summary())
  end,
})
