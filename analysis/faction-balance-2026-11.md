# Faction balance, November 2026 - measured 2026-11-10

Is one faction downright better than the other, and if so, why? This note gives what a faction study measured, the
two bot changes that came out of it, and the proxy gate before and after them on main.

The question (maintainer, 2026-11-10): the stage-0 baseline (`analysis/bot-baseline-2026-11.md` §2.2) found the proxy
winning 19.8 % at Hard as the Empire and 64.6 % as the Rebels on the three BF1 check maps, and 0/16 as the Republic
against 9/16 as the CIS on bf2_cor1 CW at Elite. A slight map bias is acceptable. If one faction is downright better,
the fix may change that faction's special units, weapons or vehicles (not their health), or make the bots use a kit
better. It may not cut one side's AI skill, change any unit's health, make one map special, or break the one-game rule
(an equivalent weapon or unit changes everywhere it appears).

**Short answer.**

- Only one pairing has a faction that is downright weaker: **the Empire on BF1 maps**, with the proxy and without it,
  and still when the two factions swap sides.
- **The bf2_cor1 splits are the map's side, not the faction.** With the missions' two teams swapped, the proxy on
  team 0 wins whichever faction it plays.
- Two bot changes, both about using a kit the class already carries, land here:
  1. **The BF1 pilots' mortar** (`arsenal::choose`): the bots now fire it from 10 to 32 m, where they used to take the
     pistol. This closes the Imperial pilot's kill gap and narrows the BF1 Empire's gap at Hard. It does not close it.
  2. **The droideka's Recover** (`ai::droideka::form_order`): a recovering bot droideka now rolls instead of walking
     deployed. This was a stuck loop that kept droidekas out of the posts. It removes the CIS lean in both CW pairings.
- **The BF1 Empire is still weaker after both** (section 4). What is left is in posts, not kills, and the sample does
  not show its cause. Nothing else was changed to chase it.

No unit's health, no AI skill, tier, aim, reaction or perception value, no weapon number and no map changed. Space
does not move. Awards are off in every round below.

## 1. Method

### 1.1 The proxy gate, both sides

Release `round_to_outcome`, the frozen Good proxy (`core::ai::proxy`). Normal and Hard, seeds 1-32, the proxy on
team 0 and on team 1, 16 a side on the missions' tickets, tick cap 90,000:

```
cd engine && OPENBF_BLACKBOX=0 OPENBF_MATCHLOG=0 SWEEP_THREADS=16 target/release/examples/round_to_outcome \
  --difficulty-check --strength good --seeds 32 --ticks 90000 --tiers normal,hard bf1_yav2 tat1 bf1_nab2 bf2_cor1
SWEEP_ERA=cw <same> bf2_cor1
```

Each runs twice: on the missions as baked, and with each mission's two `teams` entries swapped (`SWEEP_MISSION_DIR`
pointing at swapped copies). The swap makes each faction play each side 32 times a map. Each faction then fields its
roster from the other side's posts, spawns, team size and tickets. Caveat: a vehicle pad that names only one
faction's classes may spawn nothing for the other one (on bf1_yav2 the Empire has no AT-ST when it holds team 1).

Faction wins are read from the per-map `proxy on t0 / on t1` lines. On the missions as baked, team 0 is the Empire on
bf1_yav2 and tat1, the Rebels on bf1_nab2 and bf2_cor1 GCW, and the CIS on bf2_cor1 CW.

### 1.2 The study's other sets

The study behind the two changes also ran, on main 4e403ab4 with a diagnostic faction census added (not landed; it is
outside the hash and moved no round):

- bot-only rounds: no human, Normal, the map's own team sizes and tickets, seeds 42-49, tick cap 60,000, all 34
  conquest maps (GCW) and the 27 maps with a CW mission, on the plain and the swapped missions;
- a per-team census of each class's kills, deaths, time on foot, in a seat and in a post's capture region, each
  weapon's kills and the bots' shots, and posts captured and held.

Its numbers in sections 2 and 3 are that build's. Main has since added the vehicle weak-spot, jet-refill, lock-reach
and medal-row changes. The gate in section 4 is re-measured on current main.

