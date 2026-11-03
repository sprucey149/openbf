---
id: TCOL
title: Building turrets and Laik cannons are whole bodies: bolts, rockets, blasts and grenades stop on the upper half and hurt it, the damage reaches the gunner by the PS2's rule, and a turret is destroyed (wreck, respawn or pad) if the PS2's is
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer (INBOX 2026-10-21 18:40 item 2, BF2 Geonosis?): rockets, bullets and grenades pass through the UPPER half of a Laik
cannon and hit its base; its health never drops; its gunner takes no damage. A building turret has no hull collision
today (TURRA: "a building turret has no hull"); RKHUL fixed walkers by putting each collision piece where its bone binds,
RKBARC does the BARC.

## RE answer (2026-10-21, `analysis/re-turret-collision.md` [O]; report `reports/20261021_2250_card_TCOL.md`)
- The PS2 places an armed building's prims through its own pose table, the same table its turrets write their live
  turret / aimer / barrel node matrices into: prims on those nodes TURN WITH THE GUN (yaw; the aimer's prims also pitch);
  prims on any other node rest at the bind (every one of them binds at the identity, 51 of 51 over 47 classes); the coll
  mesh sits at the root. The port takes every PRNT as the identity (assets.rs:615-621): Geonosis's
  `geo_bldg_geoturret` (BF1 and BF2) has 3 of its 4 prims on `aimer_x`, bound 1.60 m up: PS2 collision top 3.18 m
  (drawn 3.21), the port's 1.58 m - the maintainer's "upper half" exactly. Same gap on BF1 Bespin's AA gun (4.46 / 2.17), BF1 Hoth
  dish (3.28 / 2.36), BF2 roof chaingun (2.36 / 0.24); BF2 Mygeeto / Felucia / Kashyyyk AA and the BF2 dish bind their
  gun at the identity, so they are right at rest and wrong once the gun turns. Census: `lead_tmp/tcol/census_table.md`.
- A hit on the building damages the building only (live BF2 Hoth, 38 / 38 updates: building down 7 - 1,600, gunner
  unchanged). A DRAWN gunner keeps a body at his seat (BF2 type 0x20, 1.35 - 1.59 m out on the dish; BF1 type 0x10)
  that every flying ordnance's per-update ray tests; a HIDDEN gunner has no body. Blasts hurt a building turret's
  gunner (only a building mount lets its rider take splash), past an LOS that ignores the victim itself.
- Health 0: the turrets switch off and their gunners are KILLED with the building's last hit (8 / 8 live: drawn ones
  leave corpses, hidden ones vanish); the building takes its destroyed model + destroyed collision (state 1) or its pad
  (state 2), regenerates MaxHealth / RespawnTime, and is rebuilt at full health (live: 10 / 24 Hoth turrets fell in
  240 s, 2 came back). The port: no hull, no ownership (`Hit::World`), blast LOS blocked by the victim's own bodies
  (blast.rs `los_past`), the gunner at the building's ORIGIN (seats.rs `carry_riders`), a dead turret removed.

## Build plan
1. **Part 1 - the body.** Posed-node prims of a building turret become movers (`CollisionWorld::with_movers`, as the
   doors), placed each tick from the turret's yaw / the aimer's direction by `re-turret-draw.md` 2.2-2.3's node formulas
   (today in `game/src/viewer/turret_pose.rs`: move the pure math to core, share it with the draw); static-node prims
   and the mesh as today. Every body of the turret's instance is the turret's: resolve `Body.instance` to the turret
   whose `Vehicle::instance` matches (core), so a bolt / rocket / shell that meets one damages the pool (HealthType
   building scale) and the blast's LOS ignores the victim's own bodies; the gunner's own shots, aim ray and exit keep
   passing them (`ray_past`, TURR). (VTHUL's acceptance is part of this.)
2. **Part 2 - the gunner.** A drawn gunner's body at his seat (`L x node x O`, `GunnerHang`), hittable; a hidden
   gunner (no pose key / IsPilotExposed 0) out of the collision world; the building's damage never reaches him.
3. **Death** is card TBLD's state machine (state 1 destroyed model + its collision K+0x388, state 2 pad, regeneration,
   rebuild); until TBLD lands, keep today's removal. Do not put `vehicle_pads.rs` in this card's files (NODMG holds it).

## Acceptance
- **Repro scratch** (RKHUL's `lead_tmp/rkhul/rkex_repro.rs` pattern, a new turret mode): BF2 GEO1 `tur_bldg_geoturret`
  first, then BF1 GEO1 `geo_bldg_geoturret`, BF1 BES1 `bes_bldg_aaa_turret`, BF2 MYG1 `tur_bldg_recoilless_lg`, BF2 HOT1
  `tur_bldg_hoth_dishturret`: a rocket and a rifle bolt aimed at the gun's middle (y 2.2 m on the cannon) from 5 / 20 /
  60 m, the gun turned 0 / 90 / 180 degrees: on main they pass above 1.6 m (expected 0 / N on the cannon); on the branch
  N / N stop on the turret, and the pool falls by the weapon's damage x its building scale.
- **Census test** over the baked classes: each posed-node prim's rest position equals the PS2's bind position (the
  census tops: cannon 3.18, BF1 AA 4.46, BF1 dish 3.28, roof chaingun 2.36), mutation-checked (identity again -> fail).
- **Part 2**: a scene where a bolt aimed at a drawn gunner's seat (dish: 1.4 m out) hits him and a hidden laser-tower
  gunner cannot be hit; 1,000 damage to the building leaves the gunner's health unchanged.
- Gates: sim / sweep moves declared (bots' bolts now hit turrets and pools; turrets fall); rounds finished unchanged;
  say in the report how many turret hits and turret deaths a 32-map sweep counts (main: 0 hull hits from bolts).

## Original card text (2026-10-21)
### Do (original)
1. RE (both games): the turret's collision prims and their PRNT bones (RKHUL's rule); does a seated unit take the
   turret's hits, the blast, both or neither; can a building turret's health reach 0, and what then (wreck, respawn
   time, TBLD's pad, an explosion). Census every building turret / cannon class on both discs.
2. Build: every building turret and cannon gets its whole shape in the collision world, bolts / rockets / blasts /
   grenades damage its pool, the gunner rule as read, destruction as read.

### Acceptance (original)
- A repro per class (BF2 GEO1's Laik cannon first): rockets and bolts aimed at the upper half stop on it N/N (was 0/N
  where it passes today), its health falls by the weapon's damage, the gunner's health moves by the PS2's rule.
- A census over every building turret class on both discs: bodies per class against the PS2's prims.
- Related cards: VTHUL (a bolt at the building damages the turret's pool), TURRA (exit 3 reads the hull hits), TBLD
  (a destroyed turret's pad). Batch with VTHUL if the files meet.

## Planner note
`files:` left empty for the re phase (no code is written), so the parked ARVSA branch's combat.rs tests do not hold it back. At `phase=build` fill `files:` - likely engine/crates/core/src/sim/vehicles.rs, sim/combat.rs, sim/blast.rs; ARVSA (parked, waits on the RTX) adds tests to combat.rs and blast.rs: rebase on it once it lands.
