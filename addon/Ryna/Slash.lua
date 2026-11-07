-- Slash commands: /ryna (opens the panel), /ryna status, /ryna combatlog instances|always|never,
-- /ryna debug on|off|toggle, /ryna census [stop], /ryna help. Status and help go to the default chat frame.
-- Also the census key binding's function, Ryna_CensusNext (Bindings.xml).
-- The chat frame reads "|" as the start of an escape code, so a literal one is written "||".
-- Command words (status, combatlog, instances, on, off, ...) are never translated.

local _, ns = ...
local L, N = ns.L, ns.N

local function onOff(on)
  return on and L["on"] or L["off"]
end

local function when(t)
  if type(t) ~= "number" then return L["never"] end
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
  ns.Print(string.format(L["v%s. Commands:"], ns.VERSION))
  ns.Print("  " .. L["/ryna: open the Ryna settings panel"])
  ns.Print("  " .. L["/ryna status: what is recorded, the journal's size, combat logging"])
  ns.Print("  " .. L["/ryna combatlog instances||always||never: when the addon turns combat logging on"])
  ns.Print("  " .. L["/ryna debug on||off||toggle: print what Ryna is doing to chat "
    .. "(/ryna debug alone: which it is)"])
  ns.Print("  " .. L["/ryna census: run a realm census now (/who, a few queries at a time, or one per "
    .. "/ryna census where the game allows /who only from a click); /ryna census stop ends it"])
end

local function censusCommand(arg)
  if arg == "stop" then
    if ns.Census.Running() then
      ns.Census.Stop(N("stopped by you"))
    else
      ns.Print(L["no census sweep is running."])
    end
  elseif arg == "" then
    -- Synchronous, inside the typed command: the first (or, in click mode, the next) query goes
    -- out from here, which is where clients that restrict /who accept it.
    ns.Census.Request()
  else
    ns.Print(L["usage: /ryna census (runs a sweep now, or its next query) or /ryna census stop"])
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

local function status()
  local db, rec = ns.db, ns.rec
  if not db then
    ns.Print(L["not initialised yet"])
    return
  end
  local client = db.client or {}
  ns.Print(string.format(L["v%s, schema %d, flavor %s, client %s (interface %s)"],
    ns.VERSION, ns.SCHEMA, ns.FLAVOR, tostring(client.version), tostring(client.interface)))
  if rec then
    ns.Print(string.format(L["%s, level %s %s, realm %s (%s)"], tostring(rec.fullName), tostring(rec.level),
      tostring(rec.class), tostring(rec.realm), tostring(rec.realmId)))
    local q = rec.quests or {}
    ns.Print(string.format(L["recorded: %d gear, %s (bank as of %s), %d active / %d completed quests"],
      #(rec.gear or {}),
      ns.Plural(#(rec.containers or {}), L["%d container slot"], L["%d container slots"]),
      when(rec.bankAsOf), #(q.active or {}), #(q.completed or {})))
    ns.Print(string.format(L["%s, %s, played %s, guild %s"],
      ns.Plural(#(rec.professions or {}), L["%d profession"], L["%d professions"]),
      ns.Plural(#(rec.reputations or {}), L["%d reputation"], L["%d reputations"]),
      rec.playedTotal and string.format(L["%dh"], math.floor(rec.playedTotal / 3600)) or L["pending"],
      rec.guild and rec.guild.name or L["none"]))
  end
  ns.Print(string.format(L["%s, %s on this account"],
    ns.Plural(count(db.characters), L["%d character"], L["%d characters"]),
    ns.Plural(count(db.guilds), L["%d guild"], L["%d guilds"])))
  local journal = db.journal
  ns.Print(string.format(L["journal: %s since %s; session %s, %d this session"],
    ns.Plural(#journal, L["%d event"], L["%d events"]), when(journal[1] and journal[1].at),
    tostring(ns.Journal.session), ns.Journal.seq or 0))
  local logging = ns.Zone.IsLogging()
  ns.Print(string.format(L["combat log: setting %s, currently %s"], db.settings.combatLog,
    logging == nil and L["unavailable"] or onOff(logging)))
  ns.Print(string.format(L["debug messages: %s"], onOff(ns.Debugging())))
  ns.Print(string.format(L["companion app: %s"], ns.Status.SyncLine()))
  for _, line in ipairs(ns.Status.UpdateLines()) do ns.Print(line) end
  if ns.errors.count > 0 then
    ns.Print(string.format(L["%s; last: %s"],
      ns.Plural(ns.errors.count, L["%d caught error"], L["%d caught errors"]), tostring(ns.errors.last)))
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
      ns.Print(string.format(L["combat logging: %s"], arg))
      ns.Run("panel", ns.Panel.Refresh)
    else
      ns.Print(string.format(L["usage: /ryna combatlog instances||always||never (now %s)"], ns.db.settings.combatLog))
    end
  elseif cmd == "debug" then
    debugCommand(arg)
  elseif cmd == "census" then
    censusCommand(arg)
  else
    help()
  end
end

SLASH_RYNA1 = "/ryna"
SlashCmdList.RYNA = function(msg) ns.Run("/ryna", ns.Slash, msg) end

-- The key binding (Bindings.xml): Key Bindings -> AddOns -> Ryna -> Next census query. A key press
-- is a hardware event, like a click, so the query goes out from here, synchronously; never move this
-- call behind a timer. The names are the game's BINDING_* globals, set here so they're localised.
_G.BINDING_HEADER_RYNA = L["Ryna"]
_G.BINDING_NAME_RYNA_CENSUS_NEXT = L["Next census query"]

function Ryna_CensusNext()
  ns.Run("census key", ns.Census.Request, true)
end
