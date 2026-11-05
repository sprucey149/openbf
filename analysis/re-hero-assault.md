# RE: Hero Assault on Mos Eisley (`tat2g_eli`, issue #106)

Read from the retail PS2 BF2 (SLUS-212.40) disc scripts (`tat2g_eli.luac`, `objectivetdm.luac`, Lua 5.0
bytecode listed by hand), the ELF (`extracted/bf2/SWBFRNT2.ELF`) and its listing
(`analysis/SWBFRNT2.ELF-decomp.c`). Listing SHA-256:
`f16195106e9058fc7e6b8ad9187b18832a18b1dd776223c88e2c837422b8d5cf`.
All line anchors below refer to that corpus. Tags: [V] read, [D] derived, [H] hypothesis.
Nothing here was compared against the running original game.

## 1. The rules

- [V] Score: `objectivetdm.luac`, `OnCharacterDeath(player, killer)`. No killer or the same team returns; otherwise the
  killer's team gains one point. When the limit is at or below the team's points the objective completes for that team.
- [V] Limit: a celebrity deathmatch reads `ScriptCB_GetAssaultScoreLimit`, `FUN_0019b868` (line 43005), which reads `DAT_005fa898`.
  Its ELF word is 180, and the session's `iASSScoreLimit` row (range 50..1000) writes it. The port's constant is
  `ASSAULT_SCORE_LIMIT`; the shell passes `session.ass_score`.
- [V] `multiplayerScoreLimit: 100` is set in `tat2g_eli.luac`, and nothing else reads it: no other script and no string
  in the ELF. The issue's "default 100" is this unread field, so the port runs 180.
- [V] No time limit: `GetGameTimeLimit` returns 0 in this objective.
- [V] The bot count comes from `ScriptCB_SetNumBots(ScriptCB_GetASSNumBots())`, the port's `session.ass_numbots`.
- [V] Reinforcements: `SetReinforcementCount` (`FUN_0018c3e8`, lines 32436–32439) turns a negative count into 0x7fffffff. The
  generator's existing `NO_TICKET_LIMIT` writes 999999.
- [V] Bleed and domination belong to `ObjectiveConquest`, which this script does not create.

## 2. No hero rules at all

- [V] The drain in `FUN_0034cea8` (line 343962) is gated by `FUN_0034da30` (lines 344432–344435), which reads hero bit 33 of
  class+0x330.
- [V] Only `FUN_0036e770` (line 367470) sets that bit, reached through `FUN_0036e800` from `SetHeroClass` (0x18cb18, line
  32701). `tat2g_eli` never calls `SetHeroClass`, and `EnableSPHeroRules` is a stub (line 32848).
- [D] So the mode has no hero manager: no unlock, no drain and no kill-heal. The port leaves `sim.heroes` as `None`.
- [V] The death clip is the exception. `FUN_002379f0` (lines 151821–151830) takes the hero death state 16 when hero bit 33 is
  set or when class+0x778 bit 4 is set, and that bit is `FleeLikeAHero` (`FUN_00252bb0` setter at lines 171707–171711, key 0x268dac2d). This is
  the only read of that bit. `com_jedi_default` and `com_hero_default` author it, so all 17 classes here kneel with
  `stand_deadhero` even with bit 33 clear. Grievous's clip table has no `stand_death_*` rows at all.
- [V] The script also turns hero music and hero VO off (`ScriptCB_EnableHeroMusic(0)`, `ScriptCB_EnableHeroVO(0)`).

## 3. Which class a bot takes

`FUN_0036f198` (lines 367838–367976), called from `FUN_0034ffe0` (line 346293), as also read in `re-ai-class-selection.md`:

- [V] It counts the class assigned to each member of the team. The reserve holds while a human is on the team or the
  game clock is under 60 s.
- [V] It shuffles the classes with the game LCG. The first pass takes a class under its minimum, the second a class
  under its maximum; while the reserve holds, a class one below its maximum is refused in both. Otherwise it returns
  -1 and the member waits.
- [V] A member keeps its old class counted until `FUN_0034f1f8` (lines 345606–345608) clears it on a -1, so the port
  tries with its own class counted and then without.
- [D] With the script's min 1, max 2, the human's team fields at most 9 hero classes. The bot team fields 8 for the
  first minute, then up to 12.

Measured in the port (seed 42, 60 s): 9 and 8 distinct classes alive at once, points [15, 8]. Run to its end, the round
closes at 180 about 10 minutes in, with team 2 ahead. Those are port runs, not measurements of the original.

## 4. World

- [V] The `tat2_tdm` layer holds six `com_bldg_controlzone_ctf` posts: `eli_cp1`, `eli_cp7`, `eli_cp2` (team 1) and
  `eli_cp8`, `eli_cp6`, `eli_cp3` (team 2).
- [V] None names a capture region, and `eli_cp8` names no control region either. The layout normally skips such an
  instance, so the mode keeps it (`Manifest::spawn_only_posts`). Conquest is unaffected, because `yavin1_commondesign`
  holds regionless posts the port does not field today.
- [V] A null capture region means nobody counts (`FUN_00359b78`, lines 352957–352961; `re-command-walker-post.md` section 3), so these posts
  cannot be taken. The port stops counting occupants for the whole mode.
- [V] The VO and music lines give team 1 the Alliance's and team 2 the Empire's, hence `hero` as `all` and `villain` as
  `imp` in the generated mission.

## 5. Not measured

- PROVISIONAL, trace: the PS2 Deathmatch AI goal. Bots use the existing `retarget_hunt` path, and the layer's base nav
  graph was only seen working in headless runs.
- PROVISIONAL, replace: the DLC heroes' min 1, max 2 in `heroes.rs` `assault_dlc_class` has no disc evidence. The DLC
  path did not run here, because `side_bf2/dlc_heroes.ron` is not installed.
- trace: the Coruscant mod-tools `cor1g_eli` text is a lead, separate from a retail authored mission. It is absent
  from the inspected retail-generated PS2 and PC mission sets. Coruscant's actual Xbox DLC mission, layer, launch
  and full round remain unverified and open under #106; this Mos Eisley slice does not close that issue.
- measure: the original's round length and score pace against the port's.

## 6. Launch boundaries

The port requires an authored non-Conquest layer and a matching mission before starting Hero Assault. Missing class
or combo inputs report a load error when a team has no usable heroes. A partially available team keeps its usable
authored heroes and reports missing classes; it never substitutes Conquest infantry. The ordinary Conquest fallback
is unchanged. Implicit and explicit mission discovery use the same authored team-seat selection. These are port input
validation rules, not measurements of retail recovery from missing baked data.

The timed runner's class census allocates maps while sampling each tick; no allocation-free claim is made. Its terminal
score comes from the logged outcome; black-box world rows are sampled and can precede the last scoring tick.
