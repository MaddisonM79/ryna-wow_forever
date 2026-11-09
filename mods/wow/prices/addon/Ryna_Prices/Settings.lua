-- Settings: Prices' toggles, each its own key in RynaDB.settings (the core keeps one flat table for
-- the core and every mod, so these all start with "price"). The mod itself is off until it's turned
-- on in the companion app; once it's on, both lines show and the snapshot is taken.
--
--   priceSellLine              the "Sell Price: ..." line on item tooltips (on); it stays away by itself
--                              wherever the game draws its own price
--   priceStackTotal            the stack's total after it, "(2g 70s for 20)" (on)
--   priceOnlyAwayFromMerchant  no sell line at all while a merchant window is open (off)
--   priceAuctionLine           the "AH: 4g 20s each · 31 listed · 2 hours ago" line (on), from this
--                              account's own snapshots and searches; never where the auction house
--                              window shows the item's price itself
--   priceAuctionStackTotal     the stack's total in it, and in everyone's line (on)
--   priceAuctionSnapshot       one snapshot of the house per visit, at most one per 15 minutes per
--                              character, and the `auction` journal event it makes (on; off: no
--                              ReplicateItems() and no event)
--   priceVendorLine            "Sold by Kiro, Durotar: 20c (+2 more)", which vendor sells the item,
--                              from everyone's visits (the app's Data/vendors.lua; on)
--   pricePooledAuctionLine     "AH (everyone): 1s 10c each · 3 listed · as of 2 hours ago", everyone's
--                              auction numbers (the app's Data/auctions.lua; on), apart from this
--                              account's own line and in another colour
--
-- Each has the short name `/ryna prices <name> on|off` takes, and the one it sits under (`under`).

local _, ns = ...
if ns.off then return end
local L = ns.L

local S = {}
ns.Settings = S

S.FEATURES = {
  { name = "sellLine", key = "priceSellLine", default = true, label = L["Show what a vendor pays on item tooltips"] },
  { name = "stackTotal", key = "priceStackTotal", default = true, under = "priceSellLine",
    label = L["Add what the whole stack sells for"] },
  { name = "awayFromMerchant", key = "priceOnlyAwayFromMerchant", default = false, under = "priceSellLine",
    label = L["Not while a merchant window is open"] },
  { name = "vendorLine", key = "priceVendorLine", default = true,
    label = L["Show which vendor sells it, from everyone's visits"] },
  { name = "auctionLine", key = "priceAuctionLine", default = true,
    label = L["Show auction house prices on item tooltips"] },
  { name = "auctionStackTotal", key = "priceAuctionStackTotal", default = true, under = "priceAuctionLine",
    label = L["Add what the whole stack goes for"] },
  { name = "pooledAuctionLine", key = "pricePooledAuctionLine", default = true,
    label = L["Show everyone's auction house prices on item tooltips"] },
  { name = "auctionSnapshot", key = "priceAuctionSnapshot", default = true,
    label = L["Take a snapshot of the auction house when you open it"] },
}

local defaults = {}
for _, f in ipairs(S.FEATURES) do defaults[f.key] = f.default end
ns.API.Settings.AddDefaults(defaults)

-- Whether a feature is on: its own toggle, and the one it sits under (if any).
function S.On(key)
  if not ns.Setting(key) then return false end
  for _, f in ipairs(S.FEATURES) do
    if f.key == key and f.under then return ns.Setting(f.under) and true or false end
  end
  return true
end

-- The feature `/ryna prices <word>` names: its short name or its setting, in any case.
function S.Find(word)
  word = tostring(word or ""):lower()
  for _, f in ipairs(S.FEATURES) do
    if f.name:lower() == word or f.key:lower() == word then return f end
  end
  return nil
end

-- "sell line on (with the stack's total, everywhere)", or "sell line off".
function S.Summary()
  if not S.On("priceSellLine") then return L["sell line off"] end
  local stack = S.On("priceStackTotal")
  local away = S.On("priceOnlyAwayFromMerchant")
  if stack and away then return L["sell line on (with the stack's total, away from merchants)"] end
  if stack then return L["sell line on (with the stack's total, everywhere)"] end
  if away then return L["sell line on (no stack total, away from merchants)"] end
  return L["sell line on (no stack total, everywhere)"]
end

-- "auction line on, snapshots on", and the like.
function S.AuctionSummary()
  local line, snap = S.On("priceAuctionLine"), S.On("priceAuctionSnapshot")
  if line and snap then return L["auction line on, snapshots on"] end
  if line then return L["auction line on, snapshots off"] end
  if snap then return L["auction line off, snapshots on"] end
  return L["auction line off, snapshots off"]
end
