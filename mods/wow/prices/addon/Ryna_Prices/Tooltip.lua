-- Tooltip: the vendor line on item tooltips, "Sells for 13s 50c (2g 70s for 20)", with the
-- `priceSellLine` setting on. The price is C_Item.GetItemInfo's 11th value (sellPrice; the documented
-- 18 returns, itemStackCount the 8th), which the client has for every item it has loaded, away from
-- any vendor; the stock tooltip shows it only while a merchant window is open. Forever has no global
-- GetItemInfo (checked in game, 2026-11-08), so C_Item's is the only one asked. An item vendors won't
-- buy (0) gets no line, nor does one whose answer has no price.
--
-- The stack's total (`priceStackTotal`) is for how many the tooltip shows where the client says (a
-- bag, bank or loot slot: the tooltip's own getter and its arguments, else the slot button's count),
-- else for a full stack (itemStackCount) of a stackable item: a link in chat shows what 20
-- Linen Cloth sell for. A slot holding one of a stackable item shows the single price only.
--
-- An item the client hasn't loaded answers nothing at all (no values), and asking again doesn't load
-- it (checked in game): the tooltip gets no line then, the item is asked for
-- (C_Item.RequestLoadItemDataByID), and when ITEM_DATA_LOAD_RESULT (or GET_ITEM_INFO_RECEIVED)
-- names that item while the same tooltip still shows it, the tooltip is refreshed (RefreshData
-- where the client has it, else the line is added by hand). Items the player can hover are
-- normally loaded already, so the usual tooltip has no wait. Item ids come from `Hitem:(%d+)`:
-- Forever's links start `|cnIQ2:`, not `|cffxxxxxx`.
--
-- With `priceOnlyAwayFromMerchant` on, no line while a merchant window is open.
--
-- Hooked like Chronicle's lines: TooltipDataProcessor where the client has it, GameTooltip's and
-- ItemRefTooltip's OnTooltipSetItem otherwise. Every client call goes through ns.Try.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local T = {}
ns.Tooltip = T

local COLOR = "|cffffffff"

local function itemInfo(item)
  return ns.Try(ns.Fn("C_Item", "GetItemInfo"), item)
end

-- The item's sell price in copper (0: vendors won't buy it, or no price in the answer) and its stack
-- size, or nil while the client hasn't loaded it (an empty answer).
function T.Price(item)
  local name, _, _, _, _, _, _, maxStack, _, _, price = itemInfo(item)
  if not ns.Usable(name) then return nil end
  if not ns.Usable(price) or type(price) ~= "number" then price = 0 end
  if not ns.Usable(maxStack) or type(maxStack) ~= "number" then maxStack = 1 end
  return price, maxStack
end

-- The line for `count` of an item selling for `price` each, or nil when vendors pay nothing.
function T.Line(price, count)
  if type(price) ~= "number" or price <= 0 then return nil end
  local line = string.format(L["Sells for %s"], ns.MoneyText(price))
  if type(count) == "number" and count > 1 and S.On("priceStackTotal") then
    line = line .. " " .. string.format(L["(%s for %d)"], ns.MoneyText(price * count), count)
  end
  return line
end

-- Whether a merchant window is open: the merchant events, or the game's MerchantFrame on screen.
local atMerchant = false
ns.On("MERCHANT_SHOW", function() atMerchant = true end)
ns.On("MERCHANT_CLOSED", function() atMerchant = false end)

function T.AtMerchant()
  if atMerchant then return true end
  local f = _G.MerchantFrame
  if type(f) ~= "table" or type(f.IsShown) ~= "function" then return false end
  local ok, shown = pcall(f.IsShown, f)
  return ok and shown and true or false
end

local function count(v)
  if ns.Usable(v) and type(v) == "number" and v > 0 then return v end
  return nil
end

-- How many of the item a bag slot holds.
function T.BagCount(bag, slot)
  local info = ns.Try(ns.Fn("C_Container", "GetContainerItemInfo"), bag, slot)
  if type(info) == "table" then return count(info.stackCount) end
  local _, n = ns.Try(ns.Fn("GetContainerItemInfo"), bag, slot)
  return count(n)
end

-- The getter that filled the tooltip and its arguments ({ getterName = "GetBagItem", getterArgs =
-- { bag, slot } }), however this client keeps it. Inside the processor's post call (`processing`)
-- only the info being processed counts: the tooltip's kept info may still be the previous item's.
local function getterInfo(tooltip, processing)
  local info = tooltip.processingInfo
  if not processing then
    if type(info) ~= "table" and type(tooltip.GetPrimaryTooltipInfo) == "function" then
      info = ns.Try(tooltip.GetPrimaryTooltipInfo, tooltip)
    end
    if type(info) ~= "table" then info = tooltip.info end
  end
  if type(info) ~= "table" or type(info.getterName) ~= "string" or type(info.getterArgs) ~= "table" then
    return nil
  end
  return info.getterName, info.getterArgs
end

