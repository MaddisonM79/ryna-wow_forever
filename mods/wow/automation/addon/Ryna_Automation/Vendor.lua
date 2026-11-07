-- Vendor: at a merchant, sell every grey (poor-quality) item in the bags and repair, each its own
-- toggle (Settings.lua), both off by default. Shift held when the merchant opens, or combat, and
-- neither happens for that visit.
--
-- Selling is one item every 0.1 seconds (ten a second, under the twelve the brief allows), and each
-- slot is looked at again right before it's sold: still the same item, still grey, not locked, the
-- merchant still open. A slot that changed (another addon sold it, the player moved it) is skipped.
-- Selling stops when the merchant closes: with no merchant, the same call would use the item.
-- Repairing goes first when the gold is there; when it isn't, it's tried again once the greys are sold.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local V = {}
ns.Vendor = V

local INTERVAL = 0.1
-- Which visit this is: a sale or a repair check from an earlier visit stops when it no longer matches.
local visit, open = 0, false

local function merchantOpen(v)
  return open and v == visit and ns.Shown("MerchantFrame")
end

local function slotInfo(bag, slot)
  local info = ns.C("C_Container", "GetContainerItemInfo", bag, slot)
  return type(info) == "table" and info or nil
end

-- The bags: the backpack (0) and the equipped bags, plus the reagent bag where the client has one.
local function bagIds()
  local n = tonumber(_G.NUM_TOTAL_EQUIPPED_BAG_SLOTS) or tonumber(_G.NUM_BAG_SLOTS) or 4
  local out = {}
  for bag = 0, n do out[#out + 1] = bag end
  return out
end

local function sellPrice(itemID)
  local itemInfo = ns.Fn("C_Item", "GetItemInfo") or ns.Fn("GetItemInfo")
  local price = select(11, ns.Try(itemInfo, itemID))
  return type(price) == "number" and price or 0
end

-- Every grey with a vendor price: { bag, slot, itemID, count, price }.
function V.Greys()
  local out = {}
  for _, bag in ipairs(bagIds()) do
    for slot = 1, tonumber(ns.C("C_Container", "GetContainerNumSlots", bag)) or 0 do
      local info = slotInfo(bag, slot)
      if info and info.quality == 0 and not info.hasNoValue and not info.isLocked and type(info.itemID) == "number" then
        out[#out + 1] = { bag = bag, slot = slot, itemID = info.itemID, count = tonumber(info.stackCount) or 1,
          price = sellPrice(info.itemID) }
      end
    end
  end
  return out
end

-- Repairing ----------------------------------------------------------------------------------------

local function repairCost()
  if not ns.G("CanMerchantRepair") then return 0 end
  local cost = ns.G("GetRepairAllCost")
  return type(cost) == "number" and cost or 0
end

local function guildCovers(cost)
  if not (S.On("autoRepairGuild") and ns.G("IsInGuild") and ns.G("CanGuildBankRepair")) then return false end
  local allowed = ns.G("GetGuildBankWithdrawMoney")
  return type(allowed) == "number" and cost <= allowed
end

-- One repair attempt: the guild's money first (when that's on and it covers the cost), then the
-- player's own. `last`: say so when there isn't the gold (the attempt after the greys are sold).
local function repair(v, last)
  if not merchantOpen(v) then return true end
  local cost = repairCost()
  if cost <= 0 then return true end
  if guildCovers(cost) then
    ns.G("RepairAllItems", true)
    -- The guild bank can still say no (its own daily limit, its own gold): look again shortly, and
    -- pay from the player's gold when it did.
    ns.After(0.5, "automation repair", function()
      if not merchantOpen(v) then return end
      if repairCost() <= 0 then
        ns.Report(string.format(L["Repaired for %s from the guild bank."], ns.MoneyText(cost)))
      else
        V.RepairOwn(v, true)
      end
    end)
    return true
  end
  return V.RepairOwn(v, last)
end

function V.RepairOwn(v, last)
  if not merchantOpen(v) then return true end
  local cost = repairCost()
  if cost <= 0 then return true end
  local money = ns.G("GetMoney")
  if type(money) ~= "number" or money < cost then
    if last then ns.Report(string.format(L["Not enough gold to repair (%s)."], ns.MoneyText(cost))) end
    return false
  end
  ns.G("RepairAllItems", false)
  ns.Report(string.format(L["Repaired for %s."], ns.MoneyText(cost)))
  return true
end

-- Selling ------------------------------------------------------------------------------------------

local function finish(v, sold, copper, repairAfter)
  if sold > 0 then
    -- Not ns.Plural, which fills in the count alone.
    ns.Report(string.format(sold == 1 and L["Sold %d grey item for %s."] or L["Sold %d grey items for %s."],
      sold, ns.MoneyText(copper)))
  end
  if repairAfter then
    -- The money from the greys arrives a moment after the last sale.
    ns.After(0.5, "automation repair", function() repair(v, true) end)
  end
end

local function sell(v, list, i, sold, copper, repairAfter)
  if not merchantOpen(v) or ns.G("InCombatLockdown") then return finish(v, sold, copper, false) end
  local item = list[i]
  if not item then return finish(v, sold, copper, repairAfter) end
  local now = slotInfo(item.bag, item.slot)
  if now and now.itemID == item.itemID and now.quality == 0 and not now.hasNoValue and not now.isLocked then
    ns.C("C_Container", "UseContainerItem", item.bag, item.slot)
    sold = sold + 1
    copper = copper + item.price * (tonumber(now.stackCount) or item.count)
  end
  ns.After(INTERVAL, "automation sell", function() sell(v, list, i + 1, sold, copper, repairAfter) end)
end

ns.On("MERCHANT_SHOW", function()
  visit = visit + 1
  open = true
  local v = visit
  if not (S.On("autoSellGreys") or S.On("autoRepair")) then return end
  local held, why = ns.Held()
  if held then
    ns.Debug("automation: merchant: nothing this visit (%s)", why)
    return
  end
  -- A frame later, so the merchant frame is up.
  ns.After(0, "automation merchant", function()
    if not merchantOpen(v) then return end
    local repaired = not S.On("autoRepair") or repair(v, false)
    local greys = S.On("autoSellGreys") and V.Greys() or {}
    if #greys == 0 then
      if not repaired then repair(v, true) end
      return
    end
    sell(v, greys, 1, 0, 0, not repaired)
  end)
end)

ns.On("MERCHANT_CLOSED", function()
  open = false
  visit = visit + 1
end)
