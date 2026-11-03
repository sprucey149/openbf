---
id: WAI2
title: Bots reach and keep walkers as the PS2 does: a claimer walks to its vehicle over the graph (the snow AT-ST is claimed and never reached), comes 75 m for one, the AT-TE routes, and a tauntaun rides to the post (PS2 282-711 m a ride; port 158-203)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Measured (2026-10-02, `reports/20261002_0520_card_WAI2.md`; PS2 side in `analysis/re-walker-ai.md` section 13)
**PS2:** a bot pilots every AT-AT and AT-TE from the first seconds and stays in (BF2 HOT1 both AT-ATs from 17-24 s to
the end of a 240 s run, 414-452 m; BF2 GEO1's AT-TE from 18.5 s; BF1 HOT1 both from 12-14 s with a gunner). Its pilot
SPAWNS 8-9 m from it: the command walker is a spawn post (card CWSP). Bots come 8-71 m for a walker (once 146 m).
**Port, which test refuses** (`lead_tmp/wai2/census2/*.txt`, 4 seeds, thinks of a bot on foot per live walker):
| class | map | refused by | numbers (seed 42) |
|---|---|---|---|
| `imp_walk_atat` | BF1 HOT1 | nobody near: no bot ever within 168 m | `far:over_40m` 24724 of 24724; the route would pass (gain 625-667 m) |
| `imp_walk_atat` | BF2 HOT1 | nobody near (205-214 m) AND the route | `route:gain_under_40` 25237 of 25516, best gain -14 / -24 m |
| `rep_walk_atte` | BF1 GEO1 | the 40 m seek range (nearest bot 52-78 m) | `far:over_40m` 23353 of 23353; gain 238-248 m passes |
| `rep_walk_atte` | BF2 GEO1 | the route (nearest bot 25-29 m) | `route:gain_under_40` 20632 of 20632, best gain 29 m |
| `imp_walk_atst_snow` | both HOT1 | CLAIMED, never reached: 9-12 claims a round end `giveup_20s` | the bot slides on a wall at (18.6,-1.0,16.6), 17.3-18.2 m from the walker at (32.6,-2.7,27.4), reach 8.0 m; the other at (56.2,7.2): 16.6 m |
| `all_walk_tauntaun` | BF1 HOT1 | boarded, but 3/8 at 20-50 s (PS2 5/8, 7/8, 7/8 at 20/30/40 s) | rides 158-203 m, the drop hub is 85.9 m from cp6 (PS2: 282-711 m, ending 4.5-41 m from cp6) |

## Build notes
- Order: (a) walk to the claimed vehicle over the nav graph, or claim only what a straight walk reaches; (b) `BOARD_SEEK`
  40 -> 75 m (PS2 >= 71 m; FEEL_NOTES); (c) the command walker's route (`routes_to` / `post_hub` for its `AISizeType`:
  why the best drop hub is FARTHER from the post than the AT-AT stands); (d) a tauntaun's last leg from the drop hub to
  the post. (e) Not measured to matter: the PS2's 20.2 m look-ahead (re-walker-ai.md section 6).
- Side observation, not carded: the port's BF2 HOT1 fields an `imp_walk_atst` (slots 29 / 33, alternating with a
  tauntaun on the same pad) from ~50 s on every seed; the PS2 run had no such class in 240 s (its walkers: 3 tauntauns,
  2 AT-ATs, 1 `imp_walk_atst_snow`). Check the pad's per-team class before trusting BF2 HOT1's AT-ST count.

## Why (the original card)
Card WAI (2026-10-02) put bots in walkers' pilot seats. Its sweep line (`walkers: ai_piloted at 60 s ...`, 4 seeds,
`lead_tmp/wai/build/final_s*.txt`) leaves three gaps against the PS2 (`reports/20261001_2340_card_WAI.md`):
- **Unarmed mounts are left at once.** BF1 HOT1: 0-2 of 4-5 tauntauns ridden at 60 s (PS2: every one at 40-60 s); a
  rider gets out at its post or drop hub and is blocked from claiming for 30 s. 34-54 rides a round of 107-230 m.
- **Never boarded:** BF2 HOT1's `imp_walk_atst_snow` (0/1 on every seed; the PS2's sampled AT-ST is that class) and
  both `imp_walk_atat`; BF1 HOT1's two snow AT-STs and two AT-ATs; BF2 GEO1's `rep_walk_atte` (0/1) and BF1 GEO1's two.
  Not looked at: whether the claim fails on the team, the seat, the 40 m seek range, the route or `RIDE_GAIN`.
- **The PS2 aims 20.2 m ahead along the leg** (re-walker-ai.md section 6); the port steers straight at the node.

