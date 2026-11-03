---
id: SHSRE
title: Where does the PS2 keep what SHASH hashes? Feasibility RE before PSHASH: for each field group (unit table with positions and health, game LCG, team tickets and scores, post owners and capture progress, round clock and respawn timers) the BF1 and BF2 RAM address or pointer chain, its type and its tick stamp - and the answer to "does the PS2 sim tick at a fixed rate we can sample at, and where is its frame counter?"
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
PSHASH reads these fields from a running PCSX2; if some group cannot be found, cheaply, the whole oracle is only partly possible and FDIFF must be scoped to what is. Do this before anyone writes the reader. The research note's warning: nobody public has done emulator-oracle diffing for a 3D game with agents, so assume nothing about stable addresses (BF1 AND BF2, per map load: pools may be heap-allocated and move).

## Evidence (already recovered - start here, do not redo)
- BF1 soldiers: health component C found by the static interface pointer `0x532D20` at `C+0xA8`; `[0]` health, `[1]` max, team u32 `C+0xD0`, world matrix `C+0x1F8`, position (feet) `C+0x228`, unique id at `(C-0x8)+0xB4`; mounted flag via `C+0x198` (`engine/artifacts/pcsx2/ps2live.py` header, `units.py`). The pool root pointer is NOT known: `ps2live.BF1.rescan` scans 32 MB for the interface pointer (about 15 ms read, 0.15 s scan).
- BF2: health component is entity `+0x170` (`analysis/re-blaster-damage.md`); no unit enumerator in `ps2live.py` yet (BF1 only).
- The game LCG: BF1 `DAT_0066eed0`, BF2 `DAT_0073d870`, `x*0x19660d+0x3c6ef35f` (`analysis/re-blaster-damage.md` section 3, `re-powerups.md:218`, `re-knockback.md:133`). Verified as an address; NOT yet checked that it is one shared state read by every system (the AI may use its own).
- Post (CP) ownership/capture: functions in `analysis/re-cp-capture.md` and `re-command-walker-post.md` (state fields not tabulated for RAM). Tickets/score and the round clock: `re-tickets-spawn.md`, `re-time-limit.md`, `re-conquest-goal.md` (read for field offsets; not yet located in RAM).
- Camera/matrix pointer `0x522AC8` (BF1). PCSX2 2.8 keeps EE RAM in `/dev/shm/pcsx2_<pid>`; reads via `pread` are 15 ms for 32 MB (`ps2live.Live`).

## Do
1. For each group in SHASH's record, a table row: field, BF1 address/chain, BF2 address/chain, type, how found ([V] with the function line from `re_check.py`, or [D] from a RAM diff of two states), unit/scale.
2. Run it live on two states each of BF1 and BF2 (a map load apart): do the addresses survive a map reload? If a root pointer is needed, find it (static pointer to the unit list / post list / team struct) and record it.
3. Tick: is the PS2 sim stepped at a fixed dt with a countable frame/tick counter in RAM (candidate: the round clock or a frame number the HUD reads)? Without one PSHASH must align by round-clock value. Record the dt and whether it is 1/60 s or 1/30 s - compare with the port's tick (`SHASH` header).
4. Which groups are NOT readable cheaply: name them and say what FDIFF drops.
5. Write `analysis/re-oracle-state.md` (OPUS RE header, four checks, verifier) and the pointer in RE_LOG.

## Acceptance
- `analysis/re-oracle-state.md` with the table above, each row tagged, for BF1 and BF2 (BF2 rows may be "not located" with what was tried); the address-survival result; the tick/frame-counter answer; and a go/no-go per group for PSHASH.
- A one-screen reader snippet (python, under `engine/artifacts/pcsx2/`, using `ps2live.Live`) that prints one BF1 record's raw fields from a live state, output pasted - proof the table is not theory.
- Any absence claim ("no tick counter", "no pool root") carries a `re_check.py absent` receipt.

## Notes
RE job (max effort). Memory-capped scopes for any PCSX2 session as for every PXC card (see `engine/artifacts/pcsx2/README.md`, Concurrency). BF1 first, BF2 only if BF1 is a go.
