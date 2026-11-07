-- Automation's behaviour against a fake client (no game needed): loads the addon's files in TOC order
-- with a stand-in for the core's mod API and the client functions it calls, fires the NPC events and
-- checks what it did. From the repository root, with Lua 5.1 (what the game runs) or later:
--
--   lua5.1 mods/wow/automation/test/automation_test.lua
--
-- This proves the logic (the toggles, Shift, once per conversation, the reward pick, the gossip rules,
-- the selling and repairing); it can't prove the client's API behaves as assumed. That's the game's.
-- luacheck: ignore 1

local ADDON = "mods/wow/automation/addon/Ryna_Automation/"

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end

-- The clock and the timers (C_Timer.After through the fake Ryna.After).
local now, timers = 0, {}
local errors = {}
local function run(label, fn, ...)
  local ok, err = pcall(fn, ...)
  if not ok then errors[#errors + 1] = label .. ": " .. tostring(err) end
end
local function advance(seconds)
  local target = now + seconds
  while true do
    table.sort(timers, function(a, b) if a.at ~= b.at then return a.at < b.at end return a.seq < b.seq end)
    local t = timers[1]
    if not t or t.at > target then break end
    table.remove(timers, 1)
    now = t.at
    run(t.label, t.fn)
  end
  now = target
end
local seq = 0

-- The client: calls recorded, state set per test.
local calls, chat = {}, {}
local function called(name)
  local n = 0
  for _, c in ipairs(calls) do if c[1] == name then n = n + 1 end end
  return n
end
local function lastCall(name)
  for i = #calls, 1, -1 do if calls[i][1] == name then return calls[i] end end
  return nil
end
local function record(name) return function(...) calls[#calls + 1] = { name, ... } end end

local function frame(shown) return { shown = shown, IsShown = function(self) return self.shown end } end

local W -- the world the fake client reads
local function reset()
  calls, chat, errors = {}, {}, {}
  W = {
    shift = false, combat = false, questID = 0, title = "A quest", objective = "Kill ten boars.",
    autoAccept = false, completable = false, moneyToGet = 0, choices = {}, trivial = nil,
    questnpc = "Creature-0-1-2-3-4-5", options = {}, available = {}, active = {},
    bags = {}, prices = {}, repairCost = 0, canRepair = true, money = 100000,
    guild = false, guildAllowed = 0, guildRepairFails = false,
  }
  _G.GossipFrame, _G.QuestFrame, _G.MerchantFrame = frame(false), frame(false), frame(false)
end
reset()

_G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) chat[#chat + 1] = msg end }
_G.GetLocale = function() return "enUS" end
_G.IsShiftKeyDown = function() return W.shift end
_G.InCombatLockdown = function() return W.combat end
_G.GetQuestID = function() return W.questID end
_G.GetTitleText = function() return W.title end
_G.GetObjectiveText = function() return W.objective end
_G.QuestGetAutoAccept = function() return W.autoAccept end
_G.AcknowledgeAutoAcceptQuest = record("AcknowledgeAutoAcceptQuest")
_G.AcceptQuest = record("AcceptQuest")
_G.QuestIsFromAreaTrigger = function() return false end
_G.UnitGUID = function(unit) if unit == "questnpc" then return W.questnpc end end
_G.UnitIsUnit = function() return false end
_G.IsQuestCompletable = function() return W.completable end
_G.GetQuestMoneyToGet = function() return W.moneyToGet end
_G.CompleteQuest = record("CompleteQuest")
_G.GetNumQuestChoices = function() return #W.choices end
_G.GetQuestReward = record("GetQuestReward")
_G.GetQuestItemLink = function(_, i) return W.choices[i] and ("[" .. W.choices[i].name .. "]") end
_G.GetQuestItemInfo = function(_, i)
  local c = W.choices[i]
  if c then return c.name, 0, c.count or 1, 1, true, c.id end
end
_G.C_Item = {
  GetItemInfo = function(x)
    local price
    for _, c in ipairs(W.choices) do if x == "[" .. c.name .. "]" then price = c.price end end
    if type(x) == "number" then price = W.prices[x] end
    return "name", "link", 1, 1, 1, "type", "sub", 1, "", 0, price
  end,
  RequestLoadItemDataByID = record("RequestLoadItemDataByID"),
}
_G.C_QuestLog = { IsQuestTrivial = function() return W.trivial end }
_G.C_GossipInfo = {
  GetOptions = function() return W.options end,
  GetAvailableQuests = function() return W.available end,
  GetActiveQuests = function() return W.active end,
  SelectOption = record("SelectOption"),
  SelectAvailableQuest = record("SelectAvailableQuest"),
  SelectActiveQuest = record("SelectActiveQuest"),
}
_G.C_Container = {
  GetContainerNumSlots = function(bag) return W.bags[bag] and #W.bags[bag] or 0 end,
  GetContainerItemInfo = function(bag, slot)
    local item = W.bags[bag] and W.bags[bag][slot]
    if item and item.itemID then return item end
  end,
  UseContainerItem = function(bag, slot)
    calls[#calls + 1] = { "UseContainerItem", bag, slot }
    W.bags[bag][slot] = {}
  end,
}
_G.NUM_BAG_SLOTS = 4
_G.CanMerchantRepair = function() return W.canRepair end
_G.GetRepairAllCost = function() return W.repairCost, W.repairCost > 0 end
_G.GetMoney = function() return W.money end
_G.IsInGuild = function() return W.guild end
_G.CanGuildBankRepair = function() return W.guild end
_G.GetGuildBankWithdrawMoney = function() return W.guildAllowed end
_G.RepairAllItems = function(guild)
  calls[#calls + 1] = { "RepairAllItems", guild }
  if guild and W.guildRepairFails then return end
  if not guild then W.money = W.money - W.repairCost end
  W.repairCost = 0
end

-- The core's mod API, as much of it as Automation uses.
local handlers, settings, defaults, tabs, commands, statusPanel, statusSlash = {}, nil, {}, {}, {}, {}, {}
local db = nil
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
local Lcore = setmetatable({}, { __index = function(_, k) return k end })
local function widget()
  local w = { checked = false }
  function w.SetText() end
  function w.SetChecked(_, v) w.checked = v end
  function w.SetScript(_, _, f) w.onClick = f end
  function w.GetChecked() return w.checked end
  function w.place() end
  function w.setActive(a) w.active = a end
  return w
end
_G.Ryna = {
  API = 1, VERSION = "test", L = Lcore,
  Try = try, Fn = fn,
  IsSecret = function() return false end,
  NonEmpty = function(s) if type(s) == "string" and s ~= "" then return s end end,
  Now = function() return now end,
  Locale = function() return "enUS" end,
  Print = function(m) chat[#chat + 1] = "Ryna: " .. m end,
  PrintRaw = function(m) chat[#chat + 1] = m end,
  Debug = function() end,
  Plural = function(n, one, many) return string.format(n == 1 and one or many, n) end,
  Run = run,
  After = function(delay, label, f)
    seq = seq + 1
    timers[#timers + 1] = { at = now + delay, label = label, fn = f, seq = seq }
  end,
  ReportError = function(label, err) errors[#errors + 1] = label .. ": " .. tostring(err) end,
  MoneyText = function(c)
    local g, s, cc = math.floor(c / 10000), math.floor(c / 100) % 100, c % 100
    local parts = {}
    if g > 0 then parts[#parts + 1] = g .. "g" end
    if s > 0 then parts[#parts + 1] = s .. "s" end
    if cc > 0 or #parts == 0 then parts[#parts + 1] = cc .. "c" end
    return table.concat(parts, " ")
  end,
  On = function(event, f)
    handlers[event] = handlers[event] or {}
    table.insert(handlers[event], f)
    return true
  end,
  DB = function() return db end,
  RegisterMod = function(spec) _G.Ryna.registered = spec return true end,
  Settings = {
    Get = function(key)
      local v = settings and settings[key]
      if type(v) == "boolean" then return v end
      return defaults[key]
    end,
    Set = function(key, value) if settings then settings[key] = value end end,
    AddDefaults = function(d) for k, v in pairs(d) do defaults[k] = v end end,
  },
  Panel = {
    AddTab = function(spec) tabs[#tabs + 1] = spec return true end,
    AddStatusLines = function(order, f) statusPanel[#statusPanel + 1] = { order = order, fn = f } end,
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

-- Load the addon as the client would: every file of the TOC, with the addon's name and namespace.
local ns = {}
local toc = assert(io.open(ADDON .. "Ryna_Automation.toc")):read("*a")
for line in toc:gmatch("[^\r\n]+") do
  if not line:match("^#") and line:match("%.lua$") then
    local chunk = assert(loadfile(ADDON .. line:gsub("\\", "/")))
    chunk("Ryna_Automation", ns)
  end
end
check(not ns.off, "the mod loads against API 1")
check(_G.Ryna.registered and _G.Ryna.registered.id == "automation", "it registers as automation")
check(#tabs == 1 and tabs[1].key == "automation" and tabs[1].order > 10 and tabs[1].order < 40,
  "one Automation tab between Status and Messages")
check(commands.auto ~= nil, "/ryna auto exists")

local function fire(event, ...)
  for _, f in ipairs(handlers[event] or {}) do run(event, f, event, ...) end
end
local function settle() advance(0.01) end
-- Ends the conversation: every frame closed, and the half second after.
local function close()
  _G.GossipFrame.shown, _G.QuestFrame.shown = false, false
  fire("QUEST_FINISHED")
  fire("GOSSIP_CLOSED")
  advance(1)
end
local function set(on)
  for k, v in pairs(on) do settings[k] = v end
end
local function fresh()
  reset()
  settings = {}
  db = { settings = settings }
  timers, now = {}, 0
end

-- Off by default: nothing at all happens. -----------------------------------------------------------
fresh()
for _, f in ipairs(ns.Settings.FEATURES) do
  check(defaults[f.key] == (f.key == "autoReport"), f.key .. " defaults to " .. tostring(f.key == "autoReport"))
end
_G.QuestFrame.shown, W.questID, W.completable = true, 11, true
fire("QUEST_DETAIL"); fire("QUEST_PROGRESS"); W.choices = {}; fire("QUEST_COMPLETE"); settle()
_G.GossipFrame.shown, W.available = true, { { questID = 12 } }
fire("GOSSIP_SHOW"); settle()
_G.MerchantFrame.shown, W.repairCost = true, 500
W.bags[0] = { { itemID = 1, quality = 0, stackCount = 1 } }
fire("MERCHANT_SHOW"); advance(2)
check(#calls == 0, "with nothing ticked, no client action is called (" .. #calls .. ")")
check(statusPanel[1].fn()[1] == "Automation: nothing on", "status says nothing on")

-- Accepting. ----------------------------------------------------------------------------------------
fresh(); set({ autoAcceptQuests = true })
_G.QuestFrame.shown, W.questID = true, 21
fire("QUEST_DETAIL"); fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 1, "an offered quest is accepted once, though the event came twice")
check(statusSlash[1].fn()[1] == "Automation: quests (accept)", "status names accept")
close()
W.questID = 22; _G.QuestFrame.shown = true; W.shift = true
fire("QUEST_DETAIL"); settle()
W.shift = false
fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 1, "Shift pauses the whole conversation, even after it's let go")
close()
_G.QuestFrame.shown = true; fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 2, "the next conversation acts again")
close()
W.questID = 23; _G.QuestFrame.shown = true; W.combat = true
fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 2, "nothing in combat")
W.combat = false; close()
W.questID = 24; _G.QuestFrame.shown = true; fire("QUEST_DETAIL"); _G.QuestFrame.shown = false; settle()
check(called("AcceptQuest") == 2, "a quest frame another addon closed first is left alone")
close()
W.questID = 124; _G.QuestFrame.shown = true; fire("QUEST_DETAIL"); W.questID = 125; settle()
check(called("AcceptQuest") == 2, "a quest frame that moved on to another quest is left alone")
close()
W.questID = 25; W.title = "Escort the Captain"; _G.QuestFrame.shown = true; fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 2, "an escort isn't accepted")
check(chat[#chat] and chat[#chat]:find("escort", 1, true), "and the chat says so")
close(); W.title = "A quest"
W.questID = 26; W.trivial = true; _G.QuestFrame.shown = true; fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 3, "a trivial quest is accepted while skipLowLevel is off")
close(); set({ autoSkipLowLevel = true })
W.questID = 27; _G.QuestFrame.shown = true; fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 3, "and left alone with it on")
close(); W.trivial = false
W.questID = 28; W.questnpc = "Player-1-2"; _G.QuestFrame.shown = true; fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 3, "a quest a player shares is left alone")
close(); W.questnpc = "Creature-0-1"
W.questID = 29; W.autoAccept = true; _G.QuestFrame.shown = true; fire("QUEST_DETAIL"); settle()
check(called("AcknowledgeAutoAcceptQuest") == 1 and called("AcceptQuest") == 3,
  "an auto-accepted quest is acknowledged")
close(); W.autoAccept = false

-- Turning in. ---------------------------------------------------------------------------------------
fresh(); set({ autoTurnInQuests = true })
_G.QuestFrame.shown, W.questID = true, 31
fire("QUEST_PROGRESS"); settle()
check(called("CompleteQuest") == 0, "an unfinished quest isn't completed")
close(); W.completable = true; _G.QuestFrame.shown = true
fire("QUEST_PROGRESS"); settle()
check(called("CompleteQuest") == 1, "a finished quest is handed in")
W.choices = {}
fire("QUEST_COMPLETE"); settle()
check(called("GetQuestReward") == 1 and lastCall("GetQuestReward")[2] == 0, "no choice: GetQuestReward(0)")
close(); W.questID = 32; _G.QuestFrame.shown = true; W.choices = { { name = "Ring", price = 5, id = 1 } }
fire("QUEST_COMPLETE"); settle()
check(called("GetQuestReward") == 2 and lastCall("GetQuestReward")[2] == 1, "one choice: GetQuestReward(1)")
close(); W.questID = 33; _G.QuestFrame.shown = true
W.choices = { { name = "A", price = 100, count = 1, id = 1 }, { name = "B", price = 30, count = 5, id = 2 },
  { name = "C", price = 120, count = 1, id = 3 } }
fire("QUEST_COMPLETE"); settle()
check(called("GetQuestReward") == 2, "a choice of rewards stops at the reward screen")
close(); set({ autoPickReward = true }); W.questID = 34; _G.QuestFrame.shown = true
fire("QUEST_COMPLETE"); settle()
check(called("GetQuestReward") == 3 and lastCall("GetQuestReward")[2] == 2, "pickReward takes the best seller (5 x 30)")
check(chat[#chat]:find("[B]", 1, true) ~= nil, "and says which")
close(); W.questID = 35; _G.QuestFrame.shown = true; W.choices[3].price = nil
fire("QUEST_COMPLETE"); settle()
check(called("GetQuestReward") == 3 and called("RequestLoadItemDataByID") == 1, "an unknown price: nothing, and asks")
check(chat[#chat]:find("aren't known", 1, true) ~= nil, "and says so")
close(); W.questID = 36; _G.QuestFrame.shown = true; W.moneyToGet = 100; W.choices = {}
fire("QUEST_PROGRESS"); settle()
check(called("CompleteQuest") == 1, "a quest that costs money to hand in is left alone")
W.moneyToGet = 0; close()

-- Gossip. -------------------------------------------------------------------------------------------
fresh(); set({ autoSkipGossip = true })
_G.GossipFrame.shown = true
W.available = { { questID = 41 } }
W.active = { { questID = 42, isComplete = false }, { questID = 43, isComplete = true } }
fire("GOSSIP_SHOW"); settle()
check(called("SelectActiveQuest") == 1 and lastCall("SelectActiveQuest")[2] == 43, "a finished quest first")
fire("GOSSIP_SHOW"); settle()
check(called("SelectAvailableQuest") == 1 and lastCall("SelectAvailableQuest")[2] == 41, "then a new one")
fire("GOSSIP_SHOW"); settle()
check(called("SelectAvailableQuest") == 1 and called("SelectActiveQuest") == 1,
  "nothing twice, and never an unfinished one")
close(); _G.GossipFrame.shown = true
W.options = { { gossipOptionID = 9, flags = 0, name = "Let me browse your goods." } }
fire("GOSSIP_SHOW"); settle()
check(called("SelectAvailableQuest") == 1 and called("SelectOption") == 0, "quests beside a vendor option: nothing")
close(); _G.GossipFrame.shown = true; W.available, W.active = {}, {}
fire("GOSSIP_SHOW"); settle()
check(called("SelectOption") == 0, "a lone option that isn't a quest's is left alone")
close(); _G.GossipFrame.shown = true; W.options = { { gossipOptionID = 7, flags = 1 } }
fire("GOSSIP_SHOW"); settle()
check(called("SelectOption") == 1 and lastCall("SelectOption")[2] == 7, "a lone quest option is taken")
close(); _G.GossipFrame.shown = true; W.options = {}
set({ autoSkipLowLevel = true, autoAcceptQuests = true })
W.available = { { questID = 44, isTrivial = true }, { questID = 45, isTrivial = false } }
fire("GOSSIP_SHOW"); settle()
check(lastCall("SelectAvailableQuest")[2] == 45, "with skipLowLevel, the grey quest isn't opened")
close()

-- Vendor. -------------------------------------------------------------------------------------------
fresh(); set({ autoSellGreys = true })
W.prices = { [1] = 50, [2] = 25, [3] = 999 }
W.bags[0] = { { itemID = 1, quality = 0, stackCount = 2 }, { itemID = 3, quality = 2, stackCount = 1 } }
W.bags[1] = { {}, { itemID = 2, quality = 0, stackCount = 2 }, { itemID = 4, quality = 0, hasNoValue = true } }
_G.MerchantFrame.shown = true
fire("MERCHANT_SHOW"); settle()
check(called("UseContainerItem") <= 1, "selling is throttled")
advance(2)
check(called("UseContainerItem") == 2, "only the two greys with a price are sold")
check(chat[#chat] == "|cff7fb2ffRyna Automation|r: Sold 2 grey items for 1s 50c.",
  "the line: " .. tostring(chat[#chat]))
fresh(); set({ autoSellGreys = true })
W.prices = { [1] = 10 }
W.bags[0] = { { itemID = 1, quality = 0, stackCount = 1 }, { itemID = 1, quality = 0, stackCount = 1 },
  { itemID = 1, quality = 0, stackCount = 1 } }
_G.MerchantFrame.shown = true
fire("MERCHANT_SHOW"); advance(0.15)
_G.MerchantFrame.shown = false; fire("MERCHANT_CLOSED"); advance(2)
check(called("UseContainerItem") < 3, "selling stops when the merchant closes")
fresh(); set({ autoSellGreys = true }); W.shift = true
W.prices = { [1] = 10 }; W.bags[0] = { { itemID = 1, quality = 0, stackCount = 1 } }
_G.MerchantFrame.shown = true; fire("MERCHANT_SHOW"); advance(2)
check(called("UseContainerItem") == 0, "Shift at the merchant: nothing")

fresh(); set({ autoRepair = true }); W.repairCost = 12345; _G.MerchantFrame.shown = true
fire("MERCHANT_SHOW"); advance(1)
check(called("RepairAllItems") == 1 and lastCall("RepairAllItems")[2] == false, "repairs with own gold")
check(chat[#chat]:find("Repaired for 1g 23s 45c.", 1, true) ~= nil, "and says what it cost")
fresh(); set({ autoRepair = true }); W.repairCost = 500; W.money = 100; _G.MerchantFrame.shown = true
fire("MERCHANT_SHOW"); advance(1)
check(called("RepairAllItems") == 0 and chat[#chat]:find("Not enough gold", 1, true) ~= nil, "not enough gold: says so")
fresh(); set({ autoRepair = true, autoRepairGuild = true }); W.repairCost = 500; W.guild = true
W.guildAllowed = 1000; _G.MerchantFrame.shown = true
fire("MERCHANT_SHOW"); advance(1)
check(called("RepairAllItems") == 1 and lastCall("RepairAllItems")[2] == true, "guild bank first")
check(chat[#chat]:find("guild bank", 1, true) ~= nil, "and says so")
fresh(); set({ autoRepair = true, autoRepairGuild = true }); W.repairCost = 500; W.guild = true
W.guildAllowed = 1000; W.guildRepairFails = true; _G.MerchantFrame.shown = true
fire("MERCHANT_SHOW"); advance(1)
check(called("RepairAllItems") == 2 and lastCall("RepairAllItems")[2] == false, "the guild said no: own gold")
fresh(); set({ autoRepairGuild = true }); W.repairCost = 500; _G.MerchantFrame.shown = true
fire("MERCHANT_SHOW"); advance(1)
check(called("RepairAllItems") == 0, "repairGuild alone repairs nothing (it sits under repair)")

-- /ryna auto. ---------------------------------------------------------------------------------------
fresh()
commands.auto.run("sellgreys", "sellgreys on")
check(settings.autoSellGreys == true, "/ryna auto sellGreys on")
commands.auto.run("sellgreys", "sellgreys off")
check(settings.autoSellGreys == false, "/ryna auto sellGreys off")
commands.auto.run("sellgreys", nil)
check(settings.autoSellGreys == false, "a 0.3.4 core (no rest): reports, changes nothing")
local before = #chat
commands.auto.run("", "")
check(#chat - before == #ns.Settings.FEATURES + 2, "/ryna auto lists every feature")

check(#errors == 0, "no handler raised: " .. table.concat(errors, "; "))
print(string.format("automation: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
