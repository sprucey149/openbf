---
id: BRSTU
title: A seat gun's burst is its heat, not n shots: the PS2 counts a burst's n as ROUNDS spent, and a weapon with unlimited rounds (every seat gun, the repeater, the chaingun) fires until its heat is one salvo short of full or the bot stops wanting to, then the class's pause - the port ends every seat burst after 3-5 shots and since CLSE4 pauses 1.65-2.0 s (mounted shots -13 %, mounted kills on BF1 maps -18 %)
milestone: M2
value: 4
phase: build
status: done
deps:
files: engine/crates/core/src/ai/gunner_foot.rs,engine/crates/core/src/ai/gunner.rs,engine/crates/core/src/weapon.rs
declares: sweep; arena; sim
needs:
effort:
branch: brstu
resume: landed cbf6f686 2026-10-21
created: 2026-10-13
alpha: yes
domain: vehicle
---

## Why
AUDIT CLSE4 (2026-10-13, PARTLY; `analysis/re-soldier-ai.md` 23.2). Card CLSE4's two trigger rules are the PS2's and
its numbers hold - but its burst rule counts n SHOTS for every weapon and every seat ("a seat's gun takes its own
class's" Barrage keys), and on the PS2 n is a count of ROUNDS spent. A weapon with unlimited rounds is never held
to n: it fires until its heat is one salvo short of full, or the bot stops wanting to, then waits the class's pause.
That is every seat gun (a mounted bot's rounds word is "unlimited" on 3,404 of 3,405 firing samples of the running
PS2) and the hand weapons with no magazine count (the repeater, the chaingun). The port's turret and tank gunners
fire 3-5 shots and then stop for 1.65 s (BF2) / 2.0 s (BF1) - the landing took a fifth of the BF1 maps' mounted
kills on a rule the PS2 does not have. A player at a turret beside a bot's, or facing a bot's tank, sees it.

## Evidence (`engine/artifacts/lead_tmp/audit-clse4/`; report `reports/20261013_0646_audit_CLSE4.md`)
- **The PS2's code** (two verifiers, both games): state 3 of the burst function ends on the meter test OR
  `n <= rounds at the start - rounds now` (B2:241999-242000, B1:155084-155085); rounds = `FUN_003037e8` /
  `FUN_002840c0` = 0x7fffffff when `RoundsPerClip` was authored under 1 or the owner slot's reserve is unlimited
  (BF2: no `WeaponAmmo` on the slot or one under 1; BF1: `WeaponAmmo` under 1). The meter is HEAT: a burst starts
  when n shots' heat (`HeatPerShot` x `ShotsPerSalvo`) fits in what is left of `EnergyBar` and ends when one more
  salvo no longer fits; with `HeatPerShot` 0 it ends only when the want drops. The pause after it is the class's
  (`BarrageDelay`, x (1.5 - 0.8 t) in BF2), counted from the burst's end. One trigger function for the soldier,
  droid, droideka, hover, walker, flyer and mounted turret.
- **Live** (`ps2/burst_state_mounted.txt`, `burst_state_wclass.txt`, `burst_unlimited_bf1.txt`): BF2, six
  captures: mounted bots are in the firing state on 28.4 % of their samples; of 407 firing runs 72 % last 1.0 s
  or more, 43 % 2.5 s or more, 91 over 6 s (a rifle on foot: 12-16 % and 1-2 %). BF1, three captures: the bots on
  foot with unlimited rounds (2-4 a capture, team 1) 67-68 % and 15-32 %, with counted rounds 4-28 % and 2-6 %.
- **The port today** (`gunner_foot::fire_discipline`, called from `gunner::aim`; main 1aa04dc, 8 seeds x 32 maps):
  the seat's Barrage rule off -> on: shots per mounted-bot-second with a target 0.155 -> 0.135 (-12.9 %, t -4.7),
  kills by mounted bots on the BF1-rule maps 74.5 -> 60.9 per 100 minutes (-18 %, t -2.7), BF2's flat; the losers
  are all classes with the default 3 / 6 / 2.0 keys (the combat speeder's missile 2,046 -> 1,237 shots, the gun
  turret 561 -> 367). No class that authors the keys fired a shot from a seat in any sweep (`wf/K3`).
- **Knocked in as an upper bound** (`knob_main3.patch`, `AUDIT_CLSE4=32` in `bin/rto_mainsw3`: no shot count for a
  seat gun or a hand weapon whose `RoundsPerClip` is under 1; the weapon's own overheat lock stands in for the
  meter test - so it fires INTO the overheat, which the PS2 avoids): mounted shots per target-second 0.162 -> 0.235
  (BF2 rules) and 0.113 -> 0.121 (BF1), mounted kills 148 -> 173 and 58 -> 71 per 100 minutes, the repeater and
  chaingun 0.48 -> 2.87 shots per second in hand; deaths with an enemy inside 10 m +0.15 points (t +0.4), kills a
  minute -0.44 (t -1.7), 256 of 256 rounds (`out/mounted.txt`, `tools/mounted.py`, `tools/cmpseed.py`).
- **Tests**: mutation GN1 (the seat's burst takes the DEFAULT class's keys) fails no test; nothing seats a bot at
  a gun and counts its shots.
- Also read, smaller: `TriggerSingle` is ON from the base weapon class and OFF from the cannon class, so BF2's 11
  `remote` guided-rocket seat classes are tapped on the PS2 and burst in the port (0.7 % of mounted shots today);
  the seat path fires a single-shot gun by a held trigger, with no tap; the seat has no `MaxRange` test (a seat
  gun without the key: 120 m on the PS2).

## Notes
- 2026-10-13 audit CLSE4 (lane 1). Filed at value 4 (a PARTLY audit's card). The count of n for a seat is older
  than CLSE4 (card VTAI's bursts were 2-5 with a 0.35-0.75 s pause); CLSE4 made it bite. `files:` overlap card
  CLSE5 (`gunner_foot.rs`): the picker runs them one at a time.
- 2026-10-21 LANDED 2b5ebf6 + cbf6f68 (lane 1; report `reports/20261021_1925_card_BRSTU.md`, kit `lead_tmp/brstu`).
  Mounted shots per target-second BF2 0.357 -> 0.428, BF1 0.397 -> 0.473 (up on 23 of 24 vehicle maps; BF1 Hoth
  0.576 -> 0.549: main fired its turret cannon into its overheat), BF1 mounted kills 498 -> 574 /100 min, seat
  overheat samples 6,501 -> 1,855 (the rest: 1.15-heat-a-shot lasers and 2-salvo triggers), 256/256 rounds, team kills
  0.79 -> 0.85 % (mounted 116 -> 148: no friend-in-line check in a seat). Not built: TriggerSingle default by root.
- 2026-10-21 AUDIT (lane 1): PARTLY - the rule and code hold (two verifiers; the [H] on slot 0x2c resolved: FUN_00331460 =
  ShotsPerSalvo x HeatPerShot, +0x294 = ShotsPerSalvo by hash 0x3b38558), the numbers reproduce in direction; 18 of 33 audit mutations survive
  (card BRSTT) and the PS2 comparison measured an armed burst, not a held trigger (card MTRIG). `lead_tmp/audit-brstu/EVIDENCE.md`.
