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

## 4. Issue 144: shared-texture material failure (2026-11-10)

Source baseline `2b9cec49d634e228940f43c7e89427ee64c63866`. This is a partial presentation correction, not lifecycle acceptance. The saved black box identifies `bf2_spa6`, `1flag`, seed 292 and clean build `2b9cec49d634`; it does not record the reported drop failure. The four supplied private images and the snapshot remain outside Git.

The spa6 mode layer places `cmn_flag` of class `com_icon_spaceflag` at the home location. Its `com_icon_spaceflag` model contains a `com_sfx_laser_yellow` segment (MTRL `0x02010054`) and a separate opaque segment using `spa1_bldg_stardestroyer_hanger_01`. The two capture marker placements use `com_prop_ctfdrop_space`, whose `spacedust1` segment has MTRL `0x01010044` and scrolling UVs. These texture sheets are in BF2 COMMON, not the map's texture table. Before this change both effect segments were baked OPAQUE because absent map-local statistics short-circuited the BF2 material rule. The BF2 executable's `FUN_0058ed70` at current `analysis/SWBFRNT2.ELF-decomp.c` lines 711150 and 711192–711213 selects translucent drawing from the segment flag and sets blend/depth state; this decision does not depend on a texture histogram. The common-texture case now takes the existing BF2 rule when a texture is named. Untyped or unnamed segments retain the prior fallback. The port still does not establish the original additive path for these materials.

The full PS2 rebake yielded BLEND for the yellow and `spacedust1` segments on all six maps (`spa1`, `spa3`, `spa6`, `spa7`, `spa8`, `spa9`), with the package segment OPAQUE. Matched port-only static captures from the old and staged packed binaries, using the same spa6 one-flag layer and camera, show the previously solid yellow facets becoming glows around a visible package, and the solid delivery sphere becoming see-through. These are controlled viewer captures, not matched native lifecycle captures. The delivery marker remains gray in that capture; its native color, additive behavior, visibility and three arrows remain unverified or unimplemented.

The static world path still instances the home flag independently (`game/src/viewer/scene/world.rs:376`); the live flag is not yet drawn at its carried or dropped position. The player's goal and the pickup share a flag hint (`game/src/main/battle.rs:690–719`). The flyer carrier offset currently adds in world coordinates (`core/src/sim/ctf.rs:434`), so attachment rotation is not established. The report does not prove drop rendering and these findings do not establish original trigger versus solid collision. No collision or capture-region rules were changed.

Native setup: emucap managed generation `launch-01m72spkeh8ez0wbnba5rvz053`, PS2 disc SHA-1 `b385edc89d053ca8238b47c6549e78f45eb88ca4`, Tracking run `01M72SQVM4QA8MWKXE4BRZR93V`. State `.10` displayed Assault objectives after 180 frames; `.09` displayed a flag indicator and zero team scores and EE contained `spa1` and the carried flag model name. These observations do not by themselves identify every mission condition. Both loads and the digital cross/triangle attempts are recorded as controlled interventions. The adapter advertised EE and digital buttons but no axes; these attempts did not reach the package, and no lifecycle trace was obtained. Captures are retained under `engine/artifacts/lead_tmp/issue-144/`; the exact managed generation was stopped.

Checks: `cargo test -p bake --lib` passed 365, ignored 12; `cargo clippy --workspace -- -D warnings` and `cargo clippy -p bake --all-targets -- -D warnings` passed. `cargo clippy --workspace --all-targets -- -D warnings` failed at `game/tests/golden_hud.rs:19` because its `HudModel` initializer lacks `ctf`; both the initializer and field are present in that state at baseline `2b9cec49`. The serial headless `scripts/gpu_tests.sh --render` passed 82 tests. `cargo test -p game --test space_ctf_scenarios` passed 7; `--test space_playtest_fixes` passed 9; `OPENBF_REQUIRE_SPACE_ASSETS=1 cargo test -p game --test space_secondary_weapons` passed 1 matrix test. The matrix is not a guided-rocket camera/control acceptance. The full `scripts/pack.py --out dist/openbf-issue-144.exe` rebaked and packed 28,708 files; staged embedded spa6 and COMMON assets loaded in a headless capture before replacing `dist/openbf.exe`. Alignment acceptance was not run: the gate requires independent reference and source bakes, and these scoped checks do not establish cross-source alignment.

Still required: native home, both carried sides, death/drop, re-pickup, timeout and capture sequences; live package drawing and rotated attachment; delivery symbol and on/off-screen/radar semantics; native effect color, blend/depth and collision/trigger trace; both faction slots on all six missions; matched native/port captures and corresponding regressions. The issue remains open.

`cargo fmt --all -- --check` passed. The size report's initial Windows invocation could not find a working `python3`; running its Python body with the installed Python then reported all 819 paths missing because Windows separators do not match its slash-based map entries. This is not a passing map gate. The touched file is 1,472 lines and its map row was updated.

## 5. Native lifecycle and presentation follow-up (2026-11-10)

