-- Tabs: Chronicle's two tabs in the core's panel (Ryna.Panel.AddTab), Recording and Census, built
-- with the core's widgets (Ryna.Panel.Widgets). The combat-log controls are the core's (its
-- Advanced tab).

local _, ns = ...
if ns.off then return end
local L = ns.L

local W = ns.API.Panel.Widgets
local WIDTH = W.WIDTH
local text, button, link, checkbox, toggle = W.text, W.button, W.link, W.checkbox, W.toggle

-- The tabs' controls.
local P = {}
P.Refresh = function() ns.API.Panel.Refresh() end

local build, layout = {}, {}

function build.recording(f)
  P.recordingIntro = text(f, "GameFontHighlightSmall", 11)
  P.recordingIntro:SetText(L["About your own characters, Ryna records their gear, bags and bank, quests, "
    .. "professions, reputations, gold and played time, your guild's roster, and a 30-day journal of what "
    .. "you do. Other players are the Census tab's."])
  P.chronicle = checkbox(f, L["Record kills and loot"],
    L["Counts the creatures you and your group kill, the experience they give and what you loot from them, "
    .. "as running totals on the character (not journal events). Off: nothing is counted, and the totals so "
    .. "far are kept."])
  toggle(P.chronicle, "chronicle")
  P.tooltips = checkbox(f, L["Show my numbers on tooltips"],
    L["Adds your own kills and drops to creature and item tooltips: how many you killed, what they dropped "
    .. "for you and how often."])
  toggle(P.tooltips, "tooltips")
end

function layout.recording(s, settings)
  s.text(P.recordingIntro, 0, 12)
  P.chronicle:SetChecked(settings.chronicle)
  P.chronicle.place(s, 4)
  P.tooltips:SetChecked(settings.tooltips)
  P.tooltips.place(s, 4)
end

function build.census(f)
  P.census = checkbox(f, L["Take part in the census"],
    L["Records other players you see (nearby, in chat, your group and your guild). "
    .. "Off: Ryna records only your own characters."])
  P.chat = checkbox(f, L["Record who I see in chat"],
    L["Only who spoke, never what they said or in which channel."])
  P.group = checkbox(f, L["Record other players in my group"],
    L["Who was in your groups."])
  P.nearby = checkbox(f, L["Record players I see nearby"],
    L["Players whose nameplates, target or mouseover you see: their level, class and guild, and where."])
  P.nameplateHint = text(f, "GameFontDisableSmall", 10)
  P.nameplateHint:SetWidth(WIDTH - 100)
  P.nameplateHint:SetText(L["The game shows nameplates for enemies only unless friendly nameplates are on "
    .. "(Shift+V). Ryna only sees nameplates the game draws."])
  -- Sets the game's nameplateShowFriends only when clicked; the addon never changes it by itself.
  P.friendly = button(f, L["Turn friendly nameplates on"], 240, 22)
  if P.friendly then
    P.friendly:SetScript("OnClick", function()
      ns.Run("friendly nameplates", function()
        ns.Sightings.ShowFriendlyNameplates()
        P.Refresh()
      end)
    end)
  end
  P.sweepHelp = L["Every 30 minutes while you're parked in a capital, Ryna asks the server who is online (/who), "
    .. "a few queries at a time, to count the realm's players by level, class and zone. "
    .. "Nothing about you is sent."]
  P.sweep = checkbox(f, L["Run a realm census while I'm in a city"], P.sweepHelp)
  toggle(P.census, "census")
  toggle(P.chat, "chatSightings")
  toggle(P.group, "groupMembers")
  toggle(P.nearby, "nearbyPlayers")
  toggle(P.sweep, "censusSweep")
  -- The click itself sends the query (C.Request runs synchronously inside OnClick): some clients
  -- accept /who only from a hardware event.
  P.sweepButton = button(f, L["Run census now"], 220, 22)
  if P.sweepButton then
    P.sweepButton:SetScript("OnClick", function()
      ns.Run("census sweep", function()
        ns.Census.Request()
        P.Refresh()
      end)
    end)
    P.sweepRetry = link(f, L["Try again"])
    if P.sweepRetry then
      P.sweepRetry:SetPoint("LEFT", P.sweepButton, "RIGHT", 8, 0)
      P.sweepRetry:SetScript("OnClick", function()
        ns.Run("census try again", function()
          ns.Census.TryAgain()
          P.Refresh()
        end)
      end)
    end
    P.sweepText = text(f, "GameFontHighlightSmall", 11)
    P.sweepText:SetWidth(WIDTH - 320)
    -- ">" rather than an arrow: the game's font has no arrow glyph.
    P.keyHint = text(f, "GameFontDisableSmall", 10)
    P.keyHint:SetWidth(WIDTH - 100)
    P.keyHint:SetText(L["Bind a key to Next census query in Key Bindings > AddOns > Ryna to run a sweep "
      .. "with taps instead of clicks. /ryna census does the same from chat."])
  end
  P.privacy = text(f, "GameFontDisableSmall", 10)
  P.privacy:SetText(L["The census keeps who was seen, never what they said. "
    .. "Its data is never sold or used for advertising."])
