---
id: LVLB
title: The level test calls a ramp behind a barrier OFF the level (BOWL's 0.75 m rule): on BF1 Rhen Var Harbor bots walking down the tunnel go lost and turn back every 1.7 s (lost 13,530 unit-s in 32 rounds; the rule off: 36), a 45 degree ramp on BF2 UTA1 and the Yavin temple walkway read off; and the rule's ON band outside a Kamino rail holds a bot for 323 s
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BOWL (b1a32db3) made `Sim::level_at` allow only `RIM_DROP` 0.75 m under the floor where a walk from a hub was stopped
by a barrier: "past a barrier and lower = a hull". That keeps bots off BF2 Kamino's and Mustafar's hulls (KAM1 hull
share 2.0 % -> 0.12 % on main, PS2 0.01-0.26 %). But a barrier is not always a rim. Where an obstacle or a side barrier
stands between a hub's centre and a RAMP, the ramp reads OFF the level, a bot on its own arc goes lost and turns back.
Found by AUDIT BOWL (2026-10-02); everything in `engine/artifacts/lead_tmp/audit-bowl/` (README first).

What the player sees: on BF1 Rhen Var Harbor, bots walking down the ice tunnel turn round every 1.7 s for up to 2.5
minutes; the team that comes down it wins 3 rounds of 32 where it won 7. On BF2 Kamino, now and then one bot slides
along the outside of a rail for minutes.

## Evidence (main 69ba267 against the same tree with `RIM_DROP = 2.0`; 32 seeds unless said)
1. **BF1 RHN1, the tunnel** (arc hub 11 <-> hub 100, floors 8.14 -> 1.15). Barrier 173, a 3.6 x 3.2 m obstacle at
   (-159.0, 36.8), stands on the arc. From hub 11 the ramp behind it is 0.9-1.9 m under the stop; hub 100, whose walk is
   clear, is the 13th to 7th nearest (`level_at` asks 6). `LEVEL_PROFILE=-166,35.9,-136,35.7,9.5,1`: 7 rows Off
   (x -156 .. -150). Lost unit-seconds 36 -> 13,530; stays in the box x -168..-138, z 24..48 with 3 or more off-level
   samples 0 -> 877 of 2,965, p90 8 -> 21 s, longest 29 -> 151 s; recycles 1 -> 21; team 0 wins 7 -> 3.
   One variable: with barrier 173 opened and the rule kept, 0 such stays (8 seeds). Trace: `rhn1_trace11_post.err`.
2. **BF2 UTA1** (showcase): the 45 degree ramp at (-144, 12.6, 120), mostly INSIDE hub 61's own disc (floor 14.40):
   off-level unit-s 24 -> 611, on 28 seeds; the map's lost unit-s 1,660 -> 2,454.
3. **BF1 + BF2 YAV1**, the temple walkway (x 96..126, z -162..-138): off-level-on-graph unit-s 1,668 -> 3,824 and
   1,357 -> 3,162. No dwell cost measured; bots walk it flagged lost.
4. **The ON band** (the other side of the same constant): the first metre of hull, 0.3-0.75 m under the deck, reads ON,
   and the rail's lip is 1.0 m over it. Seed 23, BF2 KAM1: unit 4 gen 6 slides outside cp3's rail at (-73, 59.8, 325.7)
   for 335 samples (ticks 18,120-28,290), 327 of them read ON; `shares` longest 323 s. BOWL's bar was 30 s.
5. All 32 maps, seeds 1-8: lost unit-seconds 3,055 without the rule, 7,844 with it; off-level-on-graph 2,512 -> 7,270.
6. TAT1 (showcase): see TPKT's note — a second stall spot at (-69, 19, 51) is this rule's (recycles 0 -> 21).

## Acceptance (tools in `lead_tmp/audit-bowl/tools`; `offlevel.py` and `episodes.py` read main's own `UNIT_SAMPLES`)
- `LEVEL_PROFILE=-166,35.9,-136,35.7,9.5,1 STUCK_CENSUS=1 round_to_outcome 0 1 bf1_rhn1`: no `Off` row (main: 7).
- 8 seeds x 60,000 ticks with `UNIT_SAMPLES`:
  - `episodes.py <dir> bf1_rhn1 -168 -138 24 48`: 0 stays with 3 or more off-level samples (main 212), p90 <= 8 s
    (main 21), longest <= 30 s (main 130); bf1_rhn1 recycles <= 1 (main 5).
  - `offlevel.py <main's dir> <dir>`: off-level-on-graph over the 32 maps <= 3,000 unit-s (main 7,270; the rule off
    2,512); bf1_rhn1 <= 50 (main 3,373), bf2_uta1 <= 200 (342), bf1_yav1 <= 450 (1,001), bf2_yav1 <= 450 (727).
  - BOWL's gain is kept: `shares` rim bf2_kam1 <= 0.2 % (main 0.10 %; the rule off 2.02 %), bf2_mus1 <= 0.2 %
    (0.10 %; 4.89 %); no rim stay over 30 s on bf2_kam1 over seeds 1-32 (main: 323 s on seed 23).
  - No map's finish count drops; lost deaths and recycles per map not above the same run's baseline.
- Two unit tests that fail on main: a bot walks an arc DOWN a ramp past an obstacle barrier without going lost; a bot
  0.5 m under the deck outside a rail with a 0.5 m lip is back on the deck within 10 s.

## Notes
- `alpha: yes` set by the audit (the planner may reverse): two showcase maps are in it — BF2 UTA1's ramp (item 2) and
  TAT1's second stall spot (item 6).
- `files:` is `sim/bounds.rs` alone on purpose (the `first:` cards share `ai/brain.rs`; an overlap would hold them
  up): put the two tests in `bounds.rs`'s test module on the `rim_sim` fixture, and keep observers in scratch examples.
- Untested directions, the builder's call: a point inside a hub's disc or an arc's corridor and within `BOUND_DROP` of
  that arc's floor line is on the level whatever barrier a straight walk meets; or the walk goes round an obstacle as
  the leg planner does (`NavGraph::leg_crosses_barrier` / the corner path). For the ON band: lost when beyond a
  barrier from every near hub AND under the deck, however little, or let a bot there jump the lip.
- Do not retune `RIM_DROP` for this: 0.5 and 1.0 were swept at the landing (`lead_tmp/bowl/README.md`).
- The census `rim` is a hull measure on KAM1 only (on other maps it counts ramps and rooms behind barriers); judge
  other maps by off-level-on-graph and lost unit-seconds.
- PS2 soldiers stand at two spots the rule newly calls off (BF2 DEA1 (-185.2, 50.8, -253.8), KAM1 (33.9, 78.41, -86.8));
  the port's bots do not go there today. `lead_tmp/audit-bowl/out/ps2ref.txt`.
- 2026-10-16 AUDIT RETW: card RETW's floor walk (d5ae05f4) more than DOUBLED this card's Rhen Var spot: confirmed-lost time in the
  12 m cell (-156, 36) of bf1_rhn1, seeds 1-8, 367 -> 724.5 s (the map 676.5 -> 1535 s; today's main the same, main with RETW
  switched off = the parent), lost episodes over 20 s 7 -> 46, 0 deaths / 0 recycles either way. Trace: landing seed 2 unit 31,
  ticks 9120-11460 (`lead_tmp/audit-retw/wf/self/rhn_land/bf1_rhn1_s2.err`): Off(0.8-2.5) at x -148..-155 on leg 99 -> 100, the
  walk back to x -157.5 (On), the leg east again, 78 s. The flicker as a class (also BF2 Utapau's pit) is card LFLK; re-measure
  this card's bars on today's main before building (its main numbers predate RETW).
