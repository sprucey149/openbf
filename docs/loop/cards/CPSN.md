---
id: CPSN
title: Command post sounds: the hum by the owner's faction (kept by a neutralised post), the charge / discharge / dispute loop with the PS2's pitch ramp, the captured / lost one-shots
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A command post is silent in the port. On the PS2 every owned post hums, a capture is heard as a rising loop, and a
finished capture or loss plays a one-shot at the post. RE: `analysis/re-ambience-music.md` section 4 (8a35a92).

## Evidence
- Hum: the class `AmbientSound` values are `<faction> <sound>` (`all` / `rep` -> `com_blg_commandpost_goodie`,
  `cis` / `imp` -> `com_blg_commandpost_baddie`); the OWNER team's faction entry loops at the post (3-D, gain 0.5,
  6 - 100 m). On a team change the loop stops; for a new owner the new entry starts; a neutralised post plays the
  PREVIOUS owner's again from the next tick; a post that starts neutral is silent. Not friend / foe.
- One capture handle per post: neutral post - `ChargeSound` while exactly one side captures, `DisputeSound` while
  two sides are inside, stopped when empty or captured; owned post - `DischargeSound` while an enemy side is inside
  (alone or contested), stopped when neutralised. Enemy gone with progress left: BF1 stops on the next tick; BF2
  keeps the loop while the progress decays; both start `ChargeSound` for one tick at the "saved" event.
- Pitch, every tick: charge `P x capture / T + (1 - P)`, discharge `P x (1 - neutral / N) + (1 - P)`, P =
  `SoundPitchDev` (disc 0.4: 0.6 .. 1.0); dispute none. Loops: gain 0.3, 12 - 75 m.
- `CapturedSound` / `LostSound`: one-shots at the post on the captured / neutralised event (and when a destructible
  post is killed). `CaptureMusic` / `LostMusic` are authored by no class: nothing to port.

## Do
Bake the post classes' sound keys (the four `AmbientSound` entries by faction, `ChargeSound`, `DischargeSound`,
`DisputeSound`, `CapturedSound`, `LostSound`, `SoundPitchDev`); drive two loop voices per post from the sim's post
state (owner, capture / neutralise progress, who is inside) and the two one-shots from the post events.

## Acceptance
- SOUND_LOG on a headless round of bf1_nab1 and bf2_uta1: one hum loop per owned post, named by the owner's
  faction; 0 for a post that has never been owned; after a neutralisation the old owner's name stays until the
  capture. A test over a scripted post (own -> neutralise -> capture by the other side) checks the three names.
- Pitch test: charge at progress 0, 0.5, 1 -> 0.6, 0.8, 1.0; discharge at neutralise progress 0, 0.5, 1 -> 1.0,
  0.8, 0.6 (P = 0.4 from the baked class).
- Counts: `CapturedSound` starts = captured events, `LostSound` starts = neutralised events of the round.
- PS2 check (`engine/artifacts/lead_tmp/sndc/bf2_uta1_run2.csv`): the CIS listener's own post plays `_baddie`,
  restarted at the clock of each capture / loss line (480.36, 515.23, 535.38) - the port's log shows the same
  name for a CIS-owned post.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Notes
- 2026-10-13 06:42 (lane 2, card CPBAR's RE): confirmed on today's main that no code names ChargeSound / DischargeSound / DisputeSound - the post is silent. CPBAR's build adds a read-only `Sim` helper for the units inside a post per team (`core/src/sim/round.rs`); the dispute loop (a NEUTRAL post with two sides inside) can take its condition from it. Live rows of both meters for the pitch ramp: `engine/artifacts/lead_tmp/cpbar/ps2/out/*.csv`.
