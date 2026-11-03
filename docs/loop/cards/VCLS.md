---
id: VCLS
title: Vehicle class data: bake each map's own entc PROPS (TAT1's skiff, BES1's cloud car) and read hover and seat props into typed structs with the PS2's constructor defaults
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
A vehicle's drive and seat numbers live in its class props. Side classes are baked (hashes → strings) but nothing turns
them into typed numbers, and MAP-LOCAL classes (TAT1's `tat_hover_skiff`, BES1's `bes_fly_cloudcar`) are not baked at
all — a map bake keeps only a class's geometry and collision (`ManifestClass`).

## Evidence
`analysis/re-vehicles.md` §3.1 (the hover props, their offsets and the ctor defaults, identical in both games), §5 (the
FLYERSECTION seat ring: BODY = pilot, TURRET1..8 = gunner sections, PassengerSlots ≤ 4; PilotType none/self/vehicle/
remote(/vehicleself) = 0..3(4), default 2 in BF2), §10 item 2. `bake classes` already parses any LVL
(`bake/src/classes.rs`).
