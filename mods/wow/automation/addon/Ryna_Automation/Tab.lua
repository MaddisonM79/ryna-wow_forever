-- Tab: Automation's tab in the core's panel (Ryna.Panel.AddTab), between Status and Messages, built
-- with the core's widgets (Ryna.Panel.Widgets): a checkbox per feature in groups (quest givers,
-- vendors, trainers and mail, around the world), each indented under the one it depends on, and the
-- chat-lines switch. Under the vendor's "marked junk" are a button that marks or unmarks the item on
-- the cursor and the list of marked items, each with "remove"; under training, the gold cap (the
-- widgets have no number field, so it's a line saying how to change it: /ryna auto traincap <gold>).

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local W = ns.API.Panel.Widgets
local text, checkbox, toggle, button, link = W.text, W.checkbox, W.toggle, W.button, W.link

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
    { "autoSellJunk", L["Sell the items you marked as junk"],
      L["Sells what you marked below along with the greys, counted apart. Never a quest item, never "
      .. "anything a vendor won't pay for."] },
    { "autoRepair", L["Repair"],
      L["Repairs everything at a vendor who can, with your own gold, and says what it cost."] },
    { "autoRepairGuild", L["Use the guild bank first"],
      L["Pays from the guild bank when you're allowed and it covers the cost, else from your own gold."],
      "autoRepair" },
  },
  npcs = {
    { "autoTrainAll", L["Learn everything a trainer offers, within a cap"],
      L["Buys every available skill or recipe in the trainer's order, a few a second, while the total "
      .. "stays within the cap below and your gold, and says what it learnt."] },
    { "autoMail", L["Take the gold and items from your mail"],
      L["Empties every mail at a mailbox, one thing at a time, until your bags are full. Never a "
      .. "cash-on-delivery mail (it says how many it left), and it never deletes or returns mail."] },
  },
  world = {
    { "autoAcceptRes", L["Accept resurrections"],
      L["Takes a resurrection someone casts on you. Not in combat: then the dialog stays for you."] },
    { "autoAcceptInvites", L["Accept group invites from friends and your guild"],
      L["Joins a group when a friend, a Battle.net friend or someone in your guild invites you. Anyone "
      .. "else's invite stays for you to answer."] },
    { "autoConfirmDialogs", L["Answer the bind and roll confirmations"],
      L["Says yes to looting, equipping or using an item that binds to you and to a roll on one. When you "
      .. "delete a good item it types the word for you; the click is still yours."] },
    { "autoSkipCinematics", L["Skip cinematics"],
      L["Skips in-game cinematics and movies. Hold Shift when one starts to watch it."] },
    { "autoTrackQuests", L["Track new quests"],
      L["Adds a quest to the tracker when you accept it, while the tracker has room."] },
    { "autoReleaseInBG", L["Release your spirit in battlegrounds"],
      L["Releases as soon as you die in a battleground, unless a soulstone or another way back is on "
      .. "offer. Never in a dungeon, raid, arena or the open world."] },
    { "autoLootOn", L["Keep the game's auto-loot on"],
      L["Turns on the game's own Auto Loot option at every login. Unticking it turns the option off once; "
      .. "after that it's the game's setting."] },
    { "autoRoll", L["Roll on group loot (never Need)"],
      L["Greed on every roll that offers it, pass otherwise and on anything that binds to you that you "
      .. "can't need. Your group sees it: every roll is your word to them."] },
  },
  chat = {
    { "autoReport", L["Say what it did in chat"],
      L["One line for what was sold, repaired, picked, learnt, taken or rolled, and for what it left to you."] },
  },
}

local GROUPS = { "quests", "vendor", "npcs", "world", "chat" }

-- Marked junk: a row per item, made as the list grows.
local function junkRow(i)
  local row = P.junkRows[i]
  if row then return row end
  row = { remove = link(P.frame, L["remove"]), label = text(P.frame, "GameFontHighlightSmall", 11) }
  if row.label.SetWidth then row.label:SetWidth(W.WIDTH - 140) end
  P.junkRows[i] = row
  return row
