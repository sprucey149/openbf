---
id: TBLD
title: Buildable turrets get built: an unbuilt pad (state 2, health 0 of MaxHealth) becomes its turret when repaired to full and a destroyed one falls back to its pad — PS2 bots build them within seconds (BF2 TAT2 CW 2/11 ~10 s after side select, YAV1 CW 5/6 by ~3.5 min; BF1 NAB1 8/21 before side select); the port never builds, so its 48 BF2 pads (and BF1's gun turrets after BZONE1) stay pads all battle
status: archived
---

Historical technical record. Recorded state before archival was `review`.

## Why
The RE is done (`analysis/re-build-zones.md`, 2026-10-03, commit 4e51e84; verifier 33 / 4 / 0). A pad becomes its
turret by a repair to full health, a fallen turret goes back to its pad (or to "destroyed") and regenerates at
MaxHealth / RespawnTime. The port has none of it: a buildable turret is not a sim object at all (`add_turrets` skips
it and the world draws a static pad), and a destroyed turret is removed for good. This card builds **the turret's
three states in the sim**. The tool that fills the health is card FCUT; the bots that use it are card BLDR; BF1's
start rule is BZONE1 (after BLDR).

## Evidence (`analysis/re-build-zones.md` 1, [O])
- State 0 built, 1 destroyed, 2 unbuilt. A class with an unbuilt model (`UnbuiltGeometryName`) starts in state 2
  with health 0 and regeneration 0: no seat, no turret collision, the pad model drawn.
- Health reaching MaxHealth in a state other than 0 -> built: state 0, the class's model and collision, health = max.
- Health reaching 0 in state 0 -> a class with an unbuilt model and no destroyed model falls to state 2 (the pad); any
  other to state 1. Either way regeneration = MaxHealth / `RespawnTime`; the class default for an armed building is
  120 s (BF1 `com_weap_gunturret`: 300 HP, 2.5 HP a second, measured); BF2's TUR.LVL turrets say 999999.9; GEO's
  geoturret 30; BF1 `yav_bldg_tower_turret` 50. Regeneration runs every update while health < max.
- A pad is team 0; a manned turret carries its gunner's team (as the port's turrets already do).
- PS2 RAM: an unbuilt pad 0.00 / 800, flags type 6; a fallen BF2 turret state 2, health 0, regeneration 0.0008.

## Build plan (one session)
- `core`: `Vehicle` gets a build state and a regeneration rate for `VehicleKind::Turret` (hashed); `VehicleSpec`
  carries `unbuilt` (starts in state 2), `falls_to_pad`, `respawn_time`. `Sim::destroy_vehicle` on a turret: riders
  out as today, the death blast as today, then the state change instead of removal. A turret not in state 0 takes no
  seat, no damage and has no hull in the collision queries. One entry point adds health to a turret
  (`Sim::heal_vehicle`, used by the regeneration tick here and by FCUT's cutter) and builds it at max.
- `game`: `add_turrets` spawns a buildable class as an unbuilt turret instead of skipping it (BF2 now; BF1 keeps
  starting built until BZONE1), reads `RespawnTime` (default 120 for an armed building). The world's static pad
  instance stays as it is for renders with no battle; in a battle the viewer hides it while its turret is built and
  draws the turret, and draws nothing extra while it is a pad. A destroyed (state 1) turret draws nothing (the
  destroyed model is a note on card VTURD).

## Acceptance
- A sim unit test, mutation-checked: an unbuilt turret has health 0, takes no gunner and blocks nothing; healed to
  MaxHealth it is state 0 with the turret's seat and collision; killed, a buildable class is state 2 at health 0 and
  a class with a destroyed model state 1; a turret with `RespawnTime` 120 and 300 HP is built again 120 s (+- one
  tick) after its fall with nobody near; one with 999999.9 is not after 600 s.
- `game/tests/vehicle_pads.rs`: the six BF2 pad maps spawn their 48 buildables as unbuilt turrets at the PS2's
  positions (COR1 11, TAT2 11, NAB2 9, KAS2 8, YAV1 6, END1 3) and the 71 built ones as before.
- A battle frame of a BF2 pad map shows the pads as today (the same pixels as main at t = 0 - a before / after pair
  of one pose in a battle, e.g. `tat2` at a pad).
- Gates: the sweep moves on the maps with turrets (declared `sim; sweep`); rounds finished unchanged. Say in the
  report how many turrets fall and come back in the sweep (a census line).

## Notes
- Do not start BF1's gun turrets unbuilt here: BZONE1 does it after BLDR, or NAB1's 21 turrets stay pads all battle.
- A fallen turret's riders: today's port kills them (`DeathCause::VehicleDestroyed`); keep that, it is VTURB's rule.
- Whether a pad can take damage on the PS2 was not tested ([H] no: its alive bit is clear).

## Original card (AUDIT VTUR2, 2026-09-30) - kept for the record
### Why (original)
VTUR2 (a1bf3e8) makes BF2's buildable turrets (`tur_bldg_laser`, `_beam`, `_chaingun`, `_guided_rocket`) start unbuilt.
That is right at t = 0, but nothing in the port ever builds them. On the PS2, bots build them within seconds of the
start and keep rebuilding them after they fall. So the port's 48 conquest pads (COR1 11, TAT2 11, NAB2 9, KAS2 8, YAV1 6,
END1 3) are 48 turrets the PS2 fields and the port never does. BF1's 37+ `com_weap_gunturret` (KAS2 16, NAB1 21, …) start
unbuilt too (BZONE1), and NAB1 is a showcase map.

