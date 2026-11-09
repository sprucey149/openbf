# Instant Action CTF and Space One-Flag: census and mechanics

Written 2026-11-07 for issue #126. Evidence is the PS2 BF2 executable's decompiled listing
(`SWBFRNT2.ELF-decomp.c`), PS2 BF1 executable (`SWBFRONT.ELF-decomp.c`), PS2 mission bytecode
(`bf2/mission/*.luac` and `bf1/mission/*.luac`), shell options bytecode (`ifs_instant_options.luac`),
and baked level containers (`SPA1.LVL`..`SPA9.LVL`, `INGAME.LVL`).

## 1. Instant Action census: BF1 vs BF2

### 1.1 Battlefront I (2004)
Across all 16 retail maps (`bes1`, `bes2`, `end1`, `geo1`, `hot1`, `kam1`, `kas1`, `kas2`,
`nab1`, `nab2`, `rhn1`, `rhn2`, `tat1`, `tat2`, `yav1`, `yav2`), Instant Action offers
**Conquest only**. No CTF or One-Flag mission scripts exist on the retail PS2 disc;
the `SWBFRONT.ELF` executable does not expose CTF objectives for Instant Action shell play.

### 1.2 Battlefront II (2005)
BF2 Instant Action supports 24 total maps, comprising:
- **Two-Flag CTF (`ObjectiveCTF`)**: 9 maps / 17 era missions.
- **One-Flag CTF (Ground) (`ObjectiveOneFlagCTF`)**: 9 maps / 16 era missions.
- **One-Flag CTF (Space) (`ObjectiveOneFlagCTF`)**: 6 maps / 6 era missions (`spa1`..`spa9`).
- **Conquest**: all 18 ground maps.
- **Space Assault**: all 6 space maps (`spa1`..`spa9`).
- **Hunt**: 6 maps (`end1`, `geo1`, `hot1`, `kas2`, `nab2`, `tat2`).
- **Hero Elimination**: 1 map (`tat2g_eli`).

All 39 Instant Action CTF / One-Flag offerings are retail missions on the disc.

---

## 2. Complete CTF / One-Flag map catalog & sublevels

### 2.1 Two-Flag CTF (Ground: 9 maps, 17 missions)
Each team possesses a team flag placed at their base (`homeRegion`). Teams capture by bringing
the enemy flag into their home capture region (`captureRegion`).

| Map Code | Map Name | Era Offerings | Factions | Map Sublevel |
|---|---|---|---|---|
| `cor1` | Coruscant: Jedi Temple | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `cor1_CTF` |
| `dag1` | Dagobah: Swamp | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `dag1_ctf` |
| `geo1` | Geonosis: Dust Plains | CW only | `rep` vs `cis` | `geo1_ctf` |
| `kas2` | Kashyyyk: Beachhead | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `kas2_tdm` |
| `mus1` | Mustafar: Refinery | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `MUS1_CTF` (CW) / `mus1_ctf` (GCW) |
| `myg1` | Mygeeto: War-Torn City | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `myg1_ctf` |
| `nab2` | Naboo: Theed | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `naboo2_CTF` |
| `pol1` | Polis Massa: Med Facility | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `pol1_ctf` (CW) / `pol1_CTF` (GCW) |
| `tat2` | Tatooine: Mos Eisley | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `tat2_ctf` |

### 2.2 One-Flag CTF (Ground: 9 maps, 16 missions)
A single neutral flag spawns at a central location (`homeRegion`). Either team can pick up the
neutral flag; a team scores by delivering the flag to the **enemy capture region**.

| Map Code | Map Name | Era Offerings | Factions | Map Sublevel |
|---|---|---|---|---|
| `dea1` | Death Star: Interior | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `dea1_CTF-SingleFlag` |
| `end1` | Endor: Bunker | GCW only | `all` vs `imp` | `end1_1flag` |
| `fel1` | Felucia: Marshland | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `fel1_1ctf` |
| `hot1` | Hoth: Echo Base | GCW only | `all` vs `imp` | `hoth_ctf` |
| `kam1` | Kamino: Cloning Facility | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `kamino1_1CTF` (CW) / `KAMINO1_1ctf` (GCW) |
| `tan1` | Tantive IV: Interior | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `tan1_1flag` |
| `tat3` | Tatooine: Jabba's Palace | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `tat3_1flag` |
| `uta1` | Utapau: Sinkhole | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `uta1_1flag` |
| `yav1` | Yavin IV: Temple | CW, GCW | `rep` vs `cis` / `all` vs `imp` | `Yavin1_CTF_CenterFlag` |

