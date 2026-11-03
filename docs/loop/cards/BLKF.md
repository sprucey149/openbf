---
id: BLKF
title: The black box tells the truth where the bot programme looks: ask the stop along the direction the steering really walks (holders, boarders, lost bots; today 38-44 % right vs 98 % on route legs, and 712 phantom stops on standing bots), put the hold goal / spot / strafe side in the row, stamp the BUILD's commit (the shared binary the maintainer ran now claims b1cffcc without GREN), flag going-nowhere trails in TROUBLE (the maintainer's stutterer #30 is missing), end a dead unit's trail at its death
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
AUDIT BLKB (2026-10-02, PARTLY; `loop/audits.md`, evidence `engine/artifacts/lead_tmp/audit-blkb/`, README first).
The black box records the sim exactly: frames, trails, kills and spawns match a per-tick observer, and it changes
nothing. But four things it says are wrong where the bot programme looks, and the card's own words ("what shuts their
way and which map body it is", "a snapshot belongs to the commit in its header") send a session to the wrong answer:
1. **The stop and body are asked toward the route's corner or waypoint**, not where the steering walks
   (`unit_probe::ask_stop`, `brain.rs` goal chain: lost return > hazard flee > hold goal > boarding > walk point /
   corner). Right on route legs; wrong for holders (JITR's subject), boarders and lost bots walking back. It also
   prints a stop for bots that are standing still. The hold itself is only `hold` 0/1/2: the goal, spot,
   strafe side and end tick are not in the row, so the maintainer's stutterer (#30) cannot be told apart from a bot walking its leg.
2. **The header's commit is `git rev-parse HEAD` when the battle starts**, and `dirty` describes the work tree. Neither
   describes the binary. Main's shared `engine/target/release/game` (the binary the maintainer ran; built 15:30:57 at 20990b1)
   now stamps "b1cffcc, not dirty", although it has no GREN code. The reader does not print `build_time`.
3. **The digest's TROUBLE section misses a stutterer that steers `fwd`.** The going-nowhere test
   (path > 3 m, net < 1/4) runs only on the focus unit's trail. The maintainer's own F9 digest leaves out #30, #17 and #6.
4. **A dead unit's trail ends at its death**, up to 13 s before `trail_tick`, while the legend and the digest
   say every trail ends at `trail_tick`.

## Evidence (main b1cffcc; observer `lead_tmp/audit-blkb/scratch_observer.patch`; neutral: same hashes as main)
- `tools/stops.py inv/au/*.stops.csv` (KAM1 + HOT1, seeds 60 / 7, 9000 ticks, 63,391 samples). Rows where the
  steering went round something: the black box names the steering's own stop and body in 98.3 % on route legs
  (8,966 / 9,124) and 97.5 % while side-stepping. For holders it is 37.7 % (52 / 138), boarding 37.5 % (72 / 192)
  and lost-return 43.8 % (28 / 64). Its direction is off by a median of 95°, 71° and 29° for those three. Of 7,034
  samples where the steering was not asked, 712 carry a stop and 578 of those name a body (KAM1 defender #9 standing
  by its spot: "STOP Wall at 0.00 m body 289 kam_prop_crate611").
- `win/B`: header `commit b1cffcc969c8 dirty false`, `build_time` 15:30:57. `names.weapons` has 13 entries; a real
  b1cffcc build (`win/A`) has 20 (GREN's pistols, thermal detonators, trishot, mortar).
- `playtest_f9_digest.txt`: TROUBLE lists #4, #22, #27, #18, #7, #19, #26. The trails say #30 (46.1 m walked, 3.1 m net,
  26 reversals), #17 (16.9 / 1.3) and #6 (66.2 / 15.3) went nowhere.
- `tools/compare.py`: 14 trails of dead units across 5 snapshots (1-4 each), ending up to 13.0 s before `trail_tick`.

## Notes
- Deps JITR: it reworks the holders this card exposes; take the hold fields from what JITR lands.
- Fine as it is: the kill's weapon is the killer's held one. That is wrong only for a mounted killer or the human's
  grenade: 0 of 106 sampled kills, and bots never press SECONDARY_FIRE. Names can be ambiguous: on BF1 HOT1, 160 of
  395 bodies share a label with another instance (empty names), so the reader should print the body's position too.
- PERF note: the recorder is on by default (dev mode). On KAM1 one unit row costs a 116-138 µs median/mean under load
  (13-36 µs on HOT1 / END1). The 0.5 s frame tick is 2.0 ms at ~30 units quiet and 10.5 ms max under load 65.
  Estimate [D] from ~65 µs per unit when the machine is quiet: about 8 ms every 0.5 s at 128 units and about 65 ms
  at 1,000. PERF must measure with `OPENBF_BLACKBOX=0`, or make the frame cheaper.

## Planner 2026-10-03 03:35 - value 4 -> 5
The top of the board is the bot build chain, and all of it waits on the maintainer's read of BOTD. This card is the instrument
that chain and every reading of the maintainer's F9 snapshots rest on (root `CLAUDE.md` sends each session to it), and audit
BLKB found it wrong exactly where the bot cards look (holders, boarders, lost bots: 38-44 % right). It edits
`ai/brain.rs` and `ai/state.rs`, which SPL11 splits as the chain's first step - so it lands BEFORE the maintainer's go, not
beside the split. Keep the brain edit to the probe's read-only hook (no rule changes: the bot rules wait for BOTD).
Undo: `loopctl.py set BLKF value=4`.

## Acceptance run 2026-10-03 (branch blkf, ad5ab8f + 2306d05; evidence `engine/artifacts/lead_tmp/blkf/`)
- `stops.py inv/au/*.stops.csv` (the audit's observer re-applied as scratch, KAM1 + HOT1, seeds 60 / 7, 9000 ticks,
  57,282 samples; `stops_after.txt`): blocked rows naming the steering's own stop and body: holding 95 / 95,
  boarding 173 / 173, lost-return 44 / 44, side-step 1,759 / 1,759, leg sliding 7,143 / 7,143 (100 % each; before
  38 / 38 / 44 / 98 %). Not-asked samples carrying a stop: 0 of 8,288 (before 712 of 7,034). The probe now reads the
  same direction the observer does, so this checks the wiring (every walking state stores it), not two derivations.
- Recorder + observer on vs `OPENBF_BLACKBOX=0`: stdout byte-identical, both seeds (`inv/on_s*.txt` = `inv/off_s*.txt`).
- `win/B` (windowed under Xvfb, a release binary built at ad5ab8f run from the checkout at 2306d05): header
  `build: commit ad5ab8f39961`, `checkout HEAD 2306d05b4e04`, `WARNING: the binary (ad5ab8f39961) is not the
  checkout's HEAD (2306d05b4e04)` (`win/B/digest.txt`).
- `blackbox_read lead_tmp/jitr/snapshot_20261002_kam1_f9.json` (schema 1, still reads; `playtest_f9_digest.txt`): going
  nowhere lists #30 (46.1 m / 3.1 m net, 26 reversals), #17 (16.9 / 1.3, 5), #3, #4, #6 (66.2 / 15.3, 1).
- `compare.py` on the 4 snapshots: 7 dead units' trails, each `trails[].tick` equal to the truth's last tick (3878,
  3878, 3435, 3908, 3945, 3795, 2775); 0 alive trails off `trail_tick`; frames, kills, spawns equal the truth.
- Tests: `a_dead_units_trail_ends_at_its_death` (mutation-checked: fails "150 vs 60" with the header's tick),
  `a_schema_one_file_still_reads`.
