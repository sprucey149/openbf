---
id: FAMBP
title: BF1 Naboo's fambaas as command posts outside the historical mission? In nab1c_h each fambaa is a mobile post for the Gungans (spawns at +-8 m, ValueBleed / Value_ATK / Value_DEF at the class default 100 against the map posts' 10): in conquest that would send ~87 % of the attackers' goal draws to the fambaas and stop the defenders bleeding while one lives - the maintainer decides
status: archived
issue: 82
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #82](https://github.com/sprucey149/openbf/issues/82).

## Why
FAMB brings the fambaas into every NAB1 mode (the maintainer's DECISION 1) as destructible buildings with the shield, WITHOUT
their post role. On the PS2 (historical `nab1c_h` only) a fambaa is also a command post for its team. Carried into
conquest, that changes the battle a lot, so it is the maintainer's call.

## Evidence (analysis/re-fambaa-shield.md 5)
- commandarmedanimatedbuilding = the armed animated building + the ordinary command-post component (B1 label table
  FUN_00172ab0, B1:15615-15617; post factory FUN_0029c788).
- Spawns: SpawnPointCount 6 at x +-8, z 0 / +-4; live 16 of 30 Gungan respawns in a minute came out there.
- Post class defaults 100 (B1:229234-229243; the fambaa classes author none) against NAB1's five posts at 10:
  attackers' goal draws (FUN_002618b8 / FUN_002617f8) ~87 % a fambaa [D]; a team whose post values sum above every
  bleed threshold does not bleed (FUN_002ad830) - while a fambaa lives its team never bleeds from posts [D].
- No capture region: it stops being a post only when destroyed (10000 health, inside the dome).

## Acceptance
- needs the maintainer's answer first (see needs:). With a yes: the port's spawn census shows team 1 (port) spawning at the
  fambaa points; over 8 seeds of bf1_nab1 the attackers' goal draws pick a fambaa ~87 % of the time; rounds still
  finish (outcome count unchanged); a test kills both fambaas and sees the post gone and the bleed resume.

## Notes
- Created 2026-10-16 by FAMB's RE (lane 2). Default until the maintainer answers: no post role (FAMB builds the buildings and
  the shield only).
