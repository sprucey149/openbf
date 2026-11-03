---
id: PWAI
title: BF2 bots detour to useful health/ammo pickups, including far/no-human bots under approved BOTD D1; retain the PS2 near/far rate sample and BF1 negative control
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
`analysis/re-powerups.md` §6 (`[O]`, 772d174). BF2 only: `FUN_002bd628` -> `FUN_002be050(6.0)`. A bot whose AI level of
detail is 3 or more (4 within 7 m of a human player, 3 to 20 m, less 1 per failed visibility test), on foot, looks
every 2.0 s (4.0 s more after a hit) at up to 6 items within 6 m; for one whose class gives its health type health it
lacks or ammo it lacks, and that passes a path test, it queues: here, 1 m past the item, here again. Energy and buff
items are not looked for. BF1 has no such code (the only reader of the item type is the supply behaviour).
With the player away no PS2 bot detours (the four battles of re-powerups §8 had the player on the spawn map).

## What to build
A new `core/src/ai/pickup_seek.rs` called from the bot's think (the caller's one line is the builder's to place; keep
`ai/brain.rs` edits to that line - the maintainer's numbered cards list it): the 2 s / 4 s cadence, the 6 m gather over the
sim's live items (card PWUP), the need test (PWUP's give-health / give-ammo dry run), a walkable straight line as the
path test (PROVISIONAL: `FUN_002ef6b8` is unread), the three-point detour. BF2 only. The port's stand-in for the level
of detail: a human player within 20 m.

## Acceptance (run each, paste the output)
- A PS2 sample FIRST (the rate is not measured): `lead_tmp/pwup/live/pw_live.py bf2 <state>` with the player spawned
  and standing in the battle (hold him, god mode: walk_bf2.py's hold), counting items taken within 20 m of the player
  per minute against the same battle's far items (re-powerups §8: 1.1 / 4.9 per minute with nobody near).
- Sim tests: a hurt bot 5 m from a health item with a human 10 m away reaches it within 4 s; the same bot with no
  human within 20 m also reaches it within 4 s (approved BOTD D1); repeat with a human farther than 20 m.
  A full-health, full-ammo bot does not detour; an energy or buff item is ignored; a BF1
  bot never detours. Mutation-check the need test.
- Battle census: detours per minute and items taken near the player, beside the PS2 sample's number.
- Gates: declared `sim; sweep` only.

## Notes
- Created 2026-10-02 by PWUP's RE pass (lane 2). Deps PWUP (the items must exist).
## Planner 2026-10-02 17:45 - waits for BOTD
This card changes the soldier brain (`core/src/ai`) that the bot programme is about to redesign (the maintainer, 2026-10-02: "are we just implementing so many bandaid fixes to a fundamentally flawed bot algorithm"; BOTD: no rewrite before he has read the design). Held so no lane builds on the old brain meanwhile; BOTD adopts, rewrites or closes it when it cuts its build cards. Undo: remove BOTD from `deps:`.

## BOTD (2026-10-03) - adopted; after MOVR
Under D1 (one think rate for every bot, bot-design.md 9) the PS2's "near the player only" detour applies to every bot: say so in FEEL_NOTES. Not in BOTV's deps.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decision 8 (one think rate, D1): the detour applies to every bot, not only those near the player.

## Active build scope (2026-10-24 13:06 UTC; approved BOTD D1)
The BOTD adoption above supersedes the original human-distance gate in What to
build and the original no-human negative control ("does not turn"). Preserve
the PS2 near/far sampling FIRST as source-game evidence; the port's near, far
and no-human BF2 cases all seek, as specified by bot-design.md D1 and its PWAI row.
Mutation-check both the need test and an accidental restoration of the 20 m
human gate; retain the original full-health/ammo, energy/buff and BF1 negatives.
The live caller is ai/behave.rs, not the historical ai/brain.rs. Route the detour
through MoveRequest and the sole mover; persistent/hash state belongs in
ai/state.rs, the module registration in ai.rs. Share a health/ammo-only dry-run
with sim/pickups.rs without granting an item during the query. The file set now
covers those integration points; keep the original 2 s/4 s, 6 m, three-point
detour, battle census and declared gates. Record the deviation in FEEL_NOTES only
with the eventual implementation. No PS2 number or port success was measured here.
