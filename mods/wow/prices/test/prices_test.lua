-- Prices' vendor line against a fake client (no game needed): loads the addon's files in TOC order
-- with a stand-in for the core's mod API and the client functions it calls, shows items on fake
-- tooltips and checks the lines. From the repository root, with Lua 5.1 (what the game runs) or later:
--
--   lua5.1 mods/wow/prices/test/prices_test.lua
--
-- This proves the logic (the line's text for one item and a stack, with the client's SELL_PRICE label
-- and coin icons or the fallbacks; the stack's count from the tooltip's getter (bag, bank, loot, loot
-- roll, quest, mail, trade, merchant, buyback) or the owner button, clamped to a stack, and one item's
-- price for a link or an unknown count; the post call's data before GetItem; no line twice; no line
-- where the game drew its own price; nothing for items vendors won't buy; an uncached item asked for
-- and refreshed on ITEM_DATA_LOAD_RESULT or GET_ITEM_INFO_RECEIVED, those events registered only while
-- it waits; the merchant-window setting, the toggles, /ryna prices, and nothing in chat unasked); it
-- can't prove the client's API behaves as assumed. That's the game's.
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

-- The client's items: id -> { name, price, stack, cached }.
local ITEMS
local loads = {} -- the ids C_Item.RequestLoadItemDataByID was asked for
local function resetItems()
  ITEMS = {
    [2589] = { name = "Linen Cloth", price = 13, stack = 20, cached = true },
    [4306] = { name = "Silk Cloth", price = 1350, stack = 20, cached = true },
    [6948] = { name = "Hearthstone", price = 0, stack = 1, cached = true },
    [2140] = { name = "Carving Knife", price = 7, stack = 1, cached = true },
    [9999] = { name = "Unseen Thing", price = 250, stack = 5, cached = false },
    [5000] = { name = "No Price Given", price = nil, stack = 1, cached = true },
  }
  loads = {}
end
-- Forever's links: `|cnIQ2:` colours, not `|cffxxxxxx`.
local function link(id) return "|cnIQ2:|Hitem:" .. id .. "::::::::8:1487::::::|h[" .. ITEMS[id].name .. "]|h|r" end
local function idOf(x)
  if type(x) == "number" then return x end
  return tonumber(tostring(x):match("Hitem:(%d+)"))
end

local function getItemInfo(x)
  local id = idOf(x)
  local it = id and ITEMS[id]
  -- Forever answers nothing at all (no values, not a nil) for an item it hasn't loaded, and asking
  -- again doesn't load it.
  if not it or not it.cached then return end
  return it.name, link(id), 1, 5, 1, "Trade Goods", "Cloth", it.stack, "", 132889, it.price
end

