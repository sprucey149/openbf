---
id: BRSTT
title: BRSTU's heat bursts are refereed only on the gross cases (AUDIT BRSTU): 18 of 33 mutations survive every test, 11 on paths a battle reaches - a seat gun unlimited by one rule only (BF1 gun turret 27,509 shots), a two-weapon seat's heat, the 0.99 caps that keep BF1's speeder cannon, fighter-tank / spider lasers and AT-TE turret cannon firing, the weapon-kind reset, multi-shot salvos, on-foot heat waiting, a counted single-shot seat gun's tap
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT BRSTU (2026-10-21, `engine/artifacts/lead_tmp/audit-brstu/EVIDENCE.md`, `audits.md`): the heat-burst rule is the PS2's and the
code is right on every case checked, but its tests pin only the gross mutations. Of 33 audit mutations (`lead_tmp/audit-brstu/tools/mk_mut.py --list`,
`chain_mut.sh`; core lib 799 tests) 18 survive, 11 of them on paths a battle reaches (counts from the landing's 8 x 32 sweep):
- B03 / B02: a seat gun unlimited by ONE rule only - the class's RoundsPerClip < 1 (BF1 gun turret, 27,509 shots; slot WeaponAmmo 1) or the slot's
  WeaponAmmo < 1 (4,355: combat speeder missile, BF1 geo turret RPC 300) - every test seat sets both.
- B04: a two-weapon seat reading the second weapon's heat (2,685 shots: AT-ST head, speeders, fighter tanks, hailfire, AT-TE laser).
- B11 / B12 / B20: the 0.99 caps - without them BF1's speeder cannon, fighter-tank and spider lasers and AT-TE turret cannon never fire or end
  every burst at once (no test has a gun whose every n costs over 1, or a salvo heat >= 1).
- B34: the weapon-kind reset (22,839 kind transitions with a heat burst running: pistol -> rifle, dismount).
- B23 / B13 / B14: ShotsPerSalvo > 1 (BF1 speeder cannon1, AT-TE cannons).
- B41: on foot, FIRE without a running burst while a slow-cooling unlimited gun waits on the start test.
- B44: a counted TriggerSingle seat gun's tap gated on the burst (BF1 hailfire missile, BF2 Hoth dish).
## What to build
Tests only (no behaviour change), the scenes the audit's judges proposed (`lead_tmp/audit-brstu/wf/results.json`, J1 / J2):
- `ai/man/tests.rs`: a seat gun unlimited by the class only (`seat_gun_scene(heat_gun(), false, ..)`) and by the slot only (RoundsPerClip 1000,
  `infinite = true`); a second `GunWeapon` (no heat) in the seat; a gun with heat_per_shot 0.45 (every n over 1: it still fires); a gun with
  heat_per_shot 1.0+ placed off the turret's rest (a shot per cooling cycle); heat_gun with shots_per_salvo 2 / 3 (bursts stop one SALVO short,
  start test in whole salvos); a counted TriggerSingle seat gun (rounds_per_clip 50, barrage [1,1], delay 3 s) still tapped at STTAP's cadence.
- `ai/gunner_foot.rs` tests: a bot switching from an unlimited weapon to a counted rifle keeps the rifle's bursts and pause; the unlimited
  hand weapon with heat_recover_rate 0.05 waits with the trigger up.
## Acceptance
- Re-run the audit's list against the branch (`mk_mut.py` with W pointed at the branch worktree; `chain_mut.sh`): each of B02 B03 B04 B11 B12
  B13 B14 B20 B23 B34 B41 B44 fails at least one ci test, R0 passes; paste the table.
- Behaviour unchanged: sim / walk / aim hashes and one 8 x 32 sweep byte-identical to main (`lead_tmp/audit-brstu/tools/nullcmp.sh` style).
