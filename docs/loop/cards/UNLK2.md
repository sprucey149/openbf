---
id: UNLK2
title: The career counter (OURS): the profile counts the player's career kills per lockable class (kills made as that class in a real match, conquest and instant action, bots or not) and saves them with the profile
milestone: M3
value: 3
phase: build
status: done
deps:
files: engine/crates/game/src/profile.rs,engine/crates/game/src/walk.rs,engine/crates/game/src/event_feed.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/src/main/windowed/career.rs,engine/crates/game/tests/suite/main.rs,engine/crates/game/tests/suite/career_class_kills.rs,engine/CRATES.md
declares: ci=+4
needs:
effort:
branch:
resume: 2026-10-27 class-counter implementation and hit/capture follow-up complete; serial workspace CI passed. Local main integration and packed executable rebuild authorized by the user. Historical cloud checkpoint files are absent; native size/map limitations are recorded below.
created: 2026-10-21
public_issue: https://github.com/sprucey149/openbf/issues/19
---
## Why
The maintainer (INBOX 2026-10-21 17:50 item 4b). One number per class, per class not per side.

## Implementation (2026-10-27)
- The user authorized proceeding with the issue's stated scope in this session. #13/#14 implementation is already integrated by PR #21 and 202b57e; Settings, bindings and controls UI are unchanged.
- `Career::class_kills` is a defaulted RON map keyed by the roster's existing lowercase authored class name, independent of a battle's team/class indices. An absent entry means zero. All roster soldier classes accumulate counts, so recording requires no new unlock classification, alias rules or thresholds.
- `WalkSim::step_tick` attributes the increase in the player's existing kill total after spawn/class input and combat have run. Same-life dead shooters remain eligible. The accounting is outside `core::Sim` and its hash. `flush_class_kills` drains each pending batch once; windowed play saves new batches after advancing a frame. `WalkSim::record_battle` drains any remainder and passes aggregate statistics into `Profile::record_round` at completion/abandonment. Profile storage has no dependency on the battle runtime; windowed play and headless tests share the same accounting methods.
- Real inputs: `engine/missions/bf2_uta1_con.ron`, `engine/assets_baked/bf2_uta1`, the merged BF2 common data, and `engine/assets_baked/classes/{bf2_all,bf2_imp,bf2_ingame}.ron`. The test builds the real mission rosters, collision, navigation, posts and ground rules, then uses controlled idle enemy riflemen and authored player weapons. It shortens the final enemy ticket pool to one to bound conquest completion; targets do not respawn. No assets are committed.
- `cargo test -p game --test suite career_class_kills -- --nocapture`: passed 2 tests. The real match earns officer +2 and Wookiee +1, with one pending officer kill carried across respawn/class selection; three-tick frames, a mid-match disk save/reload, repeated draining/saving and actual victory retain exactly those counts and aggregate +3. The synthetic regression covers a player dying before their bolt's kill in the same tick.
- Legacy-profile and disk round-trip coverage live in `profile.rs`. The real-map test reports SKIP if Utapau is absent; its passing local run loaded the real data and printed the +2/+1/victory evidence.

## Validation (2026-10-27, Windows)
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo build --workspace`: passed.
- `cargo test --workspace --lib --bins --tests --example arena`: reached the game integration suite with 225 passed, 1 ignored and 1 failed. The sole failure was the unchanged Mygeeto position-table timing check (0.363 s against 0.300 s); its `/proc` thread CPU probe is unavailable on Windows, so it falls back to wall time under concurrent load. Its exact isolated rerun passed. The game library tests, including legacy-profile defaults and disk persistence, passed before this suite.
- The remaining game texture-tier/walkabout-statistics/arena targets and input, manifest, mungefs, net, pak, render and ui targets passed. Render's 40 GPU integration checks and the walkabout GPU comparison retained their default ignored status; no ignored visual/golden checks were forced. `cargo test --workspace --doc` passed (all crates report zero doctests).
- `git diff --check`: passed. The native Python body of `scripts/size_report.sh --check-map --strict` was run without changing its rules; it fails because Windows backslash paths do not match the helper's slash-based map/waivers, and five pre-existing referenced paths are absent (split recipe, gates file and golden paths). After architecture cleanup, changed source sizes are windowed 1,999, walk 1,265 and profile 403 lines; the touched map rows were updated. This is a validation-tool limitation, not a passing size/map gate.
- Final source review kept one class-attribution path, one pending-counter drain and one shared battle-accounting path. The attribution helper uses early returns; profile serialization no longer depends on `WalkSim`. After cleanup, all-target workspace clippy passed, the game library reported 428 passed / 2 ignored, and both career integration regressions passed again. No code comments were added. The user authorized integration into local main and a rebuild of the packed executable in `dist/`.

## Career hit/capture follow-up (2026-10-27)

The user reported career hits/captures remaining zero after successful combat and captures, then authorized fixing them. The preceding implementation deliberately preserved those existing aggregate omissions: hits were passed as a literal zero, captures were never folded into the profile, and shots came only from the current life's primary weapon state. The investigation and build-matched Coruscant snapshot are recorded in `engine/artifacts/loop/INBOX.md`.

`EventFeed` now owns one `BattleStats` value for shots, hits and captures, read through a data-only snapshot. Shots count the player's `Event::Shot` events, including held weapons and seat guns across lives. Hits count positive-damage contacts with enemy soldiers or enemy/neutral vehicles; friendly/self, zero-damage and other-unit attacks are excluded. A blast or multi-target attack can yield several hits, so this is a damage-contact count rather than a bounded accuracy percentage. Capture/assist counting remains the feed's existing HUD rule; its ordering, eligibility and points are unchanged. No capture-credit algorithm is duplicated or changed, and CAPCR's remaining acceptance remains separate.

The normal finished/abandoned-round flow transfers all three totals into `Profile::record_round` once under the existing `round_recorded` guard. Class-kill autosaving remains incremental. Repeated settings/class saves do not fold round statistics again. The windowed save/log wrapper lives in `main/windowed/career.rs` to keep the parent under the 2,000-line hard limit. `Profile` still has no dependency on the runtime; `WalkSim` consumes the plain `BattleStats` snapshot rather than depending on the HUD feed. No serialized fields or migration are added, and prior progress is retained without guessing missing historical totals.

The real Utapau regression now performs an ordinary simulated post capture as well as combat and class switching. It records +31 shots, +52 damaging contacts, +1 capture and the same +3 kills, then checks profile save/load and repeated tick/save handling. Additional synthetic coverage checks enemy vehicle hits after destruction, friendly/self/zero/other-unit exclusions, and abandoned-round accounting. All three targeted tests pass. Restoring the original zeroed hit/capture inputs makes the real test fail at runtime (saved hits 50 versus expected 102); the original source is restored afterward. Receipt: `engine/artifacts/lead_tmp/career-stats-mutation.log`.

Follow-up validation: `cargo fmt --all -- --check`, workspace all-target clippy with `-D warnings`, workspace build, `cargo test --workspace --lib --bins --tests --example arena -- --test-threads=1` and workspace doctests all pass. The serial run includes 1,039 core tests, 428 game library tests (2 ignored) and 227 game integration tests (1 ignored), including the unchanged Mygeeto timing threshold. Other default ignored graphics/golden checks are not forced. Full log: `engine/artifacts/lead_tmp/career-stats-ci.log`. `git diff --check` passes and no Rust comments were added. Updated source sizes and the new small windowed save module are recorded in `engine/CRATES.md`.
