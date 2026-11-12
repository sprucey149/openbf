# OPUS RE (2026-09-30) - [O] UNAUDITED: no Fable pass

## Physical descent after a failed ground walk, November 11, 2026

A stranded ground bot with a lower resolved navigation floor nearby can try a
descent before the last-resort relocation or recycle. The preview steps the
soldier and uses the same world wall response as the simulation. It rejects
lethal speed, death and damage volumes, landings under the map, and floors that
are off the playable level or too steep to climb. The bot uses an ordinary
return waypoint and resumes its route after the descent.

This is an openbf recovery rule. The original BF2 stuck check
(`FUN_002f24e8`, B2:281420–281538) hands a stopped soldier a temporary random
walk (`FUN_002cb2c8` / `FUN_002cb580`, B2:253566–253735). Compare native
shelf escapes in play before claiming parity.

## Why rounds stall — current source checkpoint, November 4, 2026

The dated September/October census below remains historical evidence. The
November 4 source at `4cda5ab4` includes four later port corrections:

- `ai::behave::brain::chain` completes Hard/Elite mending at the droid's actual
  service sphere after reaching its connected hub. Disconnected and stale route
  fallbacks remain. Native station-seeking is still untraced.
- `Sim::settle_spawn` / `connected_spawn_node` use a connected authored alternative
  at the same selected friendly post when a regular bot's node has no walkable
  exit. Opening ambiguity, players, proxies, jets and walkers retain their rules.
  A changed opening spawn can change the battle from tick one.
- `ai::mover` applies the last-reinforcement stall watch using the individual
  bot's team tickets. Opposing low tickets no longer tighten a mobile bot's watch.
- `ai::behave::position::lost_jump` permits the existing ordinary jump when the full
  capsule demonstrably cancels forward movement despite a clear stepped sweep.
  Headroom, raised sweep, cooldown, reverse-speed and Droideka controls remain.

These are deliberate openbf corrections. `measure:` matched original-game
station-seeking, Endor spawn/departure, last-reinforcement movement and pinned
ledge jumps before claiming equivalent native behavior. The retained scoped
fixtures and rounds establish the port changes, not that acceptance. The Bazaar
jump clears the named BF2 Yavin Easy 3 and Normal 3 witnesses; Normal 1 still
recycles the same life at a later corner. The Geonosis and Bespin false-arrival
experiments did not demonstrate physical escape and are outside this source.

The exact 34-map Normal 3 no-jump comparison later completed: only BF2 Yavin
changes, beginning at checkpoint 2,731; the other 33 full streams agree. This
supersedes the earlier statement that that matrix had not run. The owning
#108 technical record retains the per-source vectors and unchecked broader gates.


> **AUDIT 2026-09-30 (loop lane 3, OK, SCOPED — `engine/artifacts/loop/audits.md`, report
> `engine/artifacts/loop/reports/20260930_1745_audit_SWPN.md`, evidence `engine/artifacts/lead_tmp/audit-swpn/`).**
> Audited with SWPN, whose multi-seed tables are this doc's port side. Re-derived before reading the doc; one Opus
> verifier, 11/11 SUPPORTED, wording folded in.
> - **Held.**
>   - **§0.** All eight rows reproduce from the 9-seed files. "env deaths/min" counts region + fall only; vehicle and
>     blast deaths are left out (GEO1 0.17 and YAV1 0.33 with them).
>   - **C1, port.** Every KAM1 region death came airborne after a 2-50 m drop. None was grounded, at spawn, or mounted.
>   - **C1, PS2.** My classifier over the raw KAM1 RAM samples in `lead_tmp/stall/ps2/`: 123 deaths, 2 below the decks.
>   - **C2.** The mechanism holds: `advance_route` hops on XZ distance only.
>   - **C4, in full.** Checked on the ELF: the TriggerSingle bit, the re-arm U(0.2x, 2x) with a real `max.s` at
>     0x2bb018, the sniper U(2, 3) s, and BF1's 4-call cadence. A bot gets at most one salvo per hold, not exactly
>     one. After an idle call, BF2's first press comes on the 2nd want-fire call. A second property key, 0x0ee66c41,
>     reaches the same TriggerSingle store in both ELFs; no disc ODF uses it.
>   - **C7.** The `objectiveconquest.luac` bleed-timer function holds, as does the port's `apply_bleed`.
> - **Not re-derived:** C3, C5, C6, C8, C9, the 69 % rim share of no-progress episodes, any fresh PCSX2 run.
>
> **AUDIT 2026-10-01 (with the landing STFAL, PARTLY — `engine/artifacts/loop/audits.md`, report
> `engine/artifacts/loop/reports/20261001_1845_audit_STFAL.md`, evidence `engine/artifacts/lead_tmp/audit-stfal/`).**
> Two lines of this doc do not hold.
> - **C1's DEA1 control.** "Its pits kill bots on both sides at the same rate … those deaths are the game's own": the
>   rates agreed, the falls did not. The PS2's 21 falls are route walk-offs, none after a hit in the 6 s before: 12 at the
>   pit beside cp3's spawn node (-118.9, 52.0, -235.8), 11 of them spawned there; 6 into the gap at the Tak bridge (x -171,
>   z -207..-196; `dea1g_con` opens it when `Panel-Tak` is destroyed — Connection128, Barrier222); 3 at two other edges.
>   The capture is 360 game seconds with a level reload at wall 348-374 s: 3.0 falls per wall minute, 3.4 per game minute
>   in its first battle. Of the port's 715 fall events over 8 seeds, 16 were at the first place and none in the gap, where
>   its bridge is solid. Card DPIT.
> - **C8's 20,520.** The spawn unit (unit 7, seed 5) stayed at the AT-STs for 48,649 ticks with a stuck run of 8,220; the
>   20,520-tick run that sets KAS2's `stuck_max` is unit 19's, a defender crowd at (225, 17, -5). The cause itself holds:
>   stays of 600 ticks or more at the spawn went 32 -> 3 over 9 seeds with STFAL's hull probe.

