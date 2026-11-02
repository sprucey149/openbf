---
id: GRAIA
title: The weapon choice, from its audit: 8 of 12 mutations pass every test (all four rules of the second commit), three PS2 rules the port has differently ("plenty" is the load a weapon was issued, BF1 scores one shot, BF2 does not choose a reloading weapon), and the bots' grenades put Tantive IV's team kills at 5.1 % of kills (2.0 % without them; the bar is 3 %)
milestone: M3
value: 5
phase: build
status: done
deps: GRAI
files: engine/crates/core/src/ai/arsenal.rs,engine/crates/core/src/ai/arsenal/tests.rs,engine/crates/core/src/weapon.rs
declares: sim; sweep; arena
needs:
effort:
branch: graia
resume: landed e9315326 2026-10-12
created: 2026-10-07
alpha: yes
---
## Why
AUDIT GRAI 2026-10-07: PARTLY (`engine/artifacts/loop/audits.md`; evidence `engine/artifacts/lead_tmp/audit-grai/README.md`,
`analysis/re-grenades.md` 7.5, report `reports/20261007_1735_audit_GRAI.md`). The landing's numbers reproduce byte for
byte and its rules mostly hold against both ELFs and four PS2 captures. What does not:
1. **Tests.** 8 of 12 source mutations pass all 567 core tests: all four rules of the second commit (the stand to
   throw, the clear arc, MinRange at the landing, the head start from the own grenade) and four of the first's (BF1's
   plenty threshold, the explosive claim in `think`, `GROUP_RADIUS` 9 -> 13, the lead). The thrown score is caught only
   by a fixture's premise.
2. **Three PS2 rules `arsenal::choose` has differently** (re-grenades 7.5; read by the audit, no verifier yet):
   "plenty" is the load a weapon was ISSUED (the port asks the rounds left - a rifle down to 20 / 2 rounds gives way to
   the pistol - and `the_choice_prefers_range_then_plenty_then_score` pins that reading); BF1's score is one shot's
   damage (the port uses BF2's rate on BF1 maps: a rebel sniper 20-24 m from his target takes the pistol, rate 200
   against 160, where 160 a shot against 40 keeps the rifle); BF2 does not choose a weapon that is reloading (state 4)
   - its bot fights on with the pistol. Measured, each is small: see Acceptance.
3. **The grenades' cost to friends.** Main, 8 seeds x 32 maps: 936 team kills + 87 self kills with 8,392 throws; with
   the grenade never forced (the weapon choice alone) 608 + 0. Team + self kills of kills per map: bf2_tan1 5.1 % (2.0 %
   without the grenades; 4.3 % with the card off), bf2_dea1 3.3 %, bf2_cor1 and bf2_pol1 3.0 %. BOTD's bar B4 is 3 % on
   every map; without the grenades every map is at or under 2.9 %. The landing's "1,098 -> 913" is the net of the mask
   (no rockets at infantry) and the grenades.
Also measured, for whoever weighs them: the stand to throw costs 0.54 points of contact20 on main (paired t -2.8) and
saves 16 self kills in 256 rounds; the clear-arc / MinRange tests halve the self kills (172 -> 87) and save 142 team
kills; the head start halves them too (160 -> 87). On today's main the card raises no recycles (67 against 75 off).
