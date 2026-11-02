# openbf - notes for every agent session


## Coordination and automated commits

Public issues describe a player-facing feature, reproducible bug or concrete
investigation: explain what a player sees, the intended result, bounded scope
and completion evidence. Use readable titles; keep card IDs and implementation
details in optional references. Consolidate related implementation slices under
the same feature; keep routine refactor checks, audits and maintenance in their
existing records instead of creating a public issue for every internal card.
State when code already exists so nobody duplicates implementation work.

Agree on a task's scope and ownership with the maintainer before implementation.
Honor assigned issues and tasks claimed by another contributor. Keep each feature
on its own branch and PR, with tests and before/after evidence; agree on wider
refactors or shared-system changes first.

Maintainer-run agents may commit and push fully reviewed, validated work on the
maintainer's behalf. One integration coordinator publishes to upstream main;
other workers and contributors use their own branches and PRs. A partial check or
skipped original-game requirement does not make a task complete.

Read `PLAN.md` next (short; its moved sections are pointed to inside, and `grep analysis/INDEX.md` before starting any RE), then `engine/artifacts/loop/STATUS.md`. The autoloop (cards in `engine/artifacts/loop/cards/`,
`scripts/loopctl.py`) is how work lands; the maintainer steers it through `engine/artifacts/loop/INBOX.md`.

In the public repository the cards are in `docs/loop/cards/`; `STATUS.md`, `INBOX.md`, the JOURNAL and `PERF.md` are not published.

## Playtest black box (dev tool, card BLKB)
The maintainer now plays the port himself and reports bugs with a screenshot and a line. In dev mode (ON by default while the game is unfinished; `OPENBF_DEV=0` turns it off) the windowed game keeps
the last 60 s of game time in memory and writes a snapshot of every unit's state (bots' goals, route legs, steer picks,
what shuts their way and which map body it is, positions, trails; the maintainer's position, camera, input and reticle target;
posts, vehicles, events) on **F9**, on quit, on a panic and on a stuck alarm (20 s of pushing the stick without getting
2 m away). Files: `engine/artifacts/playtest/last.json` (newest) and `prev1..prev5.json`. There is no `last.png`: the
windowed renderer cannot read its frame back, so the picture is the maintainer's own screenshot.

- **When the maintainer reports something he saw ("a bot walks into a wall", "this guy is stuck"): read the snapshot first**, with
  `cargo run -p game --example blackbox_read -- <file>` from `engine/` (under the memory cap:
  `systemd-run --user --scope --quiet --collect -p MemoryMax=8G -p MemorySwapMax=0 -p OOMPolicy=kill -- timeout 120 ...`).
  The digest prints the header, the maintainer's last 10 s, the `focus` unit (what he was looking at when he pressed F9: the
  reticle's pick, else the aim assist's, else the unit nearest the view axis) with its brain and trail, every bot whose
  steering was blocked or made no progress for over 3 s (who, where, which body, its goal) and every unit whose trail
  went nowhere (over 3 m walked for under a quarter of it net, with its reversals), every unit within 15 m of him, the posts and the last 30 s of events. Views: `--unit <id>` (that unit in every frame), `--near x,y,z,r`,
  `--events`, `--replay-spot [id]` (the place as a line for a headless follow-up). Name the cause from the unit's brain
  fields and trail, and write the finding into a card (numbers, spot, unit fields, body name) or an INBOX note for the
  loop. Say which snapshot (the build commit in its header).
  A unit's STOP and body are asked along the direction its steering really walked that tick (`walk` / `walk_at`: the
  hold goal, the vehicle it boards, a lost bot's return point, else the leg), and a bot that did not walk carries
  none; a holder's row has its goal, spot, seconds left and strafe side. A schema 1 file (before card BLKF) asked
  toward the leg's point instead: its stops are wrong for holders, boarders and lost bots, and the digest says so.
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

