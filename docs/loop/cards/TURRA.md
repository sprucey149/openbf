---
id: TURRA
title: TURR audit (PARTLY): exit 3 never fires in a battle (0 of 504 rounds: it reads the turret's hull hits and a building turret has no hull, while gunners hit from outside every arc stay seated); 18 of 52 audit mutations survive the scenes (one bot a turret, exit 2's 20 s, a turned turret's arc); the 20 s closure is not the PS2's cooldown; a Hoth lead on bolts fired past buildings
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT 2026-10-20 of card TURR (PARTLY; evidence `engine/artifacts/lead_tmp/audit-turr/EVIDENCE.md`, builds and sweeps
listed there). The landing's bars hold on 24 seeds (BF2 KAM1 seat share 6.94 %, no quiet long spell, nobody seated in
the endgame, kills a minute within the noise), but:
1. **Exit 3 never fires in a battle**: 0 of 504 rounds (24 seeds x 21 turret maps, today's main e95a687). It reads the
   turret's `Vehicle::last_hit`, which only hull damage writes, and a building turret has no hull: 5 hull hits in 168
   rounds, no building turret destroyed in 504. Gunners are hit as soldiers - 5,042 hits in 168 rounds, 35 of them from
   an enemy outside every arc (12 spells) - and none of those puts them out. Its "under 30 %" half and free_seat's
   30 % test never meet a damaged turret either (audit mutations M25 / M10: 168 of 168 rounds identical to main). The
   scene `exit_3_...` passes because it calls `damage_vehicle` by hand.
2. **18 of 52 audit mutations survive every test** (`lead_tmp/audit-turr/mut`, `tools/mk_mut.py --list`): one bot a
   turret is untested (M11: a claim on foot no longer takes the turret -> offers 12,167 -> 67,501 in 168 rounds); exit
   2's 20 s is untested (M22: 45 s -> the census's B8 bar fails, 0 -> 14 quiet long spells); no scene turns a turret
   (M34 arc_of and M25 exit 3 in world axes); M5 (the purpose ray not past the turret's own building), M10, M3, M6,
   M15, M24, M35, P3, B2, D1, D3, M1 also survive (M21 is equivalent; M2 and D3 argued equivalent in part by the judge).
3. **Labels**: `MAN_LEFT_CLOSED` is "the PS2's unboard cooldown" in man.rs - two verifiers: the PS2 writes its 20 s board
   cooldown only on an unboard order (B2:248051, `FUN_002c2ef8`, order 0x14), per UNIT for every board; the 10 s exit
   (`FUN_002cd2e0`) and the damage exit (`FUN_002c28e8`) write none - OURS. The AI seat's aim hand-off cited as
   `FUN_0026d748` B1:191963-191967 is the Aimer update and its AI smoother `FUN_00236700`; the hand-off itself is
   `FUN_00223850`'s no-player arm (B1:137845-137848: the held target +0x128 into T+0x20). The 50 m offer radius is a
   default (`SetAIVehicleNotifyRadius`: BF1 bes1*, rhn1*, BF2 spa* missions) and `SetStayInTurrets` is set by BF1
   bes1a / bes1a_h / bes1r / end1a / end1a_h as well as BF2 end1g_* (the card noted Endor only); the PS2's damage exit
   also needs the attacker within `FUN_002e3910`'s range and seen (`FUN_002accb0`, B2:247904-247907). Step A's "aim
   points 13-500 m out": the settled rows of `myg1_lg` span 13-61 m (the medians 0.004 / 3.5 m hold).
4. **Lead [H], showcase map**: the gunner's sight passes its own building and a seated target's (`past_buildings`), a
   bolt does not. Bolts fired at a target seen only past a building: BF2 HOT1 laser mortar 80.8 % (hits a bolt 0.145),
   BF1 HOT1 turret 76.2 % (0.081), BF2 KAM1 36.6 % (0.325); every other class 0-7 % (0.20-0.57). Unproven that those
   bolts stop at the wall (a mortar arcs; per-bolt fate not traced).
Design facts for the planner, not faults: exit 5 can never fire while the seat has a gun (0 of 504 rounds) so the defence
clause never keeps a gunner: defence-offered spells 63.6 % of 11 s or less, 51.3 % never with an enemy really hittable;
on main 51.1 % of seated time has nothing really hittable; a bot remembers one closed turret (A, B, A within a post).

## What to build
1. Exit 3 also reads the gunner's own hit (`Unit::last_hit` within `THINK_PERIOD`, an enemy outside every arc) beside the
   hull's; decide the PS2's range / sight gates (above) and say which in the doc.
2. Scenes, each mutation-checked with the audit's id: M11, M22, M34 + M25 (a turret turned 180 deg), M5, M10, M24, M3.
3. The labels and citations of item 3 in man.rs (and `analysis/bot-design.md` 6.1 if it repeats them).
4. Measure item 4 first (the fate of each Hoth turret bolt: wall or target); build only if the wall takes them.

## Acceptance
- Exit 3 counted on main's sweep of 24 seeds x 21 turret maps (today 0), the exits beside the outside-arc hits on seated
  gunners (`lead_tmp/audit-turr/tools/obs3_patch.py` gives them); sweep 256/256; kills a minute within the noise.
- Re-run `lead_tmp/audit-turr/tools/chain_mut.sh` (its done list emptied): only M21 (and argued M2 / D3) survive.
- B8 unchanged: BF2 KAM1 seat share 3-10 %, 0 quiet long spells, 0 endgame ticks (8 seeds).

## Planner note
Planner 2026-10-21 20:50: TCOL gives building turrets their hull (first 4); exit 3 can be refereed once it lands.
- 2026-10-21 (card TCOL re, lane 1): on the PS2 a hit on a manned building turret damages the building only (38 of
  38 live updates, `analysis/re-turret-collision.md` 3.1); a drawn gunner is hit as a body of his own at his seat, a
  hidden one cannot be hit. So exit 3's "the turret's hull hits" becomes refereeable when TCOL part 1 gives the
  turret its bodies; its "the gunner's own hit" half needs TCOL part 2 (the gunner at his seat, not the origin).
