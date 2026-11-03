---
id: FXP5
title: Particle effects, part 5: the spawn hooks FXP3 left — water hits (bolts stop at the water line: ImpactEffectWater, the explosion's WaterEffect at/below water + 0.3 m), TrailEffect riding missiles/rockets (grenades wait for GREN), vehicle DamageEffect stages / ChunkTrail / ChunkSmoke / Thrust / Wake / Stomp, BF1 LightFlash handoff to BPNT; PCSX2 pairs
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
FXP3 landed the struck-bolt route (ImpactEffect Soft/Rigid/Static/Terrain by surface, the ordnance's ExplosionName Effect, destroyed vehicles' Effect). Its card's remaining acceptance moves here: nothing in the port emits Surface::Water or Surface::Shield, no round carries its TrailEffect, and vehicles spawn none of their own effects.

## Evidence
analysis/re-fx.md §7 [O]; FXP3 card (the Acceptance and its FXB note: BF1 LightFlash, analysis/re-bolt.md §5.1); per-weapon chains lead_tmp/fxp/AGENT_C_weapons.txt; B1 explosion FUN_002701a0 (193756–193807: WaterEffect when the blast is at/below the water + 0.3 m); B1 FUN_00272d70 detonates at hit + 0.2·v (v unidentified — the port backs off along the bolt, PROVISIONAL; FXP2's NAB1 note says the PS2 flash reads whole on terrain).

## Acceptance
- A bolt crossing a map's water line ends there with Event::Impact{surface: Water}: ImpactEffectWater (BF1 unauthored → watersplash_ord), and its explosion's WaterEffect instead of Effect when at/below water + 0.3 m; unit test per rule.
- TrailEffect attached to rockets/missiles for their flight (a fx_capture or live frame, viewed).
- Vehicle effects where the port has the event (damage stages, destroyed chunks, thrust, wake, stomp): each wired or listed as a card.
- PCSX2 pairs (muzzle-side view, a ground impact, a blast) on BF1 NAB1 and BF2 HOT1 — the PS2 side is lead_tmp/fxp/ps2cap/; the port side at the matching pose; viewed, differences carded.
- Gates: sim hashes unmoved (Event fields are outputs) unless the water stop is declared (it changes bolt flight → sim/sweep/arena: declare and say why).

## Note (2026-09-30, BPNT RE, `analysis/re-point-lights.md` §4 [V, helper B])
- **BF2's explosion LightFlash** (FUN_0030c8f8 → FUN_0030e108): for an explosion class with LightRadius > 0 and
  LightDuration > 0, a NON-Static omni:
  - colour = min(1, 20·LightColor′/255) per channel (LightColor′ premultiplied by its 4th value);
  - radius 3 × LightRadius, at the blast point + (0, 3, 0);
  - fading linearly to black over LightDuration (FUN_0030e2e8).
- It lights models through BPNT's per-draw point path, and the terrain through FXBL's pass.
- Hand the port's live flashes to BPNT's runtime light list.
- BF1's LightFlash term on models is ((1 + N·L̂)/2)⁴ (`re-bolt.md` §5.4).

- 2026-09-30 (BPNT build): the explosion LightFlash goes into `game::point_light::submit` — push a
  `point_light::omni(pos + (0, 3, 0), rgb, 3 × LightRadius, false)` into the frame's list (models) and the same
  `OmniLight` into `ground` (the terrain pass); `analysis/re-point-lights.md` §4 has the colour and the fade.

**Note (cleanse CLNC, 2026-09-30; inferred from the code, not rendered):** `Event::Impact` carries `pos: b.end` (`core/src/sim/combat.rs`), the spawn ray's endpoint fixed at fire time (`Bolt::spawn`), while the id guard damages the soldier wherever he is NOW — a strafing soldier hit after 0.3 s of flight gets his impact effect ~2 m off his body, and `fx::spawn_riding` takes a moving hull's local offset from the stale point. FXP3's placeholder used `soldier.pos + HIT_HEIGHT`. Check on a strafing target (a frame pair) and, if it shows, place a `Surface::Soldier` / hull impact from the victim's current position (presentation only: events are unhashed).

- 2026-10-06 (card WADE, lane 2 - a find for this card, no new card opened): **a wading soldier makes the same ripple and a fall into water a splash** (`analysis/re-water.md` section 9.2 [O]). Soldier update B1 `FUN_00205cc8` 117243-117268 / B2 `FUN_00240bf8` 160044-160069: one GLOBAL timer for all soldiers, period 1 / (0.3 v^2 + 4) s (0.25 s at rest, 0.06 s at 6.5 m/s); on expiry the ripple record `FUN_00478778(2.0, 0.25, pos, vel)` / B2 `FUN_00563230` - added only on a flat-water map, in a wet cell, with |v| > 0, at most 100 alive, placed 0.03 s ahead along the velocity at the LAYR height, life 2.0 s, radius 0.25 + 1.5 x age (B1 552055) - and the foley surface set to water; and when v.y < -5 m/s, at most once a second (a second global), the class's splash effect (class + 0x1900 in BF1, + 0x8c4 in BF2) at his position. The ripple renderer is B1 `FUN_0048d2b8` / B2 `FUN_00577ad0`, drawn in the water's own bucket right after the near pass (two textures, records 0x69c3f0 / 0x69c400; unread past the radius line). PS2 frames with a ripple ring round a trooper: `pcsx2/ref/bf1_kas2/wade/s094.png`.
