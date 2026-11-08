-- Mod: Prices' namespace, built from the core's mod API (the `Ryna` table, the core addon's Mods.lua).
-- Prices puts prices on item tooltips (docs/ryna-agent-cores-mods.md section 7.3). Stage 1 is the
-- vendor line: what a vendor pays for the item and for its stack, from GetItemInfo's sell price,
-- which the client holds for every item it has seen, away from any vendor (Tooltip.lua). Step 2 is
-- the auction line: one snapshot of the auction house per visit (at most one per 15 minutes),
-- summarised per item into RynaDB.prices and sent up as `auction` journal events, never anything
-- about a seller (Auction.lua), and "AH: 4g 20s each · 31 listed · 2 hours ago" on the tooltip
-- (Tooltip.lua). Its settings are toggles on its own Prices tab (Settings.lua, Tab.lua); /ryna
-- prices says how they stand (Status.lua). It writes its settings and RynaDB.prices only, and it
-- never prints to chat unasked.
--
-- Every file starts with `if ns.off then return end`: without the mod API (a core addon from before
-- it), Prices loads nothing at all, and says nothing either (the app installs it with a core that
-- has the API, so there is nobody to tell).

local addonName, ns = ...

local R = _G.Ryna
if type(R) ~= "table" or type(R.API) ~= "number" or R.API < 1 or type(R.RegisterMod) ~= "function" then
  ns.off = true
  return
end

ns.NAME = addonName
ns.API = R

-- The version is the TOC's, so there is no second copy to keep in step.
local metadata = (_G.C_AddOns and _G.C_AddOns.GetAddOnMetadata) or _G.GetAddOnMetadata
ns.VERSION = R.Try(metadata, addonName, "Version")

for _, name in ipairs({ "Try", "Fn", "IsSecret", "NonEmpty", "Now", "Locale", "Print", "Debug", "Run", "After",
  "ReportError", "MoneyText", "Ago", "On" }) do
  ns[name] = R[name]
end

ns.Setting = R.Settings.Get

-- Prices' strings (Locales/), falling back to the core's (Ryna.L), which fall back to the key.
ns.L = setmetatable({}, {
  __index = function(_, key) return R.L[key] end,
  __newindex = function(t, key, value)
    if type(value) == "string" and value ~= "" then rawset(t, key, value) end
  end,
})

-- A value the client handed over that can be read: present and not one of the engine's secrets.
function ns.Usable(v)
  return v ~= nil and not ns.IsSecret(v)
end
