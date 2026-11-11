# RE: PS2 pause-menu cheat codes (BF2 SLUS-212.40) and the BF1 verdict

Tags: **[V]** read directly from decomp / disassembly / raw data, **[D]** derived
from several [V] facts, **[H]** hypothesis.

Sources: `analysis/SWBFRNT2.ELF-decomp.c` (BF2), `analysis/SWBFRONT.ELF-decomp.c`
(BF1), raw ELFs under `extracted/` (capstone MIPS64-LE for the 6 handlers Ghidra
did not define), `extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/SNW.LVL` (weapon PROPs),
Lua string tables of `extracted/*/lua/common/ifs_pausemenu.luac`.
VA -> file offset for both ELFs: `off = va - 0x160000 + 0x1000` (single LOAD).

## TL;DR

The PS2 BF2 cheats are **Morse code typed on the D-pad while paused** [V]:
Up = dot, Down = dash, Left = end of letter, Right = end of word / submit.
The word is FNV-1a-hashed (lowercase) and looked up in a 7-entry table. The
whole system is engine code (`FUN_0038c1d8`), not Lua — the pause-menu scripts
know nothing about it. BF1 has **no** such system (engine has neither the
matcher nor the tables); its `invincible`/`unlimitedammo` are dev-only
(command line + hidden fake console).

## 1. Summary table (BF2)

| Word | Morse (D-pad) | Handler | Global toggled (init) | Effect | Net-game gate | Persists |
|---|---|---|---|---|---|---|
| `sos` | `... --- ...` = U U U **L** D D D **L** U U U **L** **R** | `FUN_0038c4e8` | `DAT_00600b1c` (0) | Invincibility for entities driven by a human controller | handler + readers refuse in net game | process lifetime |
| `ammo` | `.- -- -- ---` = U D **L** D D **L** D D **L** D D D **L** **R** | `FUN_0038c528` | `DAT_00600b1d` (0) | Infinite ammo for weapons fired by a human controller | handler + readers refuse in net game | process lifetime |
| `hud` | `.... ..- -..` = U U U U **L** U U D **L** D U U **L** **R** | `FUN_0038c5c0` | `DAT_005f8fcc` (1) | HUD element show/hide on unpause (HUD stays hidden) | none | process lifetime |
| `slow` | `... .-.. --- .--` = U U U **L** U D U U **L** D D D **L** U D D **L** **R** | `FUN_0038c5d8` | `DAT_0060b680` (1.0 <-> 0.3) | 3D-sound pitch multiplier -> "slow-motion sound effects" | none | process lifetime |
| `80s` | `---.. ----- ...` = D D D U U **L** D D D D D **L** U U U **L** **R** | `FUN_0038c568` | `DAT_005ff0fa` (0) | Soldiers always drawn with their low-LOD model ("low-res soldiers") | handler + reader refuse in net game | process lifetime |
| `at` | `.- -` = U D **L** D **L** **R** | `FUN_0038c5a8` | `DAT_005ff0fb` (0) | Melee hit spawns a random `AddHitEffect` instead of `HitEffect` (retail data: Wampa paw hits show `com_sfx_bif/zap/blammo/pow`) | reader refuses in net game | process lifetime |
| `alias` | `.- .-.. .. .- ...` = U D **L** U D U U **L** U U **L** U D **L** U U U **L** **R** | `FUN_0038c4c0` | `DAT_0060c2dc` (1) via `FUN_00589900/FUN_005898f0` | Toggles the GS full-screen post-filter pass (PS2 anti-alias/deflicker) built in `FUN_00586578` — display only | none | process lifetime |

U/D/L/R = D-pad Up/Down/Left/Right, single presses. Every handler is a toggle
(XOR 1 / float swap); nothing "sets". Word hashes (table at `0x5ff220`,
`{fn, hash}` x 7): `alias 7d268157`, `sos cb36196c`, `ammo 5b9b0daf`,
`80s fe9ed9d2`, `at 57251588`, `hud e28d61c6`, `slow 6d9542b8` [V] (dumped raw,
words recovered by hashing a 530k-candidate dictionary; each is the unique hit).