**Card STALL: why rounds stall.** Every stuck unit is classified per map and set beside the PS2 (the maintainer, INBOX 2026-09-30
08:50). There are **seven causes** of the port's own, one scoring artefact, and one rule the PS2 shares.

The causes: none of them turns up in the PS2 runs of the same maps.
- **C1** units fall off walkways.
- **C2** a unit ends up stranded on the wrong floor of an XZ planning graph.
- **C3** DEA1's blast doors open into their own doorway.
- **C4** a bot fires a single-shot weapon once per trigger hold.
- **C5** units walk beside doorways instead of along the arc.
- **C6** straight last legs run into walls.
- **C8** parked vehicles block a spawn.

The scoring artefact:
- **C9** units jostling on one point inflate `stuck_max` without stalling a round.

The shared rule:
- **C7** the losing side's bleed clamps at its alive count. From then on only a kill or domination ends the round, so a
  single stuck, unreachable or unkillable survivor stalls it forever.

The PS2 comparisons are live PCSX2 runs of BF2 KAM1, DEA1 and POL1: 16 bots a side, 420 s each, every soldier sampled
from RAM (§4). On the PS2 those maps fight at 14–17 deaths/min. The port's run 0.1–1.1 kills/min.

Tags: [V] read (lines, bytes, or a PS2 RAM measurement cited); [D] derived; [H] hypothesis. `[O]` = one Opus pass +
one Opus verifier, no Fable audit (§7: 17 of 19 claims SUPPORTED, 2 PARTLY, 0 not; corrections folded in).

