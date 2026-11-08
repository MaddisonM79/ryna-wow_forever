-- Auction: the auction side of Prices (docs/ryna-agent-cores-mods.md 7.3 step 2; the API's
-- docs/wow.md section 17). What the client hands an addon here was checked in game on 2026-11-08
-- (docs/forever-client-api.md, Auction house): Forever has C_AuctionHouse only, and
-- C_AuctionHouse.ReplicateItems() gives the whole house in one pull (192,300 rows on Stormwind).
--
-- The snapshot (`priceAuctionSnapshot`, on). At AUCTION_HOUSE_SHOW, when this character's last
-- request is 15 minutes old or more (RynaDB.prices.auctionAt[<player GUID>], the throttle ATT
-- keeps too), ReplicateItems() is asked once. At REPLICATE_ITEM_LIST_UPDATE the rows
-- (GetReplicateItemInfo(i), i from 0 to GetNumReplicateItems() - 1) are read 2,000 a frame, never
-- in one blocking loop. Each row gives its count, buyout, item id and hasAllInfo (the 3rd, 10th,
-- 17th and 18th values, Blizzard's AuctionHouseDocumentation.lua); the owner and bidder values
-- (12th to 15th, nil on Forever anyway) are never even put in a variable. A row with no buyout is
-- skipped and counted; one with hasAllInfo false (its name and link not cached yet, its numbers
-- there) is counted and kept, so a cold item cache doesn't make a partial snapshot. The rest are
-- kept per item id as one number per
-- listing, unit buyout * 65536 + count (the unit buyout is buyoutPrice / count rounded to copper),
-- so 192k listings are about 3 MB of numbers and no tables of their own; sorting those numbers
-- sorts by unit price. Once every row is read, each item's numbers are summarised, again a slice
-- a frame, and its list dropped: `n` listings, `q` total quantity, `min`, and `q25` and `med` as
-- quantity-weighted percentiles (the unit price at which the cheapest 25% and 50% of the quantity
-- is reached, so min <= q25 <= med always). Then, in one go: the local store, the size guard, and
-- the `auction` journal events. The house closing or the player logging out while rows are being
-- read stops the walk and nothing is written (closing it also forgets the throttle time, so the next
-- visit asks again); once the rows are all read, the house closing no longer matters (the numbers
-- are already in hand) but logging out still stops it.
--
-- The local store, RynaDB.prices.auction[itemId] = { min, q25, med, n, q, at } (integers, copper;
-- one table for the account): replaced per item by each snapshot; entries older than 14 days are
-- dropped at login, and when it holds more than 20,000 items the 20,000 seen most recently are kept.
-- A search on the browse tab (AUCTION_HOUSE_BROWSE_RESULTS_UPDATED or _ADDED) folds the results in:
-- an item without a snapshot entry gets { min = minPrice, q = totalQuantity, at } (no median: the
-- browse list doesn't have one); one with a snapshot entry keeps it and takes only the new lowest
-- price and time (nothing within 15 minutes of the snapshot). Only results that are the item itself
-- count: no random suffix, and gear only at its own item level (A.BaseItem). Browse results are
-- never sent anywhere.
--
-- The `auction` event (docs/wow.md 17): { at, rows, part, parts, items = { { id, n, q, min, q25, med } } },
-- every value an integer, at most 2,000 items an event, split into parts with the same `at`, all
-- added in one frame so they are in the same save. The core keeps them only until the companion app
-- has uploaded them, three days at most (Journal.lua's SHORT_LIVED), not the journal's 30; a core
-- without that gets none (A.CanSend). And the journal never holds more than 6,000 auction items or
-- more than 3 earlier snapshots beside a new one, oldest dropped first (A.MakeRoom), so it stays
-- well inside the agent's 16 MB upload whatever happens between logins: a snapshot of a bigger
-- house sends its 6,000 most listed items. An item outside the API's limits (an id, count or price
-- it would refuse, or n > q) is left out of the event, so the event is never refused whole; it stays
-- in the local store. There is no field for a seller, and no seller is read.
--
-- Nothing here prints to chat. The tooltip line that reads the store is Tooltip.lua's.

local _, ns = ...
if ns.off then return end
local S = ns.Settings

local A = {}
ns.Auction = A

A.THROTTLE = 15 * 60 -- seconds between two ReplicateItems() for one character
A.ROWS_PER_FRAME = 2000 -- rows read, or listings summarised, per frame
A.KEEP = 14 * 86400 -- the local store's entries, at login
A.MAX_ITEMS = 20000 -- the local store's size
A.PART_ITEMS = 2000 -- items per event (docs/wow.md 17)
A.MAX_PARTS = 20 -- events per snapshot (docs/wow.md 17)
A.JOURNAL_ITEMS = 6000 -- auction items the journal may hold in all, across snapshots
A.JOURNAL_SNAPSHOTS = 3 -- earlier snapshots kept in the journal beside a new one
A.FALLBACK = 60 -- seconds to wait before reading rows on a client without REPLICATE_ITEM_LIST_UPDATE

-- The API's limits at ingest (docs/wow.md 17): an event breaking one is skipped whole.
local MAX_ID, MAX_N, MAX_Q, MAX_PRICE, MAX_ROWS = 10000000, 100000, 100000000, 100000000, 2000000

-- One listing as one number: unit * 65536 + count. Both are clamped so the number stays an exact
-- integer in a double (2^37 * 2^16 = 2^53); a unit price that high is far past what is ever sent.
local COUNT_SPAN = 65536
local MAX_UNIT = 2 ^ 37 - 1

local floor = math.floor

function A.Pack(unit, count)
  unit = floor(unit)
  if unit < 1 then unit = 1 elseif unit > MAX_UNIT then unit = MAX_UNIT end
  count = floor(count)
  if count < 1 then count = 1 elseif count > COUNT_SPAN - 1 then count = COUNT_SPAN - 1 end
  return unit * COUNT_SPAN + count
end

-- The numbers for one item's listings (packed, as above; sorted in place): { n, q, min, q25, med }.
function A.Stats(list)
  local n = #list
  if n == 0 then return nil end
  table.sort(list)
  local q = 0
  for k = 1, n do q = q + list[k] % COUNT_SPAN end
  local stats = { n = n, q = q, min = floor(list[1] / COUNT_SPAN) }
  local cum = 0
  for k = 1, n do
    local v = list[k]
    cum = cum + v % COUNT_SPAN
    if not stats.q25 and cum * 4 >= q then stats.q25 = floor(v / COUNT_SPAN) end
    if cum * 2 >= q then
      stats.med = floor(v / COUNT_SPAN)
      break
    end
  end
  return stats
end

-- Prices' saved data (RynaDB.prices), made on first use; nil before RynaDB.
function A.Store()
  local db = ns.API.DB()
  if type(db) ~= "table" then return nil end
  local p = db.prices
  if type(p) ~= "table" then
    p = {}
    db.prices = p
  end
  if type(p.auction) ~= "table" then p.auction = {} end
  if type(p.auctionAt) ~= "table" then p.auctionAt = {} end
  return p
end

-- The store's entry for an item, or nil.
function A.Entry(id)
  local p = A.Store()
  local e = p and type(id) == "number" and p.auction[id]
  if type(e) ~= "table" then return nil end
  return e
end

local function isNumber(v)
  return type(v) == "number" and not ns.IsSecret(v)
end

-- The character the throttle is kept for: the player's GUID.
local function charKey()
  local guid = ns.Try(ns.Fn("UnitGUID"), "player")
  if ns.Usable(guid) and type(guid) == "string" and guid ~= "" then return guid end
  return "?"
end

-- Keeps the store to A.MAX_ITEMS entries, the most recently seen.
function A.Guard(p)
  p = p or A.Store()
  if not p then return end
  local count = 0
  for _ in pairs(p.auction) do count = count + 1 end
  if count <= A.MAX_ITEMS then return end
  local list = {}
  for id, e in pairs(p.auction) do list[#list + 1] = { id = id, at = type(e) == "table" and e.at or 0 } end
  table.sort(list, function(a, b)
    if a.at ~= b.at then return a.at > b.at end
    return a.id < b.id
  end)
  for k = A.MAX_ITEMS + 1, #list do p.auction[list[k].id] = nil end
end

-- At login: entries older than 14 days (and anything malformed) are dropped, then the size guard.
function A.Prune()
  local p = A.Store()
  if not p then return end
  local cutoff = ns.Now() - A.KEEP
  for id, e in pairs(p.auction) do
    if type(id) ~= "number" or type(e) ~= "table" or type(e.at) ~= "number" or e.at < cutoff
      or type(e.min) ~= "number" or type(e.q) ~= "number" then
      p.auction[id] = nil
    end
  end
  for key, at in pairs(p.auctionAt) do
    if type(at) ~= "number" or at < cutoff then p.auctionAt[key] = nil end
  end
  A.Guard(p)
end

-- The house -----------------------------------------------------------------------------------

A.open = false
A.requested = nil -- when this session asked ReplicateItems(), until its rows are read
A.walk = nil -- the walk under way: { at, total, i, lists, ids, ... }
A.browsing = {} -- item ids the browse tab lists now (their price is on screen)

function A.OnShow()
  A.open = true
  if not S.On("priceAuctionSnapshot") or A.walk or A.requested then return end
  local p = A.Store()
  if not p then return end
  local now, key = ns.Now(), charKey()
  local last = p.auctionAt[key]
  -- A last time in the future (the clock went back) doesn't hold the next one off.
  if type(last) == "number" and now >= last and now - last < A.THROTTLE then return end
  local replicate = ns.Fn("C_AuctionHouse", "ReplicateItems")
  if not replicate or not ns.Try(function()
    replicate()
    return true
  end) then
    return
  end
  p.auctionAt[key] = now
  A.requested = now
  A.requestKey = key
  ns.Debug("prices: auction house: asked for a snapshot")
  if not A.eventOK then
    ns.After(A.FALLBACK, "prices auction fallback", function()
      if A.requested == now and A.open then A.OnReplicated() end
    end)
  end
end

-- Stops the walk; nothing of it is kept. `closed`: the house closed while rows were read, so the
-- visit got nothing and the next one may ask again at once (a logout keeps the throttle).
function A.Stop(why, closed)
  local w = A.walk
  if w then
    ns.Debug("prices: auction snapshot stopped (%s), nothing kept", why)
    local p = closed and A.Store()
    if p and w.key and p.auctionAt[w.key] == w.askedAt then p.auctionAt[w.key] = nil end
  end
  A.walk = nil
end

function A.OnClosed()
  A.open = false
  A.requested = nil
  A.browsing = {}
  if A.walk and A.walk.phase == "read" then A.Stop("the auction house closed", true) end
end

local schedule

-- Reads row i (from 0) into the walk; under pcall, so a value the engine won't let us touch only
-- costs that row.
local function readRow(w, info, i)
  -- name, texture, count, quality, usable, level, levelType, minBid, minIncrement, buyoutPrice,
  -- bidAmount, (12-15: bidder and owner, never read), saleStatus, itemID, hasAllInfo
  local _, _, count, _, _, _, _, _, _, buyout, _, _, _, _, _, _, itemID, hasAllInfo = info(i)
  -- hasAllInfo false means the item's name and link aren't cached yet; count, buyout and id are
  -- there all the same, so the row counts (skipping it would make a cold cache a partial snapshot).
  if ns.IsSecret(hasAllInfo) or hasAllInfo == false then w.notReady = w.notReady + 1 end
  if not isNumber(itemID) or itemID < 1 or itemID ~= floor(itemID) or not isNumber(count) or count < 1 then
    w.bad = w.bad + 1
    return
  end
  if not isNumber(buyout) or buyout <= 0 then
    w.noBuyout = w.noBuyout + 1
    return
  end
  local list = w.lists[itemID]
  if not list then
    list = {}
    w.lists[itemID] = list
    w.ids[#w.ids + 1] = itemID
  end
  list[#list + 1] = A.Pack(floor(buyout / count + 0.5), count)
end

-- One frame of the walk: a slice of rows, or a slice of items to summarise.
local function step(w)
  if A.walk ~= w then return end
  if w.phase == "read" then
    if not A.open then return A.Stop("the auction house closed", true) end
    local info = ns.Fn("C_AuctionHouse", "GetReplicateItemInfo")
    if not info then return A.Stop("no GetReplicateItemInfo") end
    local last = math.min(w.i + A.ROWS_PER_FRAME, w.total) - 1
    for i = w.i, last do
      if not pcall(readRow, w, info, i) then w.bad = w.bad + 1 end
    end
    w.i = last + 1
    if w.i >= w.total then w.phase, w.j = "sum", 1 end
  else
    local done = 0
    while w.j <= #w.ids and done < A.ROWS_PER_FRAME do
      local id = w.ids[w.j]
      local list = w.lists[id]
      w.lists[id] = nil
      local stats = A.Stats(list)
      if stats then
        stats.id = id
        w.out[#w.out + 1] = stats
      end
      done = done + #list
      w.j = w.j + 1
    end
    if w.j > #w.ids then return A.Finish(w) end
  end
  schedule(w)
end

schedule = function(w)
  ns.After(0, "prices auction snapshot", function() step(w) end)
end

function A.OnReplicated()
  if not A.requested or A.walk or not A.open then return end
  if not S.On("priceAuctionSnapshot") then
    A.requested = nil
    return
  end
  local total = ns.Try(ns.Fn("C_AuctionHouse", "GetNumReplicateItems"))
  -- Nothing yet: wait for the next update.
  if not isNumber(total) or total < 1 then return end
  local askedAt = A.requested
  A.requested = nil
  A.walk = { phase = "read", at = floor(ns.Now()), total = floor(total), i = 0, lists = {}, ids = {}, out = {},
    noBuyout = 0, notReady = 0, bad = 0, key = A.requestKey, askedAt = askedAt }
  ns.Debug("prices: auction snapshot: reading %d rows", A.walk.total)
  schedule(A.walk)
end

-- An item's numbers fit the API's limits (docs/wow.md 17).
local function sendable(r)
  local function within(v, max) return type(v) == "number" and v >= 1 and v <= max and v == floor(v) end
  return within(r.id, MAX_ID) and within(r.n, MAX_N) and within(r.q, MAX_Q) and within(r.min, MAX_PRICE)
    and within(r.q25, MAX_PRICE) and within(r.med, MAX_PRICE) and r.min <= r.q25 and r.q25 <= r.med
    and r.n <= r.q
end

-- Whether this core can take `auction` events: it drops them once uploaded (Ryna.Journal.ShortLived)
-- and lets a mod trim them (Events, Remove). On an older core they would sit in the journal for 30
-- days, so nothing is sent there; the local store and the tooltip line work all the same.
function A.CanSend()
  local J = ns.API.Journal
  return type(J) == "table" and J.ShortLived == true and type(J.Events) == "function"
    and type(J.Remove) == "function"
end

-- Makes room for a snapshot of `incoming` items: the journal keeps the newest A.JOURNAL_SNAPSHOTS
-- earlier snapshots (by `at`) and, with the new one, never more than A.JOURNAL_ITEMS items; the
-- oldest go first. Upload or not, since a journal past the agent's 16 MB upload limit fails every
-- push. Returns how many events went.
function A.MakeRoom(incoming)
  local J = ns.API.Journal
  local sizes, ats = {}, {}
  for _, e in ipairs(J.Events("auction")) do
    local at = type(e.at) == "number" and e.at or 0
    if not sizes[at] then
      sizes[at] = 0
      ats[#ats + 1] = at
    end
    sizes[at] = sizes[at] + (type(e.items) == "table" and #e.items or 0)
  end
  table.sort(ats, function(a, b) return a > b end)
  local keep, budget = {}, A.JOURNAL_ITEMS - incoming
  for k, at in ipairs(ats) do
    if k > A.JOURNAL_SNAPSHOTS or sizes[at] > budget then break end
    keep[at] = true
    budget = budget - sizes[at]
  end
  return J.Remove("auction", function(e) return not keep[type(e.at) == "number" and e.at or 0] end)
end

-- The snapshot as `auction` journal events, every part in the same frame so they land in the same
-- save (the API pools a snapshot once, from the first upload that carries it, and skips parts that
-- come later). At most A.JOURNAL_ITEMS items, the most listed. Returns how many events were added.
function A.Send(w)
  if not A.CanSend() then return 0 end
  local items = {}
  for _, r in ipairs(w.out) do
    if sendable(r) then items[#items + 1] = r end
  end
  local cap = math.min(A.PART_ITEMS * A.MAX_PARTS, A.JOURNAL_ITEMS)
  if #items > cap then
    table.sort(items, function(a, b)
      if a.n ~= b.n then return a.n > b.n end
      return a.id < b.id
    end)
    for k = #items, cap + 1, -1 do items[k] = nil end
  end
  if #items == 0 then return 0 end
  A.MakeRoom(#items)
  table.sort(items, function(a, b) return a.id < b.id end)
  local parts = math.ceil(#items / A.PART_ITEMS)
  local rows = math.min(w.total, MAX_ROWS)
  for part = 1, parts do
    local slice = {}
    for k = (part - 1) * A.PART_ITEMS + 1, math.min(part * A.PART_ITEMS, #items) do slice[#slice + 1] = items[k] end
    ns.API.Journal.Add("auction", { at = w.at, rows = rows, part = part, parts = parts, items = slice }, true)
  end
  return parts
end

-- The walk is done: the store, the guard, the events.
function A.Finish(w)
  A.walk = nil
  local p = A.Store()
  if not p then return end
  for _, r in ipairs(w.out) do
    p.auction[r.id] = { min = r.min, q25 = r.q25, med = r.med, n = r.n, q = r.q, at = w.at }
  end
  A.Guard(p)
  p.auctionLast = { at = w.at, rows = w.total, items = #w.out, noBuyout = w.noBuyout, notReady = w.notReady,
    bad = w.bad }
  local parts = 0
  if S.On("priceAuctionSnapshot") then parts = A.Send(w) end
  ns.Debug("prices: auction snapshot: %d rows, %d items, %d without a buyout, %d not loaded, %d unreadable, "
    .. "%d events", w.total, #w.out, w.noBuyout, w.notReady, w.bad, parts)
end

-- Whether a browse result is the item itself, so its price can stand for the item id: no random
-- suffix ("of the Bear"), and an item that isn't worn (C_Item.GetItemInfoInstant's equip slot is
-- empty) or, for gear, the item's own level (a gear item's results come one per item level, and
-- their prices differ). Gear whose level the client doesn't have loaded is skipped.
function A.BaseItem(key)
  local suffix = key.itemSuffix
  if suffix ~= nil and (not isNumber(suffix) or suffix ~= 0) then return false end
  local _, _, _, equipLoc = ns.Try(ns.Fn("C_Item", "GetItemInfoInstant"), key.itemID)
  if not ns.Usable(equipLoc) or equipLoc == "" or equipLoc == "INVTYPE_NON_EQUIP_IGNORE" then return true end
  local _, _, _, level = ns.Try(ns.Fn("C_Item", "GetItemInfo"), key.itemID)
  return isNumber(level) and isNumber(key.itemLevel) and key.itemLevel == level
end

-- A search on the browse tab: its results into the store, and the ids it shows.
function A.OnBrowse()
  local results = ns.Try(ns.Fn("C_AuctionHouse", "GetBrowseResults"))
  if type(results) ~= "table" then return end
  local now = floor(ns.Now())
  local seen, shown = {}, {}
  for _, r in ipairs(results) do
    local key = type(r) == "table" and r.itemKey
    local id = type(key) == "table" and key.itemID
    local pet = type(key) == "table" and key.battlePetSpeciesID
    if isNumber(id) and id >= 1 and not (isNumber(pet) and pet > 0) then
      shown[id] = true
      local price, quantity = r.minPrice, r.totalQuantity
      if A.BaseItem(key) and isNumber(price) and price >= 1 and isNumber(quantity) and quantity >= 1 then
        local s = seen[id]
        if not s then
          seen[id] = { min = floor(price), q = floor(quantity) }
        else
          if price < s.min then s.min = floor(price) end
          s.q = s.q + floor(quantity)
        end
      end
    end
  end
  A.browsing = shown
  if not S.On("priceAuctionLine") then return end
  local p = A.Store()
  if not p then return end
  for id, s in pairs(seen) do
    local e = p.auction[id]
    if type(e) == "table" and type(e.med) == "number" then
      -- A snapshot's entry keeps its median, quantiles and counts: a search moves only its lowest
      -- price and when it was seen (and not even that within 15 minutes of the snapshot).
      if not (type(e.at) == "number" and now - e.at < A.THROTTLE) then
        e.min, e.at = s.min, now
      end
    else
      p.auction[id] = { min = s.min, q = s.q, at = now }
    end
  end
  A.Guard(p)
end

local function shown(frame)
  if type(frame) ~= "table" or type(frame.IsShown) ~= "function" then return false end
  local ok, v = pcall(frame.IsShown, frame)
  return ok and v == true
end

-- Whether the auction house's own window shows this item's price: the tooltip belongs to one of its
-- lists (the browse results, an item's listings, the sell tab), or the browse tab lists the item.
function A.HouseShows(tooltip, id)
  local frame = _G.AuctionHouseFrame
  if not shown(frame) then return false end
  local owner = type(tooltip) == "table" and type(tooltip.GetOwner) == "function" and ns.Try(tooltip.GetOwner, tooltip)
  for _ = 1, 30 do
    if type(owner) ~= "table" then break end
    if owner == frame then return true end
    owner = type(owner.GetParent) == "function" and ns.Try(owner.GetParent, owner) or nil
  end
  return id ~= nil and A.browsing[id] == true and shown(frame.BrowseResultsFrame)
end

-- Events ----------------------------------------------------------------------------------------

local function run(label, fn)
  return function() ns.Run(label, fn) end
end

ns.On("AUCTION_HOUSE_SHOW", run("prices auction house", A.OnShow))
ns.On("AUCTION_HOUSE_CLOSED", run("prices auction house", A.OnClosed))
A.eventOK = ns.On("REPLICATE_ITEM_LIST_UPDATE", run("prices auction snapshot", A.OnReplicated)) and true or false
ns.On("AUCTION_HOUSE_BROWSE_RESULTS_UPDATED", run("prices auction browse", A.OnBrowse))
ns.On("AUCTION_HOUSE_BROWSE_RESULTS_ADDED", run("prices auction browse", A.OnBrowse))
ns.On("PLAYER_LOGOUT", function()
  A.requested = nil
  A.Stop("logging out")
end)
