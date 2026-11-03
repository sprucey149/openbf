---
id: PSHASH
title: The PS2 half of the state diff: read SHASH's fields out of a running PCSX2 every N frames (RAM via ps2live's shm reader, or PINE / the GDB stub on EE port 10501 if the shm read is not frame-synchronous) during a recorded replay, and write a trace in SHASH's format
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The second stream FDIFF compares. Reads must be taken at a KNOWN guest frame: a free-running `pread` of /dev/shm RAM from outside while the emulator runs can tear (positions from frame t, health from t+1). PINE/GDB can pause the VM or take the read at a vsync breakpoint.

## Evidence
- Existing reader: `ps2live.Live` (`pread` of `/dev/shm/pcsx2_<pid>`, 15 ms per 32 MB) and `BF1.soldiers()`; addresses and which fields are readable come from SHSRE (`analysis/re-oracle-state.md`), the recording from PSREC.
- PCSX2 GDB stub: EE on port 10501 (and IOP 10502) per the research note; PINE is the socket API (read memory, pause, frame step) [H: check this 2.8 build exposes both; `pcsx2-master/pcsx2/` has the PINE and GDB sources]. Memory-capped scope + `timeout` for the session as for all PCSX2 work.

## Do
1. Pick the sync mechanism: ideally pause at a vsync, read, resume (PINE or GDB `continue` to a breakpoint on the per-frame function from SHSRE); a plain periodic shm read is acceptable only if a torn-read test shows none (read the same group twice in the same pause window and compare, 1000 times).
2. `pstrace.py <session> <recording> --every N --out trace` writes SHASH's exact format (same field order, same quanta read from SHASH's constants), with `--full` giving the raw tuples.
3. Align ticks: PS2 frame counter / round clock to the port's tick number (SHSRE item 3) in the header.

## Acceptance
- A trace of >= 600 frames sampled every frame (or every 5 if per-frame is too slow; say which) from a save state + the PSREC recording, with the header filled; running it twice gives byte-identical traces (the PS2 side is repeatable), pasted diff.
- The torn-read test result.
- Which groups are filled and which are `n/a` (from SHSRE's no-go list) - an `n/a` group hashes to a fixed token so FDIFF skips it.
- No PCSX2 process left behind (`ps aux | grep -iE 'pcsx2|Xvfb'` for the session, pasted).

## Notes
Capture phase; BF1 first. If PSREC says the PS2 is not repeatable, park this and FDIFF with the reason.
