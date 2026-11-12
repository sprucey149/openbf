# OPUS RE (loop lane 3, 2026-10-02, card CWSP) - [O] UNAUDITED: no Fable pass
# RE: the command walker's post — an AT-AT / AT-TE is a moving command post its team spawns at

Card CWSP asked: where a team spawns at a command walker, who may and when, whether the spawn screen lists it, and what
the walker's death costs the team — both games. Read from both decomps and ELFs, the discs' classes, pads and mission
scripts, the RAM of six reference save states, the WAI2 live rows (five runs) and one live PCSX2 session on BF2's spawn
screen. Tags: [V] read (lines cited), [D] derived, [H] hypothesis, [L] seen on the running PS2 (RAM or live rows);
`[O]` marks the load-bearing claims. B2 / B1 = decomp lines of `SWBFRNT2.ELF-decomp.c` / `SWBFRONT.ELF-decomp.c`.
Scratch, tools and raw outputs: `engine/artifacts/lead_tmp/cwsp/` (README there). Earlier docs this builds on:
`re-cp-capture.md` (the post machine; four of its statements are corrected in section 9), `re-vehicles.md` section 7.1
(a vehicle's team), `re-walker-ai.md` section 13 (the measurement that opened this card), `re-tickets-spawn.md` (bleed).

## 0. The answers [O]

1. **The walker owns an ordinary post.** The command walker's constructor asks the command-post factory for a post
   and keeps the pointer (B2 `FUN_0035baa0` → `FUN_00354920`, E+0x2290; B1 `FUN_002a26e0` → `FUN_0029c788`, E+0x23e0).
   It is one more entry of the 16-slot post list — same struct, same tick, same eligibility test as a fixed post. Its
   class component sits inside the walker's class (B2 class+0xfd0, B1 class+0x28e0).
2. **Six spawn points in the walker's own frame.** The class authors `SpawnPointCount 6` and six
   `SpawnPointLocation "x y z angle"`: AT-AT (±8, 0, {0, +4, −4}) facing 5° / 355°; AT-TE (±6, 0, ±4) and (±7, 0, 0)
   facing 0° (both games, identical). A spawn draws one of the six at random and transforms it by the walker's LIVE
   matrix; a ray from the walker's bounding-sphere centre to that point (the walker itself excluded) pulls it back to
   the first terrain / building hit; the result is stood upright (yaw only). The walker's origin is at ground level,
   so the points are on the ground 8 m to each side. On the PS2: BF1 Hoth 14 of 14 opening spawns within 0.13 m of an
   authored point (median 0.00 m); BF2 Hoth 9 appearances in the 60 s run and 18 in the 240 s run (median miss 0.35
   and 0.40 m), to 395 m from the pad; BF2 Geonosis 5 and 7, all within 0.52 m. (The two runs of a map start from one
   menu state and their opening draws are the same: 6 on Hoth, 2 on Geonosis.)
3. **Who and when.** A team may spawn there iff the post is scheduled (the walker was initialised and has not been
   killed) and the walker's team equals the team. Allies never (no AllyPath). The walker's team is its pad's team from
   the moment it spawns and it does not go neutral when empty (`re-vehicles.md` 7.1), so the post belongs to the pad's
   team for the walker's whole life. It has no capture region: it cannot be neutralised or captured.
4. **The spawn screen lists it, in both games, and the player can take it.** Neither spawn map tests the owner's type:
   the D-pad moves between scheduled posts of the player's team. Live: on BF2 Hoth one `Right` put the player's chosen
   post on an AT-AT, and accepting an AT-AT's post spawned the player on that walker's x = −8 line; BF1's reference
   state shows "COMMAND POST: AT-AT" selected. The map icon is the class's `MapTexture` / `MapScale` at the walker's
   live position, turned with its heading; BF2 prints no number on it (`HUDIndexDisplay 0`).
5. **The AI treats it like any post.** One draw per death (4.0 s after it), kept until the spawn: BF1 uniform over the
   team's eligible posts (`FUN_00261e80`); BF2 weighted by the post's `AISpawnWeight` (`FUN_002e7240`; default 100; no
   Hoth or Geonosis conquest post overrides it), so uniform there too. Live shares: BF1 Hoth 11 / 9 / 6 over the
   fixed post and the two AT-ATs; BF2 Hoth (240 s) 11 / 9 / 8 / 11.
6. **Its death.** The walker's kill handler kills the post: unscheduled (so it leaves every pick at once), event 5
   ("post lost") for the walker's team, the post-kill listeners (BF2), the live-post count down by one. The pad builds
   a new walker after `SpawnTime` (Hoth 120 s, Geonosis 45 s) and the new walker takes a post again.
   - **BF2: no ticket or bleed effect.** Bleed and the all-posts check are the mission script's, over the posts it
     registered (Hoth: CP3, CP4, CP5, CP6); a walker's post is not among them. The engine does not read plain
     `ValueBleed` in BF2 at all. BF2 also skips the post-kill award for the three command vehicle types.
   - **BF1: the walker's post counts.** The team update sums `ValueBleed` over every scheduled post the team owns and
     the AT-AT's is 10: Hoth's Empire stands at 30 (rate 0), 20 with one AT-AT lost (0.1875 tickets/s) and 10 with
     both lost (0.5625 tickets/s) until the pad rebuilds them; and a team whose only posts are its walkers is not on
     the 20 s "no posts" countdown. The port plays BF2's rule on every map (PLAN, Conquest), so this is recorded, not
     built.
7. **A hostile walker's post is an AI target candidate (BF2).** The conquest goal's target pick takes every scheduled
   post not friendly to the bot's team, weighted by `Value_ATK`; the walker classes author `Value_ATK` 10 (a fixed
   post's 10) and `Value_DEF` 0. A post also has to pass a planning-graph test at its position, which for a post with
   no capture region is the walker's live position (not opened [H]). So on Hoth the AT-ATs are up to two of the Rebels'
   four attack targets, and the Empire never picks its own walker to defend. A note for card CONT.

## 1. What the discs ship [V]

`python3 engine/artifacts/lead_tmp/audit-vturb/dumpclass.py bf1|bf2 imp_walk_atat rep_walk_atte`:

| class (BASE commandwalker) | SpawnPointLocation ×6 (x y z angle°) | ValueBleed | Value_ATK All/CIS/Imp/Rep/Loc | Value_DEF | other |
|---|---|---|---|---|---|
| `imp_walk_atat` (SIDE/IMP.LVL, both games) | 8 0 0 5 · 8 0 4 5 · 8 0 −4 5 · −8 0 0 355 · −8 0 4 355 · −8 0 −4 355 | 10 | 10 / 10 / 10 / 10 / 0 | 0 ×5 | MaxHealth 30000 (BF1), 60000 (BF2); Label AT-AT; MapTexture atat_icon; MapScale 3.5; BF2 `HUDIndexDisplay 0` |
| `rep_walk_atte` (SIDE/REP.LVL, both games) | −6 0 4 0 · 6 0 4 0 · −7 0 0 0 · 7 0 0 0 · −6 0 −4 0 · 6 0 −4 0 | 10 | same | same | MaxHealth 15000 (BF1), 40000 (BF2); MapTexture atte_icon; MapScale 2.2; BF2 `HUDIndexDisplay 0` |

BF1's two `commandhover` classes (`all_hover_hovernaut`, `cis_hover_mtt`; Rhen Var Harbor only) author the AT-AT's six
points and the same values. Two key names cracked here: 0xC069B070 = `HUDIndexDisplay`, 0x66BCDAEA = `HUDIndex`
(FNV-1a-32 over bytes | 0x20; both were "uncracked" in `re-cp-capture.md` section 9).

The pads (`python3 engine/artifacts/lead_tmp/cwsp/pads.py`; every command vehicle enters through a
`com_item_vehicle_spawn`, none is placed directly):

| map (world) | pad | position | forward (pad row 2) | Team | SpawnTime | ControlZone |
|---|---|---|---|---|---|---|
| BF1 HOT1 | vehiclespawn_21, vehiclespawn_01 (AT-AT) | (220.60, −6.58, 51.11), (158.17, −5.64, 119.58) | (−0.997, 0, −0.081), (−1, 0, 0) | 1 | 120 | – |
| BF1 GEO1 | atte1, atte2 (AT-TE) | (11.61, −62.33, 220.18), (72.63, −59.19, 223.03) | (0, 0, −1) | 1 | 45 | – |
| BF1 RHN1 | commandspawn01 (hovernaut / MTT / AT-AT / AT-TE by side) | (−216.38, 0.00, 239.62) | (0.998, 0, 0.059) | 1 | 45 | CP2 |
| BF2 HOT1 (hoth_cps) | vehiclespawn_21, vehiclespawn_01 (AT-AT) | (228.79, −5.00, 51.06), (206.37, −4.05, 119.54) | (−0.997, 0, −0.081), (−1, 0, −0.003) | 1 | 120 | – |
| BF2 GEO1 (geo1_conquest) | atte1 (AT-TE) | (9.53, −62.00, 325.89) | (0, 0, −1) | 1 | 45 | – |

BF2's FEL1 and GEO1 campaign worlds hold two more AT-TE pads (`ControlZone atte_cp`); the port plays conquest.
Missions: BF2 `hot1g_con` registers CP3, CP4, CP5, CP6 with the conquest objective (`CommandPost:New` ×4,
`AddCommandPost` ×4), kills CP7, and sets `SetMemoryPoolSize('CommandWalker', 2)`; BF1 `hot1i` sets the attacking team
1 = Empire, 25 units and 250 reinforcements a side, and the bleed thresholds (31, 0), (21, 0.75), (11, 2.25), (1, 3).

## 2. The object: a walker that owns a post [V][O]

| | BF2 | BF1 |
|---|---|---|
| entity ctor | `FUN_0035baa0` (B2:354287-354310): EntityWalker ctor `FUN_00259970`, vtable 0x62be40, `FUN_00354920(E, class+0xfd0)` → E+0x2290 | `FUN_002a26e0` (B1:230396-230421): `FUN_00215c88`, vtable 0x53e2b0, `FUN_0029c788(E, class+0x28e0)` → E+0x23e0 |
| class ctor | `FUN_0035bd30`: walker class ctor `FUN_00263160`, post class ctor `FUN_00359df0(class+0xfd0)`, clears class+0x330 bit 34 | `FUN_002a2968`: `FUN_0021cca8`, `FUN_002a0b28(class+0x28e0)`, class+0x80 = 0 |
| class setter | `FUN_0035be90`: `FUN_00263f68` then `FUN_0035a3a0(class+0xfd0, …)` | `FUN_002a2a98`: `FUN_0021d980` then `FUN_002a1138(class+0x28e0, …)` |
| instance setter (vtable +0x18 / +0x14) | `FUN_0035bc88`: `FUN_00268c78` then the post's `FUN_00355950` | `FUN_002a28c8`: `FUN_00221420` then `FUN_0029d1d0` |
| init (vtable +0x1c / +0x18) | `FUN_0035bbf8`: `FUN_0025a6c8` then **activate** `FUN_00355648` | `FUN_002a2848`: `FUN_002167f8` then `FUN_0029d130` |
| kill (damage sub-vtable +0x0c) | `FUN_0035bc28`: `FUN_0025b020` (clears the alive bit) then **kill** `FUN_00354e10`, E+0x2290 = 0 | `FUN_002a2878`: `FUN_00217040` then `FUN_0029cc00`, E+0x23e0 = 0 |
| SetTeam (vtable +0x98 / +0x8c) | `FUN_0035bcd8`: `FUN_00268f20` then the post's team-changed `FUN_00359c08` | `FUN_002a2918`: `FUN_00221498` then `FUN_002a0738` |
| pool | 0x69a520, 0x22a0 bytes, `SetMemoryPoolSize('CommandWalker', n)` | 0x589630, 0x23f0 bytes |

The cleared class flag (B2 class+0x330 bit 34, B1 class+0x80) is `re-vehicles.md` 7.1's: a command vehicle takes its
pad's team at spawn and keeps it with nobody aboard. BF2's entity vtable 0x62be40 differs from EntityWalker's 0x61f850
at 14 of its 82 words: the type tests (+0x08, +0x0c, +0x10), the four hooks above plus the destructor (+0x14), +0xe0,
and six past word 60 (+0xf8 … +0x110). Slot +0x110, `FUN_004183d8`, rebuilds the walker and copies the post's state
across; its body runs only when `DAT_00600562 == 0` (ELF default 1, RAM 1) from a network path — inert in single player.

**The factory** (B2 `FUN_00354920`, B1 `FUN_0029c788`): with no index forced (`DAT_005fed1c` / `DAT_00518334` are −1
outside the network reader), reuse the list entry whose owner is gone, whose class component is this class's and whose
stored position (B2 cp+0x40, B1 cp+0x30) is within 1 m of the new owner; else take index = the current count and raise
the count; construct the post there if the slot is empty (B2 0xb50 bytes `FUN_00354290`, B1 0x6a0 `FUN_0029c148`);
attach the owner (pointer, id, position copy). The constructor copies the class component's bleed and ATK / DEF values
into the instance, sets BF2's flags to 7 with bit 1 from the class's `HUDIndexDisplay`, and `AISpawnWeight` 100.0. [L]
The stored position is the attach-time one, not updated: BF2 RAM (228.8, −5.7, 51.1) and (206.4, −4.1, 119.5) beside
walkers that had moved, BF1 (220.6, −6.9, 51.1) with the walker 47 m away — so a walker rebuilt on its pad gets its
old slot back [D].

**The post is an update task.** Scheduled = live. Activate (B2 `FUN_00355648`, B1 `FUN_0029d130`): if not scheduled,
register the map icon, schedule, live count + 1 (BF2 also fires the respawn listeners 0x720dcc). Kill (B2
`FUN_00354e10`, B1 `FUN_0029cc00`): if scheduled, unschedule, event 5 with the owner's team, the killer's award, BF2's
kill listeners 0x720db4 (`OnCommandPostKill`), a kill region's explosions when the post has one, live count − 1.
Every pick iterates scheduled posts only (B2 `FUN_00354798` / `FUN_0035ba28`, B1 `FUN_0029c6c0` / `FUN_002a2670`).
BF2's kill skips the award when the owner is a CommandFlyer, CommandHover or CommandWalker (type ids `DAT_0069a420`,
`DAT_0069a480`, `DAT_0069a570`); BF1's has no such test — the AT-AT's killer gets the post award (`FUN_002a6320`).
The planner hooks both call (`FUN_002e7910` / `FUN_002e7918`) are empty in BF2: the planner keeps no post table and
reads the live list at each draw.

