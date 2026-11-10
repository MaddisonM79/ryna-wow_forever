-- The core addon's supported-versions view (Status.lua's St.Support, St.SupportLines and St.Messages,
-- and Notice.lua's login notice), against a small fake client: loads Core.lua, Locales/enUS.lua,
-- Status.lua and Notice.lua with RynaAgentStatus as the companion app writes it, for each band. From
-- the repository root, with Lua 5.1 (what the game runs) or later:
--
--   lua5.1 cores/wow/addon/test/status_test.lua
--
-- This proves the logic; how the line and the notice look is the game's to show.
-- luacheck: ignore 1

local CORE = "cores/wow/addon/Ryna/"

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end
local function eq(got, want, what)
  check(got == want, what .. ": got " .. tostring(got) .. ", want " .. tostring(want))
end

-- The fake client: frames that keep their text, shown state and scripts; everything else a no-op.
local chat = {}
local frames = {}
local methods = {}
local function new(kind, name)
  local o = { _kind = kind, _name = name, _shown = true, _scripts = {} }
  setmetatable(o, { __index = function(_, k)
    if methods[k] then return methods[k] end
    return function() end
  end })
  frames[#frames + 1] = o
  if name then _G[name] = o end
  return o
end
function methods.SetText(self, t) self._text = t end
function methods.GetText(self) return self._text end
function methods.GetFont() return "Fonts\\FRIZQT__.TTF", 12 end
function methods.GetStringHeight() return 12 end
function methods.GetFontString(self) return self._fs end
function methods.SetFontString(self, fs) self._fs = fs end
function methods.CreateFontString() return new("FontString") end
function methods.CreateTexture() return new("Texture") end
function methods.SetScript(self, k, f) self._scripts[k] = f end
function methods.GetScript(self, k) return self._scripts[k] end
function methods.Show(self) self._shown = true end
function methods.Hide(self) self._shown = false end
function methods.IsShown(self) return self._shown end
function methods.SetHeight(self, h) self._height = h end
function methods.GetHeight(self) return self._height or 0 end

_G.CreateFrame = function(kind, name, _, template)
  local f = new(kind, name)
  if kind == "Button" and template == "UIPanelButtonTemplate" then f._fs = new("FontString") end
  return f
end
_G.UIParent = new("Frame", "UIParent")
_G.UISpecialFrames = {}
_G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) chat[#chat + 1] = msg end }
_G.GetLocale = function() return "enUS" end
_G.time = function() return 1800000000 end
_G.GetServerTime = _G.time
_G.date = os.date
_G.C_Timer = { After = function() end }
_G.C_AddOns = { GetAddOnMetadata = function() return "0.3.10" end }

local ns = {}
for _, file in ipairs({ "Core.lua", "Locales/enUS.lua", "Status.lua", "Notice.lua" }) do
  assert(loadfile(CORE .. file))("Ryna", ns)
end
local St, N = ns.Status, ns.Notice

local REDUCED = "This version of Ryna is getting old. Your characters still sync, but what you add to "
  .. "everyone's census and prices is paused until you update."
local UNSUPPORTED = "This version of Ryna is no longer supported, so nothing syncs to ryna.io. Update to carry on."
local HOW = "Open the Ryna app to update."

-- Each case starts a new login.
local function login(status)
  _G.RynaAgentStatus = status
  N.shown = nil
  if N.frame then N.frame:Hide() end
  N.ShowAtLogin()
end

local function shown()
  return N.frame ~= nil and N.frame:IsShown()
end

-- Nothing written yet, or no judgment: nothing said, no notice.
login(nil)
eq(#St.SupportLines(), 0, "no status file: no support line")
eq(shown(), false, "no status file: no notice")
login({ agent = "0.3.10", messages = { { at = 1, level = "error", text = "Couldn't reach the API" } } })
eq(#St.SupportLines(), 0, "no support table: no line")
eq(#St.Messages(10), 1, "the app's other messages are shown")

-- Current and behind have no line here (behind is the Update available lines); neither does a band
-- this addon doesn't know (failing open).
for _, band in ipairs({ "current", "behind", "sideways" }) do
  login({ support = { band = band, perfect = "0.3.11", note = "a note" } })
  eq(#St.SupportLines(), 0, band .. ": no support line")
  eq(shown(), false, band .. ": no notice")
end
login({ agent = "0.3.10", latest = { agent = "0.3.11" }, support = { band = "behind", perfect = "0.3.11" } })
eq(St.UpdateLines()[1], "Update available: companion app 0.3.11 (you have 0.3.10).", "behind: the app's update line")

-- Malformed tables are ignored.
login({ support = "unsupported" })
eq(#St.SupportLines(), 0, "support that isn't a table: nothing")
login({ support = { band = 3 } })
eq(#St.SupportLines(), 0, "a band that isn't a string: nothing")

-- Reduced: the amber line with how to update, then the note; the app's copy of the sentence (for
-- addons that don't read `support`) isn't shown a second time; no notice at login.
login({
  support = { band = "reduced", version = "0.3.6", addon = "0.3.6", perfect = "0.3.10",
    note = "0.3.4 mis-reads | auction prices" },
  messages = {
    { at = 5, level = "warn", text = REDUCED },
    { at = 4, level = "error", text = "Couldn't reach the API" },
  },
})
local lines = St.SupportLines()
eq(#lines, 2, "reduced: the sentence and the note")
eq(lines[1], "|cffffb000" .. REDUCED .. " " .. HOW .. "|r", "reduced: amber, the sentence, how to update")
eq(lines[2], "|cffffb0000.3.4 mis-reads || auction prices|r", "reduced: the note, its | shown as typed")
local msgs = St.Messages(10)
eq(#msgs, 1, "reduced: the sentence isn't repeated among the messages")
eq(msgs[1].text, "Couldn't reach the API", "reduced: the other message stays")
eq(shown(), false, "reduced: no notice (the Status tab says it)")

-- Unsupported: red, and the notice at login, once, closed with its button.
login({
  support = { band = "unsupported", version = "0.3.1", perfect = "0.3.10", note = nil },
  messages = { { at = 5, level = "error", text = UNSUPPORTED } },
})
lines = St.SupportLines()
eq(#lines, 1, "unsupported, no note: one line")
eq(lines[1], "|cffff4040" .. UNSUPPORTED .. " " .. HOW .. "|r", "unsupported: red, the sentence, how to update")
eq(#St.Messages(10), 0, "unsupported: its message isn't shown twice")
eq(shown(), true, "unsupported: the notice at login")
local f = N.frame
eq(f.body:GetText(), UNSUPPORTED .. "\n\n" .. HOW, "the notice's words")
eq(f.title:GetText(), "|cffff4040Ryna needs updating|r", "the notice's title")
eq(f.note:GetText(), "", "no note, no note line")
f.close:GetScript("OnClick")()
eq(shown(), false, "its button closes it")
N.ShowAtLogin()
eq(shown(), false, "once per login: not shown again")
check(_G.UISpecialFrames[1] == "RynaVersionNotice", "Escape closes it")

-- With a note, and the next login shows it again.
login({ support = { band = "unsupported", note = "0.3.1 loses quests" } })
eq(shown(), true, "the next login: shown again")
eq(N.frame.note:GetText(), "0.3.1 loses quests", "the note under the words")
eq(N.Create(), N.frame, "one frame, made once")

-- An addon from before `support` existed reads the app's messages, so the sentence is there for it;
-- this addon shows it when there's no support table to say it instead.
login({ messages = { { at = 5, level = "error", text = UNSUPPORTED } } })
eq(St.Messages(10)[1].text, UNSUPPORTED, "no support table: the app's message is shown")

check(#chat == 0, "nothing in chat: " .. table.concat(chat, " / "))
check(ns.errors.count == 0, "no errors caught: " .. tostring(ns.errors.last))

print(string.format("status: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
