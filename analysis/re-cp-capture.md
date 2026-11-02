# RE: Command post capture / ownership state machine (BF1 + BF2)

Sources: `analysis/SWBFRONT.ELF-decomp.c` (BF1, SLUS-208.98), `analysis/SWBFRNT2.ELF-decomp.c`
(BF2, SLUS-212.40), raw ELFs under `extracted/` (vtable dumps / string reads via a small
PT_LOAD reader written for this task), shipped data parsed directly from the LVLs
(`COMMON.LVL`/`INGAME.LVL` entc chunks, `TAT1.LVL` wrld inst/regn/path chunks, SIDE LVLs),
and the shipped Lua bytecode string tables. Confidence tags: **[V]** = read directly from
decomp / asm / bytes, **[D]** = derived from verified pieces, **[H]** = hypothesis.
Hashes are FNV-1a-32 over `c|0x20` bytes (verified against the binaries: every key below was
matched in a parser `switch`).

> **Corrections (2026-10-02, card CWSP, `analysis/re-command-walker-post.md` section 9; one Opus verifier):**
> (1) §1.1: `FUN_00355648` ACTIVATES a post (schedules it, fires 0x720dcc) and `FUN_00354e10` KILLS it (unschedules,
> event 5, fires 0x720db4) — the table has them the other way round. (2) §2.2: the class spawn-point lists are
> quaternions first, positions second (BF2 +0x11c / +0x120, BF1 +0xe4 / +0xe8). (3) §2.1: BF2 flag bit 2 is
> "selectable on the spawn map" (set and cleared only by CommandFlyer), not an alive mirror; bit 1 is the key
> 0xC069B070 = `HUDIndexDisplay` (the number on the map icon), and 0x66BCDAEA = `HUDIndex`. (4) §6: an AI member's
> spawn post is BF1 `FUN_00261e80` (uniform over the eligible posts) and BF2 `FUN_002e7240` (weighted by
> `AISpawnWeight`, cp+0x11c — open question 3's consumer); `FUN_002ac980` / `FUN_0036cca0` are the spawn screen's
> random pick and `FUN_002e87b8` scores a living bot's TARGET post. A post owned by a command walker (AT-AT, AT-TE)
> is covered there.

---

## 0. Executive summary

| Question | Answer (BF1 = BF2 unless noted) | Conf |
|---|---|---|
| State machine | 2 phases, 2 meters per CP: **neutralize meter** (owned → neutral, key `NeutralizeTime`) and **capture meter** (neutral → owned, key `CaptureTime`). Owner team index lives on the CP *entity* (BF1 `ent+0xd8`, BF2 nibble bits 32..35 of `ent+0x268`); 0 = neutral. | V |
| Per-tick progress | `meter += w(n) * dt`, `w(n) = Σ_{i=0}^{n-1} 0.5^i = 2 − 2^(1−n)` over the n counted units of the charging side (1 unit = 1.0×, 2 = 1.5×, 3 = 1.75×, cap → 2×). Completion when `meter > CaptureTime` (strict) / `meter >= NeutralizeTime`. | V |
| Enemy units on a neutral CP | If two capture-affiliations are present the CP is **contested**: capture meter frozen, dispute sound. If a *different* team is alone inside, it first **drains** the previous team's meter at its own `w(n)`, then the meter flips to it and charges. | V |
| Enemy units on an owned CP | Neutralize meter rises at `w(n_enemies)` — **defenders do not slow it**; contested only changes the VO event. | V |
| Nobody inside | Capture meter (neutral CP) **holds** forever in both games. Neutralize meter: **BF1 resets to 0 instantly** (nobody inside, or only owner inside); **BF2 holds while empty and decays at `w(n_owner)` only while owner units stand inside**. | V |
| Timing keys | `CaptureTime` (0xEB1BCBF8) and `NeutralizeTime` (0xF1A8F31B) on the **class** (entc), `HoloTurnOnTime` (0x630C0E9E). Class defaults 10.0 / 6.0 / 2.0 s. Shipped: `com_bldg_controlzone` 10 / 12 / 2.2; `com_bldg_major_controlzone` 16 / 36 / 2.2 (both games). | V |
| Who counts | Entity alive, has a soldier component, team == displayed-team nibble, its team's *capture affiliation* ≠ 0, and the unit's **soldier class flag `CapturePosts`** (0x325F7C20) ≠ 0. Default 1 for classlabels `soldier` and `walkerdroid` (droideka), 0 for every vehicle classlabel (`droid/walker/flyer/hover/treaded/carrier`) and BF1 `trap`. BF1 retail heroes set `CapturePosts = 0` (cannot capture); BF2 retail heroes inherit `soldier` → they *do* count. BF2 additionally skips AI units of teams masked out by Lua `AICanCaptureCP`. Units are tested as **points** (entity origin). | V |
| Region shapes | Region = 3 basis rows (normalized, row-vector) + centre + SIZE. **box**: `|dot(d,row_i)| < size_i` → SIZE = half-extents. **sphere**: `|d|² < w²` with `w = |SIZE|` (length of the SIZE vector). **cylinder**: axis = row1 (local Y), `|dot(d,row1)| < size_y` (half-height, centred) and `dot(d,row0)² + dot(d,row2)² < r²` with **`r = sqrt(size_x² + size_z²)`**. Strict `<` everywhere. | V |
| Spawn eligibility | Team T may spawn at a CP iff the CP entity is alive, owner ≠ neutral, and (owner == T and (`SpawnPath` set or class `SpawnPointCount` > 0)) or (owner is *friendly* to T and `AllyPath` set). Capture/neutralize progress is **ignored** (you can spawn until the instant the owner flips). BF2 also requires CP flag bit0 (cleared when a destructible CP is killed). | V |
| Spawn node | Uniform random node of the path (LCG `x*0x19660D+0x3C6EF35F`, `((r1>>16)|(r2&0xFFFF0000)) % n`), then ground-snapped (ray 10 m up / 20 m down); fallback: random class `SpawnPoint`, else the CP entity itself. | V |
| Bleed / AI values | `ValueBleed` per faction (BF2 array `cp+0xb8+faction*4`, default 10; BF1 scalar `cp+0x9c`, default 100); `Value_ATK_*` / `Value_DEF_*` per faction feed only the AI planner's CP goal value (BF2 `FUN_002e87b8`: DEF if the CP is friendly, ATK if hostile, average if neutral; BF1 `FUN_002617f8`: team 1 → ATK, teams 2/3 → DEF). | V |
| `AllyCount` | = locals spawn budget contribution: manager sums `AllyCount` of every CP whose owner is friendly to team 3 (locals) → caps locals' alive count. TAT1 uses 65536 (= unlimited). `Radius` > 0 only builds default *sphere* capture/control regions when the named ones are absent (TAT1: 0.0 → no-op). | V |
| major vs normal | Only `NeutralizeTime`/`CaptureTime` (36/16 vs 12/10), label, geometry. Same code path (`BASE=commandpost`). | V |
| Ticket's 10 "uncracked" keys | Transcription error in the ticket: TAT1.LVL contains 0xA340D6B5 etc. (0 hits for 0xA33F3AB5). They are `Value_ATK_{Alliance,CIS,Empire,Republic,Locals}` and `Value_DEF_{…}` (values 10/20/30/50/0/100 per CP). | V |

---

## 1. Function / address tables

### 1.1 BF2 (SWBFRNT2.ELF)

| addr | role | conf |
|---|---|---|
| `FUN_00357440` | **CP tick** `Update(dt, cp)` — unit scan, both meters, events, holo, ambient (1453 lines) | V |
| `FUN_00359b78` | spatial query: entities within the region's bounding radius, filtered by point-in-region (cap 64) | V |
| `FUN_00536410` | **point-in-region** (switch on shape enum at `region+0x54`) | V |
| `FUN_00537a78` | region init from `regn` descriptor: type hash → enum, SIZE scaling/normalisation | V |
| `FUN_005ad450` | region ctor wrapper (maps `sphere`=1 `box`=2 `cylinder`=3 `plane`(0xCCCB0055)=4) | V |
| `FUN_00178xxx` (line 18936 area) | `regn` chunk loader: TYPE/NAME/XFRM/SIZE → descriptor, `w = |SIZE|` | V |
| `FUN_00355950` | CP **instance** property parser (regions, paths, AllyCount, Radius, bans, ValueBleed_*, Value_ATK/DEF_*, VO_*) | V |
| `FUN_0035a3a0` | CP **class** property parser (`CaptureTime`, `NeutralizeTime`, `HoloTurnOnTime`, sounds, `HoloOdf`, spawn points…) | V |
| `FUN_00359df0` | CP class component ctor (defaults §3) | V |
| `FUN_00354290` / `FUN_00354920` | CP instance ctor (size 0xb50) / factory (registers in list `PTR_DAT_005fed20`) | V |
| `FUN_00356880` | **CanTeamSpawnAt(cp, team)** | V |
| `FUN_00356998` | **spawn transform picker** (random path node / class SpawnPoint / entity) | V |
| `FUN_0036c950` / `FUN_0036ca08` / `FUN_0036cca0` | choose spawn CP for a controller (human: keep/nearest; AI → planner `FUN_002e78b0`); random eligible CP | V |
| `FUN_0036c750` | spawn-manager update; sums `AllyCount` into locals budget `mgr+0x58` | V |
| `FUN_00359c08` | on-team-changed (holo image/colour, per-faction capture music) | V |
| `FUN_00355648` / `FUN_00354e10` | destructible-CP **killed / respawned** notifiers (lists 0x720dcc / 0x720db4, global count `DAT_0069a518`) — *not* the capture handler (corrects the anchor label) | D |
| `FUN_003e0218` | HUD/VO event dispatcher `(id 4..7, ctrl, 0,0,0, team, cp)` | V |
| `FUN_003573f8` | `AICanCaptureCP` backend: bit `team` of `cp+0x80` | V |
| `FUN_00356858` / `FUN_00359dc8` / `FUN_00359dd8` | GetBleedValue / GetValueATK / GetValueDEF `(cp, faction)` | V |
| `FUN_002e87b8` | AI goal value of a CP for a team (ATK/DEF/avg) | V |
| `FUN_002f6eb8` / `FUN_002f6ce0` | soldier class component parser / ctor (`CapturePosts` → `+0x222`, default 0; entity-class ctors `FUN_00250488` (soldier) and `FUN_001fc3a8` (walkerdroid) set `class+0x5a2 = 1`) | V |
| Lua @00191bc8 / @00191ad8 / @00191b38 / @00192610 | `GetCommandPostTeam`, `GetCommandPostCaptureRegion`, `GetCommandPostBleedValue`, `AICanCaptureCP` | V |

### 1.2 BF1 (SWBFRONT.ELF)

| addr | role | conf |
|---|---|---|
| `FUN_0029f0d8` | **CP tick** (vtable slot +0x24 of class vtbl 0x53df38) | V |
| `FUN_002a0628` / `FUN_002a06e8` | region query (`FUN_00161c00` radius query, mask 6, cap 64) / point-in-region wrapper | V |
| `FUN_004c2b08` | **point-in-region** (identical structure to BF2) | V |
| `FUN_004c2318` | region ctor: type hash → enum, SIZE scaling, axis-aligned specialisations 5..8 | V |
| `FUN_00177828` | `regn` chunk loader (`w = |SIZE|`) | V |
| `FUN_0029d1d0` | CP instance property parser | V |
| `FUN_002a1138` / `FUN_002a0b28` | CP class parser / class ctor (defaults §3) | V |
| `FUN_0029c148` | CP instance ctor (regions/paths/AllyCount = 0, counters cleared) | V |
| `FUN_0029de80` | **CanTeamSpawnAt(cp, team)** | V |
| `FUN_0029df70` | spawn transform picker (random node + ground snap) | V |
| `FUN_002ac6f0` / `FUN_002ac980` / `FUN_002a20c0` | keep-or-nearest eligible CP / random eligible CP / default CP for team | V |
| `FUN_0029dd68` / `FUN_0029ddf0` | reserve / release a spawn-queue slot on the CP (`cp+0x8c[isAI][ally]`) | V |
| `FUN_002ac378` | manager update: locals budget `mgr+0x84 = Σ AllyCount` | V |
| `FUN_00307940` / `FUN_00307a00` | event enqueue / consumer (HUD text + VO) | V |
| `FUN_0029edd0` | play CP VO entry `(cp, subjectFaction, entry, isPlayer)` | V |
| `FUN_002617f8` / `FUN_002a0b00` / `FUN_002a0b10` | AI CP value (team 1 → ATK, 2/3 → DEF) / getters | V |
| `FUN_0023da58` / `FUN_0023d778` | soldier class component parser / ctor (`CapturePosts` → `+0x228`, default 0; entity ctors `FUN_0020d238` (soldier), `FUN_001d31d8` (walkerdroid) set `class+0x16d8 = 1`, `FUN_00213dd8` (`trap`) sets 0) | V |
| `FUN_002ad490` | team post-init (vtbl +0x10, called from level init `FUN_002abb98`): locals capture affiliation | V |

---

## 2. Struct layouts

### 2.1 CP instance (BF1, ctor `FUN_0029c148`; BF2, ctor `FUN_00354290`, size 0xb50)

| meaning | BF1 | BF2 |
|---|---|---|
| owner entity ptr / id (validated each access) | +0x20 / +0x24 (`ent+0xb4` id) | +0x2c / +0x30 (`ent+0x230` id) |
| owner **team** | `ent+0xd8` (int) | `ent+0x268` bits 32..35 (set via entity vtbl SetTeam: BF1 +0x8c, BF2 +0x98) |
| displayed/original team | `ent+0xdc` | `ent+0x268` bits 36..39 |
| capturing team this tick (0 none, −1 contested) | +0x40 | +0x50 |
| per-team unit counts [8] | +0x44 | +0x54 |
| players-inside masks (counted / not counted) | +0x64 | +0x74 / +0x78 |
| charging team tag of the capture meter | +0x68 | +0x7c |
| **neutralize meter** (s) | +0x84 | +0xa0 |
| **capture meter** (s) | +0x88 | +0xa4 |
| CaptureRegion / ControlRegion / KillRegion ptr | +0x6c / +0x70 / +0x74 | +0x84 / +0x88 / +0x8c |
| SpawnPath / AllyPath / TurretPath | +0x78 / +0x7c / — | +0x90 / +0x94 / +0x98 |
| AllyCount (int, default 0) | +0x80 | +0x9c |
| AI-capture team mask (default all) | — | +0x80 |
| spawn-queue counters [isAI][ally] | +0x8c.. (4 ints, replicated) | +0xa8.. |
| ValueBleed | +0x9c (scalar) | +0xb8 + faction*4 (8 ints) |
| Value_ATK[faction] / Value_DEF[faction] | +0xa0.. / +0xb8.. (6 each; `_Alliance` = index 1) | +0xd8.. / +0xf8.. (8 each) |
| ban flags (SoldierBan=1 HoverBan=2 SmallBan=4 MediumBan=8 HugeBan=0x10 FlyerBan=0x20) | +0xd0 | +0x118 |
| AISpawnWeight (float, default 100) | — | +0x11c |
| VO table `[listener 0..5][subject 0..5][entry 0..4]` (8-byte sound refs) | +0xd4, stride 0xf0 / 0x28 / 8 | +0x120, stride 0x140 / 0x28 / 8 |
| holo object / sounds / class ptr | +0x674 / +0x678,+0x684 / +0x690 | +0xb20 / +0xb24,+0xb30 / +0xb3c |
| flags: bit0 enabled (spawnable), bit1 = class key 0xC069B070, bit2 alive-mirror | — | +0xb40 (ctor `|= 7`) |

### 2.2 CP class component (BF1 class object; BF2 at `entityClass+0x6b0`)

| key | BF1 off (default) | BF2 off (default) |
|---|---|---|
| NeutralizeTime | +0x00 (6.0) | +0x00 (6.0) |
| CaptureTime | +0x04 (10.0) | +0x04 (10.0) |
| HoloImageGeometry `"<faction> <model>"` | +0x08 + f*4 | +0x08 + f*4 |
| HoloOdf (0x6A83ED9C, hologram class name) | +0x20 | +0x28 |
| HoloTurnOnTime | +0x24 (2.0) | +0x2c (2.0) |
| AmbientSound `"<faction> <snd>"` | +0x28 + f*8 | +0x30 + f*8 |
| DischargeSound / ChargeSound / CapturedSound / LostSound / DisputeSound | +0x58 / +0x60 / +0x68 / +0x70 / +0x78 | +0x70 / +0x78 / +0x80 / +0x88 / +0x90 |
| charge-sound ramp factor (0xA33D3401, shipped 0.4) | +0x80 (0.1) | +0x98 (0.1) |
| CaptureMusic / LostMusic `"<faction> <snd>"` | +0x84 / +0xb4 (+f*8) | +0x9c / +0xdc (+f*8) |
| SpawnPoint list (`SpawnPointCount` 0x32579F4D, entries 0xA067D145 `"x y z angleDeg"`, 0xC5190CBA `"hardpoint count"`) | +0xe4 pos / +0xe8 rot / +0xec count | +0x11c / +0x120 / +0x124 |
| ValueBleed(_Neutral, _Alliance, …) | +0xf0 scalar (100) | +0x128 + f*4 (10) |
| Value_ATK_[f] / Value_DEF_[f] | +0xf4.. / +0x10c.. (100) | +0x148.. / +0x168.. (100) |
| bool 0xC069B070 | — | +0x18c bit0 (default 1) |

### 2.3 Region object (BF1 `FUN_004c2318`, 0x70 B; BF2 `FUN_00537a78`, 0x60 B) — identical layout

`+0x00/+0x10/+0x20` rows 0/1/2 = local X/Y/Z axes in world (unit length after init), `+0x30` centre,
`+0x40..0x48` SIZE x/y/z (scaled by the original row lengths), `+0x4c` `w` (= `|SIZE|` from the
loader, or the `Radius` value), `+0x50` name hash, `+0x54` shape enum
(1 sphere, 2 oriented box, 3 oriented cylinder, 4 plane, 5 axis-aligned box, 6/7/8 axis-aligned
cylinder about X/Y/Z). [V]

### 2.4 Team object fields used here

| meaning | BF1 | BF2 |
|---|---|---|
| index | +0x04 | +0x04 |
| faction id (1 all, 2 imp, 3 rep, 4 cis, 5+ locals) | +0x64 | +0x60 |
| relation bytes `[8]` (1 friend, 0 neutral, 0xff enemy; self = 1) | +0x88 | +0x84 |
| **capture affiliation** (team index a unit captures *for*) | +0x90 | +0x8c |

Affiliation: ctor sets it to the team's own index (BF1 `FUN_002acf40` tail, BF2 `FUN_0036d468`
tail). For teams with index > 2, level init (BF1 `FUN_002ad490` via team vtbl +0x10 from
`FUN_002abb98`; BF2 `FUN_0036d9f8` vtbl +0x10 from `FUN_0036d340`) rewrites it: friendly to team 1
only → 1; friendly to team 2 only → 2; friendly to both → 0 (never counts); friendly to neither →
unchanged (own index → hostile locals capture for **themselves**). [V]