## 3. Who may spawn there, and when [V][O]

`CanTeamSpawnAt(cp, T)` — B2 `FUN_00356880`, B1 `FUN_0029de80` — as `re-cp-capture.md` section 6 has it: owner present,
(BF2: flag bit 0), owner's team == T and (SpawnPath set **or class SpawnPointCount > 0**), or a friendly owner and
AllyPath set. For a walker: no SpawnPath, no AllyPath (the walker is built by its pad, with no instance properties; RAM:
both 0), class count 6. So: the walker's own team, never an ally. Callers add "scheduled" (the iterators) and, when a
member keeps an earlier choice, the owner's alive bit (B2 `FUN_0036c950` / `FUN_0036ca08`, B1 `FUN_002ac640` /
`FUN_002ac6f0`).

- The owner's team is the pad's (section 2; RAM: team 1 on both Hoths and both Geonosis maps).
- BF2's flag bits 0 (spawnable) and 2 (selectable on the spawn map) are set by the constructor and cleared only by
  CommandFlyer code (`FUN_00352a88`, `FUN_00352ef0`): for a walker both stay set for life. RAM: flags 5 (bit 1 clear =
  no number).
- No capture: the post tick's unit scan returns nothing for a null capture region (B2 `FUN_00359b78`, B1
  `FUN_002a0628`), and the walker's post has none (RAM: capture, control and kill region all 0 in both games).
