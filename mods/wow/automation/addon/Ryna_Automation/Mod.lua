-- Mod: Automation's namespace, built from the core's mod API (the `Ryna` table, the core addon's
-- Mods.lua). Automation does chores for the player: accepting and turning in quests and skipping
-- quest-only gossip (Quests.lua), selling greys and marked junk and repairing (Vendor.lua), training
-- (Trainer.lua), emptying the mailbox (Mail.lua), and around the world: resurrections and friends'
-- invites (Social.lua), bind, roll and delete confirmations and cinematics (Dialogs.lua), loot rolls
-- and auto-loot (Loot.lua), tracking new quests and releasing in battlegrounds (World.lua). Every
-- one is its own setting, off until ticked (Settings.lua), on top of the app's switch for the mod
-- itself; holding Shift (or being in combat) pauses all of it. It keeps its settings and, under
-- RynaDB.automation, the items the player marked as junk.
--
-- Every file starts with `if ns.off then return end`: without the mod API (a core addon from before
-- it), Automation loads nothing at all.

local addonName, ns = ...

local R = _G.Ryna
if type(R) ~= "table" or type(R.API) ~= "number" or R.API < 1 or type(R.RegisterMod) ~= "function" then
  ns.off = true
  local frame = _G.DEFAULT_CHAT_FRAME
  if frame and frame.AddMessage then
    frame:AddMessage("|cff7fb2ffRyna Automation|r: the Ryna addon here is too old for it, so it is off. "
      .. "Update the addon from the Ryna companion app.")
  end
  return
end

ns.NAME = addonName
ns.API = R

-- The version is the TOC's, so there is no second copy to keep in step.
local metadata = (_G.C_AddOns and _G.C_AddOns.GetAddOnMetadata) or _G.GetAddOnMetadata
ns.VERSION = R.Try(metadata, addonName, "Version")

for _, name in ipairs({ "Try", "Fn", "IsSecret", "NonEmpty", "Now", "Locale", "Print", "PrintRaw", "Debug",
  "Plural", "Run", "After", "ReportError", "MoneyText", "On" }) do
  ns[name] = R[name]
end

ns.Setting = R.Settings.Get

-- Automation's strings (Locales/), falling back to the core's (Ryna.L), which fall back to the key.
ns.L = setmetatable({}, {
  __index = function(_, key) return R.L[key] end,
  __newindex = function(t, key, value)
    if type(value) == "string" and value ~= "" then rawset(t, key, value) end
  end,
})

-- A chat line of Automation's own, when the report setting is on.
function ns.Report(msg)
  if ns.Setting("autoReport") then ns.PrintRaw("|cff7fb2ffRyna Automation|r: " .. tostring(msg)) end
end

-- A client function, called under pcall when this client has it; nil when it doesn't or it errors.
-- ns.G("AcceptQuest") is a global; ns.C("C_GossipInfo", "GetOptions") one in a namespace table.
function ns.G(name, ...)
  return ns.Try(ns.Fn(name), ...)
end

function ns.C(space, name, ...)
  return ns.Try(ns.Fn(space, name), ...)
end

-- Whether automation must keep its hands off right now: in combat, or Shift held.
function ns.Held()
  if ns.G("IsShiftKeyDown") then return true, "shift" end
  if ns.G("InCombatLockdown") then return true, "combat" end
  return false
end

-- Whether a Blizzard frame (by name, or the frame itself) is on screen; true when this client has no
-- frame by that name (nothing to go by, so nothing to bail on).
function ns.Shown(name)
  local f = type(name) == "table" and name or _G[name]
  if type(f) ~= "table" or type(f.IsShown) ~= "function" then return true end
  local ok, shown = pcall(f.IsShown, f)
  return ok and shown and true or false
end

-- Whether the player is in combat, by either of the client's words for it.
function ns.InCombat()
  return ns.G("InCombatLockdown") and true or (ns.G("UnitAffectingCombat", "player") and true or false)
