# Superluna

A free, open, 16-bit-style platformer made for modding, with controls tuned to feel like the
classic SNES platformers, Super Mario World above all.

Years of brilliant level design live in ROM hacks that exist only at a publisher's discretion and
can't be freely shared. This project aims to be a legally clean home for that kind of creativity:
an engine whose run, jump and momentum feel right in the hands of anyone who grew up on Super
Mario World, with its own characters, art, music and levels, and a mod format built for sharing
from day one.

*Not affiliated with or endorsed by Nintendo. Super Mario World is a trademark of Nintendo. This
project contains no code, assets or level data from any Nintendo game, and it does not read,
import or patch ROMs or ROM hacks.*

## What "original" means here

- **Shared with the genre (and fair game):** controls, physics feel, momentum, and genre
  mechanics such as stomping, jump-through platforms, overworld maps and secret exits.
- **Ours alone:** characters, enemies, power-ups, art, music, sound, level layouts, names and
  overall look. Contributions must be original work. Never submit anything ripped, traced or
  converted from another game.

## Layout

```
crates/
  script-api/   language-neutral boundary: events in, commands out, engine-owned state
  core/         deterministic 60 Hz simulation (integer sub-pixels, no IO, no clock)
  script-rhai/  Rhai scripting backend (integers only, sandboxed)
  headless/     `ns` CLI for agents and CI: JSON-lines protocol, replays, text view
  bot/          `ns-bot`: searches for a run that completes a level and writes its replay
  frontend/     playable game (macroquad), desktop + web, placeholder art
  web-check/    replays every level inside WebAssembly to prove the web build matches desktop
content/
  feel.ron           movement feel parameters
  feel_targets.ron   what the feel must measure as (checked by `cargo test`)
  entities.ron       entity kinds: script, hitbox, flags
  levels/*.ron       levels as text grids, each with a verified *.replay
  scripts/*.rhai     entity behaviour
```

## Play

```sh
cargo run --release -p ns-frontend         # desktop
scripts/build-web.sh && python3 -m http.server -d web/dist 8080   # browser, then open localhost:8080
```

Controls: arrows or A/D move, Z / Space / K jump, X / Shift / J run, R restart.
On desktop: F2 reloads `content/` (edit `feel.ron` and press F2 to feel the change), and F5 saves
your inputs to `meadow-1.local.inputs`.

## For agents

```sh
cargo run --release -p ns-headless --bin ns -- serve     # JSON lines on stdin/stdout
```

```json
{"cmd":"load","level":"meadow-1"}
{"cmd":"step","buttons":"RX","frames":30,"ascii":true}
{"cmd":"save"}
{"cmd":"step","inputs":"10*RXJ 20*RX"}
{"cmd":"restore","slot":0}
{"cmd":"replay"}
```

Buttons: `L R U D J X` (`J` jump, `X` run), `-` for none. Run-length inputs: `30*RX 12*RXJ`.
The full command list is in `crates/headless/src/protocol.rs`. Other commands:
`ns view LEVEL [INPUTS]` prints the screen as text, and `ns record LEVEL INPUTS --out FILE` writes
a checked replay.

## Adding a level

1. Write `content/levels/<id>.ron`: `.` empty, `#` solid, `=` jump-through platform, `G` goal,
   `P` start; other characters map to entity kinds through `legend`.
2. `cargo run --release -p ns-bot -- <id> --write` finds a completing run and writes `<id>.replay`
   (or record one yourself).
3. `scripts/ci.sh` must pass. Every level ships with a replay that completes it. Any change to
   physics, scripts or the level that breaks a replay fails CI, as does any script error.

## Scripting

Entity scripts are Rhai, with handlers `on_spawn(e)`, `on_tick(e)` and `on_event(e, name)`. See
`crates/script-rhai/src/lib.rs` for everything `e` offers, and `content/scripts/` for examples.
Scripts only read state and issue commands. All state lives in the engine (`e.set_var`), which is
what makes save states, replays and future script languages (WASM via `wasmi`) possible.

## Checks

`scripts/ci.sh` runs formatting, lints, unit and feel tests, the browser builds, replay checks on
desktop, and the same replays under WebAssembly, which must produce identical state hashes.

## License

To be confirmed. Proposed: engine MIT OR Apache-2.0, base assets CC-BY-SA 4.0.
