-- Tooltip: the vendor line on item tooltips, "Sell Price: 13s 50c (2g 70s for 20)", with the
-- `priceSellLine` setting on. The price is C_Item.GetItemInfo's 11th value (sellPrice; the documented
-- 18 returns, itemStackCount the 8th), which the client has for every item it has loaded, away from
-- any vendor; the stock tooltip shows it only while a merchant window is open. Forever has no global
-- GetItemInfo (checked in game, 2026-11-08), so C_Item's is the only one asked. An item vendors won't
-- buy (0) gets no line, nor does one whose answer has no price.
--
-- The line reads like the game's own: the client's SELL_PRICE label ("Sell Price", translated by the
-- client in every language) and its coin icons (C_CurrencyInfo.GetCoinTextureString, or the older
-- GetCoinTextureString global), falling back to "Sells for" and the core's "13s 50c" where the client
-- has neither. Where the game already shows its own price on the tooltip (a money line on it: a
-- merchant window, mostly), no line is added, so there are never two prices.
--
-- The stack's total (`priceStackTotal`) is for how many the tooltip shows where the client says: the
-- getter that filled the tooltip and its arguments (a bag, bank or loot slot, a loot roll, quest
-- rewards, mail, trade, a merchant's or buyback item), else the slot button's count, never more than
-- a stack holds. A chat link, or anything else with no count, shows one item's price only
-- (Maddison, 2026-11-08).
--
-- An item the client hasn't loaded answers nothing at all (no values), and asking again doesn't load
-- it (checked in game): the tooltip gets no line then, the item is asked for
-- (C_Item.RequestLoadItemDataByID), and when ITEM_DATA_LOAD_RESULT (or GET_ITEM_INFO_RECEIVED)
-- names that item while the same tooltip still shows it, the tooltip is refreshed (RefreshData
-- where the client has it, else the line is added by hand). Those two events are registered only
-- while a tooltip waits (they fire for every item the client loads), on a frame of Prices' own.
-- Item ids come from `Hitem:(%d+)`: Forever's links start `|cnIQ2:`, not `|cffxxxxxx`.
--
-- With `priceOnlyAwayFromMerchant` on, no line at all while a merchant window is open.
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

-- Money as the client draws it (coin icons), else the core's "1g 20s 5c".
function T.Money(copper)
  local text = ns.Try(ns.Fn("C_CurrencyInfo", "GetCoinTextureString"), copper)
  if not ns.NonEmpty(text) then text = ns.Try(ns.Fn("GetCoinTextureString"), copper) end
  if ns.NonEmpty(text) then return text end
  return ns.MoneyText(copper)
end

-- "Sell Price: <money>" with the client's own label, else "Sells for <money>".
local function label(money)
  local stock = ns.NonEmpty(_G.SELL_PRICE)
  if stock then return string.format("%s: %s", stock, money) end
  return string.format(L["Sells for %s"], money)
end

-- The line for `count` of an item selling for `price` each, or nil when vendors pay nothing.
function T.Line(price, count)
  if type(price) ~= "number" or price <= 0 then return nil end
  local line = label(T.Money(price))
  if type(count) == "number" and count > 1 and S.On("priceStackTotal") then
    line = line .. " " .. string.format(L["(%s for %d)"], T.Money(price * count), count)
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

-- Whether the game already shows its own sell price on this tooltip: a money line drawn on it (the
-- stock UI's SetTooltipMoney counts them in shownMoneyFrames, cleared with the tooltip; the data's
-- lines are drawn before the post call runs). What is drawn, not the data's SellPrice line: the
-- stock UI may carry that line and not draw it away from a merchant.
function T.GameShowsPrice(tooltip)
  local shown = rawget(tooltip, "shownMoneyFrames")
  return ns.Usable(shown) and type(shown) == "number" and shown > 0
end

local function count(v)
  if ns.Usable(v) and type(v) == "number" and v > 0 then return v end
  return nil
end

-- The `n`th of a call's results, under ns.Try.
local function nth(n, f, ...)
  return (select(n, ns.Try(f, ...)))
end

-- How many of the item a bag slot holds.
function T.BagCount(bag, slot)
  local info = ns.Try(ns.Fn("C_Container", "GetContainerItemInfo"), bag, slot)
  if type(info) == "table" then return count(info.stackCount) end
  return count(nth(2, ns.Fn("GetContainerItemInfo"), bag, slot))
end

-- A merchant's (or the buyback list's) item: C_MerchantFrame's table where the client has it, else the
-- older function's 4th value (how many one purchase gives).
local function merchantCount(new, old, index)
  local info = ns.Try(ns.Fn("C_MerchantFrame", new), index)
  if type(info) == "table" then return count(info.stackCount) end
  return count(nth(4, ns.Fn(old), index))
