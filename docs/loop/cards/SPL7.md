---
id: SPL7
title: The files still over the 1,500-line cap — `core/src/weapon.rs` (1,624), `core/src/commander.rs` (2,339), `core/src/sim/vehicles.rs` (1,938)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why / Evidence / Acceptance
Acceptance (planner 2026-09-30): `scripts/size_report.sh --check-map` lists none of the four files over 1,500 lines; every gate PASS with nothing declared (behaviour-neutral); CRATES.md rows current. The original entry below has the files.

## Original QUEUE.md entry (verbatim, 2026-09-25)
SPL7. [TODO — **NOTE 2026-09-24 (SPL6's landing): SPL1–SPL6 have all LANDED, so SPL7 is FREE; read SPLIT_RECIPE §10–§15 and copy `lead_tmp/split-render/{gen_split.py,trial_stages.sh,stage_commit.sh,regen_check.sh}` (`stage_commit.sh` commits each stage only at ci exit 0 and fills the message from the measured numbers); the `core` files carry the FULL sim set, `bake/src/map.rs` the scratch bake.** ADDED 2026-09-23 at the maintainer's request; LOW value; after CMAP and after SPL1–SPL6 have landed; one file per session — mark it IN PROGRESS with a `RESUME FROM:` line naming the next file] **The four files just over the 1,500-line cap — `core/src/nav.rs` (1,693), `core/src/weapon.rs` (1,665), `bake/src/map.rs` (1,598), `core/src/commander.rs` (1,618 since HDL): split where a real seam exists, otherwise waive** [no gate value may move] — **Recipe: `engine/artifacts/briefs/SPLIT_RECIPE.md`.** A file is split only if it has a seam that yields at least two parts of 400+ lines; otherwise add it to `scripts/size_waivers.txt` with a one-line reason — a waiver is a legitimate outcome, not a failure. The `core` files carry the FULL sim set, all unmoved (arenas included); `bake/src/map.rs` additionally needs `scripts/scratch_bake.sh` + `scripts/cmp_bake.sh` at DIFF=0 against the shipped tree. GATE: per file, as its recipe class. Files: the four above, their new module directories, `scripts/size_waivers.txt`.

## Note (cleanse CLNM2, 2026-10-05)
`core/src/sim/vehicles.rs` is 1,526 lines (over the 1,500 cap, under the 2,000 strict one) with no split card of its own: it joins this card's list. `core/src/commander.rs` is at exactly 2,000 - one more line fails `size_report.sh --strict` (it has no waiver). `game/src/blackbox.rs` (1,586) got a waiver instead: dev scaffolding kept as one module to delete.

## Note (cleanse CLNF, 2026-10-06)
`core/src/commander.rs` is 2,004 lines now (MOVR): `scripts/size_report.sh --strict` exits 1 on it (the one unwaived file over 2,000). No gate runs `--strict` (gates.py's `map` step is `--check-map`), so nothing is blocked; this card's split of it is what clears it. `core/src/weapon.rs` is back over the cap at 1,518 (SPRD).

## Commander extraction (2026-10-26)
SPLCMD owns only the trailing commander regression module extraction. The
projected 1,081/1,419-line files meet the size bar without runtime edits.
This dependency serializes shared ownership; it does not complete SPL7 or
waive its remaining map split, scratch bake or unchanged behavior gates.

## Independent remaining split (2026-10-26)
SPLMAP owns the remaining source extraction as a separate card. This preserves
all original acceptance, scratch-bake equality and prior validation debt.

## Cleanse note (2026-10-29)
`wc -l`: core/src/weapon.rs 1,624, core/src/commander.rs 2,339, core/src/sim/vehicles.rs 1,938 (from the CLNM2 note) are over the cap. Under it: core/src/nav.rs 1,485, bake/src/map.rs 902. commander.rs was being trimmed by the same cleanse, so its count may drop.