Confirmation feedback: on any recognised word the engine plays the UI sound
property `shell_menu_accept` (hash `0x6a942216`, resolved by `FUN_0043d230`,
played by `FUN_0043ceb0`) [V]. No VO, no on-screen text. The sound plays even if
the handler's own net-game check then refuses the toggle [V].

"Persists" = the flags are plain globals with **no other writers** in the
decomp (only the handlers; `alias` also from a config property `0x8958bcda` in
`FUN_003bcd50` and cleared by `FUN_00428618`/`FUN_003c9258`); no block memset
touches their neighbourhoods, nothing serialises them to the profile [D]. They
survive mission restart, level change and return-to-shell until power-off.

## 2. Input matcher — `FUN_0038c1d8` (0x38c1d8..0x38c4bc) [V]

### Where it runs
`FUN_0038bb98` is the per-frame `Update(dt, self)` of a pause controller
(`PTR_DAT_005ff0f0 + vp*0x64`, count `DAT_00600b30`). When the controller's
paused flag (`+0x18`) is set and its viewport (`+0x1c`) is >= 0, it fetches that
viewport's raw pad object `FUN_00431918(0x719c00, vp)` = `0x719c00 + 0x18 +
vp*0x28c` and calls the matcher on it. (Ghidra dropped the second argument in the
call; `a1` still holds the viewport index [D].) The "Start toggles pause" logic in
the same function uses the *mapped* function struct (`FUN_00431968`), not the
raw pad. The demo/attract flag `DAT_00600b15` only blocks the Start toggle, not
the matcher.

Consequence: only the pad of the player **who paused** (`DAT_005ff0f4`) is
listened to, only while paused. The matcher runs even though the Lua pause
menu also reacts to D-pad Up/Down (`ifs_pausemenu` has `Input_GeneralUp/Down`
handlers that move the highlighted button, and *no* `Input_GeneralLeft/Right`
handlers) [V string table]. Typing a code therefore wiggles the menu cursor —
harmless, and authentic.

### Raw pad object layout (per viewport, 0x28c bytes) [V]
Filled by `FUN_00435050`: `prev[20] = cur[20]` (memcpy 0x50), `cur` cleared,
then for i in 0..15: `if (buttons & mask[i]) cur[i] = 1.0f`; `cur[16..19]` =
analog sticks. `cur[i]` at `+0x0c + 4i`, `prev[i]` at `+0x5c + 4i`.
`buttons` (`+0xbc`) = `~((rdata[2] << 8) | rdata[3])` of the libpad packet
(`FUN_0059e980`, built in `FUN_0059e3b0`), i.e. the standard SCE active-high
word. Mask table `0x600e00` gives the index order:

| idx | mask | button | idx | mask | button |
|---|---|---|---|---|---|
| 0 | 0800 | START | 8 | 0080 | SQUARE |
| 1 | 0100 | SELECT | 9 | 0020 | CIRCLE |
| 2 | 8000 | **LEFT** | 10 | 0010 | TRIANGLE |
| 3 | 2000 | **RIGHT** | 11 | 0040 | CROSS |
| 4 | 1000 | **UP** | 12 | 0004 | L1 |
| 5 | 4000 | **DOWN** | 13 | 0008 | R1 |
| 6 | 0200 | L3 | 14 | 0001 | L2 |
| 7 | 0400 | R3 | 15 | 0002 | R2 |

(Index 0 = START is also what `FUN_00431a38` polls for "press Start to
activate controller", which cross-checks the table.)

### Algorithm (paraphrased)
```
pressed(i) := cur[i] >= 0.125 && prev[i] < 0.125        // rising edge only
if      pressed(UP)    : morse[mlen++] = '.'
else if pressed(DOWN)  : morse[mlen++] = '-'
else if pressed(LEFT)  : morse[mlen] = 0; mlen = 0;      // FUN_0038c1c0
                         for e in letters[36]: if strncmp(e.code, morse, 6)==0
                             { word[wlen++] = e.letter; break }
else if pressed(RIGHT) : word[wlen] = 0; wlen = 0;
                         h = fnv1a_lower(word)            // FUN_005393c0
                         for e in cheats[7]: if e.hash == h && e.fn
                             { play "shell_menu_accept"; e.fn(); break }
                         morse[mlen] = 0; mlen = 0