### Evidence (original; AUDIT VTUR2 2026-09-30, `reports/20260930_1400_audit_VTUR2.md`, `lead_tmp/audit-vtur2/`)
- [V] The property names: 0x97C931C5 = `UnbuiltGeometryName` (a MODEL name) and 0x48542041 = `UnbuiltHoloOdf` (FNV).
  - B2 FUN_001e4970 B2:94473 stores the model at class+0x4d4 (model table 0x684530) and its "UnbuiltCollisionMesh" at
    class+0x4d8 (table 0x663938).
  - B2:94407 stores the hologram class at class+0x624.
  - B1 twin FUN_001c0b10: B1:67394 → class+0x156c, B1:67354 → class+0x1624.
- [V] The state machine, in the building part of the ArmedBuilding (entity word 0 = 0x61a470; ctor FUN_001e7a30 → FUN_001e2d40):
  - entity+0x274 = 2 (unbuilt) when class+0x4d4 is set (B2:93226): the unbuilt model and collision, and a hologram at
    entity+0x280. Otherwise it is 0.
  - FUN_001e3638 (death): a buildable with no DestroyedGeometryName goes back to 2 (B2:93593); anything else goes to 1.
  - FUN_001e3c38 (built): → 0 (B2:93833). It restores the class+0x160 model and class+0x168 collision and refills health
    (+0x170 = +0x174, B2:93989).
  - FUN_001e3c38 is reached through the damageable sub-object at entity+0x170: vtable 0x619a40 slot +0x10 = FUN_001e5330.
    ArmedBuilding's vtable 0x61a368 slot +0x10 = FUN_001e8d08 → FUN_001e85a8 → FUN_001e3c38.
  - Other writers of +0x274 are EntityTrap's (FUN_00257010, FUN_00258050), not the buildings'.
  - B1: FUN_001bf400 sets +0x2a4 = 2 the same way (B1:66399-66405).
- [V] PS2 RAM (`bstate.py`, `bstate1.py`, `bstate_all.txt`):
  - Every unbuilt pad has health 0.00-0.02 of 800 and every built one 800 of 800.
  - With 0 AI (the walk / ai0 states) all 48 BF2 pads stay unbuilt for 5+ minutes.
  - With AI, TAT2 CW (40 soldiers, 2 engineers) has 0/11 built at side select (153.0 s) and 2/11 at the spawn map (162.6 s).
  - YAV1 CW (35 soldiers, 7 engineers): 1/6 at side select, then 2, 1 (one fell back to its pad), 1, and 5/6 at 435.6 s.
  - BF1 NAB1 (normal battle): 8/21 gun turrets built by side select; 1-bot states 21/21 unbuilt.
