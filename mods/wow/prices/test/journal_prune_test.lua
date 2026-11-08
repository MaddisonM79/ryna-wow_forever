-- The core's journal prune for the Prices mod's `auction` events (cores/wow/addon/Ryna/Journal.lua,
-- J.SHORT_LIVED): they go at login once the companion app has uploaded the save that carried them,
-- and after three days whatever happens; every other event keeps the journal's 30 days. Loads
-- Journal.lua alone with a stub namespace. From the repository root, with Lua 5.1:
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

local function prune(journal, status)
  local ns = { db = { journal = journal }, Now = function() return NOW end, Debug = function() end,
    Debugging = function() return false end }
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

-- The app pushed the save that holds every event: auction events go, the rest stay.
local pushed = { accounts = { { key = "a", savedAt = newest + 30, pushedAt = newest + 90 } } }
check(prune(events(), pushed) == "zone@432000 gold@600", "uploaded: every auction event goes: "
  .. prune(events(), pushed))

-- Saved after the events but not pushed yet (pushedAt older than savedAt): kept.
local unpushed = { accounts = { { key = "a", savedAt = newest + 30, pushedAt = newest - 100 } } }
check(prune(events(), unpushed) == "zone@432000 auction@172800 auction@3600 gold@600",
  "saved but not pushed: kept (3 days): " .. prune(events(), unpushed))

-- The last push was of a save older than the newest event (the app hasn't seen the last save): kept.
local stale = { accounts = { { key = "a", savedAt = newest - 1000, pushedAt = newest - 900 } } }
check(prune(events(), stale) == "zone@432000 auction@172800 auction@3600 gold@600",
  "the pushed save predates the newest event: kept: " .. prune(events(), stale))

-- Two accounts, one with a save that failed to push: kept.
local two = { accounts = {
  { key = "a", savedAt = newest + 30, pushedAt = newest + 90 },
  { key = "b", savedAt = newest - 5000, pushedAt = nil },
} }
check(prune(events(), two) == "zone@432000 auction@172800 auction@3600 gold@600",
  "another account's save not pushed: kept: " .. prune(events(), two))
-- An account the app knows of but that never saved anything doesn't hold it up.
local idle = { accounts = { { key = "a", savedAt = newest + 30, pushedAt = newest + 90 }, { key = "b" } } }
check(prune(events(), idle) == "zone@432000 gold@600", "an account with no save doesn't count: "
  .. prune(events(), idle))

-- Malformed status: kept.
check(prune(events(), { accounts = "nope" }) == "zone@432000 auction@172800 auction@3600 gold@600",
  "a malformed AgentStatus: kept")

print(string.format("journal prune: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
