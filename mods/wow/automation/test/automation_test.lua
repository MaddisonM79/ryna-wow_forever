-- Automation's behaviour against a fake client (no game needed): loads the addon's files in TOC order
-- with a stand-in for the core's mod API and the client functions it calls, fires the NPC events and
-- checks what it did. From the repository root, with Lua 5.1 (what the game runs) or later:
--
--   lua5.1 mods/wow/automation/test/automation_test.lua
--
-- This proves the logic (the toggles, Shift, once per conversation, the reward pick, the gossip rules,
-- the selling and repairing, and the v2 features: resurrections, invites, dialogs, cinematics,
-- tracking, battleground release, auto-loot, training, rolls, mail and marked junk); it can't prove
-- the client's API behaves as assumed. That's the game's.
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

-- The rest of the client the v2 features use: dialogs, invites, cinematics, the quest tracker,
-- death, the CVar, trainers, loot rolls, the mailbox and the cursor.
local function extend()
  W.popups, W.popupFrames = {}, {}
  W.unitCombat = false
  W.friendGuids, W.friendNames, W.bnet, W.roster = {}, {}, {}, {}
  W.watches, W.maxWatches, W.watched = 0, 25, {}
  W.instance, W.instanceType, W.soulstone, W.ghost = false, "none", nil, false
  W.cvars = { autoLootDefault = "0" }
  W.services = {}
  W.rolls = {}
  W.inbox, W.freeSlots, W.mailRefuse, W.mailUpdate = {}, 50, false, false
  W.cursor = nil
  W.classes, W.store = {}, nil
  _G.CinematicFrame, _G.MovieFrame, _G.ClassTrainerFrame, _G.InboxFrame =
    frame(false), frame(false), frame(false), frame(false)
  _G.MovieFrame.StopMovie = function() calls[#calls + 1] = { "StopMovie" } end
end
local resetV1 = reset
reset = function() resetV1(); extend() end
reset()

_G.GetTime = function() return now end
_G.UnitAffectingCombat = function() return W.unitCombat end
_G.StaticPopup_Visible = function(which) if W.popups[which] then return "StaticPopup1" end end
_G.StaticPopup_FindVisible = function(which) return W.popupFrames[which] end
_G.StaticPopup_Hide = function(which)
  calls[#calls + 1] = { "StaticPopup_Hide", which }
  W.popups[which] = nil
end
_G.DELETE_ITEM_CONFIRM_STRING = "DELETE"
_G.StaticPopupDialogs = {
  DELETE_GOOD_ITEM = { OnShow = function() end },
  DELETE_GOOD_QUEST_ITEM = { OnShow = function() end },
}
_G.hooksecurefunc = function(t, key, f)
  local old = t[key]
  t[key] = function(...) old(...) f(...) end
end
_G.AcceptResurrect = record("AcceptResurrect")
_G.AcceptGroup = record("AcceptGroup")
_G.C_FriendList = {
  IsFriend = function(guid) return W.friendGuids[guid] or false end,
  GetFriendInfo = function(name) if W.friendNames[name] then return { name = name } end end,
}
_G.BNGetNumFriends = function() return #W.bnet end
_G.C_BattleNet = {
  GetGameAccountInfoByGUID = function() return nil end,
  GetFriendAccountInfo = function(i)
    local b = W.bnet[i]
    if b then return { gameAccountInfo = { characterName = b.name, realmName = b.realm } } end
  end,
}
_G.GetNumGuildMembers = function() return #W.roster end
_G.GetGuildRosterInfo = function(i) return W.roster[i] end
_G.ConfirmLootSlot = record("ConfirmLootSlot")
_G.EquipPendingItem = record("EquipPendingItem")
_G.ConfirmBindOnUse = record("ConfirmBindOnUse")
_G.ConfirmLootRoll = record("ConfirmLootRoll")
_G.CinematicFrame_CancelCinematic = record("CinematicFrame_CancelCinematic")
_G.C_QuestLog.GetQuestWatchType = function(id) if W.watched[id] then return 0 end end
_G.C_QuestLog.GetNumQuestWatches = function() return W.watches end
_G.C_QuestLog.GetMaxNumQuestWatches = function() return W.maxWatches end
_G.C_QuestLog.GetLogIndexForQuestID = function(id) return id > 100 and 1 or nil end
_G.C_QuestLog.GetInfo = function(i) return { questID = 900 + i } end
_G.C_QuestLog.AddQuestWatch = function(id)
  calls[#calls + 1] = { "AddQuestWatch", id }
  W.watched[id] = true
  W.watches = W.watches + 1
end
_G.IsInInstance = function() return W.instance, W.instanceType end
_G.HasSoulstone = function() return W.soulstone end
_G.UnitIsDead = function() return true end
_G.UnitIsGhost = function() return W.ghost end
_G.RepopMe = record("RepopMe")
_G.C_CVar = {
  GetCVar = function(name) return W.cvars[name] end,
  SetCVar = function(name, value)
    calls[#calls + 1] = { "SetCVar", name, value }
    W.cvars[name] = value
  end,
}
-- Forever answers 0 to addons while the window lists its spells (W.countLies); the indexed reads still work.
_G.GetNumTrainerServices = function() return W.countLies and 0 or #W.services end
-- Forever's return order (name, kind, texture, level, sub);
-- W.classicOrder gives the classic one (name, sub, kind, texture).
_G.GetTrainerServiceInfo = function(i)
  local s = W.services[i]
  if not s then return end
  if W.classicOrder then return s.name, s.sub, s.kind, 136026 end
  return s.name, s.kind, 136026, 10, s.sub
end
_G.GetTrainerServiceCost = function(i) return W.services[i] and W.services[i].cost end
_G.SetTrainerServiceTypeFilter = record("SetTrainerServiceTypeFilter")
_G.BuyTrainerService = function(i)
  local s = W.services[i]
  calls[#calls + 1] = { "BuyTrainerService", i, s and s.name }
  -- A learnt service leaves the "available" list, so every index after it moves up.
  if s then
    table.remove(W.services, i)
    W.money = W.money - s.cost
  end
end
_G.GetLootRollItemInfo = function(id)
  local r = W.rolls[id]
  if r then return "icon", r.name, 1, r.quality or 2, r.bop, r.canNeed, r.canGreed, false end
end
_G.GetLootRollItemLink = function(id) return W.rolls[id] and ("[" .. W.rolls[id].name .. "]") end
_G.GetLootRollTimeLeft = function(id) return W.rolls[id] and (W.rolls[id].left or 60000) or 0 end
_G.RollOnLoot = record("RollOnLoot")
_G.GetInboxNumItems = function() return #W.inbox, #W.inbox end
_G.GetInboxHeaderInfo = function(i)
  local m = W.inbox[i]
  if not m then return end
  local n = 0
  for j = 1, 16 do if m.items[j] then n = n + 1 end end
  return "icon", nil, m.sender, m.subject, m.money, m.cod or 0, 3.5, n > 0 and n or nil
end
_G.GetInboxItem = function(i, j)
  local it = W.inbox[i] and W.inbox[i].items[j]
  if it then return it.name, it.id or 1, "icon", it.count or 1, 1, true end
end
local function tidy(i)
  local m = W.inbox[i]
  local any = false
  for j = 1, 16 do if m.items[j] then any = true end end
  if not any and (m.money or 0) == 0 and not m.text then table.remove(W.inbox, i) end
end
_G.TakeInboxMoney = function(i)
  calls[#calls + 1] = { "TakeInboxMoney", i, W.inbox[i] and W.inbox[i].sender, at = now }
  if W.mailRefuse or not W.inbox[i] then return end
  W.inbox[i].money = 0
  tidy(i)
  W.mailUpdate = true
end
_G.TakeInboxItem = function(i, j)
  calls[#calls + 1] = { "TakeInboxItem", i, j, W.inbox[i] and W.inbox[i].sender, at = now }
  if W.mailRefuse or not W.inbox[i] then return end
  W.inbox[i].items[j] = nil
  W.freeSlots = W.freeSlots - 1
  tidy(i)
  W.mailUpdate = true
end
_G.DeleteInboxItem = record("DeleteInboxItem")
_G.ReturnInboxItem = record("ReturnInboxItem")
_G.C_Container.CalculateTotalNumberOfFreeBagSlots = function() return W.freeSlots end
_G.C_Container.GetContainerItemQuestInfo = function(bag, slot)
  local item = W.bags[bag] and W.bags[bag][slot]
  return { isQuestItem = item and item.quest or false }
end
_G.GetCursorInfo = function()
  if W.cursor then return "item", W.cursor, "[Item " .. W.cursor .. "]" end
end
local v1ItemInfo = _G.C_Item.GetItemInfo
_G.C_Item.GetItemInfo = function(x)
  local a, b, c, d, e, f, g, h, i, j, price = v1ItemInfo(x)
  return a, b, c, d, e, f, g, h, i, j, price, W.classes[x]
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
  function w.SetChecked(_, v) w.checked = v end
  function w.SetScript(_, _, f) w.onClick = f end
  function w.GetChecked() return w.checked end
  function w.place() end
  function w.setActive(a) w.active = a end
  function w.Show() w.shown = true end
  function w.Hide() w.shown = false end
  function w.SetPoint() end
  function w.ClearAllPoints() end
  function w.SetWidth() end
  function w.SetText(_, t) w.text = t end
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
    All = function() return settings end,
  },
  Panel = {
    Refresh = function() end,
    AddTab = function(spec) tabs[#tabs + 1] = spec return true end,
    AddStatusLines = function(order, f) statusPanel[#statusPanel + 1] = { order = order, fn = f } end,
    RefreshIfShown = function() end,
    Widgets = {
      WIDTH = 580,
      text = function() return widget() end,
      checkbox = function() return widget() end,
      button = function() return widget() end,
      link = function() return widget() end,
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
  -- Whatever the last scenario left open closes, as the game would.
  for _, e in ipairs({ "TRAINER_CLOSED", "MAIL_CLOSED", "MERCHANT_CLOSED" }) do
    for _, f in ipairs(handlers[e] or {}) do f(e) end
  end
  reset()
  -- The scenarios check the chat lines, so the report is on here; its default (off) is checked above,
  -- and the accepting scenario checks that off means silence.
  settings = { autoReport = true }
  db = { settings = settings }
  timers, now = {}, 0
end

-- Off by default: nothing at all happens. -----------------------------------------------------------
fresh()
for _, f in ipairs(ns.Settings.FEATURES) do
  check(defaults[f.key] == false, f.key .. " defaults to false")
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
fresh(); set({ autoAcceptQuests = true, autoReport = false })
local quiet = #chat
_G.QuestFrame.shown, W.questID = true, 21
fire("QUEST_DETAIL"); fire("QUEST_DETAIL"); settle()
check(called("AcceptQuest") == 1, "an offered quest is accepted once, though the event came twice")
check(#chat == quiet, "and with the report off (the default) nothing is said in chat")
set({ autoReport = true })
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

-- Off by default, the v2 features: their events with nothing ticked call nothing. --------------------
fresh()
W.popups = { RESURRECT = true, PARTY_INVITE = true, LOOT_BIND = true, EQUIP_BIND = true, USE_BIND = true,
  CONFIRM_LOOT_ROLL = true }
W.friendGuids["Player-1"] = true
_G.CinematicFrame.shown, _G.MovieFrame.shown, _G.ClassTrainerFrame.shown, _G.InboxFrame.shown = true, true, true, true
W.instance, W.instanceType = true, "pvp"
W.services = { { name = "Fireball", sub = "Rank 2", kind = "available", cost = 100 } }
W.rolls[1] = { name = "Sword", canGreed = true }
W.inbox = { { sender = "Bob", subject = "Hi", money = 100, items = {} } }
W.bags[0] = { { itemID = 10, quality = 1, stackCount = 1, isJunk = true } }; W.prices = { [10] = 5 }
fire("RESURRECT_REQUEST", "Bob"); fire("PARTY_INVITE_REQUEST", "Ann", false, false, false, false, false, "Player-1")
fire("LOOT_BIND_CONFIRM", 1); fire("EQUIP_BIND_CONFIRM", 1); fire("USE_BIND_CONFIRM"); fire("CONFIRM_LOOT_ROLL", 1, 2)
fire("DELETE_ITEM_CONFIRM"); fire("CINEMATIC_START"); fire("PLAY_MOVIE", 1); fire("QUEST_ACCEPTED", 501)
fire("PLAYER_DEAD"); fire("TRAINER_SHOW"); fire("START_LOOT_ROLL", 1); fire("MAIL_SHOW")
_G.MerchantFrame.shown = true; fire("MERCHANT_SHOW")
advance(5)
check(#calls == 0, "with nothing ticked, none of the v2 events calls the client (" .. #calls .. ")")
_G.Ryna.registered.onLogin()
check(called("SetCVar") == 0, "auto-loot off: login leaves the game's option alone")

-- Resurrections. ------------------------------------------------------------------------------------
fresh(); set({ autoAcceptRes = true })
W.popups.RESURRECT = true
fire("RESURRECT_REQUEST", "Bob"); fire("RESURRECT_REQUEST", "Bob"); settle()
check(called("AcceptResurrect") == 1, "a resurrection is accepted once, though the event came twice")
check(lastCall("StaticPopup_Hide") ~= nil, "and its dialog is hidden")
check(chat[#chat]:find("Accepted Bob's resurrection.", 1, true) ~= nil, "and the chat says whose")
fresh(); set({ autoAcceptRes = true }); W.popups.RESURRECT = true; W.unitCombat = true
fire("RESURRECT_REQUEST", "Bob"); settle()
check(called("AcceptResurrect") == 0 and W.popups.RESURRECT, "in combat (UnitAffectingCombat): the dialog stays")
fresh(); set({ autoAcceptRes = true }); W.popups.RESURRECT = true; W.combat = true
fire("RESURRECT_REQUEST", "Bob"); settle()
check(called("AcceptResurrect") == 0, "in combat (InCombatLockdown): nothing")
fresh(); set({ autoAcceptRes = true }); W.popups.RESURRECT = true; W.shift = true
fire("RESURRECT_REQUEST", "Bob"); settle()
check(called("AcceptResurrect") == 0, "Shift: nothing")
fresh(); set({ autoAcceptRes = true }); W.popups.RESURRECT = true
fire("RESURRECT_REQUEST", "Bob"); W.popups.RESURRECT = nil; settle()
check(called("AcceptResurrect") == 0, "a dialog answered first (by the player or another addon) is left")
fresh(); set({ autoAcceptRes = true }); W.popups.RESURRECT_NO_SICKNESS = true
fire("RESURRECT_REQUEST", "Bob"); settle()
check(called("AcceptResurrect") == 1, "the no-sickness dialog counts too")
fresh(); set({ autoAcceptRes = true }); W.popups.RESURRECT = true
local accept = _G.AcceptResurrect; _G.AcceptResurrect = nil
fire("RESURRECT_REQUEST", "Bob"); settle()
_G.AcceptResurrect = accept
check(called("StaticPopup_Hide") == 0, "a client without AcceptResurrect: nothing")

-- Invites. ------------------------------------------------------------------------------------------
local function invite(name, guid) fire("PARTY_INVITE_REQUEST", name, false, false, false, false, false, guid) end
fresh(); set({ autoAcceptInvites = true }); W.popups.PARTY_INVITE = true; W.friendGuids["Player-1"] = true
invite("Ann", "Player-1"); invite("Ann", "Player-1"); settle()
check(called("AcceptGroup") == 1, "a friend's invite is accepted once")
check(chat[#chat]:find("Ann's group (a friend)", 1, true) ~= nil, "and the chat says who")
check(called("StaticPopup_Hide") == 0, "the dialog waits for the group to form")
fire("GROUP_ROSTER_UPDATE")
check(lastCall("StaticPopup_Hide") and lastCall("StaticPopup_Hide")[2] == "PARTY_INVITE", "then it's hidden")
fresh(); set({ autoAcceptInvites = true }); W.popups.PARTY_INVITE = true
invite("Stranger", "Player-9"); settle(); fire("GROUP_ROSTER_UPDATE")
check(called("AcceptGroup") == 0 and called("StaticPopup_Hide") == 0, "a stranger's invite stays for you")
fresh(); set({ autoAcceptInvites = true }); W.popups.PARTY_INVITE = true; W.friendNames.Dee = true
invite("Dee", nil); settle()
check(called("AcceptGroup") == 1, "a character friend by name (no GUID)")
fresh(); set({ autoAcceptInvites = true }); W.popups.PARTY_INVITE = true
W.bnet = { { name = "Eve", realm = "Forever" } }
invite("Eve-Forever", "Player-5"); settle()
check(called("AcceptGroup") == 1, "a Battle.net friend's character")
fresh(); set({ autoAcceptInvites = true }); W.popups.PARTY_INVITE = true
W.guild = true; W.roster = { "Zed-Forever", "Cara-Forever" }
invite("Cara", "Player-6"); settle()
check(called("AcceptGroup") == 1 and chat[#chat]:find("your guild", 1, true) ~= nil, "a guildmate, by the roster")
fresh(); set({ autoAcceptInvites = true }); W.popups.PARTY_INVITE = true
W.guild = false; W.roster = { "Cara-Forever" }
invite("Cara", "Player-6"); settle()
check(called("AcceptGroup") == 0, "not in a guild: the roster isn't read")
fresh(); set({ autoAcceptInvites = true }); W.popups.PARTY_INVITE = true; W.friendGuids["Player-1"] = true
W.shift = true; invite("Ann", "Player-1"); settle()
check(called("AcceptGroup") == 0, "Shift: a friend's invite stays too")

-- Bind, use and roll confirmations; deleting. ---------------------------------------------------------
fresh(); set({ autoConfirmDialogs = true })
W.popups = { LOOT_BIND = true, EQUIP_BIND = true, USE_BIND = true, CONFIRM_LOOT_ROLL = true }
fire("LOOT_BIND_CONFIRM", 3); fire("LOOT_BIND_CONFIRM", 3)
fire("EQUIP_BIND_CONFIRM", 5); fire("USE_BIND_CONFIRM"); fire("CONFIRM_LOOT_ROLL", 7, 2); settle()
check(called("ConfirmLootSlot") == 1 and lastCall("ConfirmLootSlot")[2] == 3, "loot bind: ConfirmLootSlot(slot), once")
check(called("EquipPendingItem") == 1 and lastCall("EquipPendingItem")[2] == 5, "equip bind: EquipPendingItem(slot)")
check(called("ConfirmBindOnUse") == 1, "use bind: ConfirmBindOnUse()")
check(lastCall("ConfirmLootRoll") and lastCall("ConfirmLootRoll")[2] == 7 and lastCall("ConfirmLootRoll")[3] == 2,
  "roll bind: ConfirmLootRoll(rollID, rollType)")
check(called("StaticPopup_Hide") == 4, "and each dialog is hidden")
fresh(); set({ autoConfirmDialogs = true })
fire("LOOT_BIND_CONFIRM", 3); settle()
check(called("ConfirmLootSlot") == 0, "no dialog up (answered already): nothing")
fresh(); set({ autoConfirmDialogs = true }); W.popups.LOOT_BIND = true; W.shift = true
fire("LOOT_BIND_CONFIRM", 3); settle()
check(called("ConfirmLootSlot") == 0, "Shift: the dialog stays")
local function deletePopup(which)
  local box = { text = "" }
  function box.SetText(_, t) box.text = t end
  function box.GetText() return box.text end
  local b1 = { enabled = false }
  function b1.Enable() b1.enabled = true end
  local p = frame(true)
  p.which, p.editBox, p.button1 = which, box, b1
  return p, box, b1
end
fresh(); set({ autoConfirmDialogs = true })
local popup, box, b1 = deletePopup("DELETE_GOOD_ITEM")
_G.StaticPopupDialogs.DELETE_GOOD_ITEM.OnShow(popup); settle()
check(box.text == "DELETE" and b1.enabled, "deleting a good item: the word is typed and the button enabled")
check(called("StaticPopup_Hide") == 0, "and the dialog stays for the click")
fresh(); set({ autoConfirmDialogs = true })
popup, box = deletePopup("DELETE_GOOD_QUEST_ITEM")
W.popupFrames.DELETE_GOOD_QUEST_ITEM = popup
fire("DELETE_ITEM_CONFIRM", "Thing"); settle()
check(box.text == "DELETE", "the quest-item delete, by the event")
fresh(); set({ autoConfirmDialogs = true }); W.shift = true
popup, box = deletePopup("DELETE_GOOD_ITEM")
_G.StaticPopupDialogs.DELETE_GOOD_ITEM.OnShow(popup); settle()
check(box.text == "", "Shift: nothing typed")
fresh()
popup, box = deletePopup("DELETE_GOOD_ITEM")
_G.StaticPopupDialogs.DELETE_GOOD_ITEM.OnShow(popup); settle()
check(box.text == "", "off: nothing typed")

-- Cinematics. ---------------------------------------------------------------------------------------
fresh(); set({ autoSkipCinematics = true }); _G.CinematicFrame.shown = true
fire("CINEMATIC_START", true); fire("CINEMATIC_START", true); settle()
check(called("CinematicFrame_CancelCinematic") == 1, "a cinematic is skipped, once")
check(chat[#chat]:find("Hold Shift", 1, true) ~= nil, "and the chat says how to watch one")
fresh(); set({ autoSkipCinematics = true }); _G.CinematicFrame.shown = true; W.shift = true
fire("CINEMATIC_START", true); settle()
check(called("CinematicFrame_CancelCinematic") == 0, "Shift when it starts: you watch it")
fresh(); set({ autoSkipCinematics = true }); _G.CinematicFrame.shown = true
fire("CINEMATIC_START", false); settle()
check(called("CinematicFrame_CancelCinematic") == 0, "one the game says can't be skipped isn't")
fresh(); set({ autoSkipCinematics = true }); _G.MovieFrame.shown = true
fire("PLAY_MOVIE", 12); settle()
check(called("StopMovie") == 1, "a movie is stopped")
fresh(); set({ autoSkipCinematics = true }); _G.MovieFrame.shown = false
fire("PLAY_MOVIE", 12); settle()
check(called("StopMovie") == 0, "no movie frame up: nothing")

-- Tracking quests. ----------------------------------------------------------------------------------
fresh(); set({ autoTrackQuests = true })
fire("QUEST_ACCEPTED", 501); fire("QUEST_ACCEPTED", 501); settle()
check(called("AddQuestWatch") == 1 and lastCall("AddQuestWatch")[2] == 501, "Forever's (questID): tracked once")
fire("QUEST_ACCEPTED", 3, 502); settle()
check(lastCall("AddQuestWatch")[2] == 502, "the older (index, questID): the ID")
fire("QUEST_ACCEPTED", 4); settle()
check(lastCall("AddQuestWatch")[2] == 904, "a lone log index the log has no quest ID for is mapped")
fire("QUEST_ACCEPTED", 501); settle()
check(called("AddQuestWatch") == 3, "a quest already tracked isn't added again")
fresh(); set({ autoTrackQuests = true }); W.watches = 25
fire("QUEST_ACCEPTED", 601); settle(); fire("QUEST_ACCEPTED", 602); settle()
check(called("AddQuestWatch") == 0, "a full tracker: nothing")
local fullLines = 0
for _, line in ipairs(chat) do if line:find("tracker is full", 1, true) then fullLines = fullLines + 1 end end
check(fullLines == 1, "and it says so once (" .. fullLines .. ")")
fresh(); set({ autoTrackQuests = true }); W.shift = true
fire("QUEST_ACCEPTED", 701); settle()
check(called("AddQuestWatch") == 0, "Shift: not tracked")

-- Releasing in battlegrounds. -----------------------------------------------------------------------
local function die() W.popups.DEATH = true; fire("PLAYER_DEAD"); settle() end
fresh(); set({ autoReleaseInBG = true }); W.instance, W.instanceType = true, "pvp"; W.popups.DEATH = true
fire("PLAYER_DEAD"); fire("PLAYER_DEAD"); settle()
check(called("RepopMe") == 1, "a death in a battleground releases, once")
for _, kind in ipairs({ "party", "raid", "arena", "none" }) do
  fresh(); set({ autoReleaseInBG = true }); W.instance, W.instanceType = kind ~= "none", kind
  die()
  check(called("RepopMe") == 0, "never in " .. kind)
end
fresh(); set({ autoReleaseInBG = true }); W.instance, W.instanceType = true, "pvp"; W.soulstone = "Use Soulstone"
die()
check(called("RepopMe") == 0, "a soulstone on offer: no release")
fresh(); set({ autoReleaseInBG = true }); W.instance, W.instanceType = true, "pvp"; W.shift = true
die()
check(called("RepopMe") == 0, "Shift at death: no release")
fresh(); set({ autoReleaseInBG = true }); W.instance, W.instanceType = true, "pvp"; W.combat = true
die()
check(called("RepopMe") == 0, "still counted as in combat: not yet")
W.combat = false; fire("PLAYER_REGEN_ENABLED"); settle()
check(called("RepopMe") == 1, "released once combat ends")

-- Auto-loot. ----------------------------------------------------------------------------------------
fresh()
commands.auto.run("lootOn", "lootOn on")
check(W.cvars.autoLootDefault == "1" and called("SetCVar") == 1, "ticking it turns the game's auto-loot on")
_G.Ryna.registered.onLogin()
check(called("SetCVar") == 1, "at login, already on: nothing to set")
W.cvars.autoLootDefault = "0"; _G.Ryna.registered.onLogin()
check(W.cvars.autoLootDefault == "1", "at login, off: back on")
commands.auto.run("lootOn", "lootOn off")
check(W.cvars.autoLootDefault == "0", "unticking turns it off once")
W.cvars.autoLootDefault = "1"; _G.Ryna.registered.onLogin()
check(W.cvars.autoLootDefault == "1", "and then leaves it to the game")

-- Training. -----------------------------------------------------------------------------------------
local function services()
  return {
    { name = "Fireball", sub = "Rank 2", kind = "available", cost = 10000 },
    { name = "Frost Armor", sub = "", kind = "unavailable", cost = 500 },
    { name = "Arcane Intellect", sub = "Rank 1", kind = "available", cost = 20000 },
    { name = "Blizzard", sub = "Rank 1", kind = "available", cost = 200000 },
    { name = "Slow Fall", sub = "", kind = "available", cost = 5000 },
  }
end
fresh(); set({ autoTrainAll = true }); W.services = services(); _G.ClassTrainerFrame.shown = true
fire("TRAINER_SHOW"); fire("TRAINER_SHOW"); settle()
check(called("SetTrainerServiceTypeFilter") == 0, "the type filter is never touched")
check(called("BuyTrainerService") == 1, "one purchase at a time")
advance(2)
check(called("BuyTrainerService") == 3, "everything within the 10g cap, after each index moved")
local bought = {}
for _, c in ipairs(calls) do if c[1] == "BuyTrainerService" then bought[#bought + 1] = c[3] end end
check(table.concat(bought, ",") == "Fireball,Arcane Intellect,Slow Fall", "in order: " .. table.concat(bought, ","))
check(chat[#chat - 1] and chat[#chat - 1]:find(
  "Learnt Fireball (Rank 2), Arcane Intellect (Rank 1), Slow Fall for 3g 50s.", 1, true) ~= nil,
  "the line: " .. tostring(chat[#chat - 1]))
check(chat[#chat] and chat[#chat]:find("1 more is over your 10g cap", 1, true) ~= nil,
  "and what was left: " .. tostring(chat[#chat]))
fresh(); set({ autoTrainAll = true }); W.services = services(); _G.ClassTrainerFrame.shown = true; W.money = 15000
fire("TRAINER_SHOW"); advance(2)
check(called("BuyTrainerService") == 2, "not more than your gold (1g 50s: Fireball, Slow Fall)")
-- Forever: the count says 0, the list is walked by index.
fresh(); set({ autoTrainAll = true }); W.services = services(); W.countLies = true; _G.ClassTrainerFrame.shown = true
fire("TRAINER_SHOW"); advance(2)
check(called("BuyTrainerService") == 3, "a count of 0 (Forever): the list is still walked by index, 3 bought")
W.countLies = nil
-- The classic return order still reads.
fresh(); set({ autoTrainAll = true }); W.services = services(); W.classicOrder = true; _G.ClassTrainerFrame.shown = true
fire("TRAINER_SHOW"); advance(2)
check(called("BuyTrainerService") == 3, "the classic return order: 3 bought")
W.classicOrder = nil
fresh(); set({ autoTrainAll = true }); W.services = services(); _G.ClassTrainerFrame.shown = true
commands.auto.run("traincap", "traincap 0")
check(settings.autoTrainCap == 0, "/ryna auto traincap 0")
fire("TRAINER_SHOW"); advance(2)
check(called("BuyTrainerService") == 0 and called("SetTrainerServiceTypeFilter") == 0, "a cap of 0: nothing")
commands.auto.run("traincap", "traincap 2.5")
check(settings.autoTrainCap == 25000, "/ryna auto traincap 2.5 is 2g 50s")
_G.ClassTrainerFrame.shown = false; fire("TRAINER_CLOSED")
W.services = services(); _G.ClassTrainerFrame.shown = true; fire("TRAINER_SHOW"); advance(2)
check(called("BuyTrainerService") == 2, "within 2g 50s: Fireball and Slow Fall")
fresh(); set({ autoTrainAll = true }); W.services = services(); _G.ClassTrainerFrame.shown = true; W.shift = true
fire("TRAINER_SHOW"); advance(2)
check(called("BuyTrainerService") == 0, "Shift at the trainer: nothing")
fresh(); set({ autoTrainAll = true }); W.services = services(); _G.ClassTrainerFrame.shown = true
fire("TRAINER_SHOW"); advance(0.25)
_G.ClassTrainerFrame.shown = false; fire("TRAINER_CLOSED"); advance(2)
check(called("BuyTrainerService") == 2, "closing the trainer stops it (two bought in the first 0.25 s, none after)")

-- Loot rolls. ---------------------------------------------------------------------------------------
local function rolls()
  return {
    [1] = { name = "Sword", canNeed = true, canGreed = true },
    [2] = { name = "Cloak", bop = true, canNeed = false, canGreed = true },
    [3] = { name = "Ring", bop = true, canNeed = true, canGreed = true },
    [4] = { name = "Pattern", canNeed = false, canGreed = false },
  }
end
local function rollType(id)
  for i = #calls, 1, -1 do if calls[i][1] == "RollOnLoot" and calls[i][2] == id then return calls[i][3] end end
end
fresh(); set({ autoRoll = true }); W.rolls = rolls()
for id = 1, 4 do fire("START_LOOT_ROLL", id) end
fire("START_LOOT_ROLL", 1); settle()
check(called("RollOnLoot") == 4, "one roll per roll ID (" .. called("RollOnLoot") .. ")")
check(rollType(1) == 2, "Greed when it could Need")
check(rollType(2) == 0, "a bind-on-pickup item you can't Need: Pass")
check(rollType(3) == 2, "a bind-on-pickup item you could Need: still Greed")
check(rollType(4) == 0, "no Greed on offer: Pass")
local needs = 0
for _, c in ipairs(calls) do if c[1] == "RollOnLoot" and c[3] == 1 then needs = needs + 1 end end
check(needs == 0, "never Need")
check(chat[#chat]:find("Passed on [Pattern]", 1, true) ~= nil, "a chat line per roll")
-- Roll IDs are unique in the game, and the mod remembers the ones it rolled: new ones from here.
fresh(); set({ autoRoll = true }); W.rolls = { [21] = rolls()[1] }; W.shift = true
fire("START_LOOT_ROLL", 21); settle()
check(called("RollOnLoot") == 0, "Shift: the roll is yours")
fresh(); set({ autoRoll = true }); W.rolls = { [31] = rolls()[1], [32] = rolls()[2] }; W.combat = true
fire("START_LOOT_ROLL", 31); fire("START_LOOT_ROLL", 32); settle()
check(called("RollOnLoot") == 0, "in combat: not yet")
W.combat = false; W.rolls[32].left = 0; fire("PLAYER_REGEN_ENABLED"); settle()
check(called("RollOnLoot") == 1 and rollType(31) == 2, "after combat: the rolls still open")

-- Mail. ---------------------------------------------------------------------------------------------
local function pump(seconds)
  local stop = now + seconds
  while now < stop do
    advance(0.05)
    if W.mailUpdate then W.mailUpdate = false; fire("MAIL_INBOX_UPDATE") end
  end
end
local function inbox()
  return {
    { sender = "Auction House", subject = "Sold", money = 50000, items = {} },
    { sender = "Bob", subject = "Pay me", money = 0, cod = 10000, items = { { name = "Ore", count = 5 } } },
    { sender = "Ann", subject = "Gifts", money = 300, items = { { name = "Cloth", count = 20 }, nil,
      { name = "Potion", count = 2 } } },
  }
end
fresh(); set({ autoMail = true }); W.inbox = inbox(); _G.InboxFrame.shown = true
fire("MAIL_SHOW"); pump(5)
check(called("TakeInboxMoney") == 2 and called("TakeInboxItem") == 2, "gold and items from every mail but the CoD one")
local cod = false
for _, c in ipairs(calls) do
  if (c[1] == "TakeInboxItem" and c[4] == "Bob") or (c[1] == "TakeInboxMoney" and c[3] == "Bob") then cod = true end
end
check(not cod, "the cash-on-delivery mail is never opened")
local gap = math.huge
local last
for _, c in ipairs(calls) do
  if c[1] == "TakeInboxItem" or c[1] == "TakeInboxMoney" then
    if last then gap = math.min(gap, c.at - last) end
    last = c.at
  end
end
check(gap >= 0.3 - 1e-9, "at most one take per 0.3 s (closest " .. gap .. ")")
check(chat[#chat - 1] and chat[#chat - 1]:find("Took 2 items and 5g 3s from the mail.", 1, true) ~= nil,
  "the line: " .. tostring(chat[#chat - 1]))
check(chat[#chat]:find("Left 1 cash-on-delivery mail for you.", 1, true) ~= nil, "and the CoD left: " .. chat[#chat])
check(called("DeleteInboxItem") == 0 and called("ReturnInboxItem") == 0, "nothing deleted or returned")
fresh(); set({ autoMail = true }); W.inbox = inbox(); _G.InboxFrame.shown = true; W.freeSlots = 1
fire("MAIL_SHOW"); pump(5)
check(called("TakeInboxItem") == 1, "bags full: it stops")
local full = 0
for _, line in ipairs(chat) do if line:find("bags are full", 1, true) then full = full + 1 end end
check(full == 1, "and says so once")
fresh(); set({ autoMail = true }); W.inbox = inbox(); _G.InboxFrame.shown = true; W.shift = true
fire("MAIL_SHOW"); pump(3)
check(called("TakeInboxMoney") + called("TakeInboxItem") == 0, "Shift at the mailbox: nothing")
fresh(); set({ autoMail = true }); W.inbox = inbox(); _G.InboxFrame.shown = true
fire("MAIL_SHOW"); pump(0.2)
_G.InboxFrame.shown = false; fire("MAIL_CLOSED"); pump(5)
check(called("TakeInboxMoney") + called("TakeInboxItem") == 1, "closing the mailbox stops it")
fresh(); set({ autoMail = true }); _G.InboxFrame.shown = true; W.mailRefuse = true
W.inbox = { { sender = "Ann", subject = "Gift", money = 0, items = { { name = "Unique", count = 1 } } } }
fire("MAIL_SHOW"); pump(20)
check(called("TakeInboxItem") == 1,
  "a take the server refused isn't retried in a loop (" .. called("TakeInboxItem") .. ")")
check(not chat[#chat] or not chat[#chat]:find("Took", 1, true), "and isn't counted as taken")
fresh(); set({ autoMail = true }); _G.InboxFrame.shown = true
W.inbox = { { sender = "Bob", subject = "Pay me", money = 0, cod = 500, items = { { name = "Ore" } } } }
fire("MAIL_SHOW"); pump(3)
check(called("TakeInboxItem") == 0 and chat[#chat]:find("Left 1 cash-on-delivery", 1, true) ~= nil,
  "only a CoD mail: nothing taken, and it says so")

-- Marked junk. --------------------------------------------------------------------------------------
local function junkBags()
  W.prices = { [1] = 10, [10] = 100, [11] = 50, [12] = 0, [13] = 40 }
  W.classes = { [11] = 12 }
  W.bags[0] = { { itemID = 1, quality = 0, stackCount = 1 }, { itemID = 10, quality = 1, stackCount = 2 },
    { itemID = 11, quality = 1, stackCount = 1 }, { itemID = 12, quality = 1, stackCount = 1 },
    { itemID = 13, quality = 1, stackCount = 1, quest = true } }
end
fresh(); W.cursor = 10
commands.auto.run("junk", "junk")
check(db.automation and db.automation.junk[10] == true, "/ryna auto junk marks the item on the cursor")
check(chat[#chat]:find("is marked as junk", 1, true) ~= nil, "and says so")
commands.auto.run("junk", "junk")
check(db.automation.junk[10] == nil and chat[#chat]:find("no longer junk", 1, true) ~= nil, "again: unmarked")
W.cursor = 11; W.classes = { [11] = 12 }
commands.auto.run("junk", "junk")
check(db.automation.junk[11] == nil and chat[#chat]:find("quest item", 1, true) ~= nil, "a quest item can't be junk")
W.cursor = nil; W.prices = { [12] = 0 }
commands.auto.run("junk", "junk 12")
check(db.automation.junk[12] == nil and chat[#chat]:find("no vendor price", 1, true) ~= nil, "nor one with no price")
commands.auto.run("junk", "junk")
check(chat[#chat]:find("Pick up an item first", 1, true) ~= nil, "nothing on the cursor: says how")
commands.auto.run("junk", "junk 10")
check(db.automation.junk[10] == true, "/ryna auto junk <itemID> toggles it")

fresh(); set({ autoSellGreys = true, autoSellJunk = true }); junkBags()
db.automation = { junk = { [10] = true, [11] = true, [12] = true, [13] = true } }
_G.MerchantFrame.shown = true; fire("MERCHANT_SHOW"); advance(2)
check(called("UseContainerItem") == 2, "the grey and the marked item; never the quest items or the priceless one")
check(chat[#chat] == "|cff7fb2ffRyna Automation|r: Sold 1 grey and 1 marked junk items for 2s 10c.",
  "the line counts them apart: " .. tostring(chat[#chat]))
fresh(); set({ autoSellJunk = true }); junkBags()
db.automation = { junk = { [10] = true } }
_G.MerchantFrame.shown = true; fire("MERCHANT_SHOW"); advance(2)
check(called("UseContainerItem") == 1 and lastCall("UseContainerItem")[3] == 2, "junk alone: the grey stays")
check(chat[#chat]:find("Sold 1 marked junk item for 2s.", 1, true) ~= nil, "the line: " .. tostring(chat[#chat]))
fresh(); set({ autoSellJunk = true }); junkBags()
W.bags[0][2].isJunk = true
_G.MerchantFrame.shown = true; fire("MERCHANT_SHOW"); advance(2)
check(called("UseContainerItem") == 1, "an item the game itself marks as junk (isJunk) sells too")
fresh(); set({ autoSellJunk = true }); junkBags()
db.automation = { junk = { [10] = true } }
W.bags[0][3] = { itemID = 10, quality = 1, stackCount = 1 }
_G.MerchantFrame.shown = true; fire("MERCHANT_SHOW"); advance(0.05)
db.automation.junk[10] = nil; advance(2)
check(called("UseContainerItem") == 1, "unmarked between scan and sale: the rest isn't sold")
fresh(); set({ autoSellJunk = true }); junkBags(); W.shift = true
db.automation = { junk = { [10] = true } }
_G.MerchantFrame.shown = true; fire("MERCHANT_SHOW"); advance(2)
check(called("UseContainerItem") == 0, "Shift at the merchant: no junk sold")

-- The tab. ------------------------------------------------------------------------------------------
fresh(); db.automation = { junk = { [10] = true, [13] = true } }
local stack = { y = 0 }
function stack.add(_, h) stack.y = stack.y - (h or 0) end
function stack.text() end
local tabFrame = {}
run("tab build", tabs[1].build, tabFrame)
W.cvars.autoLootDefault = "1"
run("tab layout", tabs[1].layout, stack, settings)
check(#errors == 0, "the tab builds and lays out: " .. table.concat(errors, "; "))
errors = {}
check(statusPanel[1].fn()[1] == "Automation: nothing on", "status: nothing on still")
set({ autoSellJunk = true, autoTrainAll = true, autoMail = true, autoAcceptRes = true, autoRoll = true })
check(statusPanel[1].fn()[1] == "Automation: vendor (marked junk), trainers and mail (train, mail), "
  .. "world (resurrections, loot rolls)", "status by group: " .. statusPanel[1].fn()[1])

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
check(#chat - before == #ns.Settings.FEATURES + 4, "/ryna auto lists every feature, the cap and the junk count")

check(#errors == 0, "no handler raised: " .. table.concat(errors, "; "))
print(string.format("automation: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