---

## 3. ODF keys — hash / offset / default / shipped values

Instance keys (wrld `inst` PROPs, parsers `FUN_0029d1d0` / `FUN_00355950`):

| key | hash | BF1 / BF2 offset | format | TAT1 retail (cp4 / cp2 / cp10) |
|---|---|---|---|---|
| Team | A2FD7D0C | entity SetTeam (BF1 `FUN_00221420` → vtbl+0x8c) | %d | 1 / 2 / 1 |
| CaptureRegion | C6C161F9 | +0x6c / +0x84 | region name → lookup (`FUN_004c4380` / `FUN_00538108`) | CPnCapture |
| ControlRegion | 805C1F8A | +0x70 / +0x88 | region name | CPnControl |
| KillRegion | 76ED2F81 | +0x74 / +0x8c | region name | — |
| SpawnPath | DDE0B14F | +0x78 / +0x90 | path name (hash lookup `FUN_00472290`/`FUN_0055d008`; "" → null) | CPnSpawnPath |
| AllyPath | DAD215B0 | +0x7c / +0x94 | path name | — |
| TurretPath | 73466220 | — / +0x98 | path name | — |
| AllyCount | 7136FD3E | +0x80 / +0x9c | %d (default 0) | 65536 |
| Radius | 0DBA4CB3 | builds sphere regions if > 0 | %f | 0.0 |
| ValueBleed | 4EBF97DA | +0x9c / — (BF2 uses `ValueBleed_<faction>` FDE7C9D2/5A7EB7DB/DC392F3F/2450A89C/D97FC017, `_Neutral` EB13C1FE) | %d | 10 |
| Value_ATK_Alliance/Empire/Republic/CIS/Locals | A340D6B5 / 4DB44670 / 9E78B410 / 00E73245 / 573C98A4 | +0xa4,+0xa8,+0xac,+0xb0,+0xb4 / +0xdc..+0xec | %d (default 100) | 30,30,30,30,10 / 20,20,20,20,10 / 10×5 |
| Value_DEF_Alliance/Empire/Republic/CIS/Locals | 44A2C4A6 / A8679457 / 8F5540CB / 5BF85AE8 / E689EF93 | +0xbc..+0xcc / +0xfc..+0x10c | %d (default 100) | 50,50,50,50,0 / 20,20,20,20,100 / 10×5 |
| SoldierBan/HoverBan/SmallBan/MediumBan/HugeBan/FlyerBan | 3994A056/58FC17EE/6D1CA06D/C8F3C76B/B08090E7/1CD984F6 | +0xd0 bits / +0x118 bits | %d>0 | — |
| AISpawnWeight | E07282C2 | — / +0x11c | %f ≥ 0 | — |
| Label | F69717FD | +0x1c / +0x24 | localisation key | CPn |
| VO_<All\|Imp\|Rep\|CIS>_<All\|Imp\|Rep\|CIS><Capture\|Lost\|?\|Saved\|Info> | e.g. 13AD1EA0 VO_All_AllCapture, 9E71F320 VO_All_AllLost, FD4B3AA1 VO_All_AllSaved, BA31CB0A VO_All_AllInfo (entry-2 names uncracked: 22BD3453 All/All, E018B5A4 All/Imp, 87834218 Imp/All) | VO table entries 0..4 | sound names | set for Capture/Lost only |
| 0x66BCDAEA (int → +0x1c BF2), 0xC069B070 (bool), 0x346F3B69 ("description", ignored) | | | | not in retail data |

