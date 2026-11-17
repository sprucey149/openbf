# Felucia's 6-acklay objective: a bug in the original mission script

Tags: [V] read in the PS2 executable and its mission bytecode, [M] measured in
save states and a live PS2 playtest, [D] derived, [H] hypothesis. This is an
original-script finding, not a claim of original behaviour to copy verbatim.

## The script

`fel1c_c` (PS2 `fel1c_c.luac`, generated referee
`engine/missions/campaign/bf2_fel1c_c.ron:667-693`) registers
`OnObjectKillClass(callback, "geo_inf_acklay")`. Its callback decrements the
`acklay_count` global **only inside** an `IsCharacterHuman(killer)` test. A
second `Ambush("acklay_spawn", 3, 5, 0.5)` is requested only when the counter
equals exactly 3, and `Objective2:Complete` only when it equals 0. Six acklays
are authored in total (`SetMemoryPoolSize("AcklayData", 6)`), arriving as two
waves of 3.

If the counter never reaches 3, the second wave is never requested and the
objective cannot complete. The allies also keep their `Defend atte_busted`
goal, which leaves them clustered at the damaged vehicle.

## What the original game means by `IsCharacterHuman`

B2 `FUN_001912b8` at `0x1912b8` (`analysis/SWBFRNT2.ELF-decomp.c:35656`) takes
the argument as an index into the 0x1a4-byte member/player pool
`DAT_00661330` (count `DAT_00661334`), bounds-checks it and pushes
`member + 0x130 >= 0`. Field `+0x130` is the player index: a human has one, an
AI member has it negative (see `re-spawn-flow.md` §1, `re-ai-difficulty.md` §2,
`re-conquest-goal.md` §5). So the check is simply "is the killer member a
human".

Kills are credited to the actual killer member, AI included. The PS2 KAM1
playtest in `re-blaster-damage.md` §5.4 recorded 91 kills, all by AI, with the
human killing nobody, and the per-player counters matched. The death handler
`FUN_0034f650` sends a friendly victim to `FUN_00362c60` and an enemy one to
`FUN_00362268`, using the killer member in both paths (`re-event-messages.md`
§5). An AI kill therefore also yields `IsCharacterHuman == false`.

## The bug

The script is written on the assumption that the player gets the acklay kills.
It has no rule making acklays player-only:

- the Ambush library's `SetAIDamageThreshold(unit, 0.5)` only writes a flag
  (`entity+0x18c`) read as a standoff-band halver in the soldier AI and the
  planner (`re-ai-difficulty.md:185`); it is not a damage or targeting veto
- the acklay class (`engine/assets_baked/classes/bf2_geo.ron:551`) has 3000 HP
  and health type `animal`, with no player-only damage rule
- the script sets team 5 (the acklays) hostile to the player's team ATT
  (`bf2_fel1c_c.ron:266-267`), so those allies will engage and kill them

On the PS2 the allies apparently do not take those kills — level layout, the
`acklay1/2/3` barriers and the opposing `Defend atte_busted` goals keep them
apart [H]. In openbf they do: a Felucia playtest logged three acklay deaths
credited to allied AI members (unit indices 7 and 12), each with
`IsCharacterHuman == false`, leaving `acklay_count` at 6. The mission then
softlocks exactly as the script's arithmetic predicts.

## Port workaround

openbf credits an acklay kill by any unit allied to the player to the local
player when firing `ObjectKill` (`game/src/campaign_runtime.rs`,
`Event::Kill` branch). The script's own counter, second wave and completion
then work unchanged. This is a deliberate deviation: it preserves the
mission's observable intent ("kill 6 acklays") while working around an
original-script bug that the port's AI behaviour makes reachable.

`replace:` an upstream script fix that counts friendly kills, or an authored
player-only acklay damage rule; either would let the workaround be removed.

Evidence: Felucia playtests in `dist/openbf_data/playtest/matches/` (2026-11-16
to 2026-11-17), the kill-credit diagnostic runs of `dist/openbf-fixed.exe`, and
the headless regression
`campaign_runtime::tests::fel1c_c_acklay_objective_counts_ally_kills_and_spawns_both_waves`.
