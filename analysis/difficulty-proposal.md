# Difficulty proposal: harder fights, harder matches, fair with any mix of humans

Status: PROPOSAL (2026-10-28), not reviewed. It changes no code. The numbers were measured on `main` 5e6efa36
in a throwaway worktree, using a small scratch instrumentation of the sweep that was not committed (appendix A).
The other session's cards (HYST, MAIS, WHY, TOKN, BSTK on `botup`) and the tier rename on `tiers2` are **not** in
these runs. §7 says which recommendations depend on them.

**Tier names.** This note uses the renamed tiers (`tiers2`): Easy (skill 14), Normal (17, the default), Hard (20)
and Elite (20 plus the extras: error cap 0.5 m, view ×1.5, 5-tick reaction, pressure ×1.6, human damage ×1.3,
bleed ×1.25). On `main` the Normal, Hard and Elite rows are already the same rows under the same names, so every
Normal/Hard/Elite number below is that tier. Only `main`'s Easy differs: it is still the old skill-8 row, so the
"Easy" lines measure skill 8, not the coming skill-14 Easy. The maintainer played the top row, which is
`main`'s Elite.

## 0. The short version

1. **Why the top tier loses.** Both teams' bots get the same tier, so a bot-only round is a coin flip that the
   map decides: the side the human is on wins 44-58 % of rounds at every tier. Coruscant (clones vs droids)
   leans *toward the clones* (66 % over 32 rounds). The tier hardly moves kills, because both sides improve
   together. One extra strong unit on the clone side, our "human proxy", raised the clones' Coruscant wins from
   4-5 of 8 to 7 of 8. Elite's human-only levers then work against it. Its pressure ×1.6 pulls enemy fire onto
   the human and away from his bots (a hypothesis, card DPRX measures it). Its damage ×1.3 makes him die more,
   which is what he saw. Neither makes his *team* lose. The bleed ×1.25 is the one lever aimed at the team, and
   it is small.
2. **Firefight.** Elite bots are not better fighters than Hard bots. On the flat field Elite beat Hard 7 of 12
   (a coin flip), and on the maps by about 15 tickets. The skill curve tops out at 20, so more aim cannot be the
   lever. The levers are behaviour: dodging when hit or aimed at (today it is rare), a per-human attack budget
   with real focus fire (bots almost never focus today: 73-79 % of the hits a bot takes come from a single
   shooter), fall-back-and-heal, grenades at covered targets, and flanking the human. §2 gives numbers per tier.
3. **Match.** The strongest lever measured is **asymmetric competence**, which the PS2 also uses (enemies 17,
   the human's teammates 11). Enemy Elite against allied Normal: the allied side won 31 % instead of 58 %. With
   a strong proxy on the allied side it won 39 %. Next come enemy objective play (defend threatened posts,
   counter-attack, cover the capturer) and squad focus. Ticket and damage handicaps rank last for fairness.
4. **Multiplayer.** One rule: each team's bots take the tier's row **minus an ally offset that grows with that
   team's human surplus** (its humans minus the other team's, never below zero). Attack budgets are counted
   **per human**. Every combination follows from these two lines (table in §4).
5. **Measuring.** A "human proxy" in the sweep: a bot that the human layers treat as human (pressure, tokens,
   damage), with fixed strong stats, played on both sides of 8 maps × 8 seeds per tier. Each tier gets targets
   for the proxy team's win rate and the proxy's K/D. Running one tier takes about 2 minutes on this machine.
6. **Build plan.** Ten small cards (§6). The first two only measure. TOKN (the other session) becomes the
   per-human attack budget. HYST must land before the dodge card.

## 1. Why does the top tier lose to the player's own AI team?

### 1.1 Method

The sweep tool (`engine/crates/game/examples/round_to_outcome.rs`) plays bot-only conquest rounds. Every run was
16 v 16 (slot 0 vacated and **not** refilled; the sweep's default refills it, which makes team 0 field 17 against
16). Since 2026-10-30 (teamsize2) the sweep's default is each map's own team sizes and calibrated tickets, with no
filler; this 16 v 16 setup on the missions' tickets is now `SWEEP_COMBATANTS=16 SWEEP_MISSION_TICKETS=1`, and
`--difficulty-check` plays it by default. The missions were the Clone Wars ones (`*_con_cw.ron`; Geonosis has only `_con`). Each round ran to its end
(60,000-tick limit, never reached). Eight maps × 8 seeds (1-8) per configuration, 64 rounds each. A round takes
about 8 s of wall time, and one invocation runs 8 maps in parallel.

