---
id: AIMA
title: The PS2's default shooting angle: the shot leaves above the camera axis (RETC measured the reticle ~5 degrees above the view centre), so the camera can look down without the bullets going into the ground - RE the aim ray, then match it in the sim and put the HUD reticle at that height
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-02: "the original ps2 games had the marker for where the bullet goes higher up, so the camera could be
angled up higher and looking down without the bullets going straight into the ground. Look into the ps2 default
shooting angle, and adjust the hud marker accordingly. It should be higher up."
The RE is done (`analysis/re-aim-assist.md` §10, [O] UNAUDITED, 2026-10-02). On the PS2 the chase camera looks
`TiltValue` BELOW the aim (BF1 5 deg, BF2 10 deg); the aim ray leaves the camera's own position along the aim; the
reticle sits where that ray goes on screen (line 178.5 of 480 on BF1, 153.35 on BF2; the frame's centre is 240); the shot
leaves a fire point 1.58 m up and flies at the first thing the ray hits 5.5-65.5 m out. The port's camera looks along the
aim, its reticle is on the window centre and its shot leaves 1.425 m up along the aim.

## Evidence
- `analysis/re-aim-assist.md` §10: the rule (10.2), the section data per game and stance (10.1), the pitch (10.3), the
  reticle (10.4), the measured shots (10.5), the drawn camera and its collision (10.6), the change list (10.8).
- `engine/artifacts/lead_tmp/aima/`: `model_bf1.py` / `model_bf2.py` (the rule re-run on a save state: 132 states to
  0.5 mm), `bolts.py`, `probe_live.py` + `live/*.json` (44 single shots at 7 pitches per game), `stored_reticle.py`,
  `frames/ps2_reticle_sheet.png` and `frames/ps2_live_pitch_sheet.png` (the PS2 side of the frame pair).

## Notes
- The rule is the same for both games; only the class data differs. PitchLimits (-0.8, 1.1) and prone +-0.4 are
  already the port's.
- Not in this card: the fire point for bots (FPNT); the camera's smoothing (AimTension 30 on BF1, MoveTension 15), its
  collision rule (§10.6: five rays from feet + 1.5 m, pull in at once, recover 0.75 / s) and the zoom sections (tilt
  3.5, offset (0.4, 0.05, 2.8)) - notes on CAMS; the reticle's colours and size (RETC).
- A choice to record in FEEL_NOTES: the PS2's blind spot (a wall or the ground nearer than 5.5 / 5.2 m along the
  reticle's ray is not seen, and the shot then lands 47-64 lines under the reticle). Faithful is the default; a modern
  alternative (start the ray at the fire point's distance) is one constant - say which was taken.

---
## The card as filed (2026-10-02), kept

## Why
The maintainer, 2026-10-02: "the hud is coming along, but the original ps2 games had the marker for where the bullet goes higher up,
so the camera could be angled up higher and looking down without the bullets going straight into the ground. Look into
the ps2 default shooting angle, and adjust the hud marker accordingly. It should be higher up."
In the port the reticle sits at the window centre and the shot goes along the camera axis (check this first), so a
third-person camera that looks down at the player's back aims into the ground.

## Evidence already in the repo
- `RETC` (this loop): "ps2live measured the crosshair at screen (320, ~178) of 640x480, ~5 degrees above the camera axis"
  (the 480-line frame's centre is 240, so the reticle is 62 px above it). `analysis/re-aim-assist.md` section 3.7 has the
  pick loop and the reticle colours.
- AIMF landed the PS2's filtered cone acquisition (`Sim::reticle_pick`).

## Notes
- Related: RETC (reticle look), AIMF (acquisition), BHUD2 (HUD tree). Decoration vs sim: the sim change moves hashes
  where a human soldier's shots are in a sweep - the AI arena is unaffected by a human-only rule; say so with numbers.

- Planner 2026-10-02 14:45: value 4 -> 5 (the maintainer's INBOX note of 10-02: OBST and AIMA, value 4-5, after the `first:` list, which is empty since STEND landed). `files:` emptied while the phase is `re` (a claimed card's files count as busy: with `sim/step.rs`, `weapon.rs` and `ui/src/hud.rs` listed this RE would have held GREN, JARC, PWUP, SQACK and BHUD3 for its whole run). Restore at the build: `engine/crates/core/src/sim/input.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/weapon.rs,engine/crates/game/src/walk.rs,engine/crates/ui/src/hud_draw.rs,engine/crates/ui/src/hud.rs`. RETC (value 3) keeps the reticle's colours and size; its place is this card's. Undo: `loopctl.py set AIMA value=4`.
