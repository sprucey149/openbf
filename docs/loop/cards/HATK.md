---
id: HATK
title: Hit-direction chevrons on the HUD: when the player is hit, both PS2 games flash the ambush_icon chevron at the screen edge toward the attacker (left / right; bottom from behind, top when the shot travels up; nothing from the front), full then a linear 1.0 s fade - analysis/re-hud-attacker.md
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
(the maintainer, 2026-10-20) When the player is shot, the original shows where the shot came from - he thinks a little arrow on screen pointing the way (maybe from another game). The BF2 HUD config does list attacker icons in its world-marker element; the port draws none.

## Evidence
`analysis/re-hud.md` §2 'World markers', `hud_target_*` textures (`re-hud-icons.md`); the sim's `last_hit` (the attacker) in `Unit`.

## Notes
- 2026-10-21 RE done (lane 2, c83a7d0, `analysis/re-hud-attacker.md`; report `reports/20261021_1514_card_HATK.md`):
  acceptance 1 MET. It IS in both PS2 games, but not as the Target element's attacker icons (those are dead data):
  a code-drawn `ambush_icon` chevron per screen edge (B2 `FUN_00391680`, B1 `FUN_002d8028`). PCSX2: 32 BF2 hits on
  KAM1 / DEA1 with the four sprites sampled per update (front 9/9 none, sides 12/14, behind 7/9; far 43-47 m same),
  frames `lead_tmp/hatk/shots/edges.png`, one BF1 hit (GEO1, bottom). Open [H] for the build: an explosion's
  direction (4 of 32 hits lit nothing), BF2 flyers (no flyer caller found).
- Decision (reversible): the port draws BF2's `ambush_icon` on every map (BF2's HUD everywhere, PLAN); BF1's own sheet
  (solid orange-red) is `common_bf1/textures/ambush_icon` if the maintainer wants BF1 maps to use it.
- 2026-10-21 16:50 BUILD (lane 2, medium) on branch `hatk` 064b595: `Event::Hit.dir` / `VehicleHit.dir`, `ui/src/hud_ambush.rs`,
  `game::event_feed::ambush_edge` (the feed lives in event_feed.rs, not hud_feed.rs: it is where each tick's events are read),
  `HudModel::ambush`, golden `hud_ambush.png`. Acceptance run: unit tests (rule quadrants + 45 deg lines, fade, PS2-frame
  places at 640x480, Hor+, player frame at four yaws, pilot / gunner / collision / flyer / turret), 8 mutations caught; a real
  headless battle (idle invincible player, BF2 KAS2 + HOT1, 3 min each): 204 / 205 hits lit the edge on the shooter's true
  side (`lead_tmp/hatk/score_ref.py ref_bf2_*.err`; the miss sat at the 45 deg line). Gates: only the declared golden.
