-- Mail: at a mailbox, take the gold and the items out of every mail that isn't cash on delivery. A
-- toggle (autoMail), off by default. Shift held or combat when the mailbox opens: nothing this visit.
--
-- One take at a time (TakeInboxMoney, TakeInboxItem), at most one every 0.3 seconds, the next one
-- after the inbox says it changed (MAIL_INBOX_UPDATE; after two seconds without word it looks anyway).
-- The inbox is read again before every take, since a mail emptied of everything may go. A mail with a
-- cash-on-delivery charge is never opened: paying is the player's call. A take the server didn't do
-- (nothing changed after it, twice) isn't tried again this visit. It stops when the inbox closes (or
-- the player moves to the Send tab), in combat, and when the bags are full (said once). Nothing is
-- ever deleted or returned: a mail left empty is the game's to tidy away, as with the game's own
-- "Open All". One line at the end: the items and gold taken, and the cash-on-delivery mail left.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local M = {}
ns.Mail = M

local INTERVAL = 0.3
local FALLBACK = 2
local visit, open, run = 0, false, nil

-- Seconds, finer than ns.Now's whole ones.
local function clock()
  return tonumber((ns.G("GetTime"))) or ns.Now()
end

local function inboxOpen(v)
  return open and v == visit and ns.Shown("InboxFrame")
end

-- Free slots in the ordinary bags; nil when this client can't say (then it isn't a reason to stop).
function M.FreeSlots()
  local total = ns.G("CalculateTotalNumberOfFreeBagSlots") or ns.C("C_Container", "CalculateTotalNumberOfFreeBagSlots")
  if type(total) == "number" then return total end
  if not ns.Fn("C_Container", "GetContainerNumFreeSlots") then return nil end
  local free = 0
  for bag = 0, tonumber(_G.NUM_BAG_SLOTS) or 4 do
    local n, family = ns.C("C_Container", "GetContainerNumFreeSlots", bag)
    if type(n) == "number" and (family == nil or family == 0) then free = free + n end
  end
  return free
end

-- One read of the inbox: how many mails are cash on delivery, a signature of what's in it (so a take
-- that changed nothing shows), and the next thing to take ({ i, kind, j, key, money, count }).
function M.Scan(skip)
  local cod, sumMoney, sumItems, target = 0, 0, 0, nil
  local n = tonumber((ns.G("GetInboxNumItems"))) or 0
  local slots = tonumber(_G.ATTACHMENTS_MAX_RECEIVE) or 16
  for i = 1, n do
    local _, _, sender, subject, money, codAmount, _, itemCount = ns.G("GetInboxHeaderInfo", i)
    money, itemCount = tonumber(money) or 0, tonumber(itemCount) or 0
    if (tonumber(codAmount) or 0) > 0 then
      cod = cod + 1
    else
      local base = tostring(sender) .. "|" .. tostring(subject)
      sumMoney = sumMoney + money
      if money > 0 and not target and not skip[base .. "|money"] then
        target = { i = i, kind = "money", key = base .. "|money", money = money, count = 0 }
      end
      if itemCount > 0 then
        for j = 1, slots do
          local name = ns.G("GetInboxItem", i, j)
          if name then
            sumItems = sumItems + 1
            local key = base .. "|item" .. j
            if not target and not skip[key] then
              target = { i = i, kind = "item", j = j, key = key, money = 0, count = 1 }
            end
          end
        end
      end
    end
  end
  return cod, n .. ":" .. sumMoney .. ":" .. sumItems, target
end

local function finish(st)
  if st.done then return end
  st.done = true
  if run == st then run = nil end
  local parts = {}
  if st.items > 0 then parts[#parts + 1] = string.format(st.items == 1 and L["%d item"] or L["%d items"], st.items) end
  if st.money > 0 then parts[#parts + 1] = ns.MoneyText(st.money) end
  if #parts > 0 then
    local what = #parts == 2 and string.format(L["%s and %s"], parts[1], parts[2]) or parts[1]
    ns.Report(string.format(L["Took %s from the mail."], what))
  end
  if st.cod > 0 then
    ns.Report(string.format(st.cod == 1 and L["Left %d cash-on-delivery mail for you."]
      or L["Left %d cash-on-delivery mails for you."], st.cod))
  end
end

local step

-- The next step after `delay`, unless a newer one has been asked for since.
local function later(st, delay)
  st.seq = st.seq + 1
  local seq = st.seq
  ns.After(delay, "automation mail", function()
    if st.seq == seq then step(st) end
  end)
end

step = function(st)
  if st.done then return end
  if not inboxOpen(st.v) or ns.InCombat() then return finish(st) end
  st.waiting = false
  local cod, sig, target = M.Scan(st.skip)
  st.cod = cod
  if st.last and sig == st.lastSig then
    -- Nothing changed since the last take: give the server a moment, then give that one up.
    st.stale = st.stale + 1
    if st.stale < 2 then
      st.waiting = true
      return later(st, FALLBACK)
    end
    st.skip[st.last.key] = true
    st.items, st.money = st.items - st.last.count, st.money - st.last.money
    st.cod, sig, target = M.Scan(st.skip)
  end
  st.stale, st.last = 0, nil
  if not target then return finish(st) end
  if target.kind == "item" then
    local free = M.FreeSlots()
    if free and free <= 0 then
      ns.Report(L["Your bags are full, so the rest of the mail is still there."])
      return finish(st)
    end
    ns.G("TakeInboxItem", target.i, target.j)
  else
    ns.G("TakeInboxMoney", target.i)
  end
  st.items, st.money = st.items + target.count, st.money + target.money
  st.last, st.lastSig, st.at, st.waiting = target, sig, clock(), true
  later(st, FALLBACK)
end

ns.On("MAIL_SHOW", function()
  -- A mailbox that shows again (a re-fired event) is the same visit.
  if not open then visit = visit + 1 end
  open = true
  if not S.On("autoMail") then return end
  local needs = { "GetInboxNumItems", "GetInboxHeaderInfo", "TakeInboxMoney", "TakeInboxItem", "GetInboxItem" }
  for _, name in ipairs(needs) do
    if not ns.Need("mail", name) then return end
  end
  ns.Soon("mail", function() return inboxOpen(visit) end, function()
    if run and run.v == visit then return end
    run = { v = visit, items = 0, money = 0, cod = 0, skip = {}, stale = 0, seq = 0 }
    step(run)
  end)
end)

ns.On("MAIL_INBOX_UPDATE", function()
  local st = run
  if not st or not st.waiting or st.done then return end
  local since = clock() - (st.at or 0)
  later(st, math.max(0, INTERVAL - since))
end)

ns.On("MAIL_CLOSED", function()
  open = false
  visit = visit + 1
  if run then finish(run) end
end)