end

-- Getter name -> how many it shows, from the getter's arguments. The names are what the tooltip's
-- setter is called with Get for Set (SetBagItem fills processingInfo.getterName = "GetBagItem").
T.COUNTS = {
  GetBagItem = function(a) return T.BagCount(a[1], a[2]) end,
  GetInventoryItem = function(a) return count(ns.Try(ns.Fn("GetInventoryItemCount"), a[1], a[2])) end,
  GetLootItem = function(a) return count(nth(3, ns.Fn("GetLootSlotInfo"), a[1])) end,
  GetLootRollItem = function(a) return count(nth(3, ns.Fn("GetLootRollItemInfo"), a[1])) end,
  GetQuestItem = function(a) return count(nth(3, ns.Fn("GetQuestItemInfo"), a[1], a[2])) end,
  GetQuestLogItem = function(a)
    local f = a[1] == "choice" and "GetQuestLogChoiceInfo" or "GetQuestLogRewardInfo"
    return count(nth(3, ns.Fn(f), a[2], a[3]))
  end,
  -- An attachment; a message's header item (no attachment index) has no count to read.
  GetInboxItem = function(a)
    if a[2] == nil then return nil end
    return count(nth(4, ns.Fn("GetInboxItem"), a[1], a[2]))
  end,
  GetSendMailItem = function(a) return count(nth(4, ns.Fn("GetSendMailItem"), a[1])) end,
  GetTradePlayerItem = function(a) return count(nth(3, ns.Fn("GetTradePlayerItemInfo"), a[1])) end,
  GetTradeTargetItem = function(a) return count(nth(3, ns.Fn("GetTradeTargetItemInfo"), a[1])) end,
  GetMerchantItem = function(a) return merchantCount("GetItemInfo", "GetMerchantItemInfo", a[1]) end,
  GetBuybackItem = function(a) return merchantCount("GetBuybackItemInfo", "GetBuybackItemInfo", a[1]) end,
}

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

-- How many the tooltip's item is, where the client says; nil when it doesn't (one item's price
-- then). `processing`: called from the processor's post call.
function T.SlotCount(tooltip, processing)
  local getter, args = getterInfo(tooltip, processing)
  if getter == "GetHyperlink" then
    -- A link (chat, ItemRefTooltip): no slot, and the owner's count would be some other button's.
    return nil
  end
  local read = getter and T.COUNTS[getter]
  if read then
    local n = ns.Try(read, args)
    if n then return n end
  end
  -- The button the tooltip belongs to: item buttons keep their count (SetItemButtonCount).
  local owner = type(tooltip.GetOwner) == "function" and ns.Try(tooltip.GetOwner, tooltip) or nil
  if type(owner) == "table" then return count(owner.count) end
  return nil
end

local function linkItemId(link)
  return tonumber(link:match("Hitem:(%d+)") or link:match("^item:(%d+)"))
end

