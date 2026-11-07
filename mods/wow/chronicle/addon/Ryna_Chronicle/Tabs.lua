-- Tabs: Chronicle's two tabs in the core's panel (Ryna.Panel.AddTab), Recording and Census, built
-- with the core's widgets (Ryna.Panel.Widgets). The combat-log controls are the core's (its
-- Advanced tab).

local _, ns = ...
if ns.off then return end
local L = ns.L

local W = ns.API.Panel.Widgets
local WIDTH = W.WIDTH
local text, button, checkbox, toggle = W.text, W.button, W.checkbox, W.toggle

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
  P.censusIntro = text(f, "GameFontHighlightSmall", 11)
  P.censusIntro:SetText(L["Ryna never sends /who itself. When you type /who, the results are recorded as "
    .. "sightings, like players you see on nameplates or in chat."])
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
  toggle(P.census, "census")
  toggle(P.chat, "chatSightings")
  toggle(P.group, "groupMembers")
  toggle(P.nearby, "nearbyPlayers")
  P.privacy = text(f, "GameFontDisableSmall", 10)
  P.privacy:SetText(L["The census keeps who was seen, never what they said. "
    .. "Its data is never sold or used for advertising."])
end

function layout.census(s, settings)
  s.text(P.censusIntro, 0, 12)
  P.census:SetChecked(settings.census)
  P.chat:SetChecked(settings.chatSightings)
  P.group:SetChecked(settings.groupMembers)
  P.nearby:SetChecked(settings.nearbyPlayers)
  P.census.place(s, 4)
  for _, cb in ipairs({ P.chat, P.group, P.nearby }) do
    cb.setActive(settings.census)
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
  s.text(P.privacy, 4, 6)
end

ns.API.Panel.AddTab({ key = "recording", label = L["Recording"], order = 20,
  build = build.recording, layout = layout.recording })
ns.API.Panel.AddTab({ key = "census", label = L["Census"], order = 30, build = build.census, layout = layout.census })
