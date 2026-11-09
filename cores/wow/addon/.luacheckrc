-- luacheck config for the Ryna addons (World of Warcraft: Forever, modern client, Lua 5.1): the core's
-- (cores/wow/addon/Ryna) and the mods' (mods/wow/*/addon/*). Run from the repository root:
--   luacheck cores/wow/addon/ mods/wow/ --config cores/wow/addon/.luacheckrc
std = "lua51"
max_line_length = 120
exclude_files = { ".luacheckrc" }

-- What the core addon defines: its SavedVariable, the mod API (`Ryna`, which the mods read), the
-- companion app's status file and its slash command.
globals = {
  "StaticPopupDialogs", -- the reload prompt is added to the client's dialog table
  "Ryna",
  "RynaAgentStatus",
  "RynaData_auctions", -- the companion app's data-layer tables (Data.lua, Data/*.lua)
  "RynaData_vendors",
  "RynaDB",
  "SLASH_RYNA1",
  "SLASH_RYNACHRONICLE1", -- Chronicle's /chronicle (mods/wow/chronicle, Window.lua)
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
