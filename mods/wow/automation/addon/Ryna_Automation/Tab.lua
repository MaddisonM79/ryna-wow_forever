-- Tab: Automation's tab in the core's panel (Ryna.Panel.AddTab), between Status and Messages, built
-- with the core's widgets (Ryna.Panel.Widgets): a checkbox per feature in two groups, quests and
-- vendor, each indented under the one it depends on, and the chat-lines switch.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local W = ns.API.Panel.Widgets
local text, checkbox, toggle = W.text, W.checkbox, W.toggle

local P = {}

-- The checkboxes, in the order they are laid out: { setting, label, help, under }.
local ROWS = {
  quests = {
    { "autoAcceptQuests", L["Accept quests"],
      L["Accepts a quest an NPC, an item or a place offers you. Not quests a player shares, nor ones that "
      .. "read like an escort: those are yours."] },
    { "autoSkipLowLevel", L["Not low-level quests"],
      L["Leaves grey (trivial) quests for you to accept by hand, if you want them."], "autoAcceptQuests" },
    { "autoTurnInQuests", L["Turn in quests"],
      L["Hands in a finished quest and takes its reward when there's only one. With a choice of rewards it "
      .. "stops there for you, and a quest that costs money to hand in is left to you too."] },
    { "autoPickReward", L["Pick the reward that sells for the most"],
      L["With a choice of rewards, takes the one with the highest vendor price and says which in chat."],
      "autoTurnInQuests" },
    { "autoSkipGossip", L["Skip quest-only gossip"],
      L["When an NPC offers nothing but quests, opens one: a finished one first, then a new one. Never a "
      .. "vendor, trainer, flight master, banker, innkeeper, battlemaster or stable master."] },
  },
  vendor = {
    { "autoSellGreys", L["Sell grey items"],
      L["Sells every grey (poor-quality) item in your bags at a vendor, a few a second, and says what they "
      .. "made. Nothing but grey."] },
    { "autoRepair", L["Repair"],
      L["Repairs everything at a vendor who can, with your own gold, and says what it cost."] },
    { "autoRepairGuild", L["Use the guild bank first"],
      L["Pays from the guild bank when you're allowed and it covers the cost, else from your own gold."],
      "autoRepair" },
  },
  chat = {
    { "autoReport", L["Say what it did in chat"],
      L["One line for what was sold, repaired or picked, and for what it left to you."] },
  },
}

local function build(f)
  P.intro = text(f, "GameFontHighlightSmall", 11)
  P.intro:SetText(L["Automation does only what you tick here. Nothing is ticked until you tick it."])
  P.shift = text(f, "GameFontNormal")
  P.shift:SetText(L["Hold Shift while you talk to an NPC and nothing happens by itself. Nor in combat."])
  P.headers = {}
  P.boxes = {}
  for _, group in ipairs({ "quests", "vendor", "chat" }) do
    P.headers[group] = text(f, "GameFontNormal")
    for _, row in ipairs(ROWS[group]) do
      local cb = checkbox(f, row[2], row[3])
      toggle(cb, row[1])
      P.boxes[#P.boxes + 1] = { cb = cb, key = row[1], under = row[4], group = group }
    end
  end
  P.headers.quests:SetText(L["At quest givers"])
  P.headers.vendor:SetText(L["At vendors"])
  P.headers.chat:SetText(L["Chat"])
end

local function layout(s, settings)
  s.text(P.intro, 0, 8)
  s.text(P.shift, 0, 12)
  local group
  for _, b in ipairs(P.boxes) do
    if b.group ~= group then
      group = b.group
      s.text(P.headers[group], 0, 6)
    end
    b.cb:SetChecked(settings[b.key] and true or false)
    if b.under then
      b.cb.setActive(settings[b.under] and true or false)
      b.cb.place(s, 30)
    else
      b.cb.place(s, 4)
    end
  end
end

ns.API.Panel.AddTab({ key = "automation", label = L["Automation"], order = 35, build = build, layout = layout })

-- A feature's checkbox label, for /ryna auto (Status.lua).
S.Label = function(key)
  for _, group in pairs(ROWS) do
    for _, row in ipairs(group) do
      if row[1] == key then return row[2] end
    end
  end
  return key
end
