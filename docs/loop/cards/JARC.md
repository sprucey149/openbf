---
id: JARC
title: The jump's take-off update: the PS2 runs the ground mover with the update's stick and THEN the jump (stick + jump pressed together from a stand land 3.75 m ahead on BF2, 4.59 m on BF1; the port 0.81 / 1.13 m), the first moving update has no gravity (apex 1.861 / 1.912 m, 28 updates; port 1.605 / 1.647 m, 26), BF2 adds the jump velocity to vy and boosts only a forward component, BF1 adds 0.25 x jump velocity x forward stick (+-2.0 m/s, not x1.3)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/soldier.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/sim/state.rs,engine/crates/core/src/collision.rs,engine/crates/core/src/ai/brain.rs
declares: sim; sweep; arena; leak
needs:
effort:
branch: jarc
resume: landed d3ff7dca 2026-10-03
created: 2026-09-29
alpha: yes
domain: physics
---

## Why (re-scoped by AUDIT AIRC, 2026-10-02 — the rule is known now; no measurement is owed)
The maintainer, 2026-10-01: "they could stand still, jump and press forwards on the stick and they'll move forwards in the
air". AIRC made the AIR part of that faithful; the audit found that on the PS2 most of it is the TAKE-OFF update, which
the port does in another order (`analysis/re-stance-roll.md` §4.3b; evidence `lead_tmp/audit-airc/README.md`):

| from a stand, stick held forward | BF2 PS2 | BF2 port | BF1 PS2 | BF1 port |
|---|---|---|---|---|
| stick and jump on the SAME update | 3.75 m (take-off 3.045 m/s) | 0.81 m | 4.59 m (4.810 m/s) | 1.13 m |
| stick one update AFTER the jump | 0.935 m | 0.81 m | 1.213 m | 1.13 m |
| moving updates, apex | 28, 1.861 m | 26, 1.605 m | 28, 1.912 m | 26, 1.647 m |

1. **Order.** The PS2 runs the stance's ground mover with the update's stick, THEN applies the jump to the velocity
   it left (one row: stick, 0.078 m of ground move, stance 4). The port takes the jump before the tick's movement
   (`Soldier::step`: the stance block, then `air_steer`), so a stick that arrives with the jump gets no ground update.
   A bot sets stick and JUMP on one tick (`brain::lost_jump`), so every bot jump from a stand is the 0.81 m case.
2. **The first moving update has no gravity** (the floor bit is still set): BF2 the drag alone (vy 8.0050 → 7.9794),
   BF1 nothing (8.0 → 8.0) and its acceleration × 1 instead of × 0.2. The port subtracts gravity on the jump tick.
3. **BF2 `FUN_00246ed0`**: vy += the jump velocity (not =); × JumpForwardSpeedFactor only when the forward component
   is > 0, else × JumpStrafeSpeedFactor (1.0): a backward jump keeps −5.25 m/s (port −6.80). JumpHeight 1.78 gives
   8.005 m/s = √(2 · 18 · 1.78) (port 8.0, `from_props` does not read it).
4. **BF1 `FUN_00208b78`**: vy = 8.0; v += forward × 8.0 × stick_fwd × 0.25, i.e. +2.0 with the stick forward, −2.0
   back, nothing released — ADDITIVE (6.5 → 8.5, −6.5 → −8.5 live). The port multiplies by 1.3 in both games.

## Why (2026-09-29, the original)
JWAL's PCSX2 traces (`analysis/re-soldier-walls.md` §5.2) measured the BF1 jump on the PS2:
- vy 8.0 at takeoff; the first airborne tick moves the full 8.0/30 with no gravity;
- after that vy falls 0.6006 a tick (g 18);
- the apex is 1.912 m.

The port sets `grounded = false` before its gravity line, so its jump tick already subtracts gravity and its apex is
1.647 m, 0.26 m lower. The forward boost also differs: a 6.5003 m/s run became 8.5002 at takeoff (+2.0, or ×1.3077),
where the port's JumpForwardSpeedFactor ×1.3 gives 8.45. Jump height decides which ledges and railings a player can
reach, and it is the feel of every jump.

## Evidence
- `lead_tmp/jwal/ps2exp/out/t1_east_run_jump_at_wall.csv`, `t3_east_jump_1m_before.csv` (columns ms,x,y,z,vx,vy,vz;
  BF1 KAS2 GCW, the stormtrooper, MaxSpeed 6.5).
- The port: `core/src/soldier.rs:364–383` (the jump), `:454–457` (gravity before the position).

## Notes
- JWAL's ledge fix is independent of this, but its band limits move with the apex: a higher jump reaches ledges
  0.26 m taller.

