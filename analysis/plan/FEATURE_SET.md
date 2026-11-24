## LOCKED FEATURE SET (maintainer's decisions, from 2026-08-30)

Current scope is Instant Action, including flying vehicles, space battles and space maps. Items for other modes, splitscreen
and LAN are decisions for later work, not current tasks.

### Standing decisions
- **Same game on every source.** openbf's gameplay rules are canonical. The original games provide evidence, and this
  document records deliberate differences. Every build plays the same inside the playable bounds for future multiplayer.
  Source editions can change the size and appearance of assets while preserving the simulation's numbers and shapes.
  The maintainer approves public correction tables of gameplay numbers and names, preferably as instructions on the
  player's own files. Original meshes, art, audio, script bytecode and localized text stay in local builds.
- **Inputs.** One source per game, identified by content, with unknown formats refused clearly. PC BF1 plus PC BF2 can build
  without a console disc; single-game PC installs still need the other game. Existing console combinations remain supported.
  From the DLC only Kit Fisto and Asajj Ventress are taken. When installed, they are enabled by default as
  additions to Hero Assault. Where versions overlap, compare them and take the
  best, or write something better than all of them.
- **HUD.** BF2's on every map, with the heat ring around the centre reticle.
- **Shell font (2026-11-04, corrected).** BF2 Light uses the player's BF2 glyphs
  and original advances, six R8 levels (6/255) of SDF alpha erosion, and a left
  parenthesis mirrored from the right. These are deliberate presentation choices.
  The approved appearance uses PS2 BF2 glyphs without higher-resolution donors.
  Prefer an identified owned PS2 source; PC/Xbox builds can also explicitly select
  a local PS2 BF2 `CORE.LVL` for menu fonts only. Without it, thin the native glyphs
  and mirror their right parenthesis: a similar treatment using different source
  glyphs. PC/Xbox builds require no PS2 copy. Battle HUD, spawn, loading and results
  retain their existing fonts, art and metrics.
- **Graphics quality.** Normal is the only preset while feature-based tiers
  are being designed. Low Power Resolution remains a separate adaptive option.
- **Grenades.** BF1 maps keep their grenade types, throwing rules, explosions
  and sounds. The BF2 HUD's round grenade artwork stays mostly gray, with a
  light blue tint for thermals, gold for concussion grenades and violet for
  haywire grenades. The BF1 thermal glows blue in flight. The
  [ground vehicle and ordnance rulebook](../vehicle-normalization.md)
  records the shared damage and armor scales for each affected type.
- **Health.** Vehicle and unit health are separate. The
  [soldier rulebook](../gameplay-normalization.md) defines the shared infantry scale.
  The [vehicle rulebook](../vehicle-normalization.md) defines BF2 numbers for matched
  ground hulls and guns, family scaling for BF1-only hulls, and the authored differences
  in projectile motion, kits, and flyer rules. The BF1 Hailfire retains its
  BF1 homing launcher and spider cannon (maintainer, 2026-11-14); the vehicle
  rulebook records its damage conversion.
- **Snipers (2026-11-14, ours).** One rule on both games' maps: the army sniper rifle does
  280, so a head hit (x3) kills any non-hero and no body hit kills a full-health soldier of
  300 or more; the award beam rifle does 560; every army sniper rifle zooms x2 then x8, the
  beam rifle one x6 step. The Tusken sniper keeps 200 and its own zoom
  ([soldier rulebook](../gameplay-normalization.md)).
- **Deployed auto-turrets (2026-11-23, ours).** The auto-turret a BF2 soldier deploys has 30
  health instead of the disc's 50 (`com_bldg_inf_autoturret`, `game::normalize::SHARED_HEALTH`),
  so a rifle destroys it in 4 bolts instead of 7 and a pistol in 5 instead of 9. A rocket still
  takes one. Wall and ceiling turrets on space battles and other placed turrets keep their own
  health. measure: the maintainer's next LAN playtest.
- **Droidekas** sprint only as a ball, on the shared bar, and the ball jump spends that bar too.

### Movement & feel
- **(the maintainer, 2026-09-03) Aim assist is NEVER off by default, at any difficulty.**
  It is a separate user toggle, default ON, saved per profile with the
  user's other customisations. No difficulty tier may flip it.
- **(the maintainer, 2026-09-03) Universalised unit behaviour.** Where a class behaves
  differently between the two games, ONE behaviour is chosen for the whole
  blended game and applied to that class on every map, whichever disc the map
  comes from; the choice is recorded here per class as it comes up. First
  decision: **the droideka in ball form can jump on every map, as in BF2**
  (BF1's ball cannot). **CORRECTED 2026-09-08** (the maintainer's decision (a), from
  `analysis/re-droideka.md`, RE_LOG 2026-09-07): the original line named the
  wrong game — BF2's ball has the button jump and BF1's has none (its only
  upward impulse is the PostCollision mirror bounce). The universal behaviour
  is therefore BF2's, with BF2's numbers: `BallJumpHeight` 1.27 m
  (v0 = √(2·9.8·1.27) = 4.99 m/s), `EnergyCostJump` 30 of `EnergyBar` 100
  (restore 10/s, 15 idle; since 2026-11-02 the port takes that share from the
  shield's bar instead, `analysis/re-droideka-unit.md` §9), grounded for < 0.1 s only, no double jump, a
  +25 % forward boost; a BF1-sourced droideka has no `BallJumpHeight`, so it
  takes 1.27 m as a PROVISIONAL default. The BUILD stays HELD (P7-class, the
  vehicle/entity work). The RE for each such difference cites both ELFs.
- **(the maintainer, 2026-10-27) One gameplay, both games.** The blanket form of the rule above:
  gameplay is the same on every map of both games — weapon damage, class health and
  stamina, jumping, rolling/dodging, blast push. (Grenades are the exception: see Standing decisions.) Where the discs differ,
  one number wins (the better game, per the 2026-10-25 rule) and is recorded next to both
  games' values. Sounds, voices, rosters and per-class ammo counts may still differ.
  Grenades throw on the press, as on the PS2 (the hold-to-charge throw is to be checked
  against the discs). Blast push is one value per explosion type, toned down, falling off
  with distance, only inside a critical radius.
- Stances: one contextual button — **moving = roll** (both games had roll; BF1's was
  harder), **stationary = stance cycle** stand → crouch → prone/army-crawl (BF1 prone
  returns; BF2 dropped it). `RE:` BF1 stance machine + both roll implementations.
  **RE DONE 2026-08-30** (analysis/re-stance-roll.md): BF2 already implements the
  contextual button (crouch btn: moving=roll / stationary=cycle); its prone action is
  a stub but engine support survives. DECISION: adopt BF2's roll predicate (stick≥0.9,
  not straight-back) + BF2 impulse roll (all-direction, RollSpeedFactor 1.7×MaxSpeed,
  10 energy, exit to stand) + BF1's prone restored in the cycle; BF1's sideways
  strafe-ramp roll documented for reference, not shipped.
- Sprint: BF2 system, **limited stamina**. Infinite stamina exists only as a cheat.
- Jet/jump-pack: **BF2 feel**; infinite fuel via cheat.
- Aim assist **on foot**: **BF1 passive magnetism only** — no BF2 square-button X lock-on.
  Intensity slider. `RE:` BF1 magnetism math + constants (top priority). Vehicles, flyers
  and space aim differently (§Vehicles & space).
- **(The maintainer, LAN playtest 2026-11-14, issue #164) Controller assist reach.** OURS,
  PROVISIONAL. A sniper rifle's assist (only weapons in the sniper table) is full to its
  MaxRange (`WeaponClass::assist_full_range`, 128 m), then its slowdown, follow and pull fade
  together to nothing at the round's own reach (300 m bolt, 500 m award beam). The PS2's
  LockOnRange fade on the pull is not applied to these weapons, so the fades do not stack.
  A sniper rifle's magnet never takes a vehicle or an aircraft (HealthType Vehicle), on a
  pad or a mouse, for every player, whatever its class authors (the maintainer, fourth
  playtest 2026-11-21); the reticle still names and colours them. The discs' snipers author
  `LockOnRange 0` and `TargetVehicle 0`, so the originals assist nothing with them; the
  soldier-only magnet is the addition. Mounts that author HealthType Animal or Droid
  (tauntaun, kaadu, droideka) keep it. Bots aim on their own path and are unchanged.
  A beam is led by its range per tick, the speed its sweep is judged at. A human on a pad
  in a building turret gets a turret assist out to the round's reach (full to three
  quarters of it), with a damped pull that follows the lead. BF1's PS2 building guns
  author no magnet, so this is a deliberate addition. A zoomed building turret turns at
  its rates over the zoom divisor, as a soldier does. Bots and the mouse are unchanged.
  measure: controller play at long range on a large map, scoped and in turrets
  (`analysis/re-aim-assist.md` §10.10).
