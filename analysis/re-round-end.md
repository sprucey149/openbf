# RE: how a conquest round ends, BF1 and BF2 (PS2)

Tags: [V] read in the decomp / ELF words / PS2 bytecode / baked data (function + line given); [D] deduced from
verified pieces; [H] hypothesis. B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`
(line numbers). Lua: BF2 PS2 bytecode disassembled with `engine/artifacts/lead_tmp/re/timelimit/lua50dis.py`
(existing dumps `dis_objectiveconquest.txt`, `dis_objective.txt` in that folder); BF1 PS2 bytecode read with
`engine/artifacts/lead_tmp/audit-annc/lua40.py`. The mod-tools Lua source under `community_mods/` was a reading aid
only; where the PS2 bytecode differs from it, that is said. Locl texts were looked up by the `locl_key` hash in
`engine/assets_baked/common_bf{1,2}/locl/english.ron`. Scratch scripts: this file's folder, `t/`.
One pass, no verifier yet.

## Headline (the rules)

- **BF2 countdown** [V]: when every *alive* command post belongs to ONE team (no neutral post, no second owner,
  locals count as an owner), the conquest script starts a **20 s** timer and shows it as "Victory In" to the
  winner's side and "Defeat In" to the other side. Any neutral post or a second owner stops it and hides it; it
  restarts from 20 at the next domination. At 0 the objective completes, and **1 s later** (`dittyTimer`)
  `MissionVictory(winner)`. Defenders may win this way too (PS2 passes `multiplayerRules = true`).
- **BF1 countdown** [V]: per TEAM, not per winner: a team that holds **no post** (its post value 0) **while no post
  is neutral** counts 20 s down (engine, `FUN_002ad830`). A neutral post anywhere **pauses** it (it does not reset);
  re-taking a post resets it to 20. At 0 the team's reinforcements are set to 0. The HUD shows **"DEFEAT IN"** to the
  counting team and **"VICTORY IN"** to the other, with the whole seconds (ceil) in `gamefont_large`.
- **0 tickets** [V]: BF2 — `MissionDefeat(team)` the moment a team's count reaches 0 (Lua), units alive or not.
  BF1 — the engine's game-over test is "a main team's reinforcements < 1"; **2.0 s** later the round ends, units alive
  or not. (This corrects `re-tickets-spawn.md` §2.6/§2.8's "remaining units are the last stand", see §1.4.)
  Neither game ends a round for "no units alive" alone.
- **End of round, BF1** [V]: every player's view is switched to a **random `AddCameraShot` view of the mission**;
  the soundfx bus is faded, every sound stopped, and the listener team's victory / defeat music played; a
  **256 x 64 px banner** from texture `ig_victory_defeat` (top half "VICTORY", bottom half "DEFEAT") is shown at
  the centre of each viewport, tinted green (64,192,64) for victory, red (192,64,64) for defeat. After
  **5 s at least, 7 s at most, and not while the music still plays** the banner is hidden and `ifs_teamstats`
  opens.
- **End of round, BF2** [V]: the same banner object (same texture by hash, same 256 x 64, same UV halves), tinted
  blue (1,86,213) for victory, red (192,64,64) for defeat; same music swap; **no camera change** (the camera-shot
  picker has no call in the game-over driver; jal census). Banner up to **7.6 s** or until the music ends
  (whichever first), then `ifs_teamstats`.
- **Stats** [V]: BF1: TEAM STATS (two team lists: name, kills, deaths, "cpp" column) -> X: PERSONAL STATS of the
  selected row -> X: PLAYER AWARDS -> X: quit; Triangle backs out one screen; on TEAM STATS Triangle quits.
  BF2: Team Stats (two lists: points, kills, deaths, flags, cpp, plus each team's awards below) -> X: Personal
  Stats -> X: career stats; **Square** quits from any of them; Triangle backs out (does nothing on Team Stats).
  Both: 30 s with no input on a stats screen quits by itself. Quitting returns to the shell.

## 1. The countdown

### 1.1 BF2: the Lua (`objectiveconquest.luac`, PS2 bytecode) [V]

- Class constant `defeatTimerSeconds = 20.0` (main pc 16). `Start` (main.f7.f4) creates timer `"defeat"`, rate
  1.0, and on its elapse (main.f7.f4.f0) stops it and calls `self:Complete(self.winningTeam)`.
- `UpdateState` (main.f7.f2), run after every `OnFinishCapture`, `OnFinishNeutralize`, `OnCommandPostRespawn`,
  `OnCommandPostKill` (main.f7.f6-f9, each returns at once once `isComplete`):
  1. first the bleed recompute for both teams (pc 4-11);
  2. loop over `commandPosts`, skipping posts for which `IsObjectAlive(name)` is false (pc 19-23);
     `GetObjectTeam == 0` (neutral) -> `ResetState` and return (pc 27-31); a second, different owner ->
     `ResetState` and return (pc 40-46);
  3. if `disallowDefensiveVictory` and the winner is `teamDEF`, return (pc 53-63); on PS2 `Start` sets
     `disallowDefensiveVictory = not multiplayerRules` = false (main.f7 pc 37-44), so defenders can win;
  4. `multiplayerRules` true (PS2 SP missions): `SetTimerValue(defeatTimer, 20)`, `StartTimer`,
     `SetDefeatTimer(defeatTimer, opposing(winner))`, `SetVictoryTimer(defeatTimer, winner)` (pc 68-92).
     (multiplayerRules false: `Complete(winner)` at once, pc 94-98 — not the PS2 path.)
- `ResetState` (main.f7.f1): `winningTeam = 0`, `StopTimer(defeatTimer)`, then `SetDefeatTimer(nil, ATT)`,
  `SetVictoryTimer(nil, ATT)`, `SetDefeatTimer(nil, DEF)`, `SetVictoryTimer(nil, DEF)`.
- [D] Every new event while one team still holds everything (a destructible post respawned / killed) restarts the
  timer at 20, because step 4 sets the value again.
- `Objective:Complete` (`objective.luac` main.f9): returns if already complete; `MapHideCommandPosts(false)`;
  `isComplete = true`; `BroadcastVoiceOver("common_objComplete")` only when `multiplayerRules` is false (so not on
  PS2 conquest); starts `dittyTimer` at **1.0 s**; its elapse (main.f9.f0) calls `MissionVictory(winner)` (no
  container) — so the round ends **21 s** after domination begins. [V]
- The port today ends at 20.0 s with no 1 s step (`core::battle::BF2_DOMINATION_DEFEAT_SECS`, `core/src/cp.rs`
  `tick_domination`), and counts all posts (no "alive" filter); a neutral post resets it, as BF2.

### 1.2 BF2: the engine side and the HUD [V]

- `SetDefeatTimer` (0x0018f880, B2 34662), `SetMissionTimer` (0x0018f8d8), `SetVictoryTimer` (0x0018f930, B2
  34692) all call `FUN_00361280(timer, kind, team)` with kind -1 / 0 / +1 (B2 34670/34685/34700). There is
  **one slot**: `DAT_0069bc48` timer, `DAT_0069bc4c` kind, `DAT_0069bc50` team (B2 358121-358123). So the
  script's second call wins: the slot holds (timer, **+1 victory**, winner). A nil timer sends
  `victorytimer.disable` and `defeattimer.disable` (B2 358124-358131).
- `FUN_003613d0(viewerTeam)` (B2 358165) returns `kind * relation(viewerTeam -> slotTeam)` (team record
  `+0x84 + slotTeam`, a signed byte). [D] +1 = the viewer is on the winner's side (friendly), -1 = the enemy.
- HUD update `FUN_003b61e0` (B2 411616-411645): when the slot holds a timer, value = timer `+0x24` (seconds left,
  clamped >= 0); result 1 -> HUD event `victorytimer` (`DAT_006a9840`), -1 -> `defeattimer` (`DAT_006a9848`),
  0 -> `objectivetimer` (`DAT_006a9838`). Event names: `FUN_003b0aa8` B2 407867-407872 with strings at 0x630a50 ..
  0x630ab0 = `objectivetimer`, `victorytimer`, `defeattimer` and their `.disable` twins (ELF words read).
- Layout (`engine/assets_baked/common_bf2/hud/1playerhud.ron`, groups `VictoryTimerGroup` from line 5634 and
  `DefeatTimerGroup`; Viewport units, y down, children add to the group's (-0.006754, 0.269734)):

  | piece | victory | defeat |
  |---|---|---|
  | label | Text `common.victoryin` = "Victory In" (BF1 table: "VICTORY IN"), Center/Top, at (0.845387, 0.040171), no font child | Text `common.defeatin` = "Defeat In", Center/Top, at (0.664217, 0.088427) |
  | number | `eventnumber victorytimer`, `numbertotime "%m:%02s"` (so "0:20"), `gamefont_medium`, scale 1.25, textbox 0.296875 x 0.09375, at (0.81051, 0.069316), colour (0, 87, 213) | same, colour (230, 10, 0), **alpha 0.4** |
  | shadow | black twin at (0.81307, 0.071945), z 129 | black twin, alpha 0.7 |

  So the number sits right of centre, about a third down the screen (absolute Viewport (0.80, 0.34)), under the
  minimap. The two labels' x differ in the data (0.84 vs 0.66) [V data; whether that is what the screen shows:
  capture C3].

### 1.3 BF1: the engine countdown and its HUD [V]

- Team update `FUN_002ad830` (B1 238604-238843): for teams 1 and 2 it sums, over every post, the post's
  `ValueBleed` when the team owns it (and counts owned posts), plus 1 per neutral post (B1 238664-238693).
  - value != 0 and owned posts >= 1 -> `+0x60 = 0`, countdown `+0x5c = 20.0` (reset) (B1 238797-238803);
  - value != 0 and owned posts = 0 (i.e. it owns none but a post is neutral) -> **return: the countdown neither runs
    nor resets** (B1 238798-238800);
  - value = 0 -> on the first frame (`+0x5c == 20.0`) the "defeat imminent" line (`FUN_002b1918`, team index 1 only,
    `re-announcer.md` T7); then `+0x60 = 1`, `+0x5c -= dt`; at <= 0: `+0x5c = 0`, initial `+0x14 = 0`,
    reinforcements `+0x10 = 0` (B1 238808-238838).
  - [D] A team that owns only posts of `ValueBleed` 0 (BF1's auxiliary posts) also counts down.
- HUD: inside the reinforcement-bar function `FUN_002da140` (B1 265267-265560): the smallest active `+0x5c` of the
  two teams; "is it my team" picks the key: own team -> `common.defeatin` (0xedd20cde) "DEFEAT IN", the other ->
  `common.victoryin` (0x0634f877) "VICTORY IN" (B1 265498-265501). Seconds = `ceilf` (`FUN_00433f50`, newlib's
  `ceilf`, B1 header line of that function), shown as `%d` (0x540c48) in **`gamefont_large`** (0x540c50) at
  scale **2.0 - 0.05 x seconds** (1.0 at 20 s, growing toward 2.0) (B1 265522-265523); the label sits above, the
  number moved down 15 px (`FUN_004b7f10(0, 15.0, ...)`, B1 265513). The alpha byte jumps to 255 at each new
  second and falls at 512/s to 128 (B1 265472-265478): a pulse per second. Colour: the team-colour table entry of
  the player's team on victory, of the enemy team on defeat (B1 265481-265490) [D: which colours those are, capture
  C4]. Above 59 s the same element shows `%d:%02d` in `gamefont_tiny`, 21-59 s `%d` in `gamefont_medium` — that
  path belongs to a second team-manager timer (`DAT_00518388 + 0xa8/+0xac`, ctor 600.0) whose use was not traced
  [H: an MP time limit].
- BF1 has no `SetVictoryTimer` / `SetDefeatTimer` binding: `analysis/bf1-strings.txt` has no `VictoryTimer` or
  `DefeatTimer` string (grep), and the only "timer" HUD names are `hud_flag_timer` and the performance timer.

### 1.4 Ending at 0 tickets [V]

- BF2: `Objective:Start` registers `OnTicketCountChange(count <= 0 -> MissionDefeat(team))`
  (`dis_objective.txt` main.f7.f1; `re-tickets-spawn.md` §1.3). The banner follows on the same update (§2.2).
- BF1: game-over driver `FUN_0033f718` (B1 328359): `FUN_002acae0(team manager)` (B1 238060) returns a bit per main
  team whose reinforcements `+0x10 < 1`; while it is non-zero a 2.0 s timer `DAT_00564728` (set 2.0 in
  `FUN_0033e590` B1 328137, its only other writer) counts down; at <= 0 the round is over (B1 328723-328746).
  Nothing in that test looks at the alive count, so the no-post countdown's "tickets = 0" ends the BF1 round 2 s
  later even with units on the field — `re-tickets-spawn.md` §2.6/§2.8 ("the remaining units are the last stand")
  reads the team update alone and misses this test [V for the code; one live sample owed, capture C5].
- Neither game has an "all units dead" end: BF2's only Lua ends are 0 tickets, domination, and the opt-in time
  limit (`re-time-limit.md`); BF1's is the mask above.

## 2. The end of the round

### 2.1 BF1 [V]

Driver `FUN_0033f718` (B1 328723-328775), once the 2 s above have run out:

1. `DAT_00519084 = 1` (game over). For each of the 2 player slots: `FUN_0019edf0(camera manager, i)` (B1 44058)
   draws an LCG index modulo the shot count `+0x94` and writes that shot's rotation (`+0xa0 + i*16`) and position
   (`+0x1a0 + i*16`) into the player's fixed camera; `FUN_0019fb10` (B1 44728) makes that camera the active one.
   The shots are the mission's **`AddCameraShot(qw, qx, qy, qz, x, y, z)`** calls (binding at 0x00184750, B1 26791:
   up to 16). The same random picker serves BF1's spawn screen (`FUN_002de5c0`, B1 267965). jal census (ELF words):
   2 calls of 0x19edf0 (0x2dee88, 0x340510), 2 of 0x19fb10 (0x340520 in this driver, 0x340c8c).
2. `DAT_0061ca98` = **7.0 s** in single player (5.0 in a network game) (B1 328737-328744).
3. `FUN_002be838(5.0, banner object, losers mask)` (B1 249713): on a non-network game, for each viewport: banner
   visible (flag 0x100), colour = `DAT_005b05b0[i]`, UVs = `DAT_005184a8[i]`, with i = 1 if the viewer's team is not
   in the losers mask (victory) else 0. Colours set at B1 249961-249968: [0] = (192, 64, 64, 255),
   [1] = (64, 192, 64, 255). UVs (ELF words at 0x5184a8, `FUN_004bbde8(u0, v0, u1, v1)` in 1/2048 units, B1 header
   of that function): [0] = (0, 0.5)-(1, 1) bottom half, [1] = (0, 0)-(1, 0.5) top half. Then soundfx bus
   (0xeceb308a) to 0, every sound stopped (`FUN_00356000`), the listener team's DefeatMusic / VictoryMusic played
   (B1 249766-249776; `re-ambience-music.md` 3.13). The 5.0 is stored at `+900` (minimum banner time).
4. The banner: texture `ig_victory_defeat` (set by `FUN_002be770`, B1 249684), 256 x 128 RGBA, baked as
   `engine/assets_baked/common_bf1/textures/ig_victory_defeat.ktx2` (decoded here: white-to-grey bevelled capitals,
   "VICTORY" on the top half, "DEFEAT" on the bottom). Size 256 x 64 px, centred (`FUN_004b5e28(256, 64, el, 1, 1)`,
   0x5184c8/cc = 256/64), in a container placed at (0.5, 0.5) of each viewport (`FUN_00479f00(0.5, 0.5, ...)` in
   the ctor `FUN_002be458`, B1 249607-249616). [H] The vertex colour multiplies the texture with the PS2's 0x80 = 1.0
   rule, so 192 reads brighter than the texture's own grey — capture C1.
5. It ends when `DAT_0061ca98 <= 0` (7 s) **or** `FUN_002bec60`: `+900 <= 0` (5 s) **and** none of the four music
   handles is playing (`FUN_002beca8`, B1 249878-249905). [D] With BF1's takes (victory 8.4 s, defeat 4.9 s,
   `re-ambience-music.md` 3, table) a win shows 7 s, a loss 5 s.
6. Then: banner hidden and the music bus 0x985c8f54 faded (`FUN_002beb60`, B1 249810); `DAT_00564738 = 1` (from now
   on `FUN_0033f718` only runs the stats-mode update `FUN_00340798`); for each player `FUN_002d2ca0` (B1 260809)
   marks the player and, for player 0, opens shell screen **`ifs_teamstats`** (0x540650); every sound stopped; a
   sound from the mission-status object (`FUN_002b15b8(DAT_005644a8)`) played with flags 0x12 [H: the stats-screen
   music].
- Not settled from code: whether the world keeps simulating behind the shot, and whether the HUD is hidden
  (capture C1).

### 2.2 BF2 [V]

- Driver `FUN_00425840`: when the winners mask `DAT_0066133c` (written by `MissionVictory` / `MissionDefeat`,
  `re-ambience-music.md` 3.13) becomes non-zero and the game-over flag `DAT_00600b3c` is clear, it sets the flag
  and, on that update, calls `FUN_00380c30(5.0, banner, mask)` and `FUN_00397840(player-0 screen, 0)`
  (B2 482557-482576, 482747-482751). No 2 s wait.
- `FUN_00380c30` (B2 379625-379706) is BF1's `FUN_002be838` twin: per viewport, banner on, colour
  `DAT_006a2e80[i]` with i = the viewer team's bit in the WINNERS mask, UVs from 0x5feff0 (same two halves); the
  table (B2 379974-379981): [0] = (192, 64, 64, 255), [1] = **(1, 86, 213, 255)** (BF2's friendly blue, the same as
  the reinforcement counter's). Texture hash 0x0623b25a = FNV(`ig_victory_defeat`) (`FUN_00380b68`), size 256 x 64
  (0x5ff010/14), shown when `DAT_005ff018` = 1 (ELF word: 1). Bus 0xeceb308a to 0, sounds stopped, the listener
  team's victory / defeat music. Banner timer `+0x340 = 7.6 s` (0x40f33333).
- End test `FUN_00381358`: not done while the timer > 0 **and** a music handle plays (`FUN_00381458`); so it ends at
  the timer or at the end of the music, whichever is first [D]. Then `FUN_00380f48` hides it, per player
  `FUN_0038b190` -> `FUN_0038ba80` opens **`ifs_teamstats`** (0x62e588), awards computed (`FUN_00364da0`, SP only),
  sounds stopped and the mission-status sound played (B2 482782-482811).
- **No camera change**: the shot picker `FUN_001b9d00` has 3 jal calls in the ELF (0x3977e8, 0x397fe0, 0x39bb1c),
  all in the spawn-screen functions `FUN_00396a00`, `FUN_00397f68`, `FUN_0039ba18`; none in `FUN_00425840`
  (0x425840-0x426918). The view stays the player's last camera; whether the world keeps running (the "frozen
  frame") is not settled from code (capture C2).
- The BF2 copy of the texture is in `extracted/bf2/lvl/DATA/_LVL_PS2/COMMON.LVL` (6 ASCII hits); it is not baked
  (`common_bf2/textures` has no such file). Whether its art matches BF1's: not checked.

## 3. The stats screens

Engine side [V]: both games open shell screen `ifs_teamstats` per player (BF1 `FUN_002d2ca0`, BF2 `FUN_0038ba80`).
`ScriptCB_QuitFromStats` — BF1 (0x0017cc50, B1 22106): flags each player and sets the next level name to "" via
`FUN_00340fb8` [D: back to the shell]; BF2 (0x0017f7b8): `FUN_0038b288` (not opened).

### 3.1 BF1 (PS2 bytecode; the PC source differs) [V]

| screen | shows | buttons | ends |
|---|---|---|---|
| TEAM STATS (`ifs_teamstats.luac`) | black backdrop (`opaque_black`); two lists (left = team 1, right = team 2) with team icons (`all/imp/rep/cis_icon`), column headers `stats_kills`, `stats_deaths`, `stats_cpp`; rows filled by `ScriptCB_GetTeamstats` (B1 22355): every member of the team sorted, columns = the three member stats `FUN_002a64e8 / 2a6618 / 2a6660` (the ones the exhaustion cull scores as A - 2B + 5C) [D: kills, deaths, command-post points]; the player's row in its own colour and the cursor starts on it | up/down move, L1/R1 page, left/right switch team list; **X: PERSONAL STATS of the selected row** (helptext "DETAILS"); **Triangle: quit** (helptext "DONE") | Triangle, or 30 s idle |
| PERSONAL STATS | title `ifs.stats.personalstatstitle` in `gamefont_large`; the row's numbers (`ScriptCB_GetPersonalStats(team, idx)`) | **X: PLAYER AWARDS**; Triangle: back | Triangle, or 30 s idle -> quit |
| PLAYER AWARDS | `ScriptCB_GetAwardStats()` list | **X: quit** (sound `shell_menu_enter`); Triangle: back | 30 s idle -> quit |

Read in the bytecode: TEAM STATS `Input_Accept` (main.f8) pushes `ifs_personalstats`, `Input_Back` (main.f9) calls
`ScriptCB_QuitFromStats`; helptexts `ifs.stats.done` / `ifs.stats.details` (main.f18); no sort and no awards button
(the PC source's `_sortKills`, `ifs_awardstats` push are absent from the PS2 file). PERSONAL STATS f5 pushes
`ifs_awardstats`, f6 pops; AWARDS f5 quits, f6 pops. `fMAX_IDLE_TIME = 30` (PUSHINT) in all three; the Update
function quits when the idle time runs out (f16 / f11). `gE3StatsTimeout` is 0 outside the E3 build, so no forced
wait.

### 3.2 BF2 (PS2 bytecode) [V]

| screen | shows | buttons | ends |
|---|---|---|---|
| Team Stats (`ifs_teamstats.luac`) | `statsscreens_bg`; title "Team Stats"; two team lists with column icons `points`, `stats_kills`, `stats_deaths`, `stats_flags`, `stats_cpp`; under them, title "Awards" and each team's award list (`ScriptCB_GetAwardStats(0)` / `(1)`, main.f4); the player's row highlighted | up/down, L1/R1, left/right as BF1; **X: Personal Stats** (main.f11); **Square: quit** (main.f12, helptext `btnmisc` "Done"); Triangle does nothing (main.f13) | Square or 30 s idle (main.f20) |
| Personal Stats | the selected row's numbers (`ScriptCB_GetPersonalStats`, main.f8) | **X: career stats** (`ifs_careerstats`, main.f10); **Square: quit** (f11); Triangle: back (f12) | 30 s idle |
| (`ifs_awardstats`) | loaded by `ingame/game_interface.luac` but pushed by no PS2 script (grep of all `bf2/lua/*/*.luac`: only its own file and `game_interface` name it; the ELF has no `ifs_awardstats` string) | | |

`fMAX_IDLE_TIME = 30.0` in all three (main pc 73 / 45 / 22).

## Current port status (2026-11-12)

- The end camera uses the mission shots. The HUD uses the baked
  `ig_victory_defeat` sheet and keeps the banner for the selected end
  music's length, clamped to 5–7 s, with 5 s as the fallback
  (`game::round_end`, `game::music`, `ui::hud`).
- Team, Personal and Career Stats use the BF2 presentation on both
  games' maps. Team rows track points, kills, deaths and captures.
  The capture column uses `stats_cpp`. Its original meaning still
  needs a nonzero stock witness.
- The original lets the team scoreboard open any character's Personal Stats,
  then only offers Career Stats for human rows. Its Personal Stats has no
  row cursor or directional handlers (ifs_personalstats main PCs 113–119
  and main.f10). The port keeps all scoreboard rows browsable, opens the
  local player's Personal Stats on Continue, and adds a focus bar and menu
  clicks. The future split-screen and LAN flow is in the feature set.
- The round-award callbacks select the highest individual value
  (BF2 FUN_00365010 and FUN_00365118, B2 361042–361130).
  Killing Spree uses kills spaced at most four seconds apart, Public
  Enemy uses kills within one life, and Survivalist counts kills
  below 25% health (B2 359243–359276). Camper measures time within
  10 m of a reference position (FUN_00363948, B2 359907–359942).
  Dead Eye and the Personal Stats “Head Shots” row share the critical
  counter (B2 47580–47583 and 361120–361122).
- The local review implements these awards from match events. The port
  aggregates across lives and reads the shooter's current health at the
  event tick. Native hero and team partitions and exact original
  countdown, banner and stats transitions still need the comparisons
  below. The local #130 review adds separate career medal counts.

## Original implementation plan

The maintainer's target is BF1's ending. For the countdown the two games differ; the deviation to name is which
rule both maps get. Suggested: **BF2's domination rule** (it is the port's rule today, and BF1's per-team rule is
the same thing whenever there are two teams and no neutral post), shown with BF2's HUD element, plus BF2's 1 s
step; BF1's "neutral pauses" quirk dropped.

1. **Sim: expose the countdown.** `Sim::domination_left() -> Option<(winner, secs)>` from the existing state, and
   add the 1 s `dittyTimer` (outcome at 21 s). Test: a fixture holds all posts, read 20.0 -> 0, outcome at 21.0;
   a neutralised post clears it.
2. **HUD: Victory In / Defeat In.** Key `common.victoryin` / `common.defeatin` by the player's side, number
   `m:ss`, BF2 layout values from `1playerhud.ron` (§1.2). Test: model unit test for both sides + a still against
   capture C3.
3. **Round-over phase in the game loop** (the state machine): outcome -> (BF1: 2 s; BF2: 0 s — pick one, name it)
   -> banner phase -> stats phase -> shell. Test: tick a fixture through the phases with a fake clock.
4. **End camera.** On entering the banner phase, switch the view to a random `camera_shots` entry
   (`spawn_backdrop` picker, a seeded draw); hide the HUD [H until C1]. Test: the camera equals shot k for the seed.
5. **Banner.** Draw `ig_victory_defeat` (bake BF2's copy or use BF1's on both) 256 x 64 at the viewport centre,
   top half tinted green on a win, bottom half red on a loss (or BF2's blue: pick one). Test: a still per outcome.
6. **Music + silence.** Stop sounds, fade the soundfx bus, play the faction's victory / defeat music (the AMBM
   card's music player); banner time = clamp(music length, 5, 7) s (BF1). Test: phase length 7 s on a win, 5 s on
   a loss with the disc takes.
7. **Team Stats screen.** Two lists, the player's row selected, columns kills / deaths / (cpp; BF2 adds points,
   flags), X -> personal, quit key, 30 s idle quit. Needs per-unit command-post points kept by the sim. Test:
   screen model from a scripted battle; idle quit at 30 s.
8. **Personal / awards screens** (BF1's chain or BF2's awards-on-team-stats: pick one), then **back to the shell**.

## Open questions / captures needed

- **C1 (BF1, 5 min):** end a BF1 conquest round (knock a team's reinforcements to 0 from a save state) and record
  the screen from the last frame of play to TEAM STATS: the shot's view, the banner's look and tint, whether the
  HUD is hidden, whether soldiers keep moving behind the banner, how long the banner stays (expect 7 s on a win,
  5 s on a loss).
- **C2 (BF2, same):** the same on BF2 with `MissionVictory` (domination) and with 0 tickets: is the frame frozen
  (sim paused) or live; banner time (expect min(7.6 s, music)).
- **C3 (BF2):** a domination countdown on screen: "Victory In" and "Defeat In" (the two label positions differ in
  the data), the number's format at 20 s and near 0 (does `%02s` round up or down).
- **C4 (BF1):** the no-post countdown: label + number position, the colours, the pulse; and confirm that a neutral
  post freezes it (1.3).
- **C5 (BF1):** confirm the 2 s end after the no-post countdown zeroes the reinforcements while soldiers are alive
  (contradicts `re-tickets-spawn.md` §2.6/§2.8).
- Not traced: BF1 manager timer `+0xa8/+0xac` (the >= 60 s HUD path); what `FUN_002b15b8(DAT_005644a8)` /
  BF2 `FUN_00373e38` play on the stats screens; what BF2 `FUN_0038b288` (QuitFromStats) does next; the exact
  meaning of BF1's third stat (`stats_cpp`) and BF2's points; BF1/BF2 per-row colour constants (`DAT_0069c420..`,
  `DAT_0076c200..`); the BF2 team-relation byte values (+1 / -1 assumed).
