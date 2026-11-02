# OPUS RE (2026-10-07) - [O] UNAUDITED

# Vehicle spawn pads at battle start: which class a pad holds, the post-load tick, the first countdown

Card VPAD2. Question: BF2 Hoth's CP6 pad (`vehiclespawn_8`, `ClassImpAtk = imp_walk_atst_snow`) showed an AT-ST in the
PS2's 0-AI frames and nothing in a port probe. What does the PS2 do at that pad, and does the port's pad setup differ
from the PS2's anywhere else? Extends `re-vehicles.md` §7 (the BF1 pad) with the BF2 twin and the load order.

**Answer.**
1. The port's real battle spawns that AT-ST exactly as the PS2 does: 45 s (`SpawnTime`) after the battle starts. The
   probe that found "nothing" skipped the mission script and stepped two ticks. No bug at this pad.
2. The census the card asked for found a different bug. The PS2 resolves a pad's class against the classes the mission
   script loaded; a name no loaded class answers is a null slot. The port resolves against the whole side file. On
   **11 of 228** BF2 pad / team slots the port spawns a vehicle the PS2 never has: HOT1 (showcase) 1, GEO1 5, NAB2 GCW
   4, Mos Eisley CW 1. BF1: 0 of 492.
3. Both games tick every pad once with a time step of 0 in the post-load step, and BF2 does it BEFORE `ScriptPostLoad`.
   A post the script re-owns therefore gives its pad one spawn for the WORLD file's owner first. Two pads in the corpus
   are affected, both on BF2 Hoth: `vehiclespawn_6` (a Rebel tauntaun stands at the Empire's CP3 and decays in about
   20 s) and `vehiclespawn_8` (no class for the world's owner: nothing, then the AT-ST after 45 s).

Tags: [V] read (lines cited, function checked with `re_check.py fn`), [D] derived, [H] hypothesis, [R] read from PS2
RAM in a save state, [M] measured on the port. Load-bearing claims carry [O].

## 1. The BF2 pad (the twin of `re-vehicles.md` §7)

Instance constructor `FUN_003716a8` (B2:369302), setter `FUN_00371990` (B2:369441), update `FUN_00373138` (B2:370596),
spawn step `FUN_003725f0` (B2:370074), decay `FUN_00371ce0` (B2:369595).

| field | BF2 | BF1 | default |
|---|---|---|---|
| vtable / update table | 0x62d1b0 / 0x62d190 at +0x18 | 0x53f040 / 0x53f020 at +0x18 | |
| ring of all pads (sentinel, count) | 0x69c050, 0x69c060 | 0x589a68, 0x589a78 | |
| `ControlZone` post | +0x74 | +0x74 | 0 |
| `SpawnCount` | +0x7c | none | 1 |
| `SpawnTime` | +0x80 | +0x78 | 10.0 |
| idle thresholds (class changed / outside) | +0x84 / +0x88 | +0x7c / +0x80 | 10.0 / 20.0 |
| `DecayTime` | +0x8c | +0x84 | 10.0 |
| class per team [8] | +0x90 | +0x88 | 0 |
| its vehicles | ring +0xd8, count +0xe8 | one handle | |
| team at the last spawn | +0xf4 | +0xf8 | -1 |
| team now (`Team`, then the post's) | +0xf8 | +0xfc | -1 |
| armed (the countdown is loaded) | byte +0xfc | +0x100 | 1 |
| countdown | +0x100 | +0x104 | 0 |
| the decay's 1 s timer | +0x104 | +0x10c | 0 |

- [V][O] **Class per team** (B2:369478-369503). For team t = 0..7 the key must equal row[side(t)][0] or, for t < 3,
  row[side(t)][t] of the table at 0x5feeb0 - the same 21 words as BF1's 0x5183a0 (`ClassNeutral` x3, then
  `Class<SideName>`, `Class<Side>Atk`, `Class<Side>Def` for Alliance, Empire, Republic, CIS, Locals, Historical). The
  value's name hash is then searched in the loaded-class list `DAT_00688e84` (class+0x14); **no match stores 0**
  (B2:369497-369503). BF1 the same (B1:239878-239891).
  - The historical override (teams 1, 2 forced to row 8, an all-zero row in BF2: B2:369481-369483; row 6, the
    `ClassHis*` row, in BF1) is off in every kept state: [R] the flag byte is 0 in 188 BF2 and 161 BF1 states. BF2
    clears it in the constructor (B2:365717) and sets it only in the Lua function `SetHistorical` (B2:32180), which no
    BF2 mission script calls; 15 BF1 `_h` scripts (the dropped historical campaign) do.
- [V][O] **`ControlZone`** binds the post by name (+0x74) and, when the post's entity exists, copies the entity's team
  into +0xf8 (B2:369537-369590). So a stock pad's load-time team is its post's WORLD team (`Team` is authored first,
  `re-vehicles.md` §7).
- [V][O] **Update** (B2:370676-370752): with a bound, existing post entity the reference team is the entity's team, or
  0 when the entity is not alive; when the pad's team differs, countdown = `SpawnTime` and team = the entity's. Then
  each vehicle of the ring is dropped (dead) or decayed, and when `count < SpawnCount` and the class slot of the pad's
  team is non-null the spawn step runs.
- [V][O] **Spawn step** (B2:370228-370288, 370578-370591): nothing while one of the pad's own live vehicles overlaps the
  pad box (BF2 only); if not armed, arm and load `SpawnTime`; `countdown -= dt`; spawn when it is <= 0. A spawn clears
  `armed` and records the team (+0xf4). A failed create retries in 1.0 s (B2:370374).
- [V] **Decay** (B2:369690-369756, 369779-369892, 369897-369902, 369995-370014): the BF1 rule. First the exits that
  reset the idle count and do nothing: a vehicle inside the map's boundary list (`FUN_0034c550`, the list
  `SetOutOfBoundsVoiceOver` reads) whose pad has no live post, or that has a team (bits 32..35 of +0x268 non-zero);
  any vehicle with an occupant. Then the threshold: `+0x88` (20 s) when the pad has no live post, the post entity is
  not alive, or the vehicle is outside the post's region (`FUN_00359bb8`); `+0x84` (10 s) when the class slot of the
  spawn-time team differs from the class slot of the team now; no decay otherwise. Once a second (the pad's +0x104),
  unless a player's unit stands within radius + radius + 10 m, the entry's idle count grows by 1; past the threshold
  the vehicle takes MaxHealth / `DecayTime` per step. This closes §7's "[H: same as B1]". BF2 keeps the idle count per
  vehicle entry (+0x18), BF1 on the pad.

## 2. The post-load tick [V][O]

- BF2 `FUN_00424e10` (B2:482078) walks the pad ring and calls each pad's update with **f12 = 0.0** (B2:482156-482157;
  ELF 0x424ff8-0x425018: `lw v0,-16300(v0)`, `mtc1 zero,$f12`, `lw a0,12(v0)`, `addiu a0,a0,4`, `lw v0,20(a0)`,
  `lw v0,12(v0)`, `jalr`). The slot is 0x62d19c = `FUN_00373658` -> `FUN_00373138(this - 4)` (B2:370879-370886).
