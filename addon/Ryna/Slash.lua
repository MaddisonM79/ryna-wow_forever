-- Slash commands: /ryna (opens the panel), /ryna status, /ryna combatlog instances|always|never,
-- /ryna debug on|off|toggle, /ryna help. Status and help go to the default chat frame.
-- The chat frame reads "|" as the start of an escape code, so a literal one is written "||".

local _, ns = ...

local function when(t)
  if type(t) ~= "number" then return "never" end
  return date("%Y-%m-%d %H:%M", t)
end

local function count(t)
  local n = 0
  if type(t) == "table" then
    for _ in pairs(t) do n = n + 1 end
  end
  return n
end

local function help()
  ns.Print("v" .. ns.VERSION .. ". Commands:")
  ns.Print("  /ryna: open the Ryna settings panel")
  ns.Print("  /ryna status: what is recorded, the journal's size, combat logging")
  ns.Print("  /ryna combatlog instances||always||never: when the addon turns combat logging on")
  ns.Print("  /ryna debug on||off||toggle: print what Ryna is doing to chat (/ryna debug alone: which it is)")
end

local function debugCommand(arg)
  local on = ns.Debugging()
  if arg == "" then
    ns.Print("debug messages are " .. (on and "on" or "off") .. ". /ryna debug on||off||toggle changes it.")
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
    ns.Print("usage: /ryna debug on||off||toggle (now " .. (on and "on" or "off") .. ")")
    return
  end
  if want == on then
    ns.Print("debug messages are already " .. (on and "on" or "off") .. ".")
    return
  end
  ns.SetDebug(want)
  ns.Run("panel", ns.Panel.Refresh)
end

local function status()
  local db, rec = ns.db, ns.rec
  if not db then
    ns.Print("not initialised yet")
    return
  end
  local client = db.client or {}
  ns.Print(string.format("v%s, schema %d, flavor %s, client %s (interface %s)",
    ns.VERSION, ns.SCHEMA, ns.FLAVOR, tostring(client.version), tostring(client.interface)))
  if rec then
    ns.Print(string.format("%s, level %s %s, realm %s (%s)", tostring(rec.fullName), tostring(rec.level),
      tostring(rec.class), tostring(rec.realm), tostring(rec.realmId)))
    local q = rec.quests or {}
    ns.Print(string.format("recorded: %d gear, %d container slots (bank as of %s), %d active / %d completed quests",
      #(rec.gear or {}), #(rec.containers or {}), when(rec.bankAsOf), #(q.active or {}), #(q.completed or {})))
    ns.Print(string.format("%d professions, %d reputations, played %s, guild %s",
      #(rec.professions or {}), #(rec.reputations or {}),
      rec.playedTotal and (math.floor(rec.playedTotal / 3600) .. "h") or "pending",
      rec.guild and rec.guild.name or "none"))
  end
  ns.Print(string.format("%d characters, %d guilds on this account", count(db.characters), count(db.guilds)))
  local journal = db.journal
  ns.Print(string.format("journal: %d events since %s; session %s, %d this session",
    #journal, when(journal[1] and journal[1].at), tostring(ns.Journal.session), ns.Journal.seq or 0))
  local logging = ns.Zone.IsLogging()
  ns.Print(string.format("combat log: setting %s, currently %s", db.settings.combatLog,
    logging == nil and "unavailable" or (logging and "on" or "off")))
  ns.Print("debug messages: " .. (ns.Debugging() and "on" or "off"))
  ns.Print("companion app: " .. ns.Status.SyncLine())
  for _, line in ipairs(ns.Status.UpdateLines()) do ns.Print(line) end
  if ns.errors.count > 0 then
    ns.Print(string.format("%d caught errors; last: %s", ns.errors.count, tostring(ns.errors.last)))
  end
end

function ns.Slash(msg)
  local cmd, arg = tostring(msg or ""):lower():match("^%s*(%S*)%s*(%S*)")
  if cmd == "" then
    ns.Panel.Open()
  elseif cmd == "status" then
    status()
  elseif cmd == "combatlog" then
    if ns.COMBAT_LOG_MODES[arg] then
      ns.Zone.SetMode(arg)
      ns.Print("combat logging: " .. arg)
      ns.Run("panel", ns.Panel.Refresh)
    else
      ns.Print("usage: /ryna combatlog instances||always||never (now " .. ns.db.settings.combatLog .. ")")
    end
  elseif cmd == "debug" then
    debugCommand(arg)
  else
    help()
  end
end

SLASH_RYNA1 = "/ryna"
SlashCmdList.RYNA = function(msg) ns.Run("/ryna", ns.Slash, msg) end
