-- Prices' auction side against a fake client (no game needed): loads the addon's files in TOC order
-- with a stand-in for the core's mod API (its clock, timers and journal among it) and a fake
-- C_AuctionHouse, opens the house, runs the frames and checks what comes out. From the repository
-- root, with Lua 5.1 (what the game runs):
--
--   lua5.1 mods/wow/prices/test/auction_test.lua
--
-- This proves the logic: the snapshot walk across frames (at most 2,000 rows a frame, several
-- thousand rows with no-buyout and not-loaded rows skipped and counted, stacks of different counts),
-- the quantity-weighted percentiles on known inputs, the 15-minute throttle per character, the house
-- closing or the player logging out mid-walk, the `auction` events (split at 2,000 items, every value
-- an integer, no strings, nothing about a seller anywhere), the browse fold, the pruning and the size
-- guard, the tooltip line in each form and hidden where the auction house shows the price, the
-- toggles, the fallback without REPLICATE_ITEM_LIST_UPDATE, /ryna prices, and nothing in chat. It
-- can't prove the client's API behaves as assumed, or what a walk of 192k rows costs a frame. That's
-- the game's.
-- luacheck: ignore 1

local ADDON = "mods/wow/prices/addon/Ryna_Prices/"

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end

local errors, chat = {}, {}
local function run(label, fn, ...)
  local ok, err = pcall(fn, ...)
  if not ok then errors[#errors + 1] = label .. ": " .. tostring(err) end
end

-- The clock and the timers: After(0) is "next frame"; frame() runs one frame's worth.
local now = 1700000000
local timers, timerSeq = {}, 0
local function after(delay, label, fn)
  timerSeq = timerSeq + 1
  timers[#timers + 1] = { at = now + delay, seq = timerSeq, label = label, fn = fn }
end
-- One frame: every timer due now that was queued before the frame began.
local function frame()
  local due, rest = {}, {}
  for _, t in ipairs(timers) do
    if t.at <= now then due[#due + 1] = t else rest[#rest + 1] = t end
  end
  timers = rest
  table.sort(due, function(a, b) return a.seq < b.seq end)
  for _, t in ipairs(due) do run(t.label, t.fn) end
  return #due
end
local function frames(max)
  local n = 0
  while n < (max or 10000) and frame() > 0 do n = n + 1 end
  return n
end
local function advance(seconds)
  now = now + seconds
  frames()
end

-- The auction house: rows (index 0 up), browse results, and what was asked.
local ROWS, BROWSE = {}, {}
local calls = { replicate = 0, info = 0 }
local SELLER = "Some Seller-Realm"
local function row(id, count, buyout, hasAllInfo)
  return { id = id, count = count, buyout = buyout, hasAllInfo = hasAllInfo }
end

-- Items the client has loaded: id -> { name, price, stack }.
local ITEMS = {
  [4306] = { name = "Silk Cloth", price = 1350, stack = 20 },
  [2140] = { name = "Carving Knife", price = 7, stack = 1 },
}
local function link(id) return "|cnIQ2:|Hitem:" .. id .. "::::::::8:1487::::::|h[x]|h|r" end

local function shownFrame(parent)
  local f = { shown = false, parent = parent }
  function f.IsShown(self) return self.shown end
  function f.GetParent(self) return self.parent end
  return f
end

local postCalls = {}
local replicateRegisters = true -- whether the client knows REPLICATE_ITEM_LIST_UPDATE
local guid = "Player-1-0000AAAA"
local function installClient()
  _G.GetLocale = function() return "enUS" end
  _G.SELL_PRICE = "Sell Price"
  _G.C_CurrencyInfo, _G.GetCoinTextureString = nil, nil
  _G.CreateFrame = nil
  _G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) chat[#chat + 1] = msg end }
  _G.UnitGUID = function(unit) if unit == "player" then return guid end end
  _G.C_Item = {
    GetItemInfo = function(x)
      local id = type(x) == "number" and x or tonumber(tostring(x):match("Hitem:(%d+)"))
      local it = id and ITEMS[id]
      if not it then return end
      return it.name, link(id), 1, 5, 1, "Trade Goods", "Cloth", it.stack, "", 132889, it.price
    end,
    RequestLoadItemDataByID = function() end,
  }
  _G.C_AuctionHouse = {
    ReplicateItems = function() calls.replicate = calls.replicate + 1 end,
    GetNumReplicateItems = function() return #ROWS end,
    GetReplicateItemInfo = function(i)
      calls.info = calls.info + 1
      local r = ROWS[i + 1]
      if not r then return end
      -- The documented 18 values; the bidder and owner ones (12-15) filled in here, so the test can
      -- prove none of them is ever kept.
      return "Item " .. r.id, 135139, r.count, 2, true, 10, "REQ_LEVEL_ABBR", 0, 0, r.buyout, 0,
        SELLER, SELLER, SELLER, SELLER, 0, r.id, r.hasAllInfo
    end,
    GetBrowseResults = function() return BROWSE end,
  }
  local house = shownFrame(nil)
  house.BrowseResultsFrame = shownFrame(house)
  _G.AuctionHouseFrame = house
  _G.MerchantFrame = nil
  postCalls = {}
  _G.Enum = { TooltipDataType = { Item = 0 } }
  _G.TooltipDataProcessor = {
    AddTooltipPostCall = function(kind, fn) postCalls[#postCalls + 1] = { kind = kind, fn = fn } end,
  }
end

-- The core's mod API, as much of it as Prices uses.
local handlers, settings, defaults, tabs, commands, statusPanel, registered, journal
local db
local function passResults(ok, ...)
  if ok then return ... end
  return nil
end
local function try(fn, ...)
  if type(fn) ~= "function" then return nil end
  return passResults(pcall(fn, ...))
end
local function fn(a, b)
  local v = _G[a]
  if b ~= nil then
    if type(v) ~= "table" then return nil end
    v = v[b]
  end
  return type(v) == "function" and v or nil
end
local function widget()
  local w = { checked = false }
  function w.SetChecked(_, v) w.checked = v end
  function w.SetScript() end
  function w.place() w.placed = true end
  function w.setActive(a) w.active = a end
  function w.SetText(_, t) w.text = t end
  function w.SetWidth() end
  return w
end
local function money(c)
  local g, s, cc = math.floor(c / 10000), math.floor(c / 100) % 100, c % 100
  local parts = {}
  if g > 0 then parts[#parts + 1] = g .. "g" end
  if s > 0 then parts[#parts + 1] = s .. "s" end
  if cc > 0 or #parts == 0 then parts[#parts + 1] = cc .. "c" end
  return table.concat(parts, " ")
end
-- As the core's Status.Ago, in English.
local function ago(t)
  if type(t) ~= "number" then return nil end
  local d = now - t
  local function plural(n, one, many) return string.format(n == 1 and one or many, n) end
  if d < 60 then return "just now" end
  if d < 3600 then return plural(math.floor(d / 60), "%d minute ago", "%d minutes ago") end
  if d < 86400 then return plural(math.floor(d / 3600), "%d hour ago", "%d hours ago") end
  return plural(math.floor(d / 86400), "%d day ago", "%d days ago")
end
local function installApi()
  handlers, settings, defaults, tabs, commands, statusPanel, registered = {}, {}, {}, {}, {}, {}, nil
  journal = {}
  db = { settings = settings }
  timers = {}
  _G.Ryna = {
    API = 1, VERSION = "test", L = setmetatable({}, { __index = function(_, k) return k end }),
    Try = try, Fn = fn,
    IsSecret = function() return false end,
    NonEmpty = function(s) if type(s) == "string" and s ~= "" then return s end end,
    Now = function() return now end,
    Ago = ago,
    After = after,
    Locale = function() return "enUS" end,
    Print = function(m) chat[#chat + 1] = "Ryna: " .. m end,
    Debug = function() end,
    Run = run,
    ReportError = function(label, err) errors[#errors + 1] = label .. ": " .. tostring(err) end,
    MoneyText = money,
    On = function(event, f)
      if event == "REPLICATE_ITEM_LIST_UPDATE" and not replicateRegisters then return false end
      handlers[event] = handlers[event] or {}
      table.insert(handlers[event], f)
      return true
    end,
    DB = function() return db end,
    RegisterMod = function(spec) registered = spec return true end,
    Journal = {
      Add = function(eventType, fields)
        journal[#journal + 1] = { type = eventType, fields = fields }
        return fields
      end,
    },
    Settings = {
      Get = function(key)
        local v = settings[key]
        if type(v) == "boolean" then return v end
        return defaults[key]
      end,
      Set = function(key, value) settings[key] = value end,
      AddDefaults = function(d) for k, v in pairs(d) do defaults[k] = v end end,
      All = function() return settings end,
    },
    Panel = {
      AddTab = function(spec) tabs[#tabs + 1] = spec return true end,
      AddStatusLines = function(order, f) statusPanel[#statusPanel + 1] = { order = order, fn = f } end,
      Refresh = function() end,
      RefreshIfShown = function() end,
      Widgets = {
        WIDTH = 580,
        text = function() return widget() end,
        checkbox = function() return widget() end,
        toggle = function(cb, key) cb.key = key end,
      },
    },
    Slash = {
      Add = function(spec) commands[spec.word] = spec return true end,
      AddStatusLines = function() end,
    },
  }
end

local function fire(event, ...)
  for _, f in ipairs(handlers[event] or {}) do run(event, f, event, ...) end
end

local ns
local function load()
  installClient()
  installApi()
  ns = {}
  local toc = assert(io.open(ADDON .. "Ryna_Prices.toc")):read("*a")
  for line in toc:gmatch("[^\r\n]+") do
    if not line:match("^#") and line:match("%.lua$") then
      local chunk = assert(loadfile(ADDON .. line:gsub("\\", "/")))
      chunk("Ryna_Prices", ns)
    end
  end
  if registered and registered.onLogin then run("onLogin", registered.onLogin) end
end

local function store() return db.prices and db.prices.auction end

-- Walks every value of a table (and its tables), calling visit(value, path).
local function walk(t, visit, path, seen)
  seen = seen or {}
  if seen[t] then return end
  seen[t] = true
  for k, v in pairs(t) do
    local p = (path or "") .. "." .. tostring(k)
    visit(k, p .. "(key)")
    visit(v, p)
    if type(v) == "table" then walk(v, visit, p, seen) end
  end
end

-- The rows: several thousand, over 300 items, in stacks of 1, 3, 5 and 20, a buyout that doesn't
-- always divide evenly, every 37th with no buyout and every 53rd not loaded yet.
local COUNTS = { 1, 5, 20, 1, 3 }
local function makeRows(n)
  local rows = {}
  for i = 0, n - 1 do
    local id = 1001 + (i * 7) % 300
    local count = COUNTS[i % 5 + 1]
    local buyout = count * (100 + (i * 31) % 977) + i % 3
    if i % 37 == 0 then buyout = 0 end
    rows[#rows + 1] = row(id, count, buyout, i % 53 ~= 0)
  end
  return rows
end

-- What the numbers should be, worked out the slow way: every unit of quantity its own entry,
-- sorted, then the ceil(q/4)-th and ceil(q/2)-th.
local function expected(rows)
  local units, n, noBuyout, notReady = {}, {}, 0, 0
  for _, r in ipairs(rows) do
    if r.hasAllInfo == false then
      notReady = notReady + 1
    elseif r.buyout <= 0 then
      noBuyout = noBuyout + 1
    else
      local unit = math.max(1, math.floor(r.buyout / r.count + 0.5))
      units[r.id] = units[r.id] or {}
      n[r.id] = (n[r.id] or 0) + 1
      for _ = 1, r.count do table.insert(units[r.id], unit) end
    end
  end
  local out, items = {}, 0
  for id, list in pairs(units) do
    table.sort(list)
    local q = #list
    out[id] = { n = n[id], q = q, min = list[1], q25 = list[math.ceil(q / 4)], med = list[math.ceil(q / 2)] }
    items = items + 1
  end
  return out, items, noBuyout, notReady
end

-- Opens the house, and the client answers with the rows.
local function openHouse()
  _G.AuctionHouseFrame.shown = true
  fire("AUCTION_HOUSE_SHOW")
end
local function closeHouse()
  _G.AuctionHouseFrame.shown = false
  _G.AuctionHouseFrame.BrowseResultsFrame.shown = false
  fire("AUCTION_HOUSE_CLOSED")
end

-- Loading. ------------------------------------------------------------------------------------------
load()
local A = ns.Auction
check(not ns.off and A ~= nil, "the mod loads, with its auction side")
check(defaults.priceAuctionLine == true and defaults.priceAuctionSnapshot == true
  and defaults.priceAuctionStackTotal == true, "defaults: the auction line, its stack total and the snapshot on")
check(#chat == 0, "nothing in chat at load or login")

-- The maths, on known inputs. ------------------------------------------------------------------------
local P = A.Pack
local s = A.Stats({ P(400, 1), P(100, 1), P(300, 1), P(200, 1) })
check(s.n == 4 and s.q == 4 and s.min == 100 and s.q25 == 100 and s.med == 200,
  "four single listings: min 100, q25 100, med 200")
s = A.Stats({ P(30, 1), P(10, 1), P(20, 10) })
check(s.n == 3 and s.q == 12 and s.min == 10 and s.q25 == 20 and s.med == 20,
  "weighted by quantity: one cheap single doesn't make the 25th percentile")
s = A.Stats({ P(5, 3) })
check(s.n == 1 and s.q == 3 and s.min == 5 and s.q25 == 5 and s.med == 5, "one listing: all three the same")
s = A.Stats({ P(1, 1), P(2, 1), P(3, 1), P(1000, 97) })
check(s.min == 1 and s.q25 == 1000 and s.med == 1000, "a big stack dominates: q25 and med are its price")
s = A.Stats({ P(10, 2), P(20, 2), P(30, 2), P(40, 2) })
check(s.q25 == 10 and s.med == 20, "q/4 and q/2 reached exactly: the lower price (q25 10, med 20)")
check(A.Stats({}) == nil, "no listings: no numbers")
check(P(3, 20) % 65536 == 20 and math.floor(P(3, 20) / 65536) == 3, "a listing packs and unpacks")
check(math.floor(P(0, 20) / 65536) == 1, "a unit price under a copper counts as 1")
check(math.floor(P(1e15, 1) / 65536) == 2 ^ 37 - 1, "an absurd unit price is clamped, the number stays exact")

-- The walk across frames. ---------------------------------------------------------------------------
ROWS = makeRows(5000)
local want, wantItems, wantNoBuyout, wantNotReady = expected(ROWS)
openHouse()
check(calls.replicate == 1, "opening the house asks for a snapshot (ReplicateItems)")
check(db.prices.auctionAt[guid] == now, "and keeps when, for this character")
check(calls.info == 0, "nothing read before the rows are there")
fire("REPLICATE_ITEM_LIST_UPDATE")
check(calls.info == 0, "REPLICATE_ITEM_LIST_UPDATE reads nothing in its own frame")
check(A.walk ~= nil and A.walk.total == 5000, "it starts a walk over 5000 rows")
local perFrame = {}
local before = 0
while A.walk do
  if frame() == 0 then break end
  perFrame[#perFrame + 1] = calls.info - before
  before = calls.info
end
check(calls.info == 5000, "every row read once: " .. calls.info)
check(perFrame[1] == 2000 and perFrame[2] == 2000 and perFrame[3] == 1000,
  "2,000 rows a frame: " .. table.concat(perFrame, ","))
local most = 0
for _, n in ipairs(perFrame) do most = math.max(most, n) end
check(most <= 2000 and #perFrame >= 4, "never more than 2,000 rows a frame, then the summing takes frames of its own: "
  .. #perFrame .. " frames")
check(A.walk == nil, "the walk is over")

local auction = store()
local bad = {}
local items = 0
for id, w in pairs(want) do
  local e = auction[id]
  items = items + 1
  if not e or e.n ~= w.n or e.q ~= w.q or e.min ~= w.min or e.q25 ~= w.q25 or e.med ~= w.med or e.at ~= now then
    bad[#bad + 1] = tostring(id)
  end
end
check(items == wantItems and #bad == 0, "every item's numbers match the slow way: " .. table.concat(bad, ","))
local stored = 0
for _ in pairs(auction) do stored = stored + 1 end
check(stored == wantItems, "and nothing else stored: " .. stored)
local last = db.prices.auctionLast
check(last and last.rows == 5000 and last.items == wantItems and last.noBuyout == wantNoBuyout
  and last.notReady == wantNotReady and last.bad == 0,
  "the last snapshot counts its rows and the skipped ones: " .. tostring(last and last.noBuyout)
  .. " without a buyout, "
  .. tostring(last and last.notReady) .. " not loaded")
check(wantNoBuyout > 100 and wantNotReady > 50, "the rows had both kinds to skip")

-- The event.
check(#journal == 1 and journal[1].type == "auction", "one auction event for 300 items")
local e = journal[1].fields
check(e.at == now and e.rows == 5000 and e.part == 1 and e.parts == 1 and #e.items == wantItems,
  "at, rows, part 1 of 1, every item")
local keys = {}
for k in pairs(e) do keys[#keys + 1] = k end
table.sort(keys)
check(table.concat(keys, ",") == "at,items,part,parts,rows",
  "exactly the documented fields: " .. table.concat(keys, ","))
local okShape, sorted, prev = true, true, 0
for _, it in ipairs(e.items) do
  local ks = {}
  for k in pairs(it) do ks[#ks + 1] = k end
  table.sort(ks)
  if table.concat(ks, ",") ~= "id,med,min,n,q,q25" then okShape = false end
  if not (it.min <= it.q25 and it.q25 <= it.med) then okShape = false end
  local w = want[it.id]
  if not w or w.med ~= it.med or w.q ~= it.q then okShape = false end
  if it.id <= prev then sorted = false end
  prev = it.id
end
check(okShape, "each item is { id, n, q, min, q25, med }, the right numbers, min <= q25 <= med")
check(sorted, "items in id order")
local nonInteger = {}
walk(e, function(v, path)
  if path:find("%(key%)$") then return end -- field names are names, not values
  if type(v) == "number" and v ~= math.floor(v) then nonInteger[#nonInteger + 1] = path end
  if type(v) == "string" or type(v) == "boolean" then nonInteger[#nonInteger + 1] = path .. "=" .. tostring(v) end
end)
check(#nonInteger == 0, "every value an integer, no strings: " .. table.concat(nonInteger, " "))
local sellers = {}
walk(db, function(v, path) if type(v) == "string" and v:find("Seller") then sellers[#sellers + 1] = path end end)
walk(journal, function(v, path) if type(v) == "string" and v:find("Seller") then sellers[#sellers + 1] = path end end)
check(#sellers == 0, "no seller anywhere in RynaDB or the journal: " .. table.concat(sellers, " "))

-- The 15-minute throttle. -----------------------------------------------------------------------------
closeHouse()
now = now + 10 * 60
openHouse()
check(calls.replicate == 1, "again 10 minutes later: no second ReplicateItems")
fire("REPLICATE_ITEM_LIST_UPDATE")
check(A.walk == nil, "and an update nobody asked for starts no walk")
closeHouse()
guid = "Player-1-0000BBBB"
openHouse()
check(calls.replicate == 2 and db.prices.auctionAt["Player-1-0000BBBB"] == now,
  "another character on the account: its own throttle")
closeHouse()
guid = "Player-1-0000AAAA"
now = now + 5 * 60
openHouse()
check(calls.replicate == 3, "15 minutes after the first: the next snapshot")
closeHouse()
check(A.requested == nil, "closing the house forgets the request")
-- A last time in the future (the clock went back): not held off.
now = now + 60
db.prices.auctionAt[guid] = now + 3600
openHouse()
check(calls.replicate == 4, "a last time in the future doesn't hold the next one off")
closeHouse()

-- The house closing mid-walk. -----------------------------------------------------------------------
journal = {}
local snapshotStore = {}
for id, x in pairs(store()) do snapshotStore[id] = x end
now = now + 20 * 60
openHouse()
fire("REPLICATE_ITEM_LIST_UPDATE")
local readBefore = calls.info
frame()
check(calls.info - readBefore == 2000, "a frame of rows read")
closeHouse()
frames()
check(calls.info - readBefore == 2000, "the house closed: no more rows read")
check(A.walk == nil and #journal == 0, "and the walk stopped: no event")
local same = true
for id, x in pairs(store()) do if snapshotStore[id] ~= x then same = false end end
check(same, "and the store untouched")

-- Closing while summing: the rows are in hand, so it finishes.
now = now + 20 * 60
openHouse()
fire("REPLICATE_ITEM_LIST_UPDATE")
frame() frame() frame()
check(A.walk and A.walk.phase == "sum", "all rows read, summing")
closeHouse()
frames()
check(A.walk == nil and #journal == 1, "the house closing while summing doesn't lose the snapshot")

-- Logging out mid-walk. --------------------------------------------------------------------------------
journal = {}
now = now + 20 * 60
openHouse()
fire("REPLICATE_ITEM_LIST_UPDATE")
frame()
fire("PLAYER_LOGOUT")
frames()
check(A.walk == nil and #journal == 0, "logging out stops the walk: no event")
closeHouse()
now = now + 20 * 60
openHouse()
fire("REPLICATE_ITEM_LIST_UPDATE")
frame() frame() frame() frame()
check(A.walk and A.walk.phase == "sum", "summing again")
fire("PLAYER_LOGOUT")
frames()
check(A.walk == nil and #journal == 0, "logging out while summing stops it too")
closeHouse()

-- Parts: 4,500 items, then exactly 2,000. ---------------------------------------------------------------
local function snapshotOf(rows)
  ROWS = rows
  journal = {}
  now = now + 20 * 60
  openHouse()
  fire("REPLICATE_ITEM_LIST_UPDATE")
  frames()
  closeHouse()
end
local many = {}
for i = 1, 4500 do many[#many + 1] = row(20000 + i, 1, 100 + i, true) end
-- An item priced past the API's limit (over 10,000 gold each): kept here, left out of the event.
many[#many + 1] = row(90001, 1, 200000000, true)
snapshotOf(many)
check(#journal == 3, "4,500 sendable items: three events: " .. #journal)
local total, partsOk, ids = 0, true, {}
for k, ev in ipairs(journal) do
  local f = ev.fields
  total = total + #f.items
  if f.part ~= k or f.parts ~= 3 or f.at ~= journal[1].fields.at or f.rows ~= 4501 then partsOk = false end
  if #f.items > 2000 then partsOk = false end
  for _, it in ipairs(f.items) do
    if ids[it.id] then partsOk = false end
    ids[it.id] = true
  end
end
check(partsOk and total == 4500, "parts 1..3 of 3, one `at`, rows on each, no item twice, 2000 at most each")
check(#journal[1].fields.items == 2000 and #journal[2].fields.items == 2000 and #journal[3].fields.items == 500,
  "split 2000, 2000, 500")
check(not ids[90001] and store()[90001] and store()[90001].med == 200000000,
  "an item past the API's price limit isn't sent, and is still kept here")
local exact = {}
for i = 1, 2000 do exact[#exact + 1] = row(30000 + i, 2, 50, true) end
snapshotOf(exact)
check(#journal == 1 and journal[1].fields.parts == 1 and #journal[1].fields.items == 2000,
  "exactly 2,000 items: one event")
local unit = journal[1].fields.items[1]
check(unit.min == 25 and unit.q == 2 and unit.n == 1, "a stack's unit price is buyout / count")

-- The browse fold. ---------------------------------------------------------------------------------------
journal = {}
now = now + 3600
local fresh = 30001 -- from the snapshot an hour ago: older than 15 minutes, so a search replaces it
BROWSE = {
  { itemKey = { itemID = 1251, itemLevel = 1, itemSuffix = 0, battlePetSpeciesID = 0 }, totalQuantity = 13754,
    minPrice = 10, containsOwnerItem = false },
  { itemKey = { itemID = 5000, itemLevel = 20, itemSuffix = 0, battlePetSpeciesID = 0 }, totalQuantity = 2,
    minPrice = 900, containsOwnerItem = false },
  { itemKey = { itemID = 5000, itemLevel = 25, itemSuffix = 0, battlePetSpeciesID = 0 }, totalQuantity = 3,
    minPrice = 700, containsOwnerItem = false },
  { itemKey = { itemID = 82800, itemLevel = 1, itemSuffix = 0, battlePetSpeciesID = 39 }, totalQuantity = 1,
    minPrice = 5, containsOwnerItem = false },
  { itemKey = { itemID = fresh, itemLevel = 1, itemSuffix = 0, battlePetSpeciesID = 0 }, totalQuantity = 9,
    minPrice = 7, containsOwnerItem = false },
}
openHouse() -- an hour after the last: this asks for a snapshot too; it isn't answered here
fire("AUCTION_HOUSE_BROWSE_RESULTS_UPDATED")
local b = store()[1251]
check(b and b.min == 10 and b.q == 13754 and b.med == nil and b.q25 == nil and b.n == nil and b.at == now,
  "a search: min from minPrice, q from totalQuantity, no median")
check(store()[5000] and store()[5000].min == 700 and store()[5000].q == 5,
  "two results for one item (item levels): the lowest price, the quantities added")
check(store()[82800] == nil, "a battle pet's result isn't kept")
check(store()[fresh].med == nil and store()[fresh].min == 7, "an hour-old snapshot entry is replaced by a search")
check(#journal == 0, "a search sends nothing")
-- A snapshot entry from the last 15 minutes says more than a search: kept.
store()[1251] = { min = 8, q25 = 9, med = 11, n = 40, q = 900, at = now - 60 }
fire("AUCTION_HOUSE_BROWSE_RESULTS_UPDATED")
check(store()[1251].med == 11 and store()[1251].min == 8, "a fresh snapshot entry is kept over a search")
closeHouse()
check(next(A.browsing) == nil, "closing the house forgets what the browse tab listed")

-- Pruning and the size guard. ---------------------------------------------------------------------------
local p = db.prices
p.auction = {
  [1] = { min = 1, q = 1, at = now - 15 * 86400 },
  [2] = { min = 1, q = 1, at = now - 13 * 86400 },
  [3] = "junk",
  [4] = { min = 1, at = now },
}
p.auctionAt = { old = now - 20 * 86400, new = now - 60 }
A.Prune()
check(p.auction[1] == nil and p.auction[2] ~= nil and p.auction[3] == nil and p.auction[4] == nil,
  "at login: entries older than 14 days and malformed ones go, the rest stay")
check(p.auctionAt.old == nil and p.auctionAt.new ~= nil, "and old throttle times")
p.auction = {}
for i = 1, 20005 do p.auction[i] = { min = 1, q = 1, at = now - 100000 + i } end
A.Guard()
local kept = 0
for _ in pairs(p.auction) do kept = kept + 1 end
check(kept == 20000 and p.auction[5] == nil and p.auction[6] ~= nil and p.auction[20005] ~= nil,
  "over 20,000 items: the 20,000 most recently seen are kept: " .. kept)
p.auction = {}
for i = 1, 20005 do p.auction[i] = { min = 1, q = 1, at = now - 100000 + i } end
run("login", registered.onLogin)
kept = 0
for _ in pairs(p.auction) do kept = kept + 1 end
check(kept == 20000, "the guard runs at login too")

-- The tooltip line. ---------------------------------------------------------------------------------------
local function newTooltip()
  local t = { lines = {}, name = "AuctionTip" }
  function t.AddLine(self, line) self.lines[#self.lines + 1] = line end
  function t.GetOwner(self) return self.owner end
  function t.IsShown() return true end
  function t.Fill(self, id, getter, args, owner)
    self.lines, self.owner = { "item" }, owner
    self.processingInfo = getter and { getterName = getter, getterArgs = args } or nil
    for _, pc in ipairs(postCalls) do pc.fn(self, { id = id, type = 0 }) end
    self.processingInfo = nil
  end
  return t
end
local function ahLine(t)
  for _, l in ipairs(t.lines) do
    local plain = l:gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "")
    if plain:match("^AH: ") then return plain end
  end
end
_G.C_Container = { GetContainerItemInfo = function(_, slot) return { stackCount = slot } end }
p.auction = {
  [4306] = { min = 30000, q25 = 40000, med = 42000, n = 31, q = 400, at = now - 2 * 3600 },
  [1251] = { min = 10, q = 13754, at = now },
  [2140] = { min = 5, q25 = 6, med = 7, n = 1, q = 1, at = now - 3 * 86400 },
}
local tip = newTooltip()
tip:Fill(4306, "GetBagItem", { 0, 1 })
check(ahLine(tip) == "AH: 4g 20s each · 31 listed · 2 hours ago", "one item: " .. tostring(ahLine(tip)))
tip:Fill(4306, "GetBagItem", { 0, 20 })
check(ahLine(tip) == "AH: 4g 20s each (84g for 20) · 31 listed · 2 hours ago", "a stack of 20: "
  .. tostring(ahLine(tip)))
tip:Fill(4306, "GetBagItem", { 0, 99 })
check(ahLine(tip) == "AH: 4g 20s each (84g for 20) · 31 listed · 2 hours ago",
  "a count past the stack size is clamped, as for the sell line: " .. tostring(ahLine(tip)))
check(#tip.lines == 3, "the sell line stays beside it")
tip:Fill(1251, "GetBagItem", { 0, 5 })
check(ahLine(tip) == "AH: from 10c each (50c for 5) · 13754 for sale · just now",
  "browse-only: the lowest price, the quantity for sale (an unloaded item still gets it): " .. tostring(ahLine(tip)))
tip:Fill(2140, nil, nil)
check(ahLine(tip) == "AH: 7c each · 1 listed · 3 days ago", "an item that doesn't stack: " .. tostring(ahLine(tip)))
tip:Fill(9999, nil, nil)
check(ahLine(tip) == nil and #tip.lines == 1, "no entry: no line")
settings.priceAuctionStackTotal = false
tip:Fill(4306, "GetBagItem", { 0, 20 })
check(ahLine(tip) == "AH: 4g 20s each · 31 listed · 2 hours ago", "stack total off: " .. tostring(ahLine(tip)))
settings.priceAuctionStackTotal = nil
settings.priceAuctionLine = false
tip:Fill(4306, "GetBagItem", { 0, 20 })
check(ahLine(tip) == nil and #tip.lines == 2, "auction line off: none (the sell line still)")
settings.priceSellLine = false
tip:Fill(4306, "GetBagItem", { 0, 20 })
check(#tip.lines == 1, "both off: nothing")
settings.priceSellLine = nil
settings.priceAuctionLine = nil
settings.priceSellLine = false
tip:Fill(4306, "GetBagItem", { 0, 20 })
check(ahLine(tip) ~= nil and #tip.lines == 2, "sell line off: the auction line on its own")
settings.priceSellLine = nil

-- Where the auction house shows the price itself: hidden.
local house = _G.AuctionHouseFrame
house.shown = true
local rowButton = shownFrame(shownFrame(house.BrowseResultsFrame))
house.BrowseResultsFrame.shown = true
tip:Fill(4306, nil, nil, rowButton)
check(ahLine(tip) == nil, "a row of the house's own browse list: no line")
A.browsing = { [4306] = true }
tip:Fill(4306, "GetBagItem", { 0, 1 })
check(ahLine(tip) == nil, "an item the browse tab lists now, hovered in the bags: no line")
tip:Fill(2140, "GetBagItem", { 0, 1 })
check(ahLine(tip) ~= nil, "another item, with the house open: the line")
house.BrowseResultsFrame.shown = false
tip:Fill(4306, "GetBagItem", { 0, 1 })
check(ahLine(tip) ~= nil, "the browse tab not showing (another tab): the line")
house.shown = false
house.BrowseResultsFrame.shown = false
A.browsing = {}

-- The snapshot toggle off: no ReplicateItems, no event. -------------------------------------------------------
settings.priceAuctionSnapshot = false
ROWS = makeRows(100)
journal = {}
local asked = calls.replicate
now = now + 3600
openHouse()
fire("REPLICATE_ITEM_LIST_UPDATE")
frames()
check(calls.replicate == asked and #journal == 0 and A.walk == nil,
  "snapshot off: no ReplicateItems, no walk, no event")
closeHouse()
-- Turned off between the request and the rows: no walk either.
settings.priceAuctionSnapshot = nil
now = now + 3600
openHouse()
settings.priceAuctionSnapshot = false
fire("REPLICATE_ITEM_LIST_UPDATE")
frames()
check(#journal == 0 and A.walk == nil, "turned off after asking: nothing read, nothing sent")
closeHouse()
settings.priceAuctionSnapshot = nil

-- /ryna prices and the status line. ---------------------------------------------------------------------
check(#chat == 0, "nothing in chat from any of it")
db.prices.auctionLast = { at = now - 2 * 3600, rows = 192300, items = 8123 }
local lines = statusPanel[1].fn()
check(lines[2] == "Prices: auction line on, snapshots on; last auction snapshot 2 hours ago: 192300 rows, 8123 items",
  "the status line: " .. tostring(lines[2]))
commands.prices.run("", "")
check(#chat == 2 + #ns.Settings.FEATURES + 1, "/ryna prices: the two status lines, the settings, the hint: " .. #chat)
local listed = table.concat(chat, "\n")
check(listed:find("auctionLine: on", 1, true) and listed:find("auctionSnapshot: on", 1, true)
  and listed:find("auctionStackTotal: on", 1, true) and listed:find("192300 rows", 1, true),
  "it lists the three auction settings and the last snapshot")
chat = {}
commands.prices.run("auctionsnapshot", "auctionsnapshot off")
check(settings.priceAuctionSnapshot == false and #chat == 1, "/ryna prices auctionSnapshot off")
check(statusPanel[1].fn()[2]:find("auction line on, snapshots off", 1, true), "the status line says so")
settings.priceAuctionSnapshot = nil
db.prices.auctionLast = nil
check(statusPanel[1].fn()[2] == "Prices: auction line on, snapshots on; no auction snapshot yet", "no snapshot yet")
chat = {}

-- The tab: six checkboxes, the auction stack total under the auction line.
run("tab build", tabs[1].build, {})
run("tab layout", tabs[1].layout, { text = function() end }, { priceAuctionLine = false })
check(#errors == 0, "the tab builds and lays out: " .. table.concat(errors, "; "))

-- A client without REPLICATE_ITEM_LIST_UPDATE: the rows are read a minute after asking. --------------
replicateRegisters = false
load()
A = ns.Auction
check(A.eventOK == false, "the event couldn't be registered")
ROWS = makeRows(300)
openHouse()
check(calls.replicate > 0 and A.walk == nil, "asked; nothing read yet")
advance(59)
check(A.walk == nil, "not before a minute")
advance(1)
frames()
check(A.walk == nil and #journal == 1 and db.prices.auctionLast.rows == 300, "a minute later, the snapshot: "
  .. #journal)
replicateRegisters = true

check(#chat == 0, "nothing in chat")
check(#errors == 0, "no handler raised: " .. table.concat(errors, "; "))
print(string.format("auction: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
