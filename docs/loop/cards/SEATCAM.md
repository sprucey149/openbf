---
id: SEATCAM
title: A seat camera can end up under the terrain: in BF2 Geonosis's geoturret the camera sat 0.30 m under the terrain sheet, 2 m from the turret at its origin's height, where the class's camera (TrackCenter 0 2.5 0, offset 0 3.6 2.0) puts it ~6 m up; check every turret and vehicle seat
milestone: M2
value: 4
phase: build
status: done
deps:
files: engine/crates/game/src/walk.rs
declares:
needs:
effort:
branch:
resume:
created: 2026-10-28
---
## Why
The maintainer's playtest 2026-10-28: "when I got in a geonosis turret, the camera went underground", and asked whether the camera drops elsewhere too.

## Evidence
- Black box `engine/artifacts/playtest/prev1.json` of 09:14 (now rotated; build a43f94e5+dirty): BF2 GEO1 seed 48, human seated in vehicle 0 "Geonosian Turret" (`tur_bldg_geoturret`, geometry `geo_bldg_geoturret`) at (-7.42, -61.63, 77.93); camera row `cam` (-9.10, -61.76, 76.86), pitch -0.35.
- Terrain sheet (`Heightfield::height_at`, measured with a throwaway example 2026-10-28): -61.65 under the turret, **-61.46 under the camera**: the camera is 0.30 m below the sheet; no cutter there.
- The class (`assets_baked/classes/bf2_tur.ron`): props "0.0 2.5 0.0", "0.0 0.0 -2.0", "0.0 3.6 2.0" (hash names to resolve: TrackCenter / EyePointOffset / TrackOffset).
- `WalkSim::settle_camera` keeps a camera on the side of the sheet it is on (deliberate, for GEO1's hangar under uncut terrain), so a seat camera that starts under the sheet stays there. Lead: the seat's camera numbers do not reach `vehicle_camera` for this building turret (zeros), placing the camera at the origin's height.

## Acceptance
- Root cause named with the numbers (which seat row reaches `vehicle_camera`, what it holds).
- A test over every baked building turret and vehicle seat of both games: the settled seat camera is above the terrain sheet where the turret stands on it.
- GEO1's hangar case (camera under the sheet on purpose) still holds.

## Result
- Names by hash: `EyePointOffset` 0 2.5 0, `TrackCenter` 0 0 -2, `TrackOffset` 0 3.6 2 (the card's lead guessed the first two the other way round), `TiltValue` 5. The numbers do reach `vehicle_camera`; it hung them on the building's origin, so the pull point was 2 m behind the origin at its height, under the sheet. `settle_camera` then held the camera there, 0.30 m under.
- Building turrets now take `TrackCenter` / `TrackOffset` on the aim frame at the turret node (`re-turret-draw.md` 4 [V]). `tur_bldg_geoturret` at the playtest's spot: camera 4.42 m over the base. No third-person RAM row of this turret was found; other turrets have them, and the camera stands within 1 cm of the PS2's on settled rows of BF1 NAB1 and BF2 MYG1 (`turret_views::the_third_person_camera_is_where_the_ps2s_camera_stood`).
- The survey found a second case: `cis_tread_snailtank`'s `TrackCenter` is 12 m below its origin (BF2 FEL1, KAS2). A vehicle's pull point is now (0, 1.5, 0) over it, the PS2's camera-collision origin for the soldier's section (`re-aim-assist.md` 10.6); that the vehicles' sections use it is [H] (10.7). Walkers keep their `TrackCenter` pull point.
- `game/tests/suite/seat_camera.rs`: every map's building turrets and pad vehicles of every con mission, each seat, turret looks swept: 130 BF1 and 61 BF2 rows. The hangar rule holds by `walk::tests::seat_camera_under_the_sheet_stays_under_it` and 24 real seats 5 m or more under a sheet.
- Not covered: vehicles are measured at their pad, not driven; a seat with `EyePointOffset` and no `TrackCenter` still takes the port's 0 2 0 default where the PS2 defaults to the eye.