- An edge not chased [H]: BF2's "ready" watchdog and the member's spawn re-check only the owner pointer and
  CanTeamSpawnAt, so a player already accepted on a walker's post when it dies may still spawn at its points while the
  dead entity keeps its id.

## 4. Where: the spawn transform [V][O]

Class keys (B2 `FUN_0035a3a0`, B1 `FUN_002a1138`): `SpawnPointCount` (0x32579F4D, "%i") allocates two arrays of
count × 16 bytes; each `SpawnPointLocation` (0xA067D145, "%f %f %f %f") appends a position (x, y, z) to one and to the
other the quaternion of a turn of `angle × 0.017453294` rad about the world up axis. B2: quaternions K+0x11c, positions
K+0x120, count K+0x124; B1: K+0xe4, K+0xe8, K+0xec (K = the class component). A third key (0xC5190CBA, "%s %i") fills
the same lists from model hardpoints; no command vehicle uses it.

The pick (B2 `FUN_00356998`, one caller, the member spawn `FUN_0034cea8`; B1 `FUN_0029df70`, one caller,
`FUN_002966f0`), when the post has no usable path and the spawner is not an ally:

```
i      = lcg_draw() % count                       # two LCG steps, ((r1 >> 16) | (r2 & 0xffff0000)) % count, unsigned
local  = matrix(quat[i], pos[i])
world  = local × owner.matrix                     # the walker's CURRENT matrix (rows right, up, forward, position)
from   = owner.bounding_sphere_centre             # B2: *(E+0x10)+0x40+16·*(E+0x14), or E+0x20 when *(E+0x10) == 0
t      = segment_cast(from, world.pos, ignore = owner, mask = B2 0x90 / B1 0x48)
if t < 1: world.pos = from + (world.pos − from) × t
out    = upright frame at world.pos: up = world up, right = normalise(up × world.forward), forward = right × up
```

