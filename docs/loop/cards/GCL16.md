---
id: GCL16
title: Game findings of cleanse 16: the seat leaves the animator's yaw / legs from before it, roster's class_anim skips the case-blind resolve, weapon_slot found by name, minimap still posts without the mission, capture credit on the arrival tick, black box panic/F9 write race + stuck events over 8 dropped, headless feed grows without --hud, unit_probe floor_dy off the graph, windowed's two battle starts
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
Cleanse 16 (card CLN11) read the game crate (diff 306c4f0..988b229). Fixed there: the player's handle went stale at the first
respawn. These were found and not fixed (behaviour; each a reviewer's read, a hypothesis until traced on main):
1. low-med: `animator.rs` `Animator::seat` (~1252) resets twist / aim / action but not `yaw` or `legs`; `step` is not called while
   seated, so a gunner who turned the gun ~90 deg and gets out snaps his legs back and plays a turn clip on his first step.
   Fix: `self.yaw = None; self.legs = Legs::default();` in `seat()`. Also the open question: the PS2's blend into and out of the
   seat is unread (the snap is PROVISIONAL, comment in `seat`'s doc).
2. low: `roster.rs` `class_anim` looks weapons up with `db.prop(&s.weapon, ..)` (exact case) where `held_slots` goes through
   `db.resolve` (case-blind, classdb.rs ~142: the discs spell names in different cases): a mis-cased slot gets the rifle set.
3. low: `roster.rs` `ClassRoster::build` re-finds `weapon_slot` by NAME after `primary_weapon` chose it; an earlier slot with the
   same weapon name is recorded instead, and `held_slots` lists the primary twice. Return the chosen slot.
4. low: `minimap::still` builds posts without the mission; `point_light::still_posts` (lights, holograms) applies it: on BF2 DEA1
   a still draws CP6 on the minimap but no hologram. Its fallback box copies `MapStatic::post_bounds`.
5. low: `event_feed.rs` `capture_credit` (~545-560) gives the 5-point capture to a viewer stamped THIS tick;
   `analysis/re-event-messages.md` section 9 (235-236) says a player stamped this frame cannot win. Require `since < tick`.
6. low: `blackbox.rs`: the panic hook's `write_rotating` (~1442) races a running F9 / stuck write thread (~1539) on `last.tmp` and the
   prev rotation (the comment at ~1528 says one writer at a time); stuck events past 8 per frame are dropped while `prev_stuck` is
   still updated (~870-881).
7. low: `main/headless.rs`: `feed.tick` every tick (~532) but `feed.take()` only under `--hud` (~570, 612): `EventFeed::out` grows
   for the whole run without it.
8. low: `unit_probe.rs:37` `g.hub(hub.max(0))`: off the graph (hub -1) floor_dy is measured against hub 0; the doc says NaN only
   without a graph. Check what census reads floor_dy before changing it (the sweep's UNIT_SAMPLES share the row).
9. cleanup: `main/windowed.rs` repeats ~20 battle-start steps in the shell start (~771-835) and the `--map` start (~1089-1150)
   (foley, engines, voice, black box, feed, fx, weather, soldier_vis turrets, music volume): one `begin_battle` helper. Note
   windowed.rs is over the size cap (card SPL8): do it with or after that split.
10. low: `bolt_vis.rs` ~377: the terrain test for a remnant is `< 0.2` m while one bisection of the 0.5 m ray step leaves up to
   0.25 m along the ray (comment corrected in CLN11, value not touched).

## Acceptance
A test per fixed item that fails on main (1: a seated gunner turned 90 deg steps off without a turn clip; 5: a viewer stamped on
the completing tick gets 2 points; 6: two writers do not lose a file; 7: `out` stays bounded); 9 is a pure move, every gate PASS.

## Notes

## Cloud build checkpoint (2026-10-24 13:49 UTC)
Item 6 implemented only in retained, uncommitted `gcl16` at
Rotation now serializes the panic/hotkey writers, with same-thread recursion skipped;
stuck crossings write directly to the bounded event ring rather than an eight-row staging array.
15 blackbox tests, fmt and game-lib Clippy PASS. Concurrent mutex-removal mutant FAILS;
the stuck fixture failed before the fix with 8 of 12 events, passes with 12 and no duplicate edge.
Item 9 is already implemented on main: `battle_resources` (`windowed.rs:897`), called by
shell `:1033` and direct launch `:1264`; introduced in `70b0557`. No refactor redone here.
Items 1-5,7,8,10 remain; Acceptance above is unchanged, full CI/required gates still owed.
No assets, missions, references or graphics device; size-map check exit 1 lists eight missing
asset/reference paths, zero missing source rows/zero stale counts.
Report `reports/20261024_1349_card_GCL16.md`; patch, exact logs and continuation `lead_tmp/gcl16_cloud/`.
No commit, push, landing, MAIN claim or agents.

## GitHub checkpoint recovery (2026-10-24)
