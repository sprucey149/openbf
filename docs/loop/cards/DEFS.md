---
id: DEFS
title: Defenders after a capture: one defender per spot, several spots per post, a cap by team share that falls while the post is quiet (the maintainer's INBOX 2026-10-12 16:00, deviations approved) - on BF2 Kamino 5 of 17 crouched on one Cover hint for 30 s; on BF2 Hoth 4-8 guards stand 1 m apart on two spots for 100+ s and it is the whole of the map's doubled mate_1p5 (card HOTM)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's playtest of BF2 Kamino conquest (INBOX 2026-10-12 16:00, ask 1; his words and wishes are there in full - read
it first): after a capture too many of the team go to `defend` the post and all of them stand on ONE spot. Card HOTM
then found the same fault as the whole of BF2 Hoth's doubled `mate_1p5_pct` (5.1 -> 10.3 over 48 seeds).

## Evidence
- Code (main 5fbbbd4): `commander::defend_spot` (core/src/commander.rs:846) = `hint_spot` = `nearest_hint` from the
  unit, no reservation; `behave::spot_free` (ai/behave.rs:752) is asked only on the hub-hold path.
- Kamino: `engine/artifacts/playtest/last.json` (the maintainer's F9, seed 75): units 17 / 23 / 24 / 26 (+ 20) on one Cover hint
  16 m from cp5 at (3,-72), 30+ s, no enemy near; 9 of 17 living on a defend order.
- Hoth (card HOTM, `engine/artifacts/lead_tmp/hotm/`, README there): team 1's guards of cp1 stand on two spots,
  (32,-2,-6.5) and (20,-2,3.5), 1 m apart. Before MOUTH (knob binary null = 4ca302f) 7 of 16 rounds have the pile, a
  mean 2.8 guards there at once, 199 guard-s a round; with MOUTH's corner rule 16 of 16 rounds, a mean 4.2, 593
  guard-s (the rule cured a slide site at (-210,-15) that held 258 unit-s a round of that team; those bots now
  arrive). Snapshots: `hotm/snap/n00/bf2_hot1_s3.json` (tick 12000: 4 guards, `defend1`, `does guard`, 0.0 m/s) and
  `hotm/snap/M/bf2_hot1_s3.json` (tick 9000: 8 guards, 140 s). Over 48 seeds the gate's value splits: on guard
  1.83 -> 6.98, everything else 3.28 -> 3.33 (`hotm/out_split.md`).

## Notes
- Filed by card HOTM's session from the maintainer's INBOX note (the planner moves the note under Done).
- Instruments: `hotm/tools/guards.py` (guard-s by post, guards at once in a box), `where.py`, `split.py`.
- 2026-10-12 17:05 planner: the INBOX note is under `## Done`. The gates.toml approval for rules 2-4 cannot be written
  ahead (an approval names `metric@map` tokens): when the branch's `gates.py --groups fidelity` names what moves, set
  `status=blocked` with "fidelity", the tokens and both blocks' values in `resume:`; the planner then writes
  `[approvals.DEFS]` on the maintainer's yes of 2026-10-12 16:00 - for moves that are the rules' intent (fewer units standing
  at a post: `still_pct`, `idle20_pct`, `mate_*`, `contact20_pct` up or down) and not for a round that stops ending.
  Card TURR now depends on this card (its defence clause asks the same "enemy seen near this post" question as rule 3:
  make that one function).
- 2026-10-12 17:56 LANDED main 2e12961 (report `reports/20261012_1755_card_DEFS.md`). Acceptance: (1) four scene tests in
  `commander::tests`, 16 mutations each fail one (`lead_tmp/defs/mut/summary.txt`); (2) census `lead_tmp/defs/census_table.md`
  (Kamino guard-s a round 1219 -> 108, guards at a post 2.53 -> 1.12, longest stand 378 -> 61 s); (3) Hoth `mate_1p5` 10.64 ->
  3.83 over 16 seeds, guard part 7.21 -> 0.01; 256 / 256. Fidelity: nothing worse, so no approval was needed. Not built: a ring
  of standing points (a defender with no hint left takes PARR's region point), sending units back to a threatened post
  (TPOS / TEAM). TURR's clause: `commander::post_threatened(sim, team, cp, threat_radius(sim))`.