Scratch (tools, logs, pictures, the verifier's `verify/VERDICTS.md`) is in `engine/artifacts/lead_tmp/stall/`. The
scratch worktree `openbf-wt/stall-scratch` holds main 027dad9 plus:
- a read-only census in `round_to_outcome`;
- the probe example `stall_probe`;
- the LVL door dump `door_dump`.

None of that landed.

## 0. The answer in one table

The instrument is main 027dad9's sim over **9 seeds** (42 and 1–8), 60,000 ticks, all 32 maps. The seed-42 rows match
the VTUR2 baseline byte for byte, hashes included. Numbers: `all/sweep_s*.txt`, `all/eps_s*.err`.

Eight maps fail the maintainer's line (finish on ≥ 7 of 8 seeds, `stuck_max` < 10,000):

| map | finished /9 | stuck_max median / max | kills/min · env deaths/min | named causes |
|---|---|---|---|---|
| BF2 KAM1 | **0** | 27,840 / 47,760 | 0.12 · **8.17** (99 % env) | C1 falls (270 of 276 deaths at seed 42) → C2 rim traps (69 % of no-progress episodes); then C7 |
| BF2 POL1 | **0** | 21,300 / 51,480 | 1.12 · 0.09 | C2 hangar ramps; C5 radar-building door; C7 (team 1 clamped at 7–15 alive) |
| BF2 DEA1 | 4 | 1,560 / 4,380 | 1.01 · 3.33 | **C3 blast doors** (53 % of stalled unit-ticks); C7 (e.g. 2 v 16) |
| BF1 KAS2 | 6 | 3,600 / 20,520 | 10.6 · 0.03 | C8 parked AT-STs at the Empire spawn; C7 (3 v 2, 12 v 5) |
| BF2 MUS1 | 7 | 2,220 / 37,140 | 15.2 · 0.04 | C1 → C2 (a fall from the 66 m deck to the 59.4 level) |
| BF2 GEO1 | 8 | 1,260 / 51,600 | 12.0 · 0.10 | C5 bunker at hub 9; **C4 standoff** (rocket / EMP / Bulldog); C2 cave |
| BF2 MYG1 | 8 | 840 / 46,920 | 19.0 · 1.24 | C6 last leg through a spiral ruin (seed 6, 1 v 3) |
| BF1 YAV1 | 8 | 1,260 / 24,240 | 17.3 · 0.02 | C9 crowd on cp5 bazaar's centre, both teams (seed 5) |

The kill and death rates are summed over 9 seeds. A map that finishes runs 10–32 kills/min.

Other maps:
- TAT2 and BF1 GEO1 each lose one seed to C7: 11 v 1, and 2 v 16.
- BF1 NAB1 (showcase) and BES2 always finish. Their `stuck_max` comes from C6 on NAB1 (a ruins column) and C9 on BES2
  (defenders jostling on their spot).
- JKAM's old KAM1 spot (unit 24 at (108, 52.25, 149.85), hub 88) is gone: 0 episodes within 8 m over 9 seeds.

## 1. The instrument (scratch, read-only on the sim)

**Episodes.** `STALL_LOG=1800,3000` opens an episode when either happens:
- a unit's `Route::stuck_run` reaches 1,800 ticks;
- a unit holds one leg for 3,000 ticks while gaining less than 2 m on its waypoint. This catches the wall slides the
  stuck window resets on ("sweep footer hides pinned units").

The opening `EP` record gives:
- team, class, position;
- the leg (from / to / dest / arc / source), and the sub-point's dy and floor;
- the locus, the steering's own probe, any soldier ahead, the wall push, the steer pick;
- objective, post, target, ammo, mount and ride;
- the SOLDIER-layer bodies within 1 m, by instance name;
- the last 200 ticks of path.

The closing `END` record gives how it ended (death / moved / leg change / progress / round end), how long it lasted,
and how far the unit moved.

**Other census lines.**
- `TRACE_UNIT=<i>,<n>`: one unit's pose, leg, fire state and nearby vehicles every n ticks.
- `DEATHS`: kills and environment deaths per team (`Event::Kill` / `EnvDeath`).
- `SHOTS`: `Event::Shot` per weapon class, over the unit-lives that carried it. These count projectiles, so a shotgun's
  8 bolts a salvo count 8.
- `AMMO`: alive units with an empty weapon. There are none: a stall is not out-of-ammo.

The census only reads the sim. Seed 42 reproduces the baseline's hashes, and a second 32-map seed-42 run reproduced
every row again.

**Analysis tools.**
- `eps.py` clusters episodes (6 m XZ, 3 m y) and gives each cluster's stalled unit-ticks, seeds, legs and a first-pass
  class.
- `fm.sh` / `floormap.py` render the top three floors under a region (optionally sliced below a height), with hubs,
  arcs, barriers and episode trails (`maps/*.png`).
- `stall_probe`:
  - `walk` drives one soldier at a point under the real physics;
  - `profile` gives floor and sweep along a line;
  - `bodies`, `doors` and `doorcheck` show which way each door body moves when it opens.

**The PS2 sampler, `stall_ps2.py <map>`.**
- It boots `bf2_menu_iaplanets.p2s` (AI Units Per Team 16) and walks `walk_bf2.py`'s menu path to the port's era. It
  checks the mission name at 0x69a638 and `numbots` 16.
- The player stays on the spawn map, never spawned.
- Every 0.5 s it reads every soldier entity's feet and health from live RAM (`walk_bf2.soldiers`: class at
  E+0x8 == E+0x4a4, health E+0x170, feet E+0x150). Every 5 s it reads post owners and tickets.
- The emulator ran at 100 % (SW renderer).

`ps2_stalls.py` splits the samples into lives. It classes each death FALL (the feet went below a floor height within
the 3 s before) or OTHER, and lists still spells (≥ 60 s within 2 m).

## 2. The causes

### C1 — units fall off walkways [V measured, both sides]

**Port, KAM1.** At seed 42 there are 276 deaths in 60,000 ticks. 270 are the death region under the platforms (the sea,
y ≈ 33); 6 are kills. Over 9 seeds that is 0.12 kills/min against 8.17 environment deaths/min.

The deaths cluster at a dozen fall sites:

| fall site | deaths | where |
|---|---|---|
| (39, 192) | 43 | east bridge, leg 121→19 |
| (−76, 28) | 43 | platform 47's west ramp |
| (0, 11) | 32 | — |
| (−29, 279) | 29 | leg 56→68 |

One unit's trace (`kam1_u23.err`): it died 12 times in 10,000 ticks, 11 of them by walking off a bridge or deck edge.
The fall moments show two things:
- the steering's `slide` taking the unit off a 5 m bridge;
- a leg's sub-point pulled off the walkway line.

**PS2, KAM1, 420 s:** 123 deaths — 121 at deck height (combat) and **2 falls (1.6 %)**. Only 3 of 26,529 alive samples
are below the decks, all mid-fall (`ps2/bf2_kam1_*`).

**So KAM1's lemming run is the port's own.** The ai.rs header says the wall sweep, slide and fan "are OURS and not on the
PS2". Those, plus the 5 m side-step and off-line corner points, carry units off 5–8 m walkways. On MUS1 the fall lands
alive, on a lower level (C2).

