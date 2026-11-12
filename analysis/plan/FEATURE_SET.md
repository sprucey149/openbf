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
  From the DLC only Kit Fisto and Asajj Ventress are taken. Where versions overlap, compare them and take the
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
  in projectile motion, kits, and flyer rules.
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

### Combat
- SBD weapons (2026-10-27, issue #27): BF2's wrist blaster, tri-shot and wrist
  rockets, including slot ammunition, damage, spread and timing, on both map sets.
  Both discs' values and the source tests are recorded in `analysis/sbd-weapons.md`.
  SBD sounds follow the map's source game: BF1 on BF1 maps, BF2 on BF2 maps.
  This presentation choice does not change the shared gameplay numbers.
- **Weapon balance.** Keep the current openbf values. A future rebalance would record each weapon's
  change and retain the original-game measurements for comparison and rollback.
- Awards & medals: **KEEP (BF2 system)** — grind-to-permanent-unlock stays. Settings
  toggle on/off; "all awards this session" cheat.
- Classes: **hybrid unlock**. In-battle: BF2-style score unlock for special classes, every
  battle. Meta: cumulative usage/feats (elite-rifle-style graduated criteria) lower the
  in-battle threshold over time → eventually permanently unlocked.

### Heroes
- **Playable heroes (BF2) AND AI-controlled heroes (BF1 presence), both on all content.**
- Heroes are **killable, always** — no BF1 invincibility ("you see a hero and you just run
  away — no fun").
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
  Squad commands: follow / hold / disperse / clear, ≤ 6 bots within 20 m of the
  player. Design, evidence and phases: Log "AI decision". Space AI (P8) will be
  the same architecture.
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
- **Unit counts (the maintainer, 2026-10-29):** Instant Action defaults to each map/era's
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
- CTF: **KEEP both variants, REWORK later** — the maintainer barely played it (childhood reason: the
  clone commander says "damn" when the flag drops — optional VO filter noted); needs a
  fun pass before it matters.
- Hunt & Hero Assault: **KEEP** + rule-mixing per Architecture #6.
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
  locations use the BF1 galaxy view, with a plain loading frame when the art is
  unavailable. The first fresh button press opens a three-second skip prompt.
  A second fresh press while it is visible ends the presentation once the
  battle is ready.

### Cheats (session-scoped, BF2-style pause-menu codes)
- From the originals: invincibility, infinite ammo.
- New: infinite stamina, infinite jet fuel, all-awards-for-session.
- All reset on app restart; registry designed for easy additions.
- In MP/splitscreen: host-only control (see Saves, sessions & host authority).

### Saves, sessions & host authority (added 2026-08-30)
- **Autosave everywhere.** Each user profile autosaves continuously: career stats and
  award/unlock progression in every mode, single-player or multiplayer. Story campaign
  autosaves at mission/checkpoint boundaries. Galactic Conquest autosaves after every
  battle/turn.
- **Host (P1) sessions.** LAN and splitscreen sessions have a designated host. Host
  chooses the session type: Instant Action (BF2-style multi-map/mode queue) or Galactic
  Conquest.
- **Host-only session authority.** In MP/splitscreen, only the host toggles cheats and
  session rules (e.g., awards on/off). Per-profile progression still saves for everyone.
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
  **Flight controls** defaults to Classic. RC Mode 2 is an opt-in layout for
  controller pilots, with yaw and throttle on the left stick and roll and pitch
  on the right. The profile saves the choice.
  BF2 aircraft with authored stunts use the contextual roll button followed
  by a direction. Double-tap it before steering to reverse. Keyboard pilots
  can use Left Alt plus direction or Caps Lock for a reversal.
  - Keyboard + mouse (2026-10-25, the public repo's pull request #1) is supported as a
    deliberate deviation. Controller play is unchanged. The mouse aims by angle (`PadState::mx`/`my`,
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
    - Bindings are one table (`game/src/kbm.rs`, `Bindings::pc_default`). Pointer lock follows the battle: it is taken on a
      click, and freed on pause, in the shell and spawn screens, on focus loss and on exit.
- Splitscreen co-op: **KEEP** for later (M5, before PACK; local — one sim, N cameras, N input devices, no netcode; NOT
  part of the multiplayer hold; card SSCR). Not current scope.
- LAN: client-server UDP, **cap 16**, bots server-side. Internet matchmaking: DROP. Held until the maintainer says (M7).
- Saves/profiles: modern format; awards/unlock progression persists per profile.

### Dropped outright (never RE these binary regions)
GameSpy + DNAS, EENet/PPP stack, NTGUI app, memory-card UI, USB headset/GSM voice, PS2
GS/VIF/VU renderer internals, IOP sound drivers, PSS movie codec, PAL/NTSC logic, SDK
plumbing. KB/M input paths (PC-ism, dropped by decision #28; our own KB/M was added 2026-10-25, see Platform & meta). BF1 Historical Campaign
scripting. BF2's on-foot X-lock aim mechanic (the vehicle/space lock-on is NOT dropped —
§Vehicles & space).
