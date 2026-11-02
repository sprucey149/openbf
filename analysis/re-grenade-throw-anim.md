# OPUS RE (2026-10-22) - [O] UNAUDITED

# The grenade throw on the soldier: which clip, which layer, when it starts, when the grenade leaves the hand (BF1 and BF2, third and first person)
AUDIT 2026-10-22 (loop lane 1): PARTLY - the clips, sets, players, release updates, fire-state lengths and both
first-person tables hold, re-measured on 22 new live throws in 20 trials (11 per game, the auditor's own sampler). Corrected: BF1's
channel rule, so a held L1 shows the rifle's reload clip (§1.2, §1.7, new §10.1); BF2's legs rule: the movement id at
the start, a crouch held over 1 s, leaving on movement (§0, §2, new §10.2); the clip phase at the release is one update
later than written (§0, §3, §10.3); counts and citations (§2.1, §4, §6, §8). Evidence
`engine/artifacts/lead_tmp/audit-gthrw/` (`ps2/`, `wf/results.json`).

Card GTHRW, lane 2. Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`; ELF words by
`off = va - 0x160000 + 0x1000`; the discs' `zaa_` banks and class PROPs; two live PCSX2 runs. Tags: **[V]** read at the
cited lines, **[M]** measured (PS2 memory, the disc), **[D]** derived, **[H]** hypothesis; `[O]` marks the load-bearing
claims. Builds on `re-soldier-anim-layers.md` (the two clip players, the sets, the action layer; its sections are
cited as AL n) and `re-grenades.md` (the throw itself; cited as GR n). Scratch, tools and rows:
`engine/artifacts/lead_tmp/gthrw/` (`census.py`, `offhand.py`, `fpnames.py`, `fptab.py`, `fpclips.py`, `elfstr.py`;
`ps2/throw_strip.py`, `ps2/rows.py`, `ps2/throw_sheet.py`; outputs `*.txt`, `ps2/out/`, `ps2/*_sheet.png`).

## 0. What the PS2 does, in one page [O]

| | BF1 | BF2 |
|---|---|---|
| set the throw is drawn from | the GRENADE's own weapon set (`AnimationBank grenade`, set 3): the soldier's in-hand weapon becomes the grenade for the throw | the PRIMARY's set: a weapon with `OffhandWeapon` (every thermal) never becomes the animator's weapon; channel 0's current weapon keeps the set |
| clip | `human_grenade_stand_shoot3` (51 frames, 1.70 s) for every thermal / crack / haywire (strength 1.0 -> fire state 3 -> upper action shoot3); crouched the same clip; prone `human_grenade_prone_shoot` (36) | the primary set's `shoot_secondary2` - for a rifle `human_rifle_stand_shoot_secondary2_full` (41 frames, 1.333 s); bazooka / tool / pistol primaries their own upper clip (`human_bazooka_` / `human_tool_stand_shoot_secondary2`, 41) over the rifle's legs; underhand (strength <= 0.4) `shoot_secondary` (31); crouched the stand clips; no prone clip in the human gun sets |
| when it starts | in the update of the L1 press: the fire entry raises the weapon's start flag, the animator driver passes the fire state on in that update | the same: the fire entry raises the "fired" bit 33 at the press |
| layer | the upper player (13 bones); the legs play it too while the movement id is <= 2, checked every update (crouched still is id 0: the legs take the stand clip too), with 0.5 s more blend; running, the legs keep the run and join the throw once he stops | the upper player; the legs play the action's lower slot only if the movement id (+0x20c0) is 0 in the action's first update - still, and not crouched still for over 1 s (id 2, the kneel stays); they leave it for the walk / run clip as soon as he moves and never rejoin (§10.2) |
| the grenade leaves the hand | at weapon timer `InitialSalvoDelay` = 0.300 s; the animator's counter (+0x2084) then reads 0.267 s, the upper player's own phase 9/51 = 0.300 s (§10.3) | at 0.533 s (the first 30 Hz update past 0.51); the upper player's phase is then 16/40 = 0.533 s, not 0.50 s (§10.3) |
| end | the clip's length, or earlier when the fire state ends (ShotDelay + 0.3 = 1.6 / 1.7 s) and the channel returns to 0, which changes the set and clears the action; while L1 is still held the channel stays 1 and the rifle's reload clip plays (§10.1) | the clip's length (1.333 s); the fire state runs on to 1.81-1.84 s under the idle; a held L1 changes nothing |
| what the hands hold | nothing: the in-hand weapon is the grenade, hidden by `HideOnFire` from the press | the rifle stays in the right hand; the left arm throws |
| first person | `_1st_grenade_shoot2` (11 frames) on the FPM arms for fire states 2-3 (`_1st_grenade_shoot`, 31, for state 1) | `humanfp_grenade_shoot2` (11) for the overhand, `humanfp_grenade_shoot` (31) underhand |

Measured live (section 5): BF1 3 of 3 throws and BF2 2 of 2 hold exactly these clips on these players at these
updates; picture strips `lead_tmp/gthrw/ps2/bf1_throw_sheet.png`, `bf2_throw_sheet.png`.

The port as of 2026-11-01: each map's game throws its own column above, for every class on the map. A class picked from
the other disc takes the map's throw: a BF2 soldier on a BF1 map plays BF1's clip, lent at load (`fit_throw_to_map`), with
his side's BF1 thermal at the BF1 count; a BF1 soldier on a BF2 map plays the BF2 clips the bake lends him. The HUD is
BF2's on every map. Section 7 is the build's original change list, as of 2026-10-22.

## 1. BF1, third person [V][M]

1. **The press.** The base weapon update (B1 `FUN_00284318`, GR 3.2) enters the fire state `FireAnim + 1` (= 1) and
   calls the fire entry. The grenade's fire entry `FUN_0028eb30` (B1:215619-215657; vtable 0x53bf48 slot +0x74, word
   0x28eb30 at 0x53bfbc): weapon+0xbc = 1 when `HideOnFire` (class+0x1f0) (215630-215631); strength +0x144 = the hold
   time +0xd0 / `MaxPressedTime` (class+0x1ec) when that is > 0, else 1.0 (215635-215641); with the variant byte
   class+0x1f4 at its default -1 the state +0xcc becomes **3 for strength > 0.7, 2 for > 0.4**, else stays 1
   (215643-215651; ELF 0x3f333333 at 0x28ebb8, 0x3ecccccd at 0x28ebd8); a byte >= 0 gives byte + 1. Then it calls
   vtable slot +0x50 (215656) = `FUN_002859f8` (word 0x2859f8 at 0x53bf98), which sets the **start flag weapon+0xc0
   = 1** (B1:209378). `[O]`
2. **The channel.** `FUN_00205cc8` (B1:117108-117115) makes channel 1 active while its current weapon's state is 1-4,
   so from the press the soldier's active weapon is the grenade (GR 2.3). AUDIT: also while channel 1's fire button is
   held or was released this update, unless the weapon is in state 8 (B1:117118-117161; §10.1).
3. **The driver** `FUN_002013a0` (B1:113796-113836), every posed update, for the ACTIVE weapon: `FUN_0022a938`
   (animator, weapon+0xec) - the weapon set; then the state: with the start flag down, states 1-4 become 0; with it up
   the state passes and the flag is cleared; `FUN_0022a958`(animator, state). So **the fire state reaches the
   animator in the update of the press, once** (a reload state 5 passes every update; with the flag up and the
   globals `DAT_00518be4` / `DAT_00518be8` set - a network session, AL 4, [H] - any state but 2 or 3 is forced to 1).
   The flag's other writers: the weapon constructor (0, B1:207927) and the throw `FUN_0028ef70`, which clears it when
   `InitialSalvoDelay` > 0 (B1:215917) - 0.3 s after the press, long after the driver took it (the live rows show the
   action in the press update). AUDIT: one more writer outside the export, a 2-instruction stub at 0x288240
   (`jr ra; sw zero,0xc0(a0)`), is unreferenced: no data word 0x288240 and no jal 0x0c0a2090 in either ELF
   [receipt:283c29c9]. `[O]`
4. **The animator.** `FUN_0022a938` (B1:142251-142261): a new set zeroes the upper action. `FUN_0022a958`
   (B1:142264-142276) stores actions 1-5; only actions 1 and 4 restart the upper clip and arm the 2.0 s alert - a
   throw (action 3) does neither. `FUN_0022aa10` case 0x20 (B1:142629-142644, 142715-142732): the upper player plays
   `U[stance][set][action]`; the lower player plays it too, with 0.5 s more blend, when the movement id is <= 2; the
   action ends when its clip time passes frames / 30 (AL 3.2).
5. **The clip.** Set 3's table (`lead_tmp/anim/sets_bf1.txt`, AL 2.2): stand shoot `grenade_stand_shoot` (54
   frames), shoot2 `grenade_stand_shoot2` (62), shoot3 `grenade_stand_shoot3` (51), charge `grenade_stand_charge`
   (46); crouch shoot/2/3 = the stand clips (AL 2.2 step 3); prone shoot/2/3 = `grenade_prone_shoot` (36); reload =
   the rifle's `rifle_stand_reload` (75). Every BF1 thermal, crack and haywire has `MaxPressedTime` 0 (or none) and no
   variant byte, so it throws at strength 1.0 into state 3; the Ewok rock and spear author the byte 2, state 3 as well
   (section 6). So **every BF1 throw plays `grenade_stand_shoot3`**. `[O]`
6. **The release.** The fire update `FUN_0028ec38` (B1:215661-215685) throws at timer >= `InitialSalvoDelay`
   (class+0x1e4; 0.3 on every BF1 grenade) when the weapon is still selected (+0xc4; a deselected one is marked
   thrown first, 215669-215671) and the clip holds a round. The clip's time starts at 0 in the press update, when the
   weapon's timer already reads one update, so the grenade leaves at **clip time 0.267 s** [M]. AUDIT: 0.267 is the
   animator's counter +0x2084; the upper player's own phase after the release update is 9/51 = 0.300 s (§10.3).
7. **The end.** The fire state lasts `InitialSalvoDelay + ShotDelay` (1.6 s Rebel, 1.7 s Imperial). At its end the
   weapon enters reload (5): in that one update the driver passes 5 and the grenade set's reload slot (the rifle's
   reload clip) is set at clip time 0; the next update channel 0 is active again, the set changes back and clears it
   - so the reload clip is seen for that one update only [M], WHEN L1 is up by the fire state's end. AUDIT: with L1
   held (or pressed again during the reload) channel 1 stays and the reload clip plays on both players (§10.1) [M].
8. **What is drawn in the hand** [M, picture]: from the press the hands are empty (the in-hand weapon is the grenade,
   hidden by `HideOnFire`; `FUN_00285650` returns at once on +0xbc, GR 3.2) until the channel returns to 0.

## 2. BF2, third person [V][M]

1. **Offhand.** Key 0x2ecd946f = FNV("OffhandWeapon") sets bit 55 of class+0x1e0 (`FUN_00328fb0` B2:320344-320349);
   the base class constructor `FUN_00327e98` clears it - the default 0 (B2:319551); a class copies it from its
   parent (`FUN_003283d8`, B2:319732); the grenade constructor `FUN_00337fd0` does not touch class+0x1e0
   (disassembly 0x337fd0-0x3380ac: stores at +0x1c, +0x108, +0xac, +0x240-+0x259, a memset of +0x260). On the disc
   it is authored by 195 PROP chunks (181 = 1): every thermal and the Ewok rock, and every mine / time bomb / turret /
   powerup dispenser, detpack, officer buff, wrist rocket and remote droid (`offhand.txt`: 82 classes) [M]. AUDIT: 195 /
   181 reproduce; "82 classes" does not - 22 classes author the key, 66 resolve to 1 through their parents (dispensers
   19, remote 16 incl. 11 vehicle / turret guided rockets, cannon 16 incl. force lightning / pull / push, grenade 9,
   detpack 5, force choke 1), 7 to 0 (invisibility, the award rocket launchers). BF1 has no
   such key (section 8). `[O]`
2. **The driver** `FUN_00239bb0` (B2:153694-153721): the active weapon = slot (bits 16-19 of soldier+0x7e0); when
   its class is offhand (and the slot index is >= 0), the weapon whose set the animator takes becomes channel 0's
   current weapon (when channel 1 is active and cur[0] holds one) or slot 0 (when the index is not 0), and a
   `secondary` flag is raised. Then (B2:153826-153866) `FUN_0027c650`(animator, that weapon+0xd0) - **the primary's
   set** - and the ACTIVE weapon's state, passed on only in the update its bit 33 is up (then cleared; in a network
   session a raised bit forces 1 or 2), to `FUN_0027c738` with force = (state 1-2) and `secondary` - when the
   set's weapon's vtable+0x5c returns 0 (the other branch is the melee path [H]). `[O]`
3. **The fire entry** 0x337678 (vtable 0x628f98 slot +0xa8, word at 0x629040; not in the decompile): bit 32 (hidden)
   when `HideOnFire`; strength as BF1; variant a1 = the byte class+0x259 if >= 0, else **1 when strength > 0.4**
   (0x3ecccccd at 0x337700), else 0; state = a1 + 1; the delay index +0x122 = a1, or 2 when prone; **bit 33 set at
   the press** (0x337754-0x337788); limit = `ShotDelay` + `InitialSalvoDelay[index]`. `[O]`
4. **The action** `FUN_00281630` (B2:203802-203837): state 1 -> shoot / **shoot_secondary** when secondary; 2 ->
   shoot2 / **shoot_secondary2**; 3, 4 -> charge, reload for a primary only. `FUN_0027c738` (B2:200723-200784) keeps a
   running shoot-type action (0-3) when no new one comes (it only clears two flag bits of +0x20e0 and stores +0x88).
5. **The clip** (RAM sets of a Mygeeto state, `lead_tmp/anim/ram_sets_bf2_myg1.txt`; disc frames `fpclips_bf2.txt`):
   (human, rifle) stand `shoot_secondary` = `human_rifle_stand_shoot_secondary_full` (31 frames), `shoot_secondary2` =
   `human_rifle_stand_shoot_secondary2_full` (41); crouch = the stand clips; bazooka set: upper
   `human_bazooka_stand_shoot_secondary(2)` (31 / 41, 17 tracks) over the rifle's full clip on the legs; tool and
   pistol sets: `human_tool_stand_shoot_secondary(2)`; officers: `clonecommander_*_shoot_secondary2_full` (36),
   `magnaguard_*_shoot_secondary2_full` (46); the super battle droid its own `shoot_secondary` (36). No human gun or
   tool set (rifle, bazooka, tool, pistol) fills a prone throw pair (the human sabre combo set does, with unresolved
   clips) [M; "no such clip anywhere on the disc" is [H]: one of two verifiers found the name search too narrow].
   **A thermal (strength 1.0) plays shoot_secondary2.** `[O]`
6. **The hand.** `FUN_003383f8` (B2:330057-330155) samples the same bank at the variant's `InitialSalvoDelay` for the
   launch offset - action 2 at class+0x244 and action 3 at +0x248 (B2:330133, 330142) - the clip pair the throw plays.
7. **Release and end.** `FUN_003377d0` (B2:329519-329554) throws at timer >= `InitialSalvoDelay[index]` (0.51 for
   every thermal) while the weapon is selected (bit 34) and the ammo object holds a round (and, for a class with an
   energy cost, the heat allows it) [V]; live at 0.533 s = clip time 0.50 s [M] (AUDIT: the upper player's phase
   after that update is 16/40 = 0.533 s, §10.3). The clip ends at its length (41
   updates); the fire state runs to 1.835 s, then state 4 for one update. The set never changed, so nothing is cut.
8. **What is drawn** [M, picture `ps2/bf2_stand_detail.png`]: the rifle stays in the right hand through the throw;
   the left arm swings back and over; the grenade appears in the air at the release. Which weapon BF2's render draws
   while an offhand weapon is active was not read [H].

## 3. Timing, both games [M]

| | BF1 (Rebel thermal) | BF2 (thermal) |
|---|---|---|
| press -> clip on the players | the next update (+0.033 s) | the next update (+0.033 s) |
| grenade leaves | +0.300 s, clip time 0.267 s, frame 7.9 of 51 (AUDIT: the player's phase 9/51 = 0.300 s) | +0.534 s, clip time 0.50 s, frame 15 of 41 (AUDIT: phase 16/40 = 0.533 s) |
| clip runs | to +1.602 s (cut at the fire state's end, 48 updates) | 41 updates, to +1.368 s |
| fire state ends | +1.635 s (reload 1.0 s) | +1.869 s (state 4) |

## 4. First person, both games [V][M]

Both PS2 games draw an FPM arm model per class in first person (`FPM/<side>/<class>.LVL`: e.g. `imp_1st_inf_
stormtrooper_arms` / `_hands`) posed by a 9-track rig, with their own animator and clip table:

- **BF1:** `FUN_001a6880` (B1:49860-49919) fills `DAT_0057f638` with the CRCs of 66 strings `_1st_<set>_<action>`
  (ELF 0x52ccf8-0x52cfe0), 6 rows x 11 columns (idle, run, shoot, shoot2, charge, reload, repair, jump, flail,
  handsdown, land) (AUDIT: 59 writes of 35 distinct strings, `_1st_tool_idle` filling unused columns; rows 0-4 are
  full, the droideka's row 5 has 4 entries and `FUN_001a38e0`'s row switch never picks it; the resolver count
  `DAT_00517440` is 59); `FUN_001a4ab8` resolves them in the bank `human_animation_fp` (COMMON.LVL; 35 of its 38 clips are
  these names, no prefix [M]). Row 4 is the grenade's: `_1st_grenade_shoot` (31 frames), `_1st_grenade_shoot2` (11),
  `_1st_grenade_charge` (5), the tool's idle / run / reload / jump.
  `FUN_001a38e0` (B1:47445-47683): the weapon is the active one while it fires (states 1-3), else channel 0's; the
  row by its set: 0 / 7 -> 0 rifle, 8 -> 1 lascannon, 1 -> 2 bazooka, 3 -> 4 grenade, else 3 tool; on the update of
  the start flag the column is 2 for state 1 and 3 for states 2-3; state 5 -> 5. So a BF1 thermal (state 3) plays
  **`_1st_grenade_shoot2`** in first person. `[O]`
- **BF2:** the table at 0x618440 (`FUN_001c1260` B2:68986-69000; empty -> `humanfp_tool_idle`): rows rifle, bazooka,
  tool, grenade (row 3: `humanfp_grenade_shoot` 31, `humanfp_grenade_shoot2` 11, `humanfp_grenade_charge` 5), then
  the droideka's. `FUN_001beeb8` (B2:67022-67045): row 3 for an offhand weapon, else rifle 0 / bazooka 1 / other 2.
  `FUN_001bfcb0` (B2:67838-68061): on the update of bit 33, column 2 for state 1 and 3 for state 2 (column 9,
  handsdown, when the class's bit 60 is set); state 4 -> 5. A thermal plays **`humanfp_grenade_shoot2`**. `[O]`
- The port has no first-person view on foot (only the scope view, which draws no body): this half of the card needs
  that view first (card FPV).

## 5. Live runs (PCSX2, 2026-10-22) [M]

`lead_tmp/gthrw/ps2/throw_strip.py bf1|bf2` (a copy of the ROLLS kit): the no-AI side-select states (BF1 Naboo
Theed `bf1_nab2_units1_gcw_sideselect.p2s`, the Rebel soldier; BF2 Coruscant `bf2_cor1_ai0_gcw_sideselect.p2s`),
L1 tapped standing and running forward; one row per game update of the soldier block, his animator block and his
channel-1 weapon (BF1 vtable 0x53bf48, BF2 0x628f98 - both the thermal), plus a video. `rows.py out <game> <trial>
--summary`:

| trial | first fire update | upper / lower clip while the throw runs | release | after |
|---|---|---|---|---|
| BF1 stand | +0.033: state 3, channel 1, set 3, action 3 | `grenade_stand_shoot3` / same, 48 updates | +0.300 (clip 0.267) | +1.635 reload: `rifle_stand_reload` at clip 0 for 1 update; +1.668 set 0, idle |
| BF1 run | the same | shoot3 over `rifle_stand_runforward` (30), `walkforward` (3), then shoot3 on both (15) | +0.300 (0.267) | the same |
| BF1 stand 2 | the same | shoot3 / same, 48 | +0.300 (0.267) | state 8 (his last grenade) |
| BF2 stand | +0.033: word 0x120200, state 2, variant 1, set 0, upper state 3 | `human_rifle_stand_shoot_secondary2_full` / same, 41 updates; then the alert idle | +0.534 | +1.869 state 4, idle |
| BF2 run | the same | shoot_secondary2 over `runforward` (1), `standalert_runforward` (29), `walkforward` (2), alert idle (9) | +0.534 | the same |

A fourth BF1 trial had no grenade left; BF2's run ended when the player died after the running throw (the spawn
screen came up), so BF2 has two throws. Strips: `ps2/bf1_throw_sheet.png` (three throws, tiles at -0.07 .. +1.30 s
from the first fire update, picture lag 0.07 s taken off), `ps2/bf2_throw_sheet.png`, `ps2/bf2_stand_detail.png`.

## 6. Disc census [M]

`census.py` (every weapon whose root BASE is `grenade`, keys resolved parent first, the last PROP wins):
- BF1, 16: thermal (4 sides + GAR), crack (4), haywire (2) - `AnimationBank grenade`, `MaxPressedTime` 0 or none,
  no variant byte -> strength 1.0, state 3 (shoot3); Ewok rock and spear - grenade bank, variant byte 2 -> state 3;
  the three mortar launchers - banks rifle / bdroid, variant 0 or `MaxPressedTime` 0.75.
- BF2, 14: eight thermals (all, cis, com, gar, gun, imp, rep, wok; AUDIT: the doc said nine, which with the rock,
  the spear and four mortars made 15) + the Ewok rock - no `AnimationBank`, `OffhandWeapon 1`, `MaxPressedTime` 0 ->
  variant 1, shoot_secondary2, release 0.51; the Ewok spear - not offhand, release 0.3; four mortar launchers - bank
  rifle, not offhand, variant 0 (`ForceFireAnimation` 0, authored on com_ only). The variant byte is
  `ForceFireAnimation` (FNV 0x079c1d86) in both games: B1 class+0x1f4 (setter B1:215992-215995, stored as read), B2
  class+0x259 (setter B2:329981-329988, a value >= 2 stored as -1, i.e. automatic).
- `InitialSalvoDelay`: BF1 0.3 on all 13 grenade-bank weapons; BF2 0.51 on the eight thermals and the rock.

## 7. The port against this - the build (card GTHRW, phase build)

Today (`game/src/soldier_vis.rs` `step_on_foot`, `game/src/animator.rs`): `fired` comes only from `Event::Shot`;
`Event::Throw` comes at the release and is not read; the set is the in-hand weapon's (`bank_of`, BF2 grenades fall to
"rifle"); `ActionKind` has Shoot and Reload; BF2's Shoot maps to `shoot`. The bake already holds every clip named
above (the BF1 grenade set and BF2's `shoot_secondary*` pairs are in each soldier's `sets.ron`).

1. A throw action started in the update the in-hand thrown weapon's cycle enters `Firing` (the press), not at the
   release: BF1 action = the PS2's fire state (3 for every fielded grenade: `MaxPressedTime` 0, no variant byte) on
   the grenade's set; BF2 pair `shoot_secondary2` (variant 1; `shoot_secondary` for variant 0) on CHANNEL 0's weapon's
   set for an offhand weapon. No alert from a BF1 throw (action 3).
2. Layers as for a shot (already built): BF1's legs join while he stands (movement id <= 2); BF2's only if he stood
   still at the start. The action ends with its clip; BF1 also ends it when the set changes back (channel 0).
   AUDIT (§10.2): BF1's test runs every update (a crouched still soldier's legs play the stand clip too); BF2's runs in
   the action's first update only - movement id 0 there (a soldier crouched still for over 1 s has id 2 and keeps
   his kneel) - and the legs leave the throw for the walk / run clip once he moves and never rejoin.
3. BF2: draw channel 0's weapon in the hand during an offhand throw (the rifle stays in the right hand); BF1 draws
   none (the port's `HideOnFire` rule already does).
4. The BF1 reload blip (one update of the rifle's reload at clip time 0) must not become a 2.5 s reload clip: the
   port computes the active channel before the cycle, so check the update the `Firing` state ends. AUDIT (§10.1):
   the PS2 DOES play that reload clip while L1 is held past the fire state's end (or pressed again during the
   reload): channel 1 stays until the release + 1 update, at most the clip's 2.5 s. Port it, or name the deviation.
5. Timing on BF1 maps: released at BF1's 0.3 s since 2026-11-01, with no stretch on the clip (the stretch and BF2's
   0.51 s on BF1 maps are gone). AUDIT (§10.3): the player's phase at the release is one update later than these figures (BF1 0.300 s, BF2
   0.533 s); which phase the drawn pose uses is [H] - the difference is one frame.
6. First person: card FPV (no first-person view exists).
7. Census: the share of throws (`Event::Throw`) whose thrower's animator holds the throw clip at the release, per map
   and game (target 100 %), plus a picture strip per game beside `ps2/*_sheet.png`.

## 8. Checks run

- **fn** (`lead_tmp/gthrw/checks/fn_b1.txt`, `fn_b2.txt`): every cited line, e.g. B1:215656 in `FUN_0028eb30`,
  B1:113835 in `FUN_002013a0`, B1:142629 in `FUN_0022aa10`, B1:117110 in `FUN_00205cc8`, B1:49906 in `FUN_001a6880`,
  B1:48376 in `FUN_001a49f8`; B2:153702 / 153863 in `FUN_00239bb0`, B2:320348 in `FUN_00328fb0`, B2:319551 in
  `FUN_00327e98`, B2:68061 in `FUN_001bfcb0`, B2:68990 in `FUN_001c1260`.
- **absent**: `re_check.py absent b1 2ecd946f --elf` -> 0 in B1, 1 in B2 (`FUN_00328fb0`) [receipt:98e36e0c]; constscan
  b1 0x2ecd946f: no raw word, no lui of the high half, no immediate of the low half (b2: lui 0x329210 / ori 0x329214);
  propscan bf1 0 chunks, bf2 195. `FUN_002859f8` has no direct caller in B1 (its header lines only)
  [receipt:bc0aa893]; elfword: the word 0x2859f8 in 17 vtables, one of them the grenade's (0x53bf98).
- **twin**: BF1 `FUN_0028eb30` + `FUN_002859f8` / BF2 0x337678 (start flag / bit 33 at the press); drivers
  `FUN_002013a0` / `FUN_00239bb0`; action setters `FUN_0022a958` / `FUN_0027c738` + `FUN_00281630`; fire updates
  `FUN_0028ec38` / `FUN_003377d0`; first person `FUN_001a38e0` / `FUN_001bfcb0`, tables `FUN_001a6880` /
  `FUN_001c1260`. BF2's offhand path has no BF1 twin (no key, no bit test).
- **word**: 0x53bf98 = 0x2859f8, 0x53bfbc = 0x28eb30, 0x629040 = 0x337678 (+0x44 = 0x3377d0; AUDIT: the
  next word, 0x629044 = vtable slot +0xac, holds 0x3377d0; slot +0x44 holds 0x32b560); 0.7 / 0.4 immediates
  (`checks/word_b1.txt`); BF2 0.4 at 0x337700.
- **writers**: bit 55 of a BF2 weapon class: the setter (B2:320348), the parent copy `FUN_003283d8` (B2:319732), the
  base constructor's clear (B2:319551, the default 0); the grenade constructor writes none (disassembly). BF1
  weapon+0xc0: the constructor (0, B1:207927), `FUN_002859f8` (1), the driver and the throw (0, B1:215917).
- **Verifier**: workflow `gthrw-re-verify`, 7 agents at medium effort, claims and citations only
  (`lead_tmp/gthrw/verify_result.json`). 24 claims in three batches: **21 SUPPORTED, 3 PARTLY, 0 NOT SUPPORTED** - the
  PARTLY ones listed side conditions the claims left out (BF2's network forcing and the melee branch of the driver,
  the selected and energy gates of BF2's fire update, two flag bits `FUN_0027c738` also clears), folded in above. 8
  absence / twin claims, each to two separate verifiers: 5 SUPPORTED by both (BF1 has no OffhandWeapon key;
  `FUN_002859f8` has no direct caller; both twins; `FUN_0022a958` arms nothing for action 3); 2 PARTLY by both on
  wording (the base constructor is a third writer of bit 55; the Ewok weapons do author the variant byte), fixed; 1
  overturned by one of its two verifiers (no human prone throw clip "anywhere": the sabre combo set fills the pair) -
  narrowed to the gun and tool sets, the broad form [H]. The verifiers added: the throw's own clear of BF1's start
  flag, and the selected-weapon gates of both fire updates.

## 9. What stays [H] or unread

- Which weapon BF2's soldier render draws while an offhand weapon is active (the picture shows the rifle).
- A charged BF1 throw (`MaxPressedTime` > 0: only the CIS and Imperial mortar launchers, 0.75) and the charge clip:
  not followed here.
- The BF2 alert idle that follows the throw clip (the alert timer's arming by an offhand weapon) - not read here.
- First person in a live run: no first-person toggle on the default pad (pcsx2 README); the PS2 view record's flag
  (re-aim-assist 10.2) would have to be written.
- The Ewok spear (BF2, not offhand, no bank) - the set it gets.

## 10. AUDIT 2026-10-22 (loop lane 1): corrections, measured [V][M]

Re-derived before the session's report was read. Live rows: the auditor's own sampler
`engine/artifacts/lead_tmp/audit-gthrw/tools/tsamp.py` (padscript's session and PINE, one raw row per game update of the
soldier block, the animator the soldier POINTS AT - BF1 soldier+0xbf0, BF2 E+0x7f0 - and every weapon slot) and decoder
`tools/dec.py` (clip names by a CRC-32 checked against the game's own `DAT_0057f638` table in a stored state); BF1 Naboo
Theed units1 (Rebel and Imperial), BF2 Coruscant ai0; SW renderer (the RTX was lost). 20 trials, 22 throws:
`ps2/all/*.pkl`, `ps2/out_b2hold/`, `ps2/out_b2crq/`, decoded `ps2/decoded.txt`. A second agent decoded the same rows
with its own parser and confirmed every reading below (`wf/results.json` N8).

### 10.1 BF1: the active channel, the fire byte, and the reload clip a held L1 shows
- `FUN_00205cc8` (B1:117106-117161) picks the channel each update: channel 1 when its weapon's state is 1-4; else the
  first channel (1, then 0) whose fire byte (soldier+0x2e8 + 2 x channel) is >= 1 or == -1, while the no-fire timer
  argument 2[0x303] <= 0 and that channel's weapon is not in state 8; else channel 0 [V, two readers SUPPORTED].
- The fire byte: >= 1 = held (the count of updates held; 1 = down this update after a release), -1 = released this
  update, <= -2 = updates since the release. Writers: the human's pad mapper `FUN_0025e098` (B1:179743-179849) and the
  AI's `FUN_0023cd40` (B1:155205-155290), same encoding [V, two readers].
- The grenade fires only on byte == 1: its vtable +0xa8 (0x28ee20, not in the export) returns `byte == 1` when
  `TriggerSingle` (class+0xcc, default 1, authored by no BF1 grenade) is set and `MaxPressedTime` is 0; case 5 of the
  weapon update `FUN_00284318` (B1:208555-208570) reads no trigger. So a press during the reload is lost, and holding L1
  when the state returns to 0 does not throw [V, two readers].
- `ReloadTime` is clamped to >= 0.01 s (`FUN_002864f0`, B1:~210283), so every throw with a grenade left passes state 5:
  1.0 s for the ALL thermal, one update for the IMP / REP / CIS / GAR thermals, crack and haywire (ReloadTime 0.0)
  [V; disc census N1].
- Hence: while L1 is held after the fire state ends, channel 1 stays, the driver passes 5 (`FUN_002013a0`), and the
  animator plays set 3's reload slot - `human_rifle_<stance>_reload`, 75 frames - on both players until L1's release
  + 1 update, at most the clip's 2.5 s (the clip-end clear `FUN_0022c7f0` B1:143647 / 0x22b6d4) [V, D].
- Live [M]: 8 of 10 BF1 trials show the doc's one-update blip only. A second L1 press 0.87 s into the Rebel thermal's
  1.0 s reload: `rifle_stand_reload` on both players for 6 updates (5 held + 1), no second throw. The Imperial thermal
  with L1 held 2.4 s: 21 more updates of `rifle_stand_reload` (20 held + 1) after the one-update blip. A replay of §1's
  rule over the 922 scored rows (agent M1) matches 867 - every miss is in these two trials.
- BF2 does not do this [M]: L1 held 2.6 s, the channel returns to 0 at state 4 and no reload clip plays (an offhand
  weapon's actions 3 and 4 map to no clip, B2 `FUN_00281630` B2:203810-203837).

### 10.2 BF2: when the legs take the throw
- `FUN_0027caf0` sends mode +0x20b8 == 0x27 (standing and crouched on foot) to `FUN_0027dfe0` (B2:201359-201360).
  There the lower player takes the action's lower slot (`FUN_00281558`, bank / stance / set / upper state) only when an
  upper action clip exists, bit 63 of +0x20e0 is up (set by `FUN_0027c738` B2:200754-200756 when the action changed;
  `FUN_00239bb0` calls it before the pose driver each update, so it is up for the action's first update only), the
  upper state is not 6 / 7, and the movement chooser `FUN_0027ed18` reads still with NOT (stance +0x20b4 == 1 and
  movement id +0x20c0 == 2) (B2:201825-201880); otherwise bit 63 is cleared and the lower takes a movement clip.
  +0x20c0 becomes 2 only after 1 s of crouched stillness (B2:~202081-202088) [V, two readers SUPPORTED the gate, PARTLY
  on its details: a turn-in-place id 3 / 4 also passes, and the `FUN_0027c978` path's bit 62].
- Live [M]: standing -> the legs on `rifle_stand_shoot_secondary2_full` for all 41 updates; crouched 0.6 s before the
  press (id 0) -> the legs on the STAND throw clip, 41 updates; crouched 1.7 s and 1.8 s before (id 2) -> the legs keep
  `rifle_crouch_idle_takeknee_lower` (upper: the stand throw clip); started still, then walked at +0.43 s -> the legs to
  `rifle_standalert_walkforward` / `runforward` in that update; started running, stopped at +0.50 s -> the legs to
  `rifle_standalert_idle_emote_full`, never the throw; crouch-walking during the throw -> `rifle_crouchalert_walkforward`.
- How the legs KEEP the throw over the later updates while he stays still is not read: both readers read the decomp as
  one update only, the rows show the whole clip [H].
- BF1 is different and per update [M]: the legs play `grenade_stand_shoot3` whenever the movement id is <= 2 - they
  leave it when he moves, rejoin when he stops (runstop: 32 updates on both), and a crouched BF1 soldier (id 0) plays
  the stand clip on his legs too.

### 10.3 The clip phase at the release
- BF1 `FUN_0022d318` starts a new clip at phase 0 (+0x9a4 = 0) and advances it in the same update (`FUN_0043e158`:
  +0x9a8 = the old phase, +0x9a4 += dt x 30 / frames); the animator's own counter +0x2084 starts at 0 [V]. At the
  release update +0x2084 reads 0.267 while the upper player's phase is 9/51 = 0.300 s - 11 of 11 BF1 throws [M]. BF2:
  the upper player's phase (+0xd10 + 0x9a4) is 16/40 = 0.533 s at the release - 11 of 11 BF2 throws [M].
- Which of the old / new phase the drawn pose samples was not read [H]: the doc's 0.267 / 0.50 and these 0.300 / 0.533
  differ by one frame (33 ms).

### 10.4 Held, re-measured
- Clips, sets and players: BF1 `grenade_stand_shoot3` (51) on set 3, both players while id <= 2, crouched too; after R2
  (pistol in hand) still the grenade set; Rebel fire state 48 updates, Imperial 51 (the clip's whole length); release
  in update 8 at weapon timer 0.300; no alert armed (+0x70 keeps falling). BF2 `rifle_stand_shoot_secondary2_full` (41)
  on channel 0's set; crouched the stand clip on the upper; the pistol primary plays `human_tool_stand_shoot_secondary2`
  over `rifle_stand_shoot_secondary2_full` on the legs; release in update 15 at 0.533; a second throw 2.17 s after the
  first plays from 0; state 4 one update at +1.835.
- The original session's rows, decoded by an agent's own parser (N7): every number of §5's table.
- Disc: BF1 16 grenade-base weapons, the 11 thermal / crack / haywire with bank grenade and no variant byte, the Ewok
  pair `ForceFireAnimation` 2, `InitialSalvoDelay` 0.3 on all 13 grenade-bank weapons, fire states 1.6 s (ALL) / 1.7 s
  (N1); BF2 offhand PROPs 195 / 181 (N2, rerun by the auditor); every clip frame count of §1, §2 and §4 from the disc's
  banks (N3; BF1's soldier banks are in COMMON/ENG/COMMON.LVL, BF2's `humanfp` in INGAME.LVL); 35 of 38
  `human_animation_fp` clips resolve to `_1st_` names, the other 3 to none (a floor, N3).
- Code: BF2's fire entry 0x337678 instruction by instruction (strict 0.4 < strength; bit 33 at 0x337754-0x337788; the
  delay index 2 = `InitialSalvoDelayProne`) (N6); both first-person paths (N4, N5; the column can still be overridden
  to 9 later in the same update when moving fast); the absence and twin claims of §8 (two readers each: all held; the
  driver twins gate states 1-4 in BF1 but only 1-2 in BF2, and BF2's needs vtable+0x5c == 0).
- The port (§7 "today"): every statement holds on main 2a94ed5, and every fielded throwing class has its clip baked
  (N9, rerun by the auditor: BF1 37 class / weapon rows; BF2 45 of 65 throwing classes baked, all with the pair; the
  other 20 - two heroes and 18 `_default_` bases - have no sets and are fielded on no mission).

## 11. Pack dispenser correction, 2026-10-28 [O] UNAUDITED

The maintainer reported that dropping a health/ammo pack plays the grenade throw. Snapshot read first:
`dist/openbf_data/playtest/last.json`, schema 8, build and checkout `2ddaaba2e0eb`, BF1 GEO1, seed 104,
human #0; weapon table entry 13 is `rep_weap_inf_powerup_dispenser`. The snapshot contains no animation poses,
so the clip mismatch is established by the renderer and the executable references, rather than measured from it.

BF1 dispenser fire entry `FUN_0028d5d8` computes launch strength but does not replace the normal `FireAnim + 1`
action with a strength variant. This differs from grenade entry `FUN_0028eb30`, which chooses actions 2/3.
BF2 dispenser instance vtable word `0x628cd8` points to fire entry `0x335478`: the instructions through
`0x33554c` set the hidden/start bits and calculate strength, without the grenade entry `0x337678`'s
strength-to-action selection. Both dispensers therefore retain their default fire action 1. See this note
sections 1/2 and `re-soldier-anim-layers.md` sections 4/7 for the animator mapping: BF1 uses the grenade bank's
plain `grenade_stand_shoot`; BF2's offhand flag keeps the primary set and uses `shoot_secondary`, not
`shoot_secondary2`. BF1 action 1 also arms the normal shot alert; the overhand action 3 does not.

The port incorrectly passed the dispenser's full launch strength to the grenade variant selector, selecting
BF1 `shoot3` or BF2 `shoot_secondary2`. It also missed the action altogether when BF1's zero-delay dispenser
finished firing in the same tick. The correction selects the plain action for pickup dispensers and observes
their successful release event for that instant-release case. Grenades keep their strength-based selection.
The regression loads all eight faction dispenser definitions and their soldier clips, standing and moving,
and checks the exact drop clip, first-frame action, one release and return to the primary weapon. The existing
real-grenade regression remains in place. No fresh live PCSX2 measurement was performed.

### 11.1 BF2 pack release point, 2026-10-28 [O] UNAUDITED

After the clip correction, the maintainer reported an early, head-height release on BF2. BF1 releases instantly
with its different animation. The latest snapshot is still the BF1 GEO1 build `2ddaaba2e0eb` above, predating
the clip correction `75be240`; it cannot establish the timing of the reported BF2 throw.

[V] BF2 class finalizer `FUN_00336e78` (B2:316866-316970) samples the release animation at class+0x24c
(`InitialSalvoDelay`, 0.5 s for these packs). Its offhand branch calls `FUN_00281858` with bone hash
`0xb7ec1d31` (`bone_l_hand`) and stores the sampled translation at class+0x490. The other branch samples
`0x2b960099` (`hp_weapons`) and applies the class+0x30 offset. The bone names reproduce with the animation
CRC in `mungefs/src/anim.rs::crc32_lower`. A second sampled offset at +0x4a0 uses class+0x250 and a different
action; its selection flags have not been fully resolved here.

[V] Dispense `FUN_00335920` (B2:316104 onward) selects that cached offset, applies the owner's transform,
and adds the class+0x270 offset. The soldier-class scale at +0x1220 also scales the sampled offset when it
differs from 1. This is distinct from always spawning at the aimer's fire point. BF1 keeps the fire-point
path described in `re-grenades.md` section 5.1.

[D] Sampling the baked BF2 engineer's `human_rifle_stand_shoot_secondary_full` at frame 15 (0.5 s) gives
left-hand translation (-0.031, 1.148, 0.721) m in model coordinates (+Z forward). The port previously spawned
at feet + 1.425 m, with no forward offset: about 0.72 m behind this hand, and with the pack's upper edge near
the head. `Dispenser::BF2_HAND` now supplies height 1.148 m and forward distance 0.721 m through the existing
`Thrown.hand` representation. Like the grenade offset, it is centered laterally; the sampled 3 cm lateral
component and per-soldier scale are not represented. Grenades and packs share `Unit::launch_origin`.

[D] The regression steps the actual animator and dispenser for all four BF2 factions, standing and moving.
At the release event the posed left hand is within 8 cm of the spawned pack's mesh bounds in every case.
The event remains within one simulation tick of the authored 0.5 s delay. BF1's zero delay and fire-point
origin are unchanged; a core regression also checks release origins under three soldier headings and a
nonzero aim pitch. Logs: `engine/artifacts/lead_tmp/pack-release-pose.log` and `pack-release-alignment.log`.
No fresh live PCSX2 measurement was performed; the maintainer's playtest of this uncommitted build is pending.