- Later in the same function: `FUN_001a5418(0x636648)`, the string "ScriptPostLoad" (B2:482293). The mission state's
  enter (0x175af8, not in the export) runs 0x174b28, then `FUN_004241b0` ("ScriptInit", B2:481913: the world LVL is
  read here), then `FUN_00424e10` (jal at 0x175b1c / 0x175b24 / 0x175b2c).
- So the order is: world loaded (pads take their posts' world teams) -> **every pad ticked once, dt = 0** -> the
  script's `SetObjectTeam` / `KillObject` -> frames. `SetObjectTeam` (B2:34753; Lua table word 0x5f9ef4) calls the
  entity's slot +0x98; a command post's (table 0x62a928) is `FUN_00351398`, which writes the team at once
  (B2:347168-347170, `FUN_00268f20` B2:185835). The pad sees it on its next update.
- BF1 twin: `FUN_0033ef20` (B1:328158-328182; ELF 0x33ef98 `mtc1 zero,$f12`), called once from the mission enter
  `FUN_00174bd8` (B1:16632). BF1 has no `ScriptPostLoad` (no such string in the ELF) and no script re-owns a post, so
  there the tick only makes the first vehicles exist before the first frame.
- [D] A pad armed by the constructor with countdown 0 spawns in this tick, and `0 - 0.0` leaves the countdown at
  exactly +0.0. [R] All eight HOT1 pads that hold a vehicle at the start read 0x00000000 there.

## 3. BF2 Hoth (hot1g_con) in PS2 RAM [R]

`engine/artifacts/lead_tmp/vpad2/pads_ram2.py`; full dump `ram_hot1.txt`. The script: `SetObjectTeam("CP3", 1)`,
`SetObjectTeam("CP6", 1)`, `KillObject("CP7")`; `ReadDataFile("SIDE\\imp.lvl", ...)` lists `imp_walk_atat` and
`imp_walk_atst_snow`, not `imp_walk_atst` (PS2 bytecode, `scripts/gen_missions.py`). All five `hoth_cps` posts author
`Team = 2`.

| pad (props) | 0-AI side select (`bf2_hot1_ai0_one_sideselect`) | walk state (`bf2_hot1_walk_cp1_cp6_b000`) |
|---|---|---|
| `vehiclespawn_8`: Team 1, CP6, ImpAtk = imp_walk_atst_snow | post cp6 (team 1); team 1; armed 1; countdown **16.174**; no vehicle; last spawn team -1 | AT-ST alive at (32.57, -2.71, 27.44); armed 0; countdown -0.027; last spawn team 1 |
| `vehiclespawn_6`: Team 0, CP3, ImpAtk = imp_walk_atst, AllDef = all_walk_tauntaun | post cp3 (team 1); team 1; **class slot 1 null**, slot 2 tauntaun; armed 0; countdown 45.000; no vehicle; **last spawn team 2** | the same |
| `vehiclespawn_9`: Team 0, ControlZone `shield` | bound to `shield` (a post-class entity, team 2); tauntaun alive | the same |
| the 4 snowspeeders, tauntaun `_15`, both AT-ATs | one vehicle each; armed 0; countdown +0.0 | the same |

- [D][O] `vehiclespawn_8`: world team 2 has no class, so the post-load tick spawns nothing; the first frame's update
  sees CP6 at team 1, reloads 45 s, and the AT-ST arrives 45 s into the battle (28.8 s had run in the first state).
- [D][O] `vehiclespawn_6`: the post-load tick spawned a tauntaun for world team 2 (armed 0, last spawn team 2). CP3 then
  went to team 1, whose slot is null: the 10 s class-changed rule decays it (11 idle steps, then 10 of MaxHealth / 10:
  dead near 20 s), and nothing spawns for the Empire after. [R] It is gone at 26.1 s (`bf2_hot1_sideselect`, the same
  pad state, 18.94 s left on `vehiclespawn_8`) and at 28.8 s; the 20 s rule would keep it to about 30 s. So the pad
  stands inside CP3's control region on the PS2 (`CP3_Control`, a box whose centre is 58 m from the pad in x; the
  port's region test must agree, or its tauntaun lives 10 s longer).
- `imp_walk_ATAT` (upper case in the world file) resolves: the name hash folds case.

## 4. The port [M]

Scratch examples (kept as `engine/artifacts/lead_tmp/vpad2/examples/*.rs`, built in a scratch worktree at 08d6343):
`vpad2_probe <map> <mission.ron> <seconds> [--no-mission]`, `vpad2_census`.
- With the mission (`port_hot1_mission.txt`): `vehiclespawn_8` armed, countdown 44.9 at 0.1 s; **spawns
  imp_walk_atst_snow at 45.03 s**. Matches the PS2.
- Without it (`port_hot1_nomission.txt`): 9 vehicles at once, a tauntaun at `vehiclespawn_6`, never an AT-ST at CP6 -
  the card's original observation, vehicle for vehicle. That probe (`lead_tmp/vdraw/vdraw_pose.rs`) did not call
  `MapLayout::apply_mission`.
- **Differs from the PS2:** `vehiclespawn_6` spawns **imp_walk_atst at 45.03 s** (the PS2: null slot, never); no
  tauntaun at the start; `vehiclespawn_9` has no post (`shield` is not in the port's roster).

## 5. Census: the PS2's class table against the port's [R][M][O]

`census2.py` / `census1.py` beside the reader: every kept state's pad ring, grouped by mission (BF2: the mission string
at 0x69a638; BF1: map + the two teams' sides - BF1 keeps its script name at 0x589688, and it is the assumed script in
every state), against `port_census.txt` and the mission bytecode's `ReadDataFile` lists. A PS2 pad is matched to the
nearest port pad: all 360 match a distinct port pad within 0.02 m.

| | missions | states | pads | team-1/2 slots | PS2 non-null | port same | port only | PS2 only | rule fits |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| BF2 | 19 | 121 | 114 | 228 | 154 | 217 (154 a class, 63 both empty) | **11** | 0 | 228 / 228 |
| BF1 | 21 | 124 | 246 | 492 | 243 | 492 (243 a class, 249 both empty) | 0 | 0 | 492 / 492 |

- Every state of a mission holds the same table. The rule: **the port's class, kept only when the script's
  `ReadDataFile` calls list it or the map's own LVL defines it** (24 BF1 slots need the map clause: `bes_fly_cloudcar`,
  `tat_hover_skiff`).
