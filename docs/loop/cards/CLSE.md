---
id: CLSE
title: Inside 10 m a PS2 bot's aimed weapon deals 65-90 hp a second, the port's 21-33 (beyond 20 m they are equal): split the port's close-range aiming time by firing, hit share, weapon and speed, the PS2's from SAIR's raw packets, name the cause and build it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card FGHT2's range measure (2026-10-09, `analysis/re-soldier-ai.md` 15.5): per second that a bot aims at a living
enemy, the damage that enemy takes is, inside 10 m, **65-90 hp on the PS2 (four BF2 captures) against 21-33 on the
port**; at 10-20 m 23-42 against 13-29; beyond 20 m about equal (18-29 against 11-26). A close fight on the port
lasts two to four times as long as the PS2's: bots that meet at a doorway or on a post stand shooting past each
other. It holds with the PS2's slow think knocked into the port (31-38 inside 10 m), so it is not awareness.

## Evidence
- `engine/artifacts/lead_tmp/fght2/tools/rng.py ps2 <sair units.csv>` / `port <botq dir> <map>` (an "aimer" = a bot
  on foot whose aim target is a living enemy: the PS2's aim order type 0 with an entity, the port's `target`; every
  fall of health is split over the enemies aiming at the victim); outputs `out/rng_kam1_pol1.txt`, `out/rng_more.txt`.
  PS2 captures `lead_tmp/sair/ps2/bf2_{kam1_v2,kam1_playtest,pol1_v2,pol1_alive}_units.csv`; the port's rows
  `lead_tmp/fght2/sw/base/bq/` (main 853ecf0, 8 seeds).
- Caveats to settle first: the PS2's inside-10 m aimer time is 65-140 s a capture; 23-28 % of the PS2's damage has
  nobody aiming (blasts, turrets, strays) against the port's 8-10 %; the port's `target` may be held on a bot that
  cannot fire (no line, reloading, mid-roll).