## Note from STEND (2026-10-02, `lead_tmp/stend/README.md`) - a spider walker walks into a post's ramp and leaves its pilot inside the platform
- BF1 GEO1 seed 42: a bot pilots a `cis_walk_spider` at cp14 (123, -59.6, 7) and cp13 (1, -58.8, -3), posts on slabs 2.4-3.7 m over the terrain with one ramp. The walker follows the terrain INTO the ramp (body origin at (120.7, -60.88, 3.7), the ramp's surface there is -59.9), stops, and the bot leaves it: `Sim::exit_point`'s candidates start inside the platform's shell, the 5 m ray up meets the slab from underneath, and the bot is put on the terrain under the post, walled in (`TRACE_UNIT=27,16280,16470 round_to_outcome 16500 42 bf1_geo1`, the `rides` lines).
- STEND made such a bot "not arrived", so the last resort recycles it and the round ends (it was 9 v 2 at 60,000 ticks). The cause is still there: a walker should not stand inside a ramp, and an exit should not land inside a closed shell (the PS2's ray-vs-leaf test may be one-sided, card COLN: then its up ray passes the slab and the drop lands on top).

## Notes
- (card CWSP, 2026-10-02) With the command walkers' spawn points in, units spawn 6-8 m from BF2 HOT1's AT-ATs (5 of the
  opening 17, half of the round's spawns) and BF2 GEO1's AT-TE and STILL never pilot them: `imp_walk_atat 0/2` and
  `rep_walk_atte 0/1` at 60 s and 180 s on 8 of 8 seeds (`lead_tmp/cwsp/build/census/cwsp_s*.txt`), where BF1 HOT1 reads 2/2
  and BF1 GEO1's AT-TEs 2/2 at 60 s. So BF2's gap is the boarding / route test alone, not distance.

- Planner 2026-10-02 14:45: `deps: WAI,STARC,PARR`. Both cards edit `ai/route.rs`; WAI2 taken first would hold the maintainer's screenshot card (PARR) for a whole build. VSND is raised to 5 so M2 keeps a value-5 card meanwhile. CWSP's note above stands: BF2's AT-ATs / AT-TE are still never piloted with units spawning beside them. Undo: `loopctl.py set WAI2 deps=WAI,STARC`.

## Built and landed (2026-10-02, main a582462, `reports/20261002_2250_card_WAI2.md`)
What was wrong, in the order found (base 142dd4b; BF1 HOT1 / GEO1 already met their counts after CWSP and OFFG):
- **BF2's AT-AT and AT-TE never routed**: a vehicle was located on the smallest hub disc it stood in whatever its type. The AT-AT
  stands in soldier hub 0 (r 66.4 m) laid over hub 60 (r 75.2 m), the first of its own HUGE path (60-61-62-63, arcs of mask 16).
  Now a vehicle type is located among the hubs it has an arc at (`nav::locate_on`).
- A claimer walks the graph to its vehicle's hub, then straight (`route::retarget_board`); seek 75 m, give-up 30 s, +2 m reach.
- A claim is LOST (not given up) to an older claim on the same vehicle or when its pilot seat is taken and 8 s have passed.
- A command walker (class with spawn points) is claimed whatever the trip / gain, and its pilot stays in (out only as a hunter
  with <= 6 of its team alive). An armed walker's route ends at the hub, not at the enemy / the post beyond it.
- A tauntaun rides on from its drop hub to the post when the floor between has no drop (BF2 Hoth's trenches: without that test
  riders bailed out inside the trench and whole teams were trapped there - one round of 8 did not end).
Numbers (4 seeds 42 / 1 / 2 / 3, `lead_tmp/wai2/v7`; base `lead_tmp/wai2/base`):
| | base | built | PS2 |
|---|---|---|---|
| BF2 HOT1 AT-AT at 60 / 120 / 180 s | 0/2 | 2/2 (one seed 1/2 at 60 s) / 2/2 / 2/2 | 2/2 |
| BF2 HOT1 snow AT-ST at 120 s | 0/1 (180 s) | 1/1 x4 | 1/1 |
| BF2 GEO1 AT-TE at 60 / 180 s | 0/1 | 1/1 x4 | 1/1 |
| BF1 HOT1 tauntauns at 30 s | 5/8 | 6/8 x4 | 7/8 |
| BF1 HOT1 m per walker ride | 207 / 295 / 239 / 180 | 275 / 290 / 322 / 301 | 282-711 (tauntaun) |
| claims given up, BF2 HOT1 (all classes) | 2-4 | 0 / 0 / 0 / 1 | - |
NOT met / open: (1) 8 seeds x 32: 255/256 beside a null's 256/256 - bf1_yav2 seed 1 (two holders in sight, card ENDH; not a vehicle
state). (2) BF1 HOT1 still gives up 2-12 claims a round, nearly all on tauntauns at the pads (-304.6,-36.6) and (-412,172.8): the
claimer is in the trench beside the pad (same hub, a wall between). (3) The other two tauntauns unridden at 30 s stand 62-200 m from
any bot; the PS2 spawns 3 rebels at (-450..-425, 175) and 2 at (-500,-75) (`lead_tmp/wai2/ps2/b1hot1_sold.csv`), the port none there.
(4) m per ride is per walker, not per tauntaun. (5) BF2 HOT1's second AT-AT pilot is seated after 30 s (PS2 24 s).
The per-reason census is scratch again: `lead_tmp/wai2/v6_with_census.patch` (on 142dd4b, WAI2_TRACE give-ups and walk-outs,
NAV_DUMP hubs / arcs / posts / walkers at 30 s); landed are `NavStats::ai_claims / ai_claim_giveups / ai_claim_lost` (the
`vehicles:` line) and the walkers line at 30 / 60 / 120 / 180 s.