### 2.3 One-Flag CTF (Space: 6 maps, 6 missions)
A single neutral space flag (`cmn_flag`, class `com_icon_spaceflag`) is suspended in open space
between the capital ships at `homeRegion`. Pilots in starfighters pick up the flag in space and fly
it into the enemy capital ship hangar (`Team1capture` / `Team2capture`) to score.

| Map Code | Map Name | Era | Attacker / Defender | Map Sublevel | Sky Sublevel |
|---|---|---|---|---|---|
| `spa1` | Space Yavin | GCW | `imp` (Team 1) vs `all` (Team 2) | `spa1_CTF` | `yav` |
| `spa3` | Space Kashyyyk | CW | `rep` (Team 1) vs `cis` (Team 2) | `spa3_ctf` | `kas` |
| `spa6` | Space Mygeeto | CW | `rep` (Team 1) vs `cis` (Team 2) | `spa6_1flag` | `myg` |
| `spa7` | Space Felucia | CW | `rep` (Team 1) vs `cis` (Team 2) | `spa7_1flag` | `fel` |
| `spa8` | Space Hoth | GCW | `imp` (Team 1) vs `all` (Team 2) | `spa8_1flag` | `hot` |
| `spa9` | Space Tatooine | GCW | `imp` (Team 1) vs `all` (Team 2) | `spa9_1flag` | `tat` |

In all six space 1-flag missions:
- Team 1 is Empire (GCW) or Republic (CW); Team 2 is Alliance (GCW) or CIS (CW).
- `hideCPs = true`: command posts are not capturable and display no conquest rings.
- Reinforcements are set to `-1` (`unlimited_reinforcements`).
- Score limit defaults to 5 (`captureLimit = 5`).

### 2.4 Campaign-only missions (excluded from Instant Action)
Campaign missions (`*_c.luac`) are story-scripted scenarios from Rise of the Empire (501st Journal)
and are strictly excluded from Instant Action:
- Ground campaign: `cor1c_c`, `dea1c_c`, `fel1c_c`, `hot1g_c`, `kam1c_c`, `kas2c_c`, `mus1c_c`, `myg1c_c`, `nab2g_c`, `pol1c_c`, `tan1c_c`, `uta1c_c`, `yav1g_c`.
- Space campaign: `spa1g_c`, `spa3c_c`, `spa6c_c`.

---

## 3. ELF & ODF mechanics

### 3.1 Score limits and session defaults
- **Score Limit (`captureLimit`)**: In `SWBFRNT2.ELF` function `FUN_0019e020` (lines 43827-43830),
  the default capture limit read from shell options is verified and clamped: if unset or not matching
  a preset, it defaults to **5 captures**.
- In `ifs_instant_options.luac`, Instant Action CTF options expose `iCTFScore` with preset default 5
  (options range 1..10 or 1..50).
- **Time Limit (`timer`)**: In `FUN_0019e020` line 43831-43839, the default timer setting is 0
  (unlimited / no time limit in standard Instant Action). If enabled, default duration is 600s or 1200s.
- **Winning**: In `objectivectf.luac` (Subproto 3) and `objectiveoneflagctf.luac` (Subproto 4 / 10),
  each capture calls `AddTeamPoints(team, 1)`. When `team_points[team] >= captureLimit`,
  `MissionVictory(team)` is called, terminating the round with victory for that team.

### 3.2 Flag item entity & ODF properties
On ground, flags use `com_item_flag` (or faction variants `com_item_flag_all`, `com_item_flag_imp`, etc.).
In space, the flag entity is `cmn_flag` of class `com_icon_spaceflag` (`BASE flag`).
Decoded ODF properties from retail PS2 `INGAME.LVL` and `SPA1.LVL`..`SPA9.LVL`:

| Property Name | FNV-1a Hash | Decoded Value | Role |
|---|---|---|---|
| `Lighting` | `0x827eb1c9` | `Dynamic` | Dynamic lighting flag |
| `GeometryName` | `0x47c86b4a` | `com_icon_spaceflag` / `com_icon_neutral_flag` | Uncarried model in world |
| `CarriedGeometryName` | `0x05cea8de` | `com_icon_spaceflag_carried` / `com_icon_neutral_flag_carried` | Attached model when carried |
| `ClothODF` | `0xf4d25eda` | `com_icon_neutral_cloth` | Cloth simulation on flag banner |
| `CarriedOffset` | `0x58a48a9e` | `0 1.5 -1.5` (space) / `0.0 1.5 -0.5` (ground) | Attachment offset from carrier root |
| `PickupSound` | `0x759c8f54` | `health_recharge defer` | Audio cue upon pickup |
| `Lifespan` | `0x7c7c544b` | `15.0` | Dropped flag lifespan / return timeout (seconds) |
| `LightOdf` | `0xe369687e` | `flaglight` | Light attachment |
| `MapTexture` | `0x9dc38d80` | `flag_icon` | Radar map icon |
| `MapScale` | `0x0e8c3fe5` | `0.75` | Radar icon scale |

