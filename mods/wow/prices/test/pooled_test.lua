-- Prices' lines from everyone's pooled tables (Pooled.lua, Tooltip.lua's VendorLine and
-- PooledAuctionLine) against a fake client: the core's real Data.lua reader over fake
-- RynaData_vendors / RynaData_auctions globals, the addon loaded in TOC order with a stand-in for
-- the rest of the mod API. From the repository root, with Lua 5.1 or later:
--
--   lua5.1 mods/wow/prices/test/pooled_test.lua
--
-- It proves: no lines and a "no pooled prices yet" status when the files are absent; nothing and a
-- "too old" status on a core without Ryna.Data; a table of another version ignored whole and said
-- so; the vendor line's text (cheapest first, "+N more", a bundle's "for N", unconfirmed, other
-- costs); everyone's auction line in its own colour (today's median and age; the week's median
-- when today has none; unconfirmed; the stack's total); nothing while the auction house shows the
-- item; the two toggles; malformed rows skipped; nothing in chat.
-- luacheck: ignore 1

local ADDON = "mods/wow/prices/addon/Ryna_Prices/"
local CORE = "cores/wow/addon/Ryna/"

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

local ITEMS = {
  [2589] = { name = "Linen Cloth", price = 13, stack = 20 },
  [2592] = { name = "Wool Cloth", price = 25, stack = 20 },
  [159] = { name = "Refreshing Spring Water", price = 5, stack = 20 },
  [6948] = { name = "Hearthstone", price = 0, stack = 1 },
  [4306] = { name = "Silk Cloth", price = 1350, stack = 20 },
}
local function link(id) return "|cnIQ2:|Hitem:" .. id .. "::::::::8:1487::::::|h[" .. ITEMS[id].name .. "]|h|r" end
local function getItemInfo(x)
  local id = type(x) == "number" and x or tonumber(tostring(x):match("Hitem:(%d+)"))
  local it = id and ITEMS[id]
  if not it then return end
  return it.name, link(id), 1, 5, 1, "Trade Goods", "Cloth", it.stack, "", 132889, it.price
end