Class keys (entc, parsers `FUN_002a1138` / `FUN_0035a3a0`) — shipped values are identical in
BF1 `COMMON/ENG/COMMON.LVL` and BF2 `INGAME.LVL`:

| class | NeutralizeTime | CaptureTime | HoloTurnOnTime | ramp 0xA33D3401 | other |
|---|---|---|---|---|---|
| com_bldg_controlzone | 12.0 | 10.0 | 2.2 | 0.4 | HoloOdf com_holo_controlzone, Charge/Captured/Discharge/Lost/DisputeSound `com_blg_commandpost_*`, AmbientSound per faction, MapTexture hud_flag_icon |
| com_bldg_major_controlzone | 36.0 | 16.0 | 2.2 | 0.4 | same; geometry `_large` (BF1) |
| com_bldg_untakeable_controlzone / _invisible_controlzone | (class default 6.0) | (10.0) | 2.2 | (0.1) | no sounds/holo |
| class ctor defaults | 6.0 | 10.0 | 2.0 | 0.1 | ValueBleed 100 (BF1) / 10 (BF2), Value_ATK/DEF 100 |

Soldier-class key: `CapturePosts` 325F7C20 → BF1 soldier component +0x228 (int), BF2 +0x222 (byte).
Component ctor default 0; entity-class ctors set it to 1 for `soldier` (BF1 `FUN_0020d238`, BF2
`FUN_00250488`) and `walkerdroid` (BF1 `FUN_001d31d8`, BF2 `FUN_001fc3a8`); BF1 `trap` sets 0.
Retail: BF1 `all_inf_lukeskywalker(snow)`, `imp_inf_darthvader`, `cis_inf_countdooku`,
`rep_inf_macewindu` = 0; BF2 only `gam_inf_gamorreanguard` = 1 (redundant). [V]

