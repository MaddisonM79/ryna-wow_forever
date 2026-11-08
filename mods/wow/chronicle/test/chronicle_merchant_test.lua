-- Chronicle's `merchant` journal event against a fake client (no game needed): loads Chronicle's
-- namespace (Mod.lua), its strings, Merchants.lua and the Recording tab with a stand-in for the core's
-- mod API, opens fake merchants and checks what was written. From the repository root, with Lua 5.1
-- (what the game runs) or later:
--
--   lua5.1 mods/wow/chronicle/test/chronicle_merchant_test.lua
--
-- This proves the logic (the event's shape, currencies only when there are any, once per NPC, the
-- 200-item cap, waiting for MERCHANT_UPDATE when links haven't loaded, and the `merchants` setting);
-- it can't prove the client's merchant API returns what's assumed. That's the game's.
-- luacheck: ignore 1

local ADDON = "mods/wow/chronicle/addon/Ryna_Chronicle/"

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end

local errors, chat, journal = {}, {}, {}
local function run(label, fn, ...)
  local ok, err = pcall(fn, ...)
  if not ok then errors[#errors + 1] = label .. ": " .. tostring(err) end
end

-- The merchant the fake client shows: { guid, name, items = { { id, price, stack, available, cost } } }.
local SHOP
local unloaded = {} -- slot -> true: the link hasn't loaded yet
-- Forever's links: `|cnIQ2:` colours, not `|cffxxxxxx`.
local function itemLink(id) return "|cnIQ2:|Hitem:" .. id .. "::::::::8:1487::::::|h[Item " .. id .. "]|h|r" end

_G.GetLocale = function() return "enUS" end
_G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) chat[#chat + 1] = msg end }
_G.UnitGUID = function(unit) if unit == "npc" and SHOP then return SHOP.guid end end
_G.UnitName = function(unit) if unit == "npc" and SHOP then return SHOP.name end end
_G.GetRealZoneText = function() return "Orgrimmar" end
_G.GetSubZoneText = function() return "Valley of Strength" end
_G.GetMerchantNumItems = function() return SHOP and #SHOP.items or 0 end
_G.GetMerchantItemLink = function(i)
  local it = SHOP and SHOP.items[i]
  if it and not unloaded[i] then return itemLink(it.id) end
end
-- The classic order: name, texture, price, quantity, numAvailable, isUsable, extendedCost.
_G.GetMerchantItemInfo = function(i)
  local it = SHOP and SHOP.items[i]
  if it then return "Item " .. it.id, 133784, it.price, it.stack, it.available, true, it.cost ~= nil end
end
_G.GetMerchantItemCostInfo = function(i)
  local it = SHOP and SHOP.items[i]
  return it and it.cost and #it.cost or 0
end
_G.GetMerchantItemCostItem = function(i, k)
  local c = SHOP.items[i].cost[k]
  return 1455228, c.amount, c.link, c.name
end

-- The core's mod API, as much of it as these files use.
local handlers, settings, defaults, tabs, boxes = {}, {}, {}, {}, {}
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
  function w.SetPoint() end
  function w.ClearAllPoints() end
  function w.SetAlpha() end
  return w
end
local db = { settings = settings, journal = journal }
_G.Ryna = {
  API = 1, VERSION = "test", L = setmetatable({}, { __index = function(_, k) return k end }),
  Try = try, Fn = fn,
  IsSecret = function() return false end,
  NonEmpty = function(s) if type(s) == "string" and s ~= "" then return s end end,
  Locale = function() return "enUS" end,
  Print = function(m) chat[#chat + 1] = "Ryna: " .. m end,
  Debug = function() end,
  Run = run,
  ReportError = function(label, err) errors[#errors + 1] = label .. ": " .. tostring(err) end,
  On = function(event, f)
    handlers[event] = handlers[event] or {}
    table.insert(handlers[event], f)
    return true
  end,
  DB = function() return db end,
  RegisterMod = function() return true end,
  Journal = {
    Add = function(eventType, fields)
      local e = { type = eventType }
      for k, v in pairs(fields or {}) do e[k] = v end
      journal[#journal + 1] = e
      return e
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
    Refresh = function() end,
    Widgets = {
      WIDTH = 580,
      text = function() return widget() end,
      checkbox = function(_, label)
        local w = widget()
        w.label = label
        boxes[#boxes + 1] = w
        return w
      end,
      button = function() return widget() end,
      toggle = function(cb, key) cb.key = key end,
    },
  },
}

local ns = {}
for _, file in ipairs({ "Mod.lua", "Locales/enUS.lua", "Merchants.lua", "Tabs.lua" }) do
  local chunk = assert(loadfile(ADDON .. file))
  chunk("Ryna_Chronicle", ns)
end

local function fire(event, ...)
  for _, f in ipairs(handlers[event] or {}) do run(event, f, event, ...) end
end
local function visit(shop)
  SHOP = shop
  fire("MERCHANT_SHOW")
end
local function leave()
  fire("MERCHANT_CLOSED")
  SHOP = nil
end
local function shop(guid, name, items) return { guid = guid, name = name, items = items } end
local function one(id, price) return { { id = id, price = price, stack = 1, available = -1 } } end
local function merchants()
  local out = {}
  for _, e in ipairs(journal) do if e.type == "merchant" then out[#out + 1] = e end end
  return out
end

check(not ns.off and ns.Merchants ~= nil, "Merchants.lua loads")
check(defaults.merchants == true, "merchants is on by default")
check(handlers.MERCHANT_SHOW and handlers.MERCHANT_UPDATE and handlers.MERCHANT_CLOSED, "the three merchant events")

-- No character record yet (before login): nothing.
visit(shop("Creature-0-1-1-1-3933-0000000001", "Innkeeper Gryshka", one(4540, 25)))
leave()
check(#merchants() == 0, "before the record exists: nothing")

ns.rec = { zone = "Orgrimmar" }

-- The event's shape. -----------------------------------------------------------------------------
local gryshka = {
  guid = "Creature-0-1-1-1-6929-0000000002", name = "Innkeeper Gryshka",
  items = {
    { id = 4540, price = 25, stack = 1, available = -1 },
    { id = 159, price = 25, stack = 5, available = 3 },
    { id = 2901, price = 0, stack = 1, available = 0 },
    { id = 18839, price = 900, stack = 1, available = -1, cost = {
      { amount = 75, name = "Honor Points", link = "|cff00ff00|Hcurrency:1901|h[Honor Points]|h|r" },
      { amount = 2, name = "Alterac Valley Mark of Honor", link = itemLink(20560) },
    } },
  },
}
visit(gryshka)
local list = merchants()
check(#list == 1, "opening a merchant writes one merchant event")
local e = list[1] or { items = {} }
check(e.npc == gryshka.guid and e.name == "Innkeeper Gryshka", "npc is the GUID and name the NPC's name")
check(e.zone == "Orgrimmar" and e.subzone == "Valley of Strength", "zone and subzone")
check(#e.items == 4, "every item: " .. #e.items)
local a, b, c, d = e.items[1] or {}, e.items[2] or {}, e.items[3] or {}, e.items[4] or {}
check(a.id == 4540 and a.price == 25 and a.stack == 1 and a.limited == false and a.currency == nil,
  "a plain item: id from the link, price, stack, unlimited, no currency")
check(b.id == 159 and b.stack == 5 and b.limited == true, "a limited bundle of 5")
check(c.limited == true, "sold out (0 available) is still limited")
check(type(d.currency) == "table" and #d.currency == 2, "an item that costs more than copper lists its currencies")
check(d.currency and d.currency[1].amount == 75 and d.currency[1].currencyId == 1901
  and d.currency[1].name == "Honor Points" and d.currency[1].itemId == nil, "a currency: amount, id and name")
check(d.currency and d.currency[2].itemId == 20560 and d.currency[2].amount == 2, "an item as a cost: its item id")
check(d.price == 900, "and its copper part")
fire("MERCHANT_UPDATE")
check(#merchants() == 1, "MERCHANT_UPDATE in the same visit writes nothing more")
leave()

-- Once per NPC. --------------------------------------------------------------------------------
visit(gryshka)
fire("MERCHANT_UPDATE")
leave()
check(#merchants() == 1, "the same NPC again: nothing (once per NPC per login)")
local other = shop("Creature-0-1-1-1-3342-0000000003", "Shan'ti", one(2320, 10))
visit(other)
leave()
check(#merchants() == 2 and merchants()[2].npc == other.guid, "another NPC: its own event")

-- Links that haven't loaded: wait for MERCHANT_UPDATE. ------------------------------------------
local slow = { guid = "Creature-0-1-1-1-3881-0000000004", name = "Grimtak", items = {
  { id = 3173, price = 4, stack = 1, available = -1 }, { id = 2672, price = 4, stack = 1, available = -1 } } }
unloaded[2] = true
visit(slow)
check(#merchants() == 2, "a link not loaded at MERCHANT_SHOW: nothing yet")
unloaded[2] = nil
fire("MERCHANT_UPDATE")
check(#merchants() == 3 and #merchants()[3].items == 2, "the first MERCHANT_UPDATE writes it, with every item")
fire("MERCHANT_UPDATE")
check(#merchants() == 3, "later updates write nothing")
leave()

-- Still not loaded at the update: what's there.
local partial = { guid = "Creature-0-1-1-1-3882-0000000005", name = "Zansoa", items = {
  { id = 6256, price = 5, stack = 1, available = -1 }, { id = 6530, price = 1, stack = 1, available = -1 } } }
unloaded[1] = true
visit(partial)
fire("MERCHANT_UPDATE")
check(#merchants() == 4 and #merchants()[4].items == 1 and merchants()[4].items[1].id == 6530,
  "a link still missing at the update: the items that loaded")
unloaded[1] = nil
leave()

-- The cap. -------------------------------------------------------------------------------------
local big = { guid = "Creature-0-1-1-1-9999-0000000006", name = "Big Shop", items = {} }
for i = 1, 250 do big.items[i] = { id = 10000 + i, price = i, stack = 1, available = -1 } end
visit(big)
leave()
check(#merchants() == 5 and #merchants()[5].items == 200,
  "at most 200 items: " .. #(merchants()[5] or { items = {} }).items)

-- C_MerchantFrame.GetItemInfo's named fields, where the client has it. ----------------------------
_G.C_MerchantFrame = {
  GetItemInfo = function(i)
    local it = SHOP.items[i]
    return { name = "x", price = it.price, stackCount = it.stack, numAvailable = it.available }
  end,
}
_G.GetMerchantItemInfo = function() error("not used when C_MerchantFrame is there") end
visit(shop("Creature-0-1-1-1-7777-0000000007", "Modern", { { id = 4600, price = 50, stack = 20, available = 4 } }))
leave()
local m = merchants()[6] or { items = { {} } }
check(m.items[1].price == 50 and m.items[1].stack == 20 and m.items[1].limited == true,
  "C_MerchantFrame.GetItemInfo is read")
_G.C_MerchantFrame = nil

-- Nothing to go by. ------------------------------------------------------------------------------
visit(shop(nil, "Nobody", one(1, 1)))
leave()
visit({ guid = "Creature-0-1-1-1-1-0000000008", name = "Empty", items = {} })
fire("MERCHANT_UPDATE")
leave()
check(#merchants() == 6, "no GUID, or nothing listed: nothing")

-- The setting off. -------------------------------------------------------------------------------
settings.merchants = false
visit(shop("Creature-0-1-1-1-2-0000000009", "Off", one(1, 1)))
fire("MERCHANT_UPDATE")
leave()
check(#merchants() == 6, "merchants off: nothing recorded")
settings.merchants = nil
visit(shop("Creature-0-1-1-1-2-0000000009", "Off", one(1, 1)))
leave()
check(#merchants() == 7, "on again: the NPC skipped while off is recorded at its next visit")

-- The Recording tab's switch. ---------------------------------------------------------------------
local recording
for _, t in ipairs(tabs) do if t.key == "recording" then recording = t end end
check(recording ~= nil, "the Recording tab")
local stack = { y = 0 }
function stack.text() end
function stack.add() end
run("recording build", recording.build, {})
run("recording layout", recording.layout, stack, { merchants = true, chronicle = true, tooltips = true })
check(#errors == 0, "the Recording tab builds and lays out: " .. table.concat(errors, "; "))
local box
for _, w in ipairs(boxes) do if w.key == "merchants" then box = w end end
check(box and box.label == "Record the merchants I visit" and box.checked == true and box.placed,
  "Record the merchants I visit: its box, ticked and placed")

check(#chat == 0, "nothing in chat")
check(#errors == 0, "no handler raised: " .. table.concat(errors, "; "))
print(string.format("chronicle merchants: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
