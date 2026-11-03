---
id: FGHT
title: Bots fight at range and do not close: an enemy within 20 m 3 % of the time on BF2 Kamino against the PS2's 17-25 % (the kill rate already matches) - RE the PS2's fight order (how far a bot moves at its target, how long, where it resumes) and build it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card CONT (2026-10-02) built the PS2's Conquest goal: a bot keeps one target post (a change per 358 on-foot unit-seconds
on BF2 Kamino, the PS2 323) and Kamino's kills / min is the PS2's (17.3). The bots still do not CLOSE: an enemy within
20 m 3.0 % of alive unit-seconds on Kamino against the PS2's 17-25 %, 23 % on Polis Massa against 31.9 %. The port
kills at the PS2's rate from further away. The maintainer plays Kamino; a fight at 70 m with nobody closing is what he sees.

## Evidence
- `lead_tmp/cont/build/sw_base` / `sw_new2` (8 seeds x 32 maps, main 8069b44 against CONT): `seeds.md`'s shares table
  has both measures per map; `lead_tmp/cont/build/sum.py <dir>... --map <key>`.
- `analysis/re-conquest-goal.md` section 5: the PS2's bots spend 20-28 % of alive time in the fight order (type 2,
  `FUN_002c4c80`, 0x250 bytes), 16-19 % standing at hint nodes, 5-12 % entering / manning turrets; 8-9 % of lives
  reach their target's capture region, median closest approach 131 m (Kamino) / 90 m (Polis Massa). What the fight
  order does, for how long and where the bot resumes is [H] - not opened.
- The port's brain: an enemy inside `HOLD_RANGE` makes the bot take a position; outside it the bot walks on and fires.

## Planner 2026-10-02 17:45 - re-phased to build, behind SAIR and BOTD
- Its RE (What to do 1-2) is folded into SAIR (section "Folded in from FGHT"): SAIR's item 3 is the same question, and the maintainer's order puts the PS2 soldier RE (SAIR) and the design (BOTD) before any further brain build. Step 3 (build in `brain.rs`) would build on the brain BOTD is about to redesign.
- BOTD either adopts this card as one of its build cards (rewrite What to do from the design; keep the Acceptance numbers) or closes it as replaced. It is in BOTV's deps, so PERF / UCNT wait for it.
- Undo: `loopctl.py set FGHT phase=re deps=`.

## SAIR 2026-10-02 - the PS2's fight, read and measured (`analysis/re-soldier-ai.md` 4, 4.11, 12; commit e98b20e)
The RE half of this card is done. What the PS2 does, for BOTD to adopt or beat:
- **There is no "close on the target" rule.** A fight holds the weapon's band (inner = min(OptimalRange, MinRange + 3):
  back off 15 m; beyond outer = min(max(Optimal, MaxRange - 10), view - 10): advance 15 m; between: the mode) and is
  dropped beyond MaxRange + 10 m. Measured distance to the aimed soldier, first -> last sample of a fight spell:
  Kamino 30.8 -> 26.9 m (median change -1.1 m), Polis Massa 9.6 -> 8.1 m (0.0); spells 2.2 / 3.4 s median, p90 10 s.
- **What brings PS2 bots within 20 m is the noise order**: every think, every enemy AI within min(radius, 30) m of a
  unit gets "go to the noise" (walk to it, at most 10 s) and a threat entry; within 3 m a fight. No ray. On Polis
  Massa 54 % of on-foot combat time is that state (Kamino 10 %). The port has no such order: this is the likeliest
  source of this card's contact gap (3 % against 17-25 %).
- A shot passing within 7.5 m: run 20 m away from it or roll (BF2), face it (BF1). Damage out of a fight: walk to
  the attacker's position.
- Movement in the attack: a per-bot fixed style (stand / circle at optimal range / zig-zag / flank by the bot's own
  number), a 50 % draw every 2-5 s to take a Cover hint within 40 m that faces the target (21 % of attack time on
  Kamino, 49 % on Polis Massa), fire throughout.
- The capture order resumes when the fight ends (interrupt and resume); a new order during a fight replaces it.