## Keyboard + mouse (the maintainer, 2026-10-25; contributed in the public repo's pull request #1)
Keyboard and mouse play is a deliberate deviation from the PS2's controller-only input. Controller play, bots and
controller-only replays must stay bit-identical. The rules:
- The mouse reaches the sim as `PadState::mx`/`my`: angles in `LOOK_UNIT` (1/65536 rad), applied once, on the
  first tick of a frame. They are not rates: no `max_turn_speed` cap and no stance turn factor (except Tumble,
  GetUp and Dead), and zoom divides them by its magnification. The pitch limits, the prone clamp and the seat aimer
  limits still hold. Both fields default to 0 and are left out of logs when 0, so old logs and snapshots load.
- `buttons::MOUSE_AIM` marks a pad aimed by the mouse; aim assist never acts on it. Moving the right stick clears it.
- Hovers and flyers: the mouse yaw becomes the turn stick, full deflection at
  `MOUSE_STEER` rad per tick. Flyers use their own flight model and also turn mouse pitch into the pitch stick.
  Walkers: the mouse aims directly, held inside the walker's yaw limits beside its legs
  (`walker_aim`); the drawn aim keeps its heading while the legs turn the hull. Seats and building turrets: the look
  turns by the angle.
- Between ticks the view draws the mouse look not yet consumed (`Sim::look_preview`, `render_blend::blend_in_looking`),
  so the drawn view is where the next tick's aim lands. A controller on foot gets the same (2026-10-29): the view
  draws the frame's share of the turn the next tick's look step takes under the held stick, the aim assist included,
  and a FIRE press carries that share (`PadState::fire_lag`) so its tick fires part way into the turn, where it was drawn.
- Bindings are one table (`game/src/kbm.rs`, `Bindings::pc_default`). Pointer lock follows the battle: it is taken on a
  click, and freed on pause, in the shell and spawn screens, on focus loss and on exit.

## Shared tools (`scripts/tools/`)
Small helpers the notes and cards cite, runnable from any directory (paths resolve from the repo root; the RE tools
read your own `extracted/bf1/SWBFRONT.ELF` / `extracted/bf2/SWBFRNT2.ELF`): `mix.py` (render a black-box snapshot's
sounds to a WAV), `elfrd.py` (ELF words by virtual address), `refs.py` (every jal / j / data reference to an address),
`eedis.py` / `eedis2.py` / `vu1dis.py` (R5900 and VU disassembly), `pure_move_check.py` (prove a file split only moved
code), `win_run.sh` + `xkey.py` (the windowed game on its own Xvfb display, driven by keys). Usage is in each file's
header and in `scripts/tools/README.md`. `scripts/re_check.py` (fn / callers / word) is the main RE checker.

## Bots (the maintainer's top concern, 2026-10-02)
"If the bots suck, the whole game is worthless." The plan is cards JITR, FFLN, BOTM, SAIR (RE the PS2 foot soldier's own
AI), BOTD (the design note `analysis/bot-design.md`; the maintainer’s original read hold was released as recorded in `engine/artifacts/loop/cards/FGHT.md`) and OBST. The PS2 behaviour is
the baseline; where the originals' bots annoy, the port deliberately does better, as a named deviation.
Reference: the official mod-tools docs are in `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/`
(`game.md` and `ZeroEditor_guide.md`: barriers, planning hubs / connections / weights, hint nodes), with each
platform's executable strings (`StringsFromExecutable/`), a PC debug build and the Gametoast forum archive
(`community_mods/gametoast_lua/doc/Archives/`).

## When the maintainer asks "what happened overnight / what did the loop do"
Read `engine/artifacts/loop/STATUS.md`, the git log and the JOURNAL - and ALWAYS include the performance line from the top
of `engine/artifacts/loop/PERF.md` (card PERF: the largest bot count per map that holds 60 fps on the maintainer's machine and on
the mid-class-2020-laptop proxy, sim ms per tick, and what changed since the last time). If PERF.md does not exist yet,
say so.

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

A new value labelled PROVISIONAL, OURS or INTERIM must say what measurement replaces it (`replace:`, `measure:` or `trace:`
in its comment); `scripts/check_provisional.py` checks added lines, and `engine/ci.sh` runs it.
