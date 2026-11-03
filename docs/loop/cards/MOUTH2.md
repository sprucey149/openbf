---
id: MOUTH2
title: BF2 Kamino's two other stall sites after MOUTH: the pod room's door (47 stalls per 100 passes against the PS2's 4 - every bot walks at ONE corner inside the room and meets the east jamb) and round hub 46 with its pod (50 against 5) - trace each beside the PS2's own nodes, name the cause, build it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-11: bots "miss ... go to the side of it, try running at the side for a few seconds, then go back a few feet" - his words of the walkways, which card MOUTH cures. Measured by MOUTH (2026-10-12, main 19708fa, 8 seeds x 3 min): the walkway legs hold 28 of BF2 Kamino's 292 stalls; these two sites hold about 150, and MOUTH's change does not move them (door 93 -> 87, hub 46 unchanged). Kamino is a showcase map and hub 46 is the spot of the maintainer's own scene.

## Evidence (`engine/artifacts/lead_tmp/mouth/`, README.md; `out/kam1_sites.md`)
| site | PS2 passes / stalls / per 100 | port passes / stalls / per 100 | port backed up |
|---|---|---|---|
| pod room door (hubs 94 -> 98), disc (91.5, 178.6) r 10 | 51 / 2 / 4 | 271 / 127 / 47 | 122 |
| round hub 46 (r 15) with `kam_bldg_pod2`, disc (-15.3, 102.1) r 20 | 206 / 10 / 5 | 404 / 201 / 50 | 19 |
(stall = `tools/pstall.py`; PS2 = two 8-minute captures `ps2/bf2_kam1_m{2,3}_units.csv`.)
- **The door.** Bots crossing the door's plane (z 178.6) northbound: the PS2's 60 are spread over x 88.3 .. 95.0; 147 of the port's 227 cross at x 93.5 .. 95.0, the east jamb, and 91 come back out southbound (the PS2: none). In the port every bot on `94->98` walks at ONE fixed point, (93.8, 190.6) - a barrier's corner inside the room - and its waypoint is hub 98's CENTRE (90.8, 193.4): `route::hub_point` gives no drawn point there. The straight line from a hub-94 point at x 93-95 to that corner meets `kam_bldg_podroom_door` / `kam_bldg_interior_front_entry` at z 178; the bot slides back south 5 m, re-plans, tries again (`tools/trace.py out/kam1_port_p.csv snap_kam1 '94>98' 2`). The PS2's followers hold hub-98 points all over its 11.7 m disc, and for some bots two corner nodes down the door's middle, (88.6, 177.7) -> (88.3, 182.5) (`tools/ps2leg.py <kam1> ps2/bf2_kam1_m2_units.csv 94 98 16 2`). [H] causes, none isolated: hub 98's point falling back to the centre; the arrival at hub 94's point by its plane 1-2 m east of it; the corner search finding no corner AT the door.
- **Hub 46.** Legs `46->44 / 42 / 45 / 43`, `43->46`, `44->46`: `stop` wall `kam_bldg_pod2` or steep, half of them standing still. Card OBST2's note: the pod is ringed by four barriers with 1.6 m gaps, the hub's centre lies inside the ring and a leg leaving it has its line through a barrier. The PS2's bots stall there too (10 in 206 passes) - a tenth as often.

## Measured 2026-10-12 (lane 2, main 5fbbbd4; `engine/artifacts/lead_tmp/mouth2/`, tools in `tools/`, tables in `out/`)
Re-measured on today's main (8 seeds x 3 min, `snap_main`): Kamino 609 P-stalls; the door 121 in 270 passes (45 per 100;
the PS2 2 in 51 = 4), hub 46 256 in 468 (55; the PS2 10 in 206 = 5). The card's [H] causes for the door are all wrong.
- **The door is a real BF2 sliding door** (`kam_bldg_podroom_door`, the instance named "open": two leaves, 3.75 m each,
  41 frames = 1.33 s, trigger r 4.0). A bot at 7 m/s is at the leaf 0.5 s after it enters the trigger. **PS2, 44 passes
  at 10 Hz** (`tools/doorpass.py --rows --ps2 ...`): lanes spread -3.2 .. +3.0 m off the centre; the inner lanes pass at
  speed, the outer ones (|s| over 2.2 m) walk up to 1.32 m short of the door's plane and STAND there about 0.5 s, the
  contact push (0.048, -0.999) every other sample, the stick at 0.429, then walk through when the leaf has cleared
  (door rebuilt from the units' feet: open 0.8-0.97 at the crossing). Nobody turns away; 0 pass southbound.
  **The port:** the wall probe meets the leaf ("wall 0.27-0.38 kam_bldg_podroom_door open"), avoid::pick commits to
  going round it (slide +1, the round allowance) and the bot follows the wall away for 10 s, 30-50 m, re-plans, comes
  back (`out/` trace of 3 stalls: s60 units 28, 31, 24).
  **Switch (scratch, `MOUTH2=1`: a leaf of a door that is opening is walked at, not gone round):** the door 121 -> 0
  P-stalls in 153 passes, Kamino 609 -> 491. P-stalls within 6 m of ANY door (`tools/doorsites.sh`), null -> switch:
  Kamino 101 -> 2, Death Star 61 -> 20, Polis Massa 38 -> 25, Mustafar 98 -> 75, Tantive IV 315 -> 261 (613 -> 383).
  All maps, 8 seeds (`sw/a_00`, `sw/a_D`): only the five door maps' rows move (27 maps byte for byte), rounds 256 / 256,
  recycles 40 / 40, lost 21 / 16, floor 19 / 19. The maintainer's scene 5 of 411 trails over 6 reversals -> 5 of 449.
- **Hub 46 is the soldier's body on the pod's skirt, not the route.** The pod stands on a dais (floor 61.6) ringed by
  four rail walls, each with a sloped skirt outside whose lowest 1.2 m is a 48 degree face (n.y 0.667), and four
  diagonal ramps between. The corners of the four rail barriers lie ON the skirts (the port 1.5 m off the rectangle,
  the PS2 1.0 m: its nodes (-9.2, 98.7), (-17.3, 96.5), ...). **The PS2's bots walk up the skirt** (30 ascents from
  the ground in 16 min; `ps2/bf2_kam1_m2_units.csv` k 27 from a STAND 1.7 m off the foot: a steady 2-3 m/s up the 48
  degree band for 0.6 s, 60.30 -> 62.04 in 1.0 s - the uphill-scaled target speed 7 x (1 - sin^2) = 3.1 m/s, so its
  stick has full hold there). **The port's cannot:** (1) the steering's steep probe calls the band a wall (card
  STHOP), the bot slides along the foot and is left at the skirt's end pushing at it (`open fwd v 0.4`, 20 stalls at
  (-7.0, 99.0) alone); (2) with the probe off (`MOUTH2=2`) a bot running at 7 m/s is stopped dead in one tick by the
  LEDGE rule: the wall pass asks the full capsule at the height the feet had BEFORE the floor is found, so a riser
  whose top edge is the face being climbed (vertical triangles (0.896, 0, 0.445) at (-17.4..-18.4, 107.9..109.8)) pushes
  him sideways and takes 6 of his 7 m/s (`TRACE_PHYS`, seed 60 unit 18 tick 1057); (3) with the ledge asked at the
  floor under the axis (`MOUTH2=32`) he keeps the projected 5.3 m/s and gets over the corner, but head on the run comes
  up 0.35 m short of the band's top (control x 0.1 under n.y 0.7: he climbs 0.94 of 1.3 m and slides back).
  Hub 46, P-stalls: main 256, door switch 216, + ledge 193, + ledge + no steep probe 142 (35 per 100). The maintainer's scene
  (it IS this spot): 5 / 5 / 6 / 10 trails over 6 reversals - no variant cures it. **Open, and the next step: what
  normal the PS2's soldier holds on that band** (S+0x4b0; BF2 FUN_002480a0 gives x 0.1 under N.y 0.7, B2:163053, and a
  steady climb is impossible under it) - one PCSX2 run. Filed as card SKIRT.