- **(The maintainer, 2026-11-19 and 2026-11-20 evening, issue #182) Leaks and the rescue.**
  OURS, PROVISIONAL; the PS2 has no such rules. Every map of both games is to be leak-free.
  Real mesh holes are closed per site with added collision (vertices, ground, wall or
  blocker solids are allowed). The wall clamp (B2) stops a soldier passing through a face,
  sloped faces included. A soldier with no headroom to stand stays crouched. A unit found
  inside a solid, shut in a closed mesh or under the terrain for 3 checks of 8 ticks, or
  held still off the ground for 8 checks (falling, a jump hung in the air, or on a slope
  in a walking stance while off the ground 4 ticks in 8), is put back on the last spot he
  stood clear. With no such spot he dies, 1 ticket from his own team like a fall
  (`DeathCause::Embedded`; an attacker's push credits the attacker); the slope case never
  kills, it only puts back. The known-leak list is private, kept beside
  the gate's baselines, and shrinks as sites are fixed; the public tree holds only its
  header. Code: `core/src/sim/rescue.rs`, the census `game/examples/leak_census.rs`.
  measure: the leak census's site and rescue counts per map, and tick cost per map
  (accepted +2-24 %, on the optimisation list).

### Combat
- SBD weapons (2026-10-27, issue #27): BF2's wrist blaster, tri-shot and wrist
  rockets, including slot ammunition, damage, spread and timing, on both map sets.
  Both discs' values and the source tests are recorded in `analysis/sbd-weapons.md`.
  SBD sounds follow the map's source game: BF1 on BF1 maps, BF2 on BF2 maps.
  This presentation choice does not change the shared gameplay numbers.
- **Bolt reach (2026-11-20, ours).** Infantry, vehicle and turret blaster bolts fly 200 m on
  both games' maps. This covers the bolt and bullet rounds of rifles, pistols, carbines,
  repeaters, bowcasters, wrist blasters and launchers, hero blasters, award weapons
  and the probe droid; the deployed auto and defense turrets (80 m); and the vehicle, building
  and map-turret guns under 200 m: BF1's AT-ST chin gun (105 m) and building gun turret
  (120 m), and BF2's snail tank cannon (195 m) (`game::normalize::lengthened_bolt`,
  `core::weapon::BOLT_REACH`). The speed stays as authored and the `LifeSpan` is lengthened.
  Before the change, BF2's rifle reached 128 m (160 m/s x 0.8 s) and BF1's 120 m (80 m/s x
  1.5 s). The remote droids' blasters keep the disc's reach (BF1 60 m, BF2 128 m; the
  maintainer, 2026-11-20), and so do the shotguns (BF1 120 m; BF2 200 m, where each pellet's
  damage falls from 65 to 15 over its 0.1 s flight; the maintainer, 2026-11-22). Rounds that already reach 200 m keep their own range: snipers,
  chainguns, the award pistols and every other vehicle gun; starfighter and
  airspeeder guns are left out (all reach 200 m or more already). Bots still open fire only
  within the disc's reach and the class `MaxRange` (`WeaponClass::engage_range`), on foot and
  in seats. A bot on foot that is hit or near-missed by an enemy beyond that range, but within
  200 m, searches for him. After the search it may return fire at him out to 200 m for 5 s
  from the hit (`perception::RETURN_FIRE_TICKS`); a seated bot does not. The Good proxy's hand
  plays by a player's rule and fires out to 200 m unprompted. A human's aim assist with a
  lengthened blaster follows the sniper's shape (`WeaponClass::give_bolt_assist`): full
  strength out to the disc's reach (112.5-128 m; 80 m for the deployed turrets, 105 m for the
  AT-ST chin gun), then a straight fade to nothing at 200 m; the acquisition, the magnet and
  the reticle's in-range cue reach 200 m. The disc's `LockOnRange` (40 m on BF2's rifles and
  pistols, 64 m on BF1's, 80 m on the repeaters) is kept for the PS2's fade of the pull, so
  within it the assist is as before and the pull still ends there; past it the slowdown and the
  tracking carry on along the curve. A building turret gunner's assist runs full to the gun's
  disc reach and fades to the lengthened one (`sim::seat_assist`). A gun whose disc magnet
  already reaches past 200 m (the snail tank's `LockOnRange` 1500) keeps it. Snipers are
  unchanged: their 300 m bolt (2000 m/s x 0.15 s) carries an assist at full strength to their
  128 m `MaxRange`, fading to nothing at 300 m (BF2's award beam: 500 m). The bots' aim is
  untouched. Measured 2026-11-20 against the pre-change branch on the same fresh seeds (awards
  off). Bots-only Hard, 61 Conquest entries x 32 seeds: team 0 won 49.8 % before and 51.5 %
  after; 19 and 21 of 61 entries were outside 40-60 %, about what chance gives at 32 seeds;
  per-entry differences were within noise (sum of z² 56.2 on 61 degrees of freedom). Good proxy
  against Elite bots on Coruscant, Hoth, Geonosis and Yavin (12 entries, 32 seeds per side): the
  proxy's side won 44.0 % before and 43.0 % after (z² 26.5 on 24). The bot census
  (`BOT_CENSUS`: note-hit, note-near, answer-fire) shows return fire in every entry: on
  bots-only Hard a round notes about 78 shooters from hits and 530 from near misses, and the
  trigger is held at a noted shooter for about 1000 bot-ticks. The vehicle and turret guns, measured
  2026-11-20 against the infantry-only branch on the same seeds (bots-only Hard, 6
  vehicle-heavy entries x 32 seeds: bf2_hot1 gcw, bf2_kas2 both eras, bf1_hot1 gcw, bf2_geo1
  cw, bf1_rhn2 gcw): team 0 won 88/192 (45.8 %, Wilson 38.9-52.9) before and 91/192 (47.4 %,
  40.5-54.4) after, no entry moving by more than 2 of 32; kills by seated bots 9674 and 9614,
  vehicles destroyed 7640 and 7617 (`VEHICLE_CENSUS`).
- **Weapon balance.** Keep the current openbf values. A future rebalance would record each weapon's
  change and retain the original-game measurements for comparison and rollback.
- Awards & medals: **KEEP (BF2 system)** — earn permanent unlocks through
  play. Career award bonuses can be disabled while earning and stats
  continue. At 32 career medals in a category, its requirement drops
  to 75% on every match difficulty. The session award cheats remain
  available.
- Classes: **hybrid unlock**. In-battle: BF2-style score unlock for special classes, every
  battle. Meta: cumulative usage/feats (elite-rifle-style graduated criteria) lower the
  in-battle threshold over time → eventually permanently unlocked.

### Heroes
- **Playable heroes (BF2) AND AI-controlled heroes (BF1 presence), both on all content.**
- **BF1 Conquest rosters.** Five approved map-and-era matchups add variety
  with existing BF2 kits. The [assignment record](../re-bf1-conquest-heroes.md#bf1-map-specific-matchups)
  lists those choices and the original rosters.
- Heroes are **killable, always** — no BF1 invincibility ("you see a hero and you just run
  away — no fun").
- Hero lock ([ours] 2026-11-19): from the Yes on the hero offer until the hero dies, the
  hero is the owner's only class; a post class change shows "Heroes can't change class".
  Pause-menu Respawn (1 ticket, a death) is the way out. The disc lets an owner keep the
  hero through lives in other classes (`RE:` hero spawn/possession flow).
  Death during the offer ([ours] 2026-11-22): as on the disc, the offered player who dies
  before answering owns the hero, and his class screen lists every class with the hero row
  chosen. The lock starts only when he spawns as the hero; spawning in a normal class
  declines the offer (as a No would), where the disc kept him the owner for later lives.
- Health (maintainer, 2026-10-30 — supersedes the September no-time-drain rule):
  regular BF2 conquest uses **BF2's standard hero health pool and saber-shaped health bar**.
  Health shrinks **with time and damage**, and **regenerates for killing enemies**. Use the
  disc's authored health and verified drain/kill-heal values. Keep the rate and gains as
  data so the maintainer can change them later; zero drain remains a valid option. This
  decision does not set Hero Assault's rules. `RE:` hero spawn/possession flow (BF2),
  NPC hero behavior (BF1), and the disc's hero health and kill-heal values.
- Push flinch (maintainer, 2026-11-05): a saber or Force push that neither throws nor
  tumbles locks input for **1/3 s on the ground and 1/2 s in the air**, shorter than
  BF2's 0.5 s and 1.0 s (`core::soldier::BF2_FLINCH_*`).
- Fall-death kill credit (maintainer, 2026-11-14, #173): a death from landing damage, the
  kill floor, a death region or a landing under the map is credited to the attacker who
  last choked, Force-pushed or -pulled, saber- or emitter-shoved or blast-threw the
  victim, as a kill (`Unit::forced_by`, `Sim::kill_environment`). A move that leaves him
  standing records nothing; a friend counts only where friendly fire would have hurt the
  victim (never a bot's throw of its own team's human), as a team kill. The credit lapses
  once he has been back in control (on his feet, or flying his own jets) for 0.5 s, or
  10 s after the move, or when he boards a vehicle (`FORCED_GRACE`, `FORCED_MAX`, [ours]
  PROVISIONAL, measure: plans). Water, lava and damage regions stay world deaths; his own
  blast never credits. The originals credit no fall (a named deviation).
- AI saber pursuit ([ours], issue #189, recorded 2026-11-22): a bot holding a saber
  closes on any target it sees. The PS2 ends its attack beyond the weapon's range plus
  10 m, about 14 m for a saber (`re-bf2-hero-flow.md` §6). Capping the pursuit at 14 m
  lowered conquest hero kills per life by about a third over 8 seeds. The block cycle
  and the turn through `InputLock` follow the PS2. measure: an original-game hero
  bot's pursuit distance before tightening this.

### AI

Current port corrections (2026-11-04; issue #108): Hard/Elite health-droid seekers
finish their graph route at the actual service sphere; disconnected authored bot
spawns may use a connected node at the same selected command post; the stricter
last-reinforcement stall watch follows the bot's own team; physically cancelled
infantry movement can qualify for the existing headroom-checked recovery jump.
These deliberate openbf rules retain their individual eligibility checks and
measurement obligations. They do not establish native-game AI equivalence or
resolve the remaining Yavin, Geonosis and Bespin route traps. Source and retained
witnesses: `analysis/re-round-stalls.md`, current checkpoint below its header.

- Infantry AI base: **RESOLVED 2026-09-01 (the maintainer, Fable session 8): openbf's OWN
  AI** — modern architecture (authored-graph navigation + local steering, a
  per-team utility commander, behaviour tree, perception, a skill vector, squad
  commands) fed by the discs' `plan` graphs, `Hint` nodes, `BARR` barriers, post
  values and tuning knobs; neither original's decision code is ported. Four
  difficulties (Easy/Normal/Hard/Elite) on a three-layer model: symmetric
  competence, per-HUMAN pressure, team handicap only against bot-only teams.
  Named deviations from that line (all [ours] PROVISIONAL): card DTEAM's weaker
  bots for a team with more humans (2026-10-28), and the #171 proposals awaiting
  The maintainer (2026-11-14): Hard/Elite allies keep the host row with a skill
  cut, LAN Hard/Elite rounds give both teams the host row, Hard/Elite bot snipers
  Normal's scope reach, Elite bot snipers Normal's dispersion floor, a focusing
  bot yields to a much nearer enemy bot
  (`engine/artifacts/loop/FEEL_NOTES.md`, the #171 entry).
  Squad commands: follow / hold / disperse / clear, ≤ 6 bots within 20 m of the
  player. Design, evidence and phases: Log "AI decision". Space AI (P8) will be
  the same architecture.
- **(the maintainer, fourth playtest 2026-11-21/22) Round-start entrance.** At round start
  each bot enters after its own seeded delay of 0-10 s (one draw of the sim's LCG per bot,
  in unit order, so every LAN peer sees the same entrance) instead of the whole opening army
  appearing on one tick, as both discs do (BF2 `FUN_002e7240`). A delayed bot comes in through
  the ordinary respawn: its post by `AISpawnWeight`, its class by the class weights, no ticket
  spent. Humans and proxies spawn as before; after the window spawning is unchanged. Campaign
  rounds (whose opening wave waits for the player's first spawn) are not staggered.
  OURS, PROVISIONAL (`core::sim::spawn::OPENING_STAGGER_TICKS`, `Sim::stagger_opening_ai`);
  measure: the opening against a playtest.
- **(the maintainer, fourth playtest 2026-11-21/22) Elite bots split at the opening.** The
  Elite tier's `hunts` column (attack the nearest post not held, never hold) applies to an
  ordinary bot only when the commander has not already drawn it a post to take: a bot keeps its
  own draw (the discs' rule, one post by its authored value per bot), so an army out of one base
  no longer all heads for the one nearest post (BF2 Coruscant's archives). A Defend plan still
  becomes the nearest post not held; a proxy keeps the nearest-post rule outright
  (`core::ai::proxy::objective`). OURS, PROVISIONAL; measure: Elite parity on the next screen.
- **(the maintainer, 2026-11-22, fourth playtest) Adaptive difficulty.** [ours] PROVISIONAL, no counterpart
  in either original. At every tier, when the humans (players and stand-ins) are all on one side, that side's
  enemy bots drift up to 10 % stronger or weaker than the tier's base: their aim error radius and their reaction
  to a new contact scale by up to ±10 %. The drift follows the humans' standing against them, read every 10 s:
  each side's remaining tickets as a share of its start (Space Assault, CTF, Hero Assault, Hunt and XL: points
  as a share of the side's target), the humans' share minus the bots'. A lead within ±10 points (a 45/55 split)
  is a dead band; past it the target rises linearly to the full 10 % at a 25/75 split, humans ahead making the
  bots harder and humans behind making them easier, and each update closes a third of the gap. A side that cannot
  lose tickets (campaign rules, unlimited reinforcements) reads as even. Off (factor exactly 1.0, not hashed) in
  bots-only rounds and with humans on both sides, so bots-only parity sweeps and pins keep their numbers; the
  humans' own bots and every stand-in never drift. Integer per-mille in core, so every LAN peer derives the same
  factor. Built: `core/src/sim/adaptive.rs` (`Sim::adaptive_factor`, logged by the session-stats build);
  `round_to_outcome`'s `SWEEP_ADAPTIVE=0` turns it off for a sweep.
  measure: the session-stats factor timelines against the humans' win rates per tier before any retune.
- **(the maintainer, 2026-10-01) AI class weights.** An AI spawn (and respawn) draws its class by weight: the team's alive-AI cap
  is shared out over its classes — the default class (the mission's first: the rifleman) 4 parts, a standard class 2, a
  special class 1 (the table's entries from the fifth on: BF2's officer and special slots, BF1's fifth class — the
  officers, wookiees, droidekas, dark and jet troopers) — each living bot of a class lowers that class's weight, and every
  class keeps the same small floor, so all-riflemen or all-droidekas is possible and rare. On a BF2 team of 16 that is
  about 5 riflemen, 3 of each standard class and 1 of each special one. The human always picks their own class. Built
  023e3fd (the spread) and AICLW (the shares; `Sim::ai_class_weights`, `core/src/sim/spawn.rs`). The 4 : 2 : 1 is ours
  and PROVISIONAL; the PS2's own rule is untraced (card AICLS) and is not the target.
- **(the maintainer, 2026-10-01) Bots hold ground and fight, and not as statues.** A bot that reaches a hub with an enemy in
  sight takes a position there (an authored Cover or Snipe hint if one is near, else a point in the hub; never in water),
  holds it for a dwell and fights from it, then advances past a hub before it may hold again; a defender also takes an
  authored position when the enemy is merely near. On its position it shuffles to another spot now and then, strafes on
  some bursts, rolls instead of side-stepping on some hits, and runs clear of a grenade — each by a percent chance per
  opportunity that rises with the difficulty (Easy 15 / 10 / 20 / 30 %, Normal 35 / 30 / 35 / 60 %, Hard 55 / 50 / 50 /
  85 %, Elite 70 / 65 / 60 / 95 %; `core::skill::Tier`), so an Easy bot mostly stands there to be shot. In the endgame
  nobody waits in a position. Built: HOLD (`core/src/ai/brain.rs`). The rule and every number are ours and
  PROVISIONAL; **the tier chances are the maintainer's to tune.** Measured on BF1 KAS2 against the PS2: standing still 20.5 % → 24.3 %
  (PS2 55 %), in the lagoon 42.7 % → 41.1 % (PS2 22 %) — the targets are NOT met; holding more stalls BF2 Kamino, where the port kills 3–5 a minute
  against the PS2's 17.6 — the kill rate is the next lever. Grenades are not thrown yet (card GREN): the reaction is
  built and tested against a grenade put on the ground by hand.
- **(the maintainer, 2026-10-01) AI bounds and the way back.** Bots do not cross the edges of the playable level on their own:
  a disc `BARR` barrier with a hull, a drop or nothing behind it is a wall for the steering (BF2 Kamino's rim rails and
  bridge sides are ringed with them; the PS2's walk step never crosses one). A bot that ends up off the level anyway
  walks back — to the nearest hub it can stand level with, jumping a low wall with a run-up — and after 45 s with no
  way back it is given up and respawns. Under the map (a hard landing beneath the floor it fell from) and below the
  kill floor every soldier dies, the invincibility cheat included. Built: KFLR 5b12384, BNDS (`core/src/sim/bounds.rs`,
  `core/src/ai`). "On the level" is ours and PROVISIONAL: the hubs' floors walked from the posts, the barriers, the
  collision.
- **Unit counts (the maintainer, 2026-10-29):** Standard Conquest defaults to each map/era's
  16–32 combatants per main team, preserving its side ratio; a human occupies one
  combatant slot. Team size and Reinforcements are percentages of that map's
  defaults (100%), with resolved counts and an equal-map average shown. The opening
  roster and respawn target agree; the menu's ceiling is 32 per side, subject to
  frame-rate validation. Calibrate each map's reinforcements independently for a
  typical Normal round with the Good human stand-in to last **6–9 minutes, aim 7**.
  Difficulty tiers were tuned at 16v16; this decision does not retune them, and
  `round_to_outcome --difficulty-check` still plays 16 per side on the missions' tickets.
  Historical reference (**RE DONE 2026-08-30**, re-tickets-spawn.md): BF2's "16"
  is its default alive-AI cap (NumBots, DAT_006004fc=16) over SetUnitCount=20 member
  pools; BF1 PS2 runs 25–32/team. Generated missions retain those original values;
  the approved Instant Action defaults are a named deviation.
- **Bots do not fire into walls (the maintainer, fourth playtest, 2026-11-22; issue #189).** A bot on foot
  holds its trigger on every tick its line from the muzzle (a droideka's gun) to the enemy soldier's centre or
  head is blocked, and on that tick it does not stand to fight: it walks on, still aiming, until the line opens
  or its next think drops the target. Perception looks once a think and also takes an enemy bot's own sighting,
  so before this about 40 % of the bots' pulls on bf2_cor1 / bf2_tan1 / bf2_tat3 went into a wall. BF2's
  trigger tests no line: it holds only for 2 s after a bolt struck short while the bot stands within 1 m
  (`FUN_002b8028`); BF1's rule is not traced. Not a proxy's hand, a thrown or lobbed round, or a droid,
  turret or station target. Ours: `core::ai::gunner_foot::line_walled`. measure: `SWEEP_WALLS` blocked pulls per round, before and
  after; replace: if the originals' trigger is shown to test the line.

### Vehicles & space
- Ground vehicle handling: **BF2** (verify BF1/BF2 similarity during RE; revisit if they
  diverge more than expected).
- Flyers: **faithful per source map** — BF1 maps keep their flyable craft, BF2 maps stay
  as shipped.
- Aiming in vehicles, flyers and space (the maintainer, 2026-09-23 — **likely, not yet locked**: "will
  probably need"): keep **BF2's lock-on-target mechanic** there, while soldiers on foot keep
  BF1 magnetism. Settle it when P7/P8's RE reaches it. `RE:` what BF2's vehicle/space
  lock-on actually is on the disc (weapon `LockOn*` properties, reticle/targeting code) and
  whether it shares anything with the on-foot X-lock; BF1 aim assist against vehicles
  (`re-aim-assist.md` covers soldiers only) is the comparison.
- Space Assault: **KEEP but needs rework** — least-loved mode; enhancement design DEFERRED.

### Modes & rules
- **XL:** Single-player Instant Action on BF2 Geonosis (Clone Wars), Hoth
  (Galactic Civil War), and Kashyyyk (both eras). Command posts are fixed spawn
  locations. There is no capture meter, bleed, or domination. Each team starts
  at `0/350`, an enemy kill earns a point, and 350 wins. Respawns are unlimited.
  All four PC `mission.lvl` scripts configure `ObjectiveTDM:New` with 350
  (`geo1c_xl` pc 28, `hot1g_xl` pc 13, `kas2c_xl` and `kas2g_xl` pc 15).
  `objectivetdm` starts both scores at zero and enables their display (start
  prototype pc 18–38). Its death callback awards points and checks the XL
  target (pc 20, 30, 46–56). These are zero-based bytecode instruction indices.
  Team sizes (maintainer, 2026-11-20): 100% Team Size is the PC game's own XL
  size per side, read from each XL script's `SetupTeams`: Geonosis 80, Hoth
  120, Kashyyyk 160 (both eras), both teams equal. Every XL script also sets
  `SetSpawnDelay(10, 0.25)`, which is already the spawn-wave default. Team
  Size scales these with no ceiling up to 200% (160 / 240 / 320 per side).
  OURS: every Conquest post stays an XL spawn (the PC scripts remove some
  posts; the maintainer kept them all). Players change class at their own
  posts as in Hunt; posts capture nothing. LAN refuses XL rounds. Sim tick
  cost at these sizes (2026-11-21, headless, 12 rounds at 100%): mean 7.5 /
  8.1 / 11.4 ms, p99 18.8 / 19.8 / 24.6 ms on Geonosis / Hoth / Kashyyyk,
  inside the 33 ms tick. Optimising for these sizes is planned work.
  Headless check on 2026-11-20 (the PR branch, base `4250d394`).
  `cargo test -p core --lib sim::deathmatch::tests` passed 13 tests, including
  the winning kill at 350. `cargo test -p ui --lib team_points_tests` passed
  two HUD tests. `cargo test -p game --bin game xl_launches_all_supported_eras_from_instant_action -- --nocapture`
  checked score and capture HUD state, spawns and fixed ownership for 120
  ticks in each of eight cases. The release package loaded all four reused
  maps and missions in headless traces. Full rounds remain for playtesting.
- Conquest: **BF2 rules + BF1 no-overspawn invariant**: live units on a team may never
  exceed remaining reinforcements — when a player respawns into the last slots, excess AI
  despawn (e.g., 3 tickets left, 2 players + 1 AI alive; player dies & respawns → the AI
  teammate despawns). Preserves comeback tension. `RE:` BF2 ticket/bleed + BF1 spawn
  reserve logic.
  **BF1 command-walker exception (the maintainer, 2026-10-22; B1WP):** on BF1 Geonosis and Hoth,
  a live command walker is an uncapturable held post, contributing ValueBleed 10 to
  the bleed/countdown sum; a destroyed walker contributes nothing until it respawns.
  Verify the maintainer's recalled lifecycle against the PS2: original-pad respawn, crew reboarding
  and advance into enemy space (Geonosis) or attack on the shield generator (Hoth);
  the generator is an uncapturable post until destroyed, then removed. Build the measured
  PS2 behaviour and report any difference from the recollection in STATUS. This is the
  approved scope, not evidence that the lifecycle has already been implemented or verified.
- **Every death costs one ticket (the maintainer, 2026-11-19; issue #171).** A death costs
  its own team exactly one ticket on every map and in every ticket mode. Deliberate
  difference: BF2 Death Star: Interior conquest (both eras) has its script charge Lua
  team 1 (Rebels, Republic) a second ticket for each of its own deaths
  (`OnCharacterDeathTeam` → `AddReinforcements(1, -1)` in `ScriptPostLoad`). openbf
  drops that rule. The mission generator no longer extracts it, and the loader ignores
  the `death_drains` field that older generated missions still carry. Modes scored by
  flags or points (CTF, Hunt, Space Assault) keep their unlimited tickets, and their
  points are scores, not tickets. Locals (team 2 and up) still pay none.
  Starting tickets may differ per side for faction balance. No side may start with more
  than 60 % of the pair's total.
- **No weighted scoring outside Space Assault (the maintainer, 2026-11-19; Space Assault
  excepted 2026-11-20).** Every enemy kill scores its team exactly one point in Hunt, CTF,
  Hero Assault and the other point modes, whatever the victim (bot, human, hero) and
  wherever it dies. Self-kills, team kills and deaths with no credited killer score 0
  for bots and humans alike; nothing scores below zero. Deliberate differences from the
  original games: Hunt ignores the scripts' `pointsPerKillATT/DEF` (BF2 Geonosis 1:2,
  Hoth and Naboo 1:3, Kashyyyk 2:1) and openbf's own BF1 Hunt kill values. Every Hunt
  keeps the disc's unit health, and openbf never rebalances a side by health or damage.
  Hunt is for fun, so it fields big sides (OURS, the maintainer 2026-11-20 late): each
  mission sets how many of each side are alive at once per difficulty
  (`hunt_alive_caps`, at least 8 a side, a human counting toward his own) and each
  side's total units (`hunt_units`), both free to go past the disc's counts. Soldiers
  spawning on a node another soldier stands on step out to a free spot around it, and
  a soldier whose spawn node has a live enemy within 20 m takes another node of his
  post clear of enemies (BF2 Endor and BF2 Mos Eisley spawn both sides along one path).
  Balance, in order: big sides for both; per-side points to win per difficulty
  (`hunt_points_to_win`, neither side needing more than 60 % of the pair's total); more
  alive at once for the side that loses too often (up to 16 extra, so at most 48 a side;
  the sim tick at 48 a side measured 1.2-2.0 ms on average, p99 3-5.6 ms, against a
  33 ms budget); then Hunt-only easier bots for a side (`hunt_skill_pct`, 30-100 %, the
  maintainer 2026-11-21). An eased bot keeps its skill level and mainly misses more: its
  aim point lands (100 / percent) squared times farther off (2x at 71 %, 4x at 50 %,
  about 11x at 30 %). It reacts a little slower, never past 1 s of clear sight, and
  fires weapons that need no aim (the arc casters, the incinerator, the flamethrowers)
  at that percent of their rate. Humans and the human stand-in are never eased. No
  other mode reads these fields.
  Fitted values (2026-11-21, refitted with the reworked easing; bots only, every award
  off, the 5-minute Hunt timer): every Hunt fields 32 alive a side on every difficulty
  from a pool of 64; no row uses extra alive (8 or 16 more for the losing side gave no
  consistent change in the kill split, since each extra soldier is also one more
  target). Points to win run about 43-265 a side, so a median round lasts about
  3.3-4 minutes with 27-110 kills a minute for both sides together. Easing falls on the
  army side (30-75 %; most BF1 armies and BF2 Naboo at or near the 30 % floor) and on
  the Jawas on both Mos Eisley maps (30-80 % on Easy to Hard); the original tiers stay
  as they are on the other side. Judged on fresh seeds only (24 rounds a row, 40 where
  the share landed within 5 points of 40/60), 44 of the 56 map and difficulty rows land
  inside 40-60 % for team 0. Outside, at the 30 % floor and the 60/40 limit, where the
  army still kills more than 1.5 times as fast: BF1 Geonosis Elite, BF1 Kashyyyk Islands
  Elite, BF1 Kashyyyk Docks Hard and Elite, BF2 Hoth Easy, BF2 Kashyyyk Elite and all
  four BF2 Naboo rows (the army wins 96-100 %). Also outside after two target fits:
  BF1 Mos Eisley Normal (0.33) and BF2 Geonosis Hard (0.29). In Hunt a sticky throw (Ewok
  spear, Gungan booma, thermal detonator) does
  not take the one-thrower-per-target claim, so a group of natives can all throw at
  the same soldier.
  The Hunt score option defaults to "Per map", which plays those targets (50 each where a
  mission sets none); any other value N keeps each map's split, scaled so the two targets
  average N. At the timer the side further along toward its own target (the larger
  share of it) wins (confirmed by the maintainer, 2026-11-20). The
  battle HUD shows each side's points against its own target.
  CTF (one point per capture) and Hero Assault (one per kill) were already even.
  Personal score (scoreboard, hero and class unlocks, medals, career) is unchanged.
- **Space Assault keeps the original BF2 scoring (the maintainer, 2026-11-20).** 180
  points to win; an enemy bot killed 1, a human 2, a hero 5, a kill near a post 2; a
  manned enemy fighter destroyed 3 (plus its riders' kills), an empty flyer or empty
  armed building 1; a human's self-kill -1 and team kill -2 (bots never lose points);
  each critical ship system its authored objective points (18 per system group). The
  shell's Assault score option (shared with Hero Assault, default 180) is Space
  Assault's points to win as shown. This is the rule set of LightVelox's Space Assault
  work (PR #183); the overnight one-point-per-kill trial with a 50-point limit was
  reverted.
- **One Jawa for both games (the maintainer, 2026-11-20; OURS).** Every Jawa, on BF1 and
  BF2 maps and in every mode, carries the fusion cutter and BF2's arc caster. BF1's
  Jawa, a repair-only native on its disc, takes BF2's arc caster, the same weapon on
  every map. In Hunt the Jawas are one of the two sides and the arc caster is at full
  strength. In every other mode they are a neutral native team that attacks both armies
  and both armies can shoot (on both discs they are friendly to both); they still spawn
  where the map places them and take no command post, as before. Outside Hunt their arc
  caster does 60 % of its damage: a deliberate, named exception to "no per-side damage
  changes", chosen so the Mos Eisley Jawas are a real but smaller threat than the Dune
  Sea's Tuskens. Killing a Jawa costs no side a ticket or a point and is nobody's team
  kill; it scores the killer's personal score like any enemy kill. A soldier a Jawa
  kills costs his side one ticket, like any death. Any native's death scores no team
  point in the point modes. With the Jawas fighting, both Mos Eisley maps were refitted
  for even sides: on BF1's map five of the Jawas' fourteen spawn spots, the ones crowding
  one army's posts, are no longer used; BF2's Clone Wars map gives the CIS's exposed post
  the same extra respawn weight as the Galactic Civil War map; and the starting tickets
  of three rows moved (all within the 60 % cap). Both Mos Eisley Hunts field 32 a side
  and are evened by points to win and by easing the Jawas (their arc caster fires less
  often when eased; see the Hunt entry above).
- CTF: **KEEP both variants, REWORK later** — the maintainer barely played it (childhood reason: the
  clone commander says "damn" when the flag drops — optional VO filter noted); needs a
  fun pass before it matters.
- Hunt & Hero Assault: **KEEP** + rule-mixing per Architecture #6.
  Hunt matches the original games (the maintainer, 2026-11-14; issue #161). Named
  openbf choices:
  - Each BF1 Hunt adaptation fields its planet's BF2 Hunt roster and unit counts, with
    BF1's class for the same unit: Endor scout troopers against Ewok troopers and scouts,
    Geonosis clone sharpshooters, Kashyyyk BF2's MagnaGuard (built from BF2's side; BF1
    has none) against Wookiee mechanics and warriors, Naboo Plains super battle droids
    against Gungan defenders and soldiers, Mos Eisley Tuskens against Jawas. BF1 Theed
    and the Dune Sea have no BF2 Hunt and keep their native encounters.
  - BF1 Hunt Ewoks throw BF2's Ewok spear and rock; BF1's were AI-only weapons. Their
    repair tool stays BF1's.
  - Class change works at your own posts in Hunt by BF2's near-a-post rule; a BF2 post
    whose class has no class-change radius gives none, as in the original. Every BF1 post
    (BF1 has no class change) takes BF2's control-zone 5 m.
  - The Hunt class screen gives each native class its own name where the text tables
    share one ("Ewok Warrior", "Ewok Scout").
  - A BF2 Hunt plays the original's Hunt world: the sublevel holding the mission's
    `<map>_hunt` world, its posts and spawns, the script's barrier switches, and every
    object its `KillObject` calls name removed from the scene (model and collision):
    Hoth's Hunt opens the Echo Base back shields and removes the shield generator, as
    the original does. The map boundary is the map's own (no Hunt sublevel carries
    one).
- BF1 Historical Campaigns: **DROP** (it is essentially curated conquest battles in movie
  order with narration stills/VO between; per the maintainer's rule that's not worth keeping —
  narration assets still get baked, revivable).
- **Special teams as options, played the conquest way (the maintainer, 2026-09-28).** The player
  eventually picks who fights on a map: the four factions and the special teams (Wampas,
  Ewoks, Gungans, Geonosians, Naboo guards, Wookiees, Jawas/Tuskens, BF2's hunt rosters,
  BF1's historical rosters), each with its own weapons, items, sounds and movement, under
  **conquest rules with no hunt score** (the maintainer dislikes hunt scoring). The historical
  campaign's scripting stays dropped; its rosters and `_h` worlds are content sources.
  Cards TEAMS (survey) → TEAMD (design); not built ahead of M1/M2.
- **BF1 Naboo Plains always has the Gungan fambaa and its shield (the maintainer, 2026-09-28)**, in
  every mode, as the map's unique feature (the classes load only from `NAB1_H.LVL` on the
  disc; card FAMB).
- BF2 story campaign: **KEEP**. Stretch idea (LATER): extend/mod the story or a small new
  storyline of our own.
  - **Award weapons count for their base weapon's objectives (the maintainer, 2026-11-21).** An
    objective that names a weapon ("take out the sniper droids with the sniper rifle") also
    counts kills with the award that replaces it in the same kit (the award beam rifle), paired
    from the class data. The original reports the award's own class, so its kills never counted.
  - **One campaign from any copy: the console campaign's rules (the maintainer,
    2026-11-21).** Rise of the Empire builds from whichever Battlefront II copy the player
    owns, and a PC or Xbox copy plays the console campaign's rules. Those copies' own
    scripts differ: PC Geonosis adds a timer around the Mace Windu objective, PC Kashyyyk
    wraps an objective's completion in a test, icons, victory delays and AI difficulty
    numbers change, and their shield library splits the recharge between the linked
    objects. Committed programs (`scripts/campaign_programs.py`, one table per copy beside
    its correction table) rebuild the canonical translation out of the copy's own script,
    checked by digest, so no console disc is needed. Lines whose audio a copy does not hold
    play silent there; the PC has no planet transition movies.
- Galactic Conquest: **KEEP, BF2 galaxy-map + credits version**; credits/reward economy
  needs a tuning pass. Research task R1: survey community GC mods on the PC ports for
  improvement ideas.
- **Era mix-and-match (NEW)**: traditional matchups by default, option for cross-era
  battles (e.g., droids vs stormtroopers). Cheap because factions are baked rosters;
  watch AI class assumptions + VO fallbacks.
- **New modes (LATER, candidates)**: firefight/horde (defend a CP against escalating
  waves), gun game (weapon ladder per kill), king of the hill (single contested CP),
  elimination (no-respawn rounds — pairs well with no-overspawn taste), hero hunt
  (rotating hero vs squad). Pick 1–2 post-1.0.

- **BF2 animated props (Jabba's Palace's rancor, Jabba, trapdoor, torture droid):**
  BF2's `animatedprop` classes play their idle and their triggered clips, and an
  animated prop a placed class carries on a hardpoint is drawn there (the rancor in
  its pit, Jabba on his dais). The trapdoor in front of Jabba opens when someone
  stands on its centre and drops him into the pit; the rancor grabs a soldier who
  comes within 2 m of the spot in front of it, holds him in its hand and kills him
  (one ticket, no killer, its belch) when the grab ends — the disc's update, B2
  `FUN_002312c0` (`core::animated_prop`). The trapdoor's two leaves ride their
  bones, so they now cover the hole they used to leave open. Differences
  [ours, PROVISIONAL]: the triggers and the catch test the live units' feet; the
  idle's start point is a fixed hash, not a random draw; the held soldier hangs
  1 m under the rancor's hand without the `thrown_flail` animation (measure: the
  PS2's held-soldier pose); the rancor itself has no collision body (trace: its
  bone-parented prims on the PS2); `DisableForCloneWars` is not read (no BF2 map
  sets it); the ambient `SoundProperty` of a carried prop is not played (trace:
  the carrier's ambience on the PS2).

### Presentation & audio
- Music mix (maintainer, 2026-11-05): battle music plays at **0.7 x** the disc's gain
  (about -3 dB) before the player's music volume option (`game::music::MUSIC_MIX`).
- **Widescreen-native (the maintainer, 2026-08-30): 16:9/16:10/ultrawide, hor+** — vertical FOV
  keeps the traced PS2 values (render::Camera is fov_y-based, so hor+ falls out of the
  projection; PS2-traced FOV constants drop into fov_y when camera RE lands). HUD and
  shell are authored ASPECT-AWARE (anchored to edges/corners/safe areas, never
  stretched). 4:3-authored content (BF1 load screens, movies) presents PILLARBOXED,
  never stretched. RE flag: check BF1 aim-assist/reticle code for 4:3 screen-space
  assumptions (logged in RE_LOG §aim-assist open items).
- Shell/menus: **BF2** theme & flow. Loading screens: **BF1** incl. the beep sequence.
- **(the maintainer, 2026-09-03) One UI for the whole blended game: BF2's shell, menus and
  HUD on every map of both games.** Look and feel stay BF2's; the CONTENT of
  the menus (settings, details, information) may change to fit our features,
  modernisation and simplification. A BF1 map or unit whose weapon, grenade or
  item has no BF2 art shows its BF1 icon inside the BF2 HUD (bake the BF1 HUD
  icon sheets alongside BF2's; the HUD looks the icon up by weapon class, BF2
  first, BF1 as the fallback). Nothing in a BF1 map uses BF1's HUD or menus.
  The sniper scope is BF2's `weapon_scope` on every map (2026-11-02).
- **(the maintainer, 2026-09-03) One voice set, one command set, everywhere.** Squad
  commands work on every map of both games; the spoken chatter for them is
  ONE universal set (BF1's tactical chatter baked as the universal lines,
  replaced by BF2's if the port ever finds them in BF2's global bank) — the
  same fallback pattern as the HUD icons.
- HUD: **BF2**.
- End-of-round (the maintainer, 2026-10-27 — replaces "BF2"): the round really ends; **BF1's scenic
  shot of the map** under the victory/defeat banner (not BF2's frozen frame), then the stats
  screens the player flips through (scoreboard, medals).
- (the maintainer, 2026-10-27) When one team holds every CP, the HUD shows "Victory in N" / "Defeat in N"
  for the countdown.
- (the maintainer, 2026-10-27) Class change at a friendly CP: Triangle near your own CP opens class
  select (BF2), on every map of both games.
- Announcer/VO: **per source map**. SFX where games differ: **BF2**.
- Music: union, per-map source game.
- Loading uses the BF1 sequence for locations with a matching sequence. BF2-only
  destinations show the BF1 galaxy, a view of the destination from orbit, and a
  still of the ground map or space battle. Each transition uses BF1's scan, zoom
  and sounds. Available images still play if an input is missing, with a plain
  loading frame as the last fallback. The first fresh button press opens a
  three-second skip prompt. A second fresh press while it is visible ends the
  presentation once the battle is ready.
- **(the maintainer, fourth playtest 2026-11-21/22) No footsteps while a soldier floats.** A step sounds
  only when one of the soldier's drawn feet came under its `MinFootHeight` (0.17 m by default) within the last
  15 ticks (`game::foley::MIN_FOOT_HEIGHT`, `FOOT_WINDOW`), so the Emperor's floating sprint is silent. The
  original applies the foot-height rule to drawn third-person soldiers; BF2's first person uses its distance rule
  alone (`analysis/re-foley.md` §4.2-4.3). openbf applies the foot rule in every view, so a floating hero is silent
  in first person too. OURS (the first-person gate); measure: Grievous and MagnaGuard steps in a playtest, whose
  clips sit near the threshold.

### Cheats (session-scoped, BF2-style pause-menu codes)
- From the originals: invincibility, infinite ammo.
- New: infinite stamina, infinite jet fuel, all-awards-for-session.
- All reset on app restart; registry designed for easy additions.
- In MP/splitscreen: host-only control (see Saves, sessions & host authority).

### Saves, sessions & host authority (added 2026-08-30)
- **Profile selection.** Start opens the local profile list with the last used name selected.
  Players can create, select and delete profiles. The existing single save imports once.
  Settings, control bindings, career stats and unlocks are scoped to the selected profile.
- **Autosave everywhere.** Each user profile autosaves continuously: career stats and
  award/unlock progression in every mode, single-player or multiplayer. Story campaign
  autosaves at mission/checkpoint boundaries. Galactic Conquest autosaves after every
  battle/turn.
- **Host (P1) sessions.** LAN and splitscreen sessions have a designated host. Host
  chooses the session type: Instant Action (BF2-style multi-map/mode queue) or Galactic
  Conquest.
- **Host-only session authority.** In MP/splitscreen, only the host toggles cheats and
  session rules (e.g., awards on/off). Per-profile progression still saves for everyone.
- **Post-match results (2026-11-12).** The team scoreboard lists all players and
  bots. Continue opens the viewer's Personal Stats, followed by their Career
  Stats. In splitscreen, show each local player's match and career pages in
  order. The controller assigned to the displayed player advances those pages.
  In LAN, each client sees the same team scoreboard and its own match and career
  pages. After confirming the last page, the client shows “Waiting for other
  players” until the host has received confirmation from every connected player.
  The host then advances the playlist. A disconnected player leaves the ready check.
- **MP Galactic Conquest autosave ownership: OPEN** — host-only save vs replicated to all
  players (so anyone can continue solo) vs hybrid. Deferred design item #8.

### Classic-plus assets (added 2026-08-30 — Harrisonfog remaster integration)
- **(the maintainer, 2026-09-03) Render quality is a RUNTIME toggle, never a gameplay
  change.** Like Halo MCC's original/anniversary switch: the in-game menu picks
  the visual tier. The bake keeps the base artwork and any eligible override
  side by side. The renderer selects the tier at runtime. The simulation reads
  the same heightfield, `coll`/`prim` collision, placements, missions and
  navigation at either tier. These inputs follow the shared gameplay rulebook
  on every supported source. Any remaster mesh is a draw-only override.
  Every classic-plus change requires byte-identical `--sim-hash` results and
  replays at both tiers. Later texture liberties are decided case by case for
  the visual tier.
  **(the maintainer, 2026-09-03, refinements):** the switch lives in the PAUSE menu's
  graphics settings, flipped mid-level without reloading — so both tiers are
  resident (or hot-swappable) at runtime and a mid-round flip is a pure
  renderer event (the sim hash must not move across it). A remastered render
  mesh uses the base model's name and instance transform. The bake checks its
  bounds against the base model with the documented tolerance. Placements,
  collision and mission data follow the same gameplay rules at both tiers.


- **HD textures**: bake learns to read PC-format LVL textures (D3D formats, audit A2
  §5; LibSWBF2/swbf-unmunge = format reference) and emits them as name-matched
  overrides over the base artwork. The planned remaster coverage is BF2 plus
  BF1 Bespin Platforms. The planned local upscale covers the remaining BF1
  maps and units.

  **Override chain (locked design, the maintainer 2026-08-30): Harrisonfog HD (name-matched) >
  our upscale output > PS2 original**. **REFINEMENT CONFIRMED by the maintainer and IMPLEMENTED
  2026-08-31 (`bake::tier`): PC STOCK sits between Harrisonfog and our upscale,
  taken only where the PC texture is LARGER
  **and is the SAME ART.** Measured over every BF2 map (analysis/
  community-projects.md): 370 of 1,040 name-matched textures (34.8 %) are
  strictly larger on the PC, the modal case a clean 2x in each dimension. Real
  2x source art beats a 2x upscale; but only where it is bigger, because PC is
  lossy DXT1 and at equal size a PS2 palettized texture may be cleaner — and
  only where it is the same painting, because `sky_hoth` is a blizzard on the
  PS2 and a blue sky on the PC (24 such rejects, plus 11 detail-compressed
  bases; 312 taken). That survey covered BF2. PC BF1 support came later.
  On the BF2 survey maps, texture bytes roughly doubled and measured frame
  time stayed within the same range. The upscale pass takes per-category
  settings in the bake config.
  **DLSS is NOT the tool for this (asked + answered 2026-08-30).** DLSS is a
  TEMPORAL FRAME upscaler — it reconstructs a rendered frame from motion
  vectors, depth and sub-pixel jitter accumulated across frames. It has no
  offline image mode and cannot bake a static texture; "run DLSS on a texture"
  is not an operation that exists. The right tool for offline texture upscaling
  is the ESRGAN family, which is already the plan below. (DLSS as a RUNTIME
  frame upscaler is a separate, legitimate idea — but it is NVIDIA-only, which
  conflicts with the Intel-iGPU floor, wgpu does not expose it so it would need
  raw Vulkan interop, and the measured frame times leave no performance problem
  for it to solve. FSR1/NIS as a plain spatial shader is the vendor-neutral
  option if upscaling is ever wanted. Revisit only if classic-plus lighting
  makes the frame expensive.)
  Evidence base (both gitignored, read-only):
  `community_mods/harrisonfog/upscale-feasibility/RESULTS.md` — Real-ESRGAN pass is
  VIABLE (seconds/texture on the RTX) but must be category-aware: props/buildings/
  rock 4× clean; dither-shaded terrain layers + palettized gradient skies need a
  de-dither/deband pre-pass, 2×, or substitution by Harrison's layers on shared
  planets; alpha preserved. Genuinely upscale-only BF1 content is small (Rhen Var,
  Cloud City bes1, BF1-unique props/skies).
  `community_mods/harrisonfog/LOOK-REFERENCE.md` — Harrison's exact 2025 preset
  numbers (saturation 1.13, contrast 1.03, bloom blend 0.225/intensity 0.1,
  vignette 0.25→1.0, ASSAO r=0.5/mult=0.8, tonemapper None, grain+DoF OFF), the
  full MIT Shader Patch HLSL tree extracted locally (86 files), and the derived
  wgpu feature checklist. **Render-crate roadmap requirement: the remaster ships
  A8L8 _bump normal maps + 1024–2048px terrain layers → classic-plus NEEDS normal
  mapping with tangent generation — plan the vertex format for tangents NOW so the
  mesh format doesn't need a redo** (noted in render/src/mesh.rs).
- **Addon maps** (`10-addon-maps-keep/`): map VARIANTS in the data model — one
  canonical entry per map, variants selectable (e.g. Realistic Geonosis/Hoth vs
  installer versions; CW/GCW space skins as era variants). Not map-list spam.
- **The look**: reimplemented NATIVELY in wgpu. References in order: Shader Patch
  source (SleepKiller, MIT — reading github.com/SleepKiller/shaderpatch allowed and
  encouraged) with Harrisonfog's 2025 preset as the target numbers; then the 2019
  MasterEffect configs (bloom/AO/tonemap/SMAA values). NEVER port or ship the
  d3d9.dll/ReShade binaries.
- **Not wanted**: Anakin's menu/shell framework (native BF2-style shell per PLAN),
  Remaster script manager, .exe patches. Improved-Sides beyond-vanilla extras
  (TFU/phase-3 troopers, Falcon, legion skins) = opt-in variants later, default
  rosters vanilla-faithful (no-bloat rule).
- **Replacement models** (Part 2): later experiment (skeleton/anim risk) — textures
  first.
- **Legal**: everything stays local-only and uncommitted; personal build only. Any
  public release needs Harrisonfog/Javitolo98/Anakin permission; Shader Patch alone
  is MIT.

### Platform & meta
- Input: **controller first**; PS2 default mapping, full remap; on-screen keyboard for text entry.
  **Flight controls** defaults to Classic on every source map: left stick
  moves the aircraft. In aircraft that can stop, reverse and strafe, left X/Y
  translates sideways/forward and right X/Y aims yaw/pitch. In continuously
  flying aircraft left X/Y steers yaw/pitch and right X/Y rolls/throttles.
  Select by supported motion capabilities, not map or individual class
  (maintainer, 2026-11-14).
  RC Mode 2 is an opt-in layout for
  controller pilots, with yaw and throttle on the left stick and roll and pitch
  on the right. Pull the right stick back to pitch up and push it forward to
  pitch down. The profile saves the choice.
  BF2 aircraft with authored stunts use the contextual roll button followed
  by a direction. Double-tap it before steering to reverse. Keyboard pilots
  can use Left Alt plus direction or Caps Lock for a reversal.
  - Keyboard + mouse (2026-10-25, the public repo's pull request #1) is supported as a
    deliberate deviation. Controller play follows the default layout above. The mouse aims by angle (`PadState::mx`/`my`,
    `core/src/sim/look.rs`), under pointer lock, without aim assist; the bindings are one table in `game/src/kbm.rs`.
    Sensitivity and invert-Y are profile settings. A flyer's pilot is the exception: the mouse reaches the sim as ordinary sticks, by the PC's held rate (`re-flyer.md`).
  - The keyboard + mouse rules (moved from `AGENTS.md`, 2026-11-02):
    Keyboard and mouse play is a deliberate deviation from the PS2's controller-only input. Controller play, bots and
    controller-only replays must stay bit-identical. The rules:
    - The mouse reaches the sim as `PadState::mx`/`my`: angles in `LOOK_UNIT` (1/65536 rad), applied once, on the
      first tick of a frame. They are not rates: no `max_turn_speed` cap and no stance turn factor (except Tumble,
      GetUp and Dead), and zoom divides them by its magnification. The pitch limits, the prone clamp and the seat aimer
      limits still hold. Both fields default to 0 and are left out of logs when 0, so old logs and snapshots load.
    - `buttons::MOUSE_AIM` marks a pad aimed by the mouse; aim assist never acts on it. Moving the right stick clears it.
    - Hovers and flyers: the mouse yaw becomes the turn stick, full deflection at
      `MOUSE_STEER` rad per tick. Flyers use their own flight model and also turn mouse pitch into the pitch stick.
      Walkers: the mouse aims directly, held inside the walker's yaw limits beside its legs
      (`walker_aim`); the drawn aim keeps its heading while the legs turn the hull. Seats and building turrets: the look
      turns by the angle.
    - Between ticks the view draws the mouse look not yet consumed (`Sim::look_preview`, `render_blend::blend_in_looking`),
      so the drawn view is where the next tick's aim lands. A controller on foot gets the same (2026-10-29): the view
      draws the frame's share of the turn the next tick's look step takes under the held stick, the aim assist included,
      and a FIRE press carries that share (`PadState::fire_lag`) so its tick fires part way into the turn, where it was drawn.
    - Bindings are one table (`game/src/kbm.rs`, `Bindings::pc_default`), and each context (infantry, vehicle,
      starfighter) may save a layout of its own (`game/src/control_layout.rs`). The stock on-foot table (2026-11-14, the
      maintainer's choice after the first LAN playtest, from a playtester's own remap) is Left mouse fire, Middle mouse
      secondary fire, Right mouse zoom, Space jump, Left Ctrl roll / stance, Left Shift sprint, R reload, E use, mouse
      wheel next weapon and G next secondary weapon. Every shown default works on a three-button mouse; the back and
      forward thumb buttons also fire the secondary and switch the secondary weapon, and Z, C and Q stay as extra keys.
      Vehicles and starfighters keep the table from before it (Right mouse secondary fire, Middle mouse or Z zoom;
      `Bindings::vehicle_default`, `control_layout::fallback`, `control_layout::context_bindings`). Getting into or out
      of a vehicle swaps the table without letting go of held keys (`Kbm::swap_context`). Mouse sensitivity defaults to
      1.0, shown as 100 %, in 5 % steps from 5 % to 200 % (`ui::menu::MOUSE_SENSITIVITY_RANGE`). On foot, look is
      multiplied by `kbm::ON_FOOT_LOOK_SCALE` (2.8), so 100 % turns as the old 2.8 did; in a vehicle, a turret or a
      starfighter the factor is 1.0 and 100 % turns as before (`control_layout::look_scale`). A dev-phase migration,
      slated for removal before v1 (`Settings::migrate_dev_v0`), carries a profile from before settings version 1: an
      untouched stock table takes the new one, and a chosen sensitivity v becomes v / 2.8. The mouse look
      reads raw device counts (`DeviceEvent::MouseMotion`): on Windows and Linux X11 the display scale and the system
      pointer speed do not change it. measure: Wayland's unaccelerated deltas under fractional scaling, and macOS, whose
      deltas are accelerated points, so the default may feel different there. Pointer lock follows the battle: it is taken on a
      click, and freed on pause, in the shell and spawn screens, on focus loss and on exit. A mouse player's spawn takes it
      back with no click (2026-11-14, the maintainer's request after the first LAN playtest, #128): mouse look had the
      pointer as he went behind the spawn screens, or the pointer accepted a spawn screen. A controller player's spawn
      leaves it free, so a stray mouse move never marks his pad mouse-aimed and turns his aim assist off. Focus lost on
      the way still waits for a click.
- Splitscreen co-op: **KEEP** for later (M5, before PACK; local — one sim, N cameras, N input devices, no netcode; NOT
  part of the multiplayer hold; card SSCR). Not current scope.
- LAN: client-server UDP, **cap 16**, bots server-side. Internet matchmaking: DROP. Held until the maintainer says (M7).
- Saves/profiles: modern format; awards/unlock progression persists per profile.
- Load diagnostics (2026-11-22, #189, after an Apple Silicon player's game sat on the loading screen) are ours;
  the originals have none. A single-player load that fails (the worker's error or panic, the GPU upload or round
  start, a map that is not baked) shows "Couldn't load <map>: <reason>" over the menu and the next loading screen
  for 20 s, with the session log's path (`game/src/main/windowed/load_notice.rs`), where it used to move on in
  silence. The shell and a LAN launch also write stdout and stderr to `openbf_data/logs/openbf-<UTC time>.log`,
  the newest 5 kept, a panic with its backtrace included (`game::session_log`, Unix only for now, `OPENBF_LOG=0`
  turns it off). A loading prints a heartbeat every 10 s (phase, worker done, frames drawn, last frame) and both
  sides of its GPU upload, and advances without redraws when the window gets none (`loading::STALL`).

### Dropped outright (never RE these binary regions)
GameSpy + DNAS, EENet/PPP stack, NTGUI app, memory-card UI, USB headset/GSM voice, PS2
GS/VIF/VU renderer internals, IOP sound drivers, PSS movie codec, PAL/NTSC logic, SDK
plumbing. KB/M input paths (PC-ism, dropped by decision #28; our own KB/M was added 2026-10-25, see Platform & meta). BF1 Historical Campaign
scripting. BF2's on-foot X-lock aim mechanic (the vehicle/space lock-on is NOT dropped —
§Vehicles & space).
