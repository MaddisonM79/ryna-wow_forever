-- Merchants: the `merchant` journal event (docs/ryna-agent-cores-mods.md section 7.3), with the
-- `merchants` setting on (Recording tab, on by default). When the player opens a merchant, what that
-- NPC sells and for how much:
--
--   { npc = <GUID>, name, zone, subzone, filter, partial,
--     items = { { id | currencyId, price, stack, limited, currency = { { amount, name, itemId | currencyId } } } } }
--
-- `price` is copper for `stack` of the item (the bundle the merchant sells at once); `limited` is
-- whether the merchant has a limited supply (numAvailable >= 0); `currency` is there only when the
-- item also or only costs something other than copper (GetMerchantItemCostInfo / CostItem: tokens,
-- honor, items), else left out. A slot that sells a currency rather than an item (a currency link,
-- or C_MerchantFrame's currencyID) is `{ currencyId, price, stack, limited }`. `filter` is
-- GetMerchantFilter()'s value where the client has one: the merchant window lists only what the
-- filter shows (the game resets it to the player's class when the window opens), and
-- LE_LOOT_FILTER_ALL (1) is the whole list. At most 200 items.
--
-- When it's written. A visit's list is complete when every slot has an item or currency link; then
-- it's written at once (MERCHANT_SHOW), else as soon as it is complete (tried again on
-- GET_ITEM_INFO_RECEIVED, which is what the game's own merchant frame waits on, and on
-- MERCHANT_UPDATE), and whatever had loaded is written at MERCHANT_CLOSED with `partial = true`
-- rather than losing the visit. One event per visit and filter. The NPC is done for the login
-- (a /reload starts afresh) only once its complete, unfiltered list was written; a filtered or
-- partial one leaves it to the next visit.
--
-- So the journal doesn't grow by a list per visit, each complete list's fingerprint (a hash over its
-- items' ids, prices and stacks, and the filter) is kept in RynaDB.chronicle.merchants, keyed by the
-- NPC's GUID (GUID#filter for a filtered list): { hash, at }. A complete list whose fingerprint is
-- unchanged and was written in the last 7 days isn't written again; a changed price or list is.
-- Fingerprints older than 30 days go at login. Merchants are NPCs and items are items: nothing here
-- is about a person.
--
-- Forever's return orders differ from the wiki's in places (its trainer API did), so the item reads
-- prefer C_MerchantFrame.GetItemInfo's named fields where the client has it (asked at run time), and
-- from GetMerchantItemInfo take only the values classic and modern clients put in the same place
-- (price, stack and numAvailable, the 3rd to 5th). Forever has C_MerchantFrame and no global
-- GetMerchantItemInfo (probed in game, docs/forever-client-api.md), so the global is only the classic
-- fallback. Every client call goes through ns.Try.

local _, ns = ...
if ns.off then return end

local M = {}
ns.Merchants = M

M.MAX_ITEMS = 200
M.FRESH = 7 * 86400
M.KEEP = 30 * 86400

ns.API.Settings.AddDefaults({ merchants = true })

local NonEmpty = ns.NonEmpty

local function usable(v)
  return v ~= nil and not ns.IsSecret(v)
end

local function number(v)
  if usable(v) and type(v) == "number" then return v end
  return nil
end

-- The NPCs done for this login, by GUID.
M.seen = {}
-- This visit, from MERCHANT_SHOW to MERCHANT_CLOSED: { guid, name, wrote = { [filter] = true }, last, done }.
M.visit = nil

local function linkId(link, kind)
  if not usable(link) or type(link) ~= "string" then return nil end
  return tonumber(link:match("|H" .. kind .. ":(%d+)") or link:match("^" .. kind .. ":(%d+)"))
end

-- Price, stack, numAvailable and (C_MerchantFrame only) the currency it sells, for slot i.
local function slotInfo(i)
  local info = ns.Try(ns.Fn("C_MerchantFrame", "GetItemInfo"), i)
  if type(info) == "table" then
    return number(info.price), number(info.stackCount), number(info.numAvailable), number(info.currencyID)
  end
  local _, _, price, stack, available = ns.Try(ns.Fn("GetMerchantItemInfo"), i)
  return number(price), number(stack), number(available), nil
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

-- The merchant's items as listed now: the list, and whether every slot had loaded.
function M.Items()
  local n = number(ns.Try(ns.Fn("GetMerchantNumItems"))) or 0
  local items, complete = {}, n > 0
  for i = 1, math.min(n, M.MAX_ITEMS) do
    local link = ns.Try(ns.Fn("GetMerchantItemLink"), i)
    local id = linkId(link, "item")
    local price, stack, available, soldCurrency = slotInfo(i)
    local currencyId = not id and (linkId(link, "currency") or soldCurrency) or nil
    if id or currencyId then
      items[#items + 1] = {
        id = id, currencyId = currencyId, price = price or 0, stack = stack or 1,
        limited = (available or -1) >= 0, currency = currencies(i),
      }
    else
      complete = false
    end
  end
  return items, complete
end

-- The merchant filter (nil when the client has none), and whether it shows the whole list.
function M.Filter()
  return number(ns.Try(ns.Fn("GetMerchantFilter")))
end

local function isAll(filter)
  if filter == nil then return true end
  local all = _G.LE_LOOT_FILTER_ALL
  return filter == (type(all) == "number" and all or 1)
end

-- A cheap hash of a list: its items' ids, prices and stacks, sorted, and the filter.
function M.Fingerprint(items, filter)
  local parts = {}
  for _, it in ipairs(items) do
    parts[#parts + 1] = (it.id and ("i" .. it.id) or ("c" .. tostring(it.currencyId))) .. ":"
      .. tostring(it.price) .. ":" .. tostring(it.stack)
  end
  table.sort(parts)
  local s = tostring(filter) .. "|" .. table.concat(parts, ",")
  local h = 5381
  for i = 1, #s do h = (h * 33 + s:byte(i)) % 4294967296 end
  return string.format("%08x:%d", h, #items)
end

-- RynaDB.chronicle.merchants (nil before RynaDB is set up).
function M.Store()
  local db = ns.db
  if type(db) ~= "table" then return nil end
  if type(db.chronicle) ~= "table" then db.chronicle = {} end
  if type(db.chronicle.merchants) ~= "table" then db.chronicle.merchants = {} end
  return db.chronicle.merchants
end

-- Drops fingerprints older than 30 days (and malformed ones). Returns how many went.
function M.Prune(now)
  local store = M.Store()
  if not store then return 0 end
  now = now or ns.Now()
  local gone = 0
  for key, e in pairs(store) do
    if type(e) ~= "table" or type(e.at) ~= "number" or now - e.at > M.KEEP then
      store[key] = nil
      gone = gone + 1
    end
  end
  return gone
end

-- The NPC the merchant window is for: its GUID and name.
local function npc()
  local guid = ns.Try(ns.Fn("UnitGUID"), "npc")
  if not usable(guid) or type(guid) ~= "string" or guid == "" then return nil end
  local name = ns.Try(ns.Fn("UnitName"), "npc")
  return guid, usable(name) and NonEmpty(name) or nil
end

local function write(v, items, filter, partial)
  local zone = NonEmpty(ns.Try(ns.Fn("GetRealZoneText"))) or NonEmpty(ns.Try(ns.Fn("GetZoneText"))) or ns.rec.zone
  local subzone = NonEmpty(ns.Try(ns.Fn("GetSubZoneText")))
  return ns.Journal.Add("merchant", {
    npc = v.guid, name = v.name, zone = usable(zone) and zone or nil, subzone = usable(subzone) and subzone or nil,
    filter = filter, partial = partial or nil, items = items,
  })
end

-- Writes this visit's list once it's complete; `final` (MERCHANT_CLOSED) writes what had loaded,
-- as partial, when it never was. Returns the event, or nil.
function M.Record(final)
  local v = M.visit
  if not v or v.done or not ns.rec or not ns.Setting("merchants") then return nil end
  local filter = M.Filter()
  local items, complete = M.Items()
  if #items > 0 then
    v.last = { filter = filter, items = items, complete = complete }
  elseif final and v.last then
    -- The window has closed and the client may list nothing now: the last reading.
    filter, items, complete = v.last.filter, v.last.items, v.last.complete
  end
  if #items == 0 or (not complete and not final) then return nil end
  local key = tostring(filter)
  if v.wrote[key] then return nil end
  v.wrote[key] = true
  if not complete then return write(v, items, filter, true) end

  local all = isAll(filter)
  if all then
    v.done = true
    M.seen[v.guid] = true
  end
  local store = M.Store()
  local storeKey = all and v.guid or (v.guid .. "#" .. key)
  local hash, now = M.Fingerprint(items, filter), ns.Now()
  local prev = store and store[storeKey]
  if type(prev) == "table" and prev.hash == hash and type(prev.at) == "number" and now - prev.at < M.FRESH then
    return nil
  end
  if store then store[storeKey] = { hash = hash, at = now } end
  return write(v, items, filter, false)
end

ns.On("MERCHANT_SHOW", function()
  M.visit = nil
  if not ns.rec or not ns.Setting("merchants") then return end
  local guid, name = npc()
  if not guid or M.seen[guid] then return end
  M.visit = { guid = guid, name = name, wrote = {} }
  M.Record(false)
end)

-- Late item data, and the list changing (a filter picked, an item bought out): try again.
for _, event in ipairs({ "GET_ITEM_INFO_RECEIVED", "MERCHANT_UPDATE", "MERCHANT_FILTER_ITEM_UPDATE" }) do
  ns.On(event, function()
    local v = M.visit
    if v and not v.done then M.Record(false) end
  end)
end

ns.On("MERCHANT_CLOSED", function()
  local v = M.visit
  if v and not v.done then M.Record(true) end
  M.visit = nil
end)