Investigated on source revision `9be51bf1` and its working changes. The managed PS2 generation `launch-01m735s8sknwfqkxpzm1t3f3v6` used disc SHA-1 `b385edc89d053ca8238b47c6549e78f45eb88ca4`; Tracking run `01M72VMAKT08BE6TKVECD35ZS1` records the interventions. State `.09` was identified as `spa8_1flag` from its live Lua mission string; `.10` is Assault. The adapter exposed digital controls, not analog axes. To reach otherwise inaccessible states, the player was repositioned by controlled EE writes at a frozen boundary. After native pickup the native member and flag carrier pointers agreed. A controlled downward-velocity write caused death and native drop; re-spawning and repositioning reached a native re-pickup. In a separate drop, the native timer returned the object to its home position. For capture, the native imperial region was moved to the carrier, and the team score increased and the flag reset. This was a relocated trigger experiment, not an ordinary flight or a matched capture sequence. Screenshots and saves are retained only under `engine/artifacts/lead_tmp/issue-144/`.

Native marker draw entry `FUN_001c5610` (current decompilation lines 71374 onward) passes team tint into the model rendering path `FUN_0057cf60` (700363 onward). At its actual call boundary `0x1c5720`, the controlled friendly marker supplied bytes (1,86,213,255) and the other marker supplied (200,100,100,255). `com_prop_ctfdrop_space` uses the authored `spacedust1` material with 0x04 and 0x40 flags; `FUN_0058ed70` lines 711150 and 711192–711213 establish its translucent, no-depth-write material path. Its scroll comes from its baked material. The PS2 GS color-byte unity is 128. Screenshots show a patterned, non-opaque sphere, blue or red relative to the player, and three yellow arrows. The new world additive path is still a linear-target approximation of GS display-space compositing; the retained comparison does not establish pixel parity or exact effect animation.

`FUN_003a7630` at current lines 389554–389565 and 389648–389653 updates the pulsing map marker from its base scale and fractional game time. The inspected flag map marker had base 3, minimum 0.9, and yellow bytes (223,223,32). `FUN_003abc60` at lines 392668–392710 supplies the on-screen hint pulse; the off-screen marker is not scaled the same way. The original one-flag script selects `hud_target_flag_onscreen` for pickup, `hud_objective_icon_circle` for the delivery map marker, and the objective style for delivery; the latter supplies the three arrows. The native script names faction-relative `1Flag_take`, `1Flag_drop` and `Flag_score` sound events, with a three-second voice gate. The matching names exist in the local space sound bank. Native playback and the port output have not yet been acoustically compared.

The home model's independently submitted map instance is now hidden when CTF state exists; the live flag supplies either authored loose or carried model at the simulation position, with the offset rotated by flyer or soldier orientation. The native model's light/effect attachment and native carried orientation throughout mounting and dismounting are not established by this experiment. The marker's GS color, translucent/additive material and vertex alpha are retained; the port's capture-region mapping is unchanged. These facts do not establish native solid-collision behavior, and no new collision body was introduced.

Controlled native pickup, death/drop, re-pickup, timeout and region-relocated score were observed for one team on spa8. The opposing team's carried sequence, natural flight/capture, matched-depth lifecycle captures, precise material compositing and timing, indicator behavior throughout the lifecycle, voice playback, and trigger versus solid-collision behavior remain acceptance work. The six-map asset and simulation checks are not native visual acceptance.

The one-flag script's marker callback is team-scoped: when a carrier exists it removes the pickup marker for both teams, enables the carrier team's delivery marker, and removes the other team's delivery marker. The voice callback compares elapsed mission time strictly greater than three seconds before broadcasting; its startup value makes the first call eligible. The local queue now follows those rules for one-flag. A sound-bank regression resolves and decodes all 12 Imperial/Alliance pickup, drop and score combinations in spa8; it does not constitute an acoustic comparison to PCSX2.

Checks on the follow-up: `cargo test -p core` passed 2,071 unit tests and 15 integration tests (2 ignored); `cargo test -p game --test space_ctf_scenarios` passed 7; `cargo test -p game --test space_playtest_fixes` passed 9; `OPENBF_REQUIRE_SPACE_ASSETS=1 cargo test -p game --test space_secondary_weapons` passed the 12-map/mode, 86-mount, 16-pilot-secondary matrix. It does not establish guided-rocket camera/control parity. Focused game library tests passed for pose, one-flag marker team routing, voice gating and sound-bank decoding. `scripts/gpu_tests.sh --render` passed 83 serial headless tests; `scripts/gpu_tests.sh --game space_ctf_visuals` passed the six-map live-model and two-team marker-color check, including visible-pixel comparisons for spa8. `cargo clippy --workspace -- -D warnings`, `cargo clippy -p game --tests -- -D warnings`, and strict all-target Clippy for bake, render and ui passed. Previously broken HUD fixtures were initialized with the existing non-CTF default. `cargo fmt --all -- --check` passed.

`py -3.11 scripts/gates.py --out engine/artifacts/lead_tmp/issue-144/alignment --groups alignment` could not start because `engine/artifacts/loop/gates.toml` was absent; independent reference and source bakes were also not supplied. This is not alignment acceptance. The PS2 map group was rebaked after the vertex-alpha change. `cargo build --release -p game` and `game.exe --write-pack` packed 28,708 current files to `dist/openbf-issue-144.exe`; an embedded spa8 one-flag replay rendered from `dist` before copying to `dist/openbf.exe`. Both executable SHA-256 values were `44ce5278bf8ca5ef9c9a420283e669c1b1f2da557e59fdbcfce4ec4164978128`. The playtest/profile directory was preserved.