So the spawn faces the walker's heading plus the authored angle, yaw only, whatever the walker's pitch or roll. No
"occupied" test: BF1 Hoth put three units on (8, 0, 0) of one AT-AT within 0.8 s. [L] The bounding-sphere centre is
(−3.3, +10.0, +1.0) from the AT-AT's origin in world axes (radius 16.1 m) and (0.1, +4.2, −2.3) for the AT-TE (11.0 m)
in the BF2 states — the ray comes down from the body. The walker's origin is at its feet: a spawned unit's height is
within 0.2-0.8 m of the walker's origin height in the live rows.

## 5. The spawn screen [V][L][O]

Both spawn maps hold one selected post and move it with the D-pad / stick (B2 `FUN_0039a058`, B1 `FUN_002e03b0`): over
the scheduled posts, skipping the current one, a candidate needs the owner's team equal to the screen's team (BF2 also
flag bit 2) and a positive projection on the stick's direction from the current post; the lowest
`along + 0.25 × perp²` wins; positions are the owners' live ones. Nothing tests the owner's class. The pick is written
to the member's chosen post (B2 member+0x144, B1 member+0xb0), the one the spawn then uses (section 4).

- [L] BF2 (`lead_tmp/cwsp/ps2/`, SW session from `ref/states/bf2_hot1_spawnmap.p2s`): `Right` → the human member's
  chosen post = list index 7 (an AT-AT's; two more `Right`s changed nothing); `Down` → 8 (the other); `Down` → 4
  (CP3). The frames show the ring on the AT-AT icon (`c2_spawnmap_sheet.png`). Accepting index 8 and a class: the
  player's soldier (the human member's +0x148) stands at the walker's local (−7.62, 0.78, 0.95) — on the x = −8 line;
  the walker had walked on since the spawn, so which of that side's three points it was is not shown.