local function money(c)
  local g, s, cc = math.floor(c / 10000), math.floor(c / 100) % 100, c % 100
  local parts = {}
  if g > 0 then parts[#parts + 1] = g .. "g" end
  if s > 0 then parts[#parts + 1] = s .. "s" end
  if cc > 0 or #parts == 0 then parts[#parts + 1] = cc .. "c" end
  return table.concat(parts, " ")
end
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
  local w = {}
  function w.SetChecked() end
  function w.SetScript() end
  function w.place() end
  function w.setActive() end
  function w.SetText() end
  function w.SetWidth() end
  return w
end

-- The core's real Data.lua, loaded into a namespace of its own.
local coreNs = {}
assert(loadfile(CORE .. "Data.lua"))("Ryna", coreNs)

local settings, defaults, registered, statusSlash, postCalls
local function installClient()
  _G.GetLocale = function() return "enUS" end
  _G.SELL_PRICE = "Sell Price"
  _G.C_CurrencyInfo, _G.GetCoinTextureString = nil, nil
  _G.CreateFrame = nil
  _G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) chat[#chat + 1] = msg end }
  _G.C_Item = { GetItemInfo = getItemInfo, RequestLoadItemDataByID = function() end }
  _G.GetItemInfo = nil
  _G.MerchantFrame = nil
  _G.AuctionHouseFrame = nil
  postCalls = {}
  _G.Enum = { TooltipDataType = { Item = 0, Unit = 2 } }
  _G.TooltipDataProcessor = {
    AddTooltipPostCall = function(kind, f) postCalls[#postCalls + 1] = { kind = kind, fn = f } end,
  }
end

local function installApi(withData)
  settings, defaults, registered, statusSlash = {}, {}, nil, {}
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
    MoneyText = money,
    Now = function() return 1700000000 end,
    Ago = function(t) if type(t) ~= "number" then return nil end return (1700000000 - t) .. "s ago" end,
    After = function(_, label, f) run(label, f) end,
    Journal = { Add = function() return {} end, ShortLived = true, Events = function() return {} end,
      Remove = function() end },
    On = function() return true end,
    DB = function() return { settings = settings, prices = { auction = {}, auctionAt = {} } } end,
    RegisterMod = function(spec) registered = spec return true end,
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
      AddTab = function() return true end,
      AddStatusLines = function() end,
      Refresh = function() end,
      RefreshIfShown = function() end,
      Widgets = { WIDTH = 580, text = widget, checkbox = widget, toggle = function() end },
    },
    Slash = {
      Add = function() return true end,
      AddStatusLines = function(_, f) statusSlash[#statusSlash + 1] = f end,
    },
    Data = withData and coreNs.Data or nil,
  }
end

local ns
local function load(withData)
  installClient()
  installApi(withData)
  ns = {}
  local toc = assert(io.open(ADDON .. "Ryna_Prices.toc")):read("*a")
  for line in toc:gmatch("[^\r\n]+") do
    if not line:match("^#") and line:match("%.lua$") then
      assert(loadfile(ADDON .. line:gsub("\\", "/")))("Ryna_Prices", ns)
    end
  end
  if registered and registered.onLogin then run("onLogin", registered.onLogin) end
end

local tooltips = 0
local function newTooltip()
  tooltips = tooltips + 1
  local t = { lines = {}, name = "PooledTip" .. tooltips }
  function t.GetName(self) return self.name end
  function t.NumLines(self) return #self.lines end
  for i = 1, 40 do _G[t.name .. "TextLeft" .. i] = { GetText = function() return t.lines[i] end } end
  function t.AddLine(self, s) self.lines[#self.lines + 1] = s end
  function t.Show(self) self.shown = true end
  function t.IsShown() return true end
  function t.GetItem(self)
    if self.itemId then return ITEMS[self.itemId].name, link(self.itemId), self.itemId end
  end
  function t.GetOwner(self) return self.owner end
  function t.Fill(self, itemId, count, owner)
    self.lines, self.itemId, self.owner = { ITEMS[itemId].name }, itemId, owner or (count and { count = count })
    for _, pc in ipairs(postCalls) do
      if pc.kind == 0 then pc.fn(self, { id = itemId, type = 0 }) end
    end
  end
  return t
end

local function plain(s) return (s:gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "")) end
local function lineStarting(tip, prefix)
  for i = 2, #tip.lines do
    local l = plain(tip.lines[i])
    if l:sub(1, #prefix) == prefix then return l, tip.lines[i] end
  end
  return nil
end
local function status()
  local out = {}
  for _, f in ipairs(statusSlash) do
    for _, l in ipairs(f()) do out[#out + 1] = l end
  end
  return out[#out] -- the pooled piece is the third line
end

-- The tables, as the companion app writes them (version 1).
local function tables(version)
  _G.RynaData_vendors = {
    version = version, writtenAt = 1700000000 - 7200, flavor = "wowf",
    items = {
      [2589] = { { 3881, "Kiro", "Durotar", 20, 1, 2, false }, { 3882, "Grol", "The Barrens", 25, 1, 1, false },
        { 6929, "Innkeeper Gryshka", "Orgrimmar", 30, 1, 1, false } },
      [159] = { { 6929, "Innkeeper Gryshka", "Orgrimmar", 25, 5, 1, true } },
      [4306] = { { 7, "", "", 100, 1, 3, false } },
      [6948] = { "not a row", { "npc?", "x", "y", "z" } },
    },
  }
  _G.RynaData_auctions = {
    version = version, writtenAt = 1700000000 - 3600, flavor = "wowf",
    items = {
      [2589] = { 80, 110, 3, 2, 2, 1700000000 - 1800, 110, 1, 2 },
      [2592] = { 0, 0, 0, 0, 0, 0, 400, 3, 5 },
      [159] = { 8, 0, 1, 1, 1, 1700000000 - 60, 0, 0, 0 },
      [4306] = "bad",
    },
  }
end

-- Absent files: nothing, and the status says to sync. ---------------------------------------------
_G.RynaData_vendors, _G.RynaData_auctions = nil, nil
load(true)
check(not ns.off and registered and registered.id == "prices", "the mod loads")
check(defaults.priceVendorLine == true and defaults.pricePooledAuctionLine == true, "both pooled lines default on")
local tip = newTooltip()
tip:Fill(2589, 1)
check(lineStarting(tip, "Sold by") == nil and lineStarting(tip, "AH (everyone)") == nil,
  "absent files: no pooled lines")
check(lineStarting(tip, "Sell Price") ~= nil, "the sell line is still there")
check(status() == "Prices: no pooled prices yet: sync the companion app, then /reload",
  "status: " .. tostring(status()))

-- A core without Ryna.Data. --------------------------------------------------------------------
tables(1)
load(false)
tip = newTooltip()
tip:Fill(2589, 1)
check(lineStarting(tip, "Sold by") == nil and lineStarting(tip, "AH (everyone)") == nil, "old core: no pooled lines")
check(status() == "Prices: no pooled prices: the Ryna addon is too old for them",
  "old core status: " .. tostring(status()))

-- Another version: ignored whole. -------------------------------------------------------------
tables(2)
load(true)
tip = newTooltip()
tip:Fill(2589, 1)
check(lineStarting(tip, "Sold by") == nil and lineStarting(tip, "AH (everyone)") == nil, "version 2 files: ignored")
check(status() == "Prices: pooled prices from another version: update the companion app and the addon",
  "version status: " .. tostring(status()))
_G.RynaData_vendors.version = 1 -- one table right, the other not: still said
load(true)
check(status() == "Prices: pooled prices from another version: update the companion app and the addon",
  "one table of another version is enough to say so: " .. tostring(status()))

-- The lines. ----------------------------------------------------------------------------------
tables(1)
load(true)
tip = newTooltip()
tip:Fill(2589, 1)
local v = lineStarting(tip, "Sold by")
check(v == "Sold by Kiro, Durotar: 20c (+2 more)", "the cheapest known seller and how many more: " .. tostring(v))
local a, raw = lineStarting(tip, "AH (everyone)")
check(a == "AH (everyone): 1s 10c each · 3 listed · as of 1800s ago",
  "today's median, listings and age: " .. tostring(a))
check(raw and raw:sub(1, 10) == "|cffa8d8ff", "everyone's line is in its own colour: " .. tostring(raw))
check(#tip.lines == 4, "name, sell, vendor, pooled auction: 4 lines, not " .. #tip.lines)
tip:Fill(2589, 20)
a = lineStarting(tip, "AH (everyone)")
check(a == "AH (everyone): 1s 10c each (22s for 20) · 3 listed · as of 1800s ago", "a stack's total: " .. tostring(a))
tip:Fill(159, 1)
v = lineStarting(tip, "Sold by")
check(v == "Sold by Innkeeper Gryshka, Orgrimmar: 25c for 5 · unconfirmed · + other costs",
  "a bundle, one observer, other costs: " .. tostring(v))
a = lineStarting(tip, "AH (everyone)")
check(a == "AH (everyone): 8c each · 1 listed · as of 60s ago · unconfirmed",
  "no median: the min, unconfirmed: " .. tostring(a))
tip:Fill(2592, 1)
check(lineStarting(tip, "Sold by") == nil, "nobody sells Wool")
a = lineStarting(tip, "AH (everyone)")
check(a == "AH (everyone): 4s each this week · 5 snapshots", "only the week: " .. tostring(a))
tip:Fill(4306, 1)
v = lineStarting(tip, "Sold by")
check(v == "Sold by a vendor: 1s", "a nameless vendor, no zone, three observers: " .. tostring(v))
check(lineStarting(tip, "AH (everyone)") == nil, "a malformed auction row is skipped")
tip:Fill(6948, 1)
check(lineStarting(tip, "Sold by") == nil, "malformed vendor rows are skipped")
check(status() == "Prices: pooled prices as of 7200s ago: 4 items with a vendor, 4 with auction prices",
  "status counts the tables and takes the older age: " .. tostring(status()))

-- The auction house showing the item: everyone's line stays away like the own one. ------------
_G.AuctionHouseFrame = { IsShown = function() return true end }
tip = newTooltip()
tip:Fill(2589, 1, _G.AuctionHouseFrame)
check(lineStarting(tip, "AH (everyone)") == nil, "no pooled auction line while the house shows the item")
check(lineStarting(tip, "Sold by") ~= nil, "the vendor line still shows there")
_G.AuctionHouseFrame = nil

-- The toggles. ----------------------------------------------------------------------------------
settings.priceVendorLine = false
tip = newTooltip()
tip:Fill(2589, 1)
check(lineStarting(tip, "Sold by") == nil and lineStarting(tip, "AH (everyone)") ~= nil, "vendor line off")
settings.priceVendorLine, settings.pricePooledAuctionLine = true, false
tip = newTooltip()
tip:Fill(2589, 1)
check(lineStarting(tip, "Sold by") ~= nil and lineStarting(tip, "AH (everyone)") == nil, "pooled auction line off")
settings.pricePooledAuctionLine, settings.priceAuctionStackTotal = true, false
tip = newTooltip()
tip:Fill(2589, 20)
a = lineStarting(tip, "AH (everyone)")
check(a == "AH (everyone): 1s 10c each · 3 listed · as of 1800s ago",
  "no stack total when that is off: " .. tostring(a))

-- Reading the tables never touched the stores, and nothing was said. ---------------------------
check(#chat == 0, "nothing in chat: " .. table.concat(chat, " | "))
check(#errors == 0, "no errors: " .. table.concat(errors, " | "))

print(string.format("pooled_test: %d checks, %d failures", checks, failures))
os.exit(failures == 0 and 0 or 1)