-- The tooltip's item: its link and id. In the processor's post call the tooltip data (`data`) comes
-- first: when a tooltip appends a second item's data, GetItem still answers the primary one. The
-- tooltip's own GetItem fills in what the data lacks, and its link is used only when it is the same
-- item.
function T.ItemOf(tooltip, data)
  local link, id
  if type(data) == "table" then
    if ns.Usable(data.hyperlink) and type(data.hyperlink) == "string" and data.hyperlink ~= "" then
      link = data.hyperlink
    end
    if ns.Usable(data.id) and type(data.id) == "number" then id = data.id end
    if not id and link then id = linkItemId(link) end
  end
  if type(tooltip.GetItem) == "function" then
    local _, l, i = ns.Try(tooltip.GetItem, tooltip)
    if not ns.Usable(l) or type(l) ~= "string" or l == "" then l = nil end
    if not ns.Usable(i) or type(i) ~= "number" then i = l and linkItemId(l) or nil end
    if not id then id = i end
    if not link and l and i == id then link = l end
  end
  if not id and link then id = linkItemId(link) end
  return link, id
end

-- Whether the tooltip already has `line` (a refresh by hand, or a client running the hook twice).
function T.HasLine(tooltip, line)
  if type(tooltip.NumLines) ~= "function" or type(tooltip.GetName) ~= "function" then return false end
  local n, name = ns.Try(tooltip.NumLines, tooltip), ns.Try(tooltip.GetName, tooltip)
  if type(n) ~= "number" or type(name) ~= "string" then return false end
  local want = COLOR .. line .. "|r"
  for i = 1, n do
    local region = _G[name .. "TextLeft" .. i]
    local text = type(region) == "table" and type(region.GetText) == "function" and ns.Try(region.GetText, region)
    if text == want or text == line then return true end
  end
  return false
end

-- The load events, on a frame of Prices' own so they can be registered only while a tooltip waits.
-- Without CreateFrame (not a real client) they go through the core's dispatcher, always registered.
local LOAD_EVENTS = { "ITEM_DATA_LOAD_RESULT", "GET_ITEM_INFO_RECEIVED" }
local waiter = ns.Try(ns.Fn("CreateFrame"), "Frame")
if type(waiter) ~= "table" or type(waiter.RegisterEvent) ~= "function" then waiter = nil end
T.waiter = waiter

-- The tooltip waiting on an item: { tooltip, id }, or nil. Set through setPending only.
T.pending = nil

local function setPending(p)
  T.pending = p
  if not waiter then return end
  local method = p and waiter.RegisterEvent or waiter.UnregisterEvent
  if type(method) ~= "function" then return end
  for _, event in ipairs(LOAD_EVENTS) do
    -- Registering an event this client doesn't have raises an error; that event is just skipped.
    pcall(method, waiter, event)
  end
end

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
      setPending({ tooltip = tooltip, id = id })
      ns.Try(ns.Fn("C_Item", "RequestLoadItemDataByID"), id)
    end
    return
  end
  if T.pending and T.pending.tooltip == tooltip then setPending(nil) end
  if T.GameShowsPrice(tooltip) then return end
  local n = T.SlotCount(tooltip, type(data) == "table") or 1
  -- A frame's count can't be more than a stack holds: clamp it where the stack size is known.
  if maxStack > 1 and n > maxStack then n = maxStack end
  local line = T.Line(price, n)
  if not line then return end
  if show and T.HasLine(tooltip, line) then return end
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
  setPending(nil)
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
-- The core's dispatcher calls fn(event, ...); a frame's OnEvent gets (frame, event, ...).
local function onLoaded(_, itemID, ok)
  ns.Run("prices item loaded", T.Received, itemID, ok)
end
if waiter and type(waiter.SetScript) == "function" then
  waiter:SetScript("OnEvent", function(_, event, ...) onLoaded(event, ...) end)
else
  for _, event in ipairs(LOAD_EVENTS) do ns.On(event, onLoaded) end
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
