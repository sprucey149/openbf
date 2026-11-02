---
id: UVO2
title: BF2's unit voice, the VOSound lines (the maintainer's INBOX 2026-10-12 18:55, ask 3; BF2's half of USND): the class's 162-211 lines picked by event and context (who is above, behind, near; what kind of unit the target is), spoken only by a unit within 50 m of the event's subject - the player, when the caller names none - an enemy 31 times in 101, the nearest team-mate when no speaker is named; AcquiredTarget, FriendlyFire, Grenade, Repair, GivePowerup, the slaughter counters, and for the player's own kills HeadshotKill and the killing sprees; each line property once per 40 +- 20 s (the Rebels 80 +- 40); the PS2 says 8-12 lines a minute round a player in the fight and one in four minutes round one at his post; RE done 2026-10-13 (analysis/re-unit-chatter.md 3, 5)
milestone: M3
value: 5
phase: build
status: done
deps: USND,UCALL
files: engine/crates/game/src/vo.rs,engine/crates/game/src/classdb.rs,engine/crates/game/src/event_sounds.rs,engine/crates/game/src/announce.rs,engine/crates/core/src/sim/state.rs,engine/crates/core/src/perception.rs
declares:
needs:
effort:
branch: uvo2
resume: landed b74b508b 2026-10-16; audit 20261026_0741 INCOMPLETE / UNAUDITED; restore primary evidence and verified history before retry (private continuation below).
created: 2026-10-13
alpha: yes
domain: audio
---
## Why
The maintainer's INBOX 2026-10-12 18:55, ask 3 (umbrella card USND; BF2's half of slices 1, 3 and 4): on BF2's maps the
units' voice is the class's `VOSound` lines - 162 to 211 a class - and the port plays none. The PS2 says 8 to 12 a
minute round a player in the fight and nearly nothing (one line in four minutes) round one standing at his spawn
post: the rule is built round the player (`analysis/re-unit-chatter.md` 3, 5 `[O]`).

## Evidence (the PS2's rule)
- **The data**: `VOSound = <sound> <event> [conditions]` on the class (tokens split on space , + - ;); a child class
  shares its parent's chain (in RAM 13 soldier classes hang on one 197-entry list on Mygeeto). `VOUnitType` 0-255;
  `VORadius` default 50 m, authored by no class.
- **The event function**: speaker given or, if none, the subject's nearest team-mate inside that team-mate's
  radius; the speaker must be within its `VORadius` of the SUBJECT, and the subject is the local player's unit
  whenever the caller names none (every event below but the slaughter counters and NotShootingCriticalHit); a
  speaker whose team is the subject's enemy goes on 31 times in 101; an AI speaker only at detail level 3 or 4;
  then the class entries of that event whose conditions all match the context (object above / below, behind /
  left / right, under 10 m / 30 m / over 100 m, health under a quarter, what the object is, what the speaker sits
  in, the OBJECT's `VOUnitType`), the one with the most matching conditions first; played 3-D at the speaker
  through the voice queue, or at once and 2-D when the speaker is the subject himself.
- **The events with lines and a trigger**: AcquiredTarget (the AI's tracker takes a target; 71 in 101),
  FriendlyFire (hit by a friend; the victim speaks), Grenade (BF1's flee rule), RepairStart / RepairEnd (an AI
  engineer's repair), GivePowerup, NearbyFriendlySlaughter / NearbyEnemySlaughter (four deaths of one kind inside
  8 s within 25 m of an AI unit), HeadshotKill and KillingSpree4 / 8 (a HUMAN's kills: a team-mate near him / his
  own unit; kills each within 4 s of the last), NotShootingCriticalHit, PathFollowWait, RebelsShootDeadBody (Rebel
  soldiers only), and the `SC_...` command and response lines of `re-squad-orders.md` 7 (on the quick stream).
- **Never**: `MissileIncoming` and the eight `...VO` request names are authored and fired by nothing.
- **The gates**: each line property 40 +- 20 s (`cis`, `imp`, `rep`), 80 +- 40 (`all`), 30 +- 5 (`wok`); takes
  locked 10 +- 3 s for the CIS, mostly unlocked for the Republic; BF2 steps to the next take when the drawn one is
  locked (`lead_tmp/usnd/bf2_snd_all.tsv`).
- **PS2 rates** (`lead_tmp/usnd/live_bf2.txt`, per minute, post / fight): lines UTA1 0.25 / 8.25, MYG1 0.25 /
  11.0; AcquiredTarget 0.25 / 6.25, 0 / 8.25; FriendlyFire 0 / 1.25, 0 / 1.5; RepairStart 0 / 0.25, 0 / 0.5;
  Grenade 0 / 0.25; SC responses 0-0.5.

## Notes
- [H] in the RE: whether HeadshotKill needs a headshot (build it on a head hit, PROVISIONAL); NotShootingCriticalHit's
  rule is read in part. HeadshotKill and the sprees are the "praise of the player" of card PRAISE: whichever card
  is built first takes them and leaves a MERGED note.
- The port has no AI detail level: every bot within the radius may speak (the radius does most of the thinning);
  say so as a named deviation if the census needs it.
## Outcome (2026-10-16, landed b74b508)
- Built: `game/src/vo.rs` (entries, context, ranking, speaker rule), AcquiredTarget, FriendlyFire, Grenade, KillingSpree4 / 8, the slaughter counters, BF2's take stepping. Not built: HeadshotKill, repairs, GivePowerup, PathFollowWait, RebelsShootDeadBody, NotShootingCriticalHit, SC_ (no port trigger); the AI detail-level gate (named deviation).
- Census (`lead_tmp/uvo2/census/tab_post.txt`, `tab_fight.txt`): post 1.53 / 1.06 VO lines a minute (bar at most 1: NOT met -> card UVO3); fight 5.25 / 6.88 lines (0.64 / 0.63 of the PS2's: NOT within 30 %), AcquiredTarget 0.78 / 0.69 -> card UVO4.
- Mixer (`lead_tmp/uvo2/win2/`, MYG1 CW windowed + follow, 21 VO starts in 3 windows): peaks identical with and without them (1.26 / 1.06 / 1.25 x full scale, the mix's own); no overloaded tick needs a VO start.