- [H] The build is a repair to full health (the fusion cutter / engineer's repair, health 0 → MaxHealth). The hologram
  (`com_holo_turret`, BASE hologram: colour "0 225 98", "build", icon `com_icon_turret`, 35.0) is probably shown only to
  builders: a sniper at 8.6 m (BF2 COR1 walk/cp1_cp2_b090) and a trooper at 15 m (TAT2 walk/cp1_cp2_b270) see none.

### Acceptance (original)
- RE (`analysis/re-build-zones.md`): the repair path that fills a state-2 building's health (who can repair — class /
  weapon), its rate, and the AI's build goal (which bots go to a pad and when). Settle the hologram's visibility on the PS2.
- Build: in a port battle with AI, pads become turrets and fallen buildables become pads again. Measure the built count
  over time on BF2 TAT2 CW and YAV1 CW against the PS2's numbers above; the gate is ≥ 1 pad built within 60 s on each.
- A sim unit test: repair to MaxHealth → state 0 with the turret's collision and draw; kill → state 2, health 0.
- Declare the sweep / leak / arena moves.

### Notes (original)
- BZONE1 (the BF1 start rule) depends on this card. Landing it alone would leave NAB1's 21 turrets as pads all battle.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Note from VTURD's RE (2026-10-13, lane 2, `analysis/re-turret-draw.md` 5)
- The DRAW of a destroyed turret is card VTURX (value 3, deps VTURD + this card): on the PS2 the turret's tasks are
  removed at death (the angles freeze), the draw poses whatever model is current, and none of the 49 destroyed
  models on the discs holds a turret / aimer / barrel node - the wreck is one rigid model at the instance. PS2
  frames of two wrecks: `lead_tmp/vturd/ps2/myg1_destroyed_a.png` (BF2 Mygeeto's anti-aircraft gun, state 1, health
  0 of 4000), `lead_tmp/vturd/ps2/hot1_bots_lasermortar_turet4_a0.png` (a Hoth laser tower fallen in a battle).
- `antiaircraft_chunk1` (the destroyed model of BF1's Bespin gun and of BF2's `tur_bldg_recoilless_*`) is one name
  for two different models on the two discs: look it up per game.
