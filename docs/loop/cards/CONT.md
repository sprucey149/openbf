---
id: CONT
title: Bots do not close with the enemy: on the PS2 a Kamino soldier has an enemy within 20 m 25 % of the time, in the port 2-5 %; a bot's objective changes every 10 s (71 % back to one it just left) and the commander's threat term sends attacks where no enemy stands
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/commander.rs,engine/crates/game/examples/round_to_outcome.rs,engine/crates/core/src/sim/state.rs,scripts/sweep_seeds.py
declares: sim; sweep; arena
needs:
effort:
branch: cont
resume: landed 4c1460d5 2026-10-02
created: 2026-10-02
alpha: yes
---
## Why
Card OFFG (2026-10-02) fixed the bridges; BF2 Kamino now runs 13.8 kills / min against the PS2's 17.3 and the gap that is left is contact. The maintainer plays Kamino; a fight that happens at 70 m with nobody closing is what he sees.

## Evidence (`lead_tmp/offg/README.md` section 5; tools in `lead_tmp/offg/tools/`)
- `contact.py`: nearest enemy of every alive unit sample. PS2 KAM1 (420 s, `lead_tmp/stall/ps2/bf2_kam1_soldiers.csv`, teams by class pointer): within 20 m 24.9 %, 40 m 39.6 %, 60 m 52.6 %, median 55 m. Port (seeds 1-3, with OFFG's fix): 2.4-4.7 %, 13.5-18.0 %, 31-35 %, median 72-75 m.
- `thrash.py`: an objective change per 9.6 unit-seconds; 71 % return to an objective held within the last 60 s; median run 5 s. BF2 POL1 the same (one per 10 s, 60 %).
- Causes of 1,847 changes (KAM1 seed 1, 18,000 ticks, PLAN_LOG in `tools/scratch_all_probes.patch`): another base beats it by 1.2 x 33 %; own base <= 0 from THREAT_W 20.5 %; bumped by the cap 16 % (units plan in index order, so a late index loses its place to an early one that just switched); quiet post 7 %; crowd 6.6 %; owner 4 %; noise alone 3.7 %.
- `commander.rs`: an attack's base is value x 1 / (1 + hops) - THREAT_W x enemies within 30 m. Kamino has 130 hubs: bases are 0.1-1.0 and one defender (0.5) takes the post off the list.
- The PS2's own planner is not RE'd (`commander.rs` module header). BF2's `lua/mission/objectiveconquest.luac` holds the strings AddAIGoal, Conquest, AIGoals, AIGoalWeight (strings only; the call's arguments are unread - `lead_tmp/audit-annc/dis50.py` reads a function's control flow) and the ELF registers AddAIGoal / DeleteAIGoal / ClearAIGoals: a bounded RE of the Conquest goal's post picker would replace guesses.

## The card's first plan (before the RE; kept for the record)
1. Bounded RE first (the RE discipline, verifier included): BF2's Conquest AI goal - how a bot picks its post and when it re-picks. If it is found in a session, build that; if not, step 2 with the PS2's contact numbers as referee.
2. Ours (PROVISIONAL): keepers first (a unit that holds a still-valid objective keeps its place under the cap before any newcomer); the kept objective judged without the threat and crowd terms; the threat term relative (value x near / (1 + w x threat)) or gone. Variants as env knobs in a scratch worktree, 8 seeds x 32 maps each, same base.
3. Port `contact.py`'s and `thrash.py`'s measures into round_to_outcome's `shares` line.
First acceptance: KAM1 within 20 m >= 12 % (PS2 24.9 %), kills / min >= 16, objective changes at most one per 30 unit-seconds; POL1 kills / min not under 16.0, recycled not over 30 (hysteresis 3 took them to 11.7 and 234: `lead_tmp/offg/q/stable_h3`).

## Notes

- CWSP RE (2026-10-02, `analysis/re-command-walker-post.md` section 8, verifier-checked): BF2's conquest-goal target
  pick `FUN_002e8318` takes EVERY scheduled post not friendly to the bot's team as a candidate, weighted by
  `FUN_002e87b8` (Value_ATK of the bot's faction for a hostile post, the mean of ATK and DEF for a neutral one), minus
  posts whose ban flags match the bot's vehicle class, the excluded one, value <= 0, and posts failing a
  planning-graph test `FUN_002ebf70` at the post's position (not opened). The position (`FUN_002a6c68`) is the capture
  region's, or the OWNER's LIVE position when there is no region — so a hostile AT-AT's post (Value_ATK 10, the same
  as a fixed post) is a moving attack target: on BF2 Hoth up to two of the Rebels' four candidates. Own posts are
  candidates only in pass 1 (when no attack candidate exists), by Value_DEF (a walker's is 0). Start the RE from
  that function; BF1's twin is `FUN_002618b8` (not opened).

- Planner 2026-10-02 09:30: `files:` emptied while this card is `phase: re`. An RE writes `analysis/` only, and the
  picker counts a claimed card's files as busy: with `core/src/commander.rs` and `game/examples/round_to_outcome.rs`
  listed, this RE would have made STEND (the maintainer's `first: 7`), CWSP and WAI2 ineligible for its whole session. When you
  re-phase to build, put them back: `engine/crates/core/src/commander.rs,engine/crates/game/examples/round_to_outcome.rs`
  (and any probe goes in a scratch worktree, as OFFG's did).

- RE 2026-10-02 (lane 2, report `reports/20261002_1105_card_CONT.md`, docs 0f5b13a): verifier 46 SUPPORTED / 10 PARTLY
  / 0 NOT SUPPORTED of 56, all folded in. Two live 9-minute PS2 battles (`lead_tmp/cont/ps2/`, sampler
  `lead_tmp/cont/cont_ps2.py <map>`, analysis `analyse.py` / `changes.py` / `after_capture.py`): 0 of 578 draws took a
  friendly post; the draw is even over distance ranks; Polis Massa's 20-valued posts drawn 118 of 185 (125.5 by value,
  95.9 if uniform); bots on foot change target once per 233-323 alive unit-seconds; 8-9 % of lives reach their
  target's capture region (median closest approach 131 m on Kamino) — the fight is between the posts.
- Not for this card (non-showcase, post-alpha notes for the planner): ten BF2 conquest scripts add their own goals —
  `Deathmatch` for a locals team (geo1c, kas2c/g, nab2c/g, tat3c/g), `Defend` / `Destroy` goals on kas2c's gate panel
  and barricades. Those goal classes are not opened.
- Both PS2 samples have the player on the spawn map: every bot then thinks at the slowest interval (an order arrives
  4.0 s after a spawn). A sample with the player in the battle would show whether nearby bots react faster.

## Note from OBST's measurement (2026-10-02, `lead_tmp/obst/README.md`) - the objective flip seen as a stall
2,162 of 16,224 stalled 10 s windows (13 %) are bots at full speed with NOTHING shutting their way and under 3 m net:
the objective changes and the bot turns round. BF2 KAM1 seed 1, unit 13, ticks 3615-3960 by cp1 at (-28, 60.3, 36):
attack3 -> attack4 -> attack2 -> attack3 -> attack2 -> attack3, a change every 1.5-3 s, walking arc 40 between hubs 47
and 48 and back. Worst maps: BF2 UTA1 260 windows, BF1 RHN1 244 (207 at one spot, (-156, 7.2, 36)), BF2 KAM1 225.
`lead_tmp/obst/all/spots.md`, class `open`. A before / after number for this card: the `open` count.

## Build 2026-10-02 (lane 1, branch `cont` eeee7de, report `reports/20261002_1510_card_CONT.md`) - acceptance as run
8 seeds x 32 maps x 60,000 ticks, main 8069b44 with the same measures (`lead_tmp/cont/build/sw_base`) against the
branch (`sw_new2`); `lead_tmp/cont/build/sum.py`.
- Draw test `the_draw_is_by_value_over_the_posts_that_are_not_the_teams`: 10,000 draws on Polis Massa's start, team 2
  16.7 / 33.3 / 0 / 17.1 / 0 / 32.9 % over cp1..cp6con (want 16.7 / 33.3 / - / 16.7 / - / 33.3), team 1
  0 / 39.7 / 20.8 / 0 / 39.6 / 0 (want 40 / 20 / 40); 0 own posts. BF1's rule in
  `the_bf1_draw_weighs_team_1_by_atk_and_team_2_by_def`. Mutation (every weight 1): four draw tests fail. **MET.**
- BF2 KAM1: target changes one per 358 on-foot unit-s (base 9; PS2 323) **MET**; kills / min 17.3 (base 14.5; PS2
  17.3) **MET**; nearest enemy within 20 m **3.0 %** (base 2.0 %; wanted >= 12 %) **NOT MET** -> card FGHT.
- BF2 POL1: within 20 m 23.1 % (base 26.8 %; wanted >= 20) **MET**; target changes one per 595 **MET**; kills / min
  19.0 (base 18.7) **MET**; recycled 12 (base 2; cap 30) **MET**.
- Finish counts: 256 / 256 on both, no map drops **MET**. Recycled 277 -> 413 (TAT1 48 -> 112, BF2 DAG1 8 -> 26,
  BF2 GEO1 45 -> 67, BF1 HOT1 7 -> 23, BF2 HOT1 18 -> 31, BF1 YAV1 9 -> 20): bots now cross the whole map to a
  drawn post and meet more of the stall spots LLEG / OBST / PARR own.
- TAT1 changes target once per 27 on-foot unit-s: its dune-sea posts share one hub no soldier route leaves, so a
  unit flips between Hunt (on that hub) and Attack (off it). Note for LLEG.

## Audit 2026-10-02 (lane 3, `reports/20261002_2300_audit_CONT.md`): PARTLY -> card HUBM
The RE and every landed number hold (both ELFs re-read, verifier 33 / 3 / 0 of 36; clean rebuilds of 2e54da7 and
4c1460d5 print the CWSP and CONT gate baselines and the session's 16 raw sweep files byte for byte). Missed: the reach
test this build added to `plan` rests on `NavGraph::locate_on`, which lacks the PS2's hub mask, and the build's own
answer to TAT1 (Hunt as a result of the draw) hides it: on main TAT1's bots hunt 52.2 % of on-foot unit-seconds.
Four rules have no test (BF1's exclusion, the passenger rule, the reach drop in the offer, the no-route redraw).
Evidence `lead_tmp/audit-cont/README.md`.
