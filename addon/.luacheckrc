-- luacheck config for the Ryna addon (World of Warcraft: Forever, modern client, Lua 5.1).
-- Run from the repository root: luacheck addon/
std = "lua51"
max_line_length = 120
exclude_files = { ".luacheckrc" }

-- What the addon defines: its SavedVariable, the companion app's status file and its slash command.
globals = {
  "RynaAgentStatus",
  "RynaDB",
  "SLASH_RYNA1",
  "SlashCmdList",
}

-- Client APIs the addon references directly. Everything uncertain is looked up through
-- _G (ns.Fn) instead, so it doesn't need listing here.
read_globals = {
  "_G",
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
  "UnitLevel",
  "UnitName",
  "UnitRace",
  "UnitSex",
  "UnitXP",
  "UnitXPMax",
}