-- Bags (C_Container), loot, and inventory slots (the bank's main slots among them).
local BAGS, LOOT, INVENTORY = {}, {}, {}

-- Frames (CreateFrame): the events each has registered, and its scripts.
local frames = {}
local function createFrame()
  local f = { events = {}, scripts = {} }
  function f.RegisterEvent(self, e) self.events[e] = true end
  function f.UnregisterEvent(self, e) self.events[e] = nil end
  function f.SetScript(self, k, h) self.scripts[k] = h end
  frames[#frames + 1] = f
  return f
end

local postCalls = {}
local function installClient(processor)
  _G.GetLocale = function() return "enUS" end
  _G.SELL_PRICE = "Sell Price" -- the client's GlobalStrings
  _G.C_CurrencyInfo, _G.GetCoinTextureString = nil, nil -- coin icons: tested on their own below
  frames = {}
  _G.CreateFrame = function() return createFrame() end
  _G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) chat[#chat + 1] = msg end }
  _G.C_Item = {
    GetItemInfo = getItemInfo,
    RequestLoadItemDataByID = function(id) loads[#loads + 1] = id end,
  }
  _G.GetItemInfo = nil -- Forever has none
  _G.C_Container = {
    GetContainerItemInfo = function(bag, slot)
      local n = BAGS[bag] and BAGS[bag][slot]
      if n then return { stackCount = n } end
    end,
  }
  _G.GetLootSlotInfo = function(slot) if LOOT[slot] then return "icon", "Item", LOOT[slot], nil, 1 end end
  _G.GetInventoryItemCount = function(_, slot) return INVENTORY[slot] end
  _G.MerchantFrame = { shown = false, IsShown = function(self) return self.shown end }
  postCalls = {}
  if processor then
    _G.Enum = { TooltipDataType = { Item = 0, Unit = 2 } }
    _G.TooltipDataProcessor = {
      AddTooltipPostCall = function(kind, fn) postCalls[#postCalls + 1] = { kind = kind, fn = fn } end,
    }
  else
    _G.Enum, _G.TooltipDataProcessor = nil, nil
  end
end

-- A tooltip: what the client puts on it, and the getter it was filled by. Named, with its
-- <name>TextLeft<i> regions, as the game's tooltips are.
local tooltips = 0
local function newTooltip(refresh)
  tooltips = tooltips + 1
  local t = { lines = {}, shown = false, shows = 0, scripts = {}, refreshes = 0, name = "FakeTooltip" .. tooltips }
  function t.GetName(self) return self.name end
  function t.NumLines(self) return #self.lines end
  for i = 1, 40 do _G[t.name .. "TextLeft" .. i] = { GetText = function() return t.lines[i] end } end
  function t.AddLine(self, s) self.lines[#self.lines + 1] = s end
  function t.Show(self) self.shown = true; self.shows = self.shows + 1 end
  function t.IsShown(self) return self.shown end
  function t.GetItem(self)
    if self.itemId then return ITEMS[self.itemId].name, link(self.itemId), self.itemId end
  end
  function t.GetOwner(self) return self.owner end
  function t.HookScript(self, ev, f) self.scripts[ev] = f end
  if refresh then
    function t.RefreshData(self)
      self.refreshes = self.refreshes + 1
      self:Fill(self.itemId, self.getter, self.args, self.owner)
    end
  end
  return t
end

local function plain(s)
  return (s:gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""))
end

-- The core's mod API, as much of it as Prices uses.
local handlers, settings, defaults, tabs, commands, statusPanel, statusSlash, registered
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
local function installApi()
  handlers, settings, defaults, tabs, commands, statusPanel, statusSlash, registered = {}, {}, {}, {}, {}, {}, {}, nil
  db = { settings = settings }
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
    -- The clock, the timers and the journal (the auction side's; test/auction_test.lua tests them).
    Now = function() return 1700000000 end,
    Ago = function() return "just now" end,
    After = function(_, label, f) run(label, f) end,
    Journal = { Add = function() return {} end },
    On = function(event, f)
      handlers[event] = handlers[event] or {}
      table.insert(handlers[event], f)
      return true
    end,
    DB = function() return db end,
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
      AddStatusLines = function(order, f) statusSlash[#statusSlash + 1] = { order = order, fn = f } end,
    },
  }
end

local function fire(event, ...)
  for _, f in ipairs(handlers[event] or {}) do run(event, f, event, ...) end
  for _, f in ipairs(frames) do
    if f.events[event] and f.scripts.OnEvent then run(event, f.scripts.OnEvent, f, event, ...) end
  end
end

-- Loads the addon as the client would (every file of the TOC, with its name and namespace), then
-- logs in. `processor`: the client has TooltipDataProcessor.
local ns
local function load(processor)
  installClient(processor)
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

-- Fills a tooltip with an item, as the client's setter would, and runs the hooks.
local function filler(tip)
  function tip.Fill(self, itemId, getter, args, owner)
    self.lines, self.shown = { ITEMS[itemId].name }, true
    self.itemId, self.getter, self.args, self.owner = itemId, getter, args, owner
    if T_MODE == "processor" then
      self.processingInfo = getter and { getterName = getter, getterArgs = args } or nil
      for _, pc in ipairs(postCalls) do
        if pc.kind == 0 then pc.fn(self, self.data or { id = itemId, type = 0 }) end
      end
      self.processingInfo = nil
      self.info = getter and { getterName = getter, getterArgs = args } or nil
    elseif self.scripts.OnTooltipSetItem then
      self.scripts.OnTooltipSetItem(self)
    end
  end
  return tip
end

-- The price line on a tooltip (without colour codes), or nil.
local function priceLine(tip)
  for i = 2, #tip.lines do
    local l = plain(tip.lines[i])
    if l:match("^Sell Price: ") or l:match("^Sells for ") then return l end
  end
  return nil
end
local function priceLines(tip)
  local n = 0
  for i = 2, #tip.lines do
    local l = plain(tip.lines[i])
    if l:match("^Sell Price: ") or l:match("^Sells for ") then n = n + 1 end
  end
  return n
end

-- Loading and registering. ---------------------------------------------------------------------------
resetItems()
load(true)
T_MODE = "processor"
check(not ns.off, "the mod loads against API 1")
check(registered and registered.id == "prices" and registered.addon == "Ryna_Prices", "it registers as prices")
check(#tabs == 1 and tabs[1].key == "prices" and tabs[1].label == "Prices" and tabs[1].order > 35
  and tabs[1].order < 40, "one Prices tab, after Automation and before Messages")
check(commands.prices ~= nil, "/ryna prices exists")
check(defaults.priceSellLine == true and defaults.priceStackTotal == true
  and defaults.priceOnlyAwayFromMerchant == false, "defaults: the line and the stack total on, everywhere")
check(ns.Tooltip.mode == "processor" and #postCalls == 1, "hooked through TooltipDataProcessor, items only")
check(#chat == 0, "nothing in chat at load or login")

-- The line. --------------------------------------------------------------------------------------
local tip = filler(newTooltip(true))
tip:Fill(2589, "GetBagItem", { 0, 1 })
BAGS[0] = { [1] = 1, [2] = 20, [3] = 7 }
tip:Fill(2589, "GetBagItem", { 0, 1 })
check(priceLine(tip) == "Sell Price: 13c", "one Linen Cloth in a bag slot: the single price only: "
  .. tostring(priceLine(tip)))
check(priceLines(tip) == 1, "one line, once")
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == "Sell Price: 13s 50c (2g 70s for 20)", "a stack of 20: the stack's total: "
  .. tostring(priceLine(tip)))
tip:Fill(4306, "GetBagItem", { 0, 3 })
check(priceLine(tip) == "Sell Price: 13s 50c (94s 50c for 7)", "a stack of 7: " .. tostring(priceLine(tip)))
check(tip.shows == 0, "the processor path doesn't re-show the tooltip itself")

-- No count known (a chat link with no slot or button): one item's price (Maddison, 2026-11-08).
tip:Fill(2589, nil, nil)
check(priceLine(tip) == "Sell Price: 13c", "no count: one item's price, no stack total: " .. tostring(priceLine(tip)))
tip:Fill(2140, nil, nil)
check(priceLine(tip) == "Sell Price: 7c", "an item that doesn't stack: the single price: " .. tostring(priceLine(tip)))

-- The slot button's count, when the tooltip doesn't say which slot.
tip:Fill(4306, nil, nil, { count = 5 })
check(priceLine(tip) == "Sell Price: 13s 50c (67s 50c for 5)", "the owner button's count: " .. tostring(priceLine(tip)))

-- Loot: the loot slot's quantity.
LOOT[2] = 3
tip:Fill(2589, "GetLootItem", { 2 })
check(priceLine(tip) == "Sell Price: 13c (39c for 3)", "loot: the slot's quantity: " .. tostring(priceLine(tip)))

-- Items vendors won't buy: nothing.
tip:Fill(6948, "GetBagItem", { 0, 1 })
check(priceLine(tip) == nil and #tip.lines == 1, "an item that sells for nothing gets no line")

-- An answer with no price in it: no line.
tip:Fill(5000, nil, nil)
check(#tip.lines == 1, "an answer without a sell price adds nothing")

-- Not loaded: nothing, a load request, then the line once the client has the item. ---------------
check(select("#", _G.C_Item.GetItemInfo(9999)) == 0, "the fake answers an unloaded item with no values")
tip:Fill(9999, "GetBagItem", { 0, 2 })
check(#tip.lines == 1, "an unloaded item adds nothing")
check(ns.Tooltip.pending and ns.Tooltip.pending.id == 9999, "and waits for the item")
check(#loads == 1 and loads[1] == 9999, "and asks the client to load it (RequestLoadItemDataByID)")
fire("ITEM_DATA_LOAD_RESULT", 1234, true)
check(tip.refreshes == 0, "another item loading changes nothing")
ITEMS[9999].cached = true
fire("ITEM_DATA_LOAD_RESULT", 9999, true)
check(tip.refreshes == 1, "ITEM_DATA_LOAD_RESULT refreshes the tooltip still showing the item")
-- The slot says 20 of an item that stacks to 5: clamped to the stack.
check(priceLine(tip) == "Sell Price: 2s 50c (12s 50c for 5)",
  "and the line is there, the count clamped to a stack of 5: "
  .. tostring(priceLine(tip)))
check(ns.Tooltip.pending == nil, "nothing waits any more")

-- The load events are registered only while a tooltip waits.
local waiter = ns.Tooltip.waiter
check(waiter ~= nil and next(waiter.events) == nil, "nothing waiting: no load events registered")
ITEMS[9999].cached = false
tip:Fill(9999, "GetBagItem", { 0, 2 })
check(waiter.events.ITEM_DATA_LOAD_RESULT and waiter.events.GET_ITEM_INFO_RECEIVED,
  "a tooltip waiting: both load events registered")
tip:Fill(2589, "GetBagItem", { 0, 1 })
check(ns.Tooltip.pending == nil and next(waiter.events) == nil,
  "the tooltip moving on to a loaded item: nothing waits, the events are unregistered")
ITEMS[9999].cached = true

-- The load failed: no refresh, and nothing waits.
ITEMS[9999].cached = false
tip:Fill(9999, nil, nil)
fire("ITEM_DATA_LOAD_RESULT", 9999, false)
check(tip.refreshes == 1 and ns.Tooltip.pending == nil, "a failed load: no refresh, nothing waits")

-- The tooltip moved on before the item came: no refresh.
ITEMS[9999].cached = false
tip:Fill(9999, nil, nil)
tip:Fill(2589, nil, nil)
ITEMS[9999].cached = true
fire("GET_ITEM_INFO_RECEIVED", 9999, true)
check(tip.refreshes == 1, "a tooltip showing another item now isn't refreshed")
-- Hidden by then: no refresh either.
ITEMS[9999].cached = false
tip:Fill(9999, nil, nil)
tip.shown = false
ITEMS[9999].cached = true
fire("GET_ITEM_INFO_RECEIVED", 9999, true)
check(tip.refreshes == 1, "a hidden tooltip isn't refreshed")

-- A tooltip without RefreshData: the line is added by hand and the tooltip re-shown.
local plainTip = filler(newTooltip(false))
ITEMS[9999].cached = false
plainTip:Fill(9999, nil, nil)
check(#plainTip.lines == 1, "unloaded again: nothing")
ITEMS[9999].cached = true
fire("GET_ITEM_INFO_RECEIVED", 9999, true) -- the older event counts too
check(priceLine(plainTip) == "Sell Price: 2s 50c" and plainTip.shows == 1,
  "no RefreshData: the line (one item, no count known) is added and the tooltip shown again: "
  .. tostring(priceLine(plainTip)))

-- The by-hand path never adds the line twice (the client running the hook again, a second load event).
ITEMS[9999].cached = false
plainTip:Fill(9999, nil, nil)
ITEMS[9999].cached = true
fire("ITEM_DATA_LOAD_RESULT", 9999, true)
check(priceLines(plainTip) == 1, "loaded: one line")
run("hook again", ns.Tooltip.OnItem, plainTip, nil, true)
check(priceLines(plainTip) == 1 and plainTip.shows == 2, "the hook running again by hand doesn't add a second line")

-- The bank's main slots: GetInventoryItem and its count. ------------------------------------------
INVENTORY[40] = 12
tip:Fill(4306, "GetInventoryItem", { "player", 40 })
check(priceLine(tip) == "Sell Price: 13s 50c (1g 62s for 12)", "a bank slot (GetInventoryItem): its count: "
  .. tostring(priceLine(tip)))

-- The stack clamp, and links ignore the owner's count. ----------------------------------------------
tip:Fill(4306, nil, nil, { count = 99 })
check(priceLine(tip) == "Sell Price: 13s 50c (2g 70s for 20)", "an owner count above the stack is clamped to 20: "
  .. tostring(priceLine(tip)))
tip:Fill(2140, nil, nil, { count = 4 })
check(priceLine(tip) == "Sell Price: 7c (28c for 4)", "an item whose stack size is 1 isn't clamped: "
  .. tostring(priceLine(tip)))
local ref = filler(newTooltip(true))
ref:Fill(4306, "GetHyperlink", { link(4306) }, { count = 3 })
check(priceLine(ref) == "Sell Price: 13s 50c",
  "ItemRefTooltip in processor mode, a link: one item's price, not the owner's count: " .. tostring(priceLine(ref)))

-- Append mode: the data's item, not GetItem's (the primary one). -------------------------------------
tip.data = { id = 2589, type = 0 }
tip:Fill(4306, nil, nil)
tip.data = nil
check(priceLine(tip) == "Sell Price: 13c",
  "appended data (Linen Cloth) under a Silk tooltip: the data's item: "
  .. tostring(priceLine(tip)))
tip.data = { hyperlink = link(2589), type = 0 }
tip:Fill(4306, nil, nil)
tip.data = nil
check(priceLine(tip) == "Sell Price: 13c", "appended data with only a hyperlink: its item: "
  .. tostring(priceLine(tip)))

-- The count from each getter the client names. ------------------------------------------------------
_G.GetLootRollItemInfo = function(id) if id == 7 then return "icon", "Silk Cloth", 4 end end
_G.GetQuestItemInfo = function(kind, i) if kind == "reward" and i == 1 then return "Silk Cloth", "icon", 6 end end
_G.GetQuestLogRewardInfo = function(i, questID) if i == 1 and questID == 99 then return "Silk Cloth", "icon", 8 end end
_G.GetQuestLogChoiceInfo = function(i) if i == 2 then return "Silk Cloth", "icon", 9 end end
_G.GetInboxItem = function(m, a) if m == 1 and a == 2 then return "Silk Cloth", 4306, "icon", 11 end end
_G.GetSendMailItem = function(i) if i == 3 then return "Silk Cloth", 4306, "icon", 12 end end
_G.GetTradePlayerItemInfo = function(i) if i == 1 then return "Silk Cloth", "icon", 13 end end
_G.GetTradeTargetItemInfo = function(i) if i == 2 then return "Silk Cloth", "icon", 14 end end
_G.C_MerchantFrame = { GetItemInfo = function(i) if i == 5 then return { stackCount = 10 } end end }
_G.GetBuybackItemInfo = function(i) if i == 1 then return "Silk Cloth", "icon", 1350, 15 end end
local function countOf(getter, args, owner)
  tip:Fill(4306, getter, args, owner)
  return (priceLine(tip) or ""):match("for (%d+)%)$")
end
check(countOf("GetLootRollItem", { 7 }) == "4", "a loot roll: its quantity")
check(countOf("GetQuestItem", { "reward", 1 }) == "6", "a quest reward (GetQuestItem)")
check(countOf("GetQuestLogItem", { "reward", 1, 99 }) == "8", "a quest log reward, with its quest id")
check(countOf("GetQuestLogItem", { "choice", 2 }) == "9", "a quest log choice")
check(countOf("GetInboxItem", { 1, 2 }) == "11", "a mail attachment")
check(countOf("GetInboxItem", { 1 }, { count = 3 }) == "3", "a mail's header item: no count of its own, the button's")
check(countOf("GetSendMailItem", { 3 }) == "12", "an item being mailed")
check(countOf("GetTradePlayerItem", { 1 }) == "13", "my side of a trade")
check(countOf("GetTradeTargetItem", { 2 }) == "14", "their side of a trade")
check(countOf("GetMerchantItem", { 5 }) == "10", "a merchant's item: C_MerchantFrame's stackCount")
_G.C_MerchantFrame = nil
_G.GetMerchantItemInfo = function(i) if i == 5 then return "Silk Cloth", "icon", 1350, 5 end end
check(countOf("GetMerchantItem", { 5 }) == "5", "a merchant's item without C_MerchantFrame: the older function")
check(countOf("GetBuybackItem", { 1 }) == "15", "a buyback item")
check(countOf("GetQuestItem", { "reward", 4 }, { count = 2 }) == "2",
  "a getter with no answer: the owner button's count")
_G.GetQuestItemInfo = nil
check(countOf("GetQuestItem", { "reward", 1 }) == nil, "a getter this client lacks, no button: one item's price")
check(priceLine(tip) == "Sell Price: 13s 50c", "  and the line is the single price: " .. tostring(priceLine(tip)))

-- The game's own price on the tooltip (a money line it drew): no second one. -------------------------
tip.shownMoneyFrames = 1
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == nil and #tip.lines == 1, "the game drew its own price: no line from us")
tip.shownMoneyFrames = 0
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) ~= nil, "no money line drawn: ours")
tip.shownMoneyFrames = nil

-- Coin icons and the label. ---------------------------------------------------------------------------
_G.C_CurrencyInfo = { GetCoinTextureString = function(c) return "<" .. c .. ">" end }
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == "Sell Price: <1350> (<27000> for 20)",
  "C_CurrencyInfo's coin icons: " .. tostring(priceLine(tip)))
_G.C_CurrencyInfo = { GetCoinTextureString = function() return "" end }
_G.GetCoinTextureString = function(c) return "[" .. c .. "]" end
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == "Sell Price: [1350] ([27000] for 20)",
  "an empty answer from C_CurrencyInfo: the older global's: " .. tostring(priceLine(tip)))
_G.C_CurrencyInfo = { GetCoinTextureString = function() error("no") end }
_G.GetCoinTextureString = nil
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == "Sell Price: 13s 50c (2g 70s for 20)", "a formatter that raises: the core's money text: "
  .. tostring(priceLine(tip)))
_G.C_CurrencyInfo = nil
_G.SELL_PRICE = nil
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == "Sells for 13s 50c (2g 70s for 20)", "no SELL_PRICE on the client: our own label: "
  .. tostring(priceLine(tip)))
_G.SELL_PRICE = "Verkaufspreis"
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(plain(tip.lines[2]) == "Verkaufspreis: 13s 50c (2g 70s for 20)", "the client's label, in its language: "
  .. plain(tip.lines[2] or ""))
_G.SELL_PRICE = "Sell Price"

-- Only away from merchants. ----------------------------------------------------------------------
_G.MerchantFrame.shown = true
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) ~= nil, "by default the line shows at a merchant too")
settings.priceOnlyAwayFromMerchant = true
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == nil, "away-from-merchant on: no line while the merchant window is open")
_G.MerchantFrame.shown = false
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) ~= nil, "and the line again once it's closed")
_G.MerchantFrame = nil
fire("MERCHANT_SHOW")
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == nil, "the merchant events count when there's no MerchantFrame (a replaced UI)")
fire("MERCHANT_CLOSED")
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) ~= nil, "and MERCHANT_CLOSED ends it")
settings.priceOnlyAwayFromMerchant = nil

