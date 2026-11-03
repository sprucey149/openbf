---
id: SPLSND
title: Extract sound playback from sounds.rs into a private module while preserving lookup, mixing, APIs and every sound rule
status: archived
issue: 6
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #6](https://github.com/sprucey149/openbf/issues/6).

## Why and evidence
At the source parent, SPL8's `sounds.rs` had 1,864 lines and combined
bank/stream lookup, loop voices and mixer instrumentation with
prepared voice playback, one-shots, cries, stations and layer scheduling.
The latter responsibility can move without changing any rule or caller.
This isolates that existing SPL8 requirement; windowed.rs and every original
parent gate remain outstanding. The assets slice is already validated.

Trial baseline 7ddd8bd27d6271514f1e43c0f2dd25d0e854389a:
- Root 1,864 -> 1,328 lines; private `sounds/playback.rs` 549 lines after fmt.
- Canonical helper: 1,474 old/1,474 new meaningful lines, five visibility-only
  changes, zero new-only lines, exit 0/PURE MOVE. Changing one moved pitch
  literal is rejected with exit 1. Formatted regeneration is byte-identical.
- Existing sound tests keep their names/bodies: 32 pass on parent and trial,
  two existing bank/device cases ignored. This is source/fixture evidence.
- Durable trial generator: reports/20261026_0939_split_sounds.py. It asserts
  exact baseline anchors and imports; no product edit is implemented by planning.

## Exact scope
Move the existing `SoundLib` methods from sound/decode caching through waiting
into a private playback child, except `mix` and `note`, which remain with mixer
handoff and instrumentation in the root. Root also retains shared types/fields,
loading, lookup, attenuation/bus/pan laws, loops/ambience and the existing tests.
Inherent methods keep every `SoundLib` API path; no caller edit or re-export of
new API is needed. The five forced `pub(super)` methods are sound,
prepare_voice, prepare_take, voice_secs and start_prop_with. No public widening.
Keeping mix/note in the root avoids the signature-width/rustfmt proof trap.

Use explicit imports, preserve original body text and comments, allocation,
iteration/draw order, timing, rate floor, voice lock, cry/station ownership and
recording. No cleanup, behavior fix, new abstraction, performance claim, new
test, removed test or renamed test is part of the extraction. Preserve every
GSND/VOICEP/WHUM/UCRY/UVO2 rule and pending check. References use symbols after
this move; old line-number evidence is retained as history, not silently edited.
The coordinator updates CRATES and any stale entry-point references with the
reviewed integration. Remove only sounds.rs's existing SPL8 line from scripts/size_waivers.txt
when the extraction is implemented; preserve all other waivers.

## Acceptance
1. Generate from the actual source parent, format normally, and require:
   `python3 scripts/tools/pure_move_check.py <parent>:engine/crates/game/src/sounds.rs engine/crates/game/src/sounds.rs engine/crates/game/src/sounds/playback.rs`
   prints PURE MOVE, exit 0, no edited/lost lines and no new meaningful code.
   Only the five stated parent-visibility changes are allowed. Regenerate
   byte-identically after fmt; an independent moved-line edit fails the proof.
   Both files <1,500 lines, every public symbol/path and test identity unchanged.
2. Parent/candidate identical `cargo test -p game --lib -- --list` and focused
   sounds test list. Full game library, focused sounds tests, workspace build,
   fmt/all-target Clippy, `cargo doc -p game --no-deps` pass with no added doc
   warnings. Preserve existing ignored flags; report missing-data early returns
   explicitly. No copied historical pass and no data skip earns acceptance.
3. `scripts/size_report.sh --check-map --strict` and normal CI/map/protect gates
   pass. All original SPL8 applicable GPU/golden/sim/sweep/arena/leak/bake/parity/
   behavior gates pass unchanged, no declarations or rebaselines. Match paired
   real-content parent/candidate sound event/property/take/gain/pitch/ear/duration
   rows for BF1/BF2 voices, layers, cries, stations and engine/ambient controls.
   Real-bank and explicitly configured device tests execute with nonzero cases;
   keep missing-bank/probability/interval/lock controls and view/listen to the
   ordinary paired scenes. Zero new sound, hash, golden or output differences.
4. Report exact parent/source/check SHAs, generator/proof, file counts, test
   counts, warning-set comparison, raw content checks and normal gate receipts.
   Full parent SPL8 is not done until its other source and gate requirements
   finish. Preserve current approvals, thresholds and goldens.