```text
# scratch build, own target dir, capped (appendix A lists the env vars added in scratch)
CARGO_TARGET_DIR=<scratch target> systemd-run --user --scope -q -p MemoryMax=16G \
  cargo build --release -p game --example round_to_outcome --example arena
# one configuration, per seed:
SWEEP_ERA=cw SWEEP_COMBATANTS=16 SWEEP_MISSION_TICKETS=1 SWEEP_DEATHS=1 SWEEP_THREADS=8 SWEEP_DIFFICULTY=<tier> \
  [SWEEP_TEAMS=<t0>:<t1>] [SWEEP_PROXY=1,<dealt>,<taken>] \
  timeout 900 systemd-run --user --scope -q -p MemoryMax=16G \
  target/release/examples/round_to_outcome 60000 <seed> \
  bf2_cor1 bf2_kam1 bf2_geo1 bf2_myg1 bf2_nab2 bf2_uta1 bf2_tat3 bf1_kas2
```

Columns: "kills" means kills made BY that team (the other team's deaths by `Event::Kill`). "Margin" is team 1's
tickets left minus team 0's at the end; positive means team 1 won by that much. Each mission's tickets are 150 a
side (BF1 Kashyyyk Docks has 250).

Sides (team 0 / team 1): Coruscant CIS/REP, Kamino CIS/REP, Geonosis REP/CIS, Mygeeto REP/CIS, Naboo CIS/REP,
Utapau CIS/REP, Tatooine REP/CIS, BF1 Kashyyyk Docks CIS/REP.

### 1.2 Symmetric tiers (both teams at the same tier, as the game does today)

All 8 maps, 64 rounds per tier:

| tier (both teams) | team 1 wins | mean margin (t1 − t0) | kills by t0 / t1 per round |
|---|---|---|---|
| Easy (skill 8 on main) | 28/64 (44 %) | −17.9 | 95.0 / 84.4 |
| Normal | 33/64 (52 %) | −12.3 | 99.5 / 90.5 |
| Hard | 32/64 (50 %) | −13.3 | 106.0 / 95.0 |
| Elite | 37/64 (58 %) | −1.8 | 104.8 / 92.0 |

Per map (team 1 wins of 8, mean margin):

| map | Easy | Normal | Hard | Elite | who wins |
|---|---|---|---|---|---|
| bf2_cor1 (CIS v REP) | 5, −3 | 6, +12 | 6, +9 | 4, −4 | **REP 21/32**, small margins (±30) |
| bf2_kam1 (CIS v REP) | 0, −85 | 0, −98 | 0, −96 | 0, −103 | team 0 always, by ~100 |
| bf2_myg1 (REP v CIS) | 8, +59 | 8, +76 | 8, +69 | 8, +79 | team 1 always |
| bf2_uta1 (CIS v REP) | 3, +1 | 8, +58 | 7, +53 | 8, +60 | team 1 |
| bf2_geo1 (REP v CIS) | 7, +42 | 5, +25 | 6, +18 | 7, +52 | team 1 |
| bf2_nab2 (CIS v REP) | 1, −35 | 3, −8 | 2, −21 | 3, −22 | team 0 |
| bf2_tat3 (REP v CIS) | 2, −53 | 3, −39 | 3, −35 | 6, +18 | mixed |
| bf1_kas2 (CIS v REP) | 2, −70 | 0, −124 | 0, −103 | 1, −95 | team 0 |

What this says:
- **The tier does not change who wins.** Symmetric competence makes every tier a coin flip, which the map then
  biases. The tier raises kills only a little: about +10 % per side from Easy to Hard, and Elite kills no more
  than Hard.
- **Coruscant leans toward the clones.** The REP won 21 of 32 symmetric rounds. Margins are small (±30 of 150),
  so one good player decides the round. The maintainer played that exact case.

### 1.3 Is the bias the map, the classes or the spawn?

Card-free test: the two teams' rosters (faction + class list) were **swapped** in copies of six missions. Posts,
spawn ownership and tickets stayed with the team number. Normal, 8 seeds:

| map | team 1 wins, original | team 1 wins, factions swapped | verdict |
|---|---|---|---|
| bf2_cor1 | 6/8 (REP), +12 | 1/8 (now CIS), −34 | **faction**: REP favoured by ~23 tickets; team slot ~−10 |
| bf2_nab2 | 3/8 (REP), −8 | 6/8 (now CIS), +22 | faction: CIS favoured by ~15 |
| bf2_kam1 | 0/8, −98 | 0/8, −79 | **map/spawn side**: team 0 wins whoever it is |
| bf2_myg1 | 8/8, +76 | 8/8, +76 | map/spawn side |
| bf2_uta1 | 8/8, +58 | 8/8, +54 | map/spawn side |
| bf2_geo1 | 5/8, +25 | 5/8, +19 | map/spawn side (mild) |

So the bias is **mostly the map's side** (posts and spawns): Kamino, Mygeeto and Utapau are 32-0. **Coruscant and
Naboo are faction (class) effects.** One class-side fact appears in every CW run's log: `roster cis: skipped class
cis_inf_droideka ... not an infantry class`. The CIS fields five classes and the REP six (the jet trooper
included). That is a likely contributor on Coruscant, and its own card (§6, DROST). The map-side bias is a
separate, map-balance problem. It matters here because it decides who wins, whatever the difficulty.

### 1.4 What one good human changes: the "human proxy"

`SWEEP_PROXY=1,<dealt>,<taken>` (scratch) takes one team-1 bot and gives it a permanent damage-dealt and
damage-taken multiplier (through `Buffs`). It also counts that unit's kills and deaths across respawns. It is NOT
treated as a human by the sim, so pressure, human damage, bleed handicap and tokens do not apply to it (card DPRX
fixes that). Elite on both teams, 64 rounds:

| team 1 has... | team 1 wins | mean margin | proxy kills / deaths per round |
|---|---|---|---|
| a plain bot (control, ×1/×1) | 37/64 (58 %) | −2.1 | 5.0 / 6.1 |
| a proxy ×2 dealt, ×0.5 taken | 39/64 (61 %) | +3.8 | 9.4 / 4.8 |
| a proxy ×3 dealt, ×0.33 taken | 41/64 (64 %) | +8.3 | 11.6 / 4.2 |
| on Coruscant only, ×3 proxy | 7/8 (control 5/8) | +28.8 (control +6.1) | 16.2 / 5.0 |

The proxy *understates* a real good player. It walks with the team's objective logic, so it kills only 12-16
times a round, where a good human typically gets 25-40 against bots. Even so, it turns Coruscant from 5/8 into
7/8 for the clones. **That reproduces the maintainer's round**: a map that already leans his way, a team of
equal bots, and one player who wins more fights than a bot does.

What Elite adds against him today, and why it does not stop the win:
- **Damage into a human ×1.3** (D12, ours): he dies more. He said so.
- **Pressure ×1.6** (target choice): enemy bots prefer him. Every bot that turns to him is a bot not shooting his
  teammates. He saw "my teammates killed more droids too". This is a **hypothesis**: the decoy effect was not
  measured, because the proxy is not `is_player`. Card DPRX measures it first.
- **Bleed ×1.25** against a team with a human: it bites only while his team holds fewer posts, which a winning
  team does not.
- **Nothing makes his teammates weaker or the enemy team better at the objective.** On the PS2 his BF2
  teammates were skill 11 against enemies at 17 (`re-ai-difficulty.md` §2.1). The port gives both teams 20.

### 1.5 Asymmetric tiers (`Sim::team_difficulty`, set by the sweep in scratch)

Team 0 = "the enemy", team 1 = "the human's team". Each pair was also run the other way round, so the
map-side bias cancels in the "effect" column (tier A's advantage = (margin with A on t1 − margin with A on t0) / 2).

| enemy : allies | allies win | mean margin | reversed (allies win) | effect of the stronger tier |
|---|---|---|---|---|
| Elite : Normal | 20/64 (31 %) | −54.7 | Normal : Elite → 52/64 the Elite side | **+49 tickets, wins 75 %** |
| Hard : Normal | 21/64 (33 %) | −40.4 | Normal : Hard → 44/64 | +37 tickets, wins 68 % |
| Elite : Hard | 31/64 (48 %) | −25.7 | Hard : Elite → 34/64 | +15 tickets, wins 52 % (coin flip) |
| Elite : Easy (skill 8) | 0/64 | −106.8 | — | total rout |
| Elite : Normal + ×2 proxy | 25/64 (39 %) | −43.3 | — | proxy 7.6 K / 6.4 D |
| Elite : Normal + ×3 proxy | 25/64 (39 %) | −38.7 | — | proxy 10.4 K / 5.3 D |
| Elite : Hard + ×3 proxy | 35/64 (55 %) | −8.3 | — | proxy 11.8 K / 4.6 D |

Flat-field arena (8 v 8, one life, `arena 3000 1 flat vs all 12`):

```text
beats: normal>easy yes  share 0.777  t +21.77  won 12 of 12  kills 193/56  margin +5.6
beats: hard>normal yes  share 0.727  t +10.54  won 12 of 12  kills 192/73  margin +4.7
beats: elite>hard coin flip  share 0.531  t +1.05  won 7 of 12  kills 153/134  margin +0.7
```

In-tier rows (`arena 3000 <seed> flat all`, seeds 1-3): hit rate Normal 0.38-0.42, Hard 0.34-0.39, Elite
0.27-0.31. Elite shooters do not miss more. Elite *targets* dodge and react more (REACT's Elite row), so
Elite-on-Elite hits fall. Kills/min: Normal 48-55, Hard 46-51, Elite 41-43.

**Recommendation (Q1).** The top tier lost because (a) symmetric competence makes the round a map coin flip,
(b) Coruscant leans toward the clones, (c) one competent human tips a coin flip, and (d) Elite's extras either
punish only the human's own deaths (damage) or may even help his team (pressure). Fix the match with the
per-team ally offset of §3/§4, which the PS2 also used. Fix the fight with behaviour levers (§2), because aim is
already at the skill cap. Fix Coruscant's class gap (droideka) as a separate faithfulness item.

## 2. Firefight difficulty: levers that do not cheat

Elite's aim is already the skill curve's top (skill 20: spread ×1, dispersion 0, error floor 0.4 m, cap 0.5 m).
Measured: Elite ≈ Hard bot-vs-bot. **More aim is not the lever. The fight has to get harder by what bots do.**

Measured gaps today:
- **Dodging is rare**, from the 2026-10-28 measurement in the brief, per bot-minute in a fight: 12-18 hits taken,
  0.7-3 REACT steps, ~0.1 rolls, 0.15-0.9 D11 side-steps. Causes: ~40 % of hits land inside the 1.5 s REACT
  cooldown, Tactician (cover/take) bots never move when hit, REACT draws "stay" 35 % at Normal, and rolls usually
  fail the clearance check.
- **Focus fire almost never happens.** For one team-1 bot over 4 seeds × 8 maps per tier, each hit it took was
  sorted by how many distinct enemies had hit it in the last 3 s (scratch `PROXYHITS`):

| tier | 1 shooter | 2 | 3 | 4+ | hits sampled |
|---|---|---|---|---|---|
| Normal | 73 % | 22 % | 4 % | 0 % | 1,080 |
| Hard | 79 % | 18 % | 3 % | 0 % | 947 |
| Elite | 73 % | 22 % | 4 % | 0 % | 878 |

  Bot-vs-bot, three or more shooters on one target within 3 s is ≤4 % at every tier. Against a human the
  pressure weight raises this, but by an unmeasured amount (DPRX).

Proposed levers, per tier. All are OURS and PROVISIONAL, and the arena plus the proxy gate (§5) set the final
numbers. "Today" is `main`.

| lever | Easy | Normal | Hard | Elite | today |
|---|---|---|---|---|---|
| **Attack budget per human** (bots allowed to *engage* one human at once; the rest fight his teammates or move) = TOKN's tokens | 1 | 2 | 4 | 6, plus focus (below) | no limit (pressure weight only) |
| **Focus on the human** (when one budget holder hits, a second holder within 30 m that sees him fires within 0.5 s) | no | no | yes | yes, and a third joins | ~never (table above) |
| **React on hit**: stay / strafe / roll / move | 50/30/5/15 | 35/30/15/20 | 15/35/20/30 | 5/35/25/35 | 35/30/15/20 at Normal, 10/30/25/35 at Elite |
| REACT cooldown (hits inside it get no draw) | 2.0 s | 1.5 s | 1.0 s | 0.7 s | 1.5 s every tier (≈40 % of hits fall inside) |
| Tactician bot in cover when hit | stays | REACT draw | REACT draw | REACT draw; two hits leave the spot | never moves |
| Roll fails clearance → | stay | strafe instead | strafe instead | strafe instead | nothing |
| **Evasive step when aimed at** (the human's reticle on it ≥ 0.3 s, the PS2's 18° zig-zag test) | 10 % | 30 % | 50 % | 65 % | none |
| Target: a dodge measure, moving bot-minutes in a fight with ≥1 move per 3 hits | ≥ 20 % | ≥ 40 % | ≥ 60 % | ≥ 75 % | 7-24 % |
| First shot after first sight (reaction) | 0.3 s | 0.3 s | 0.2 s | 0.17 s | same (keep; HYST keeps it the reaction handicap) |
| **Fall back when hurt** (< 35 % health → nearest covered spot or a health droid, return when > 70 %) | no | no | yes | yes, and calls a friend to cover | no |
| **Grenade** at a covered target, at a cluster of ≥ 2, at a capturer | no | the PS2's window | + covered target | + cluster + capturer | the PS2's window + cluster |
| **Flank the human** (a human who held one spot for > 8 s gets one Flank-role attacker routed to his side) | no | no | 1 bot | 2 bots | none (FLNK is post-to-post only) |
| **Target choice** | nearest | nearest | + who aims at me, the capturer | + the wounded, the gunner, the human inside the budget | nearest × pressure |
| View multiplier (× the map's, MAIS) | 1.0 | 1.0 | 1.25 | 1.5 | same |

Not used: wall-hacks (the perception assertion stays), aim at the skill cap plus anything, and a damage
multiplier. **The human damage multiplier (×1.15 Hard, ×1.3 Elite) should go** once the levers above measure
at least as hard (card DRETIRE). It is the lever the maintainer felt as "I died more" without the match getting
harder. bot-design §7 decision 2 already dropped it by default. The PS2's own level-3 lever, the head/critical
hit-location multiplier for AI attackers (`Sim::hit_scale`, Hard and Elite), is the PS2's and stays.
Superseded 2026-11-10 (playtest B1): bots get no head or critical-prim factor on ground rounds at any tier;
space rounds keep the Hard-and-up gate (`Sim::hit_scale`).

Aim assist stays on (LOCKED FEATURE SET, 2026-09-03). The PS2's Elite switched it off (`re-ai-difficulty.md`
§9.3). The port cannot use that lever, which is one more reason the bots must be better.

**Recommendation (Q2).** First, in order: fix the REACT plumbing (cooldown by tier, Tactician reacts, roll falls
back to a strafe), add the evasive step when aimed at, and make TOKN the per-human attack budget with focus at
Hard/Elite. Then fall-back-and-heal and grenades at covered targets. Then flank-the-human. Retire the human
damage multiplier last, and only after the proxy gate shows Hard and Elite at least as hard without it.

## 3. Match-level difficulty: making the round harder to win

Ranked by how fair and natural it feels (most natural first). "Strength" is measured where a number exists.

| # | lever | fair/natural | strength | notes |
|---|---|---|---|---|
| 1 | **Enemy objective play** (opposition teams at Hard/Elite): defend a post the human's team is capturing (send the nearest 2-3), counter-attack a just-lost post within 30 s, cover every capture (TEAM's Cover role, today Hard/Elite both teams), spread attackers over two posts when ahead | invisible, reads as "they're smart" | not measured; WHY reason codes can count it | The PS2's commander has no such reaction; a named deviation |
| 2 | **Asymmetric competence**: the human's teammates one step below the enemy (§4's ally offset) | the PS2 does exactly this (17 vs 11); players rarely notice teammates being a bit worse | **strong**: Elite:Normal moved allies' wins 58 % → 31 % (−50 tickets) | Needs the per-team rule for multiplayer (§4) |
| 3 | **Per-human attack budget + focus fire** (§2) | natural: "they gang up on the dangerous player" | not measured yet (needs TOKN + DPRX) | Per human, so it scales with any human count |
| 4 | **Enemy vehicle and turret use** at Hard/Elite (crew turrets near threatened posts, take vehicles toward the human's posts) | natural | not measured | Depends on vehicle AI (VTAI/TURR) |
| 5 | **Spawn advantage**: enemy respawn wave a little sooner (spawn delay 10 s → 8 s at Elite for teams without a human surplus) | mostly invisible; a bit gamey | not measured | Ticket count unchanged |
| 6 | **Bleed handicap** (today ×1.25 at Elite against a team with a human) | invisible but unearned; no effect while the human's team is winning | small | Keep as the host option it is; do not raise it |
| 7 | **Ticket handicap** (enemy +N tickets) | visible on the HUD; feels like a cheat | linear, easy to tune | Only as a host option |
| 8 | **Rubber band** (BF2's ±4 skill from the score gap, `re-ai-difficulty.md` §6) | the PS2 did it in single-player, but it punishes playing well | strong | PLAN rejected it; keep it out |
| 9 | **Damage into humans** (×1.3) | the least natural; it hits the human, not the match | — | Retire (§2) |

**Recommendation (Q3).** Build 2 (asymmetric competence, by the §4 rule) first. It is cheap: `team_difficulty`
already exists, and it is the only lever measured to move the round by a full ~50 tickets. Then 1 (objective
play at Hard/Elite for teams that have fewer humans) and 3 (budget + focus). Keep 6 as it is, never build 8, and
retire 9.

## 4. Multiplayer fairness: one rule for every mix

**Terms.** `H(t)` = humans on team t. The *human surplus* of team t is `S(t) = max(0, H(t) − H(other))`.
Humans replace bots one for one (a team's unit count stays the mission's), as PLAN's no-overspawn rule already
implies.

**Rule.**
1. **Bot competence per team.** Every bot of team t takes the host tier's row with an **ally offset** of
   `S(t)` steps: `row(t) = tier − offset(tier, S(t))`. With no surplus, the team's bots get the full row. The
   offset is a skill delta on the 1..20 scale plus the behaviour columns of the lower row. It is one table:

   | host tier | offset per surplus human | floor |
   |---|---|---|
   | Easy | 0 (new players: the human's team should win) | — |
   | Normal | −1 skill per surplus human | 14 |
   | Hard | −3 skill per surplus human (one row: Hard → Normal columns) | 11 |
   | Elite | −3 skill and Elite's extras off per surplus human; 2nd and later −2 each | 11 (the PS2's teammate value) |

   PROVISIONAL starting points. The proxy gate (§5) sets them. The one measured anchor: Elite enemies with
   Normal allies (≈ a −3 offset) took the allied side from 58 % to 31 %, and with a strong proxy to 39 %.
2. **Per-human pressure.** Attack budgets (TOKN) and the "flank the human" role are **per human**, owned by the
   *other* team's bots. Each human gets the same budget whichever side he is on and however many humans there
   are. With humans on both teams, each team's bots spend their budget on the humans they face.
3. **Team handicaps** (bleed, ticket, spawn levers of §3) apply only to a team with `S(t) > 0`, scaled by
   `S(t)`. They never apply when the human counts are equal.
4. **Objective play** (§3 lever 1) is a tier column on every team. In co-op it matters on the enemy side, and in
   versus it is symmetric.

This replaces the PS2's "all AI on the enemy table in netplay" for unequal teams. It reduces to the PS2's
behaviour when the counts are equal (both teams full row) and to the PS2's single-player asymmetry at 1v0.

Examples, host at Elite (E = full Elite row, offsets as in the table above):

| humans (A v B) | S(A), S(B) | team A's bots | team B's bots | budgets on humans |
|---|---|---|---|---|
| 1 v 0 | 1, 0 | Elite −3, extras off (≈ Hard row at 17) | E | B's bots: 1 budget (6) on the human |
| 2 v 0 (split-screen co-op) | 2, 0 | 17 − 2 = 15 | E | 2 budgets, one per human |
| 3 v 0 | 3, 0 | 13 | E | 3 budgets |
| 4 v 0 | 4, 0 | 11 (floor) | E, plus bleed ×1.25 (S = 4 caps it) | 4 budgets |
| 1 v 1 | 0, 0 | E | E | each side's bots: 1 budget on the human they face |
| 2 v 1 | 1, 0 | Elite −3 (≈ 17) | E | A's bots: 1 budget on B's human; B's bots: 2 budgets |
| 3 v 1 | 2, 0 | 15 | E | 1 / 3 budgets |
| 2 v 2 | 0, 0 | E | E | 2 / 2 |
| 0 v 0 (sweep, demo) | 0, 0 | E | E | none |

Host at Normal, 1 v 0: allies 16, enemies 17 (close to symmetric, so a new player can carry). At Easy: no offset.

Odd numbers and mid-round joins: S is recomputed when a human joins or leaves. The new skill is applied **at each
bot's next spawn**, never to a living bot, so no bot changes mid-fight. That is deterministic and hash-friendly,
the same way `skill` is set at spawn today (`spawn/runtime.rs:597`).

**Recommendation (Q4).** Adopt the surplus rule above. It is one function of `(tier, H(A), H(B))` returning two
`team_difficulty` values plus a skill delta. The arena's `team_difficulty` field already carries half of it.

## 5. Measuring "hard enough" without the maintainer playing

**The human proxy (card DPRX).** Make the scratch instrumentation a real sweep/arena option, plus one sim flag
so the proxy is *treated as a human* by the human layers: pressure, TOKN tokens, the human damage multiplier,
bleed handicap and the §4 surplus. The flag is `Unit::proxy_human` (not hashed when unset). The proxy is a bot
with fixed "competent human" stats:
- **P-good**: skill 20, damage dealt ×2.5 and taken ×0.4 (a stand-in for a human's better target choice, cover
  and health pickups), REACT Elite row, and it always fights the nearest enemy it sees. Calibrate it once against
  The maintainer: in one playtest at Normal, note his K/D and kills/round, then set P-good's multipliers so it
  matches that within 20 %.
- **P-avg**: skill 17, ×1.5 / ×0.67 (a decent player).

Each tier's run puts the proxy on **team 1 and then team 0** (the map side cancels), on 8 maps × 8 seeds (128
rounds, about 4 minutes).

**Targets per tier** (with P-good; PROVISIONAL, for the maintainer to confirm):

| tier | proxy team win rate | proxy K/D | mean margin, proxy team |
|---|---|---|---|
| Easy | ≥ 90 % | ≥ 4 | ≥ +60 |
| Normal | 70-85 % | 2.5-4 | +20..+60 |
| Hard | 50-65 % | 1.5-2.5 | −10..+30 |
| Elite | 30-45 % | 1.0-1.5 | −40..0 |
| and with P-avg at Elite | ≤ 25 % | < 1 | |

Today's measurement with the closest stand-in (a ×3/×0.33 proxy, not treated as human): Elite 64 % and K/D 2.8.
Elite fails its target by about 20 points.

**Gates.**
- Report-only first: a `difficulty` line in the loop's sweep gate, per tier: win rate, K/D and margin over 128
  rounds, plus the Wilson 90 % interval. It goes red only when a tier's interval falls entirely outside its band.
- The arena `vs all` verdicts must read `yes` for every neighbouring pair, **including elite>hard** (today a
  coin flip). Pre-registered: paired t ≥ 2 over 12 seeds.
- The focus histogram and the dodge measure (§2) become census lines (WHY's reason codes give the "why").
- Determinism: every new column is set at spawn or drawn from the sim RNG, so the existing hash gates carry it.
  The unset proxy flag leaves every hash unchanged.

**Recommendation (Q5).** Build DPRX first, record today's table as the before column, and have the maintainer
calibrate P-good once. After that, every card in §6 is judged by the same 4-minute run.

## 6. Build plan: small cards, in order

Each card is landable alone with the sweep green. "Overlap" names the other session's cards.

| # | card | what | test | overlap / order |
|---|---|---|---|---|
| 1 | **DPRX** | Sweep + arena options: `SWEEP_ERA`, `SWEEP_TEAMS`, `SWEEP_NOFILL`, `SWEEP_PROXY`, both-sides mode, focus histogram, proxy K/D line; `Unit::proxy_human` read by pressure / damage / bleed / tokens | proxy flag unset → hash unchanged; proxy line deterministic | none; first. Measures the pressure decoy hypothesis (§1.4) |
| 2 | **DBASE** | Run the 128-round table per tier (P-good, P-avg) and the maintainer's one calibration; write the before-column into this note; add the report-only gate line | numbers only | after DPRX; after `tiers2` lands (so Easy is the 14 row) |
| 3 | **DTEAM** | The §4 surplus rule: `fn team_rows(tier, humans[2]) -> [(Difficulty, skill_delta); 2]`, applied at spawn; game + split-screen + net session feed `humans` | unit tests for each §4 example row; 0 v 0 = today's hash | needs `tiers2`; independent of botup |
| 4 | **TOKN** (other session) | Hit tokens per human: Easy 1 / Normal 2 / Hard 4 / Elite off | — | **Proposal: make TOKN the per-human attack budget** (who may *engage*, not only who may *hit*), with Elite = 6 rather than off, so one object carries §2's budget and §4's per-human rule. Measured: bot-vs-bot, ≥4 shooters on one target in 3 s is ~0 %, so Hard 4 should be near-neutral against today. Normal 2 trims ~4 % of hits. Elite off keeps today. TOKN never makes Hard/Elite easier only if DPRX's proxy confirms it. |
| 5 | **DFOCUS** | Focus fire at Hard/Elite (a second/third budget holder joins within 0.5 s) | focus histogram: Hard ≥ 15 % of hits on a human from 2+ shooters, Elite ≥ 25 % | after TOKN (uses its budget) |
| 6 | **DDODGE** | REACT cooldown by tier, Tactician reacts, roll→strafe fallback, evasive step when aimed at; §2 rows | dodge measure per tier hits its §2 target; arena elite>hard `yes` | **after HYST** (wake on hit changes the reaction path); WHY codes measure it |
| 7 | **DOBJ** | Opposition objective play at Hard/Elite: defend a post under capture, counter-attack a lost post, two-post spread | proxy-team margin drops at Hard/Elite; WHY reason counts | after DTEAM; touches the commander, not botup's files |
| 8 | **DFALL** | Fall back and heal at Hard/Elite; grenade at covered target / capturer; flank-the-human role | census lines; proxy K/D drop | after DDODGE; FLNK's role code is reused |
| 9 | **DROST** | CIS droideka missing from CW rosters ("not an infantry class"): an RE check against the PS2 mix and a fix or a named deviation | the roster test; Coruscant swap test re-run | independent; map balance, not difficulty |
| 10 | **DRETIRE** | Remove the human damage multiplier (Hard/Elite) and review the bleed handicap, once DBASE shows Hard/Elite in band without them; the maintainer's call | proxy gate stays in band | last |

How to split the work with the other session: botup owns the bot's *own* behaviour files (`ai/wake.rs`, the
Tactician, TOKN). This plan's DTEAM, DOBJ and DPRX live in the sim's spawn/round code, the commander and the
examples, so the two can run in parallel. DDODGE and DFOCUS touch botup's area and should go to that session or
wait until botup is on main.

## 7. What the other session's cards change

| card | effect on this proposal |
|---|---|
| **HYST** (wake on hit/near miss/new enemy; slow think for travel only) | Changes the reaction path the dodge numbers came from. Some "hits in the REACT cooldown" may become reactions, but the cooldown itself is REACT's. **DDODGE waits for HYST** and re-measures the 7-24 %. The first-shot delay column (§2) is unchanged, as HYST keeps it. |
| **MAIS** (map AI view multiplier × tier's `view_mult`, aggressiveness, snipe suitability, capture bans) | Bot-vs-bot balance per map may shift (aggressiveness, capture bans), so **DBASE must run after MAIS** and §1.2's per-map table should be re-run. None of the 8 maps used here is Dagobah or Felucia. |
| **WHY** (reason codes) | The measuring tool for DDODGE, DOBJ and DFALL. No numbers here depend on it. |
| **TOKN** (hit tokens: Easy 1, Normal 2, Hard 4, Elite off) | §6 row 4. It must not make Hard/Elite easier than today: the focus data says Hard 4 is near-neutral bot-vs-bot, but against a human under pressure ×1.3 it may bind. DPRX's proxy-as-human measures exactly this before TOKN lands, or right after. Combined with the human damage multiplier, fewer hits × 1.3 damage is a confusing double lever, which is another reason for DRETIRE. |
| **BSTK** (stuck measure skips droid pilots) | None. |

Recommendations that already depend on them: DDODGE (HYST), DFOCUS (TOKN), DBASE (MAIS for the map table).
Recommendations that would change once they land: the per-map bias table (MAIS) and the dodge baseline (HYST).

## Appendix A: the scratch instrumentation (not committed; card DPRX makes it real)

In `round_to_outcome.rs` only:
- `SWEEP_ERA=cw` loads `missions/<key>_con_cw.ron` when it exists (else `_con.ron`).
- `SWEEP_MISSION_DIR=<dir>` reads missions from another directory. Used for the faction-swap copies: a Python
  script swapped the two `teams` entries in copies under `/tmp`.
- `SWEEP_TEAMS=<t0>:<t1>` sets `sim.team_difficulty`.
- `SWEEP_NOFILL=1` vacated slot 0 without the filler bot (16 v 16). Obsolete since 2026-10-30: the sweep has no
  filler and plays the map's own sizes; `SWEEP_COMBATANTS=16 SWEEP_MISSION_TICKETS=1` is this setup.
- `SWEEP_PROXY=<team>,<dealt>,<taken>[,<skill>]` picks the first alive AI of `<team>` after the opening wave and
  sets `buffs.offense = (1, dealt)` and `buffs.defense = (1, taken)` every tick while it is alive. Respawns reuse
  the unit index, which was checked: the generation count equals deaths + 1. It also prints `PROXY ... kills K
  deaths D` and `PROXYHITS shooters-in-3s hist [1,2,3,4,5+]`.

Arena runs used the unmodified `arena` example: `arena 3000 <seed> flat all` and `arena 3000 1 flat vs all 12`.

Caveats:
- 8 seeds per map is enough for the ALL rows (64 rounds, about ±12 % on a win rate) but thin per map.
- The proxy is not treated as a human.
- `main`'s Easy is skill 8, not the coming 14.
- BF1 Kashyyyk Docks ran 16 v 16, not its mission's 32 v 25.
