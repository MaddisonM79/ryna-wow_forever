-- Mod: Chronicle's namespace, built from the core's mod API (the `Ryna` table, the core addon's
-- Mods.lua). Chronicle is everything Ryna records about a character: the record (Character.lua),
-- the zone and deaths (Zone.lua), bags and bank (Containers.lua), quests (Quests.lua), the guild
-- roster (Guild.lua), other players seen, the answers to the player's own /who among them
-- (Sightings.lua, Census.lua), kills and loot (Chronicle.lua), the tooltips (Tooltip.lua), and the
-- in-game live view: this session's numbers (Session.lua), the session strip (Strip.lua) and its
-- sections of Ryna's window (Window.lua, Characters.lua, Items.lua). It writes the same RynaDB the
-- core addon carried before the split (docs/wow.md section 2), so the companion app and ryna.io see
-- no change; the live view adds only
-- RynaDB.chronicle.session (a session kept across a /reload) and its own settings.
--
-- The files below call ns.Try, ns.On, ns.Journal.Add and so on as they always have: here those are
-- the API's functions, so nothing reaches into the core's own namespace. ns.db reads Ryna.DB() each
-- time (the core may rebuild RynaDB at load). Every file starts with `if ns.off then return end`: with
-- a core addon too old for the API (0.2.6 and earlier carry Chronicle inside themselves), Chronicle
-- loads nothing at all, so the two never record the same thing twice.

local addonName, ns = ...

local R = _G.Ryna
if type(R) ~= "table" or type(R.API) ~= "number" or R.API < 1 or type(R.RegisterMod) ~= "function" then
  ns.off = true
  local frame = _G.DEFAULT_CHAT_FRAME
  if frame and frame.AddMessage then
    frame:AddMessage("|cff7fb2ffRyna Chronicle|r: the Ryna addon here is too old for it, so it is off. "
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
  "DebugChanged", "Debugging", "Plural", "N", "Run", "After", "Debounce", "ReportError", "CanonicalName",
  "MoneyText", "DurationText", "ListText", "On" }) do
  ns[name] = R[name]
end

ns.Setting = R.Settings.Get
ns.Journal = { Add = R.Journal.Add }

-- Chronicle's strings (Locales/), falling back to the core's (Ryna.L), which fall back to the key.
ns.L = setmetatable({}, {
  __index = function(_, key) return R.L[key] end,
  __newindex = function(t, key, value)
    if type(value) == "string" and value ~= "" then rawset(t, key, value) end
  end,
})

-- ns.db is RynaDB, read through the API every time (never stored here).
setmetatable(ns, {
  __index = function(_, key)
    if key == "db" then return R.DB() end
    return nil
  end,
})
