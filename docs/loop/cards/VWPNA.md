---
id: VWPNA
title: Audit every turret and vehicle weapon on every map of both games: what it fires (bolt, rocket, shell, mortar, beam), how (salvo, charge, heat, refire), its ordnance's damage and blast, against the disc; the BF2 spider walker's gun is a BEAM (cis_weap_walk_spider_beam, base com_weap_veh_beam) and the port has no beam-firing code
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer's playtest 2026-10-28: "We need to check the weapons of all turrets and vehicles and make sure they're shooting the right thing in the right way and doing the correct damage. Turrets on different maps do different things and have different firing ammo with different damage and behaviors." Also: "Is the spider walker like some laser thing that it shoots in the originals?" - yes, a beam.

## Evidence
- `assets_baked/classes/bf2_cis.ron`: `cis_weap_walk_spider_beam` (wpnc, base `com_weap_veh_beam`), `cis_weap_walk_spider_beam_ord` (ordc, base `com_weap_veh_beam_ord`).
- `core/src/weapon/ordnance.rs` knows `OrdKind::Beam` (the AI's arsenal reads it); no beam fire / draw path found (grep 2026-10-28).
- Turret weapons differ per map (e.g. `tur_weap_recoilless_kas_*` an exploding shell, `tur_weap_geoturret`); open cards to fold in: RKSEAT, VTURW, TCOL, BRSTT, WPAT.

## Acceptance
- A table (analysis/) of every seat weapon class reachable in the baked maps of both games: ordnance kind, the fire law fields, damage / blast numbers, against what the port does; one card per mismatch.
- The beam built as its own card once the RE reads how the PS2 fires and draws it.