wlen &= 0x1f;  mlen %= 6;                                // wrap, no error
```
State: `morse` = `DAT_006a4930` (8 B), `mlen` = `DAT_005ff258`; `word` =
`DAT_006a4938` (32 B), `wlen` = `DAT_005ff259`; both lengths start at 0 [V].

Properties that matter for a port:
* **One symbol per frame, priority Up > Down > Left > Right.** Simultaneous
  presses lose the lower-priority one.
* **No timeout** and **no reset on pause/unpause/level change** — the buffers
  are static and only the matcher touches them [D]. A half-typed word survives
  resuming the game; the next pause continues it. The only way to flush is to
  press Right (submit) — an unrecognised word just resets silently.
* **Unrecognised Morse letter** (no table match) is dropped; the letter buffer
  resets anyway. Mistyped letters therefore corrupt the current word until the
  next Right.
* Letter buffer wraps at 6 symbols (`% 6`), word at 32 chars (`& 0x1f`), so a
  7th symbol without Left overwrites position 0 (garbage, never matches).
* Letter table `0x5ff100`: 36 x `{u8 letter, pad, char* code}` = a..z, 0..9
  with standard ITU Morse (`a=.-`, `b=-...`, ..., `0=-----`, ..., `9=----.`) [V].
* Hash = FNV-1a-32 over bytes with `c | 0x20`, offset `0x811c9dc5`, prime
  `0x01000193` (`FUN_00539308`) — same as the project-wide name hash.
* The matcher **toggles**; entering a code twice restores the previous state.
  Sound plays on every recognised submission.

## 3. Engine effects (BF2)

### Common gate: "network game"
`cVar = DAT_0060054f ? DAT_00600549 : DAT_00600548; allowed = (cVar == 0)`.
`DAT_00600549` is set by `ScriptCB_SetAmHost` -> `FUN_00400030` (host) /
`FUN_00400058` (client); `FUN_003d5110` latches it into `DAT_00600548` when the
lockstep session starts; `FUN_00426c58` clears both on teardown [V]. So the gate
is "a network (journaled/lockstep) session is pending or running" [D]
(a5-tickrate.md calls the same pair record/replay flags; same mechanism). For
openbf single-player this gate is always open.

### `sos` — invincibility (`DAT_00600b1c`)
Readers [V]:
* `FUN_001dfb38` (damage application; "TakeDamage" of the health component
  at entity+0x170) — after the net-replay branch and the `DAT_005fcbd8`
  (debug no-damage global, init 0, no writers) early-out:
  ```
  if (DAT_00600b1c && !netgame)
      for each controller c in list(entity + 0x240)     // FUN_002690c8
          if (c->playerIndex /*+0xc0*/ >= 0) return;      // no damage at all
  ```
  Runs *before* damage scaling, shields, spawn invincibility
  (`DAT_005fcbdc`, default 2 s), etc.
* `FUN_001df640` (instant kill / set health 0, `argument 3 != 0` = "respect
  invincibility") — identical loop, `return` skips the kill. Callers seen with
  `argument 3 = 1`: `FUN_001eddb8` (soldier crushed/pushed >5 units into
  geometry [H]) and `FUN_001f4d30` [H unknown kill type].

Who is affected: **any entity whose controller list (`entity+0x240`, intrusive
list, item at node+0xc, `+0xc0` = human player index or -1 for AI) contains a
human controller** — both split-screen players qualify, AI never [D]. Vehicles:
a vehicle with a human occupant should have that controller in its list [H,
occupant attachment not traced], which would make occupied vehicles immune as
well. Fall damage / death regions: only those that route through the two
functions above with the invincibility flag; kill paths that call
`FUN_001df640` with `argument 3 = 0` (if any) bypass it [H].

### `ammo` — infinite ammo (`DAT_00600b1d`)
Readers [V]: base weapon virtuals `FUN_00327658` (start firing a round) and
`FUN_003274f8` (end/cancel). Ammo state at weapon+0x90: `+0xc` = pool (float,
`FLT_MAX` = unlimited in ODF), `+0x10` = current round charge (0..1).
```
if (weapon->controller/*+0x6c*/->playerIndex/*+0xc0*/ >= 0 && DAT_00600b1d && !netgame)
    FUN_003039c8(ammo)   // charge = 1.0, pool untouched     (start)
    FUN_003039c0(ammo)   // charge = 0,   nothing refunded    (end)
