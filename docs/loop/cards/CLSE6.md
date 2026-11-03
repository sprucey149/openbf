---
id: CLSE6
title: Polis Massa inside 10 m: 3.4 port bots aim at one enemy where the PS2 has 1.4 (Kamino 2.1 / 1.3) - the largest term of CLSE3's 0.27 x per aimer-second (crowding 0.41 x, the victim's hurt rate 0.62 x); 35 % of the port's close aimer time is in cp2con's medical room against the PS2's 6.7 %: find why the port's fights pile onto one enemy there, then the hurt rate
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card CLSE3 (report `loop/reports/20261022_0720_card_CLSE3.md`, evidence `lead_tmp/clse3/`) split Polis Massa's inside-10 m damage per
aimer-second (port 22.0 hp against the PS2's 80.8 = 0.27 x on main 9193a4c) and found the rate is mostly a CROWD term: the metric
splits every health fall over the victim's aimers, and a port aimer there shares its victim with a mean 3.38 aimers (5+ for 36.7 %
of the time) against the PS2's 1.40 (alone 69.6 %); the victim itself loses 63.6 hp per second against the PS2's 103.3 (0.62 x).
1.40 / 3.38 x 0.62 = 0.26, the measured 0.27. Kamino: 2.12 against 1.31 aimers, 57.6 against 100.9 hp/s.
Neither PS2 gunnery rule knocked in moved the figure (CLSE3 switches: the PS2's sight line eye 1.7 m -> target point 1.55 m:
22.0 -> 19.2-20.4; the PS2's 22.5 deg trigger cone: 22.0 -> 22.7 / 20.5).

## Evidence (`lead_tmp/clse3/out/`)
- `crowd.txt` (`tools/crowd.py`): aimers per victim and the victim's hurt rate, both sides; `local_force.txt`: a port close
  aimer has 1.59 friends within 10 m (PS2 0.94), 1.52 enemies (PS2 1.69); with 2+ enemies near it takes the nearest 66 % (PS2 53 %).
- `cp0_room.txt` + `where_port.py` / `where_ps2.py`: AI unit time within 8 m of cp2con 5.8 % port / 4.3 % PS2, but 35 % of the port's
  close aimer time is there (PS2 6.7 %); the cp stands on `pol1_prop_medical_bed3` (a 1.37 m box) and half that time the target is
  hidden (`blockers_pol1.txt`).
- `ps2_los_pol1.txt`: the PS2's close aimers put through the port's sight test - the line is blocked for 14 % of their time (the
  port's own aimers: 35 % not seen) and the PS2 fires 35 % of that time anyway.

## What to do
1. Per aimer inside 10 m, both sides: who else holds its victim (range, seen or told, behaviour) - is it the told contacts (card
   TEAM, OURS), the retarget rule (nearest-first, 1.5 s, +10 hysteresis), or a denser local force (1.59 friends against 0.94)?
2. Knock the candidate into the CLSE3 scratch (`lead_tmp/clse3/scratch_9193a4c.patch`, `tools/mini.sh`) and re-run crowd.py.
3. Then the victim's hurt rate (0.62 x): `tools/cause.py` (a shot at a target inside 10 m hits it 44 %; 39 % fly past into a wall).

## Acceptance
- Aimers per victim inside 10 m on Polis Massa at 2.0 or under (PS2 1.40; main 3.38) and the per-victim hurt rate at 0.67 x the
  PS2's 103 hp/s or more, 8 seeds x both maps, team kills not above main's share - or the numbers that say why not.
