---
id: STTAP
title: Bots tap single-shot weapons as the PS2 does (BF2 FUN_002baf70 / FUN_002baa38, BF1 FUN_0023cc20 / FUN_0023c698) instead of holding the trigger and firing once per engagement
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/brain.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/ai/draw.rs,engine/crates/core/src/ai.rs,engine/crates/core/src/skill.rs,engine/crates/game/examples/round_to_outcome.rs,engine/CRATES.md
declares: sweep; arena
needs:
effort:
branch: sttap
resume: landed aecdc0e1 2026-10-02
created: 2026-09-30
alpha: yes
first: 5
---
## Why
STALL C4 (`analysis/re-round-stalls.md` §2 C4, [V][O]). A `TriggerSingle` weapon fires only on a fresh press (B1
`FUN_00289150`, `core::weapon::fire_predicate`). The port's bot sets FIRE on every tick it wants to shoot, so it fires once
and then holds the trigger. The only releases are incidental: a tick when `want_fire` fails. Snipers, rocketeers,
officers and engineers stand in standoffs that never resolve.

## Evidence
- GEO1 seed 42: units 11 (rocket launcher) and 15 (EMP launcher) face unit 30 (Bulldog) at 14–15 m. They are loaded and
  within the aim tolerance, and nobody fires for 50,000 ticks (`lead_tmp/stall` `TRACE_UNIT`).
- 32 maps, seed 42 (`lead_tmp/stall/shots/eps_s42.err`): rocket launchers fire 1.5–1.9 shots per unit-life, sniper
  rifles 2.2–2.3, torpedo launchers 1.6–3.0. The automatic median is 22.
- **The PS2 taps in both games** (all [V][O]; bytes in the doc):
  - **BF2, non-sniper:** `FUN_002bb1b0` → `FUN_002baf70`. It presses one frame, releases, and re-arms at
    clock + U(0.2·x, 2·x), x = max(0, 1 − lerp(0.1, 0.9)((skill−1)/19)).
  - **BF2, sniper** (TriggerSingle and ZoomMax ≥ 6, `FUN_002ace10`): `FUN_002baa38`, one press per U(2, 3) s.
  - **BF1:** `FUN_0023cd40` → `FUN_0023cc20`, a fixed cadence on the trigger byte: 1 → 2 → −1 → −2 → 1, one fresh press
    every 4 controller calls. Snipers also get `FUN_0023c698`'s U(2, 3) s cooldown.

## Notes
- Where the AI's trigger state lives, and whether it is hashed, is the builder's call: it is sim state.
- Say in the commit that the bleed and HOLD_RANGE are unchanged.

## Note from SWPN (2026-09-30)
Judge the fix over seeds 1-8: `scripts/sweep_seeds.py --out <dir> --target <your target> --baseline lead_tmp/loop_baselines/20260930_1600_SWPN` (main: 27/32 maps >= 7/8, 227/256 rounds; ~2 min).

## Note from the SWPN audit (2026-09-30, `reports/20260930_1745_audit_SWPN.md`)
- **Judge baseline.** The baseline above predates STFAL. Use `lead_tmp/audit-swpn/main_e09c854/` (main e09c854, seeds
  1-8 + 42): its seeds 1-8 give 28/32 maps ≥ 7/8 and 228/256 rounds. Or re-run main if it has moved. Give both runs
  the same seeds: the Δ columns are raw count differences.
- **C4 re-derived from the ELF: SUPPORTED.**
  - BF2 `FUN_002baf70`: press, release, re-arm at now + U(0.2x, 2x), x = max(0, 1 − lerp(0.1, 0.9,
    FUN_002a2bb8())). The max is a real `max.s` at 0x2bb018; the decomp's `(int)` is an artefact.
  - After an idle call, the first press comes on the 2nd want-fire call.
  - Snipers (TriggerSingle and ZoomMax ≥ 6) use `FUN_002baa38`: one press, then U(2.0, 3.0) s.
  - BF1 `FUN_0023cc20` gives one fresh press every 4 calls.
  - A second property key, 0x0ee66c41, sets the same flag in both ELFs. No disc ODF uses it (0 byte hits).
  - The port gives at most one salvo per hold. A press that lands in ShotDelay, reload or overheat fires nothing.

