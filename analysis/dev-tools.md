# Developer tools and working notes

Moved from `AGENTS.md` on 2026-11-02 so that file can stay short. The script helpers are listed in `scripts/README.md` and `scripts/tools/README.md`.

## Playtest black box (dev tool, card BLKB)
The maintainer now plays the port himself and reports bugs with a screenshot and a line. In dev mode (ON by default while the game is unfinished; `OPENBF_DEV=0` turns it off) the windowed game keeps
the last 60 s of game time in memory and writes a snapshot of every unit's state (bots' goals, route legs, steer picks,
what shuts their way and which map body it is, positions, trails; the maintainer's position, camera, input and reticle target;
posts, vehicles, events) on **F9**, on quit, on a panic and on a stuck alarm (20 s of pushing the stick without getting
2 m away). Files: `engine/artifacts/playtest/last.json` (newest) and `prev1..prev5.json`. There is no `last.png`: the
windowed renderer cannot read its frame back, so the picture is the maintainer's own screenshot.
The F9 save notice appears over the spawn and pause menus after the write finishes.

- **When the maintainer reports something he saw ("a bot walks into a wall", "this guy is stuck"), read the snapshot first.**
  Run from the worktree root with its own Cargo target.

  ```bash
  export CARGO_TARGET_DIR="$PWD/engine/target"
  systemd-run --user --scope --quiet --collect -p MemoryMax=8G -p MemorySwapMax=0 -p OOMPolicy=kill -- \
    nice -n 10 cargo run --manifest-path engine/Cargo.toml -p game --example blackbox_read -- path/to/snapshot.json
  ```

  The digest prints the header, the maintainer's last 10 s, the `focus` unit (what he was looking at when he pressed F9: the
  reticle's pick, else the aim assist's, else the unit nearest the view axis) with its brain and trail, every bot whose
  steering was blocked or made no progress for over 3 s (who, where, which body, its goal) and every unit whose trail
  went nowhere (over 3 m walked for under a quarter of it net, with its reversals), every unit within 15 m of him, the posts and the last 30 s of events. Views: `--unit <id>` (that unit in every frame), `--near x,y,z,r`,
  `--events`, `--replay-spot [id]` (the place as a line for a headless follow-up). Name the cause from the unit's brain
  fields and trail, and write the finding into a card (numbers, spot, unit fields, body name) or an INBOX note for the
  loop. Say which snapshot (the build commit in its header).
  STOP and body are probed from the sampled unit pose along `walk_dir`, the direction requested before local
  avoidance. The selected heading and resulting pad can differ. `walk` and `walk_at` identify the request and its
  point, such as the hold goal, the vehicle being boarded, a lost bot's return point, hazard escape, an item pickup,
  the final health droid approach or the route leg. A bot without a recorded request has no stop probe. A holder's row
  has its goal, spot, seconds left and strafe side. Schema 1 (before card BLKF) used the route leg for every request.
  The digest flags unreliable schema 1 stops for holders, boarders and lost bots.
  Since schema 4 (card OBST2) a unit's line also says what it `does` (the behaviour that ran: travel, fight, guard,
  recover with its failed-window count, ...) and, for a bot going round an obstacle, `ROUND side +1/-1, nearest N m,
  allowance X of 10 s` (the side it committed to, the nearest its walk has been to its aim point since, how much of
  the progress window's allowance is spent; `at a RIM` = none). An older file has neither, and the digest says so.
  Since schema 9 (card WHY) a file keeps each bot's last reasons: why it entered each behaviour and why each walk
  failed (`wall`, `rim`, `body`, `round_spent`, `ledge`, `no_gain`, `off_graph`, ... with the leg's arc); `--unit <id>`
  prints them and a troubled bot's line carries its last four.
- **When the maintainer reports a noise ("a terrible sound", "static", "it crackled when X fired"): read the SOUNDS rows first**
  (card SNDB; a schema 3 file). The digest ends with every one-shot the mixer was handed in the last 10 s (`--sounds`:
  all 60 s) - seconds before F9, the property's name, gain (the level by distance included), pitch, metres from his
  ear, the take's length, voices sounding, the take's file under `engine/assets_baked/` - and an `OVERLOAD` line for
  every tick of the 60 s whose starts sum over 1.0 ("N starts, sum G: names", the nearest, and the largest sum over
  ONE take: equal takes add up sample for sample, different ones do not). Name the sound from those rows, `ffplay`
  its take, and render what he heard with `python3 scripts/tools/mix.py <snapshot> engine/assets_baked
  --wav out.wav` (peak against full scale; no pan, engine loops or music in it). The rows need a sound device: a
  headless round and `scripts/tools/win_run.sh`'s default (no device) write none.
- **When the maintainer reports damage or firing that feels wrong** ("it only took half his health", "I fired two at once"): read
  the event rows (`--events`). Since schema 8 they carry each of his shots (`#n fired <weapon>`) and every hit he deals
  or takes: the damage, the victim's health after it, and direct (with the round's flight time) or blast.
  Schema 17 also records `fall_damage`: `a` is the human victim, and `v` holds damage and remaining health.
  The reader prints these as landing damage; they carry no attacker or weapon credit.
- **When a controller walks slowly, will not sprint or roll** (#142): the human rows carry the left stick as gilrs
  gave it (`stick_raw`, schema 16) next to the scaled `lx`/`ly`, and `stick_full`, the travel the game took as a full
  push on each axis that tick (`input::StickRange`: the pad's own learned peak per axis for a soldier on foot in a
  ground mode, `STICK_FULL` in a seat and in space). A raw peak under 0.9 on a full push is an axis that tops out
  short; under `STICK_FULL_MIN` that axis is out of range. Without a snapshot the reporter runs
  `cargo run --release -p input --example pads -- --live` and pastes the peaks line.
- **A timed battle with no renderer:** `game --map assets_baked/tat2 --mode eli --headless --seconds 60` steps the mission
  `missions/<map>_<mode>.ron` for that long, prints points, alive counts and classes every 10 s, and writes
  `artifacts/playtest/headless/<map>_<mode>_<seed>.json`.
- **Use it yourself** when chasing a bot or sim bug: a headless run writes the same file — `BLACKBOX_AT=<tick>` or
  `BLACKBOX_ON=lost|recycle|stuck[:<ticks>]` on `round_to_outcome` writes
  `engine/artifacts/playtest/headless/<map>_s<seed>.json` (`BLACKBOX_DIR` moves it; never the maintainer's own `last.json`).
- A snapshot belongs to the BUILD commit in its header: the commit stamped into the binary when it was compiled
  (`+dirty` = compiled from a tree with uncommitted source edits). The checkout's HEAD when the battle started is
  printed beside it, and the digest prints a WARNING when the two differ (a stale binary run from a newer checkout
  does not hold the newer code). If main has moved past the build commit, say so before trusting a "still broken"
  reading. A dead unit's trail ends at its death (`trails[].tick`), not at the header's `trail_tick`.
- The file is plain JSON with short keys; its `legend` block says what each key means, `names` resolves the class,
  post, weapon, vehicle and body indices. `python3 -c "import json; ..."` works when the digest is not enough.
- **Match log (card DPRX)**: beside the 60 s ring, dev mode also writes one compact file per battle with the
  player's WHOLE round - every shot, every hit dealt or taken, kills, deaths, spawns (class), position / stance /
  stick 6 times a second, each enemy he comes to see (for time to first hit), the result, the tickets and every
  cheat that was on at any tick: `engine/artifacts/playtest/matches/<YYYYmmdd-HHMMSS>_<map>.jsonl` (the black
  box's folder; `OPENBF_MATCHLOG=0` turns only it off), written at the outcome or on quit. `python3
  scripts/human_profile.py show <log>` prints the player's profile (K/D, kills/min, accuracy, sight-to-hit, life,
  movement) and refuses a log with cheats on; `calibrate <log>...` fits the sweep's human proxy to it
  (`core::ai::proxy`, card DPRX). The snapshot's `session` block (schema 10) is the same tally in brief.
- It is dev scaffolding and goes away when the game is playable for real: one module, `engine/crates/game/src/blackbox.rs`
  (its row type, `game/src/unit_probe.rs`, stays — the sweep's `UNIT_SAMPLES` shares it). On with dev mode, which is on by
  default (`OPENBF_DEV=0` turns both off); `OPENBF_BLACKBOX=0` turns only the black box off.

## LAN desync reports (issue #168)
When a LAN game reports a desync, both machines write
`<black box folder>/desync/desync-<map>-<seed>-r<round>-s<slot>-t<tick>-<host|guest>.json`
(`game::netplay::desync`; `OPENBF_BLACKBOX_DIR` moves the folder). Read them with
`game --desync-report HOST.json [GUEST.json] [--assets DIR] [--missions DIR] [--no-resim]`: it prints both
hashes, builds and OS/CPU, the static digest by group, the first tick and state group where the two sides'
per-tick rows differ, and re-simulates the round from the report's setup and bundles to say which side this
build agrees with. `game --net-soak --perturb-at T --report-dir DIR` makes a pair on purpose.

## Profiling tools on this machine (the maintainer, 2026-10-21)
`perf` 7.2 and `valgrind` 3.27 (callgrind, massif, dhat) are installed; `samply`, `heaptrack` and `flamegraph` are not.
Flat profile of a release binary: `perf record -F 499 -o <file> -- <bin>`, then `perf report --no-children --sort symbol
--stdio` (own processes only; `perf record --call-graph dwarf` FAILS here). For callers: `valgrind --tool=callgrind`
(~50x slower) and `callgrind_annotate --tree=caller --inclusive=yes`. Copy the binary first, run under the memory cap +
`timeout`; wall clock is void while the loop loads the machine - quote instruction counts and shares. First finding: card
HULLC (vehicle hulls re-welded every tick, 37.6 % of the sim on BF2 KAS2).

To price a change to the frame, compare inside one run: `OPENBF_FRAMEAB=base,order:mesh,no:terrain OPENBF_GPU_TIMING=1
OPENBF_BENCH_DETAIL=out.json game --map <dir> --bench ...` draws each world step once per state and prints each state's
median difference from the first (`render::frame_ab`). Separate runs cannot do this while anything else uses the machine:
another job moved the Intel GPU's timestamps by up to 4x between runs. The same bench lists the slowest ticks with their worst
bot and ray counts.

## Bots (the maintainer's top concern, 2026-10-02)
"If the bots suck, the whole game is worthless." The plan is cards JITR, FFLN, BOTM, SAIR (RE the PS2 foot soldier's own
AI), BOTD (the design note `analysis/bot-design.md`) and OBST. The original games provide evidence for bot behaviour.
openbf names deliberate differences.


Reference: the official mod-tools docs are in `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/`
(`game.md` and `ZeroEditor_guide.md`: barriers, planning hubs / connections / weights, hint nodes), with each
platform's executable strings (`StringsFromExecutable/`), a PC debug build and the Gametoast forum archive
(`community_mods/gametoast_lua/doc/Archives/`).

## PCSX2 automation & original-game verification (`emucap`)
A locally configured `emucap` adapter can drive PCSX2 over PINE.
Its commands can launch the emulator, load a user-supplied save state,
send controller input, capture frames and read emulated memory.
Use the adapter's local setup instructions. The port black box
records port-side play and simulation evidence.


## Start in a vehicle from the CLI

`--vehicle <class-or-substring>` enables walk mode and puts the player in
seat 0 of a matching vehicle. It works without `--ai` or `--mission`; that
standalone form loads the map's vehicle pads and starts no AI wave. An explicit
mission or AI count retains its usual roster and battle setup.

Headless diagnostic from the repository root, using the local baked map:

```powershell
cargo run --manifest-path engine/Cargo.toml -p game -- --map engine/assets_baked/bf2_geo1 --vehicle hailfire --headless --seconds 1
```

For interactive inspection, omit `--headless --seconds 1`. Use the full class
name, such as `cis_tread_hailfire`, to select a specific variant. Matching is
case-insensitive: an exact class wins across existing vehicles and spawn pads;
otherwise the selector prefers the player's team, then neutral/owner-compatible
choices. An unavailable exact class does not silently select a different variant.

The shortcut can use an existing seat or create a pad-defined vehicle. Full or
blocked pads are skipped, and an unsuccessful request leaves their team and
spawn records unchanged. If no candidate can be used, the error lists the map's
classes. This is a developer startup shortcut, not a change to ordinary in-game
boarding rules. Automated checks use headless simulation and never open a window.

On Windows/MSVC the game executable reserves an 8 MiB main-thread stack for
unoptimized winit startup; this does not increase simulation stack use or change
fixed-tick rules. Regression tests live in
`core::sim::vehicles::tests::immediate_pad_spawns_preserve_capacity_and_failed_request_state`
and `game::main::battle::vehicle_start::tests`.
