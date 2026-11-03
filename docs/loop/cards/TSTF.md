---
id: TSTF
title: Four test defects found by TSTX's inventory - a temp-file race between bake classes tests, a loadscreen test that panics instead of skipping, a `return` that ends a terrain test's map loop early, and roster / leaf checks that pass vacuously in a worktree (their data sits in gitignored lead_tmp)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Evidence (`lead_tmp/tstx/test_inventory.md`, "Other problems found"; read-only inventory, NOT yet checked by hand)
1. `bake/src/classes.rs`: three tests write the same per-process temp file; under `cargo test` (threads, one pid) they
   can delete each other's file. Under nextest (one process per test) the race cannot happen, but the fallback path
   still runs them as threads. Give each its own name.
2. `bake/src/loadscreen.rs:160`: no check that the BF2 COMMON file exists - panics where every other data test skips.
3. `mungefs/src/terrain.rs:920` and `:1164`: a `return` that probably should be `continue` - one missing map ends the
   loop and the rest are never tested.
4. Some `game/tests/roster.rs` and `game/src/viewer/leaf.rs` checks read `engine/artifacts/lead_tmp/` data (gitignored):
   in a worktree whose `engine/artifacts` is not linked they pass WITHOUT testing anything. Either move the data they
   need under the tracked tree (small files) or make the absence a printed SKIP and count them.

## Do / Acceptance
Check each claim first (the inventory is a model's read). Fix the real ones; for 3, say how many more maps are now
tested. ci counts may move; nothing fails.

## Checked source findings (2026-10-24, unlanded candidate)
- Shared `bake_or_skip` tags do collide across threaded tests (BF1 ALL is used by three callers; other repeated tags also collide). The helper now adds a process-wide atomic serial to each pid-scoped output. Real concurrent disc bakes remain untested without extracted data.
- BF2 COMMON was unconditionally opened after TAT2. Separate independently gated tests now exercise each available file; intended CI inventory delta +1.
- Both named terrain loops had early returns; the 27-map ALPH census has the same defect. All three now continue with tested/skipped summaries. Full-corpus aggregate assertions still require all 27 inputs. With every input absent, attempted map cases rise 3 -> 33 (30 more); actual additional real maps tested = 0. A synthetic later DEA stub after missing COR is checked (1 tested/2 skipped), and corrupting that later file fails with BadMagic.
- Inventory item 4 was partly stale: leaf and flash census already printed per-file skips. Added counted coverage summaries including missing side prerequisites; invalid leaf vector JSON/read errors no longer masquerade as missing data. Original RAM/census files remain in ignored evidence locations; no replacement reference data invented or tracked.
- Production prefixes of all four source modules are byte-identical to 3d487d9; roster is integration-test code. Source-map counts refreshed in the isolated checkout. Full acceptance and landing are still pending; see resume/report.

## GitHub checkpoint recovery (2026-10-24)