end

local function itemLabel(id)
  local name = ns.ItemInfo(id)
  if not name then
    ns.C("C_Item", "RequestLoadItemDataByID", id)
    return string.format(L["item %d"], id)
  end
  return string.format("%s (%d)", name, id)
end

local function show(region, shown)
  if not region then return end
  if shown and region.Show then region:Show() elseif not shown and region.Hide then region:Hide() end
end

local function layoutJunk(s)
  if P.mark then s.add(P.mark, 24, 34, 6) end
  local ids = ns.Vendor.JunkIDs()
  show(P.junkNone, #ids == 0)
  if #ids == 0 then s.text(P.junkNone, 34, 8) end
  for i, id in ipairs(ids) do
    local row = junkRow(i)
    show(row.remove, true)
    show(row.label, true)
    row.label:SetText(itemLabel(id))
    if row.remove then
      s.add(row.remove, 18, 34, 2)
      row.label:ClearAllPoints()
      row.label:SetPoint("LEFT", row.remove, "RIGHT", 8, 0)
      row.remove:SetScript("OnClick", function()
        ns.Run("automation junk", function()
          ns.Vendor.ToggleJunk(id)
          ns.API.Panel.Refresh()
        end)
      end)
    else
      s.text(row.label, 34, 2)
    end
  end
  for i = #ids + 1, #P.junkRows do
    show(P.junkRows[i].remove, false)
    show(P.junkRows[i].label, false)
  end
  s.y = (s.y or 0) - 6
end

local function build(f)
  P.frame = f
  P.junkRows = {}
  P.intro = text(f, "GameFontHighlightSmall", 11)
  P.intro:SetText(L["Automation does only what you tick here. Nothing is ticked until you tick it."])
  P.shift = text(f, "GameFontNormal")
  P.shift:SetText(L["Hold Shift (at an NPC, or when something starts) and nothing happens by itself. Nor in combat."])
  P.headers = {}
  P.boxes = {}
  for _, group in ipairs(GROUPS) do
    P.headers[group] = text(f, "GameFontNormal")
    for _, row in ipairs(ROWS[group]) do
      local cb = checkbox(f, row[2], row[3])
      local key = row[1]
      toggle(cb, key, function() S.Changed(key, ns.Setting(key)) end)
      P.boxes[#P.boxes + 1] = { cb = cb, key = key, under = row[4], group = group }
    end
  end
  P.headers.quests:SetText(L["At quest givers"])
  P.headers.vendor:SetText(L["At vendors"])
  P.headers.npcs:SetText(L["At trainers and mailboxes"])
  P.headers.world:SetText(L["Around the world"])
  P.headers.chat:SetText(L["Chat"])
  P.cap = text(f, "GameFontHighlightSmall", 11)
  P.mark = button and button(f, L["Mark or unmark the item on the cursor as junk"], 300, 22) or nil
  if P.mark then
    P.mark:SetScript("OnClick", function() ns.Run("automation junk", function() S.MarkJunk() end) end)
  end
  P.junkNone = text(f, "GameFontDisableSmall", 10)
  P.junkNone:SetText(L["Nothing is marked as junk. Pick an item up and press the button (or /ryna auto junk)."])
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
    local checked = settings[b.key] and true or false
    -- Auto-loot shows the game's own option as it is now, where the client can say.
    if b.key == "autoLootOn" and ns.Loot.AutoLoot() ~= nil then checked = ns.Loot.AutoLoot() end
    b.cb:SetChecked(checked)
    if b.under then
      b.cb.setActive(settings[b.under] and true or false)
      b.cb.place(s, 30)
    else
      b.cb.place(s, 4)
    end
    if b.key == "autoSellJunk" then layoutJunk(s) end
    if b.key == "autoTrainAll" then
      P.cap:SetText(string.format(L["Cap: %s. Change it with /ryna auto traincap <gold>; 0 trains nothing."],
        ns.MoneyText(S.TrainCap())))
      s.text(P.cap, 34, 10)
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