end

function layout.census(s, settings)
  P.census:SetChecked(settings.census)
  P.chat:SetChecked(settings.chatSightings)
  P.group:SetChecked(settings.groupMembers)
  P.nearby:SetChecked(settings.nearbyPlayers)
  P.sweep:SetChecked(settings.censusSweep)
  local C = ns.Census
  local mode = C and C.Mode()
  -- Automatic sweeps send from a timer, which a click-only or blocked client refuses.
  local clickOnly = mode == "click" or mode == "blocked"
  P.sweep.help:SetText(clickOnly and L["The game only allows /who from a click on this client."] or P.sweepHelp)
  P.census.place(s, 4)
  for _, cb in ipairs({ P.chat, P.group, P.nearby, P.sweep }) do
    cb.setActive(settings.census and not (cb == P.sweep and clickOnly))
    if cb == P.sweep and clickOnly and settings.census then cb.help:SetAlpha(1) end
    cb.place(s, 30)
    if cb == P.nearby then
      P.nameplateHint:SetAlpha(settings.census and 1 or 0.5)
      s.text(P.nameplateHint, 60, 4)
      if P.friendly then
        local on = ns.Sightings.FriendlyNameplatesOn()
        P.friendly:SetText(on and L["Friendly nameplates are on"] or L["Turn friendly nameplates on"])
        if settings.census and not on then P.friendly:Enable() else P.friendly:Disable() end
        s.add(P.friendly, 22, 60, 12)
      end
    end
  end
  if P.sweepButton then
    local label, usable = L["Run census now"], false
    if C then
      usable = settings.census and C.Available() and true or false
      if mode == "blocked" then
        label, usable = L["Census: not allowed by the game"], false
      elseif mode == "click" or C.Stepping() or (C.Running() and C.sweep.byClicks) then
        label = string.format(L["Next census query (%d of %d)"], C.NextStep())
        usable = usable and (not C.Running() or C.Stepping())
      else
        usable = usable and not C.Running()
      end
    end
    P.sweepButton:SetText(label)
    if usable then P.sweepButton:Enable() else P.sweepButton:Disable() end
    local retry = P.sweepRetry and mode == "blocked"
    if P.sweepRetry then
      if retry then P.sweepRetry:Show() else P.sweepRetry:Hide() end
    end
    P.sweepText:ClearAllPoints()
    P.sweepText:SetPoint("LEFT", retry and P.sweepRetry or P.sweepButton, "RIGHT", 8, 0)
    P.sweepText:SetText(C and C.StatusLine() or "")
    s.add(P.sweepButton, 22, 34, 6)
    s.text(P.keyHint, 34, 14)
  end
  s.text(P.privacy, 4, 6)
end

ns.API.Panel.AddTab({ key = "recording", label = L["Recording"], order = 20,
  build = build.recording, layout = layout.recording })
ns.API.Panel.AddTab({ key = "census", label = L["Census"], order = 30, build = build.census, layout = layout.census })
