-- Slash: Chronicle's /ryna command (Ryna.Slash.Add), /ryna census [stop], and the function behind
-- the core's "Next census query" key binding.
-- Command words are never translated.

local _, ns = ...
if ns.off then return end
local L, N = ns.L, ns.N

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

ns.API.Slash.Add({ word = "census", order = 50, run = censusCommand,
  help = L["/ryna census: run a realm census now (/who, a few queries at a time, or one per "
    .. "/ryna census where the game allows /who only from a click); /ryna census stop ends it"] })

-- The key binding (the core's Bindings.xml): a key press is a hardware event, like a click, so the
-- query goes out from here, synchronously; never move this call behind a timer.
ns.API.SetBinding("RYNA_CENSUS_NEXT", function() ns.Census.Request(true) end)