**Note (cleanse CLNC, 2026-09-30):** `core/src/ai.rs`'s fire discipline (`BURST_MIN`, `BURST_MAX`, `PAUSE_BASE_S`) is PROVISIONAL with no owner card: the PS2's per-weapon-class burst bounds (`vtbl+0x34/+0x38`) and pause constant (`weaponclass+0x21c`) are untraced ODF keys (`analysis/re-ai-difficulty.md` §4.1/§4.4 gives only the shapes). The comment now points here; trace them with this card's trigger RE or split them off.

## Note from GREN (2026-09-30, analysis/re-grenades.md §3.1, §7.2)
Every grenade weapon is `TriggerSingle` (the base class default is 1 and no thermal / crack / haywire class overrides
it; RAM 1 in both games): a bot's throw goes through this card's single-shot tap. Card GRAI (deps STTAP) adds the
choice of the grenade slot and the firing channel.

## Note from the maintainer's "rounds that do not end" pass (2026-10-01, main 4603e9e, `lead_tmp/endings/README.md`) — `first: 5`
Still unbuilt at 4603e9e: nothing under `core/src/ai*` reads `trigger_single`. It is the one kill-rate fix that reaches
every map (BF2 GEO1 is the slowest finishing map at 10.5 kills / min, rounds up to 19 min; HOLD's holders fight from a
position, so a rocket or sniper holder fires once and stands).

## Note from RCYC (2026-10-01) - a firefight that kills nobody holds a round open
BF2 KAM1 seed 4 (a RCYC variant run, `lead_tmp/rcyc/kam1_s4.txt`): the round stood 1 v 2 for 30,000 ticks with two rocketeers (units 6 and 23) crouched 16 m apart at hub 2, each `seen` by the other, neither dying. RCYC never recycles a bot fighting an enemy it perceives, so this is this card's: acceptance, added - no pair of bots in sight of each other inside 25 m for 3,000 ticks without a kill (8 seeds, KAM1).

## Built 2026-10-02 (branch `sttap` c917799, gated, PARKED for a land job — report `reports/20261002_0810_card_STTAP.md`)
`core/src/ai/brain.rs` `tap`: BF2 `FUN_002baf70` / `FUN_002baa38`, BF1 `FUN_0023cc20` / `FUN_0023c698`, per `Sim::ground_rules`.
The trigger state is in `AiState`, hashed only once a single-shot weapon has moved it (`--sim-hash 100` unchanged).
Acceptance, as run (9 seeds 1-8 + 42 x 32 maps, against the same commit with the tap branch off; `lead_tmp/sttap/{base,tap}`):
- **Unit tests: met.** `a_bf2_bot_taps_a_single_shot_weapon`, `a_bf2_sniper_fires_once_per_two_to_three_seconds`,
  `a_bf1_bot_presses_a_single_shot_weapon_every_fourth_tick`; the automatic burst test is unchanged. With the branch off the
  three fail (skill-1 rocket pace 3 shots in 10 s, sniper 1 shot in 30 s).
- **Shots census: NOT met as written.** Median single-shot class, pulls per unit-life: 3.35 -> 4.54 over 9 seeds (seed 42:
  3.48 -> 4.49); the card asked >= 5. Rocket launchers 2.9-3.6, sniper rifles 3.5-4.5, automatics 27.7 (`lead_tmp/sttap/shots.py`).
- **GEO1 seed 42: met.** BF2 GEO1 seed 42's longest standoff 4,740 -> 420 ticks (14 s).
- **No standoff of 3,000 ticks: met on KAM1** (longest 690 over 9 seeds; 1,770 before). All maps: 35 pairs -> 1
  (BF2 YAV1 seed 7, units 9 and 28, 3,930 ticks from tick 8,430 — not looked into).
- **Finish counts: one map down one, one up one.** 287 -> 287 of 288; BF2 GEO1 8/9 -> 9/9, BF1 GEO1 9/9 -> 8/9 (seed 7 stands
  9 v 2 at 60,000 ticks with no standoff in it — card STEND's case). Kills / min 24.0 -> 26.3, median round 15,705 -> 14,491 ticks.

## Landed 2026-10-02 (main aecdc0e, report `reports/20261002_0935_land_STTAP.md`)
Re-run before the commit on the rebased binary (`lead_tmp/sttap/land/repro_tap`, `repro_base`): standoffs of 3,000 ticks
35 -> 1; seeds 1-8 finish 255 -> 256 of 256, no map down; pulls per life 3.35 -> 4.54 (the card's 5 not reached).
Two seed labels above are wrong: BF1 GEO1's round that does not end is seed 42 (the gate's sweep row, now 31/32), and BF2
GEO1 seed 42's longest standoff is 7,140 -> 570 (4,740 -> 420 is seed 4).