end

-- A feature this client can't do (a function or event it doesn't have): it does nothing, and says why
-- once under /ryna debug.
local missing = {}
function ns.Missing(feature, what)
  local key = feature .. ":" .. what
  if missing[key] then return end
  missing[key] = true
  ns.Debug("automation: %s: this client has no %s, so it does nothing", feature, what)
end

-- A global client function, or nil and the debug line once.
function ns.Need(feature, name, space)
  local f = space and ns.Fn(space, name) or ns.Fn(name)
  if not f then ns.Missing(feature, space and (space .. "." .. name) or name) end
  return f
end

-- Runs fn a frame from now, once however often `key` is asked for before then, and only if nothing
-- holds automation off at either moment. `check` (optional) is asked again then: the game's own
-- frame or dialog still up.
local soon = {}
function ns.Soon(key, check, fn)
  if soon[key] then return end
  local held, why = ns.Held()
  if held then
    ns.Debug("automation: %s: left to you (%s)", key, why)
    return
  end
  soon[key] = true
  ns.After(0, "automation " .. key, function()
    soon[key] = nil
    if ns.Held() then return end
    if check and not check() then
      ns.Debug("automation: %s: the game's dialog closed first, leaving it", key)
      return
    end
    fn()
  end)
end

-- StaticPopups: whether one of `whichs` is up (true when this client can't say), its frame, a part
-- of it ("EditBox", "Button1") however this client names it, and hiding it.
function ns.PopupShown(...)
  local visible = ns.Fn("StaticPopup_Visible")
  if not visible then return true end
  for i = 1, select("#", ...) do
    if ns.Try(visible, (select(i, ...))) then return true end
  end
  return false
end

function ns.PopupFrame(which)
  local f = ns.G("StaticPopup_FindVisible", which)
  if type(f) == "table" then return f end
  local name = ns.G("StaticPopup_Visible", which)
  if type(name) == "string" and type(_G[name]) == "table" then return _G[name] end
  return nil
end

function ns.PopupPart(popup, part)
  if type(popup) ~= "table" then return nil end
  local lower = part:gsub("^%u", string.lower)
  local v = popup[lower] or popup[part]
  if type(v) == "table" then return v end
  if type(popup["Get" .. part]) == "function" then
    v = ns.Try(popup["Get" .. part], popup)
    if type(v) == "table" then return v end
  end
  if type(popup.ButtonContainer) == "table" and type(popup.ButtonContainer[part]) == "table" then
    return popup.ButtonContainer[part]
  end
  local name = type(popup.GetName) == "function" and ns.Try(popup.GetName, popup)
  if type(name) == "string" and type(_G[name .. part]) == "table" then return _G[name .. part] end
  return nil
end

function ns.HidePopups(...)
  local hide = ns.Fn("StaticPopup_Hide")
  if not hide then return end
  for i = 1, select("#", ...) do ns.Try(hide, (select(i, ...))) end
end

-- Automation's own saved data (RynaDB.automation): { junk = { [itemID] = true } }. nil before RynaDB.
function ns.Store()
  local db = ns.API.DB()
  if type(db) ~= "table" then return nil end
  if type(db.automation) ~= "table" then db.automation = {} end
  if type(db.automation.junk) ~= "table" then db.automation.junk = {} end
  return db.automation
end

-- An item's name and vendor price, and whether it's a quest item, by C_Item.GetItemInfo.
function ns.ItemInfo(item)
  local itemInfo = ns.Fn("C_Item", "GetItemInfo") or ns.Fn("GetItemInfo")
  local name, link, _, _, _, _, _, _, _, _, price, classID = ns.Try(itemInfo, item)
  -- Enum.ItemClass.Questitem is 12.
  local questClass = (type(_G.Enum) == "table" and type(_G.Enum.ItemClass) == "table"
    and _G.Enum.ItemClass.Questitem) or 12
  return name, link, type(price) == "number" and price or nil, classID == questClass
end
