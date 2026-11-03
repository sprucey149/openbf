---
id: FLNK
title: Flank routes: per post pair the best other way within +35 % is stored at load and a share of the attackers (10-30 %, drawn per match) takes it - the route table has no second way to tilt (the maintainer's annoyance 2; bot design 6.2)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's annoyance 2 (2026-10-02): "Some maps have totally unused parts ... On some maps you just know where everyone is going to go" - and his cautions: "It is still fun to have fighting hotspots", bots must not become "dumb" or spread thin. Measured (`analysis/bot-design.md` M5, 6.2): toward a post the authored route table has a second next hop at 0-2 % of its entries on ten maps, so a tilt of the weights cannot do it; another route within +35 % exists for 20 of 20 post pairs on Mygeeto, 22 of 30 on Tatooine Dune Sea, 13 of 18 on Yavin Arena, 18 of 30 on Polis Massa, 13 of 29 on Utapau, 7 of 12 on BF2 Hoth, 3 of 17 on Kamino. The PS2: every never-visited hub lies off every authored post-to-post route (`re-soldier-ai.md` 10b).

## Notes
- Cut from `analysis/bot-design.md` (card BOTD, 2026-10-03). Waits for the maintainer's read (deps BOTD). In BOTV's deps.
- "Another route" may only reuse ways authored for other post pairs (Mygeeto: 8 of its 20); it still splits an attack. The dead ground flank routes reach is 3-37 % of the off-route hubs; the rest is reached by positions (card TPOS).
- Unit counts per map are card UCNT.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decision 5 is settled: **flankers 20 % per match, drawn between 10 and 30**.
- 2026-10-05 planner: value 4 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set FLNK value=4`. `effort: high` (the builds the maintainer's top concern rests on; a build runs at medium by default). Undo: `loopctl.py set FLNK effort=`.
- 2026-10-20 build (lane 1, branch flnk dbcb413): built and gated. Acceptance, run (files `lead_tmp/flnk/`):
  - Table = M5's search on the ten maps (`table_t1.err`, the sweep's `flank` line): MYG1 20/20, UTA1 13/29, HOT1 7/12,
    NAB1 2/20, TAT1 22/30, YAV2 13/18, KAM1 3/17, POL1 18/30, DEA1 18/30; KAS2 24/42 - the M5 CSV's posts were stale
    (cp4/cp5 on hubs 69/33, today 42/47); `flank.py` re-run with today's hubs (`tools/flank_hubs.py`, `kas2_posts_now.csv`) also says 24.
  - Groups: an attack starts from the post whose hub is within 2 arcs of the hub it was planned from (bots plan from the
    spawn nodes' hubs round a post: MYG1 30 start hubs, 5 posts - with post hubs only, 0 roles in a round); a flanker
    joins its route within 3 arcs. Roles end: arrived / died / new objective (`ends A/D/T/O`) - most die on the way.
  - Null (FLANK=false) = main's 60000 42 rounds on all 32 maps but the hash (`null_42.txt`).
  - B11 (8 seeds, 32 where marked; null -> flank): KAM1 13.1 -> 12.8 (32 s; PS2 32), POL1 22.6 -> 18.9 (32 s; t -4.5; PS2 28);
    showcase NAB1 0 -> 0, TAT1 33.5 -> 34.5, YAV2 13.0 -> 12.4, UTA1 7.6 -> 9.8, MYG1 26.0 -> 23.8 (32 s, t -2.2),
    HOT1 45.8 -> 36.9 (32 s, t -4.2); DEA1 31.3 -> 26.4 (t -5.7). Main already met B11 on KAM1/POL1.
  - B5/B6/B7 hold: contact20 no drop past t -2 on any map over 32 seeds; deaths/min END1 t -2.5 at 32 seeds was noise
    (72 fresh seeds 27.52 -> 27.54); B7 DEA1 36.5 -> 28.6 (PS2 27.3, floor 18.3). Fidelity gate PASS.
  - Test `two_of_ten_attackers_flank_at_a_share_of_a_fifth_and_none_of_two` + 5 more; 5 of 5 mutations caught.
  - Sweep 256/256 at Normal, Hard and Elite. Recycles: BES1 6 -> 15 at 8 seeds was noise (72 seeds 85 -> 86).
- LATER (item 4, not this landing): lean the share to the flank while friends die on the main route.
- 2026-10-20 land (lane 1): LANDED 04c725a on eece67c, 0 goldens, sweep + arena_tat2 re-baselined. Table counts, shares and B11 re-checked on the gate's 8 fidelity seeds (report `reports/20261020_1328_land_FLNK.md`). Item 4 stays a LATER note.
