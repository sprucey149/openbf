---
id: UNLK3
title: Unlock stages from career kills (OURS): 0 kills = the full points (8 / 12), 50 kills = half (4 / 6), 100 kills = unlocked for good - one table in profile; the class select shows the progress ('14 / 25 kills with the Jet Trooper')
status: archived
---

Historical technical record. Recorded state before archival was `in-progress`.

## Why
The maintainer (INBOX 2026-10-21 17:50 item 4b). The kill numbers and whether officers count as special are asked in STATUS
(default: 25 / 100, officers count with their 8-point tier).

## Acceptance
- Tests at 0, 24, 25, 99 and 100 kills for an 8- and a 12-point class; a picture of the class select line.

## The maintainer 2026-10-28 (stats screens review)
Permanent unlocks are a named deviation he wants as **awards**: unlocking a locked class for good is something the
player earns and sees on the end-of-round screens, not only a silent counter. Shape still to agree with him: (a) an
award line ("Jet Trooper unlocked") on the stats pages when the 100th kill lands, or (b) the round's awards
themselves (Killing Spree, Tank Buster ...) counting toward the unlock. Ask before building.

## The maintainer 2026-10-28 (design, settles the question above)
"Put it next to the other lifetime awards sections [the Career Stats medals]. So when you make kills with an
unlockable character, say a droideka, on that screen it shows that number increasing. The droideka kills would
just be added to the total kills that round in the round stats page. Nothing separate saying how many unlockable
character kills you got. And there should be one for every unlockable character. In bf2, each team has 2
unlockable characters each, so there is 8 from bf2. [...] So in the bf1 maps make those unlockable as well the
ones that are locked in bf2, like the droideka or the jet trooper or wookie or dark trooper."

## Build (2026-10-28)
- One table, `game/src/unlocks.rs` `UNLOCKABLES`: BF2's 8 classes with `PointsToUnlock` > 0 (resolved through the
  BASE chain in the baked `bf2_{all,imp,rep,cis}.ron`; `_hunt` / `_training` variants author 0 and stay out):
  Bothan Spy 8 (`all_inf_officer`, `_jungle`, `_snow`), Wookiee Warrior 12 (`all_inf_wookiee`, `_snow`; BF1
  `all_inf_smuggler`, `_smugglersnow`), Imperial Officer 8 (`imp_inf_officer`, `_gray`, `_snow`), Dark Trooper 12
  (`imp_inf_dark_trooper`; BF1 the same name + `imp_inf_dark_troopersnow`), Clone Commander 8 (`rep_inf_ep3_officer`),
  Jet Trooper 12 (`rep_inf_ep3_jettrooper`, `rep_inf_ep2_jettrooper`, `_rifleman`, `_sniper`; BF1
  `rep_inf_jet_trooper`), MagnaGuard 8 (`cis_inf_officer`), Droideka 12 (`cis_inf_droideka`, both games). Names are
  BF2's table's `entity.<side>.<rest>`. BF1 has no officers, so the four officers have no BF1 class. Tests pin the
  table to the baked class data and the names to the table (both skip without the bake).
- Counter = the sum of `Career::class_kills` (UNLK2) over the entry's classes; no profile field added, so kills
  recorded before this card count.
- UNLK1 folded in: `roster` gives a BF1 class with no `PointsToUnlock` its counterpart's cost (`unlocks::bf1_cost`),
  only on a BF1 class db. `game/tests/suite/unlocks_bf1.rs`: BF1 Kamino's jet trooper and Yavin's dark trooper and
  Wookiee smuggler refuse at 11 points and spawn at 12. The BF1 droideka is locked by the table but the port does
  not field a droideka yet (card DEKF), so it has no real-map test.
- Stages: `unlocks::effective_cost` (0-24 full, 25-99 half rounded up, 100+ 0), tested at 0/24/25/99/100 for 8 and 12.
  The sim gets the player's costs as an input like `spawn_points` (`Sim::spawn_costs`, not hashed); the points test
  of `Sim::class_refusal` reads it, the max test keeps the class's own cost (an unlocked-for-good class still has its
  max). Windowed play writes it each frame with the points (`unlocks::set_player_gate`), so a kill that crosses 25 or
  100 lowers the cost mid-round once its batch is flushed to the profile. The class screen's "Get %d more points",
  silhouette and Max line follow (spawn_page test).
- Career Stats: Down past a medal column's end turns the medal box to "Career Unlocks" (its wedge shows on the box's
  bottom border; an up wedge on the unlocks page), the same rows four a column: name, career kills, the cost now
  ("12 PTS" / "6 PTS" / "UNLOCKED"), and the info box the next stage ("14 of 25 kills with the Jet Trooper: it costs 12
  points a battle, 6 from 25 kills, and from 100 it is unlocked for good."). Up past their top comes back. The page
  could not hold a third box at 4:3 with the PS2's measured layout, hence the second page. Kills stay in the round's
  normal kill total; nothing per-round counts unlockable kills. Render: `stats_capture career_unlocks 640 448`.
- Not done: the class select's own progress line ('14 / 25 kills with the Jet Trooper') and its picture (the card's
  title/acceptance); the class screen shows only the earned-down cost.

## The maintainer 2026-10-28 (after the first build)
Half cost moved to **50** kills (100 = unlocked for good unchanged). The info lines reworded like BF2's award lines
("50 career kills with the Jet Trooper halves its unlock cost. 100 career kills unlock it permanently.", "You have
unlocked the Jet Trooper permanently.").
