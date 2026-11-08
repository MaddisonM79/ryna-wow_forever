-- The core's journal prune for the Prices mod's `auction` events (cores/wow/addon/Ryna/Journal.lua,
-- J.SHORT_LIVED): they go at login once the companion app has uploaded them for this account (its
-- entry in AgentStatus.lua, matched by RynaDB.meta.accountId, has journalThrough at or past the
-- event), and after three days whatever happens; every other event keeps the journal's 30 days.
-- Also the mods' view, Journal.Events and .Remove. Loads Journal.lua alone with a stub namespace.
-- From the repository root, with Lua 5.1:
--
--   lua5.1 mods/wow/prices/test/journal_prune_test.lua
-- luacheck: ignore 1

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end

local NOW = 1700000000
local DAY = 86400

local ME = "0123456789abcdef"
local function load(journal, status, id)
  local ns = { db = { journal = journal, meta = { accountId = id } }, Now = function() return NOW end,
    Debug = function() end, Debugging = function() return false end }
  _G.RynaAgentStatus = status
  assert(loadfile("cores/wow/addon/Ryna/Journal.lua"))("Ryna", ns)
  return ns
end
local function prune(journal, status, id)
  if id == nil then id = ME end
  local ns = { db = { journal = journal, meta = { accountId = id or nil } }, Now = function() return NOW end,
    Debug = function() end, Debugging = function() return false end }
  _G.RynaAgentStatus = status
  assert(loadfile("cores/wow/addon/Ryna/Journal.lua"))("Ryna", ns)
  ns.Journal.Prune()
  local kinds = {}
  for _, e in ipairs(ns.db.journal) do kinds[#kinds + 1] = e.type .. "@" .. (NOW - e.at) end
  return table.concat(kinds, " ")
end

local function events()
  return {
    { type = "zone", at = NOW - 40 * DAY },
    { type = "zone", at = NOW - 5 * DAY },
    { type = "auction", at = NOW - 4 * DAY },
    { type = "auction", at = NOW - 2 * DAY },
    { type = "auction", at = NOW - 3600 },
    { type = "gold", at = NOW - 600 },
  }
end
local newest = NOW - 600

-- No word from the app: auction events keep three days, the rest 30.
check(prune(events(), nil) == "zone@432000 auction@172800 auction@3600 gold@600",
  "no AgentStatus: auction events older than 3 days go, the rest keep 30 days: " .. prune(events(), nil))

-- This account's events pushed through the newest: every auction event goes, the rest stay.
local function status(...)
  return { accounts = { ... } }
end
local pushed = status({ key = "a", accountId = ME, journalThrough = newest })
check(prune(events(), pushed) == "zone@432000 gold@600", "uploaded: every auction event goes: "
  .. prune(events(), pushed))

-- Pushed through only part of it: the auction events after journalThrough stay.
local partly = status({ key = "a", accountId = ME, journalThrough = NOW - DAY })
check(prune(events(), partly) == "zone@432000 auction@3600 gold@600",
  "pushed through a day ago: the newer auction event stays: " .. prune(events(), partly))

-- Another account pushed everything; this one (skipped after a failure) pushed nothing: kept.
local sibling = status({ key = "b", accountId = "ffffffffffffffff", journalThrough = NOW },
  { key = "a", accountId = ME, journalThrough = nil })
check(prune(events(), sibling) == "zone@432000 auction@172800 auction@3600 gold@600",
  "a sibling account's push says nothing about this one: " .. prune(events(), sibling))

-- An agent that predates the id (no accountId in its status), or a core that has none: kept.
local oldAgent = { accounts = { { key = "a", savedAt = NOW, pushedAt = NOW } } }
check(prune(events(), oldAgent) == "zone@432000 auction@172800 auction@3600 gold@600",
  "an agent without the id: kept (3 days): " .. prune(events(), oldAgent))
check(prune(events(), pushed, false) == "zone@432000 auction@172800 auction@3600 gold@600",
  "no accountId in RynaDB: kept (3 days)")

-- Malformed status: kept.
check(prune(events(), { accounts = "nope" }) == "zone@432000 auction@172800 auction@3600 gold@600",
  "a malformed AgentStatus: kept")

-- The mods' view: the events of one type, and removing some.
local ns = load(events(), nil, ME)
local list = ns.Journal.Events("auction")
check(#list == 3 and list[1].type == "auction", "Events: the auction events, in order")
check(ns.Journal.Remove("auction", function(e) return e.at < NOW - 3 * DAY end) == 1, "Remove: one went")
check(#ns.db.journal == 5 and #ns.Journal.Events("auction") == 2 and #ns.Journal.Events("zone") == 2,
  "Remove touches only that type")
check(ns.Journal.Remove("zone", "nope") == 0 and #ns.db.journal == 5, "Remove without a function: nothing")

print(string.format("journal prune: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