**Controls.**
- **DEA1:** its pits kill bots on both sides at the same rate — the PS2 at 3.0 falls/min, the port at 3.33 env
  deaths/min. Those deaths are the game's own.
- **BF1 BES1** (Bespin Platforms, which finishes): falls are 33 % of deaths (11/min). BF1 has no AI-count option, so
  measuring the PS2 there needs `ps2live.py` (§6).

### C2 — units on the wrong floor of the XZ graph [V port; not seen on the PS2]

The BF2 graphs author every hub at one height: KAM1 at y 69.7, POL1 and DEA1 at y 0. Arrival and locate test only XZ,
and a leg's waypoint takes the floor at the unit's own level (RE_LOG "Arrival and locate"; `ai::route::hub_waypoint`).

So a unit that reaches a hub's XZ on another floor counts as arrived. Its next legs then walk under or beside the floor
they need. `hub_has_floor` accepts a hub if *any* floor lies under it at the unit's level. KAM1's platforms have sloped
lower rims that run under the decks, so there is always such a floor.

| map | cluster | what the probe shows |
|---|---|---|
| KAM1 | (−69.9, 50.5, 269.6): 176 episodes, 1.02 M unit-ticks, 3 seeds | the unit is on platform 55's lower rim (50.5), 10 m under the deck (60.3), under the ramp to hub 66 (69.1). Its straight line meets a 1.3 m lip, then open air (`profile`, `maps/kam1_a.png`). It got there by sliding off the cp2 deck alive (unit 23, 13th life, t 10,395) |
| KAM1 | (−58.8, 57.3, 29.0) and 5 more rim clusters | the same shape under platforms 47 and 2 (`maps/kam1_b.png`) |
| POL1 | (112.6–119.9, 13.1, −38): hangar_04, 3 clusters, 1.1 M unit-ticks, 9 seeds | hangar floor 13.1. Hubs 168 and 4 are on the ~20 m corridor, reached by ramps rising from the far ends. The units "arrived" at hub 168 in XZ on the floor below, and push at hub 4 through the ramp's side (`maps/pol1_b.png`, sliced at y 20) |
| TAT2, seed 8 | six attackers under cp0 ("cp7") | cp0's capture box is at y 9–13; they stand at y 0 for the whole round |
| MUS1 | (−66.3 / −36.7 / −29.6, 59.4, …) | fell from 66 onto the 59.4 level (unit 30, 14th life, t 12,105), then walks at hubs on 66–68 |
| GEO1 | unit 27 in `geo_prop_cave1` | the next hub's floor is NO_GROUND at its level |

**PS2.**
- **KAM1:** no alive sample is on a rim; the lowest 1 % of heights is the 51.4 deck.
- **POL1:** in 420 s, no PS2 soldier stayed near the hangar_04 spots (0 and 2 samples within 4 m).

### C3 — DEA1's blast doors open into their own doorway [V port rig; V PS2 crossings]

**The rig.** `dea1_prop_door_blast` is a `door`: a 16 m trigger at `hp_bdoor`, 151 frames. It is the only door class
whose bones bind away from the origin. `door_dump` on DEA1.LVL gives:
- `bone_leftdoors` at x −5, `bone_rightdoors` at x +5 (POL1's and KAM1's door bones bind at 0);
- the geometry bound to each bone lies on the **opposite** side: the left leaf's segments at x +0.74..+7.36 and its
  three prims at +2.37..+4.13, the right leaf's at −7.36..−0.74;
- the `open` clip moves `bone_leftdoors` −5 m.

**What the port does with it.** The bake reads the prims' `XFRM` and the segments as model space and applies the bone's
model-space delta (`bake/src/door.rs`; `game/src/assets.rs` composes `m * d * local`). So:
- the prims slide **inward** — at open = 1.0 the six bodies overlap x −67.4..−58.7, mid-doorway;
- shut, the doorway has a 1.3 m slit;
- 16 of DEA1's 24 blast-door bodies move INWARD, while every other door on 5 maps moves outward or up
  (`doorcheck.txt`);
- the walk probe stops at z −73.7 with the door fully open.