- [L] BF1: `ref/states/bf1_hot1_spawnmap.p2s` shows "COMMAND POST: AT-AT" and its human member's chosen post is index 7.
- The icon [V, reader A's citations, the BF2 number line and both movers re-read here]: BF2 `FUN_003a6a48` draws each
  registered post at its owner's live position in the owner's team colour with the owner class's `MapTexture` and
  `MapScale`, rotated by the owner's heading unless the owner's type is 1 (a walker's is 0x18), and prints `"%d"` of
  `HUDIndex` only when `HUDIndexDisplay` is set (B2:401684-401685) — the AT-AT has no number. Activating the post
  removes the walker's ordinary vehicle marker first (`FUN_001ad1c0`). BF1 `FUN_002cdb98` likewise; BF1 registers a
  map icon only for an owner class with a `MapTexture`, and names the selection by the class `Label` ("AT-AT").

## 6. Which post an AI member takes [V][O]

Both games: a death sets the member's 4.0 s timer and, for an AI, clears its chosen post (B2 `FUN_0034f650`
B2:345913-345916, B1 `FUN_00297440` B1:222156-222159). When the timer runs out the member asks for a post; the answer
is kept while the post's owner is alive and the team may still spawn there (B2 `FUN_0036c950`, B1 `FUN_002ac640`), and
is redrawn otherwise. A human goes through keep-or-nearest instead (`FUN_0036ca08` / `FUN_002ac6f0`).

- **BF1** `FUN_00261e80`: every scheduled post passing CanTeamSpawnAt, one uniform draw
  (`candidates[trunc(n × f)]`, `FUN_002324e0`; no draw at all when n = 1).
- **BF2** `FUN_002e7240` (slot +0x20 of all eight AI-goal vtables, reached through `FUN_002e78b0`): weight = the post's
  `AISpawnWeight` (cp+0x11c; ctor 100.0; instance key 0xE07282C2, clamped ≥ 0); `r = rand(0, S − 1)` and the first
  post in list order at which the running subtraction reaches ≤ 0. That function is the weight's only reader. The disc
  authors the key once as instance data (KAS2.LVL, "1000", instance `baserush`); campaign scripts set it at run time
  (`hot1g_c`: eight `SetProperty(<post>, 'AISpawnWeight', …)` calls); the conquest scripts `hot1g_con` and `geo1c_con`
  have none, so every post there keeps 100.
- Neither looks at position, value, bans, the queue counters or what owns the post.

[L] RAM, members' chosen posts (`members1.py` / `members2.py`): BF1 Hoth side-select state, Empire 24 AI: fixed post
10, AT-ATs 9 and 5 — the opening wave's own split (WAI2's "14 of 24"). BF2 Hoth, 18 AI: CP6 5, CP3 5, AT-ATs 4 and 4.
Live appearances (`spawn_share.py`): BF1 Hoth 65 s: 11 / 9 / 6 (χ² 1.5 against thirds); BF2 Hoth 240 s: CP3 11, CP6
9, AT-ATs 8 and 11 (χ² 0.7 against quarters); BF2 Geonosis 240 s: 23 of 63 at the AT-TE with four Republic posts at
the start (post ownership changed during the run; not a clean test).

### 6.1 AUDIT of card BSPN (2026-10-12) - [O] UNAUDITED

Section 6 read again for the audit of the landing 2d1fb60 (`engine/artifacts/loop/audits.md`). Two verifiers worked
separately on 17 claims with their citations only (V1: 12 supported, 4 partly, 1 not; V2: 11 / 6 / 0; they differ on
none of the points below). What stands, what is corrected, and the PS2 data the section lacked:

- **Stands [O]:** BF2's draw is `FUN_002e7240` as above: the sum at B2:272864, `FUN_002a3e48(0, S - 1.0)` at
  B2:272869 (a FLOAT in [0, S - 1), B2:225577-225583), the first post at <= 0 at B2:272874. The weight's constructor
  value 100.0 (B2:349321) and its setter (B2:350475-350478, clamped >= 0). The death's 4.0 s and the AI's cleared post
  (B2:345913-345915). ONE ask when the timer has run out, guarded by the byte member+0x164 (B2:344001-344002,
  346322-346323). BF1's uniform draw (B1:182423-182427). The node is `random % count`, unsigned in both games
  (`divu` at B2 0x356b18 / 0x356d44 and B1 0x29e0f8 / 0x29e2cc; the export's `(int)` cast is wrong); the only world
  queries of the node pick are the ground rays (B2:351104, 351110, 351221).
- **Corrected - how a held post is given up.** "Kept while the team may still spawn there, redrawn otherwise" is
  BF1's mechanism: its member update calls the keep-or-draw `FUN_002ac640` on EVERY update after the timer has run out
  (B1:221035-221036), so the held post is put to `FUN_0029de80` each tick. BF2 differs: with the byte set and a post
  held, `FUN_0034cea8` tests only the post's owner handle (B2:344093-344158, 344312-344350); the 18 call sites of
  `FUN_00356880` lie in 13 other functions, with FUN_0034cea8 and the spawn function `FUN_0034e1f0` not among them
  [receipt:d719752b]. BF2's one late test is at the top of `FUN_0034e1f0`: the friend byte of the member's team toward
  the post's owner (B2:344888-344889). Where it fails the spawn is abandoned, the post cleared and `FUN_0034ffe0` run
  again - class AND post drawn anew (B2:345008-345011) - and the member passes the wave gates once more. The outcome is
  BF1's (a bot does not spawn at an enemy's post). Bit 0 of post+0xb40 and the spawn path are left untested by BF2 at
  that point. [H] a post gone neutral: the friend byte toward the neutral team was not traced.