### 3.3 Pickup radius: soldiers on foot vs starfighters
- **Soldiers on foot**: Ground flag pickup is evaluated via character trigger collision with radius
  approximately **2.5 m** (`SWBFRNT2.ELF` `FUN_0035e370` line 343802).
- **Space starfighters**: In space battles, `cmn_flag` has a bounding collision sphere
  `p_-v-collision` with radius **20.0 m** (`SPA1.LVL` prim chunk data `0x0000a040`). This allows
  high-speed starfighters flying at 70–100 m/s to reliably intercept and pick up the flag in open space.

### 3.4 Carrier tracking across flyer mount and dismount
In `SWBFRNT2.ELF` (`FUN_0035d050` line 343062-343064 and `FUN_00269758` line 185500-185545):
- When a unit picks up a flag, the flag stores the carrier unit handle (`flag.carrier = unit`), and the
  carrier soldier stores a pointer to the flag at offset `+0x18c`.
- When the carrier boards a vehicle or flyer (`Unit::mount` set), the flag remains bound to the unit.
  `FUN_00269758` checks whether a vehicle's driver (`+0x328`) or passengers carry a flag (`+0x18c`).
  The carried flag position follows the vehicle's transformed world pose with `CarriedOffset` (`0, 1.5, -1.5`).
- When the carrier exits or lands the vehicle, the flag remains bound to the soldier without dropping.
- Only one flag can be carried by a unit at any time (`FUN_0035d050` line 343043-343048).

### 3.5 Drop on death or crash
- **Soldier death on foot**: When a carrier unit dies, `FUN_0035d2f8` (FlagDrop) is invoked. The flag is
  detached, its position is set to the carrier's final coordinates, and a return countdown timer starts.
- **Flyer destruction / crash in space**: In `SWBFRNT2.ELF` flyer death paths (`FUN_0025f4a0` line 179236
  and `FUN_001f3508` line 104504), when a flyer with a flag carrier is destroyed or crashes into an
  obstacle/ship, the pilot dies, and `FUN_0035d2f8` drops the flag at the flyer's exact 3D coordinates in space.
- **Re-pickup cooldown**: In `FUN_0035e370` line 343802, a dropped flag enforces a **0.5 s delay**
  (`current_time - drop_time > 0.5`) before any unit can re-pickup the flag, preventing instant recapture
  glitches during explosive death.

### 3.6 Return timeout, out-of-bounds, and touch-return rules
- **Dropped lifespan**:
  - In space 1-flag: `com_icon_spaceflag` ODF specifies `Lifespan = 15.0` seconds. If untouched for 15 seconds,
    the flag resets (`FUN_0035d7c8`) to `homeRegion` (`home_1flag`).
  - In ground CTF: default return timeout is 60.0 seconds (`FUN_0035d2f8` line 343281: *(decompiler detail omitted)*,
    IEEE 754 60.0f).
- **Out of bounds / death region**: If a flag falls into a death region (`AddDeathRegion`), below the map
  kill floor, or outside world boundaries (`SetWorldExtents`), `FUN_0035e058` line 343726-343731
  immediately triggers FlagReset (`FUN_0035d7c8`), restoring the flag to `homeRegion`.
- **Touch return vs home reset**:
  - **1-Flag CTF**: The flag is neutral (`team = 0`). Any friendly or enemy unit touching the dropped flag
    picks it up. It returns to home **only** on timer expiry, out-of-bounds drop, or successful capture.
  - **2-Flag CTF**: If Team A's dropped flag is touched by a Team A unit, it immediately **returns to base**
    (Touch Return, `objectivectf.luac` Subproto 4 `OnReturn`). If touched by an enemy Team B unit, Team B
    picks it up (`OnPickup`).

### 3.7 Capture region check & team scoring
- In **One-Flag CTF** (`objectiveoneflagctf.luac` Subprotos 8 & 9):
  - Team 1 (Attacker, e.g. Empire) must transport the flag into Defender's capture zone (`Team2capture`,
    inside Alliance hangar) to score.
  - Team 2 (Defender, e.g. Alliance) must transport the flag into Attacker's capture zone (`Team1capture`,
    inside Imperial hangar) to score.
  - Delivery awards +1 team point (`AddTeamPoints(team, 1.0)`), credits the carrier with capture bonus points
    (`AddFlagCapturePoints`), emits `FlagCapture`, and resets the flag back to neutral home (`home_1flag`).
  - First team to reach `captureLimit` (default 5) wins the round.
