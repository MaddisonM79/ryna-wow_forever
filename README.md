# ryna-wow_forever

The player-side half of a World of Warcraft: Forever companion: a game addon and a small agent that
runs on your computer, reads what the addon and the game write to disk, and sends summaries to a
backend. Characters, bags and bank, gold, quests, professions, the guild roster, and every fight
from the combat log, parsed locally into per-fight totals, ability breakdowns and timelines.

The hosted half (the API, the web portal with the trends and meters, and the assistant connector)
is private and runs at ryna.io for its own people. This repository is the part that lives on your
machine, published so it can be read, audited and reused. Anyone can run a backend that speaks the
same format; that is allowed and not a goal.

## Status

Pre-alpha. The design is settled and nothing here is usable yet. The order of work:

1. Addon and agent core: identity, gold, bags, bank, professions, reputations, quests, roster, pushed
   from a command-line agent on macOS.
2. Combat log: tailing, segmenting fights, aggregating, pushing summaries.
3. The desktop app (macOS and Windows), the only thing a player installs; it syncs while it is open.
4. Guild: roster-driven membership, guild bank, guild runs.

## What is in here

- `addon/`: the Lua addon, tagged for Forever. It writes a table to SavedVariables on logout and
  `/reload`, and turns combat logging on inside instances. It has no UI beyond `/ryna status`.
- `agent/`: the agent, a Rust workspace (`Cargo.toml` at the root). `agent/core` is the library:
  the SavedVariables reader, install discovery, the sign-in (OAuth device grant: the agent never
  sees your password), the pusher, and the watch loop. `agent/cli` is a command line over it for
  developers; the desktop app (`agent/app`) that people install runs the same core while it is
  open. `agent/fixtures` holds scrubbed test files. The combat-log parser comes in stage 2.
- `scripts/capture.sh`: turns a real `Ryna.lua` into a scrubbed fixture (the original stays in the
  gitignored `captures/`).

## What leaves your computer

Only what you can see in the addon's SavedVariables file and in the per-fight summaries the agent
computes: your characters, their inventory, quests, professions and reputations; the guild roster;
and for each fight, who was in it (character names and in-game ids), what they did, and when. Raw
combat-log text is uploaded only if you turn that on. Nothing is read from anywhere but the game's
own folder, and the agent signs in as you, to the backend you point it at.

The data is never sold and never used for advertising. That includes the pooled census of players
seen in the world, which exists so the people who play can see how many of them there are and
when. It is for the community that plays, and for nothing else.

## Licence

MIT. See `LICENSE`.
