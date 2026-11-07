-- Mod: Automation's namespace, built from the core's mod API (the `Ryna` table, the core addon's
-- Mods.lua). Automation does chores at quest givers and vendors for the player: accepting and turning
-- in quests, skipping quest-only gossip (Quests.lua), selling greys and repairing (Vendor.lua). Every
-- one is its own setting, off until ticked (Settings.lua), on top of the app's switch for the mod
-- itself; holding Shift while talking to an NPC pauses all of it. It keeps nothing but its settings.
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

-- Whether a Blizzard frame is on screen; true when this client has no frame by that name (nothing to
-- go by, so nothing to bail on).
function ns.Shown(name)
  local f = _G[name]
  if type(f) ~= "table" or type(f.IsShown) ~= "function" then return true end
  local ok, shown = pcall(f.IsShown, f)
  return ok and shown and true or false
end