**Statistics.** Wilson 95 % score intervals on every win rate; they treat rounds as independent, and rounds on one map
are not (the map decides much of the result), so a pooled interval is narrower than the truth. The bot-only pools
also give the mean of the per-map rates with a Student t interval over the maps. A census difference is starred when
it is outside the band of `scripts/bot_census.py` (2 sd sqrt(1/n_A + 1/n_B), sd the spread of the per-seed values),
that is larger than two blocks of the same code differ by re-rolling. A star does not say why.

## 2. The BF1 pilot's mortar

**What the study found.** Of the BF1 GCW classes the pilot had the largest K/D gap: the Imperial pilot 0.766 against
the Rebel pilot's 1.22 in proxy rounds (band 0.028), 0.911 against 1.27 bot-only (band 0.060). The Imperial pilot
(`imp_inf_pilotatst`, `imp_inf_pilotatat`, `imp_inf_pilottie`) carries a mortar launcher first (3 clips of 5, 15
rounds issued) and an endless pistol second. The Rebel pilot carries a pistol and a shotgun (120 rounds).

The BF1 weapon choice counts a weapon as plenty only when it was issued over 20 rounds (`analysis/re-grenades.md` 7.5,
the original's rule), and a weapon that is not plenty loses to a plenty one in range. The mortar's band is 10/32/48 m,
the pistol's 0/16/32 m, so from 10 to 32 m the bot always took the pistol and fired the mortar only past 32 m: about 2
of its 15 rounds a life. Bot-only, the Imperial pilots fired the pistol 283,523 times and the mortar 11,764 times.
Kills a shot were 0.17 for the mortar and 0.019 for the pistol. The Rebel pilot's shotgun is plenty and outscores the
pistol, so it fought with the shotgun. The CIS pilot droid carries the same BF1 kit, and the CIS was the weaker side
of BF1 CW (41.2 % bot-only). On BF2 maps the threshold is over 2 rounds, so every mortar (15-25 issued) is plenty, and
BF2's Imperial officer, who carries it, has K/D 1.12.

**The change** (`arsenal::choose`, OURS, `analysis/re-grenades.md` 7.5): a weapon whose ordnance is a shell counts as
plenty on every map.

- **One-game rule.** One rule for one launcher kind, on every map. On BF2 maps it changes nothing (every BF2 round of
  the study was byte-identical), so it brings BF1 maps to what BF2 maps already do. It moves only shell launchers: the
  Imperial and CIS pilots' mortar on BF1 maps. The BF1 smuggler's launcher fires a lobbed sticky round and is not
  touched.
- **Not a skill change.** No aim, reaction, perception or tier value, and no faction branch. The bot fires a weapon
  it already carried. A human's kit is unchanged.
- Asset-free test: `a_bf1_pilot_keeps_its_mortar_over_the_pistol_in_both_bands` (the mortar at 20 m and 40 m, the
  pistol inside 10 m, a lobbed launcher's 15 rounds still not plenty).

**What it did in the study** (BF1 GCW, Empire against Rebels):

| metric | before, bot-only | after | before, proxy rounds | after |
|---|---|---|---|---|
| team K/D, Empire minus Rebels | -0.054 * | +0.014 | -0.070 * | +0.009 |
| pilot K/D, Empire v Rebels | 0.911 v 1.27 * | 1.24 v 1.22 | 0.766 v 1.22 * | 1.06 v 1.16 * |
| mortar share of the Empire's kills | 7.2 % | 17.5 % | 4.4 % | 12.5 % |
| posts captured a round, Empire minus Rebels | -0.72 * | -0.52 * | -0.72 * | -0.60 * |
| post-minutes held a round, Empire minus Rebels | -2.40 * | -1.94 * | -4.14 * | -3.34 * |
| Empire's deaths by its own side or itself, % (Empire minus Rebels) | 1.30 (+0.15) | 1.61 (+0.54 *) | 1.25 (+0.16 *) | 1.37 (+0.34 *) |

**Friendly fire.** The mortar is a splash weapon fired from 10 m, often into a contested post, so the study's census
also counts each faction's deaths caused by its own side or by the soldier himself (one figure; the census does not
split the two). Under the change the Empire's share rose by about 0.3 points of its deaths: 1.30 to 1.61 % bot-only
(the faction difference's band 0.26 after), 1.25 to 1.37 % in proxy rounds (band 0.09). The Rebels' share did not
rise (1.15 to 1.06, 1.09 to 1.03). It is a real cost, and small against the kill gap the change closes. It is of the
same order as the grenade friend check's measured 1.19 % (`ai::arsenal`, card GRAIA), though that figure is a share of
kills, not of deaths. The CIS in BF1 CW moved
1.15 to 1.21 %, inside its band. With the droideka change too the figures are the same to the digit in BF1 GCW.

It closes the kill gap. It does not close the post gap, and posts decide rounds: over the study's 976 bot-only rounds,
winning had a Spearman rank correlation of 0.83 with a faction's lead in post-minutes held and 0.45 with its lead in
K/D. In BF1 CW it pushes the CIS's K/D past the Republic's (team +0.15, starred) while the CIS's win share stays near
even.

An alternative was measured and not taken: giving the two BF1 pilots BF2's 5 clips of the mortar (25 rounds, past the
original rule's 20). It made the same switch, overshot the pilot K/D in bot-only rounds (1.30 against 1.19, starred),
gave a human pilot extra rounds too, and narrowed the Hard gate gap less (6.3 points, inside a re-roll).

## 3. The droideka's Recover

**What the study found.** In both CW pairings the CIS stood in enemy posts less than the Republic, and the droideka
least of all: 1.30 % of its foot time in BF1, 0.90 % in BF2, against 2.3-2.9 % for the other CIS classes. A trace
of the bots' behaviour split it:

| game | class | Travel | Fight | Recover | deployed | in an enemy post |
|---|---|---:|---:|---:|---:|---:|
| BF1 | CIS droideka | 48.8 % | 17.0 % | 33.9 % | 54.2 % | 1.30 % |
| BF1 | other CIS classes | 50.8 % | 36.3 % | 0.7 % | - | 2.89 % |
| BF2 | CIS droideka | 50.7 % | 15.2 % | 33.3 % | 51.2 % | 0.90 % |
| BF2 | other CIS classes | 55.0 % | 33.9 % | 2.7 % | - | 2.31 % |

Shares of bot foot time, bot-only, the plain CW set. A droideka spent a third of its foot time in Recover, almost all
of it deployed. `form_order` deployed it in Recover. Deployed, it walks slowly, kept failing the walk's progress
window, and the failure count clears only on progress or arrival, so it stayed in Recover. The original deploys a
stuck droideka only for a short walk to a point within 10 m that ends 2 m out, then rolls again
(`analysis/re-droideka-unit.md` 5.1). The port's Recover is not that walk: it keeps the leg to the goal. So deploying
there kept the droideka in the walking form for the rest of its leg. The deploy-to-fight rule was not the cause: Fight
is a smaller share of the droideka's time than of any other class's.

**The change** (`ai::droideka::form_order`, OURS): only a fight deploys a bot droideka. It applies to every droideka on
every map; a player's droideka is not affected. Test: `the_form_order_deploys_only_to_fight`.

**What it did in the study:** droideka time in Recover 25-27 %, deployed 20-24 %, in enemy posts 2.16 % (BF1) and
1.62 % (BF2). Droideka K/D fell (1.67 to 1.44 in BF1, 1.38 to 1.14 in BF2). Bot-only CIS win share, both side
assignments: BF2 CW 105/240, 43.8 % (37.6-50.1) to 128/240, 53.3 % (47.0-59.5); BF1 CW 99/240, 41.2 % (35.2-47.6) to
116/240, 48.3 % (42.1-54.6), and with the mortar change too 123/240, 51.2 % (45.0-57.5; 42.5-60.0 over maps). Both CW
post gaps fell inside noise: BF2 CW posts captured -0.42 * to +0.03, post-minutes held -1.81 * to +0.42; BF1 CW
post-minutes held -3.39 * to -0.64. Why a rolled droideka still spends a quarter of its time in Recover was not traced.

The two changes do not interact: the mortar change moves only BF1-map pilots, this one only droidekas. Together the
study's BF1 GCW rounds equalled the mortar change's round for round, and its BF2 CW rounds this one's.

## 4. Before and after on main

Main 99c3d354 against this branch (main plus the two changes). The gate of section 1.1, seeds 1-32, missions as baked
and swapped pooled, so each faction plays each side of each map 32 times. 2,560 rounds, all finished.

| tier | pairing | faction | main | Wilson 95 % | this branch | Wilson 95 % |
|---|---|---|---:|---|---:|---|
| Normal | BF1 GCW, 3 maps | Empire | 131/192 (68.2 %) | 61.3-74.4 | 139/192 (72.4 %) | 65.7-78.2 |
| Normal | BF1 GCW, 3 maps | Rebels | 178/192 (92.7 %) | 88.1-95.6 | 174/192 (90.6 %) | 85.7-94.0 |
| Hard | BF1 GCW, 3 maps | Empire | 36/192 (18.8 %) | 13.9-24.9 | 50/192 (26.0 %) | 20.3-32.7 |
| Hard | BF1 GCW, 3 maps | Rebels | 111/192 (57.8 %) | 50.7-64.6 | 94/192 (49.0 %) | 42.0-56.0 |
| Normal | bf2_cor1 GCW | Empire | 48/64 (75.0 %) | 63.2-84.0 | 48/64 (75.0 %) | 63.2-84.0 |
| Normal | bf2_cor1 GCW | Rebels | 56/64 (87.5 %) | 77.2-93.5 | 56/64 (87.5 %) | 77.2-93.5 |
| Hard | bf2_cor1 GCW | Empire | 43/64 (67.2 %) | 55.0-77.4 | 43/64 (67.2 %) | 55.0-77.4 |
| Hard | bf2_cor1 GCW | Rebels | 43/64 (67.2 %) | 55.0-77.4 | 43/64 (67.2 %) | 55.0-77.4 |
| Normal | bf2_cor1 CW | CIS | 55/64 (85.9 %) | 75.4-92.4 | 48/64 (75.0 %) | 63.2-84.0 |
| Normal | bf2_cor1 CW | Republic | 54/64 (84.4 %) | 73.6-91.3 | 56/64 (87.5 %) | 77.2-93.5 |
| Hard | bf2_cor1 CW | CIS | 30/64 (46.9 %) | 35.2-58.9 | 31/64 (48.4 %) | 36.6-60.4 |
| Hard | bf2_cor1 CW | Republic | 43/64 (67.2 %) | 55.0-77.4 | 43/64 (67.2 %) | 55.0-77.4 |

Per map, BF1 GCW (64 rounds a cell, Empire / Rebels): Hard bf1_nab2 11/42 -> 13/32, bf1_yav2 15/40 -> 19/35, tat1
10/29 -> 18/27; Normal bf1_nab2 52/62 -> 50/59, bf1_yav2 39/58 -> 43/56, tat1 40/58 -> 46/59.

The faction gap, the better faction's rate less the other's, in points, with the 95 % margin of its change (rounds
treated as independent, so the margin is if anything too narrow):

- BF1 GCW, Hard: 39.1 -> 22.9, a change of 16.1 +- 13.0. Outside a re-roll, though not by much.
- BF1 GCW, Normal: 24.5 -> 18.2, a change of 6.3 +- 10.7. Inside a re-roll.
- bf2_cor1 CW, Hard: Republic ahead by 20.3 -> 18.8 (1.6 +- 23.7). Normal: CIS ahead by 1.6, then the Republic by
  12.5 (14.1 +- 18.2). Both inside a re-roll; 64 rounds a cell cannot tell them apart. The study's own CW measures
  of the droideka change are the bot-only ones of section 3, on 15 maps.
- bf2_cor1 GCW: every round identical (sorted `round` lines byte-equal, plain and swapped). Neither change acts there.

**The gate's own verdicts are unchanged.** On the missions as baked, the GCW gate (the three BF1 maps and bf2_cor1)
is Normal 200/256 on main and 200/256 here, Hard 126/256 and 127/256, PASS at both tiers on both. bf2_cor1 CW, report
only, is Normal 53/64 -> 58/64 (90.6 %, one round over the 77-90 % band) and Hard 35/64 -> 37/64.

The study's Elite run (on its base, plain and swapped, both changes): BF1 GCW Empire 30/192 (15.6 %, 11.2-21.4) ->
32/192 (16.7 %, 12.1-22.6), Rebels 100/192 (52.1 %, 45.0-59.0) -> 96/192 (50.0 %, 43.0-57.0). Elite did not narrow.
bf2_cor1 CW Elite: CIS 19/64 -> 27/64, Republic 17/64 -> 23/64.

**The proxy.** The gate's Good proxy always plays class 0, the rifleman, so neither change reaches the instrument the
gate reads. The Average proxy takes the bots' weighted class draw, so an Average proxy that draws a BF1 pilot now
fires the mortar from 10 to 32 m, and one that draws a droideka now rolls in Recover. This is a declared move of the
frozen proxy (`core::ai::proxy`, the freeze rule); no gate here uses the Average proxy, and its asset-free pin
`an_all_proxy_skirmish_keeps_its_hash` still holds (its fixture has neither a shell weapon nor a droideka).

**Round goldens** (`scripts/round_goldens.py`) are re-blessed on this branch. **The move still needs the maintainer's
approval before it lands**, as the pins file's rule asks; this section is the card that declares it. Main's binary passes all 12 ground pins,
so the move is this branch's. The 2 space goldens pass unchanged. The 12 BF1 ground goldens move from tick 3000 on
(BF1 maps with a mortar pilot on the Empire's side); the pins file has the new values. End hashes, old -> new:

| golden | end hash, main | end hash, this branch | result now |
|---|---|---|---|
| bf1_yav2 easy / normal / hard / elite | 848b5d92e63dacd1 / 91b900241281aff2 / c41d3ea6c7d7a77a / 41704bcdfbde2c01 | 820636fa28d6b260 / 23c992d3cd8a13ce / 42d36c943086c83a / 7503e718547c910a | team 0 / 0 / 1 / 1 |
| tat1 easy / normal / hard / elite | 3f04695dae3c0e07 / 63b8afc66c774a91 / 993dda8becec6427 / 343638cd15104d43 | fb16f2d0885fc3d0 / aaa8e37ab8b7ee2f / 5bbb9fb1d86a7674 / db3d41b0a84cb644 | team 0 / 1 / 1 / 1 |
| bf1_nab2 easy / normal / hard / elite | cb778b8477df2ea9 / d66420016d517ab2 / c9e130d7db480c17 / 35d9a0b3c9bc067a | 4d68cb5bd7779392 / 789a597bbc51e108 / 6dc2f535fcbaa561 / 89e081899caff14e | team 0 / 1 / 1 / 0 |

`game --sim-hash 100` is 5573f2b603ed95f4 on main and on this branch.
`cargo test -p core --lib original_hash_pins` passes.

## 5. What is left

**The BF1 Empire is still downright weaker at Hard and Elite.** At Hard the proxy wins 26.0 % as the Empire and 49.0 %
as the Rebels; at Elite the gap did not move. Bot-only, the mortar change moved the BF1 GCW Empire's win share only from
40.0 % to 41.7 % (35.6-48.0; 37.0-46.3 over maps), inside a re-roll. Its team K/D is now even with the Rebels', but it
still captures fewer posts (-0.52 a round, starred) and holds them for less time (-1.94 post-minutes, starred).

The census lines still apart, none tested by a change, so none shown to be the cause:

- **close-range kills in posts:** the Rebels' shotgun takes 21.9 % of their kills, the Empire's 7.9 % (the Empire's
  shotgun is only the dark trooper's). The mortar's 10 m minimum range does not reach inside a post;
- **walker riding:** the Empire's bots ride walkers 3.8 minutes a round longer, and a rider counts for no post. In the
  swapped proxy rounds the ride shares were equal and the Empire still lost, so this is at most a part of it;
- **the dark trooper in enemy posts:** 2.16 % of its foot time against 3.11 % for the Rebels' counterpart (starred).

Not levers, from the study: the proxy's own play (its K/D, accuracy and life time match for either faction), class-pick
shares (pilot 18.8 against 19.4 %), and the Imperial rifle (BF2's own weapon, 6 hits a kill against 5; the gap is small
and the same in both games).

The vehicle normalization still pending (BF2 vehicle health and guns on every map) is the same for both factions and
not a faction lever. On the study's numbers it would likely widen the BF1 Empire's gap a little: the Empire gets about
twice the Rebels' share of kills from vehicles, and BF2's anti-infantry vehicle guns are much weaker. Measure it on its
own if it is adopted.

**Not checked.** Bot-only rounds were not re-run on current main; their numbers are the study's (main 4e403ab4 with
the census build). Elite was not re-run on current main. The three gate maps' rounds are not independent, so the
margins above are approximate. Why a rolled droideka still spends a quarter of its foot time in Recover was not traced.
No live play.