- 2026-10-21 (card TCOL re, lane 1, `analysis/re-turret-collision.md` 4): live BF2 Hoth bots battle (240 s, per
  update): 10 of 24 building turrets fell, 2 came back to state 0 at full health (repaired); 8 of 8 manned deaths killed
  the gunner in the same update with the building's last hit (drawn gunners leave corpses, hidden ones are deleted,
  B2 `FUN_001e8498` -> `FUN_002f4e00`; BF1 `FUN_001c4ad0` -> `FUN_0023ac68`). The death also swaps the COLLISION:
  state 1 -> E+0x70 := K+0x388 (the destroyed model's `DestroyedCollisionMesh` + its prims), state 2 -> K+0x4d8 (the
  unbuilt mesh) (B2:93601-93636). Build this state machine after TCOL part 1 (the turret's bodies are then its own).

## 2026-10-28 build (branch `tbld`; the maintainer's playtest ask)
The maintainer: "When a turret is destroyed, the normal looking turrets should just show the little square on the ground, and not
draw the turret. Other turrets like the hoth turrets or geonosis ... have destroyed looking looks ... When it has even 1
health, it shows the normal intact look. The fusion cutter should be able to heal turrets like it can vehicles or
health/ammo bots." Then (same day): "when a turret is destroyed it can only be fixed by a fusion cutter for all maps. It
shouldn't magically heal itself after some set time. ... units with a fusion cutter when near a broken turret should have
a higher chance to go fix it as its next objective ... Make sure that multiple engineers aren't given the objective of
fixing the same turret at the same time though."

### RE (tags per RULES 5)
- [V] B2 `FUN_001e3638` B2:93584-93660 (read again): death with K+0x384 (`DestroyedGeometryName`) -> state 1, that model
  and K+0x388 collision; none and K+0x4d4 (`UnbuiltGeometryName`) -> state 2, the unbuilt model and K+0x4d8; neither ->
  state 1, model hidden and collision off (slot +0x50, `FUN_001690c8`; "hidden" is the doc's [O] reading of those two).
- [V] B2 `FUN_001e3458` B2:93483-93492 (read again): "built" only when health >= MaxHealth with the state not 0. So a
  destroyed turret at 1..max-1 is still destroyed; "even 1 health -> intact" holds for a turret that never fell.
- [D] disc census (`assets_baked` classes, fnv keys): the little square is `com_prop_buildzone` (common bakes both games,
  no `coll`), the unbuilt model of BF1 `com_weap_gunturret` and BF2 `tur_bldg_laser/_beam/...`. Wrecks: BF1 Hoth
  `hoth_bldg_dish_turret_destroyed` / `hoth_bldg_hoth_turret_destroyed`, Geonosis `geo_bldg_geoturret_dest`, Bespin
  `antiaircraft_chunk1`; BF2 the same Hoth / Geonosis wrecks, `com_bldg_chaingun_tripod_dest` / `_roof_dest`,
  `antiaircraft_chunk1` (MYG1, KAS2), `fel1_weap_prop_turret_chunk`, `uta1_bldg_turret_destroyed`,
  `yav_bldg_watchtower_hulk`. All baked with their collision; **no bake step is needed**. Neither model: BF2
  `tur_weap_built_gunturret` (KAM1, COR1, NAB2), DEA1 `tur_bldg_mortar`, BF1 `yav_bldg_tower_turret`, `tat_bldg_barge_turret`.
- [O][V] cutter (`re-build-zones.md` 2): BuildingHealth / BuildingRebuild / BuildingBuild = BF1 60 / 20 / 30, BF2
  100 / 50 / 50, a shot per 0.5 s; BF1 carriers are the pilots (slot 3, channel 1), BF2 the engineers (slot 2,
  channel 0) and the Jawas. A fallen turret also regenerates MaxHealth / RespawnTime on the PS2 [V B2:93789].

### Built
- core `sim/building_turrets.rs`: `TurretFall` (Wreck / Pad / Hidden), the fallen bodies swapped in at death
  (`CollisionWorld::instance_parts` / `replace_instance`), `Sim::repair_building` (built at MaxHealth only),
  `Target::Building` for the cutter (`sim/repair.rs`). **No regeneration** `[ours]` (the maintainer): `RespawnTime` is not read.
- game `turret_fall.rs` (the class rule, its collision parts, the fallen model per instance), `viewer/turret_fall.rs`
  (`SceneGpu::fallen`, swapped in `view_draws`). Deviation `[ours]`: a class with neither model drawn as
  `com_weap_gunturret` falls to the pad (the PS2 hides it).
- core `ai/fix.rs`: cutter carriers take the nearest broken turret within 50 m, one claim a turret, give up after 20 s
  with no gain; walk, turn, cutter up, trigger.
- Tests: core `building_turrets::tests` (3, mutation-checked: rise at > 0 fails all three); game suite `turret_fall` (6:
  look + bodies on BF1 YAV2 / HOT1, BF2 KAM1 / HOT1 / GEO1; cutter rates; view swap (GPU); bots build one at a time
  (claim mutation fails it); 300 s with no engineer stays down), `turret_enter` rewritten (stays down, cutter builds).
- Stills: `engine/artifacts/lead_tmp/tbld/{bf1_yav2_gunturret,bf1_hot1_dish,bf2_kam1_gunturret,bf2_hot1_dish,bf2_geo1_geoturret}.png`
  (example `turret_fall_stills`).

### Open
- BF2's 48 buildable pads still are not spawned (they could now start as down `Pad` turrets with health 0 and be
  built by the same cutter and bots); the hologram; the sweep census of falls / rebuilds; gates.
