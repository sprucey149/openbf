---
id: HUBP
title: A bot walks at a random point of the next hub's sphere, drawn per unit per leg (the PS2's waypoint), not at the hub's centre: bots file through hub centres - a teammate within 1.5 m 12-22 % of the time against the PS2's 2-9 %
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The one spread the PS2 gives its bots on a route is the waypoint: at every hub each unit draws its own point in the
NEXT hub. The port sends every bot at the hub's centre (`ai/route.rs` `hub_waypoint`, "OURS (PROVISIONAL): the PS2
walks at a re-rolled point in the hub"), so bots walk in single file, stack on centres and contest the same metre:
a teammate within 1.5 m 12.5 % (KAM1), 21.9 % (POL1), 21.5 % (DEA1), 11.8 % (KAS2) of the time against the PS2's
6.6 / 5.7 / 9.3 / 2.0 % (`analysis/bot-quality.md`). A plain defect against the PS2 (card SAIR, `re-soldier-ai.md` 2
and the table row "point in the hub"); behind BOTD only because it moves every bot's path.

## Evidence
- [V, verifier A14] B2 `FUN_002ea128` (B2:275051-275109), BF1 `FUN_002657c8` (B1:185275-185332): waypoint = hub
  centre + normalize(three LCG values in [-1, 1)) x max(0, radius - h), h = 4 m for a droideka, else 0: a point on
  a SPHERE of the hub's radius (so anywhere in the hub's disc in XZ, and off its height - the walk uses XZ).
- [V] Drawn once per hub advance, per unit, for the NEXT hub (`FUN_002ed2d0`); re-drawn up to 100 times while the
  point lies inside a barrier (`FUN_002ed3d8` / `FUN_002eea68`); no timer re-draws it; the destination hub's
  waypoint is the order's own point (the post's region point) when that lies in it.
- [V] Steering on the leg (BF2): the projection of the unit on the leg advanced by |v| + 0.2 m, clamped to the leg's
  end (`FUN_002a5fa8`); arrival XZ 1.0 m (0.5 m last node) or the node's plane passed.
- The draw must come from the port's per-unit hash (`ai/draw.rs`), not a shared LCG, to keep the round replayable.

## Notes
- Related PS2 facts for the same code, small: the arrival floor is 1.0 m (port 1.5), and the PS2's only avoidance
  of other units is the contact side-step (unit position + 5 m along the contact's perpendicular, side kept).

## BOTD (2026-10-03) - adopted as designed: build step 2, after SPL11
`analysis/bot-design.md` 4.5. The draw lives in the Navigator (`ai/route.rs`). Acceptance unchanged; bar B3 (a teammate within 1.5 m at most the PS2's) is finished by card TEAM. In BOTV's deps.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**
- 2026-10-05 planner: value 4 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set HUBP value=4`.

## Landed 2026-10-06 (408bc9c) - what the acceptance measured
Report `reports/20261006_1135_card_HUBP.md`; tables `lead_tmp/hubp/ab/full_base` against `full_var2` (8 seeds x 32).
- A teammate within 1.5 m: KAS2 9.5 -> 2.9 (met), DEA1 18.8 -> 11.6 (met), KAM1 9.9 -> 5.8 (met, under the PS2's
  6.6), POL1 18.9 -> 13.9 (**missed**, bar 12.3: 68 % of its hubs are under 2 m). All maps: mean 12.5 -> 7.2 %,
  lower on 32 of 32.
- 256 of 256 rounds; recycles 229 -> 211; kills +0.9 %; "lost" deaths 4 -> 6; walking in place 1.77 -> 1.82 %.
- BF1 BES1 worse (deaths under the floor 147 -> 234, recycles 5 -> 29): card BESR.
- Unit tests: 6, mutation-checked (8 mutations).
- 2026-10-06 **AUDIT: PARTLY** (`reports/20261006_1535_audit_HUBP.md`, `lead_tmp/audit-hubp/README.md`). Held: the
  PS2 rule (both decomps; 1,058 live waypoints in RAM), the port's draw, every number above (rebuilt binaries = the
  gate baselines and `full_base` / `full_var2` byte for byte). Not said: on Hoth 52-62 % of legs still take the
  centre and 24 % at hubs of 16 m and more (the census "97-99 % of legs" was the four acceptance maps'); the arrival
  plane moved for a tauntaun's leg to a post; the barrier test has no unit radius; "kills +0.9 %" is 30 maps (all
  32: -0.05 %; 40 seeds -0.56 %); "lost" 14 -> 30 over 40 seeds; three mutations alive. Card HUBO.
