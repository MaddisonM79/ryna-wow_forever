-- Pooled: everyone's prices, as the companion app brings them into the game (the data layer,
-- docs/ryna-agent-cores-mods.md 7 and 7.3; the API's docs/wow.md 17.2). After a sync the app writes
-- Interface/AddOns/Ryna/Data/vendors.lua and Data/auctions.lua, which the core addon loads at login
-- and /reload and hands out through Ryna.Data.Get(name, version). This file reads the two tables
-- Prices knows (version 1) once at login and answers the tooltip's questions:
--
--   vendors[itemId]  = { { npcId, name, zone, price, stack, observers, currency }, ... }  best first
--   auctions[itemId] = { min, med, listings, snapshots, observers, asOf, weekMed, weekDays, weekSnapshots }
--
-- A table from another version (an older app, or a newer one than this addon) is ignored whole, and
-- /ryna prices says so; a missing one reads as "no pooled prices yet". A core without Ryna.Data (one
-- from before the data layer) gives nothing, and says that instead. Every row is checked for shape
-- before it is used: the file is the app's, but a half-written or hand-edited one must not error a
-- tooltip. Nothing here is about a person: NPCs, zones, prices and counts.

local _, ns = ...
if ns.off then return end

local P = {}
ns.Pooled = P

P.VERSION = 1 -- the tables' version this addon reads (the app's prices_data::VERSION)

-- At login: the two tables, or why not ("core": no Ryna.Data; "absent"; "version").
function P.Load()
  local D = ns.API.Data
  P.vendors, P.auctions = nil, nil
  if type(D) ~= "table" or type(D.Get) ~= "function" then
    P.vendorsWhy, P.auctionsWhy = "core", "core"
    return
  end
  P.vendors, P.vendorsWhy = D.Get("vendors", P.VERSION)
  P.auctions, P.auctionsWhy = D.Get("auctions", P.VERSION)
end

local function isNumber(v)
  return type(v) == "number"
end

-- A vendor row of the file, as a table with named fields, or nil when it isn't one.
local function vendorOf(row)
  if type(row) ~= "table" or not isNumber(row[1]) or not isNumber(row[4]) then return nil end
  return {
    npcId = row[1],
    name = type(row[2]) == "string" and row[2] or "",
    zone = type(row[3]) == "string" and row[3] or "",
    price = row[4],
    stack = isNumber(row[5]) and row[5] > 0 and row[5] or 1,
    observers = isNumber(row[6]) and row[6] or 0,
    currency = row[7] == true,
  }
end

-- The vendors known to sell item `id`, best first (confirmed by two people, then copper-only, then
-- cheapest per piece, as the API orders them), or nil when the table has none.
function P.Vendors(id)
  local t = P.vendors
  if not t or type(id) ~= "number" then return nil end
  local rows = t.items[id]
  if type(rows) ~= "table" then return nil end
  local out = {}
  for i = 1, #rows do
    local v = vendorOf(rows[i])
    if v then out[#out + 1] = v end
  end
  if #out == 0 then return nil end
  return out
end

-- Everyone's auction numbers for item `id`, or nil when the table has none.
function P.Auction(id)
  local t = P.auctions
  if not t or type(id) ~= "number" then return nil end
  local row = t.items[id]
  if type(row) ~= "table" then return nil end
  local a = {}
  for i, key in ipairs({ "min", "med", "listings", "snapshots", "observers", "asOf", "weekMed", "weekDays",
    "weekSnapshots" }) do
    a[key] = isNumber(row[i]) and row[i] or 0
  end
  if a.asOf <= 0 and a.weekDays <= 0 then return nil end
  return a
end

-- When the tables were written (the older of the two), or nil.
function P.WrittenAt()
  local D = ns.API.Data
  if type(D) ~= "table" or type(D.WrittenAt) ~= "function" then return nil end
  local a, b = D.WrittenAt(P.vendors), D.WrittenAt(P.auctions)
  if a and b then return math.min(a, b) end
  return a or b
end

-- The status line's piece: "pooled prices last changed 2 hours ago: 312 items with a vendor, 1204 with auction
-- prices" (the app rewrites a table only when its rows change, so the age is the data's, not the sync's).
function P.Status()
  local L = ns.L
  if P.vendorsWhy == "core" then return L["no pooled prices: the Ryna addon is too old for them"] end
  if P.vendorsWhy == "version" or P.auctionsWhy == "version" then
    return L["pooled prices from another version: update the companion app and the addon"]
  end
  if not P.vendors and not P.auctions then return L["no pooled prices yet: sync the companion app, then /reload"] end
  local D = ns.API.Data
  return string.format(L["pooled prices last changed %s: %d items with a vendor, %d with auction prices"],
    ns.Ago(P.WrittenAt()) or "?", D.Count(P.vendors), D.Count(P.auctions))
end
