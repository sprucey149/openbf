---
id: HITSND
title: Impact audio follows the firing ordnance at contact, without a rifle sound per splash victim
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
GSND finding 1 is reproducible through the simulation and the event-sound caller.
A projectile retains its firing weapon, but collision audio reads a unit's
current weapon. Splash damage also starts that weapon's contact sound for every
victim. The existing Impact event already records the contact's weapon and
position; a damage event is not a separate ordnance contact.

## Evidence
On main 983addb, temporary event_sounds tests use built-in weapon classes with
distinct generated collision names, a flat heightfield and public Sim::step /
event_sounds::each. No audio device, shipped assets or injected events are used.

- Fire at an idle enemy 20 m ahead, release, then SWITCH_WEAPON before arrival:
  Impact keeps fired_contact's class; each reports held_contact on tick 10.
- Enter a generated building turret through enter_vehicle and FIRE its gun:
  the real bolt carries the seat weapon; each reports the rider's rifle sound.
- Slot 1 fires into terrain while slot 0 holds the other weapon: each reports
  held_contact instead of fired_contact at (30.054926,-0.15711606,-1.6761211).
- An ordinary soldier hit sounds at the victim centre (0,1,-20), although
  Impact records (-0.025676366,0.9459828,-19.500618).
- Public Sim::explode damages two enemies and emits two Hit events, no Impact;
  each starts fired_contact twice at their centres. The explosion has no
  contact sound or visual sound in this control.

Five target assertions fail on the parent; unchanged firing-property control
passes. A temporary diagnostic that reads collision_sound and position from
Impact, preserving its explosion handling and removing Hit/BoltExpired contact
starts, makes all six pass. Diagnostic and probe source were restored exactly;
this planner commits no runtime repair. These are synthetic runtime witnesses,
not a shipped-bank prevalence measurement or new PS2 evidence.

Existing policy/reference: analysis/re-weapon-sounds.md section 4.1 describes an
explosion's SoundProperty once per explosion, section 5.5 the ordnance class's
CollisionSound at its contact, section 6 sticky contact sounds. sim/combat.rs
push_impact already records actual bolt weapon/position for soldier, vehicle,
terrain and world contacts, before any resulting splash Hit events. Existing
BF2 water/foliage uncertainties remain; this slice does not settle them.

## Scope
GSND finding 1 only, including the adjacent wrong-viewer-weapon BoltExpired
path of the same defect. Use existing contact provenance without changing the
simulation/event schema or inferring contact identity from damage recipients,
shooter state, event adjacency or approximate position matching. Keep explosion
sounds once per existing source, sticky bounce/stick/water handling, fire and
reload/equip/empty, cry and voice behavior. Keep every actual contact (including
coincident contacts), with no extra contact for each splash victim. No sound
bank, material policy, RNG, damage, simulation hash, thresholds or PS2 constants
change. GSND retains findings 4/5/SPWS and all of its original Acceptance;
VOICEP's implemented findings 2+3 still require their full validation.

## Acceptance
1. Permanent device-free caller tests reproduce the five parent assertions
   above through actual Sim::step firing/switching/seating, actual Impact/Hit
   output, public Sim::explode and event_sounds::each. Verify setup first:
   correct fired class, completed contact and actual victim damage. Preserve
   the unchanged-weapon property control. No backend skip counts as a pass.
2. Cover a real thrown grenade's fuse/contact/blast path and a rocket's direct
   contact plus multiple splash victims: the existing explosion sound starts
   once, sticky contact sounds retain their own cadence, damage alone adds no
   current-weapon contact. Original GSND grenade bar remains required. Add
   world and vehicle contacts, two coincident distinct ordnance classes,
   missing collision property, pure airborne expiry and stale-target controls;
   assert exact names/counts/positions. Keep existing tick-delivery tests.
3. Run exact-parent probes and separately restore current-shooter selection,
   local-viewer selection, victim-centre position, Hit-based contact starts,
   and duplicate BoltExpired playback. Each relevant permanent assertion must
   fail; a compile/backend/setup failure is not a mutation receipt. Restore
   the candidate and rerun clean. Calls to each must preserve sim hash/events.
4. Run focused event_sounds tests, full game library, fmt, workspace all-target
   Clippy and committed map/protect checks, one build at a time; full CI and
   unchanged applicable audio/GPU/golden/parity/sim/sweep/arena/leak/bake gates
   remain required. Keep CRATES/module docs current and size policy intact.
5. On real BF1/BF2 classes/missions, record bank/input/build identity and replay
   seat-gun, in-flight switch, rocket multi-victim and thrown-grenade scenes.
   Log every Impact/StickyContact/StickyBlast and handed sound name/position,
   compare parent/candidate, list changed sound rows, listen to the takes.
   Require firing-ordnance contact names/positions, one contact per actual
   bolt impact and no false current-weapon contacts per splash victim; retain
   explosion/contact controls. Fixtures do not earn real-content acceptance.
