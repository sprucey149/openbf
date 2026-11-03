---
id: LEAFF
title: Forest leaves, ground gusts and leafpatch bird anchors
status: archived
issue: 15
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #15](https://github.com/sprucey149/openbf/issues/15).

Airborne leaves already landed in ae4d886, "Falling leaves beneath forest
canopies". This change extends that implementation and its shared 100-leaf pool.

Ground gusts attempt 20 terrain-following leaves every 30 seconds near an
authored emitter. Native impulse/decay move them. Moving soldier and vehicle
footprints stir once per contact. This trigger is a named presentation
extension using read-only sim state and the original kick. Traced native
kick callers were gusts/projectile hits; original actor trigger parity is
not established.

The reported sound anchor belongs to BirdParticleManager. The property supplies
nearby anchors/counts for 30-second (BF1) / 20-second (BF2) bursts, script-authored textures/sizes and
five-second life. No shipped mission assigns the separate bird/grass sound
slots, so no invented leaf sound is added. Evidence and equations are in
analysis/re-falling-leaves.md.

Leaves and birds use native atlas orientation, unfogged particle blending and
quarter-alpha cutoff. Battle clocks pause with the sim; scene changes clear
all state. Maps without falling emitters do not enable these additions.

Local original-game checks cover 63 flutter + 15 tumble leaves, one ground
impulse and three bird trajectories/wing pairs. Both-disc census verifies
six-per-patch falling counts and three-per-anchor bird counts. GPU checks cover
cutout/blending, ordinary particle controls and scene clear. forest_capture
reproduces local before/after views and requires identical RGBA on a non-emitting
control. Game files and images stay ignored.

The maintainer requested a commit on main and packed dist/openbf.exe refresh.
This card does not assert upstream integration or exact original actor triggers.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
