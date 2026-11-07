-- Status: the recording's own lines, for the core's Status tab and /ryna status (through
-- Ryna.Panel.AddStatusLines and Ryna.Slash.AddStatusLines).

local _, ns = ...
if ns.off then return end
local L = ns.L

local St = {}
ns.Status = St

-- The core's, for the files here that show times and outside text (Census.lua, Tooltip.lua).
St.Ago = ns.API.Ago
St.Plain = ns.API.Plain

-- Counts for "what has been recorded this session".
function St.Counts()
  local rec, db = ns.rec, ns.db
  local c = { items = 0, gear = 0, active = 0, completed = 0, roster = 0 }
  if not rec then return c end
  for _, row in ipairs(rec.containers or {}) do
    if type(row) == "table" then c.items = c.items + (tonumber(row.count) or 1) end
  end
  c.gear = #(rec.gear or {})
  local q = rec.quests or {}
  c.active, c.completed = #(q.active or {}), #(q.completed or {})
  local guild = rec.guild and db and db.guilds[rec.guild.guid]
  if type(guild) == "table" and type(guild.roster) == "table" then c.roster = #guild.roster end
  return c
end

-- The Status tab's census line: off, the running sweep, or the last one.
local function censusLine()
  local C = ns.Census
  if not C then return nil end
  if not ns.Setting("census") then return L["Census: off"] end
  if C.Running() or C.Mode() == "blocked" then return string.format(L["Census: %s"], C.StatusLine()) end
  local l = C.LastSweep()
  if not l then return L["Last census sweep: none yet"] end
  return string.format(L["Last census sweep: %s, %s"],
    ns.Plural(l.players or 0, L["%d player"], L["%d players"]), St.Ago(l.at) or "?")
end

-- The Status tab's Chronicle line: "Chronicle: 41 creatures, 112 items", or off.
local function chronicleLine()
  if not ns.Setting("chronicle") then return L["Chronicle: off"] end
  local creatures, items = 0, 0
  if ns.Chronicle then creatures, items = ns.Chronicle.Counts() end
  return string.format(L["Chronicle: %s, %s"], ns.Plural(creatures, L["%d creature"], L["%d creatures"]),
    ns.Plural(items, L["%d item"], L["%d items"]))
end

-- The Status tab: the character and what is recorded (20, before the core's journal lines), then
-- the census (40, after the core's combat-log line and before the companion app's).
ns.API.Panel.AddStatusLines(20, function()
  local lines = {}
  local rec, c = ns.rec, St.Counts()
  if rec then
    lines[#lines + 1] = string.format(L["Character: %s, level %s"], tostring(rec.fullName), tostring(rec.level))
  else
    lines[#lines + 1] = L["Character: not loaded yet"]
  end
  lines[#lines + 1] = string.format(L["Recorded: %s, %d equipped, %s, %d completed, %s"],
    ns.Plural(c.items, L["%d item in bags and bank"], L["%d items in bags and bank"]), c.gear,
    ns.Plural(c.active, L["%d quest in the log"], L["%d quests in the log"]), c.completed,
    ns.Plural(c.roster, L["%d guild member"], L["%d guild members"]))
  lines[#lines + 1] = chronicleLine()
  return lines
end)

ns.API.Panel.AddStatusLines(40, function()
  local census = censusLine()
  return census and { census } or {}
end)

local function count(t)
  local n = 0
  if type(t) == "table" then
    for _ in pairs(t) do n = n + 1 end
  end
  return n
end

local function when(t)
  if type(t) ~= "number" then return L["never"] end
  return date("%Y-%m-%d %H:%M", t)
end

-- /ryna status: the character (20, after the core's versions line, before the journal).
ns.API.Slash.AddStatusLines(20, function()
  local db, rec, lines = ns.db, ns.rec, {}
  if rec then
    lines[#lines + 1] = string.format(L["%s, level %s %s, realm %s (%s)"], tostring(rec.fullName),
      tostring(rec.level), tostring(rec.class), tostring(rec.realm), tostring(rec.realmId))
    local q = rec.quests or {}
    lines[#lines + 1] = string.format(L["recorded: %d gear, %s (bank as of %s), %d active / %d completed quests"],
      #(rec.gear or {}),
      ns.Plural(#(rec.containers or {}), L["%d container slot"], L["%d container slots"]),
      when(rec.bankAsOf), #(q.active or {}), #(q.completed or {}))
    lines[#lines + 1] = string.format(L["%s, %s, played %s, guild %s"],
      ns.Plural(#(rec.professions or {}), L["%d profession"], L["%d professions"]),
      ns.Plural(#(rec.reputations or {}), L["%d reputation"], L["%d reputations"]),
      rec.playedTotal and string.format(L["%dh"], math.floor(rec.playedTotal / 3600)) or L["pending"],
      rec.guild and rec.guild.name or L["none"])
  end
  lines[#lines + 1] = string.format(L["%s, %s on this account"],
    ns.Plural(count(db.characters), L["%d character"], L["%d characters"]),
    ns.Plural(count(db.guilds), L["%d guild"], L["%d guilds"]))
  return lines
end)
