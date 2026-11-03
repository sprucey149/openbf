---
id: ATUR
title: The BF2 sniper's portable auto-turret: the dispenser throws a `portableturret` that falls, lands upright, turns on the nearest enemy by its stationary controller, fires its cannon in salvos inside MaxRange, takes damage by BuildingScale, expires at its Lifetime and is replaced past MaxItems
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
`analysis/re-portable-turret.md` (`[O]`), issue #45. Every BF2 sniper carries a portable auto-turret in slot 4, channel
1; the class kit dropped the slot (14 classes counting variants).

## What to build
1. **The weapon** (§0, §5): the dispenser throw of the `OrdnanceName` turret at the charge-scaled speed plus the
   owner's velocity, the owner's team; past `MaxItems` the oldest is killed and replaced.
2. **The turret** (§2, §3, §4): the `portableturret` class keys with the constructor defaults; the half-step fall;
   landing on a contact with n.y > 0.7, the friction / rebound bounce otherwise; `Lifetime` expiry; the −500 floor.
3. **Its AI and gun** (§6): the stationary controller's target choice (view 100 m, fight inside MinRange..MaxRange
   under the gun's `AITarget` mask, held to MaxRange + 10), the idle look, the hover stick turn under `MaxTurnSpeed` /
   `MaxPitchSpeed` and `PitchLimits`, fire inside MaxRange within 22.5°, the gun's own salvos; kills credit the owner.
4. **Damage**: bolts and blasts hit it by its `HealthType` scale (Building); destroyed, expired or replaced, it explodes
   with its `ExplosionName`.
5. **Game** (§7): the `com_weap_inf_dropturret` model with its side sheet `com_prop_dropturret_rep`, hovering on its
   own geometry above the landing point, front (+Z) to the aim, drawn between ticks; the band's UV scroll in the world
   and on the HUD's item icon; its explosion look and sound; the headless log.
6. **Bots** (§6.1): press the dispenser when the target is a soldier on foot in a cluster of his side (the grenade's
   group test) and the weapon is ready; no range or live-turret test, so the ammo, the reload and `MaxItems` set the
   rate. The weapon choice never takes the dispenser.

## Acceptance (run and paste into the report)
- Unit tests: class keys and constructor defaults; lifetime and destroyed ends; land and bounce; the turn law and pitch
  limits; deploy lands upright on the ground; an enemy in range is shot dead for the owner; one beyond MaxRange is
  tracked but not fired on; blasts and bolts by BuildingScale until it explodes; expiry at 60 s; a second throw
  replaces the first; a bot throws only at a soldier in a cluster of his side, not at a lone one or one with a friend
  of the bot by him; over a long fight a bot keeps at most `MaxItems` alive and throws once a round, a reload apart;
  the BF2 scroll bytes parse to the band's rate; a HUD mesh part scrolls with the HUD clock; the render blend draws it
  between ticks; turrets are hashed and replay exactly.
- Census: every carrying BF2 class builds the slot (slot 4, channel 1, 1 clip), reads the disc's dispenser / turret /
  gun / explosion values, deploys, lands, fires at an enemy and expires at 60 s ± 0.2 (14 classes); the baked model
  (common and side) carries the band's scroll and its sheet is baked.
- Gates: `sim` declared.