- **Corrected - what clears the post.** B2:345009 is that abandon branch, not the spawn: after a successful spawn
  member+0x144 still holds the post (its three stores in the member code are the constructor, `FUN_0034f2c0` and the
  death). Each life draws afresh because the DEATH clears it. BF1 likewise: the post is cleared at B1:222156-222159.
- **Not a twin:** BF1 `FUN_00261f88` (called from the posts' set-team through `FUN_002a0738`, B1:229000) re-chooses
  the bot's GOAL post (member+0x18, `FUN_00297e08` B1:222584-222607), not the spawn post (member+0xb0). BF2's hooks at
  the same two places are empty: `FUN_002e7910` / `FUN_002e7920` are `jr ra; nop` (B2:353007, 350239 call them).
- **The alive cap [O]:** at `DAT_006004fc` (ELF default 16) alive AI the timer goes back to 5.0 s with post and class
  kept (B2:344241-344243); an earlier cap (team+0x24 <= team+0x38, B2:344082-344084) returns with the timer as it is.
- **[L] PS2 data that tell the two rules apart.** The Hoth counts above cannot: against a flat list of nodes (BF1
  cp6 4 nodes and 6 per AT-AT; BF2 cp3 7, cp6 4) they read p 0.10 and p 0.72. Six independent captures made for
  other cards (BF2 Dagobah x 2, BF2 Kamino x 3, BF1 Bespin Platforms; 737 AI life starts, each within 0.5-3 m of a
  baked spawn node) do: pooled by the node count of the post taken, chi2 54.5 against the flat list (8 dof, p 6e-9)
  and 9.8 against an even draw over the posts (p 0.28). The clean case is BF1 Bespin, where one team holds cp9 / cp16
  / cp17 (13 / 8 / 7 nodes) for the whole 475 s and takes 29 / 35 / 45 starts: chi2 21.3 against the flat list, 3.6
  against the even draw (2 dof). Within a post the starts are even over its nodes (16 of 17 posts p > 0.05).
  Tools: `engine/artifacts/lead_tmp/audit-bspn/wf/K4/` (an3.py, out3.txt) and `audit-bspn/tools/ps2_bes1_starts.py`.
- **A mission script can move a post's spawn path [D]:** BF2 Kamino's conquest scripts carry `SetProperty`,
  `spawnpath`, `cp2_spawn` as constants, and the PS2's cp2 bots start at the 12 nodes of `cp2_spawn` (60 of 60
  starts in three captures) while the world data authors `path33` for that post (card SPTH).

## 7. What the walker's death costs [V][D][O]

- **Both:** the post leaves every pick the moment the kill handler runs; members who had drawn it redraw (the keep
  test fails on the dead owner). The team gets event 5 (BF1: its HUD line and voice line; BF2: the voice line only —
  `re-event-messages.md`); what a post with no authored VO then plays is not opened [H].
- **BF2:** tickets and bleed are untouched. `objectiveconquest.luac`'s bleed function (`Start`'s closure 0) and its
  all-posts check (closure 2) walk `self.commandPosts` — the posts the mission registered with `AddCommandPost` — and
  `hot1g_con` registers CP3-CP6 only. Its `OnCommandPostKill` handler does run for the walker's post (it removes a map
  marker by the post's entity name and re-runs the two functions over the registered posts). The class's plain
  `ValueBleed 10` is dead data: the hash 0x4EBF97DA is nowhere in BF2's ELF (no data word, no `lui` of its upper half,
  no immediate of its lower half); RAM shows the class default [0, 10, 10, 10, 10, 0, 10, 10].
- **BF1:** the engine's team update `FUN_002ad830` walks the scheduled posts: one the team owns adds its `ValueBleed`
  (the walker's 10, parsed by `FUN_002a1138`; RAM 10 beside the fixed posts' instance 10) and counts as held, a neutral
  one adds 1. With `hot1i`'s thresholds (sorted ascending at run time, `re-tickets-spawn.md` 2.4) the Empire's 30 at
  the start is rate 0; one AT-AT down → 20 → 0.75 / 4 = 0.1875 tickets/s; both down → 10 → 2.25 / 4 = 0.5625
  tickets/s, while `alive < tickets` [D]. The 20 s "no posts" countdown runs only at value 0, so a live AT-AT holds it
  off. Hoth's second Imperial "post" (list entry 5, far under the map, AllyPath only, ValueBleed 0) adds nothing.