-- The toggles. -----------------------------------------------------------------------------------
settings.priceStackTotal = false
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(priceLine(tip) == "Sell Price: 13s 50c", "stack total off: the single price only")
settings.priceStackTotal = nil
settings.priceSellLine = false
tip:Fill(4306, "GetBagItem", { 0, 2 })
check(#tip.lines == 1, "sell line off: nothing at all")
ITEMS[9999].cached = false
tip:Fill(9999, nil, nil)
check(ns.Tooltip.pending == nil or ns.Tooltip.pending.tooltip ~= tip,
  "sell line off: nothing waits on an uncached item")
ITEMS[9999].cached = true
settings.priceSellLine = nil

-- The tab, the status line and /ryna prices. ---------------------------------------------------------
local stack = { y = 0 }
function stack.text() end
run("tab build", tabs[1].build, {})
run("tab layout", tabs[1].layout, stack, { priceSellLine = true, priceStackTotal = true })
check(#errors == 0, "the tab builds and lays out: " .. table.concat(errors, "; "))
errors = {}
check(statusPanel[1].fn()[1] == "Prices: sell line on (with the stack's total, everywhere)",
  "the status line: " .. statusPanel[1].fn()[1])
check(statusSlash[1] and statusSlash[1].order == statusPanel[1].order, "and the same in /ryna status")
check(#chat == 0, "nothing in chat from any tooltip")
commands.prices.run("", "")
-- The two status lines (the sell line's, the auction side's), a line per setting, and the hint.
check(#chat == 2 + #ns.Settings.FEATURES + 1, "/ryna prices prints its state when typed: " .. #chat)
check(chat[1] == "Ryna: Prices: sell line on (with the stack's total, everywhere)", chat[1])
chat = {}
commands.prices.run("stacktotal", "stacktotal off")
check(settings.priceStackTotal == false and #chat == 1, "/ryna prices stackTotal off")
check(statusPanel[1].fn()[1] == "Prices: sell line on (no stack total, everywhere)", statusPanel[1].fn()[1])
commands.prices.run("awayFromMerchant", "awayFromMerchant ON")
check(settings.priceOnlyAwayFromMerchant == true, "/ryna prices awayFromMerchant ON (any case)")
commands.prices.run("sellline", "sellline")
check(chat[#chat] == "Ryna: sellLine is on. /ryna prices sellLine on||off changes it.", chat[#chat])
commands.prices.run("nope", "nope on")
check(chat[#chat] == "Ryna: no price setting called nope. /ryna prices lists them.", chat[#chat])
commands.prices.run("sellline", "sellline off")
check(statusPanel[1].fn()[1] == "Prices: sell line off", statusPanel[1].fn()[1])

-- A client without TooltipDataProcessor: the item scripts, and the tooltip re-shown. -------------
chat = {}
resetItems()
local gt = filler(newTooltip(false))
_G.GameTooltip = gt
_G.ItemRefTooltip = filler(newTooltip(false))
load(false)
T_MODE = "scripts"
check(ns.Tooltip.mode == "scripts", "no TooltipDataProcessor: OnTooltipSetItem")
check(gt.scripts.OnTooltipSetItem ~= nil and _G.ItemRefTooltip.scripts.OnTooltipSetItem ~= nil,
  "GameTooltip and ItemRefTooltip hooked")
BAGS[0] = { [2] = 20 }
gt:Fill(4306, nil, nil, { count = 20 })
check(priceLine(gt) == "Sell Price: 13s 50c (2g 70s for 20)" and gt.shows == 1,
  "the script path adds the line and re-shows: " .. tostring(priceLine(gt)))
check(#chat == 0, "nothing in chat")

check(#errors == 0, "no handler raised: " .. table.concat(errors, "; "))
print(string.format("prices: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
