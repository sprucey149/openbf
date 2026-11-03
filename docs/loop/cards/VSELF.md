---
id: VSELF
title: Self-piloted turrets (PilotType = self): the PS2's automatic turret is a seat with a stationary AI controller of its own and nobody in it - BF2 Hoth 1-flag CTF / hunt (14), the campaign's turrets, the dispensed auto turret; build a soldier-less gunner on ai::gunner (re-turret-ai 3, 6)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The PS2's "automatic turret" is a turret section whose ODF says `PilotType = self`. The port parses the key and only
makes such a seat unboardable (`game/src/vehicle_pads.rs` `seat_setup`); nothing ever fires it. No conquest world on
either disc places one, so no battle the port plays today is missing a gun for this reason: this is content for the
modes PLAN keeps for later (CTF, hunt, BF2's campaign) and for the dispensed auto turret. `defer: post-alpha`.

## Evidence (`analysis/re-turret-ai.md`, card VTAT, 2026-10-07; verifier 41 of 46 SUPPORTED, 5 PARTLY folded in)
- BF2 [O]: the seat is its own pilot from construction; the armed building's spawn gives it a
  UnitStationaryController; a pass every 15th AI step gives a self-piloted controller with no order the order type 10
  = UnitWaitAgent; from there it is an ordinary AI gunner with nobody in it (sections 2, 3). It cannot leave (no
  character). Its team is its owner's; it goes with the building and returns with it (the roof gun: 120 s).
- Live, BF2 Hoth 1-flag CTF (section 6): 14 turrets (13 `tur_bldg_chaingun_tripod`, 1 `tur_bldg_chaingun_roof`), all
  with that controller on 86,605 rows; a look point 84-96 % of the time (drawn within 30 deg of the base direction);
  soldiers held as targets out to 100 m, fought to MaxRange + 10 m (89 m for the 80 m chaingun), the trigger pressed
  only at 10-77 m; think every 4 s unwatched, 1 s in view, 1.6 s in a fight; bursts of 0.7-7.6 s.
- Where they are (section 1): HOT1 14 (`hoth_objective` 10: campaign, 1-flag, hunt; `hoth_ctf` 4), KAM1 11, MUS1 10,
  FEL1 5, MYG1 4, NAB2 3, UTA1 2 (campaign worlds), 316 on the space maps; `com_bldg_inf_autoturret` is what a
  dispenser drops (`Lifetime 60.0`).
- BF1 has NO such AI (section 4): its only self-piloted turrets are the fambaa's (card FAMB: `Controllable = 0` +
  `AutoFire = 1`, no controller).
- Kept: `engine/artifacts/lead_tmp/vtat/ps2/hot1g_1flag_battle.p2s`, the captures `ps2/ctf*`, the sampler
  `vtat_live.py` (boots from the Instant Action planet list: Hoth, mode row 2).

## What to build
1. A seat whose PilotType is `self` gets a gunner with no soldier: team = the owner's, perception from the seat's eye
   and view direction, `ai::gunner`'s aim and trigger (card VTAT's dispatch), the PS2's wait / fight: a look point
   within 30 deg of the spawn facing (re-drawn rarely), a fight only on a target inside a weapon's MinRange..MaxRange
   that its `AITarget*` mask allows, kept to MaxRange + 10 m, never an exit.
2. Gone with the building, back with it.
3. A mode or scene that places them: BF2 Hoth's `hoth_ctf` / `hoth_objective` worlds (the layer pick is
   `game/src/assets.rs` `mode_layer_index`).

## Acceptance (run and paste)
- A headless scene test at BF2 Hoth's `tur_bldg_chaingun_tripod16` (disc pose (-516.5, -0.6, -95.8), team 1), the
  PS2's band: an enemy soldier standing at 60 m in its arc is fired at within 10 s; one at 95 m is aimed at and NOT
  fired at; one at 110 m is ignored; a friendly soldier at 30 m is never fired at. Mutation-checked (the range gate
  removed fails the 95 m case).
- A census line per map: auto-turret seats, shots, kills (0 everywhere in conquest; the Hoth scene > 0).
- Sweep 8 seeds x 32 maps unchanged in conquest (`sim` hash equal: no conquest world places one).

## Notes
- Created 2026-10-07 by VTAT's RE (lane 2). Value 3, post-alpha: no showcase conquest battle has one.
- The PS2's watched turrets twitch (344-471 look points in 64 s in the camera's view): not a behaviour to copy.