else
    FUN_00303958(ammo)   // move one round from pool to charge
    FUN_00303928(ammo)   // refund unused charge
```
Coverage: the base virtuals sit in 15 / 19 weapon vtables and the subclass
overrides (`FUN_00331c98`, `FUN_00331da0`, `FUN_00335778`, `FUN_003379a8`)
call them [V] -> every weapon class, including vehicle/turret weapons fired by
a human, and only human-controlled weapons [D]. Fire rate is unchanged (the
per-round charge still cycles). Clip/reload handling was not traced [H]: what
is frozen is the pool at `+0xc`.

### `hud` — HUD visibility (`DAT_005f8fcc`)
Only reader: `FUN_0038b590` (pause/unpause) — when the flag is set it calls the
ten HUD-group visibility functions (`FUN_00393900`, `FUN_00393028`,
`FUN_0038e9c0`, `FUN_00391200`, `FUN_0037e6d8`, `FUN_00389b48`, `FUN_0037df18`,
`FUN_003835f0`, `FUN_0037ed58`, `FUN_00390d20`) and five per-viewport elements
with `visible = !paused` [V]. Toggling the flag to 0 while paused means the
"show again on unpause" pass is skipped, so the HUD (hidden at pause time)
stays hidden; typing `hud` again and unpausing shows it [D]. Per pausing
viewport only.

### `slow` — slow-motion sound (`DAT_0060b680`)
Only reader: `FUN_00540f40`, the 3D-voice Doppler pitch. In the Doppler branch
it returns `pitch * DAT_0060b680`; the other branch returns 1.0 unscaled [V].
So every positional SFX plays at 0.3x pitch/speed; UI sounds and music are not
routed through it [D]. Game time is **not** slowed (`DAT_0060b680` is not a
clock scale) [V].

### `80s` — low-res soldiers (`DAT_005ff0fa`)
Only reader: `FUN_00239bb0` (soldier draw). Normally `argument 3 == 0` selects the
full model (`soldier[0x58]`) and `argument 3 != 0` the LOD model
(`soldier[0x29b]`); with the flag (and not in a net game) the LOD model is
forced [V]. Pure render.

### `at` — random alternate hit effects (`DAT_005ff0fb`)
Only reader: `FUN_00340c60`, the per-frame sweep/hit test of the melee-style
weapon class whose parser is `FUN_00342528` (8-slot already-hit list; class
label [H] `melee`). Parser: `HitEffect` (`0xc0929477`) -> slot 0, count=1;
`AddHitEffect` (`0x18bdf340`) appends up to 4 more (class `+0x24c + 4i`, count
`+0x260`, max 5) [V]. On a hit, if the flag is set, not net game and count > 1,
it picks a random slot 1..count-1 instead of slot 0 [V]. Retail data: only
`hot_weap_lightsaber_wampa` has `AddHitEffect` (`com_sfx_bif`, `com_sfx_zap`,
`com_sfx_blammo`, `com_sfx_pow`; its `HitEffect` is `hot_sfx_weap_pawsmash`;
`com_weap_inf_lightsaber` has `HitEffect = com_sfx_sabrehit` only) [V SNW.LVL].
So the cheat is the "comic-book Wampa" gag; everywhere else it is a no-op.

### `alias` — GS post-filter (`DAT_0060c2dc`)
`FUN_00587278` (frame end) calls `FUN_00586578` when the flag is set, which
emits GIF packets for a full-screen textured pass (PS2 anti-alias / deflicker
blend) [V]. No gameplay meaning; openbf can treat it as a no-op or map it to a
post-process toggle.

## 4. What is NOT there
* No unlock-all, hero, era, campaign, award or medal code. The table has
  exactly 7 entries and all 7 handlers were disassembled [V]. Unlockables go
  through `ScriptCB_UnlockUnlockable`/`ScriptCB_UnlockableState` (profile) and
  awards through the stats path; neither has a cheat hook.
* `extracted/bf2/lua/shell/ifs_freeform_cheat.luac` is a Galactic Conquest
  developer screen ("Cheat Menu": Next Turn, +100/+1000/+10000 credits,
  set/add/remove player & enemy planets/fleets) [V strings]; reachability at
  retail is an open question (no menu links it in the string tables checked).
* BF2 ships the `debugmenu` console group (`ScriptCB_GetConsoleCmds`
  enumerates it) but no `debugmenu.*` items exist in the retail ELF [V].
* No "cheat" strings anywhere in BF2 Lua or ELF — the words live only as
  hashes, the Morse table as `.-` strings.

## 5. BF1 (SLUS-208.98) verdict: **no pause-menu code system** [V]
1. The BF1 pause controller update `FUN_002d2d08` (structural twin of BF2
   `FUN_0038bb98`: Start-edge on `+0x70/+0xd8`, HUD group hides) has no matcher
   call and no equivalent function exists.
2. `SWBFRONT.ELF` contains no Morse letter table (`.----`, `---..`, `-.-.`
   etc. absent; only an unrelated `-----` and an ellipsis), no `{fn,hash}`
   cheat table, and no `shell_menu_accept` string.
3. What BF1 does have is developer-only:
   * command-line switches `/invincible` and `/unlimitedammo` parsed by
     `FUN_00170840` into `DAT_00519060` / `DAT_00519064` (with neighbours
     `DAT_00517cf4/8`, `DAT_00519058`);
   * console items `debugmenu.invincible` / `debugmenu.unlimitedammo`
     registered by `FUN_00341060` (bound to the same two bytes), listed and
     executed by `ScriptCB_GetConsoleCmds` (0x1837c0) / `ScriptCB_DoConsoleCmd`
     (0x1838a8) — i.e. the pause menu's "Fake Console" (`ifs_fakeconsole`)
     button, which the retail script hides because the engine defines
     `gFinalBuild = true` (`DAT_00515c3c = 1`, pushed in `FUN_0033e590`) [V/D].
   * The effects mirror BF2 exactly: `FUN_001bbc98` / `FUN_001bc190` skip
     damage when the victim's controller has `+0xe8 >= 0` (human) and
     `DAT_00519060`; `FUN_00285398` / `FUN_002854c0` skip ammo deduction when
     `DAT_00519064` and the owner controller is human [V].

## 6. Port guidance for openbf

### `ui::menu` — sequence recogniser (`engine/crates/ui/src/menu.rs`)
* Add a `MorseCode` recogniser owned by the pause menu, fed with the pausing
  player's D-pad **rising edges** each tick while paused: `Up -> '.'`,
  `Down -> '-'`, `Left -> commit letter`, `Right -> submit word`, evaluated in
  that priority with at most one event per tick.
* Buffers: letter buffer capacity 6 (index `% 6`), word buffer 32 (`& 0x1f`).
  Keep them in session state, **do not** clear on resume/level load (authentic
  behaviour); an unrecognised letter is dropped, an unrecognised word resets.
* Letter table: standard ITU Morse for `a-z0-9` (exact strings in section 2).
* Submit: lowercase the word and compare against `sos ammo hud slow 80s at
  alias` (or FNV-1a-lower hash against the 7 constants above). On a hit: play
  `shell_menu_accept`, then toggle. D-pad Up/Down may keep moving the cursor
  as on PS2; Left/Right must not be consumed by the menu rows.
* Existing menu rows can stay as a discoverability aid, but the toggles they
  drive should be the same session flags the recogniser toggles.

### `core::sim::Cheats` (`engine/crates/core/src/sim/state.rs` since SPL5, 2026-09-24; `sim.rs` before)
Mapping of the authentic set onto the registry:
| PS2 word | openbf field | notes |
|---|---|---|
| `sos` | `invincible` | gate in damage-apply **and** instant-kill for every entity with a human controller (both split-screen players; occupied vehicles [H]); before shields/spawn-invuln. AI unaffected. |
| `ammo` | `infinite_ammo` | skip pool deduction in weapon start/end-fire for human-controlled weapons of all classes incl. vehicle weapons; fire-rate cycle unchanged. |
| `hud` | (new, render) | hide HUD after resume; toggle. |
| `slow` | (new, audio) | 3D SFX pitch x0.3; not a time scale. |
| `80s` | (new, render) | force soldier LOD model. |
| `at` | (new, fx) | needs `AddHitEffect` support in the weapon class; only the Wampa uses it. |
| `alias` | no-op | PS2 GS post-filter. |
`infinite_stamina` (which since 2026-10-28 also keeps a jet class's fuel full; the separate jet-fuel cheat was removed), `all_awards`, `damage_award` have **no PS2
counterpart** (BF2 or BF1) — keep them as openbf extensions and label them so.
`all_awards` (menu label "Award weapons", #142) gives the player every BF2 medal bit but the damage award's (medal 8, its own row `damage_award`, "Damage award (x1.5)") on either game's maps, so
the class's award weapons replace the ones they stand for through BF2's switch filter (re-grenades.md §2.4; a BF1 class
carries BF2's awards, award-weapons.md §3); with `damage_award` on, the damage award's x1.5 applies (award-weapons.md §1); bots and the space
modes keep their kits and numbers. The other medal effects, and earning medals, wait for #130.
Persistence: flags live for the process, never saved to the profile, not reset
by mission restart or return-to-shell; in a network session `sos`/`ammo`/`80s`
must refuse to toggle and `at` must not apply.

## Port (openbf, card CHEAT2)

Typed while paused on the pausing player's D-pad (keyboard: the arrow keys),
exactly as §2: U = Up (dot), D = Down (dash), L = Left (end of letter),
R = Right (submit). A recognised word plays `shell_menu_accept` and toggles.
The pause menu's rows drive the same flags.

| Word | Cheat | Morse | D-pad sequence | Origin |
|---|---|---|---|---|
| `sos` | invincibility | `... --- ...` | U U U L D D D L U U U L R | PS2 |
| `ammo` | infinite ammo: the reserve never goes down; the clip empties and reloads for free | `.- -- -- ---` | U D L D D L D D L D D D L R | PS2 |
| `clip` | no reloading: the clip stays full, shots draw from the reserve, then the clip, to 0 | `-.-. .-.. .. .--.` | D U D U L U D U U L U U L U D D U L R | `[ours]` |
| `run` | infinite stamina | `.-. ..- -.` | U D U L U U D L D U L R | `[ours]` |
| `all` | unlock all classes (every class costs 0 points) | `.- .-.. .-..` | U D L U D U U L U D U U L R | `[ours]` |
| `cheat` | all five above on (or all off, if all five are on) | `-.-. .... . .- -` | D U D U L U U U U L U L U D L D L R | `[ours]` |

`ammo` + `clip` together: nothing is drawn and nothing reloads (fire forever).
They apply to player units only, on foot and in vehicle seats. `hud`, `slow`,
`80s`, `at`, `alias` keep their §6 behaviour.

`[ours]` lifetime (a named deviation from the PS2's power-off): the flags stay on
across missions, playlist maps and restarts, and all go off (with `hud` and
`slow`) when the player returns to the main menu or quits; never saved to the
profile. The Morse buffers themselves are never reset, as on the PS2. While
paused, D-pad Left/Right never reach the menu rows (outside the Respawn popup),
so typing a word cannot step a slider; Up/Down still move the cursor.

## 7. Open questions
* Are there `FUN_001df640` callers with `argument 3 = 0` (kills that ignore
  invincibility, e.g. death regions / scripted `KillObject`)? Untraced.
* Vehicle occupancy: confirm that a human occupant's controller is linked into
  the vehicle entity's `+0x240` list (drives "invincible while in vehicle").
* Clip/reload interaction with the frozen ammo pool.
* Name of config property `0x8958bcda` (sets the `alias` flag) and whether the
  GC `ifs_freeform_cheat` screen is reachable at retail.
* `at` is the unique dictionary hit for `0x57251588`; a longer unlisted word
  colliding is possible but improbable (~1e-4).
