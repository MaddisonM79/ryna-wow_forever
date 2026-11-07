# Cores

A core is a game behind the agent: it knows how to find the game, read what the game leaves on disk,
push it to ryna.io, and bring data back down, and it runs fine with no mods. Each core has a folder
here with its manifest, `core.json` (id, name, version, `runtime` builtin or sidecar, the token
`resource` and `scopes` it needs, the folders it `detect`s the game in, the game files it `reads`
and `writes`, and where its `mods` live), which the agent reads at start; the agent drives the core
through one interface, `ryna_agent::core::Core` in `agent/host`. Today there is one, `wow/`: the
Rust core (`wow/core`, crate `ryna-wow-core`), its in-game addon (`wow/addon/Ryna`), and the test
fixtures. See `docs/ryna-agent-cores-mods.md` (sections 4 and 5).