- **AIRC, 2026-10-02 — BF2 is measured, and a trap is found** (`lead_tmp/airc/ps2/out/`, `analyse.py out bf2|bf1
  <trial>`; `analysis/re-stance-roll.md` §4.3a):
  - BF2 (Mos Eisley, rebel soldier, MaxSpeed 7.0): vy 8.0050 at take-off (= √(2·18·1.78): JumpHeight 1.78), 7.9794 on
    the next update (the air drag alone, NO gravity), then −0.6254 a tick; apex **1.861 m**, 27 airborne updates. The
    boost is 6.998 → 9.0986, × 1.300: BF2's factor is right as it is.
  - BF1 (Yavin Arena, rebel soldier, 6.5): vy 8.0000, 8.0000, 7.3995; apex 1.912 m; boost 6.499 → 8.499 again. The
    second-class measurement (+2.0 or ×1.3077) is still owed.
  - AIRC ported BF2's drag, which also acts on vy: the port's BF2 apex is now 1.57–1.60 m (was 1.647), 0.26–0.29 m
    under the PS2's. On BF2 KAM1's rim probe two stops a standing jump used to clear at 1.65 m are no longer cleared
    (`lead_tmp/airc/rim/`, the y 68.6 level) — this card gives them back.
  - **The trap:** with the take-off tick's gravity skipped (tried in AIRC, apex 1.91), `sim::step`'s
    `no_jump_passes_a_ledge_taller_than_its_reach` fails: a falling soldier's ground probe starts `STEP_HEIGHT` over
    his feet, so he lands on a 2.25 m ledge — and the PS2 does not (JWAL: the 2.26 m lobe top is never reached, feet
    0.35 m short). The apex fix needs the airborne step-up bounded with it.

## AUDIT AIRC note (2026-10-02) — for the builder
- The Acceptance above replaces the planner's 04:30 list (its step 1, a second BF1 class, is answered by the code:
  B1 `FUN_00208b78` B1:118190, and by −6.5 → −8.5 live). Value 4 → 5: the maintainer's own request, done the natural way
  (stick and jump together), is 4.6× / 4.1× short in the port. Reverse: `loopctl.py set JARC value=4`.
- BF2's AI jump is NOT this rule: `FUN_00246ed0`'s controller branch SETS v = dir × MaxSpeed × JumpForwardSpeedFactor
  + (0, vy, 0) when the AI mover has posted a jump direction (ctrl+0x4f0; RE_LOG "BF2 arc `flags`") — a flagged-arc
  jump is a full boosted leap from any speed. The port's bots press JUMP only in `brain::lost_jump`.
- `ai::brain::tests::a_lost_bot_jumps_a_low_wall_from_where_it_stands` will move with the arc, and it is fragile
  today: the bot is NOT at rest when it jumps (10 ticks of slide along the rail at 9 m/s before `lost` confirms, then
  a running jump along the rail) and it comes down on the rail's top 7 cm before the rail and the test hull end; with
  BF2's drag removed it flies off the hull and dies (`lead_tmp/audit-airc/bot_trace_{main,nodrag}.txt`). Start it
  `lost`-confirmed and still, on a hull that goes on past the landing.
- BF1 integrates the frame's own 33.37 ms (vy −0.6006 a tick), BF2 a fixed 1/30 s: the port's 1/30 is BF2's.

## Landed (2026-10-03, main 67c539b + d3ff7dc; report `reports/20261003_0140_card_JARC.md`)
- Acceptance 1, 2, 4 (BF2), 5, 6: met by tests in `core::soldier` / `sim::step`. 3: BF2 3.033 m/s and 3.74 m (PS2
  3.045 / 3.75); BF1 4.333 m/s and 4.12 m (PS2 4.810 / 4.59) - BF1's first ground update, card BF1BK (noted there).
- 7 not met as written: the rim probe's totals are the pre-AIRC ones (standing 9, running 9 over) but at y 68.6 one
  stop reads OVER (dir 12, running), not dirs 4 and 10: those were the 0.6 m air mantle over a 1.77 m rail, which the
  trap's fix (`AIR_STEP` 0.3 m in the wall pass) bounds.
- 8 seeds vs a null: 256 / 256 both, recycles 215 -> 206, lost 3 -> 1. Leak: ceiling 37,858 -> 45,892 (the taller
  jump), push 430 -> 605, floor 5,089 -> 4,914, float 120 -> 121.
- Left: a jump from an airborne stand (BF2's function tests only the stance; its caller is unread), BF2's AI jump branch.
- For the planner: BF2 KAM1 recycles 2 -> 7 over 8 seeds with the change (leg 46 -> 44 at (-12..-18, 60, 108), null
  2 seeds, now 4; (84, 54, 144) on 2) and one seed's stuck_max 11,940. Untraced; `lead_tmp/jarc/sw_null`, `sw_jarc`.

## AUDIT JARC (2026-10-03): PARTLY (`reports/20261003_0615_audit_JARC.md`, `lead_tmp/audit-jarc/README.md`)
- Held: every rule and number above, on 32 new PS2 trials (diagonal, strafe, back diagonal, sprint) and from both
  ELFs' code; JumpHeight's setter read (sqrtf(2 h x 18): exact for every class).
- The KAM1 line above is closed: over 40 seeds recycles go 19 -> 26 at the landing and 15 -> 25 on main, at the
  spots the parent already has (+0.2 a round, 1.2-1.7 SE); no map moves beyond 2 SE.
- "Left: a jump from an airborne stand" is answered: the PS2 takes the press for 0.3 s off the floor, and never
  from a crouch -> card JGATE (value 4). The ledge a jump mounts (PS2 measured, the port more generous) -> card
  JLEDG. A jump from under a deck ends on it up to 2.60 / 2.70 m -> note on VCOL.