## BOTD (2026-10-03) - What to do, from the design (bot-design.md 4.4); the Acceptance above stays, bars B5-B7 added
Build the first three behaviours of the table on SPL11's seams, under the PS2's interrupt rule (one change per think; what was interrupted resumes):
- **Investigate** - the PS2's noise order (SAIR 4.1): an enemy the bot hears within min(its radius, 30) m -> walk to it for at most 10 s, within 3 m fight; out of a fight, a hit by an attacker inside view distance -> walk to his position (at least 15 m out), then two looks of 0.4 s (SAIR 4.2).
- **Fight** - the target is the threat list's best inside weapon range + 10 m. Movement by the PS2's band until card TPOS: nearer than inner = min(optimal, min + 3) back off 15 m; beyond outer = min(max(optimal, range - 10), view - 10) advance 15 m; else stand (a third of the bots crouch). Given up when the target is beyond range + 10 m for 2 s; at least 2 s.
- **Evade** - out of a fight, a shot passing within 7.5 m or a hit by an attacker out of view: 20 m away, or a roll (by tier). The grenade flee moves here.
Check first that the baked weapon class carries OptimalRange and MinRange (`rebake=classes` if not).
**Acceptance, added:** **B5** an enemy within 20 m inside x 0.67-1.5 of the PS2 on KAM1, POL1, DEA1, KAS2; **B6** deaths a minute inside x 0.67-1.5; **B7** the three busiest cells' share of deaths >= 0.67 x PS2; the share of on-foot fight time in Investigate printed beside the PS2's 10 % (Kamino) and 54 % (Polis Massa).
Build step 7, after MOVR. In BOTV's deps.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decision 7: **the corpse shot is not built.** Decision 3 (the hide / fire cycle replaces the shuffle and strafe chances) is card TPOS's; this card must not add new random strafe rules.
- 2026-10-05 planner: `effort: high` (the builds the maintainer's top concern rests on; a build runs at medium by default). Undo: `loopctl.py set FGHT effort=`.

## Notes
- 2026-10-05 (the maintainer, via Claude): "I'm seeing bots just walk and then when I come near and they switch to firefight mode, everyone stops in their tracks which puts them all right next to each other. They should move around in the area, spread out a little bit or something." The Fight behaviour's entry must not be 'stop and fire where you are' (bot design 4.4's Fight row: the Tactician's position for (target, weapon)).

## Landed 69f8c84 (2026-10-07 03:53) - the FIGHT only; the contact bars are NOT met -> card FGHT2
- **Landed:** a bot fights a target it sees inside MaxRange + 10 m from a position of its fixed style - half stand
  (a third crouched, a spot 4 m clear of the other fighters), half circle (a routed walk to a ring point, a 1.9 s
  pause, the next; never backing away; the point in a nav hub); not against a rider. `ai/behave.rs`
  `fight_position` / `ring_point` / `stand_spot` / `style`, `FightState`, `WeaponClass::ai_range`.
- **Acceptance, measured (8 seeds x 32 maps against 55f055d, `lead_tmp/fght/ab/base` and `ab/c5_all`):** Kamino an
  enemy within 20 m 3.5 -> 3.7 % (bar 12: NOT met), kills a minute 17.9 -> 17.6 (15-20: met); Polis Massa 25.3 ->
  24.2 % (bar 28: NOT met); 256 of 256 rounds finish (met). B5 (botq): Kamino 3.7 -> 3.9 (PS2 15.1), Polis Massa
  32.0 -> 24.1 (35.9; the floor is 24.05), Death Star 2.3 -> 5.3 (18.6), Kashyyyk 2 5.0 -> 5.0 (5.3). All maps:
  contact 10.0 -> 11.2 % (up on 27 of 32), recycles 61 -> 59.
- **What it does for the maintainer's 10-05 note** (in a fight, a teammate within 3 m): Kamino 17.1 -> 13.1 %, Polis Massa
  51.5 -> 39.9 %, Death Star 50.0 -> 34.9 %, Kashyyyk 2 15.7 -> 8.9 %; fighters standing still 61-72 -> 45-56 %.
- **Not landed, measured:** Investigate (the noise order) and Evade (a passing shot) - re-soldier-ai 15.2, 15.3;
  the switch build is `lead_tmp/fght/knobs_k6.patch`. The premise "the noise order closes the gap" is false.
- Report `reports/20261007_0354_card_FGHT.md`.
