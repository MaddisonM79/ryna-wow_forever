-- Journal: the append-only event table, docs/wow.md section 2 "journal".
-- Every event is { session, seq, at, char, type, ...fields }. A session is
-- "<login unix>-<GUID hex>", and seq restarts at 1 per session.

local _, ns = ...

local J = {}
ns.Journal = J

local KEEP_SECONDS = 30 * 86400
local CAP = 100000 -- launch week: the addon may run days before the agent first uploads
local SLACK = 1000 -- compact only when this far over the cap, so appends stay cheap

local function keepNewest(list, n)
  if #list <= n then return list end
  local out = {}
  for i = #list - n + 1, #list do out[#out + 1] = list[i] end
  return out
end

-- Bulky event types, kept only until the companion app has uploaded them (this account's
-- journalThrough in AgentStatus.lua), and never longer than this many seconds. The Prices mod's
-- `auction` snapshots run to megabytes each, and the API's ingest takes at most 16 MB and keeps
-- every event it stores, so carrying them for 30 days would fill it.
J.SHORT_LIVED = { auction = 3 * 86400 }

-- This account's entry in the companion app's AgentStatus.lua (RynaAgentStatus, rewritten after each
-- sync and read at login): the one whose accountId is RynaDB.meta.accountId. Nil when the app
-- predates the id, or hasn't pushed this account yet.
local function myAccount()
  local a = _G.RynaAgentStatus
  local accounts = type(a) == "table" and a.accounts
  local id = type(ns.db) == "table" and type(ns.db.meta) == "table" and ns.db.meta.accountId
  if type(accounts) ~= "table" or type(id) ~= "string" then return nil end
  for _, acct in ipairs(accounts) do
    if type(acct) == "table" and acct.accountId == id then return acct end
  end
  return nil
end

-- Whether the companion app has uploaded event `e`: this account's journalThrough (the newest event
-- `at` the app pushed for it) has reached it. Another account's push says nothing about this one.
function J.Uploaded(e, acct)
  acct = acct or myAccount()
  return type(acct) == "table" and type(acct.journalThrough) == "number" and type(e) == "table"
    and type(e.at) == "number" and acct.journalThrough >= e.at
end