- **Rebuild:** the pad's `SpawnTime` after the death (`re-vehicles.md` section 7); the new walker's constructor and
  init give it a post again.

## 8. A hostile walker's post as an AI target (BF2) [V][D]

`FUN_002e8318(member, exclude, pass)` (B2:273642-273818), the conquest goal's target pick: pass 0 takes every
scheduled post that is not owned by a team friendly to the member's, unless (a) its ban flags match the member's
vehicle class, (b) it is the excluded one, (c) its value is ≤ 0, or (d) — when the member's entity has a unit
controller and the planning graph exists — `FUN_002ebf70(graph, controller, position)` does not return 1
(B2:273773-273809; not opened: what the test is [H]). The position is `FUN_002a6c68`'s: the capture region's when the
post has one, else **the owner's live position** — for a walker, where it stands now. Value = `FUN_002e87b8` =
Value_ATK of the member's faction for a hostile post, the mean of ATK and DEF for a neutral one; a weighted random
pick. Pass 1 (only when pass 0 finds nothing) takes any post. No test of the owner's type. RAM, BF2 Hoth, Alliance
row: CP6 10, CP3 10, each AT-AT 10, the invisible post 0 → the AT-ATs are two of the Rebels' four candidates before
test (d) [D]. For the Empire its own AT-AT is worth Value_DEF 0: never a defend target. What a bot does on arriving at
a post with no capture region is not read [H].

> **Answered 2026-10-02 (card CONT, `re-conquest-goal.md` sections 3-4):** test (d) is planning-graph reachability for
> the unit's type, not a distance; and a bot that reaches a post with no capture region (a walker's) ends its capture
> behaviour 5 s after arriving and draws a new target — it cannot capture it and does not stay.

## 9. Corrections to earlier docs

- `re-cp-capture.md` 1.1: `FUN_00355648` activates a post (fires 0x720dcc), `FUN_00354e10` kills it (fires 0x720db4);
  the doc had them the other way round.