-- How many the tooltip's item is, where the client says; nil when it doesn't. `processing`: called
-- from the processor's post call.
function T.SlotCount(tooltip, processing)
  local getter, args = getterInfo(tooltip, processing)
  if getter == "GetBagItem" then
    local n = T.BagCount(args[1], args[2])
    if n then return n end
  elseif getter == "GetInventoryItem" then
    local n = count(ns.Try(ns.Fn("GetInventoryItemCount"), args[1], args[2]))
    if n then return n end
  elseif getter == "GetLootItem" then
    local _, _, n = ns.Try(ns.Fn("GetLootSlotInfo"), args[1])
    if count(n) then return n end
  end
  -- The button the tooltip belongs to: item buttons keep their count (SetItemButtonCount).
  local owner = type(tooltip.GetOwner) == "function" and ns.Try(tooltip.GetOwner, tooltip) or nil
  if type(owner) == "table" then return count(owner.count) end
  return nil
end

-- The tooltip's item: its link and id, from the tooltip, else the tooltip data.
function T.ItemOf(tooltip, data)
  local link, id
  if type(tooltip.GetItem) == "function" then
    local _, l, i = ns.Try(tooltip.GetItem, tooltip)
    if ns.Usable(l) and type(l) == "string" and l ~= "" then link = l end
    if ns.Usable(i) and type(i) == "number" then id = i end
  end
  if type(data) == "table" then
    if not link and ns.Usable(data.hyperlink) and type(data.hyperlink) == "string" and data.hyperlink ~= "" then
      link = data.hyperlink
    end
    if not id and ns.Usable(data.id) and type(data.id) == "number" then id = data.id end
  end
  if not id and link then id = tonumber(link:match("Hitem:(%d+)") or link:match("^item:(%d+)")) end
  return link, id
end

-- The tooltip waiting on GET_ITEM_INFO_RECEIVED: { tooltip, id }.
T.pending = nil

-- Adds the line to `tooltip`; `show` re-shows it (the script hooks, and a refresh by hand).
function T.OnItem(tooltip, data, show)
  if not S.On("priceSellLine") then return end
  if type(tooltip) ~= "table" or type(tooltip.AddLine) ~= "function" then return end
  if S.On("priceOnlyAwayFromMerchant") and T.AtMerchant() then return end
  local link, id = T.ItemOf(tooltip, data)
  if not link and not id then return end
  local price, maxStack = T.Price(link or id)
  if price == nil then
    if id then
      T.pending = { tooltip = tooltip, id = id }
      ns.Try(ns.Fn("C_Item", "RequestLoadItemDataByID"), id)
    end
    return
  end
  if T.pending and T.pending.tooltip == tooltip then T.pending = nil end
  local line = T.Line(price, T.SlotCount(tooltip, type(data) == "table") or maxStack)
  if not line then return end
  tooltip:AddLine(COLOR .. line .. "|r")
  if show and type(tooltip.Show) == "function" then tooltip:Show() end
end

local function shown(tooltip)
  if type(tooltip.IsShown) ~= "function" then return false end
  local ok, v = pcall(tooltip.IsShown, tooltip)
  return ok and v and true or false
end

-- The item the client just loaded (`ok` false: it couldn't): the tooltip that waited on it, if it
-- still shows that item.
function T.Received(itemID, ok)
  local p = T.pending
  if not p or tonumber(itemID) ~= p.id then return end
  T.pending = nil
  if ok == false then return end
  local tooltip = p.tooltip
  if not shown(tooltip) then return end
  local _, id = T.ItemOf(tooltip, nil)
  if id ~= p.id then return end
  if type(tooltip.RefreshData) == "function" and ns.Try(function()
    tooltip:RefreshData()
    return true
  end) then
    return
  end
  T.OnItem(tooltip, nil, true)
end

-- ITEM_DATA_LOAD_RESULT (itemID, success) answers RequestLoadItemDataByID; GET_ITEM_INFO_RECEIVED
-- (itemID, success) is the older event for the same.
for _, event in ipairs({ "ITEM_DATA_LOAD_RESULT", "GET_ITEM_INFO_RECEIVED" }) do
  ns.On(event, function(_, itemID, ok)
    ns.Run("prices item loaded", T.Received, itemID, ok)
  end)
end

local function onItem(tooltip, data, show)
  ns.Run("prices tooltip", T.OnItem, tooltip, data, show)
end

-- Once, at login. Returns how the hook went in: "processor", "scripts" or nil.
function T.Register()
  if T.mode then return T.mode end
  local enum = _G.Enum
  local types = type(enum) == "table" and enum.TooltipDataType
  local postCall = ns.Fn("TooltipDataProcessor", "AddTooltipPostCall")
  if postCall and type(types) == "table" and types.Item ~= nil then
    if ns.Try(function()
      postCall(types.Item, function(tooltip, data) onItem(tooltip, data, false) end)
      return true
    end) then
      T.mode = "processor"
    end
  end
  if not T.mode then
    local hooked = false
    for _, tip in ipairs({ _G.GameTooltip, _G.ItemRefTooltip }) do
      if type(tip) == "table" and type(tip.HookScript) == "function" and ns.Try(function()
        tip:HookScript("OnTooltipSetItem", function(self) onItem(self, nil, true) end)
        return true
      end) then
        hooked = true
      end
    end
    if hooked then T.mode = "scripts" end
  end
  ns.Debug("prices: tooltips: %s", T.mode or "no hook on this client")
  return T.mode
end
