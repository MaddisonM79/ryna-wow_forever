-- Social: accepting a resurrection, and a group invite from a friend or a guildmate. Each its own
-- toggle (Settings.lua), off by default; Shift held or combat when it would act, and the game's
-- dialog stays for the player.
--
-- A resurrection is taken a frame after RESURRECT_REQUEST, out of combat only, and only while the
-- game's dialog for it is still up (another addon or the player may have answered it). An invite is
-- taken only from a character friend (C_FriendList), a Battle.net friend (C_BattleNet) or someone in
-- the guild (IsGuildMember, else the guild roster by name); from anyone else the dialog stays. The
-- invite dialog is hidden once the group forms (GROUP_ROSTER_UPDATE), as ElvUI does: hiding it at
-- once can read as a decline.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local Social = {}
ns.Social = Social

-- Resurrections ---------------------------------------------------------------------------------

local RES, RES_NO_SICKNESS, RES_NO_TIMER = "RESURRECT", "RESURRECT_NO_SICKNESS", "RESURRECT_NO_TIMER"

ns.On("RESURRECT_REQUEST", function(_, caster)
  if not S.On("autoAcceptRes") then return end
  if ns.InCombat() then
    ns.Debug("automation: resurrection: in combat, leaving the dialog")
    return
  end
  if not ns.Need("acceptRes", "AcceptResurrect") then return end
  ns.Soon("resurrection", function()
    return not ns.InCombat() and ns.PopupShown(RES, RES_NO_SICKNESS, RES_NO_TIMER)
  end, function()
    ns.G("AcceptResurrect")
    ns.HidePopups(RES, RES_NO_SICKNESS, RES_NO_TIMER)
    local who = ns.NonEmpty(caster)
    ns.Report(who and string.format(L["Accepted %s's resurrection."], who) or L["Accepted a resurrection."])
  end)
end)

-- Invites ---------------------------------------------------------------------------------------

-- "Name" and "Name-Realm" are the same character when one of them has no realm.
local function sameName(a, b)
  a, b = ns.NonEmpty(a), ns.NonEmpty(b)
  if not a or not b then return false end
  a, b = a:lower(), b:lower()
  if a == b then return true end
  local baseA, realmA = a:match("^([^%-]+)%-(.+)$")
  local baseB, realmB = b:match("^([^%-]+)%-(.+)$")
  if realmA and realmB then return false end
  return (baseA or a) == (baseB or b)
end

local function guid(v)
  if type(v) ~= "string" or v == "" or ns.IsSecret(v) then return nil end
  return v
end

local function characterFriend(name, id)
  if id and ns.C("C_FriendList", "IsFriend", id) then return true end
  local info = name and ns.C("C_FriendList", "GetFriendInfo", name)
  return type(info) == "table"
end

local function bnetFriend(name, id)
  if id and type(ns.C("C_BattleNet", "GetGameAccountInfoByGUID", id)) == "table" then return true end
  local n = tonumber((ns.G("BNGetNumFriends"))) or 0
  for i = 1, n do
    local account = ns.C("C_BattleNet", "GetFriendAccountInfo", i)
    local game = type(account) == "table" and account.gameAccountInfo or nil
    if type(game) == "table" then
      if id and game.playerGuid == id then return true end
      local who = game.characterName
      if ns.NonEmpty(game.realmName) and ns.NonEmpty(who) then who = who .. "-" .. game.realmName end
      if sameName(name, who) then return true end
    else
      -- An older client: the fifth value is the character's name.
      local _, _, _, _, character = ns.G("BNGetFriendInfo", i)
      if sameName(name, character) then return true end
    end
  end
  return false
end

local function guildmate(name, id)
  local inGuild = ns.Fn("IsInGuild")
  if inGuild and not ns.Try(inGuild) then return false end
  local isMember = ns.Fn("IsGuildMember")
  if isMember and id then return ns.Try(isMember, id) and true or false end
  for i = 1, tonumber((ns.G("GetNumGuildMembers"))) or 0 do
    if sameName(name, (ns.G("GetGuildRosterInfo", i))) then return true end
  end
  return false
end

-- Whether the inviter is someone the player knows, and which kind (for the chat line).
function Social.Known(name, id)
  id = guid(id)
  if characterFriend(name, id) then return "friend" end
  if bnetFriend(name, id) then return "bnet" end
  if guildmate(name, id) then return "guild" end
  return nil
end

local hidePending = false

ns.On("PARTY_INVITE_REQUEST", function(_, name, _, _, _, _, _, inviterGUID)
  if not S.On("autoAcceptInvites") then return end
  if not ns.Need("acceptInvites", "AcceptGroup") then return end
  ns.Soon("invite", function() return ns.PopupShown("PARTY_INVITE") end, function()
    local kind = Social.Known(name, inviterGUID)
    local who = ns.NonEmpty(name) or L["someone"]
    if not kind then
      ns.Debug("automation: invite from %s: not a friend or guildmate, leaving it", who)
      return
    end
    hidePending = true
    ns.G("AcceptGroup")
    ns.Report(string.format(kind == "guild" and L["Joined %s's group (your guild)."]
      or L["Joined %s's group (a friend)."], who))
    -- If the group never says it formed, the dialog is left as it is.
    ns.After(2, "automation invite", function() hidePending = false end)
  end)
end)

ns.On("GROUP_ROSTER_UPDATE", function()
  if not hidePending then return end
  hidePending = false
  ns.HidePopups("PARTY_INVITE")
end)