- `re-cp-capture.md` 2.2: the class spawn-point lists are quaternions first, positions second (B2 +0x11c / +0x120, B1
  +0xe4 / +0xe8). 2.1: BF2 flag bit 2 is "selectable on the spawn map" (CommandFlyer's latch), not an alive mirror;
  bit 1 is `HUDIndexDisplay`. Section 9's open names: 0xC069B070 = `HUDIndexDisplay`, 0x66BCDAEA = `HUDIndex`.
- `re-cp-capture.md` 6: the AI's spawn pick is BF1 `FUN_00261e80` (uniform) and BF2 `FUN_002e7240` (AISpawnWeight —
  its open question 3's consumer), not `FUN_002ac980` and a `FUN_002e87b8` score: `FUN_002ac980` and BF2's
  `FUN_0036cca0` are the spawn screen's random pick, `FUN_002e87b8` scores a living bot's target post (section 8).
- `re-walker-ai.md` 13's "6 of the 16 Imperials" and this doc's "8 of 18 AI members had drawn an AT-AT" are the same
  draw seen two ways (16 is the alive cap).

## 10. For the build

The port carries the `SpawnPointCount` / `SpawnPointLocation` entries for command walkers from
`game/src/vehicle_class/walker.rs` into the sim's walker class. `game/src/vehicle_pads.rs` also carries the model
`SPHR` centre when the vehicle manifest supplies it. Existing vehicle manifests already carry the sphere.

For a live command walker, `core/src/sim/spawn/runtime.rs` rotates the authored point and yaw through the current
vehicle frame. After the node draw, it casts from the sphere centre in that same frame toward the node (section 4).
The first terrain or static building ordnance hit supplies the final point, with exact ties going to terrain. When
the segment is clear, the authored endpoint supplies the final point. A missing or invalid sphere retains ordinary
floor placement. The command walker's entry is a spawn post for its team. The BF2 bleed and all-posts rule omits it
on every map (section 7).

The resolved live-walker point bypasses ordinary spawn-floor seating. Fixed-post spawns and walker fallbacks still
use that seating path, including the same-post connected-node correction for ordinary bots on Endor. The opening
wave keeps its pending-pad path. Post identity, node order and the existing RNG draws are preserved.

The spawn map includes eligible live command walkers. The player's pick stays selected while eligible and otherwise
falls back to the nearest eligible post when its location is available (SPSEL). A bot keeps or draws an eligible post
before the node draw (BSPN). A player who has never selected a post draws from the pooled nodes. A dead walker's
points leave the pool immediately and return when its replacement spawns.

The opening wave is placed before the first vehicle-pad tick in `setup_conquest`. It uses the pending command walker
pad's pose and ordinary floor placement. The retained original PS2 timings put the BF2 walker at 0.06 s and its
first units from 0.9 s, with the BF1 first wave at 10 s.

Test vectors: the six points above; RAM pose BF2 Hoth walker (228.79, −6.00, 51.06), right (0.308, 0, 0.952), forward
(−0.952, 0, 0.308) → point 0 at (231.26, −6.00, 58.68), point 5 at (230.14, −6.00, 42.22) (`cp_ram2.py`).

## 11. Checks run, the verifier, what stayed [H]

Hops: the class label table → the commandwalker class ctor and setter → the entity ctor → the post factory, ctor and
attach → the two vtables' hooks → activate / kill → the iterators → CanTeamSpawnAt → the spawn transform and its one
caller → the class parser's two keys → the spawn-map mover → the AI's pick → the team update (BF1) and the conquest
script (BF2) → the conquest goal's target pick. Each step in both games.

- Check 1 (`re_check.py fn`): every cited line, both corpora (the claims list's 59 claims; seven cites sat one or two
  lines outside the named function and are corrected above).
- Check 2 (absences): `ValueBleed` 0x4EBF97DA in BF2 — `re_check.py absent` 0 hits, `elfword.py` 0, and the
  verifier's wider scan (raw bytes at any alignment, any `lui 0x4ebf / 0x4ec0`, any immediate 0x97da) 0; BF1 2 hits.
  `FUN_00356998` / `FUN_0029df70` one caller each (`callers` + `refs.py`: one `jal`, no data word). Writers of
  cp+0xb40: every `sd` at that offset listed (verifier, `immscan.py`), none in command-walker code. Readers of
  cp+0x11c: `FUN_002e7240` only among functions that touch a post (verifier: 127 load sites typed by function-level
  markers, not data flow). `AISpawnWeight` instance bytes on the BF2 disc: 1 (KAS2), with a sanity hit count of 414
  for SpawnPath.
- Check 3 (the twin): every BF2 function above has its BF1 twin opened, with its callers (section 2's table; sections
  3, 4, 5, 6). BF1 has no flag word, no AISpawnWeight, no HUDIndex keys (0 hits each, ELF scan).
- Check 4 (`re_check.py word`): the vtables (0x62be40, 0x61f850, 0x62bb20, 0x62bd38, 0x53e2b0), the format strings
  ("%i", "%f %f %f %f", "%s %i", "%d"), the type names (CommandFlyer / CommandHover / CommandWalker), 100.0 and 4.0
  as `lui` immediates (verifier, disassembly).
- Check 5 (flags): the forced post index (`DAT_005fed1c`, `DAT_00518334`: ELF and RAM −1, written only by the
  bitstream reader and a network respawn), the net-client gates (`DAT_00600548` / `DAT_00600549` / `DAT_00600560`,
  BF1 `DAT_00518be4` / `DAT_00518c08`: all 0 in a conquest battle's RAM; set only through the multiplayer shell),
  `DAT_00600562` (1), cp+0xb40's bits — defaults and writers by the verifier.
- PS2: `cp_ram2.py` / `cp_ram1.py` on four side-select states (both Hoths, both Geonosis maps); `first_seen.py` on
  five live runs; `spawn_share.py`; `members1.py` / `members2.py`; the live session `ps2/spawnmap2b.sh` and
  `ps2/spawn_at_atat.sh` (SW renderer; 7 + 3 states kept).
- Readers: two Opus readers (the spawn screen; the AI's pick), reports `AGENT_A.md` / `AGENT_B.md`; their decisive
  functions were re-read here (both movers, `FUN_002e7240`, `FUN_0036c950`, `FUN_00261e80`, `FUN_002ac640`, the two
  death handlers) and their outcome checked on the PS2 (the live session; the RAM and live shares).
- **Verifier** (one Opus agent, claims and citations only, `verify/VERDICTS.md`): 56 SUPPORTED, 3 PARTLY, 0 NOT
  SUPPORTED of 59. PARTLY, all folded in above: the factory's new index is the old count (section 2); the two entity
  vtables differ at 14 slots, not 8 (section 2); the target pick has a fourth, planning-graph exclusion that reads the
  capture region's position (section 8). Its caveats, also folded in: the six points form a set symmetric in x and z,
  so the live rows prove |x| and the z set about the walker's live pose, not the side, the index or the 5° yaw; one
  hot1L row was counted twice (18 distinct appearances in that run, not 19); the live spawn shows the
  player on the x = −8 line, not which point; `elfword.py` alone cannot prove an absence (16-instruction window).

Stayed [H]: what `FUN_002ebf70` tests and what a bot does on reaching a walker's post (section 8); the voice line a
post without authored VO plays on event 5; the "ready on a dead walker's post" edge (section 3); BF1's target pick for
a walker's post (`FUN_002618b8`, not opened); whether the spawn's authored 5° / 355° yaw is observable (not measured).