- Not a gap: the conquest script locks pod room doors 32 and 28; no bot stands within 2 m of either on the PS2 or the port.

## Built and landed 2026-10-12 (main ac13ca6, docs cae9777; report `reports/20261012_1729_card_MOUTH2.md`)
- **The door rule** (`ai::steer` `StopKind::Door`, `Sim::door_opening`, the walk-on in `avoid::pick`): a leaf of a
  door that is opening - a living unit in a trigger, the clip not at its end - is walked at, not gone round.
  Acceptance 3, the door: **0 stalls in 157 passes** (main 121 in 270; the PS2 2 in 51). Kamino 609 -> 485. Stalls
  within 6 m of any door: Kamino 101 -> 1, Death Star 61 -> 29, Polis Massa 38 -> 31, Mustafar 98 -> 83, Tantive IV
  315 -> 257. The maintainer's scene 5 of 411 -> 3 of 403. Landing gates: the sweep's five door maps moved (declared), every
  other gate PASS, fidelity 561 values none worse. 24 seeds x the 5 door maps, main before -> landed: rounds 120 /
  120 both, recycles 7 -> 7, lost 49 -> 43, floor 0 -> 1, under 1 -> 3; Polis Massa alone over 96 seeds recycles
  26 -> 15 (its 3 -> 6 in 24 was noise). Tests: 3, with 10 mutations each failing one (`mouth2/tools/mutate.py`).
