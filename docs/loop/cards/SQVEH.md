---
id: SQVEH
title: Vehicle-seat squad orders the BF1 way: PILE IN / HOP IN (AI on foot within 30 m board the issuer's vehicle), EVERYONE OUT / BAIL OUT (its AI seats leave, 20 s before they may board again), a gunner's ALL CLEAR / HOLD HER STEADY, and followers boarding the vehicle their leader boards
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
SQACK's RE (`analysis/re-squad-orders.md` §1, §2, §4.4) found that a SEATED BF1 player has a second set of orders on
the same D-pad, and that a follower boards the vehicle its leader boards. The port has vehicles and seats (M2) and no
such orders. Split from SQACK so the on-foot orders land first.

## Evidence (all [V] from the decomp, none run live — that is the card's first step)
- Driver: Right PILE IN (type 4), Left EVERYONE OUT (type 5); Up / Down have no label, no line and no HUD.
  Turret gunner: Up ALL CLEAR (type 0), Down HOLD HER STEADY (type 2), Right HOP IN (4), Left BAIL OUT (5).
  Lines: `<s>_inf_com_chatter_tac_needgunner` (get in), `_moveout` (get out), `_allclear`, `_stop`.
- Type 4: friendly AI on foot within 30 m (3-D), not droidekas, board the order's vehicle (goal 5). Type 5 and the
  gunner's types 0 / 2 go to the AI seats of the issuer's own vehicle: leave + `now + 20 s` before the unit's OWN seat
  search runs again (corrected 2026-09-30, see the PS2 log — an order still boards it) / release / hold. Every taker answers `ai_<s>_inf_com_chatter_tac_yes` as on foot.
- A `UnitFollowAgent` whose leader's character is in a vehicle gets a board goal for that vehicle (B1:163128-163146).
- AI in a seat say the type-4 line themselves with p 0.3 and AI on foot within 30 m then board (§5).

## PS2 log (2026-09-30, lane 3 — the check of `resume:`, DONE; `re-squad-orders.md` §4.4 / §6.1 once committed)
Six scenes, BF1 NAB1 CW, the player in a CIS AAT's pilot seat (2 seats: pilot + 1 gunner), game-clock times after the
press; logs `lead_tmp/sqveh/scenes/{O1,OP4,OP5,L15,P3,FB}/events.csv`, sampler `lead_tmp/sqveh/sv_live.py`, state
`lead_tmp/sqveh/nab1_cis_aat_pilot.p2s`, HUD `lead_tmp/sqveh/hud_sheet.png`.
- HUD: two arms only, EVERYONE OUT (left) | PILE IN (right), up 0.03 s after the press for 2.0 s. Lines 0.03 s after
  the press: `cis_inf_com_chatter_tac_needgunner` (PILE IN), `cis_inf_com_chatter_tac_moveout` (EVERYONE OUT).
- PILE IN, 1 free seat: 4 / 6 / 1 on-foot receivers within 30 m took it (0.07-0.43 s); **exactly 1 seated**, 1.57 /
  1.07 / 1.10 s after the press (the first to think); 3 / 2 / 1 "yes" answers. 0 free seats (2 presses): 4 / 2
  receivers took it, one held the board state for 0.47-0.50 s, **nobody seated**; 3 / 2 answers.
- EVERYONE OUT (6 presses): the AI gunner left 0.07-0.50 s after the press; 1 "yes" (0.53-1.40 s).
- **The 20 s is NOT "may not board again"** (the card's old acceptance line): the take stores game time + 20.000 in
  the character entry +0xd4 (RAM), read only by the AI's OWN seat search (`FUN_00253640`); a PILE IN 15 s after the
  eviction seated the same unit (15.7 s after it left). Unordered, nobody boarded the AAT in 44 s.
- Follower (FB): FOLLOW ME on foot -> 2 followers; the player sat in the AAT 3.0 s later -> one follower FOLLOW ->
  board after 0.03 s, in the gunner seat 1.03 s after; the other follower released after 0.17 s.
- Seat shuffle (1 observation): the player leaves the pilot seat -> the AI gunner takes it and drives off; the player
  gets back in -> he is the pilot, the AI is the gunner again.
- NOT run: the gunner's four orders (a human always got the pilot seat; the free-standing turret try did not seat
  him) and a vehicle with 2+ free seats — build those from the [V] read, label PROVISIONAL.

## Acceptance (rewritten 2026-09-30 from the PS2 log; each a core test with the numbers pasted into the report)
- PILE IN from a pilot, n on-foot friendly AI within 30 m, k free AI seats: exactly min(n, k) end seated, each within
  2.0 s of the press (PS2 1.07-1.57 s, k = 1); k = 0 -> nobody seated and every receiver back in its old goal within
  one think (0.5 s); a unit beyond 30 m or a droideka never takes it.
- EVERYONE OUT: every AI seat of the issuer's vehicle empty within 0.5 s (one think); the unit's own seat search is
  off until now + 20 s; a PILE IN 15 s after the eviction seats that unit again (PS2: 15.7 s after it left).
- A FOLLOW ME follower gets a board goal when its leader sits in a vehicle with a free seat and is seated within 2 s
  (PS2 1.03 s); with no free seat it is released.
- Events by name: `<s>_inf_com_chatter_tac_needgunner` / `_moveout`, the taker's `ai_<s>_inf_com_chatter_tac_yes`;
  HUD cross with the two pilot labels for 2.0 s.

## Acceptance (original text, superseded)
- The PS2 scene of `resume:` logged (who boards, after how long, the lines), added to `re-squad-orders.md` §6.
- Port, same scene as a test: PILE IN with 3 AI on foot within 30 m and k free seats -> min(3, k) board; EVERYONE OUT ->
  every AI seat empties and none boards again within 20 s; a follower boards the vehicle its leader enters. Counts and
  the 20 s equal to the PS2 log; lines by name.

## Notes
- 2026-09-30 (lane 3): the launcher picked this card as a `land` job because it was `parked` with MAIN free — there
  was no branch. The session ran the PS2 check instead and set `status=todo` (build waits on `deps: SQACK`).

- SQACK landed 2026-10-03 (83ea41b): the hooks this card builds on are in `core/src/squad.rs` - `Order` (add the seat
  types), `Sim::give_order` / `take_order` (the pending order taken at the unit's think, the answer event
  `Event::OrderTaken`), `squad_pass` (a seated player's D-pad is skipped there today: `u.mount.is_some()`), and
  `steer`'s Follow branch (the leader's mount is where the board goal goes). Receivers today are AI on foot only; a
  bot in a vehicle takes no order (the PS2's do, with x3 reach). The scene test to extend:
  `game/tests/squad_orders_ps2.rs`.
