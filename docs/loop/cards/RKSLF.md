---
id: RKSLF
title: Bots blow themselves up with their own rockets once rockets explode (card RKEX): the launcher's cycle releases the rocket ticks after the AI's press (InitialSalvoDelay, BF2 salvo of 2; every near shot left with FIRE up), by then into the wall or the vehicle beside the muzzle - self-kills over the 32-map fidelity sweep 64 -> 360 (seeds 1-8)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card RKEX makes a struck rocket detonate its ExplosionName / ExplosionImpact class (B1 FUN_00272d70 / B2 FUN_0030f550). On the branch rkex (1b04dfb) the fidelity gate refused teamkill_pct (all maps) 1.17 -> 1.84 %. Split of the 32-map botq kill events (engine/artifacts/lead_tmp/rkex/tk.py on fidelity_rebase_20261020b vs rkex/gates): self-kills 64 -> 360, team-mate kills 605 -> 723 (762 before RKEX's friend_in_blast).
Trace (scratch prints, 3 BF1 maps x 3 seeds, lead_tmp/rkex/rkself*.txt, rkshot.txt): 18 self-kills, the blast 1.3-2.7 m from the shooter, 15 on a WORLD body (cover beside the muzzle), 2 a hull at point blank, 1 a soldier. Every exploding shot that landed within 5 m left on a tick with FIRE NOT pressed: the weapon's cycle releases it after the press (BF2 InitialSalvoDelay 0.25 s, SalvoCount 2; BF1 a tick later), so a press-time check (tried: the muzzle ray + the target range within the blast + the wind-up walk) changed nothing (18 -> 13).
## Notes
Build it ON branch `rkex` (1b04dfb; RKEX is blocked on this very refusal, its worktree <repo parent>/openbf-wt/rkex): without RKEX no rocket explodes, so there is nothing to measure on main. Land the two together (RKEX's land after RKSLF's commits are on its branch), or merge RKSLF's commits into `rkex`. The scratch repro test of RKEX: lead_tmp/rkex/rkex_repro.rs.

Planner 2026-10-20 17:50: `branch: rkex` set so `loopctl start RKSLF` opens RKEX's own worktree (its explosion commits
bab9c23 / 1b04dfb stay under this fix); `first: 8` carried from RKEX (the maintainer's order for the rockets; this card is the only way
RKEX lands). Landing RKSLF lands both: the land session then sets RKEX `status=done` with the same commit. RKEX stays
`blocked` out of the rotation until then. The maintainer may instead approve `teamkill_pct@agg for RKEX` (STATUS question 64).