**PS2, DEA1, 420 s.** Soldiers crossed blast door 2 (the port's knot) **21 times** and door 0 **18 times**. Every
crossing was in the centre band (|along| < 4.3 m), where the port's opened leaves stand. 15 soldiers passed the port's
knot spot, the longest staying 8 s (`dea1_doors.py`).

**Port.** 220 no-progress episodes on 8 of 9 seeds, 1.73 M unit-ticks, at the knot (−60.8, 50, −72.8) on leg 4→1. The
second DEA1 cluster, (−86, 50, −99..−115) on leg 6→3, is blast door 0.

[D] Reading the geometry relative to its bone fits both ends: shut, the leaves meet in the middle; open, each moves 5 m
apart. Which composition the engine applies, and whether it covers the drawn segments too, is [H] until the rig is
rebuilt and checked against the PS2 crossings.

**Settled 2026-10-01 (card STDOR, `re-bf2-doors.md` §10).** The bone frame is the rule, for the drawn segments and the
prims, rotation included (MUS1's doors bind tilted 20.8°). PCSX2 frames of blast door 2 shut and opening match the
rebuilt rig. With it: no INWARD body on the 5 door maps; the walk probe crosses doors 0 and 2; DEA1 over 8 seeds
finishes 0/8 → 8/8 (median 609 s) at 12.3 kills / min (PS2 14.0); walking in place on seed 1 14.0 % → 2.3 %, with
neither door cell among the top clusters (`lead_tmp/stdor/`).

### C4 — a bot fires a single-shot weapon once per trigger hold [V]

**The weapon rule.** A `TriggerSingle` weapon fires only on a fresh press. The fire predicate is B1 `FUN_00289150`
(B1:211541–211559):
- charge (class+0x1e0 > 0) → command byte == −1;
- class+0xcc (TriggerSingle) == 0 → byte > 0;
- else → byte == 1.

`core::weapon::fire_predicate` mirrors it. Its test `trigger_single_fires_once_per_press_rate_limited` holds 1 s and
gets exactly one shot.

**The port's bot.** It rebuilds its pad every tick and sets FIRE on every tick `want_fire` holds (`ai::brain::think`).
Nothing in `think` knows `trigger_single`:
- FIRE comes up only on a tick when `want_fire` fails: aim outside the tolerance, out of range, contact not seen,
  reaction gate, burst pause;
- D11's burst counter only counts ticks that produced a shot, and a burst is at least 2 long.

So a single-shot bot holding a steady aim fires once, and no pause ever starts. The verifier's B1 nuance: the 1–5
rounds such bots fire per hold come from those incidental releases.

**The standoff.** GEO1 seed 42:
- units 11 (rocket launcher) and 15 (EMP launcher) face unit 30 (Bulldog) at 14–15 m, clear LOS, within tolerance;
- 1–5 rounds loaded, no reload, no heat;
- last shots at ticks 5,664 / 4,890 / 4,889;
- nobody dies for 50,000 ticks (`TRACE_UNIT`).

**Shots per unit-life, 32 maps, seed 42** (`shots/eps_s42.err`, classes with ≥ 10 lives):

| weapon family | shots per life |
|---|---|
| rocket launchers | 1.5–1.9 |
| torpedo launchers | 1.6–3.0 |
| sniper rifles | 2.2–2.3 |
| arccaster, mortar | 3–5 |
| automatics | median 22.0 |

Across all classes: rockets 1.54–2.00, snipers 1.00–2.89. Multi-bolt weapons count their bolts, so shotguns (8 a
salvo), the incinerator (15) and the bowcaster (7) are not comparable.

**The PS2 taps, in both games, by its own rules.**

*BF2 dispatch.* The AI fire controller `FUN_002bb1b0` (7 callers; the soldier's `FUN_0020b840` at B2:122042) sends a
weapon whose class has bit 0x32 of +0x1e0 set to `FUN_002baf70` (B2:242171–242178, its only caller). That bit is
`TriggerSingle` (re-ai-sniper §0 item 2 and §2's tables; the writer is B2:321001 / 321026).

*BF2, not a sniper* (`FUN_002baf70`, B2:242024–242065):
- While the bot wants to fire and its gate ai+0xf0 is clear, it presses for **one frame**, sets the gate, and sets the
  re-arm time ai+0xf4 = clock + `U(0.2·x, 2·x)`.
- x = max(0, 1 − v), where v = `FUN_002a2cf0` = lerp(0.1, 0.9) of the skill fraction (skill − 1)/19.
- On every other wanting frame it **releases**, and it clears the gate once the clock passes ai+0xf4.
- The bytes (ELF 0x2baff4–0x2bb038): `lui 0x3f80` (1.0), `sub.s`, `max.s` against 0, `lui 0x3e4c` / `ori 0xcccd` at
  0x2bb00c–10 (0.2), `mul.s`, `add.s f20,f20,f20`, `jal 0x2a3e48` (the uniform draw), `jal 0x4240f8` (the clock). The
  lerp endpoints are 0x3dcccccd / 0x3f666666 (0x2a2cf4–0x2a2d04).
- Result: a skill-20 bot re-arms after 0.02–0.2 s, a skill-1 bot after 0.18–1.8 s.

*BF2, sniper* (`FUN_002ace10`: TriggerSingle and ZoomMax class+0xb4 ≥ 6.0). The weapon goes instead to
`FUN_002baa38`: a want is suppressed until the clock passes ai+0xe8, which is then re-armed at clock + U(2.0, 3.0).
That is one shot every 2–3 s.

*BF1* (the verifier found this; I re-read it). The fire controller is `FUN_0023cd40` (B1:155147; the soldier's
`FUN_0020aad0` calls it at B1:119900). A TriggerSingle weapon (class+0xcc ≠ 0, B1:155185) goes to `FUN_0023cc20`
(B1:155107–155144, its only caller B1:155192). It runs a **fixed** cadence on the AI's trigger byte at ai+0x48+2·slot:
- `FUN_0023cc20` presses on 0 or ≤ −2, holds on 1, and releases on ≥ 2 or −1.
- `FUN_0023cd40` then steps the byte (B1:155204–155257): wanting, 0→2, <0→1, >0→b+1 (cap 100); not wanting, 0→−2, >0→−1, <0→b−1 (floor −100).
- Held steady, the byte runs 1 → 2 → −1 → −2 → 1: one fresh press (byte 1, the only value the predicate fires on)
  every 4 controller calls.
- A sniper (`FUN_00239cf0`: TriggerSingle, ZoomMax ≥ 6) also passes `FUN_0023c698`'s cooldown of clock + U(2.0, 3.0)
  (B1:154853–154892).
- [D] That the weapon reads this byte through w+0x70 is matched by encoding, not traced.

**Why it matters.** This is no map's own stall. But it turns every engagement between single-shot carriers into a
permanent standoff: the target is seen within `HOLD_RANGE`, so nobody walks, and nobody fires. It also slows every map's
killing, and C7's endgames need kills.

### C5 — units walk beside doorways instead of along the arc [V port]

**POL1**, (18.5, 25.1, 80.0): 100 episodes on all 9 seeds, 0.97 M unit-ticks.
- Arc 0 (hub 3 → hub 0, "Connection59") runs through `pol1_prop_door3`'s doorway at x 17.2.
- Units leave hub 3 (radius 1.6) 2–3 m south of the arc line and walk straight at hub 0, 66 m away.
- They meet the frame post beside the door and slide on it.
- The walk probe stops at (18.1, 78.6), 3.5 m from the door's trigger centre (radius 3.0), so the door stays shut
  (`maps/pol1_a.png`).
- **PS2:** 17 soldiers passed that spot in 420 s, none staying longer than 16 s.

**GEO1**, hub 9 (−80, −60.3, 265): units on `geo_bldg_bunker_1`'s floor, 2.5 m up, walk straight at hubs 12 / 0 / 13
outside and slide on the bunker's inner walls.

The PS2's soldiers aim straight at the node too (RE_LOG "Local obstacle avoidance"). What differs is that the port's
steering leaves the line (C1).

### C6 — straight last legs into walls [V port]

- **BF1 NAB1**, (−106.0, −108.5, 62.9): 41 episodes, 7 seeds. The locus is `NEAREST 2` (off-graph), so the route is only
  the last leg, straight at cp2 57 m away, into `nab1_prop_ruins_column2`.
- **BF2 MYG1**, seed 6: attackers on the last leg to cp0 (15 m) walk into `myg1_bldg_med_spiral_ruin2` for 54,140 ticks.

### C7 — the endgame rule [V PS2 rule; V port]

**The rules.**
- ObjectiveConquest's bleed drains only while alive < tickets. Otherwise it sets the rate to 0 and stops its timer, and
  the recompute runs on capture and neutralize events (`re-tickets-spawn.md` §1.3 [V]).
- Stock BF2 conquest has no time limit and no cull (`re-time-limit.md` §0 [V][O]).
- BF1 has an exhaustion cull: −900 s, then one AI every 15 s (`re-tickets-spawn.md` §2.6 [V]). The port does not run it;
  it plays every map under BF2's rules (PLAN §LOCKED FEATURE SET).

**What that does.** Once the losing side is clamped, the round needs every survivor killed, or every post held for
20 s. Every failing round ends like this:

| map | tickets | alive |
|---|---|---|
| KAM1 | 15/9 | 15/9 |
| DEA1 | 2/16 | 2/16 |
| KAS2 | 3/2 | 3/2 |
| POL1 | 130/13 | 16/13 |
| TAT2 | 11/1 | 11/1 |

With C1–C6 and C8 in play, one survivor that is stuck, walled off or in a standoff is enough. TAT2 seed 8: the last
Wookiee stays 12 m from cp3's two defenders, behind a wall, and they never go after it.

**The port's commander** offers Hunt only when no enemy or neutral post is reachable, or when a unit has no other offer
(`HUNT_VALUE` then competes only with defend scores). It has no endgame rule.

**A side note on the bleed.** The port recomputes it every tick instead of stopping it until the next capture. That is
more aggressive than the Lua, not a stall cause; it is noted for the bleed's owner.

### C8 — parked vehicles block a spawn [V port]

**BF1 KAS2**, seeds 3, 5, 8 and 42: 20 episodes at (114.6, −0.3, 293.0), the Empire spawn.
- Units walk south into three parked AT-STs (v16–v18 at z ≈ 291, 2.1 m away).
- The wall pass pushes them back +0.08 m every tick, while the steering's step-lifted probe reads clear most ticks.
- Without the vehicles, the probe soldier walks the same line at 6.5 m/s (`TRACE_UNIT=7` seed 5; `walk`).
- On seed 5 that unit's stuck run reaches 20,520 ticks.

### C9 — crowds on one point (scoring, not a stall)

- A defender within ~1 m of its spot, pressed by a neighbour, keeps its stick deflected and never arrives
  (`CORNER_ARRIVE`). Seen at BES2 (−96.4, −33.1, 186.7), and on KAS2's platforms at hubs 34 and 26.
- On YAV1 seed 5, 42 units of both teams pile on cp5 bazaar's capture centre with no targets.
- `stuck_max` counts all of these (BES2 up to 9,060).

## 3. The PS2's own stuck handling — what stays open

The PS2 mover's stuck test is B1 `FUN_0026b450` (thresholds from `FUN_0026b3b0`). It sets mover+0x6c when either
happens:
- the unit has not moved 2 m from its anchor within 3 s (droideka 6 s);
- it has stopped closing on its node for 5 s (droideka 6 s).

The detector is skipped while the unit is within 0.2 m (XZ) of mover+0x20: 0x3d23d70b at 0x26b4a4.

Agents read the flag through `FUN_0023b880`, at 7 `jal` sites in 6 functions (B1). What each agent does then —
re-path, new goal, give up — is **not read** [H]. The one site opened, `FUN_00253920`, builds and posts a type-2 order
when the flag is set.

The port's fix does not wait on it. The PS2's bots do not fall, strand, knot or stand off at these places (§4), so the
fixes go at the causes.

## 4. The PS2 runs

| map | sampled | deaths (combat · falls) | per minute | still ≥ 60 s | the port's clusters on the PS2 |
|---|---|---|---|---|---|
| BF2 KAM1 | 420 s, 37 entities, 30,013 samples | 121 · 2 | 17.3 · 0.29 | 18, all on decks (cp5's 79 m platform, the 51.4 decks) | no sample on a rim; 3 of 26,529 below the decks, all mid-fall |
| BF2 DEA1 | 420 s, 31 entities, 24,234 samples | 98 · 21 (pits < 35 m) | 14.0 · 3.0 | 6 | the knot: 15 soldiers passed within 6 m, longest 8 s; 21 + 18 blast-door crossings, all through the centre |
| BF2 POL1 | 420 s, 37 entities, 29,740 samples | 115 · 0 | 16.4 · 0 | 16, at posts (cavern docking station, cp areas) | radar door: 17 soldiers passed, longest 16 s; hangar_04: 0 and 2 samples |

POL1's tickets stood at 89/20 after 7 min, so that PS2 round was heading to an ordinary end. The sampler only reads.
Frames: `ps2/<map>_*.png`. The states are kept: `ref/states/bf2_{kam1,dea1,pol1}_ai16_gcw_sideselect.p2s`.

## 5. Build cards

One cause each, each with a measurable acceptance:
- **STTAP** — C4, per game;
- **STDOR** — C3;
- **STFAL** — C1 + C8;
- **STLVL** — C2;
- **STARC** — C5 + C6;
- **STEND** — C7 + C9.

SWPN, the 8-seed sweep, keeps its card. This session's scratch census and its 9-seed table are its first baseline.

## 6. Open [H]

- ~~The blast door's engine-side composition (C3).~~ Settled by STDOR, 2026-10-01 (see C3).
- The PS2 agents' reactions to the stuck flag (§3).
- BES1's falls on the PS2.
- That BF1's weapon reads the controller's trigger byte through w+0x70 (C4, matched by encoding, not traced).

## 7. Checks run and the verifier

**Self-checks** (outputs in `checks.txt`):
- `re_check fn` on B2 242171 / 242178 / 242032–242061, and on B1 155185 / 155192 / 155123 / 155137 / 119900 / 155204 /
  154853 / 154881;
- `callers` for `FUN_002baf70` (1), `FUN_002bb1b0` (7) and `FUN_0023cc20` (1);
- `word` at 0x2bb00c / 0x2bb010 and 0x2a2cf0–0x2a2cfc;
- `eedis` over 0x2baff4–0x2bb040 and 0x2a2cf0–0x2a2d10.

**Twin, BF1.** `FUN_0023cd40` → `FUN_0023cc20` → `FUN_0023c698`. The verifier found it; I re-read it.

**Absence.** No BF1 function draws a U(0.2·x, 2·x) re-arm. That holds over all 20 callers of BF1's uniform draw
`FUN_00232420`, every B1 `* 0.2` / 0x3e4ccccd site, and the AI range's inline LCG. BF1's rule is the fixed cadence
instead.

**Verifier.** One Opus verifier, in the foreground, given the claims and citations only (`verify/claims.md`), returned
**17 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED** (`verify/VERDICTS.md`). Folded in:
- **B1:** the port's bot releases FIRE incidentally on any tick `want_fire` fails (C4 above).
- **C4:** the per-class ranges and the multi-bolt caveat.
- **A8:** BF1's twin exists (it was "not found"), with its own rule.
- **A6:** the citation is re-ai-sniper §0 item 2 and §2's tables, not §1.
- **A5:** the lerp's argument is the skill fraction.
- **A7:** the range ends at 211559.
- **A9:** the 7 callers and the getter come from the decomp and ELF, not RE_LOG. The 0.04 test is XZ.
- **C1:** the command needs `../extracted`.
- **Side fix:** `re-ai-difficulty.md` line 61 said bit 0x35 (it is 0x32), and line 207 called the tap a "hold". Both are
  corrected in this commit.

## 8. Re-measured on main 4603e9e (2026-10-01, after STFAL, STHOP, GRND, KSTK, KFLR, BNDS, RAIL, AICLW, HOLD)

The maintainer, 2026-10-01: "some maps the games go really long or won't finish at all". A port-side measurement only: no new PS2
run and no RE. Evidence: `engine/artifacts/lead_tmp/endings/README.md` (sweep, per-unit samples, scripts).

**Instrument.** `sweep_seeds.py --seeds 1-8 --ticks 120000`: 32 maps × 8 seeds at twice the gate's 60,000 ticks (66.7
game-minutes), so a slow round and a round that never ends separate. `UNIT_SAMPLES` runs of DEA1, KAM1, POL1, HOT1 and
YAV1 give one row per unit per second.

**Result.** 245 of 256 rounds end. The kill rate decides it: every map that ends runs 10–37 kills/min.

| map | finished /8 | round minutes | kills/min (PS2, §4) | cause |
|---|---|---|---|---|
| BF2 DEA1 | **0** | all cut at 66.7 | **0.4** (14.0) | C3, unchanged: card STDOR |
| BF2 KAM1 | 7 | 35–58, one cut | **3.3** (17.3) | **C10 (new)**: card OFFG |
| BF1 YAV1 | 6 | 8–12, two cut | 9.7 | C2 + C7 at a new spot: cards STLVL, RCYC |
| BF2 GEO1 | 8 | 8.5–19.4 | 10.5 | C4, C5: cards STTAP, STARC |
| the other 28 | 8 each | medians 7–12 | 16–37 | — |

**C3 on DEA1 is now the whole map.** Seed 1 ends 116 v 16 tickets with 16 v 16 alive. Team 0 walks in place at
(−84, 50, −108), blast door 0; team 1 at (−60..−66, 50, −72), blast door 2. An enemy is within 60 m 79 % of the time,
through a door. `stuck_max` and STUCK_CENSUS do not see it (0 units over 5,000 ticks): the door pushes units back and
forth, which resets the stuck window. The measure that does see it is net displacement — speed over 1 m/s and under 1 m
moved in 10 s — at 15 % of unit-seconds.

**C10 — off the graph (new, KAM1).** Share of alive unit-seconds by `NavGraph::locate`, seed 1:

| map | hub | arc | Nearest (on neither) | steer `slide` | net m/s over 10 s, sliding · forward |
|---|---|---|---|---|---|
| BF2 KAM1 | 31 % | 22 % | **47 %** | **42 %** | 0.82 · 2.17 |
| BF2 POL1 | 11 % | 57 % | 32 % | 26 % | 1.02 · 3.07 |
| BF2 HOT1 | 27 % | 70 % | 3 % | 12 % | 1.58 · 4.98 |

Inside locus Nearest on KAM1, 64 % of the time is a wall slide. The bots reach each other slowly: an enemy within 60 m
20 % of the time (POL1 56 %), 2.0 AI shots/s (HOT1 11.5) at a hit rate of 0.15 (HOT1 0.22). So KAM1's low kill rate is
contact, not aim. Not split yet: how much of the 47 % is spawn nodes, HOLD positions, the BNDS return walk, or hub and
arc coverage. The PS2's own share can be read by running §4's KAM1 samples through the port's `locate` [H until done].

**C2 + C7 on BF1 YAV1.** Seeds 2 and 8 end 3 v 1 and 1 v 1 with the posts split. Team 0's last unit slides at
(107.4, −62.7, −136.1), on the arc at hub 17 and 3.3–3.9 m under that hub's floor, for 3,424 s on seed 8; team 1's last
unit defends cp3 34 m away with the line of sight blocked. BNDS's 45 s recycle does not open: the unit is on the level.

**Still as §2 found them:** C4 (nothing under `core/src/ai*` reads `trigger_single`) and C5 at POL1's radar door (the
top walking-in-place spot on seed 1, 456 unit-seconds).

**The order the maintainer set (cards' `first:` field):** STDOR, RCYC (a last-resort recycle whose census lists every use as a
defect to close), OFFG, STLVL, STTAP, STARC, STEND. The disc's opt-in conquest time limit (`re-time-limit.md`) already
ends any round for a player who sets it; it stays Off by default.
