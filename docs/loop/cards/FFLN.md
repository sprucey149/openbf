---
id: FFLN
title: Bots shoot their own teammates: 8 of 19 kills in 65 s of BF2 Kamino were team kills (droids killing droids around the human) - measure the PS2's friendly fire for AI, and give a bot a line-of-fire check
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/brain.rs,engine/crates/core/src/sim/combat.rs,engine/crates/core/src/sim/state.rs,engine/crates/game/examples/round_to_outcome.rs
declares: sim; sweep; arena
needs:
effort:
branch: ffln
resume: landed 142dd4b1 2026-10-02
created: 2026-10-02
alpha: yes
first: 2
---

## Why
The maintainer, 2026-10-02: "the bots have no sense of teammates nearby it seems so they'll just shoot their teammate that is standing in the way of me."

## Evidence (`lead_tmp/jitr/snapshot_20261002_kam1_f9.json`, events)
- 19 kills in 65.6 s, **8 by a teammate**: (killer, victim) #24->#21, #10->#13 (rocket launcher), #17->#29, #17->#25, #26->#22, #19->#18, #23->#19, #23->#31 - six of them CIS on CIS in the cluster within 15 m of the human at (-4, 60.3, 108.6).
- `Sim::friendly_fire_scale` is 1.0 by default (`sim/state.rs` 567): a teammate takes full damage. No test for a friend in the line of fire exists in `core/src/ai`.

## The bot programme (the maintainer, 2026-10-02) - five cards, read together
`JITR` (stop the tweaking now) -> `FFLN` (teammates in the line of fire) -> `BOTM` (bot-quality numbers, PS2 beside the port) -> `SAIR` (RE the PS2 foot soldier's own AI) -> `BOTD` (the design: what the best bot for this game is, then the build cards). `OBST` (walking into props) belongs to the same family.
The maintainer: "If the bots suck, the whole game is worthless so we NEED to get this right." He agrees to RE the PS2 soldier AI FIRST, then keep what is good and DELIBERATELY do better where the originals annoy (the originals' bots are not perfect; better difficulty levels, more awareness). The port's soldier brain today is ~9,200 lines (`core/src/ai`, `commander.rs`) of rules added one symptom at a time (hold, shuffle, strafe, dodge, slide, fan, stuck, stall, lost, recycle), most of them OURS/PROVISIONAL: no RE note exists for the soldier's second-to-second movement and combat.
Reference pulled 2026-10-02: the official mod-tools documentation as markdown, `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/` (`game.md`, `ZeroEditor_guide.md` - barriers, planning hubs/connections/weights/filters, hint nodes; `odf_guide.md`; `Battlefront2_scripting_system.md`), executable strings per platform `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/StringsFromExecutable/` (the PS2's too), a PC DEBUG build `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/Debugger_exes/`, 15 years of Gametoast forum posts `community_mods/gametoast_lua/doc/Archives/Gametoast.com/*.json`.

## Result (2026-10-02, branch `ffln` ba69e21; report `reports/20261002_2140_card_FFLN.md`)
- **PS2's rule** (analysis/re-blaster-damage.md section 6.3): friendly-fire scale 1.0 in all 326 battle states (both games); a bot's hit on a human teammate is dropped (B2 FUN_001dfb38 91318-91326, B1 FUN_001bc190 64355-64362). The port: `friendly_fire_scale` 1.0 (was), and `resolve_bolt` now drops a bot's bolt into a human of its team.
- **PS2's team-kill share**: BF2 KAM1, player pinned at the maintainer's spot, 515 s: 91 AI kills, 11 of a teammate (12.1 %), 4 self. The PS2's bots have no working line-of-fire rule; the port's is OURS.
- **Team kills / kills, 8 seeds x 32 maps**: 17.7 % -> 1.7 % (per map 8.4-33.6 % -> 0.6-3.1 %); friend hits / bolt hits 19.1 % -> 1.5 %. Tables: `lead_tmp/ffln/seeds_base_teamfire.md`, `seeds_v2_teamfire.md`.
- **The maintainer's scene** (`SWEEP_SLOT0=player SLOT0_AT=-4.0,60.3,108.6`, seed 60, 1969 ticks = 65.6 s): 11 team kills of 25 -> 0 of 21. (At 300 s: 61 of 139 -> 4 of 78.)
- **Enemy kills per minute of round**: +9.9 % over the 32 maps; 8-seed lows bf2_nab2 -12.0 %, bf2_kas2 -4.7 % were noise: over 32 seeds +1.1 % (se 3.9 %) and +2.8 % (se 3.2 %). Time waiting on a blocked line: 63,632 bot-seconds of 256 rounds (1.7 % of bot time).
- Not covered: a thrown grenade near friends (4 of the PS2's 15 were the killer's own; the port's remaining team kills are bolts from spread and friends walking into a burst).