---

## 4. Capture algorithm (both games; BF2 differences marked)

Per CP, once per frame, `Tick(dt)` (BF1 `FUN_0029f0d8`, BF2 `FUN_00357440`):

*(decompiler excerpt omitted from the public copy)*

Notes [V]: the capture meter of a neutral CP never decays; it keeps its charging-team tag, so a
returning team continues where it left off and a different team must drain it first. On an owned
CP the neutralize meter is not reset by a capture: BF1 relies on the `neutral == N` test to zero
the leftover silently on the next frame with the capturers still inside (or the "nobody inside"
reset); **BF2 has no such reset — after a capture the leftover meter (= N) decays at the new
owner's unit rate and holds while the CP is empty, so an early counter-attack neutralises in
less than N seconds** (see vector 9; needs PCSX2 confirmation of the gameplay effect, code is
unambiguous). BF2 early-outs after the unit scan when `FUN_003e4a30() != 0` [H: round not running].

---

## 5. Region containment (Q3) — the math the port must implement

Loading (`regn` chunk → descriptor, BF1 `FUN_00177828`, BF2 loader at decomp line ~18936):
`XFRM` rows 0..2 become basis rows (w = 0), the translation becomes the centre row (w = 1),
`SIZE` = 3 floats, and **`w = sqrt(sx²+sy²+sz²)`** is stored as the 4th size component. TYPE is
hashed (`sphere` AFD98518, `box` 70C67E32, `cylinder` AD1EBFEF, `plane` CCCB0055).

