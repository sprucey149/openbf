---
id: AIRC
title: A jump steers in the air: a standing jump with the stick forward moves forward (the maintainer, 2026-10-01: 'that is what the original games do and it is better'), for the player and for bots - measure the PS2 on open floor first, then build
milestone: M3
value: 5
phase: build
status: done
deps: BOWL
files: engine/crates/core/src/soldier.rs,engine/crates/core/src/ai/brain.rs
declares: sweep; arena; leak
needs:
effort:
branch: airc
resume: landed 8c34344e 2026-10-02
created: 2026-10-01
first: 2
alpha: yes
domain: physics
---

## Why
The maintainer, 2026-10-01 22:00: "let the unit still go forwards after airborne. They don't have to be walking forwards before
jumping to jump forwards. They could stand still, jump and press forwards on the stick and they'll move forwards in the
air. Bots can do that too. That is what the original games treat it and it is better." This is the call RAIL
(c36c8a2) left to him ("the maintainer's call if he wants more air control than the PS2").

## What the port does now (`core/src/soldier.rs`, the movement block)
In `Stance::Jump` / `Fall` the stick's TARGET speed is `MaxSpeed x ControlSpeed[jump]` = 9 x 0.03 = 0.27 m/s, approached
at `Acceleration x 0.03` = 2.1 m/s2. So a standing jump with the stick forward travels about 0.2 m, and a running jump
with the stick held is braked toward 0.27 m/s.

## The evidence for "the PS2 has no air control" is weaker than the docs say
- `analysis/re-stance-roll.md` 4.3 reads ControlSpeed jump = 0.03 / 0.03 / 0.5 from the class [V], and says the ground
  states take target = input x Max x factor. That the AIR state scales the TARGET by 0.03 (and not only the
  acceleration) is the reading to check: with the target at full speed and the acceleration at 2.1 m/s2, a 0.9 s
  standing jump gains about 1.9 m/s and travels about 0.85 m - which is what the maintainer describes.
- The PCSX2 line everyone quotes, "a jump against the wall goes straight up" (`analysis/re-soldier-walls.md` section 5:
  "pressed against the wall with W held, vx reads 0.0000"), was measured AGAINST A WALL. Nobody has measured a
  standing jump with the stick forward on open floor.

## Notes
- Do BOWL (`first: 1`) first: with the rail no longer a one-way valve for bots, air control over rails cannot put more
  of them on the hulls.

## Result (2026-10-02, lane 2) — report `reports/20261002_0355_card_AIRC.md`
- **Measured, both games, open floor** (`lead_tmp/airc/ps2/`, `analysis/re-stance-roll.md` §4.3a): a standing jump
  with the stick forward lands 0.935 m ahead (BF2) / 1.214 m (BF1). BF2 adds the stick as 2.1 m/s² under a drag; BF1
  goes to 0.2 x the walk speed at once and never brakes. The maintainer was right; the "no air control" was a wall artefact.
- **LANDED** main 63975ed + 8c34344 (2026-10-02 03:55): `Soldier::air_steer` per game; `brain::lost_jump` jumps from where
  the bot stands; the leak census settles for 420 ticks.
- **Acceptance:** tests pin the PS2's speeds update for update; port travel 0.810 m (BF2) / 1.127 m (BF1) — short of
  the PS2 by the port's 2 missing airborne updates (JARC); the rail stop at (-11.8, 61.62, 102.2): standing jump no ->
  OVER; bot test passes and fails under two mutations; 8 seeds: 256 -> 254 rounds (KAM1 and YAV1 one last-survivor
  stalemate each; a null variant gives 256) — the "no map drops" bar is not met to the letter, read as noise.
