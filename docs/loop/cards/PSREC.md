---
id: PSREC
title: Frame-exact, repeatable PS2 runs: record pad input into PCSX2 so a save state plus the recording replays the SAME run every time (today's XTEST key events are wall-clock timed and cannot be) - and the same recording converts to an openbf InputLog
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A state diff needs both sides fed the same input on the same frame. `pcsx2_session.py` presses keys through XTEST with wall-clock waits (`Down:150 wait:1100`; menus drop presses at less than 1.1 s spacing): two runs of the same script drift by frames, so a PS2 trace taken twice would differ for reasons that have nothing to do with the port. Without a repeatable PS2 run the oracle is noise.

## Evidence
- `engine/artifacts/pcsx2/README.md` "Driving a live session": input is XTEST; "Menu input rule: leave >= 1.1 s".
- PCSX2 2.x has an input-recording feature (Tools > Input Recording; `.p2m` files that start from a save state, `pcsx2-master/pcsx2/Recording/`) [H: confirm the file format and whether it can be started headless / by hotkey from the session, and whether it replays frame-exact for this 2.8 build]. PCSX2 also has PINE (a socket API) and the GDB stub (EE port 10501): check whether a pad can be driven from there instead.
- The port side already takes a per-tick `PadState` (i16 sticks, u32 buttons) as `InputLog` (`core/src/sim/input.rs`).

## Do
1. Establish which mechanism gives a frame-exact pad feed: PCSX2 input recording, a PINE/GDB write to the pad buffer, or a PCSX2 patch. Test: from one save state, play the same recording twice; read the player's position at +600 frames both times; they must be bit-equal (the ps2live reader gives the position).
2. A script `pcsx2_session.py record|replay <name> <file>` (or a documented manual recipe if the feature cannot be scripted), memory-capped scope + `timeout` like every PXC run.
3. A converter `recording -> openbf InputLog` (stick axes to i16, buttons to the port's bit order), so one recording drives both sides. State plainly what does not convert (analog trigger sensitivities, pad frames the port does not read).

## Acceptance
- Two replays from the same state: positions at frames 60, 300, 600 bit-equal, pasted (and, if they are NOT equal, the cause: PCSX2 non-determinism such as RTC/RNG seed from the clock, and the fix or the finding that PS2 runs cannot be made repeatable - in which case say so and recommend stopping PSHASH/FDIFF).
- The converter turns a 600-frame recording into an InputLog that the port replays (SHASH's `game --sim-trace ... --input`); the player's stick motion in the port matches the recording's first 60 frames (paste the numbers).
- A README section in `engine/artifacts/pcsx2/README.md` with the recipe.

## Notes
This card is the make-or-break for the whole oracle: a "not repeatable" finding is a valid, useful result and closes PSHASH/FDIFF as no-go. Independent of SHASH.
