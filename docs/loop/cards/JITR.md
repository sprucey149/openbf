---
id: JITR
title: Bots holding a position stutter: a droid ran 46 m in 10 s with 26 reversals inside a 1.5 m box (the maintainer's F9 snapshot, BF2 KAM1) - one owner for a holding bot's move goal, a commit time, a walk for a short step, and a slide side that does not flip
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/brain.rs,engine/crates/core/src/ai/steer.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/ai/draw.rs,engine/crates/core/src/ai.rs,engine/crates/core/src/sim/state.rs,engine/crates/game/examples/round_to_outcome.rs,engine/CRATES.md
declares: sim; sweep; arena
needs:
effort:
branch: jitr
resume:
created: 2026-10-02
alpha: yes
first: 1
---

## Why
The maintainer, 2026-10-02 (BF2 Kamino, invincible, standing still on a platform): "the droids were all standing next to each other and just tweaking back and forth ... stuttering back and forth and standing next to each other ... all standing there shooting me while tweaking. It seems very unnatural."

## Evidence - the maintainer's black box snapshot (kept: `lead_tmp/jitr/snapshot_20261002_kam1_f9.json`, commit 20990b1, seed 60, tick 1969; `blackbox_read <file> --unit 30`)
- **#30 cis officer**, 9 m from the human, `hold 2` for the whole 10 s trail: path 46.1 m, net 3.1 m, **26 reversals** in 40 trail points, speed 5.2-7.0 m/s in every 0.5 s sample, inside x -10.6..-8.4, z 111.2..113.5. `sidestep` reads 31 in every sample for 10 s (a counter that should run down); hp 252 -> 203 -> 103 while the human did not fire (teammates' shots - card FFLN). Three rules write a holding bot's goal and nothing arbitrates: the strafe on every new burst (`brain.rs` ~420, `h.goal = pos + side * HOLD_STRAFE`, alternating side), the periodic shuffle (~154), the on-hit dodge (~213, a side-step on the route or a roll). Each step is run at full speed.
- **#18 sniper / #19 rifleman** at (-4..-11, 114..121): `steer slide` with the slide side flipping -1 / +1 every 0.5 s against `kam_prop_crate` 29 / 39 (`stop wall@0.0..0.6`), 11 and 8 reversals, hold 2, wp 19-20 m away and never nearer.
- Trail table of that snapshot (path / net / reversals in 10 s): #30 46.1 / 3.1 / 26, #18 47.8 / 14.1 / 11, #19 43.1 / 15.4 / 8, #17 16.9 / 1.3 / 4.
- Not isolated yet: WHICH of the three rules drives #30 (print the goal's writer per tick first; the fix follows what it shows).

## The bot programme (the maintainer, 2026-10-02) - five cards, read together
`JITR` (stop the tweaking now) -> `FFLN` (teammates in the line of fire) -> `BOTM` (bot-quality numbers, PS2 beside the port) -> `SAIR` (RE the PS2 foot soldier's own AI) -> `BOTD` (the design: what the best bot for this game is, then the build cards). `OBST` (walking into props) belongs to the same family.
The maintainer: "If the bots suck, the whole game is worthless so we NEED to get this right." He agrees to RE the PS2 soldier AI FIRST, then keep what is good and DELIBERATELY do better where the originals annoy (the originals' bots are not perfect; better difficulty levels, more awareness). The port's soldier brain today is ~9,200 lines (`core/src/ai`, `commander.rs`) of rules added one symptom at a time (hold, shuffle, strafe, dodge, slide, fan, stuck, stall, lost, recycle), most of them OURS/PROVISIONAL: no RE note exists for the soldier's second-to-second movement and combat.
Reference pulled 2026-10-02: the official mod-tools documentation as markdown, `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/` (`game.md`, `ZeroEditor_guide.md` - barriers, planning hubs/connections/weights/filters, hint nodes; `odf_guide.md`; `Battlefront2_scripting_system.md`), executable strings per platform `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/StringsFromExecutable/` (the PS2's too), a PC DEBUG build `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/Debugger_exes/`, 15 years of Gametoast forum posts `community_mods/gametoast_lua/doc/Archives/Gametoast.com/*.json`.

## Built (2026-10-02, lane 1, branch `jitr`) - what the trace showed and what changed
Step 1, the per-tick writer trace (the maintainer's scene re-run headless: `SWEEP_SLOT0=player SLOT0_AT=-4.0,60.3,108.6`, KAM1 seed 60, a temporary `eprintln` in `think`): of 7,090 holder-ticks, 2,469 were walking; in 1,513 of those the route's side-step was live (a holder's on-hit dodge wrote `r.sidestep = ±31` and the countdown never ran while holding, so the goal was walked 5 m off to one side - #30's pinball); 918 were slides against a wall between the bot and a goal 1-2 m behind it (slide until the way opens, walk back, slide the other way - #13, the #18/#19 kind); strafe-on-burst and shuffle took goals on their own clocks; every holder at a hub took the same nearest hint.
Fix (all OURS / PROVISIONAL until SAIR, constants in `core::ai`): `HoldState::step_to` is the one writer (shuffle, strafe, the holder's on-hit 2.5 m step), `HOLD_COMMIT` 2 s; the side-step counts down while holding and never offsets a hold goal; `HOLD_WALK_DIST` 3 m walked at stick 0.5; `HOLD_BLOCKED` 1 s off the straight line -> stand here (`settle`); `HOLD_SPACING` 2 m between holders' points and from a player (`spot_free`; a taken hint falls back to a drawn point); the slide side kept `SLIDE_KEEP` 1.5 s on the way to a held position only (for leg walkers it was a wash: KAS2 stuck-spot pacers 7 -> 28 with it, 6 without - left to OBST).
Measured (instrument `lead_tmp/jitr/tools/jitter.py`, reproduces the card's #30 46.1 / 3.1 / 26 exactly):
- The maintainer's scene, 6 snapshots (t 1200-3600): holders over the limit (net < 4 m and > 6 reversals or > 15 m) 4 -> **0**; holder reversals per 10 s 4.06 -> 0.60; holder path 25.6 -> 19.8 m; units with > 2 slide flips 19 -> 4 (no holder among them).
- 3 maps (KAM1, KAS2, HOT1) x 8 seeds x 2 ticks: holder jitterers 8 -> 1, holder reversals 1.04 -> 0.74; all units 70 -> 46 jitterers (the rest are leg walkers at known stuck spots - OBST).
- 8 seeds x 32 maps (`lead_tmp/jitr/seeds_{main,v2}/seeds.md`): rounds finished 256/256 -> 256/256, maps >= 10k stuck 0 -> 0, recycled 378 -> 372, kills/min 30.27 -> 31.18 (per map -13 % .. +22 %, seed noise), slide share 11.7 -> 10.3 %, walking-in-place 2.42 -> 2.39 %.
- Arena: flat identical numbers (monotonic yes); tat2 kills up on every tier (normal 39.8 -> 45.6), monotonic `no` on main and on the branch alike (not this card's).
- Unit tests `a_holders_goal_is_kept_for_its_commit_time`, `a_short_step_on_a_position_is_walked` (both mutation-checked: fail with the rule removed).
