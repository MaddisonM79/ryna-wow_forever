-- Vendor: at a merchant, sell every grey (poor-quality) item in the bags, sell the items the player
-- marked as junk, and repair, each its own toggle (Settings.lua), all off by default. Shift held when
-- the merchant opens, or combat, and none of it happens for that visit.
--
-- Selling is one item every 0.1 seconds (ten a second, under the twelve the brief allows), and each
-- slot is looked at again right before it's sold: still the same item, still grey or still marked,
-- not locked, the merchant still open. A slot that changed (another addon sold it, the player moved it)
-- is skipped. Selling stops when the merchant closes: with no merchant, the same call would use the
-- item. Repairing goes first when the gold is there; when it isn't, it's tried again once the selling
-- is done.
--
-- Marked junk: the mod's own list (RynaDB.automation.junk[itemID]), marked and unmarked with
-- /ryna auto junk (the item on the cursor, or an item ID) or the button on the Automation tab. This
-- client has no junk mark of its own that addons can set (C_Container.GetContainerItemInfo has no
-- isJunk; ElvUI works its "junk" out from the quality), so the list is the mod's; if a later client
-- does hand addons an isJunk flag, an item it marks is sold with the list. Never a quest item (the
-- bag's quest info, or the item's class), and never an item with no vendor price.

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

-- Marked junk ------------------------------------------------------------------------------------

local function junkList()
  local store = ns.Store()
  return store and store.junk or {}
end

local function questItem(bag, slot, itemID)
  local q = ns.C("C_Container", "GetContainerItemQuestInfo", bag, slot)
  if type(q) == "table" and (q.isQuestItem or (type(q.questID) == "number" and q.questID > 0)) then return true end
  local _, _, _, isQuest = ns.ItemInfo(itemID)
  return isQuest
end

-- Why an item can't be junk ("quest", "noprice"), or nil when it can.
function V.JunkRefusal(itemID)
  local _, _, price, isQuest = ns.ItemInfo(itemID)
  if isQuest then return "quest" end
  if price ~= nil and price <= 0 then return "noprice" end
  return nil
end

-- Marks or unmarks an item: true (now marked), false (unmarked), or nil and why not.
function V.ToggleJunk(itemID)
  local store = ns.Store()
  if not store then return nil, "nodb" end
  if store.junk[itemID] then
    store.junk[itemID] = nil
    return false
  end
  local why = V.JunkRefusal(itemID)
  if why then return nil, why end
  store.junk[itemID] = true
  return true
end

-- The marked item IDs, in order.
function V.JunkIDs()
  local ids = {}
  for id in pairs(junkList()) do
    if type(id) == "number" then ids[#ids + 1] = id end
  end
  table.sort(ids)
  return ids
end

-- What a slot is to sell: "grey", "junk" (marked by the player or the game), or nil.
local function kind(info, bag, slot)
  if not info or info.hasNoValue or info.isLocked or type(info.itemID) ~= "number" then return nil end
  if info.quality == 0 and S.On("autoSellGreys") then return "grey" end
  if S.On("autoSellJunk") and (junkList()[info.itemID] or info.isJunk == true)
    and not questItem(bag, slot, info.itemID) and sellPrice(info.itemID) > 0 then
    return "junk"
  end
  return nil
end

-- Everything to sell: { bag, slot, itemID, count, price, kind }.
function V.Sellable()
  local out = {}
  for _, bag in ipairs(bagIds()) do
    for slot = 1, tonumber(ns.C("C_Container", "GetContainerNumSlots", bag)) or 0 do
      local info = slotInfo(bag, slot)
      local k = kind(info, bag, slot)
      if k then
        out[#out + 1] = { bag = bag, slot = slot, itemID = info.itemID, count = tonumber(info.stackCount) or 1,
          price = sellPrice(info.itemID), kind = k }
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
  local greys, junk = sold.grey, sold.junk
  -- Not ns.Plural, which fills in the count alone.
  if greys > 0 and junk > 0 then
    ns.Report(string.format(L["Sold %d grey and %d marked junk items for %s."], greys, junk, ns.MoneyText(copper)))
  elseif greys > 0 then
    ns.Report(string.format(greys == 1 and L["Sold %d grey item for %s."] or L["Sold %d grey items for %s."],
      greys, ns.MoneyText(copper)))
  elseif junk > 0 then
    ns.Report(string.format(junk == 1 and L["Sold %d marked junk item for %s."]
      or L["Sold %d marked junk items for %s."], junk, ns.MoneyText(copper)))
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
  if now and now.itemID == item.itemID and kind(now, item.bag, item.slot) == item.kind then
    ns.C("C_Container", "UseContainerItem", item.bag, item.slot)
    sold[item.kind] = sold[item.kind] + 1
    copper = copper + item.price * (tonumber(now.stackCount) or item.count)
  end
  ns.After(INTERVAL, "automation sell", function() sell(v, list, i + 1, sold, copper, repairAfter) end)
end

ns.On("MERCHANT_SHOW", function()
  visit = visit + 1
  open = true
  local v = visit
  if not (S.On("autoSellGreys") or S.On("autoSellJunk") or S.On("autoRepair")) then return end
  local held, why = ns.Held()
  if held then
    ns.Debug("automation: merchant: nothing this visit (%s)", why)
    return
  end
  -- A frame later, so the merchant frame is up.
  ns.After(0, "automation merchant", function()
    if not merchantOpen(v) then return end
    local repaired = not S.On("autoRepair") or repair(v, false)
    local items = V.Sellable()
    if #items == 0 then
      if not repaired then repair(v, true) end
      return
    end
    sell(v, items, 1, { grey = 0, junk = 0 }, 0, not repaired)
  end)
end)

ns.On("MERCHANT_CLOSED", function()
  open = false
  visit = visit + 1
end)
