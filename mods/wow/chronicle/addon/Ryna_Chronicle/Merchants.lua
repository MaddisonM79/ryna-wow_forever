-- Merchants: the `merchant` journal event (docs/ryna-agent-cores-mods.md section 7.3), with the
-- `merchants` setting on (Recording tab, on by default). When the player opens a merchant, what that
-- NPC sells and for how much:
--
--   { npc = <GUID>, name, zone, subzone,
--     items = { { id, price, stack, limited, currency = { { amount, name, itemId | currencyId } } } } }
--
-- `price` is copper for `stack` of the item (the bundle the merchant sells at once); `limited` is
-- whether the merchant has a limited supply (numAvailable >= 0); `currency` is there only when the
-- item also or only costs something other than copper (GetMerchantItemCostInfo / CostItem: tokens,
-- honor, items), else left out. Items whose link the client hasn't loaded yet have no id and are
-- left out. At most one event per NPC per login (a /reload starts afresh) and at most 200 items.
--
-- Recorded at MERCHANT_SHOW when every item's link is there; otherwise at the first MERCHANT_UPDATE
-- of that visit that finds any, with what has loaded by then (once written, the visit is done). A
-- visit that records nothing (no GUID for the NPC, nothing listed yet) leaves the NPC to the next
-- visit. Merchants are NPCs and items are items: nothing here is about a person.
--
-- Forever's return orders differ from the wiki's in places (its trainer API did), so the item reads
-- prefer C_MerchantFrame.GetItemInfo's named fields where the client has it, and from
-- GetMerchantItemInfo take only the values classic and modern clients put in the same place (price,
-- stack and numAvailable, the 3rd to 5th). Every client call goes through ns.Try.

local _, ns = ...
if ns.off then return end

local M = {}
ns.Merchants = M

M.MAX_ITEMS = 200

ns.API.Settings.AddDefaults({ merchants = true })

local NonEmpty = ns.NonEmpty

local function usable(v)
  return v ~= nil and not ns.IsSecret(v)
end

local function number(v)
  if usable(v) and type(v) == "number" then return v end
  return nil
end

-- The NPCs recorded this login, by GUID.
M.seen = {}
-- This visit: { guid, done } from MERCHANT_SHOW to MERCHANT_CLOSED.
M.visit = nil

local function linkId(link, kind)
  if not usable(link) or type(link) ~= "string" then return nil end
  return tonumber(link:match("|H" .. kind .. ":(%d+)") or link:match("^" .. kind .. ":(%d+)"))
end

-- Price, stack and numAvailable for slot i.
local function slotInfo(i)
  local info = ns.Try(ns.Fn("C_MerchantFrame", "GetItemInfo"), i)
  if type(info) == "table" then
    return number(info.price), number(info.stackCount), number(info.numAvailable)
  end
  local _, _, price, stack, available = ns.Try(ns.Fn("GetMerchantItemInfo"), i)
  return number(price), number(stack), number(available)
end

-- What slot i costs besides copper, or nil.
local function currencies(i)
  local n = number(ns.Try(ns.Fn("GetMerchantItemCostInfo"), i))
  if not n or n <= 0 then return nil end
  local costItem = ns.Fn("GetMerchantItemCostItem")
  local out = {}
  for k = 1, math.min(n, 5) do
    local _, amount, link, name = ns.Try(costItem, i, k)
    local c = { amount = number(amount), name = usable(name) and NonEmpty(name) or nil,
      itemId = linkId(link, "item"), currencyId = linkId(link, "currency") }
    if c.amount or c.name or c.itemId or c.currencyId then out[#out + 1] = c end
  end
  if #out == 0 then return nil end
  return out
end

-- The merchant's items: the list, and whether every slot's link had loaded.
function M.Items()
  local n = number(ns.Try(ns.Fn("GetMerchantNumItems"))) or 0
  local items, complete = {}, n > 0
  for i = 1, math.min(n, M.MAX_ITEMS) do
    local id = linkId(ns.Try(ns.Fn("GetMerchantItemLink"), i), "item")
    if id then
      local price, stack, available = slotInfo(i)
      items[#items + 1] = {
        id = id, price = price or 0, stack = stack or 1, limited = (available or -1) >= 0,
        currency = currencies(i),
      }
    else
      complete = false
    end
  end
  return items, complete
end

-- The NPC the merchant window is for: its GUID and name.
local function npc()
  local guid = ns.Try(ns.Fn("UnitGUID"), "npc")
  if not usable(guid) or type(guid) ~= "string" or guid == "" then return nil end
  local name = ns.Try(ns.Fn("UnitName"), "npc")
  return guid, usable(name) and NonEmpty(name) or nil
end

-- Writes the event for this visit if it hasn't been; `force` writes what has loaded even when some
-- links haven't (MERCHANT_UPDATE). Returns the event, or nil.
function M.Record(force)
  local v = M.visit
  if not v or v.done or not ns.rec or not ns.Setting("merchants") then return nil end
  if M.seen[v.guid] then
    v.done = true
    return nil
  end
  local items, complete = M.Items()
  if #items == 0 or (not complete and not force) then return nil end
  v.done = true
  M.seen[v.guid] = true
  local zone = NonEmpty(ns.Try(ns.Fn("GetRealZoneText"))) or NonEmpty(ns.Try(ns.Fn("GetZoneText"))) or ns.rec.zone
  local subzone = NonEmpty(ns.Try(ns.Fn("GetSubZoneText")))
  return ns.Journal.Add("merchant", {
    npc = v.guid, name = v.name, zone = usable(zone) and zone or nil, subzone = usable(subzone) and subzone or nil,
    items = items,
  })
end

ns.On("MERCHANT_SHOW", function()
  M.visit = nil
  if not ns.rec or not ns.Setting("merchants") then return end
  local guid, name = npc()
  if not guid or M.seen[guid] then return end
  M.visit = { guid = guid, name = name }
  M.Record(false)
end)

ns.On("MERCHANT_UPDATE", function()
  local v = M.visit
  if v and not v.done then M.Record(true) end
end)

ns.On("MERCHANT_CLOSED", function()
  M.visit = nil
end)