Init (BF1 `FUN_004c2318` / BF2 `FUN_00537a78`), `len_i = |row_i|` before normalisation:

* sphere → enum 1, nothing else (radius = `w`).
* box → `size_i *= len_i`; if every row has exactly one component with |c| > 0.02 the region
  becomes an axis-aligned box (enum 5, sizes permuted onto world axes), else rows are normalised
  → enum 2.
* cylinder → `size_x *= len_0; size_y *= len_1; size_z *= len_2; r = sqrt(size_x² + size_z²);
  size_x = size_z = r`. If row1 (local Y) has a single non-zero component the region becomes an
  axis-aligned cylinder about that world axis (enum 6/7/8: X/Y/Z), else normalised → enum 3.

Point test `d = p − centre` (BF1 `FUN_004c2b08` / BF2 `FUN_00536410`), strict inequalities:

| enum | inside iff |
|---|---|
| 1 sphere | `d·d < w²` |
| 2 box | `abs(d·row0) < size_x && abs(d·row1) < size_y && abs(d·row2) < size_z` (half-extents) |
| 3 cylinder | `abs(d·row1) < size_y` (half-height, centred) `&& (d·row0)² + (d·row2)² < size_x²` |
| 4 plane | `p·row2 < w` |
| 5 AA box | `abs(d.x) < size_x && abs(d.y) < size_y && abs(d.z) < size_z` |
| 6/7/8 AA cyl | `abs(d.axis) < size_y && (other two)² sum < size_x²` |

