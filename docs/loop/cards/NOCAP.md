---
id: NOCAP
title: Uncapturable command posts: BF1 HOT1's shield generator ('shield') and END1's bunker panel ('cp1') are posts on the PS2 (a hologram and its r-8 light in RAM in every walk state, owned by their team) but have no CaptureRegion, and the port's layout loads only posts that have one - so the port has no post, hologram, light (and, to check, spawn point) there
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Found by card B1LL's light referee (lead_tmp/b1ll/referee.py over the BF1 walk states, referee.txt): of 1203 view-0 omnis the PS2 files, the only 10 the port lacks are the hologram lights of HOT1 'shield' (hoth_bldg_shieldgenerator, light at (-423.92, -18.21, 186.03)) and END1 'cp1' (end_bldg_bunker_controlpanel, (137.85, -5.14, -229.31)). Both instances carry Team, Label, ControlRegion and SpawnPath but no CaptureRegion; MapLayout::from_manifest (game/src/layout.rs ~318) skips any instance without CaptureRegion as 'not a command post'. The hologram (card CPHOL) is missing there for the same reason. Census of the baked manifests (ControlRegion without CaptureRegion): bf1_end1 1, bf1_hot1 1, bf1_geo1 4 (no PS2 hologram in GEO1's states: check what they are), bf2_hot1 1, and many BF2 ctf / campaign-only posts.

## Evidence
- RAM: lead_tmp/b1dk/lights1.py on bf1_hot1_walk_*_b000 / bf1_end1_walk_*_b000: a mask-2 r-8 hologram light at those points, enemy-red from team 1.
- PS2 rule to settle first (RE, bounded): is a post with no CaptureRegion an untakeable post that still spawns its team, shows on the minimap and counts for the bleed? (BF2 has com_bldg_untakeable_controlzone; HOT1's shield is the Alliance's home.)

## Acceptance
- referee.py: the 10 misses -> 0 (place within 5 cm, owner colour).
- A battle on BF1 HOT1 / END1: bots of the owning team spawn at the post when the PS2 does (count per 8 seeds), the post is never captured; sweep finish counts not worse than main beyond noise.
