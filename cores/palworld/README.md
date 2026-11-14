# The Palworld core

Ryna's core for Palworld: it finds the Mac App Store build's worlds, waits for a save to settle,
parses it (`src/gvas`, `src/rawdata`, `src/sav.ts`: a port of palworld-save-tools, Oodle through
ooz-wasm), builds a snapshot (`src/snapshot`), and pushes the snapshot and the save files to the
API's Palworld ingest, then a heartbeat. It is a **sidecar**: the agent starts `node agent.mjs serve`
when the core starts and stops it with the core, and the two speak line-delimited JSON over stdin
and stdout (`src/serve.ts`; the interface is `docs/ryna-agent-cores-mods.md` 13.4). It is macOS
only for now (`core.json` `platforms`).

The agent paces it: each `sync` carries the agent's access token, and the core holds that token for
the one pass and nothing more. It keeps no credential on disk and never touches the keychain. Its
state folder (from `hello`) holds `state.json` (each world's last push fingerprint) and
`status.json` (the last passes, which the heartbeat sends).

What it sends keeps the launchd agent's shapes: the heartbeat's `agent` is this computer's host
name (the API keys a person's agents by it) and its `version` the core's; each snapshot's `agent` is
`{ version, saveTools }` with the core's version. The calls' User-Agent is `ryna-agent/<v> palworld/<v>`.

It reads only the save folder (`core.json` `data.reads`) and, to label a snapshot with the game's
version, `/Applications/Palworld.app/Contents/Info.plist`. It writes nothing in the game's folders.

## Working on it

```bash
pnpm install
pnpm typecheck && pnpm test && pnpm build      # dist/agent.mjs, one file (ooz's wasm is inlined)
pnpm agent snapshot /tmp/s.json [--world ID] [--root DIR]   # one world's snapshot, pushed nowhere
```

`snapshot` is for checking a parser fix by hand. After a game update breaks parsing, the fix is a
type hint (`src/gvas/hints.ts`) or a decoder (`src/rawdata/`); `pnpm exec tsx scripts/capture-rawdata.ts`
then refreshes the decoder byte fixtures `test/rawdata-fixtures.test.ts` replays.

**This repository is public.** Fixtures cut from a real save go through `scripts/scrub.ts` (names
become placeholders, every GUID the world generated a stand-in; `capture-rawdata.ts` does this
itself). Read the diff before committing one all the same.

The snapshot's `SCHEMA` and `test/fixtures/snapshot.json` are the wire contract with the API's
ingest, which refuses a schema it doesn't know: change them only with the API.

## The package

`node scripts/core-package.mjs` (from the repository root) builds
`ryna-core-palworld-<version>-<target>.tar.gz` for `aarch64-apple-darwin` and `x86_64-apple-darwin`:
Node's official binary for the target (checked against Node's `SHASUMS256.txt`), `agent.mjs`,
`core.json` and the licences, flat at the archive's root. The release signs each with the updater's
key. See `NOTICE.md` for the third-party code in it (ooz-wasm is GPL-3.0-or-later).
