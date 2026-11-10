-- Items: what every character on the account holds, from the saved file (each record's `containers`,
-- what Containers.lua records: bags and keyring at each login, the bank as of its last visit).
-- Nothing new is recorded here.
--
--   I.Index()                 item id (a string) -> { id, name, quality, link, total, bags, bank,
--                             holders = { [guid] = { bags, bank } } }, rebuilt only when a record's
--                             containers changed (Containers.lua's version) or the account did
--   I.TooltipLine(id)         the one tooltip line naming the other characters that hold the item,
--                             "Alyra: 120 in the bank · Brennic: 34", or nil (Tooltip.lua adds
--                             it, with the `tooltips` and `tooltipHolders` settings on)
--
-- And the Items section of Ryna's window (under Me): a search across every character's bags and bank,
-- one row per item with its total, how many in bags and in the bank, and who holds it; the tooltip is
-- the game's own item tooltip with each holder's numbers under it. With a character picked, only that
-- character's items.
--
-- Only this account's own characters are ever named; nothing about another player is here.

local _, ns = ...
if ns.off then return end
local L = ns.L

local I = {}
ns.Items = I

local MAX_HOLDERS = 5
local SEP = " · "

ns.API.Settings.AddDefaults({ tooltipHolders = true })

local function num(v)
  if type(v) == "number" then return v end
  return tonumber(v)
end

local function plain(s)
  return ns.Status.Plain(s) or s
end

local function itemIdOf(row)
  local id = num(row.itemId)
  if not id and type(row.link) == "string" then id = tonumber(row.link:match("item:(%d+)")) end
  return id and tostring(id) or nil
end

local function linkName(link)
  if type(link) ~= "string" or ns.IsSecret(link) then return nil end
  return link:match("|h%[(.-)%]|h")
end

-- Each character's short name for the tooltip line: its first name, or its full name when another
-- character on the account shares the first name (or it has none).
local function shortNames(chars)
  local count, out = {}, {}
  for _, rec in pairs(chars) do
    if type(rec) == "table" and ns.NonEmpty(rec.firstName) then
      count[rec.firstName] = (count[rec.firstName] or 0) + 1
    end
  end
  for guid, rec in pairs(chars) do
    if type(rec) == "table" then
      local first = ns.NonEmpty(rec.firstName)
      out[guid] = plain((first and count[first] == 1 and first) or ns.NonEmpty(rec.fullName) or first)
        or L["Unknown character"]
    end
  end
  return out
end

local cache = { db = nil, key = nil, items = {}, names = {} }

local function cacheKey(chars)
  local n = 0
  for _ in pairs(chars) do n = n + 1 end
  return tostring((ns.Containers and ns.Containers.version) or 0) .. ":" .. n
end

function I.Index()
  local db = ns.db
  local chars = db and type(db.characters) == "table" and db.characters or nil
  if not chars then return {} end
  local key = cacheKey(chars)
  if cache.db == db and cache.key == key then return cache.items end
  cache.db, cache.key = db, key
  local items = {}
  for guid, rec in pairs(chars) do
    if type(rec) == "table" and type(rec.containers) == "table" then
      for _, row in ipairs(rec.containers) do
        local id = type(row) == "table" and itemIdOf(row)
        if id then
          local e = items[id]
          if not e then
            e = { id = id, total = 0, bags = 0, bank = 0, holders = {} }
            items[id] = e
          end
          local link = ns.NonEmpty(row.link)
          if link and not e.link then
            e.link = link
            e.name = linkName(link)
          end
          if e.quality == nil then e.quality = num(row.quality) end
          local n = num(row.count) or 1
          local where = row.container == "bank" and "bank" or "bags"
          local h = e.holders[guid]
          if not h then
            h = { bags = 0, bank = 0 }
            e.holders[guid] = h
          end
          h[where] = h[where] + n
          e[where] = e[where] + n
          e.total = e.total + n
        end
      end
    end
  end
  cache.items = items
  cache.names = shortNames(chars)
  return items
end

function I.Entry(id)
  if id == nil then return nil end
  return I.Index()[tostring(id)]
end

-- The holders of an entry, most first: { guid, name, bags, bank, n }, all of them or all but `except`.
function I.Holders(e, except)
  local list = {}
  if not e then return list end
  for guid, h in pairs(e.holders) do
    if guid ~= except then
      list[#list + 1] = { guid = guid, name = cache.names[guid] or L["Unknown character"], bags = h.bags,
        bank = h.bank, n = h.bags + h.bank }
    end
  end
  table.sort(list, function(a, b)
    if a.n ~= b.n then return a.n > b.n end
    return a.name < b.name
  end)
  return list
end

local function number(n)
  return ns.Session.Number(n)
end

-- "120 in the bank", "34", "34 (120 in the bank)".
local function holderText(h)
  if h.bank > 0 and h.bags > 0 then
    return string.format(L["%s (%s in the bank)"], number(h.n), number(h.bank))
  elseif h.bank > 0 then
    return string.format(L["%s in the bank"], number(h.bank))
  end
  return number(h.bags)
end

-- The tooltip line: the other characters (not the one you're on) holding item `id`, most first.
function I.TooltipLine(id)
  if not ns.Setting("tooltipHolders") then return nil end
  -- Inside the Items section the window lists every holder under the item already.
  local RW = ns.API.Window
  if type(RW) == "table" and RW.TooltipSection and RW.TooltipSection() == "items" then return nil end
  local e = I.Entry(id)
  if not e then return nil end
  local holders = I.Holders(e, ns.rec and ns.rec.guid)
  if #holders == 0 then return nil end
  local parts = {}
  for i = 1, math.min(#holders, MAX_HOLDERS) do
    parts[i] = string.format(L["%s: %s"], holders[i].name, holderText(holders[i]))
  end
  local line = table.concat(parts, SEP)
  if #holders > MAX_HOLDERS then line = line .. SEP .. string.format(L["%d more"], #holders - MAX_HOLDERS) end
  return line
end

-- The section -----------------------------------------------------------------------------------------

local Wn = ns.Window
if not (Wn and Wn.available) then return end
local RW = ns.API.Window

local ALL_COLUMNS = {
  { title = L["Item"], width = 210 }, { title = L["Total"], width = 60, justify = "RIGHT" },
  { title = L["In bags"], width = 66, justify = "RIGHT" }, { title = L["In the bank"], width = 80, justify = "RIGHT" },
  { title = L["Characters"], width = 170 },
}
local ONE_COLUMNS = {
  { title = L["Item"], width = 260 }, { title = L["Total"], width = 70, justify = "RIGHT" },
  { title = L["In bags"], width = 80, justify = "RIGHT" }, { title = L["In the bank"], width = 90, justify = "RIGHT" },
}

local totals = {}

function I.Rows(ctx)
  local rows = {}
  local guid = ctx.character
  totals.character, totals.kinds, totals.bags, totals.bank = guid, 0, 0, 0
  for id, e in pairs(I.Index()) do
    local bags, bank
    if guid then
      local h = e.holders[guid]
      if h then bags, bank = h.bags, h.bank end
    else
      bags, bank = e.bags, e.bank
    end
    if bags then
      totals.kinds = totals.kinds + 1
      totals.bags, totals.bank = totals.bags + bags, totals.bank + bank
      local name = e.name or string.format(L["item %s"], id)
      if RW.Match(name, ctx.search) then
        rows[#rows + 1] = { ref = id, e = e, link = e.link, quality = e.quality, bags = bags, bank = bank,
          sort = { name:lower(), bags + bank, bags, bank } }
      end
    end
  end
  table.sort(rows, function(a, b)
    if a.sort[1] ~= b.sort[1] then return a.sort[1] < b.sort[1] end
    return a.ref < b.ref
  end)
  return rows
end

function I.Fill(row, ctx)
  local e = row.e
  row.icon = RW.ItemIcon(row.ref)
  local who = ""
  if ctx.character == nil then
    local names = {}
    for i, h in ipairs(I.Holders(e)) do
      if i > 3 then
        names[#names + 1] = string.format(L["%d more"], #I.Holders(e) - 3)
        break
      end
      names[#names + 1] = h.name
    end
    who = table.concat(names, ", ")
  end
  row.cells = {
    plain(e.name or string.format(L["item %s"], row.ref)), number(row.bags + row.bank),
    row.bags > 0 and number(row.bags) or "", row.bank > 0 and number(row.bank) or "", who,
  }
end

-- Under the game's item tooltip: each holder's bags and bank, the bank's age where it isn't now.
function I.Tooltip(row, tip, ctx, hadLink)
  local e = row.e
  if not hadLink then tip:AddLine(RW.QualityColor(e.quality) .. plain(e.name or row.ref) .. "|r") end
  tip:AddLine(" ")
  local db = ns.db
  for _, h in ipairs(I.Holders(e)) do
    if ctx.character == nil or ctx.character == h.guid then
      local rec = db and db.characters and db.characters[h.guid]
      local where = {}
      if h.bags > 0 then where[#where + 1] = string.format(L["%s in bags"], number(h.bags)) end
      if h.bank > 0 then
        local age = type(rec) == "table" and num(rec.bankAsOf) and ns.Status.Ago(rec.bankAsOf)
        where[#where + 1] = age and string.format(L["%s in the bank (as of %s)"], number(h.bank), age)
          or string.format(L["%s in the bank"], number(h.bank))
      end
      local color = type(rec) == "table" and RW.ClassColor(rec.class) or "|cffffffff"
      tip:AddDoubleLine(color .. h.name .. "|r", table.concat(where, SEP), 1, 1, 1, 1, 1, 1)
    end
  end
end

function I.Tiles(ctx)
  if totals.character ~= ctx.character then return nil end
  local tiles = {
    { label = L["Kinds of item"], value = number(totals.kinds) },
    { label = L["In bags"], value = number(totals.bags) },
    { label = L["In the bank"], value = number(totals.bank) },
  }
  local rec = Wn.Record(ctx.character)
  if rec then
    tiles[4] = { label = L["Bank read"], value = num(rec.bankAsOf) and ns.Status.Ago(rec.bankAsOf) or L["never"] }
  end
  return tiles
end

function I.Footer(ctx)
  if ctx.character == nil then
    return L["From your saved file: bags as of each character's last login, the bank as of its last visit."]
  end
  local rec = Wn.Record(ctx.character)
  if not rec then return "" end
  local bank = num(rec.bankAsOf) and ns.Status.Ago(rec.bankAsOf) or L["never: open the bank once"]
  if ctx.character == ctx.current then
    return string.format(L["Bags as they are now; the bank as of your last visit (%s)."], bank)
  end
  return string.format(L["From your saved file: %s's bags as of %s, the bank as of %s."], Wn.Name(rec),
    ns.Status.Ago(rec.lastSeen) or L["an unknown time"], bank)
end

if RW.AddSection({
  key = "items", group = "me", order = 30, label = L["Items"], icons = true,
  columns = function(ctx) return ctx.character == nil and ALL_COLUMNS or ONE_COLUMNS end,
  version = function() return (ns.Containers and ns.Containers.version) or 0 end,
  rows = I.Rows, fill = I.Fill, tiles = I.Tiles, tooltip = I.Tooltip, footer = I.Footer,
  empty = L["Nothing in the bags or bank yet. Each character's are recorded at login."],
}) then
  Wn.keys.items = true
end
