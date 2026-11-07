-- luacheck config for the Ryna addons (World of Warcraft: Forever, modern client, Lua 5.1): the core's
-- (cores/wow/addon/Ryna) and the mods' (mods/wow/*/addon/*). Run from the repository root:
--   luacheck cores/wow/addon/ mods/wow/ --config cores/wow/addon/.luacheckrc
std = "lua51"
max_line_length = 120
exclude_files = { ".luacheckrc" }

-- What the core addon defines: its SavedVariable, the mod API (`Ryna`, which the mods read), the
-- companion app's status file, its slash command and its key binding (the function Bindings.xml
-- calls and the binding's header and name).
globals = {
  "StaticPopupDialogs", -- the reload prompt is added to the client's dialog table
  "Ryna",
  "BINDING_HEADER_RYNA",
  "BINDING_NAME_RYNA_CENSUS_NEXT",
  "Ryna_CensusNext",
  "RynaAgentStatus",
  "RynaDB",
  "SLASH_RYNA1",
  "SlashCmdList",
}

-- Client APIs the addon references directly. Everything uncertain is looked up through
-- _G (ns.Fn) instead, so it doesn't need listing here.
read_globals = {
  "_G",
  "C_CVar",
  "C_Timer",
  "CreateFrame",
  "date",
  "GetCursorPosition",
  "GetInventoryItemLink",
  "GetMoney",
  "GetTime",
  "time",
  "UnitClass",
  "UnitFactionGroup",
  "UnitGUID",
  "UnitIsPlayer",
  "UnitIsUnit",
  "UnitLevel",
  "UnitName",
  "UnitRace",
  "UnitSex",
  "UnitXP",
  "UnitXPMax",
}