`Radius` key (both instance parsers): if > 0 and CaptureRegion/ControlRegion is still null, a
sphere region is created from the CP entity's matrix with `w = Radius` (SIZE xyz left 0). [V]

TAT1 worked values (region centre = XFRM translation): `cp2capture` sphere SIZE (0.1, 8.14867,
0.1) → **r = 8.1499**; `cp10capture` cylinder SIZE (4.287898, 10.0, 20.576218), rows
(−1,0,0),(0,1,0),(0,0,−1) → AA cylinder about Y, **r = 21.018, half-height 10.0**;
`cp7capture` cylinder (17.42389, 12.373662, 12.955763), tilted rows → oriented cylinder,
r = 21.713, half-height 12.374; `cp4capture` box (7.314838, 2.781044, 4.102591), slightly
rotated rows (|row0.y| = 0.02004 > 0.02 defeats the AA test) → oriented box with those
half-extents. Control regions (boxes ~35×10×35) are not used by the capture tick (BF1 AI code
`FUN_002afa90` tests unit presence in `ControlRegion`; BF2 Lua sees only the capture region).

---

## 6. Ownership → spawning (Q4)

Eligibility `CanTeamSpawnAt(cp, T)` (BF1 `FUN_0029de80`, BF2 `FUN_00356880`) [V]:
```
if cp has no owner entity: false            [BF2: also require cp.flags bit0 (enabled)]
if owner.team == T:  return cp.SpawnPath != null or class.SpawnPointCount > 0
if teams[owner.team].relation[T] > 0: return cp.AllyPath != null      # friend
return false                                # neutral team 0 has no friends → neutral CP unusable
```
Callers additionally require the CP entity to be alive. Neither meter is consulted.

CP choice: humans keep their selected CP if still eligible, else the nearest eligible one to it
(BF1 `FUN_002ac6f0`, BF2 `FUN_0036ca08`); BF1 falls back to `FUN_002a20c0` (eligible CP chosen by
distance relative to hostile-owned CPs [H exact metric]) and AI code uses a uniformly random
eligible CP (`FUN_002ac980`); BF2 AI asks the planner (`FUN_002e78b0`), whose CP goal value is
`FUN_002e87b8` (Value_DEF if the CP is friendly to the team, Value_ATK if hostile, average if
neutral, indexed by the team's faction id). BF1 planner value `FUN_002617f8`: team 1 → ATK, team
2/3 → DEF. Values are AI weights only — they never touch capture. [V]

Spawn transform (BF1 `FUN_0029df70(cp, allyPath?)`, BF2 `FUN_00356998`) [V]:
1. path = AllyPath if the spawner's team ≠ owner team else SpawnPath; if it has nodes:
   `idx = rnd % nodeCount` (LCG above), take node position + rotation; ray-cast 10 m up then
   20 m down (BF1 `FUN_00161d68` mask 0x48, BF2 `FUN_00161c60` mask 0x90) and drop the point onto
   the ground if a hit is within 20 m.
2. else if class `SpawnPointCount` > 0 and not an ally: random class SpawnPoint transformed by
   the CP matrix.
3. else: the CP entity's own position.
Orientation is rebuilt from the node's forward and the world up.

`path` chunk (TAT1.LVL) [V]: `NAME` (string), `INFO` (u32 nodeCount, u16 3), `PROP` (NUL-separated
property-name strings), `PNTS` = nodeCount × 28 B = `f32 x,y,z, f32 qx,qy,qz,qw`. BF2's loader
(`FUN_0055d060`) keys children by hashed names `position` (934F4E0A, z negated on load),
`rotation` (21AC415F), `properties` (11DE6CDC). `CP3SpawnPath` has 13 nodes.

BF1 also keeps a per-CP spawn-queue counter `cp+0x8c[isAI][sameTeam]` (reserve `FUN_0029dd68`,
release `FUN_0029ddf0`, replicated) — bookkeeping only. AllyCount → locals budget: BF1
`mgr+0x84 = Σ cp.AllyCount` over CPs whose owner's relation byte to team 3 is > 0 (clamped by
`mgr+0x50`); BF2 `mgr+0x58` likewise (`FUN_0036c750`). [V]