- The 11 slots the port fills and the PS2 leaves null:

  | mission | pad | team | class the port spawns |
  |---|---|---|---|
  | hot1g_con | vehiclespawn_6 | 1 (Empire) | imp_walk_atst |
  | geo1c_con | ift1, ift2, ift3 | 1 (Republic) | rep_hover_fightertank |
  | geo1c_con | cp5hailfire3, cp5hailfire4 | 2 (CIS) | cis_hover_stap |
  | nab2g_con | cp3_vspawn x2, cp1_vspawn, cp1_vspawn1 | 2 (Alliance) | all_hover_combatspeeder |
  | tat2c_con | com_item_vehicle_spawn | 2 (CIS) | cis_hover_aat |

- A class key on one side only is ordinary: BF2 pads with a class for 0 / 1 / 2 of the two fielded teams are
  11 / 52 / 51 on the PS2 (port 3 / 57 / 54); BF1 39 / 171 / 36 on both.
- Posts the mission re-owns that a pad follows: only HOT1's CP3 and CP6 (KAM1's re-owned posts have no pad).
- Pads bound to a post-class object the port's layout lacks (the port gives them no post): BF2 HOT1 `vehiclespawn_9`
  (`shield`); BF1 HOT1 `vehiclespawn_9`, `_10` (label G), END1 `v_spawn3`, `v_spawn11` (S), GEO1 `laat1`, `laat3` (A).
  Same vehicles at the start; they would differ once that object dies or changes team. Not built.

