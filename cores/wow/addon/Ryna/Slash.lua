-- Slash commands: /ryna (opens the panel), /ryna status, /ryna combatlog instances|always|never,
-- /ryna debug on|off|toggle, /ryna help, and whatever a mod adds through Ryna.Slash.Add
-- (none of Chronicle's today). Status and help go to the default chat frame; a mod adds /ryna status
-- lines through Ryna.Slash.AddStatusLines.
-- The chat frame reads "|" as the start of an escape code, so a literal one is written "||".
-- Command words (status, debug, on, off, ...) are never translated.

local _, ns = ...
local L = ns.L

local function onOff(on)
  return on and L["on"] or L["off"]
end
ns.OnOffText = onOff

local function when(t)
  if type(t) ~= "number" then return L["never"] end
  return date("%Y-%m-%d %H:%M", t)
end

-- The commands: { word, run(arg), help, order }, the core's and the mods'. `help` is a line for
-- /ryna help, already translated.
local commands = {}
ns.slashStatusSources = {}

function ns.SlashAdd(spec)
  if type(spec) ~= "table" or type(spec.word) ~= "string" or type(spec.run) ~= "function" then return false end
  local word = spec.word:lower()
  for _, c in ipairs(commands) do
    if c.word == word then return false end
  end
  commands[#commands + 1] = {
    word = word, run = spec.run, help = spec.help, order = tonumber(spec.order) or 100, seq = #commands + 1,
  }
  table.sort(commands, function(a, b)
    if a.order ~= b.order then return a.order < b.order end
    return a.seq < b.seq
  end)
  return true
end

local function help()
  ns.Print(string.format(L["v%s. Commands:"], ns.VERSION))
  ns.Print("  " .. L["/ryna: open the Ryna settings panel"])
  for _, c in ipairs(commands) do
    if type(c.help) == "string" then ns.Print("  " .. c.help) end
  end
end

local function debugCommand(arg)
  local on = ns.Debugging()
  if arg == "" then
    ns.Print(string.format(L["debug messages are %s. /ryna debug on||off||toggle changes it."], onOff(on)))
    return
  end
  local want
  if arg == "on" then
    want = true
  elseif arg == "off" then
    want = false
  elseif arg == "toggle" then
    want = not on
  else
    ns.Print(string.format(L["usage: /ryna debug on||off||toggle (now %s)"], onOff(on)))
    return
  end
  if want == on then
    ns.Print(string.format(L["debug messages are already %s."], onOff(on)))
    return
  end
  ns.SetDebug(want)
  ns.Run("panel", ns.Panel.Refresh)
end

-- The core's /ryna status lines: versions first (10), the journal (30), the combat log (40), then
-- the rest (50).
local statusSources = ns.slashStatusSources
statusSources[#statusSources + 1] = { order = 10, seq = 0, fn = function()
  local client = ns.db.client or {}
  return { string.format(L["v%s, schema %d, flavor %s, client %s (interface %s)"],
    ns.VERSION, ns.SCHEMA, ns.FLAVOR, tostring(client.version), tostring(client.interface)) }
end }

statusSources[#statusSources + 1] = { order = 30, seq = 0, fn = function()
  local journal = ns.db.journal
  return { string.format(L["journal: %s since %s; session %s, %d this session"],
    ns.Plural(#journal, L["%d event"], L["%d events"]), when(journal[1] and journal[1].at),
    tostring(ns.Journal.session), ns.Journal.seq or 0) }
end }

statusSources[#statusSources + 1] = { order = 40, seq = 0, fn = function()
  local logging = ns.CombatLog.IsLogging()
  return { string.format(L["combat log: setting %s, currently %s"], ns.db.settings.combatLog,
    logging == nil and L["unavailable"] or onOff(logging)) }
end }

statusSources[#statusSources + 1] = { order = 50, seq = 0, fn = function()
  local lines = {
    string.format(L["debug messages: %s"], onOff(ns.Debugging())),
    string.format(L["companion app: %s"], ns.Status.SyncLine()),
  }
  for _, line in ipairs(ns.Status.UpdateLines()) do lines[#lines + 1] = line end
  if ns.errors.count > 0 then
    lines[#lines + 1] = string.format(L["%s; last: %s"],
      ns.Plural(ns.errors.count, L["%d caught error"], L["%d caught errors"]), tostring(ns.errors.last))
  end
  return lines
end }

local function status()
  if not ns.db then
    ns.Print(L["not initialised yet"])
    return
  end
  for _, line in ipairs(ns.OrderedLines(ns.slashStatusSources)) do ns.Print(line) end
end

ns.SlashAdd({ word = "status", order = 10, run = status,
  help = L["/ryna status: what is recorded, the journal's size, combat logging"] })
local function combatLogCommand(arg)
  if ns.COMBAT_LOG_MODES[arg] then
    ns.CombatLog.SetMode(arg)
    ns.Print(string.format(L["combat logging: %s"], arg))
    ns.Run("panel", ns.Panel.Refresh)
  else
    ns.Print(string.format(L["usage: /ryna combatlog instances||always||never (now %s)"], ns.db.settings.combatLog))
  end
end

ns.SlashAdd({ word = "combatlog", order = 20, run = combatLogCommand,
  help = L["/ryna combatlog instances||always||never: when the addon turns combat logging on"] })
ns.SlashAdd({ word = "debug", order = 40, run = debugCommand,
  help = L["/ryna debug on||off||toggle: print what Ryna is doing to chat "
    .. "(/ryna debug alone: which it is)"] })

-- A command's run(arg, rest): `arg` is the first word after the command, `rest` everything after
-- the command (both lowercased), for a command that takes more than one word (/ryna auto <key> on).
function ns.Slash(msg)
  local cmd, rest = tostring(msg or ""):lower():match("^%s*(%S*)%s*(.-)%s*$")
  if cmd == "" then
    ns.Panel.Open()
    return
  end
  local arg = rest:match("^(%S*)")
  for _, c in ipairs(commands) do
    if c.word == cmd then return c.run(arg, rest) end
  end
  help()
end

SLASH_RYNA1 = "/ryna"
SlashCmdList.RYNA = function(msg) ns.Run("/ryna", ns.Slash, msg) end