-- A mod's view of the journal (Ryna.Journal.Events, .Remove): the events of one type, and removing
-- some of them. The Prices mod uses them to keep its bulky `auction` events in bounds.
function J.Events(eventType)
  local out = {}
  local journal = ns.db and ns.db.journal
  if type(journal) ~= "table" then return out end
  for i = 1, #journal do
    local e = journal[i]
    if type(e) == "table" and e.type == eventType then out[#out + 1] = e end
  end
  return out
end

-- Removes the events of `eventType` for which drop(e) is true; returns how many went.
function J.Remove(eventType, drop)
  local journal = ns.db and ns.db.journal
  if type(journal) ~= "table" or type(drop) ~= "function" then return 0 end
  local out, n = {}, 0
  for i = 1, #journal do
    local e = journal[i]
    if type(e) == "table" and e.type == eventType and drop(e) then
      n = n + 1
    else
      out[#out + 1] = e
    end
  end
  if n > 0 then ns.db.journal = out end
  return n
end

-- On login: drop events older than 30 days (and anything malformed), the short-lived types once
-- uploaded or past their own limit, then apply the cap.
function J.Prune()
  local now = ns.Now()
  local cutoff = now - KEEP_SECONDS
  local old = ns.db.journal
  local acct = myAccount()
  local out, short = {}, 0
  for i = 1, #old do
    local e = old[i]
    if type(e) == "table" and type(e.at) == "number" and e.at >= cutoff then
      local limit = J.SHORT_LIVED[e.type]
      if limit and (J.Uploaded(e, acct) or e.at < now - limit) then
        short = short + 1
      else
        out[#out + 1] = e
      end
    end
  end
  ns.db.journal = keepNewest(out, CAP)
  ns.Debug("journal: %d events kept from earlier sessions, %d older than 30 days dropped, %d bulky ones "
    .. "dropped (uploaded or past their limit)", #ns.db.journal, #old - #ns.db.journal - short, short)
end

function J.StartSession(guid)
  local hex = (type(guid) == "string" and guid:match("%-(%x+)$")) or "0"
  J.loginAt = ns.Now()
  J.session = string.format("%d-%s", J.loginAt, hex)
  J.seq = 0
  J.char = guid
end

-- One debug line per appended event: "journal: questDone 123 "Title"", "census: seen <name> in chat".
local SEEN_WHERE = {
  chat = "in chat", group = "in your group", roster = "on the guild roster",
  nameplate = "nearby (nameplate)", target = "nearby (target)", mouseover = "nearby (mouseover)",
  who = "from /who",
}

local function quoted(s)
  return s and (' "' .. tostring(s) .. '"') or ""
end

local function describe(e)
  local t = e.type
  if t == "seen" then
    return string.format("census: seen %s%s %s%s", tostring(e.fullName or e.guid),
      e.faction and (" (" .. e.faction .. ")") or "", SEEN_WHERE[e.source] or ("from " .. tostring(e.source)),
      e.zone and (" in " .. e.zone) or "")
  elseif t == "questAccept" then
    return string.format("journal: questAccept %s%s%s", tostring(e.questId), quoted(e.title),
      e.level and (" (level " .. tostring(e.level) .. ")") or "")
  elseif t == "questDone" then
    local title = ns.NonEmpty(ns.Try(ns.Fn("C_QuestLog", "GetTitleForQuestID"), e.questId))
    return string.format("journal: questDone %s%s, %s xp, %s", tostring(e.questId), quoted(title),
      tostring(e.xp or 0), ns.MoneyText(e.gold or 0))
  elseif t == "zone" then
    local where = tostring(e.zone or "unknown zone") .. (e.subzone and (", " .. e.subzone) or "")
    if e.instance then
      where = where .. " (instance " .. e.instance .. (e.difficulty and (", " .. e.difficulty) or "") .. ")"
    end
    return "journal: zone " .. where
  elseif t == "gold" then
    return string.format("journal: gold %s%s, now %s", (e.delta or 0) > 0 and "+" or "",
      ns.MoneyText(e.delta), ns.MoneyText(e.total))
  elseif t == "level" then
    return string.format("journal: level %s (played %s in all)", tostring(e.level), ns.DurationText(e.playedTotal))
  elseif t == "group" then
    return string.format("journal: group %s, %s", tostring(e.kind),
      e.members and (#e.members .. " other members") or "members not recorded")
  elseif t == "who" then
    return string.format("census: your /who %s answered %s (answer %s)", tostring(e.query), tostring(e.count),
      tostring(e.batch))
  elseif t == "session" then
    return "journal: session " .. tostring(e.kind) .. (e.zone and (" in " .. e.zone) or "")
  elseif t == "death" then
    return "journal: death" .. (e.zone and (" in " .. e.zone) or "")
  end
  return "journal: " .. tostring(t)
end

-- quiet: no debug line for this event (the caller prints one summary for a batch of them).
function J.Add(eventType, fields, quiet)
  if not J.session then return nil end
  J.seq = J.seq + 1
  local e = { session = J.session, seq = J.seq, at = ns.Now(), char = J.char, type = eventType }
  if fields then
    for k, v in pairs(fields) do e[k] = v end
  end
  local journal = ns.db.journal
  journal[#journal + 1] = e
  if #journal > CAP + SLACK then ns.db.journal = keepNewest(journal, CAP) end
  if not quiet and ns.Debugging() then
    local ok, line = pcall(describe, e)
    if ok and line then ns.Debug("%s", line) end
  end
  return e
end

function J.Count()
  return ns.db and #ns.db.journal or 0
end