---

## 7. Events / VO / HUD (Q5)

> **Correction (2026-09-30, card EVMSG, `analysis/re-event-messages.md` §1):** the HUD lines `game.infomsg.captured` /
> `game.infomsg.lost` are **BF1 only**. BF2's consumer `FUN_003e0320` handles events 4–7 with the post's voice line
> (`FUN_003570b8`) and no text; the two Locl hashes are used nowhere in BF2's ELF. Read "HUD game.infomsg.*" below and
> in §4's pseudo-code as BF1's.

> **Correction (2026-10-03, card AMBM, `analysis/re-ambience-music.md` 4):** the post's looping "ambient" is the
> class key `AmbientSound` (`<faction> <sound>`, the OWNER's faction; kept by a neutralised post), not the `VO_*Info`
> entry 4, which is the announcer's line. The ramp key 0xA33D3401 is `SoundPitchDev` and it sets the loop's PITCH:
> charge `P x capture / T + 1 - P`, discharge `P x (1 - neutral / N) + 1 - P`. `DisputeSound` plays on a NEUTRAL post
> with two sides inside only; an owned post under attack plays `DischargeSound`, contested or not.

Event ids (BF1 `FUN_00307940`, BF2 `FUN_003e0218`; args `(id, creditPlayer, 0,0,0, team, cp)`):
4 captured → HUD `game.infomsg.captured` + VO entry 0 (`VO_<L>_<S>Capture`); 5 neutralised →
`game.infomsg.lost` + entry 1 (`…Lost`); 6 (first frame of a contested neutralisation) → entry 2
(name uncracked; unused by retail data); 7 saved/recovered → entry 3 (`…Saved`); entry 4
(`…Info`) is the looping ambient. Listener (L) = local player's team faction, subject (S) =
event team's faction (BF1 `FUN_0029edd0`). The credited player is the highest player bit inside.
BF2 additionally fires listener lists for Lua/HUD: 0x720d6c capture (re)started, 0x720d84
dispute, 0x720d9c capture finished (`OnFinishCapture`), 0x720d24 neutralise started, 0x720d3c
neutralise recovering, 0x720d54 neutralise finished (`OnFinishNeutralize`), 0x720dcc / 0x720db4
CP killed / respawned (destructible CPs; `OnCommandPostKill/Respawn`). [V ids/lists, D names]
Sounds: ChargeSound loops while capturing (and on BF2 recovery), DischargeSound while being
neutralised, DisputeSound while contested; the loop's parameter ramps between `1−ramp` and 1 with
the meter (ramp = class key 0xA33D3401 = 0.4). Holo: `FUN_00359c08`/holo calls swap image +
colour on team change; `HoloTurnOnTime` = seconds before completion during which the old holo is
switched off. [V/D]

---

## 8. Reference vectors (retail `com_bldg_controlzone`: T = 10, N = 12; dt-independent)

1. Neutral CP, 1 unit of A: captured 10.0 s after entry (`w=1`). 3 units: 5.714 s. 4: 5.333 s.
2. Owned by A, 1 unit of B: neutral after 12.0 s; then B's capture meter starts at 0 →
   owned by B at 22.0 s total. 4 B units: 6.4 s + 5.333 s = 11.733 s.
3. Owned by A, 1 B + 2 A inside: neutral after 12.0 s (defenders irrelevant); event 6 fires on
   the first frame; event 5 at 12 s.
4. Neutral CP: A charges to 6.0 s and leaves; 1 unit of B enters: meter drains 6 s, flips to B,
   charges 10 s → B owns it 16.0 s after entering.
5. Neutral CP: A charges to 6.0 s, everyone leaves for any duration, A returns with 2 units:
   captured after (10 − 6)/1.5 = 2.667 s (no decay while empty).
6. Neutral CP: 1 A + 1 B inside → contested, meter frozen indefinitely; B leaves → A's meter
   resumes.
7. Owned by A, B neutralises to 6.0 s then leaves: **BF1** meter → 0 on the next tick;
   **BF2** meter holds 6.0 while empty; with 1 A unit inside it decays to 0 in 6.0 s (event 7).
8. Major CP (T = 16, N = 36), 3 attackers: neutralise 20.571 s, capture 9.143 s.
9. BF2-only leftover: B captures at t0 with 1 unit and stays; A returns at t0+2 s with 1 unit
   (contested): leftover meter = 12 − 2 = 10 → A neutralises at t0+12 s instead of t0+14 s
   (BF1: full 12 s from A's entry, i.e. t0+14 s).
10. Point tests: `cp10capture` (centre c, AA-Y cylinder r = 21.018, h = 10): c+(15,0,14) inside
    (421 < 441.77); c+(15,0,15) outside (450); c+(0,10,0) outside (10 is not < 10).
    `cp2capture`: c+(8.1,0,0) inside, c+(8.2,0,0) outside. `cp4capture`: c + 7.0·row0 inside,
    c + 7.4·row0 outside; c + 2.8·row1 outside (2.781 half-extent).
11. Eligibility: TAT1 cp3 (Team 3 = locals, no AllyPath): only team 3 (and no one else) may
    spawn there; a neutral CP is never spawnable; cp10 owned by team 1 with locals friendly to
    team 1 and no AllyPath → locals cannot spawn there.
12. Locals with `AllyCount` 65536 on every owned CP → locals budget = ∞ (clamped by the
    manager); BF1 hero (`CapturePosts 0`) standing alone on a neutral CP → no progress.

---

## 9. Open questions

1. BF2 post-capture leftover neutralize meter (vector 9) — CLOSED 2026-10-13 (card CPBAR,
   `analysis/re-capture-display.md` 6, live rows on BF2 Mygeeto): after a capture the meter reads 11.87 of 12 and
   falls at 1.0 a second while the new owner stands inside, holds while the post is empty, and with an enemy inside
   rises again from where it stands (9.03 -> 12.0: the post was neutral 3.0 s later). BF1 reads 0.0 one update after
   the capture. The same rows show both games' rules for an EMPTY owned post (BF1 16.2 -> 0.0 in one update, BF2
   holds 5.43 for 5 s) and that defenders do not slow the neutralize meter (BF2: 1.0 a second with both inside).
2. (VO entry 2's name is CLOSED, 2026-09-30, `analysis/re-announcer.md` §2.1: `VO_<L>_<S>InDispute`, e.g.
   22BD3453 = `VO_All_AllInDispute`; no retail post sets it.) Still open: the names of the instance keys
   0x66BCDAEA (int, BF2 `+0x1c`) and 0xC069B070 (bool, default 1); class keys 0xA33D3401 (ramp),
   0x4BC9B119 (5.0 in BF2 entc, parsed by the generic entity parser `FUN_0026a340`); 0x73425BAD
   and 0xD89B1A5C ("BODY") have no parser in either PS2 ELF (dead data).
3. `KillRegion` (+0x74/+0x8c) and `TurretPath` consumers; BF2 `AISpawnWeight` consumer.
4. Meaning of the "displayed team" nibble/`ent+0xdc` (identical to team for all normal units);
   the BF1 `+0xd8/+0xdc` equality test may exclude units mid team-switch.
5. BF2 `FUN_003e4a30()` early-out and `DAT_005fcbdc` (MP join grace) values.
6. BF1 `FUN_002a20c0` default-CP metric (distance to hostile CPs).

---

## 10. Port guidance — Rust `core::cp`

* Data: per CP `{owner: TeamIdx(0=neutral), capture: f32, capture_team: TeamIdx, neutral: f32,
  capture_region, control_region, spawn_path, ally_path, ally_count, value_bleed[8],
  value_atk[8], value_def[8], ai_mask: u8}`; per class `{capture_time (10), neutralize_time (6),
  holo_turn_on (2)}`; per soldier class `capture_posts: bool` (true for `soldier`/`walkerdroid`
  classlabels, false for vehicles; honour the ODF override — BF1 heroes are 0).
* Tick exactly as §4 with BF2 semantics as the openbf baseline (BF2 recovery decay + hold);
  expose a `bf1_instant_reset` flag if BF1 fidelity is wanted. Decide explicitly whether to keep
  BF2's leftover-meter quirk (§4 note) — recommend zeroing `neutral` on capture and documenting
  the divergence, since the quirk looks unintentional (BF1 had the `== N` guard).
* Counting: point-in-region on the unit origin; only alive soldiers with `capture_posts`;
  vehicles never count (their soldier component default is 0); occupants inside vehicles are not
  in the scan (BF1 measured, card WCREW 2026-10-16: in 257 BF1 save states, 73 CP rows - instance word 0x53df50 at
  +0x14, `FUN_0029c148` - have an alive seated soldier, character +0xb8 != 0, of a capturing team inside the
  CaptureRegion +0x6c; in 73 of 73 the counts at +0x44 equal the soldiers on foot only, with team 3 folded into
  its allied team; the alive bit ent+0xac bit 3 is set on every seated soldier, so the exclusion is in the region
  query `FUN_00161c00` (mask 6), not the scan's tests [D]; BF2 not sampled [H]. Tool and rows:
  `engine/artifacts/lead_tmp/wcrew_cpcount.py`, `wcrew_cpcount.txt`); affiliation via the team table (locals → their single friendly main team; friendly
  to both → never; hostile to both → themselves).
* Regions: implement the three shapes with the SIZE semantics of §5 (sphere `w=|SIZE|`,
  cylinder `r=√(sx²+sz²)`, box half-extents, all strict `<`), normalising rows and scaling sizes
  by the row lengths at load; keep the axis-aligned fast paths optional (results identical).
* Spawning: eligibility of §6; random node via the shared LCG (`SimRng` must reproduce
  `x*0x19660D+0x3C6EF35F` and the `(hi16|hi16')%n` draw) then ground snap; AllyPath for allies.
* Events: emit captured / neutralised / contested-start / saved with (team, cp, credited
  player); map to HUD strings `game.infomsg.captured/lost` and the VO table
  `[listener faction][subject faction][entry]` when sound lands.