- **Acceptance 3, hub 46: NOT met** (52 per 100 passes; the bar is 10). It is the soldier's body on the pod's skirt
  - card SKIRT (value 5, phase re) has the traces, the three faults and the bars. Acceptance 4 = split.
- For the planner, of the maintainer's INBOX note 2026-10-12 16:00 item 2 (card DOORS, not filed yet): his ask (2) "a bot at
  a door that is closed or still opening WAITS" is this landing, and what he saw is the trace on this card. Ask (1)
  "through the centre line" is not the PS2's (lanes up to 3.2 m off it) and the pod room door needs none of it now;
  ask (3) holds for a 1.33 s door, and is open for the Death Star's 5 s blast doors (a wait of up to 2.7 s against a
  3 s stuck window). What is left for DOORS: Tantive IV's 257 and Mustafar's 83 door stalls (not a leaf, untraced).

## Notes
- Depends on MOUTH (the walkway rule lands first; both touch `ai/route.rs`). `files:` empty while this is `measure`.
- The same kit on any other map: `tools/table.py` names the worst leg per map (BF2 Geonosis `9->13`: 97 stalls; BF2 Coruscant `65->64`; Mos Eisley `170->199`); those are not this card - a note for the planner.

## Notes from MOUTH's build (2026-10-12 13:31, lane 1; main 273a441; `lead_tmp/mouth/build/`)
- MOUTH landed two rules (the leg line from the last corner passed; a hub hop opens a new progress window). Kamino
  after it, 8 seeds x 3 min: 239 of the card's stalls and 616 P-stalls (main 258 / 711); the walkway legs 0 / 6. The
  door `94>98` (131 P-stalls) and hub 46's legs are untouched, as measured: this card.
- **A third site, leg `56>51`** at (22.7, 51.2, 2.8): the plan's corner lies in / at `kam_bldg_ramp1`'s wall. Bots walk
  straight at it, meet the wall at 0.0-0.6 m, go round the wrong way for 5 s and re-plan: 21 P-stalls on main, 41 with
  the corner rule alone, 22 with both landed rules (`build/kam1_main_P.csv`, `kam1_MH_P.csv`; `tools/trace.py ...
  "56>51"`). A corner nothing can stand on - the planner's point, not the follower's line.
- Two edges of the corner rule, not built: (1) when the continued corner search (`advance_route`, card OBST2) finds
  no further corner the leg's corners are cleared and the line is `Route::from`'s again; (2) a corner counts as passed
  1.0 m short of it (`CORNER_ARRIVE`), from either side - at BF1 Kamino's hub 13 the corner (147.74, 160.31) was
  "passed" at (148.39, 159.61), on the rail's side of it.
- Tools added in `lead_tmp/mouth/build/`: `hopfail.py` (failed windows by ticks since the hop; needs the debug prints
  of `scratch_dbg_hop.patch`), `permap.py <cause> <sweep dirs>`, `worlddeaths.py <BOT_SAMPLES csv>` (lives ended with
  no kill event: spot and last 3 s), `sw.sh` / `acc3.sh` / `snapk2.sh` (the knob binary `rto_knobs2` =
  `mouth_hopwin_knobs.patch` on 4ca302f: `MOUTH=1`, `HOPWIN=1`; null = main).