## 6. What the build needs

1. **Loaded classes.** `scripts/gen_missions.py` writes each script's `ReadDataFile` class lists into the mission RON;
   `vehicle_pads::build` leaves a slot null when the class is neither listed nor a class of the map's own
   `classes.ron`. Referee: both census scripts print `port_only 0`; the pad test pins the 11 slots.
2. **The post-load tick.** A pad's first tick runs its spawn step under its load-time (world) team and follows its post
   from the second tick on. Only `bf2_hot1` moves: a tauntaun at `vehiclespawn_6` from the first tick, dead near 20 s.

## Checks run

- `re_check.py fn` on all 39 cited B2 lines and 5 B1 lines: each in the function named.
- `re_check.py word`: 0x636648 "ScriptPostLoad", 0x636618 "ScriptInit"; 0x62d19c = 0x373658; 0x62a9c0 = 0x351398;
  0x5f9ef0 / 0x5f9ef4 = "SetObjectTeam" / 0x18fa50; both class tables (21 words each, BF2 rows 7, 8 zero).
- `re_check.py absent`: `DAT_0069c054` appears in three B2 functions - read by `FUN_00371938` (teardown) and
  `FUN_00424e10`, written by `FUN_00373678` (the ring's static init); `DAT_00589a6c` in four of B1. `refs.py`:
  `FUN_00373658` only as the table word 0x62d19c; `FUN_0033ef20` one `jal` (0x174c38). Byte search: "ScriptPostLoad"
  0 times in the BF1 ELF, once in BF2's.
- Twin: BF1's constructor, setter and post-load loop opened; its callers walked.
- Flags by RAM, not by writers: the historical byte and the network bytes 0x600548 / 0x600549 are 0 in every kept state.
- Disassembly: `scripts/tools/eedis.py` (0x175af8, 0x424f60-0x425020, 0x33ef20-0x33efd0).
- Verifier (one Opus agent, claims and citations only; `engine/artifacts/lead_tmp/vpad2/verify/VERDICTS.md`): 42 claims,
  **40 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED**. The two, both corrected above: the ring-head wording (one of the three
  functions only writes it) and the decay, whose early exits and third 20 s case were missing. It re-read the RAM with
  its own reader, re-ran both censuses and wrote its own by hash (same totals), and found no second place that updates
  a pad before `ScriptPostLoad` and no other writer of a pad's team (constructor, `Team`, `ControlZone`, the follow).

## Still [H]

- The object `ControlZone = shield` binds (a post-class entity outside the conquest roster): what it is to the battle,
  and whether its death stops its pad. Not opened.
- The convex-region list the decay tests first (`FUN_0034c550`, BF1 `FUN_002956d8`): unnamed, as in §7.
- BF2's spawn step ends with a team-bonus test that takes 75 % of MaxHealth from the new vehicle
  (B2:370533-370566, `FUN_0034b1b0(0x69a248, enemy, 0xd265e905)`): it looks like BF1's sabotage, which
  `re-vehicles.md` §7 says BF2 lacks. Read, not checked - a point for that section's next audit.
- When the tauntaun at `vehiclespawn_6` dies to the second: derived from the decay rule and bracketed by one state.
