# openbf - the archived log, 2026: what is still worth keeping

Pruned 2026-11-02 from a 6,751-line chronology of sessions, handoffs and landing windows. This file keeps reverse-engineering facts found nowhere else in `analysis/`, the maintainer's dated decisions, and the sections other files cite. For anything else, the full text is in git: `git log -1 --until=2026-11-02T00:00 -- analysis/PLAN_LOG_2026.md` names its last full version. Entries are as written then and may be out of date: `analysis/plan/FEATURE_SET.md` and `PLAN.md` win.

## Part 1 — PLAN.md Log, 2026-08-29 to 2026-09-03

### 2026-08-29 / 2026-08-30 — kickoff, P0–P6, the first Opus sessions

- 2026-08-30 (agent, autonomous): **P2 COMPLETE — wgpu map viewer.** (1) Render-geometry
  chain RE'd and baked: wrld inst class → entc GeometryName (PROP 0x47C86B4A) → gmod LOD
  table → modl; manifest gains classes+geometries; `Scene::resolve_model` (137/249 TAT1
  instances drawable — rest are dust emitters + com_* classes living in COMMON.LVL, bake
  those at P4). (2) `game --map <baked>` renders TAT1 windowed (gilrs fly-cam: sticks +
  triggers + boost) and `--headless --capture-frame` emits PNG; `--cam/--size` flags.
  (3) XFRM convention PROVEN empirically: rows = object basis vectors in world (row-vector
  convention); decisive test = Tusken-camp barrier-arc tangency 7/7 (first probe via
  sandcrawler tilt was misleading — logged in RE_LOG). (4) Golden committed:
  goldens/tat1_overview.png + Tier-2 perceptual-diff test (`cargo test -p game --test
  golden_tat1 -- --ignored`, shared GOLDEN_CAM). (5) Tier-4 feel artifact for the maintainer's async
  review: engine/artifacts/p2_flythrough/tat1_flythrough.mp4 (7s aerial tour, 84 frames,
  local-only) + README listing known v0 gaps: sarlacc pit needs terrain cutting
  (CUTR/cutterQuadMask), no turrets/droids/CP visuals yet, a few gray submeshes. All
  commits Tier-0/1 green. → P3 (soldier v0): RE subagents launched for BF1 aim-assist
  magnetism + BF1 stance machine/both rolls.
- 2026-08-30 (agent, autonomous, P4 session 2): **P4 combat v0 underway — weapons,
  world, walk-mode firing landed.** (1) core::weapon + core::lcg: fire cycle /
  spread / heat / damage transcribed from FUN_00288d10/FUN_0032e8c0 + family, ODF
  prop parser (PropSource), BF1/BF2 rifle fixtures, all re-blaster-damage §9
  vectors as Tier-1 tests. Lead read the bodies in full and corrected the report
  (RE_LOG): the trigger-init handler fires the first salvo (refire branch refuses
  TriggerSingle → one salvo per press), weapon+0xc4 is the ACTIVE flag not the
  fire button, BF2 doesn't re-roll the period at salvo start, BF1 heat model
  recovered (overheat ≥1.0, clears < HeatThreshold). (2) core world: units
  (soldier+health+weapon+team, generation-checked handles), bolts with the single
  predictive raycast vs heightfield + capsules (dodge-after-shot still hits, late
  entrants never hit — authentic, PCSX2-flagged), impact → TakeDamage → death →
  battle.on_death, aim assist moved into Sim with the flat-fire lead solve, dev
  cheats (`--cheat invincible,ammo`), per-tick events. (3) bake `classes` now emits
  wpnc/ordc/expc (agent, lead-verified against the baked RON): rifle values match
  §8 byte-for-byte; CP classes CaptureTime 10 / NeutralizeTime 12 (major 16/36);
  recharge droids SoldierHealth 15 (BF1) / 25 (BF2); infantry weapons are
  `WeaponName1..4` (hash 0x2CE1A1D1..); BF2 rep rifle fires `rep_weap_inf_rifle_ord`
  via an ordc→ordc chain; ClassDb FNV fixed to the engine's `|0x20` form.
  (4) game walk mode runs on core::Sim: R2 fires, dummies are real 300 HP troopers,
  bolts render as team-tinted boxes, `--seed`, motion.csv gains shots/hits/kills/
  health. DECISION: sim entities live in a deterministic slot arena (core::sim
  units/bolts), not hecs — hash-stable and replay-safe; hecs stays available if
  entity variety later warrants it. IN FLIGHT: RE subagent on CP capture state
  machine + region containment (analysis/re-cp-capture.md); next = bake spawn
  paths + CP layout, capture logic, BF2 bleed wiring, minimal AI, Tier-4 firefight
  artifact.
- 2026-08-30 (the maintainer, mid-session): **Harrisonfog remaster integration directive** →
  §Classic-plus assets added (HD texture overrides from PC LVLs, native wgpu look
  from Shader Patch MIT + preset numbers, addon maps as variants, no shell/exe
  ports); R1 graphics half resolved, R2 + W1 queued. Sources organized + verified
  under community_mods/harrisonfog/ (INVENTORY.md; gitignored local-only).
- 2026-08-30 (Opus session, the maintainer asked): **Golden gallery — one sheet per map
  and per unit** (`goldens/maps/*.png`, `goldens/units/*.png`, 36 images, 18 MB
  total). Each map sheet is the auto-framed overview beside a ground-level view
  at the map's first command post with a reference soldier standing in it for
  scale; each unit sheet is four fixed poses with the side's rifle. Rendered by
  `game::gallery` (deterministic cameras from baked bounds + the map's own CPs),
  gated by `tests/golden_gallery.rs` (perceptual diff, `UPDATE_GOLDENS=1` to
  regenerate), and regenerated in one command by `scripts/goldens.sh`
  (`--check` to verify). The maintainer asked for these to eyeball progress; they earned
  their keep in the first run.
- 2026-08-30 (the maintainer, decisions): **(1) The Lua mod ecosystem is NOT a goal** —
  "a lot of what is out there just makes everything messy"; a mod worth having
  gets ported into native Rust instead. Architecture #3 updated; this closes the
  question Phoenix raised. **(2) PC SWBF2 install supplied** at `PC_swbf2_exe/`
  (gitignored immediately — 9.6 GB of copyrighted material was sitting untracked
  in the tree). **(3) Measured: the PC textures ARE better for about a third of
  them** — 2x dimensions on 6-35 textures per map across four maps, essentially
  never smaller, name matching direct (79/79 on DEA1). Real 2x source art beats
  a 2x upscale, so a PC-stock tier is PROPOSED for the override chain, taken
  ONLY where larger (PC is lossy DXT1; at equal size the PS2 palettized texture
  may be cleaner). Awaiting the maintainer's confirmation. **(4) DLSS asked about and
  answered: wrong tool** — it is a temporal FRAME upscaler with no offline image
  mode; offline texture upscaling is the already-planned Real-ESRGAN pass.
  **(5) Released to Opus from the hold list:** per-map `lght`/`sky_`/fog, and the
  colour-space questions (COLR, linear-vs-gamma shading, the LUMI-neutral
  re-check). See HANDOFF §NEWLY OPUS-SAFE.

### 2026-08-31 — the four Opus sessions: light, water, sky objects, and the maintainer's defect hunt

- 2026-08-31 (Opus session 3): **Every planet gets its own light.**
  `render::Lighting` was ONE hardcoded sun + flat ambient for all 32 maps.
  Each map authors its own in `sky_` (both games) and `lght` (BF2 only — no BF1
  map LVL carries one, measured over all 285 LVLs). Both are munged config
  trees whose property names survive only as FNV-1a-32 hashes; recovered by
  CONFIRMED PREIMAGE the way `bake sound-names` works — **122 of 123 `sky_`
  hashes and 23 of 23 `lght` hashes**, candidates from shipped `.sky`/`.LGT`
  sources, swbf-unmunge's curated table, both PS2 ELFs and the PC exe (which
  contains none of them). Full write-up + per-map table:
  **`analysis/re-sky-lighting.md`**, indexed in RE_LOG.
  Two conventions cross-checked against each other on BF2 GEO1, the one map
  that stores its sun twice: the `lght` quaternion is (w, x, y, z) with
  forward = rotated −Z, and `SunInfo.Angle` = (elevation + 90, −azimuth). They
  agree to **2.8°**; none of the other 23 order/axis combinations lands within
  20°. Executable test. `render::Lighting` gains a hemisphere ambient (which
  is exactly the pair `GlobalLights` authors) and linear distance fog.
  THREE judgement calls, all OURS and PROVISIONAL, all in `bake::lighting`:
  (1) **colour space** — the authored 0..255 values are gamma and are stored
  LINEARISED (sRGB EOTF), the same rule the `_dtl` GPU test already proved,
  and it also makes an authored 128/128/128 fog appear mid-grey instead of
  sRGB 188; (2) **which maps get fog** — nine of 34, after dropping
  `far <= near`, pure-black colours, and `near <= 0 && far >= 2000`; ten of the
  sixteen BF1 maps ship `FogRange(0, 2700..30000)` with placeholder colours and
  rendering those literally turned Tatooine black, Bespin red and Geonosis
  white, which is how the rule was found rather than predicted; (3) an
  **ambient floor** of 0.045 linear, because BF2 COR1 authors `GlobalLights`
  0/0/0 and MUS1's key light points UP (lava), and both render black taken
  literally. All 42 goldens regenerated and viewed as before/after contact
  sheets (`engine/artifacts/lighting/`). **HOLD FOR FABLE: what actually
  enables fog on the PS2** (`SkyInfo` hash 0xaf8bb8ce preimages to "Enable"
  but stores 1559.0 on eight BF1 maps and 1.0 on the other eight — byte-
  verified, and the split does not line up with which maps carry plausible
  fog). **HOLD FOR FABLE: the `SunInfo` back light** (`BackAngle`/`BackColor`/
  `BackDegree`, authored on all 16 BF1 maps, parsed but not rendered).
- 2026-08-31 (Opus session 3): **Config-format erratum — a string argument is
  an OFFSET, not the constant 4.** A `DATA` string slot holds
  `4 * argCount + the byte offset of that string inside the trailing blob`,
  and the blob packs NUL-terminated strings back to back (byte-verified on BF2
  MUS1's `Effect("mus_sfx_lavaspray_skydome", "hp_lava5", 6.0)`). The old
  reading was the one-argument special case: it returned only the first string
  and read later string slots as denormal floats. `4 * argCount` is also why
  `lght`'s `Light("sun")` (argc 2) stores 8, which had been unexplained.
  `ConfigNode` gains per-argument `strings`/`str_arg`; `f32(i)` refuses a
  string slot. Also recorded: `Type` and `StaticReflectionName` genuinely
  COLLIDE at 0x5127f14d, so a hash match is evidence and not proof.

- 2026-08-31 (Opus session 4, the maintainer asked, bounded): **Fog: three candidates
  eliminated, one strong lead, and the lead was reverted on purpose.** No
  decompiled C was read; sources were our decoded `sky_` data, the PC
  install's own `.lvl` sky chunks and the community docs.
  **`Enable` (0xaf8bb8ce) is NOT a fog switch** — it is 1 on all 18 PS2 BF2
  maps AND all 29 PC maps without exception, varying only on BF1 (1559.0 on
  eight, 1.0 on eight). A field constant across two platforms cannot select
  fog per map. **`FogRamp` (0xfaaa0e17) carries no per-map information** —
  exactly 3.0 on all 16 BF1 maps, absent from every BF2 and PC map.
  **`WorldFogRange` (0xd6c706f2) is parked** — two maps only (BF2 DAG1
  −1.7,−5 and MUS1 60,−155, identical PS2 and PC), descending and negative,
  so not a distance pair; a height band fits but two maps cannot decode it.
  **On BF2 there is no separate enable**: a map has fog iff it authors
  `SkyInfo.FogColor` with far > near, and the set authoring none is IDENTICAL
  on PS2 and PC (COR1, DEA1, NAB2, TAN1, TAT2, UTA1), which says the absence
  is authorship. **The lead: BF1 keeps its real fog under
  `DomeInfo.LowResTerrain`** (colour + `FogNear` + `FogFar`, on 15 of 17 map
  LVLs), and BF2 promoted exactly those values into `SkyInfo` — HOT1
  (203,201,203 / 100..1000), END1 and YAV1 (144,153,156 / 90..350) match
  **byte-for-byte on colour AND both distances**, KAM1 on colour and near. On
  the three maps session 3 found broken it gives the plausible colour where
  `SkyInfo` gives nonsense: TAT1 black → pale desert haze, BES1 red →
  gold-brown sunset, GEO1 white → orange. It was implemented, rebaked,
  rendered and VIEWED — Hoth gains BF2 Hoth's blizzard from identical
  numbers, Endor and Tatooine improve — and then **REVERTED, because BF1
  KAS2's `LowResTerrain.FogColor` is `255, 0, 0`**: the same placeholder, in
  the other field. The better source still needs a placeholder filter, so it
  swaps heuristics for heuristics rather than producing a rule, and per the maintainer's
  brief a well-evidenced negative beats a guess that looks right on one map.
  **`bake::lighting::fog_from` is UNCHANGED and all 42 goldens verify.**
  Also found, not decoded: `FogColor`'s 4th component behaves like a density
  (HOT1 255 blizzard, TAT1 80 haze, KAS1 30, NAB2 0); and a 176,378-name
  generated preimage search found nothing for `0x23e10337` (a `SkyInfo`
  sub-block carrying its OWN `FogRange` on BF2 KAS2 and MYG1), `0x3c84cf36`,
  `0xb1bfdd49`, `0xfef82e0b`, `0xd246dcd4`, `0xa99aeb4f` or `0xb4c88558`.
  **HOLD FOR FABLE, sharpened: does BF1 apply `LowResTerrain`'s fog to the
  whole world or only to the low-res terrain sheet, and what filters the
  placeholder colours both fog fields carry?** Tier-4:
  `engine/artifacts/fog_rule/`.
- 2026-08-31 (Opus session 4): **Sky objects — the capital ships every map
  authored and none drew.** `sky_ SkyObject` was parsed and unused; twenty
  maps author flights of distant craft (BF1 TAT1 two each of four capital
  ships at 350–500 m and 2500 m out; BF1 GEO1 **20 gunships and 20 droid
  fighters** at 300–600 m — the Battle of Geonosis). The unverified claim
  about where the geometry lives was CHECKED FIRST: all 17 distinct names
  were searched for across every `modl` in every LVL on both discs, and on
  every GROUND map they are in a SIDE LVL (ALL/CIS/IMP/REP), never the map's
  own; only the space maps carry theirs locally. Two resolve nowhere and are
  reported rather than guessed at (BF2 COR1 authors an EMPTY geometry; BF2
  KAS2 names `rcis_fly_gunship_dome`, which exists in no LVL). The bake pulls
  them out of the SIDE LVLs beside the map — note SIDE LVLs nest content in
  `lvl_` wrappers, so `content_chunks` is needed and a top-level walk finds
  ZERO models. PROVISIONAL and ours: the BEARING (the discs author count,
  altitude band and distance but no azimuth; ships are spread evenly around
  the compass), and two authored fields are deliberately NOT consumed because
  undecoded — `InDirectionFactor` and the unnamed `0xb4c88558` pair.
  **It also uncovered an unrelated bug: `sky_from` matched dome geometry
  case-insensitively but stored the AUTHORED name, while the runtime looks
  meshes up by exact name and `modl` names are lowercased — so every dome
  whose authored name is not lowercase drew NOTHING. BF1 RhenVar 1 and 2 had
  no sky at all; BF1 Bespin lost its `Cloud City` shell.** Two Tier-1 tests
  now assert every sky mesh resolves. iGPU cost from the player's eye:
  Geonosis 2.02 ms, Bespin 1.83, Coruscant 1.52.

### 2026-09-01 — the cleanse, walkable collision, the AI decision, the P9 spine, Phases A/A.1/A.2

  **CORRECTION 1 — the "one-ticket stall" is not the no-overspawn invariant
  and needs no decision from the maintainer.** The session-5 entry above read the
  symptom, not the cause. `ROUND_VERBOSE=1 cargo run --release -p game
  --example round_to_outcome -- 60000 42 bf1_kas2 bf2_tan1 bf1_bes1 bf2_dag1`
  (the dump was added this session and kept): on BF1 KAS2 all sixteen
  winning-side AI stand 5.6 m from `cp10`/`cp11` at y = −5.9 — the
  heightfield under the dock — while the capture box spans y ∈ (−1.3, 3.3),
  so `Region::contains` (3D, strict, FAITHFUL) is false for every one, the
  post is never neutralised, there is no domination, and the losing side's
  last unit is the IDLE PLAYER 200 m away, outside `ai::ENGAGE_RANGE` (60 m).
  BF2 DAG1: sixteen AI at y = 4.7 around `cp4` whose cylinder tops out at
  y = 0.5 (a floor below the heightfield). BF2 TAN1 and BF1 BES1: units at
  y = 0 under volumes at y ≈ 2..7 and y ≈ 100..113. The four "never fight"
  maps are the same defect one step earlier. **One cause for all eleven: no
  walkable model collision.** The bleed clamp at `alive < tickets` is
  faithful (re-tickets-spawn.md §1.3) and is not involved. The placeholder AI
  then camps an uncapturable post instead of hunting the last defender —
  that is Deferred design #1, not a round-end rule.
  **CORRECTION 2 — BF2 HOT1's "domination win in 20 s" is a bug.** At tick 1
  all five posts are owned by team 1 because every `com_bldg_controlzone` in
  `hoth_cps` authors `Team "2"` (prop hash 0xA2FD7D0C, verified in the baked
  manifest); the mission Lua reassigns them at load and the placeholder RON
  cannot express that. The mission-data task must carry per-CP initial
  ownership.
  **CORRECTION 3 — dome bands.** The session-3 numbers quoted in the "Sky
  domes" entry ("KAS1 0..40.6°, TAT1 −29.6..39.8 plus a cap; above the band
  the frame keeps its clear colour") were superseded by the session-5 census:
  KAS1 is 0.0..90.0, every base dome reaches the zenith, 29 of 32 reach below
  the horizon. The Log entry stands as written; the code and README copies
  are corrected.

  **Tier-4:** `engine/artifacts/collision/` — soldiers on Kamino's platform,
  Bespin's platform and Kashyyyk's docks, with the motion CSVs that prove the
  heights, and the four residual stalls' verbose dumps.
  **HOLD FOR FABLE (new):** the soldier fall-damage rule; the `AddDeathRegion`
  engine consumer; the PS2 collision layer masks (per primitive, per polygon,
  and whether `prim PRNT` transforms are ever non-identity); the step/slope
  rule and the soldier-vs-world response; the six `LEAF` flag bytes.
  **Deferred with a home:** the shared `manifest` crate (item 5; Opus brief
  in HANDOFF; run now that the schema has settled); bolts vs model collision
  (Opus-safe, HANDOFF); the P9 changes (Opus, specs above); the AI base
  (the maintainer). Commits: f8576ec (missions), 9638d3a (the walkable world), plus
  this entry.

- 2026-09-01 (the maintainer, after session 7): **Single-file build requested.** the maintainer
  wants the game as ONE movable binary (personal convenience; this build is
  never shared, per the guardrails a playable bundle with LucasArts assets
  stays strictly local). Direction recorded, not yet built; folds into P10
  packaging. Two steps, both Opus-safe (HANDOFF §NEWLY OPUS-SAFE): (1)
  exe-relative asset resolution (small, do first); (2) a pack appended to the
  exe with a trailing footer, read by the game from its own file — NOT
  `include_bytes!` (assets_baked is 1.3 GB; compile-time embedding is the
  wrong tool). Also: the release binary is 379 MB because the workspace
  profile keeps `debug = true`; the packaging step should `strip` its copy,
  never the profile (the debug info is what makes crash reports readable).

- 2026-09-01 (Fable session 8, with the maintainer): **AI decision — openbf writes its OWN
  infantry AI; the discs supply the data, the RE pass supplies the feel
  constants. Deferred design #1 RESOLVED.** the maintainer's direction: he does not mind
  the AI behaving differently from either original ("take advantage of 20
  years of research"); FOUR difficulties (Easy / Normal / Hard / Elite) where
  the old BF2 Elite sits at about our Normal, Hard beats both originals'
  hardest, and Elite is very hard but winnable by a veteran; EVERY AI unit
  improves with difficulty (in MP all bots must behave the same regardless of
  team) and yet the game must get harder for the human in single-player and in
  any MP layout (same team, opposing teams, 2-vs-1); and BF1's teammate
  commands come back. Recommendation given and accepted 2026-09-01: build one
  AI, take the originals' DATA, not their decision code.

  **Evidence the recommendation stands on (measured 2026-09-01, session 8;
  `scripts/ucfb_tree.py --census` over every ground map).** Both discs ship
  the SAME three AI data sets on every ground map: `Hint` nodes (BF1 31–266
  per map, BF2 72–973), `BARR` barriers (BF1 83–739, BF2 151–759) and a
  `plan` planning graph of hubs and connections (BF1 one chunk per map; BF2
  one to four, one per mode layer; BF1 TAT1 115 hubs / 151 arcs, BF2 TAT2
  200 / 242; INFO's third word is 5 on both — five unit path types,
  PROVISIONAL until the RE entry lands). So the BF1-vs-BF2 difference is the
  ENGINE, not the map data (A3 §AI evidence, ELF strings): BF1 = a
  connectivity-graph follower (`ConnectivityGraphFollower`, `PathFollower`),
  a hint class family in its RTTI (Snipe/Patrol/Cover/JetJump/Minefield/
  Land), formations, follow/hold agents (`UnitFollowAgent`/`UnitHoldAgent`,
  i.e. the squad commands) and two global skill ints (`ai.PlayerSkillLevel`,
  `ai.EnemySkillLevel`); BF2 = a goal arbiter (`ConquestGoal`, `DefendGoal`,
  `DestroyGoal`, `FollowGoal`, `CTFGoal`, `CTFGoalD`, `DeathmatchGoal`),
  runtime arc/barrier blocking, per-team and per-unit orders, squad commands
  reaching Lua (`followchr`, `stopfollowchr`, `waitforpickup`,
  `getintovehicle`, `getoutofvehicle`), `AICanCaptureCP`, a view multiplier,
  a damage threshold and `Enable/DisableAIAutoBalance`; only `BaseHint`
  survives in BF2's strings. The maintainer's Bespin observation (BF1: everyone through
  the central corridor; BF2: units spread to the outer platforms) is the
  shape a value-scored goal arbiter produces versus a single best-route
  follower — recorded as CONSISTENT, not verified. Difficulty in the
  originals: BF1's shell has three levels (Easy/Medium/Hard,
  `ifs_opt_general.lua` cycles 1..3) and its 17 campaign scripts set the
  PLAYER team and the ENEMY team separately per level — `yav1i_h.lua`: Easy
  (+4, +4), Medium (+2, −2), Hard (+6, −1) via `Set{Player,Enemy}Team
  Difficulty{Easy,Medium,Hard}` (A3: a delta applied only when the profile's
  level matches; the sign convention and the consumer FUN_00237af0 are
  untraced — HOLD FOR FABLE). BF2 PS2's shell also has three (its
  `ifs_opt_general.lua` maps 1/2/3 to easy/medium/hard; CORE.LVL carries the
  UTF-16 strings "Easy" ×2, "Normal" ×6, "Elite" ×16; the PC shell offers only
  medium/hard). What Elite CHANGES on the PS2 is untraced: `SetAIDifficulty`'s
  decode point FUN_002a2988 (A3 open Q2), the per-difficulty
  `ScriptCB_SetNumBots` table 0x5fa808 (re-tickets-spawn §5) and the AI
  dispersion multiplier's writer (weapon+0x138, re-blaster-damage §10.4) are
  the three leads — HOLD FOR FABLE, and the anchor for "old Elite ≈ our
  Normal". No stock BF2 mission script calls `SetAIDifficulty` (grep over the
  PC sources); the shell level reaches the engine through the profile.

- 2026-09-01 (Fable session 8): **The AI data enters the bake — spec.**
  **NAV DATA SPEC — bake the authored AI data (Fable, 2026-09-01; Opus
  implements as its own commit AFTER the shared `manifest` crate).** Source
  of truth: RE_LOG "`plan` / `plnp` / `Hint` / `BARR`" and its SPEC block
  with pinned vectors. Scope is DATA ONLY — no runtime consumer, no AI
  change; the sweep must be unchanged (27 of 32, per-map hashes identical).
  (1) `mungefs::plan`: `Plan { hubs: Vec<Hub { name, pos: [f32;3], radius,
  slots: [u8; 8] }>, arcs: Vec<Arc { name, a: u16, b: u16, type_mask: u32,
  flags: u32 }>, routes: RouteTable }` where `RouteTable` is the BF2 row
  form for BOTH games: `rows[type 0..5][hub] : Vec<Vec<u8>>` (row k =
  k-th alternative, `hubCount` bytes each, entry byte = weight<<3 | slot,
  0 = none; ≤ 4 rows). Detect the layout by exact tiling, never by game:
  NODE size == hubCount·(40 + 20·hubCount) ⇒ BF1 fixed records (unpack
  each u32 into ≤ 4 row bytes, low byte first, 0 ends); else the BF2
  variable record (`u8 rowCount[5]` after the 40 B, then the rows) and the
  records must tile NODE exactly; ARCS size == arcCount·22 ⇒ BF1, ·26 ⇒
  BF2 (`flags`, else 0); anything else is `UcfbError`, never a panic
  (the crate's no-panic contract — add the truncated/oversized cases to
  its tests). Accept hubCount 0 (BF2 KAM1 @0x4DA88). `Plnp { groups:
  Vec<Vec<u32>> (arc-name hashes per group), records: Vec<Vec<u8>>
  (INFLATED, each == rawSize; index = blocked-group bitmask) }` parsed
  from the `plnp` that FOLLOWS a `plan` (same parent); use the zlib
  inflater the workspace already has (check `Cargo.lock` for flate2 /
  miniz_oxide before adding one) with a size cap = the declared rawSize.
  (2) `mungefs::world`: parse `Hint` (`INFO{TYPE, NAME, XFRM}` + PROP*)
  and `BARR` (`INFO{NAME, XFRM, SIZE, FLAG}`, PROP* ignored) from every
  `wrld`, alongside `regn`/`inst`; keep PROPs raw (key hash + string) and
  ALSO resolve the five known keys. (3) Manifest (in the shared crate):
  `ManifestNav { hubs: Vec<ManifestNavHub { name, pos, radius, arcs:
  Vec<u16> (the slots in order, 0xff dropped) }>, arcs: Vec<ManifestNavArc
  { name, a, b, type_mask, flags }>, routes: String (path of the sidecar),
  plnp: Option<ManifestPlnp { groups: Vec<Vec<u32>>, records: String
  (sidecar path) }>, hints: Vec<ManifestHint { name, kind: u8, xform:
  [[f32; 3]; 4] (rows + translation AS STORED), primary_stance: Option<u8>,
  secondary_stance: Option<u8>, mode: Option<u8>, radius: Option<f32>,
  command_post: Option<String>, props: Vec<(u32, String)> }>, barriers:
  Vec<ManifestBarrier { name, pos: [f32; 3] (z NEGATED, the loader's
  world position), rows: [[f32; 3]; 3], size: [f32; 3], flags: u32 }> }`.
  `Manifest.nav: Option<ManifestNav>` for the base world, APPENDED after
  `terrain_cutters` (existing RON text unchanged up to it); `ManifestLayer.
  nav: Option<ManifestNav>` for each BF2 mode layer (its own `plan`, its
  own wrld's hints/barriers; a layer with no `plan` gets `None` and the
  runtime falls back to the base — the last TOP-LEVEL plan wins for the
  base, matching BF2's last-loaded-wins). Sidecars under `<map>/nav/`:
  `routes_<layer>.bin` = `u16 hubCount, u8 types (5)`, then for t in 0..5,
  for hub: `u8 rows`, rows × hubCount bytes; `plnp_<layer>.bin` = `u16
  recordCount`, then per record `u32 len` + bytes. (`scripts/rebake.sh`
  must clear `nav/`.) (4) Tests: byte-oracle Tier-1 tests on every pinned
  vector in the RE SPEC (TAT1 hubs 0/2, arcs 0/1, first Hint, first BARR;
  TAT2 hubs 0/1 with their rows, arcs 0/1, Hint, BARR; KAS2 plnp header +
  record 0 = hubCount zero bytes; POL1 plnp records inflate to 189 and
  72745 B), a no-panic test per truncation, and an `all_maps_load`-style
  census over the baked maps: every arc endpoint < hubCount, every slot's
  arc touches its hub, rows ≤ 4, every routed slot index < the hub's arc
  count, hint kinds ∈ {0,1,2,4,5,6,7,8}. (5) Acceptance: `scripts/rebake.sh
  maps` succeeds on all 32; `diff` of each manifest against its pre-change
  RON is EXACTLY the appended `nav` block (and `nav: None` on any layer
  without a plan); `ci.sh` green; `gpu_tests.sh` untouched (no visual
  change — do not run it); `round_to_outcome` twice: 27 of 32, hashes
  identical to the pre-change sweep (no consumer yet). Report the hub/arc/
  hint/barrier totals per map against RE_LOG's census (`census_out.txt`).

- 2026-09-01 (Fable session 8): **PHASE A DESIGN — navigation on the
  authored graphs (Fable, main thread; Opus builds from the spec that
  follows the nav-data bake).** Inputs: RE_LOG "`plan` / `plnp` / `Hint`
  / `BARR`" (the graph is a per-type routing table walked by a weighted
  chooser; hub y is the editor's terrain height, so the follower is
  XZ-driven and y comes from the ground; the tables reach DAG1's cave and
  POL1's base; BES2 has two authored gaps) and the residual stalls (BES2
  no climb, RHN1 no way down from the citadel, DAG1/POL1 no way in, KAS2
  under the pier). **Decided:**
  1. `core::nav::NavGraph` — an INPUT to the sim like the heightfield and
     the collision world (not hashed): `hubs` (pos, radius, arc slots),
     `arcs` (a, b, type_mask, flags), `routes` (BF2 row form for both
     games), `barriers` (XZ rectangles + mask). Built by `game::layout`
     from the manifest: the mode layer's `nav` when it has a plan, else
     the base; hints and barriers = base + layer.
  2. `locate(pos, mask) -> Locus::{Hub(i), Arc(j, t), Nearest(i)}` — the
     PS2 rule in XZ: the containing hub with the SMALLEST radius, else the
     arc whose corridor contains the point (distance to the segment <
     rA·(1−t) + rB·t), else the nearest hub. XZ is a [D] fact (POL1's hubs
     are all at y 0 under 13–33 m floors; DAG1's cave hubs carry the
     terrain height); the known cost is that a unit on the terrain OVER
     DAG1's cave locates to a cave hub and takes the routed detour through
     an entrance pit — acceptable for Phase A, a same-level test is a
     later refinement.
  3. `next_hop(cur, dest, type, arrival_arc, r: u32)` — the PS2 chooser
     exactly (budget = 31 − weight of the arrival slot's entry; weighted
     pick over the remaining alternatives; dead end → another arc of the
     hub whose mask allows the type), with `r` a per-unit deterministic
     hash like `ai::aim_wobble` (unit index, tick, salt) so `think` stays
     `&Sim` and replay-exact. **Empty entry → `route_bfs`**: breadth-first
     over arcs whose mask has the type bit (XZ arc length as the cost, so
     it is A*-cheap on ≤ 300 hubs), the port's own fallback the PS2 lacks;
     ROUND_VERBOSE counts fallbacks per map.
  4. The follower lives in `core::ai`: `AiState.route: Option<Route {
     dest_hub, next_hub, via_arc, waypoint }>`. On (re)targeting: dest =
     locate(target post's capture centre).hub-or-nearest; waypoint = the
     next hub's XZ with y from `ground_under`; arrival = XZ distance ≤ the
     hub's radius (min 1.5 m, PROVISIONAL) → advance. Steering: face the
     waypoint and walk (the existing yaw-first pad), no beeline to the
     post; the CP's own capture volume is the last leg. A unit whose XZ
     progress toward its waypoint stays under 0.3 m over 60 ticks with a
     non-zero pad is STUCK (PROVISIONAL numbers): re-locate and re-route
     once, then side-step alternately left/right for 30 ticks — measured
     by the sweep, not tuned by taste. Barriers whose mask has SOLDIER are
     treated as walls by the stuck logic only in Phase A (the authored
     corridors do not cross them by construction; ROUND_VERBOSE reports
     any route segment that does).
  5. **AI walls ON**: `Sim.walls_for_ai` is retired and the wall pass
     applies to every unit; the corridor-following is what makes that
     survivable. Lethal landings and death regions stay as they are.
  6. Objective selection stays the placeholder's (nearest post not mine)
     for Phase A — the commander is Phase B — with two additions: an
     unroutable post (no locate, no route, no BFS path) is skipped for the
     nearest routable one, and with no capturable post at all the unit
     HUNTS: dest = locate(nearest enemy's position). That is the RHN1
     "camp on the citadel" fix.
  7. Types: every infantry unit routes as SOLDIER (mask 1) in Phase A;
     jet troopers on `jet_jump` arcs and vehicles on HOVER/SMALL/MEDIUM/
     HUGE come with P7/Phase E.
  **Acceptance (the sweep is the gate):** `round_to_outcome` finishes
  **32 of 32** with walls on for every unit, two runs byte-identical; no
  unit stuck longer than 10 s in any finished round (a per-map maximum
  printed by the sweep); ROUND_VERBOSE shows locus, route and fallback
  per unit; `gpu_tests.sh` untouched (no visual change). Tier-4 for the maintainer:
  a BF2 DAG1 and a BF1 BES2 capture series showing units enter the cave
  and climb to the skyway posts.

- 2026-09-01 (Fable session 8): **Phase A landed — the AI follows the
  authored graphs** (Opus build from "PHASE A DESIGN", lead-verified).
  `core::nav` (NavGraph, locate / hub_for / next_hop / route_bfs, barriers,
  the route-sidecar parser), `game::layout::nav_graph` (sidecar via the
  manifest's own path; a mode layer's plan replaces the base's, its hints
  and barriers append), `Sim.nav` + `Sim.nav_stats` (inputs/outputs, not
  hashed), the follower in `core::ai` (`Route` with nine hashed fields:
  retarget / plan_leg / arrival / stuck / side-step), **`walls_for_ai`
  REMOVED — the wall pass applies to every unit.** 17 new tests. **Sweep:
  28 of 32** (27 before), two runs identical (107 s each now that more
  maps play to an outcome): the four session-7 stalls are FIXED — BES2
  climbs to the skyway posts, RHN1's winners come down from the citadel,
  DAG1's cave post is captured, KAS2 leaves the pier — and four are open:
  BF2 GEO1, KAS2 and UTA1 (regressions from the wall pass now applying to
  AI) and BF2 POL1 (was). BFS fallbacks: 0 on all 32 maps — the authored
  tables answered every hop asked; straight-line fallbacks only on DAG1
  (10) and TAT1 (3), where the chooser walks a unit over an arc whose mask
  excludes SOLDIER (RE_LOG's 902 authoring-noise entries) into another
  component. **One failure mode behind all four stalls (ROUND_VERBOSE,
  per-tick traces): a unit walks a straight leg head-on into model
  geometry the authored corridor passes through — `push_out` is a
  positional push along the contact normal, so a head-on approach makes
  zero progress while a diagonal one already slides — and the 30-tick
  side-step moves it a few metres before it walks back in.** GEO1: units
  "arrive" inside Hub30 (r 13 m, sitting on cp4) 12.8 m out and beeline
  into the bunker's east wall; UTA1 adds the engaged deadlock — a unit
  frozen against a wall with `stuck_ticks` pinned, because the engage path
  returns before the stuck window and the terrain-only LOS lets it see the
  enemy through the wall; KAS2 jams on two arcs mid-corridor; POL1's units
  follow the surface XZ of corridors authored at y 0 for the base 13–33 m
  below (the cost decision 2 named) and stall at the entrance structures.
  `stuck_max` over the 32 maps: median 1,560 ticks, six maps under the
  design's 300 — the gate (32 of 32, nothing stuck over 10 s) is NOT met
  and nothing was tuned to chase it; the fix is the steering layer
  ("PHASE A.1 DESIGN"), after the bolts/LOS item gives `ai::visible` the
  model-collision test. Deviations the build recorded: `Route` carries
  `waypoint`, `ref_dist`, `stuck_run` and `source` beyond the spec (all
  hashed, a test per field); `NavGraph::new` is `None` for a hubless plan
  (BF2 KAM1) and `locate`/`hub_for` return `Option`; a per-type
  connected-component table and `CommandPost.nav_hub` cache reachability
  (inputs, unhashed); `think` returns `(PadState, NavTick)` for the
  diagnostics; `locate` does not skip `flags & 7` arcs (the PS2's does —
  PROVISIONAL until the arc-flag RE). **Data finding:** SOLDIER arcs cross
  SOLDIER barriers on every map (3 on POL1 to 58 on KAM1/MYG1; 40 of DAG1's
  69), so either `BARR SIZE` is full extents rather than half ([H] in
  RE_LOG) or barriers are not corridor walls — **HOLD FOR FABLE (barrier
  semantics)** before anything acts on them; nothing does in Phase A.
  Player replays byte-identical (the wall pass and the hash format
  changed, the player's motion did not). PROVISIONAL constants:
  `ARRIVE_MIN_RADIUS` 1.5 m, `STUCK_WINDOW` 60, `STUCK_PROGRESS` 0.3 m,
  `SIDESTEP_TICKS` 30. Noted, untouched: units still end some rounds at
  NO_GROUND depths (−204 DAG1, −300 TAT1, −281 BF2 HOT1), pre-existing.

- 2026-09-01 (Fable session 8): **A.1 addendum — barriers are the PS2's
  "Obstacles", and how it walks around them** (Fable RE agent → RE_LOG
  "Barriers are the PS2's Obstacles…"; the containment test re-read by the
  lead: BF1 `FUN_00267920` takes two dot products with the stored rows and
  compares each with `SIZE + r` — half extents, rows as stored, XZ).
  Facts that change A.1: (1) barriers never touch the routing tables; on
  every hub-to-hub leg the PS2 tests the straight segment (radius r + 0.5)
  against enabled barriers blocking the unit's type and, if it crosses
  one, runs an **A\* over the barrier silhouette corners** (corners
  expanded by r + 0.5, search band max(25 m, leg length), depth < 32) to
  get a detour; the walk step re-aims at a corner when a step would cross
  a barrier, a unit inside a barrier is pushed out, a waypoint inside one
  is re-rolled. So SOLDIER arcs crossing SOLDIER barriers is NORMAL data
  (702 of 4,245 on the discs; the PS2's own corner search resolves 1,150
  of 1,262 crossing legs, 100 % on BF1; the 112 unresolved are BF2
  door/shield barriers the mission Lua toggles, 27 of them exactly the
  maps' `plnp` groups — that is what `plnp` is for). (2) FLAG polarity =
  "blocks these types"; all barriers load ENABLED; BF2 also spawns
  dynamic barriers from destructible buildings (later). (3) Arrival is
  the mover's, in XZ (1 m at the last node, else 4 m / class) — the port's
  XZ arrival is faithful; BF1's hub LOCATE is 3-D and BF2's hub test is XZ,
  so our XZ locate is BF2's, a superset of BF1's (PROVISIONAL either way).
  (4) Local avoidance in the originals = barriers + a contact push (side-
  contact normals summed, a 5 m side-step) + stuck timers (2 m / 3 s /
  5 s) — no feelers, no obstacle circles (`ai.showobstacles` has no
  readers). (5) BF2 arc flags: bit 0 only excludes an arc from locate;
  bits 1–2 turn the leg into a jump request (45° up toward the node) —
  nothing is directional; BF1 has none. **A.1 therefore adds, before the
  wall probe: the leg-vs-barrier test and the corner-walk A\* producing
  intermediate waypoints (2-D, over the corners of enabled barriers whose
  rectangle meets the search band; a barrier containing the leg's start
  or goal is ignored for that leg so the 20 hubs authored inside barriers
  cannot deadlock); the wall probe/slide stays for the geometry barriers
  do not cover; the contact push from the wall pass is reused as the
  PS2's side-step cue.** Squad commands (Phase D) got their engine shape
  too: BF1 broadcasts to team units within 45 m / 30 m, follow = 10 m steps
  toward the leader, released beyond 20 m; BF2 targets the reticle with a
  follower-slot cap — the maintainer's 6-within-20-m rule stands as decided.

- 2026-09-01 (Fable session 8, the maintainer's request): **Texture alpha census over
  every baked texture — and ALPHA RULE v2 (spec).** Opus measured 2,182
  baked textures (32 maps + common + side), joined to 14,746 `segm`
  materials over 243 LVLs (the join reproduces `bake::map`'s `tex_flags`
  exactly on all 2,182 rows — the instrument was checked both ways before
  anything was read off it); `engine/artifacts/texture_census/README.md`,
  `census.csv`, 18 close-ups, two contact sheets, seven scripts. Classes:
  A flagged+bimodal (true cutout, right today) 400 (+491 opaque-alpha), **B
  flagged+mid-band heavy (alpha-tested to shreds today) 303, of which 44
  are on drawn instances**, C composited 35, **D unflagged real cutouts
  rendered OPAQUE today 16** (Theed's banners hang as solid rectangles on
  both discs, Kashyyyk's round deck edge is squared off, a grate, a rope,
  Jawa junk), E non-Normal RTYPs 48, F flat 119, G detail bases 106; 902 of
  the 1,229 flagged textures have no material at all (they take the
  `unwrap_or` default and draw nothing). The lead viewed both sheets.
  **What the flag word means (first data on HOLD 10(g)), 268 flagged
  textures with a real material that the test erodes, base P(false
  cutout) = 0.29:** bit 0x02 → 0.10 (the real cutout flag; the leaves and
  the bamboo are 0x07), 0x04 → 0.36, **0x10 → 0.69, 0x40 → 0.56, 0x80 →
  0.71, 0x400 → 0.76, 0x1000000 → 0.81**; by render type `Normal` → 0.23,
  `Scroll` → 0.76, BF2 RTYP 3 → 0.81 (BF2's 3 behaves exactly like BF1's
  `Scroll` — first evidence for the BF2 enum), `Specmap` → 1.00. Reading:
  those bits and types mark alpha used as a scroll/specular MODULATION
  channel, not a mask. **Two facts that change the fix:** (1) the
  per-texture OR is lossy — 417 textures are named by segments with
  different flag words, 157 by both a flagging and a non-flagging one (31
  of the 44 visible B); `kam_crate`'s Scroll/Specmap come from segments
  that are not the crate body (0x05 Normal) — so the rule must be **per
  draw**, the granularity the GLB already has; (2) 10 of the 44 visible B
  textures want BLENDING, not opaque (fountain water ×5, `yav_waterfall_02`
  ×2, northern lights, `myg1_sky_mist`, comm screen, console fx — all 0x04
  + Scroll). Also: `myg1_mountains` (0x03, alpha max 115) draws NOTHING
  today (tested away, and the blend pass requires 0x04); `common.ron` has
  no `transparent` list so 15 common class-C textures can never blend;
  PC-tier alpha is a different channel (class D is judged on PS2 alpha);
  no placeholder textures anywhere; `tan1_sky` is pure black and
  `bes2_detail_1` uniform 128; the mid-band threshold table says 0.15
  would box Endor's 471 trees (mid 0.254) and the bamboo (0.214) while
  0.30 flips the crate (0.550) and boxes nothing the sheets show as real.
  **ALPHA RULE v2 (Fable, PROVISIONAL until HOLD 10(g) is read out of the
  ELF; Opus implements after A.1):** per DRAW (segment material), from the
  PS2-source alpha stats — `mod` = flags & (0x10|0x40|0x80|0x400|0x1000000)
  or RTYP ∈ {Scroll, Specmap, BF2 3}; `mid` = fraction of texels in
  32..223; `a0` = fraction ≤ 31; `amax`:
  1. `amax` ≤ 128 → BLEND, whatever the flags (fixes `myg1_mountains`).
  2. `mod` set: 0x04 set AND (RTYP Scroll/3 OR bit 0x400) AND mid ≥ 0.30
     → BLEND (the fountains, waterfalls, mist, lamp glow); otherwise OPAQUE
     (crate body strip, antenna, pole, bunker panels, pod room).
  3. no `mod`: 0x02 set → TEST (leaves, bamboo); else 0x04 set → TEST when
     mid ≤ 0.30, else OPAQUE (the crate body); else (unflagged) → TEST when
     a0 ≥ 0.05 and mid ≤ 0.10 (class D: banners, grate, rope, deck edge,
     Jawa junk), else OPAQUE.
  Mechanism: the bake writes the verdict as glTF `alphaMode`
  (OPAQUE/MASK/BLEND) on each primitive's material plus the stats in
  extras; the renderer routes MASK to the alpha-tested pass and BLEND to
  the blend pass from the material, not from the manifest's per-texture
  `transparent` list (kept, derived, for the HUD/fx consumers until they
  move); `CommonManifest` gets the same treatment. Acceptance: the census
  re-run under the new rule (its class table is the test), `kam_crate`
  solid in the close-up, leaves/bamboo/banners cutouts, the ten effects
  blended, `myg1_mountains` visible; a before/after contact sheet of every
  map golden viewed by the lead before `UPDATE_GOLDENS`; ci green. Expect
  most of the 42 goldens to move — that is the point. **Also found by the
  census, separate defect:** `--scene-report` over all 32 maps counts 148
  untextured draws (BF1 BES1 64, BF2 KAS2 36, DEA1 11, UTA1 9, DAG1 7, GEO1
  6, MUS1 4; 22 maps at zero) — list their texture names and find the
  lookup that fails (Opus-safe, after the rule). The 107 skipped classes
  are all "no drawable model" (emitters, spawns, sound markers, and turret
  classes whose model lives in a SIDE LVL the map bake does not pull —
  the turrets are P7's).

- 2026-09-01 (Fable session 8): **PHASE A.2 DESIGN — thin walls, levels and
  jump arcs (from the A.1 sweep evidence).** The six maps still stalled
  after A.1 (BF2 POL1, UTA1, KAM1, TAN1, GEO1, BF1 YAV2) share two new
  signatures: (1) a unit running at full speed with `steer fwd` and zero
  progress — the probe capsule is translated 1 m ahead, so a THIN wall
  between the unit and the probe position is never touched by it while
  the wall pass pushes the unit back every tick; (2) waypoints whose
  ground is NO_GROUND or on another floor — `hub_waypoint` takes
  `ground_under(hub.pos)` from the hub's AUTHORED y, which on POL1 (all
  hubs at y 0 under floors 13–33 m up), UTA1 and TAN1 finds nothing, and
  on KAM1 a platform-to-platform leg the geometry does not join (8 m
  below, 40 m away). **Decided:**
  1. **The probe becomes a sweep.** `CollisionWorld::wall_sweep(cap, dir_xz,
     dist) -> Option<(t, normal)>`: parry's linear shape cast
     (`query::cast_shapes`, in `query/` — sqrt-only per the audit) of the
     soldier capsule along `dir` up to `dist`, same AABB prefilter and
     floor filter, first hit in body index order with strict `<` on t. It
     replaces `wall_probe` everywhere (`wall_probe` stays as a test
     helper or goes).
  2. **The wall pass is the contact cue.** `Unit.wall_push: Vec3` (the
     summed push the wall pass applied this tick, hashed) is the PS2's
     side-contact accumulator (`soldier+0x970`); steering treats a
     non-zero push as "blocked, normal = the push direction" even when the
     sweep is clear, and uses it for the side-step's side.
  3. **Same-level waypoints.** The waypoint's y is the ground at the hub's
     XZ from the UNIT's level: `ground_under((hub.x, unit.y, hub.z))`. If
     that is NO_GROUND, the leg's floor is not on the unit's level and the
     leg is REJECTED from here: `next_hop` is re-asked with that arc
     excluded, then `route_bfs` over arcs whose far end HAS ground at the
     unit's level, then the PS2's random-other-arc; a map where every
     alternative fails is reported, not walked into. `hub_for` for the
     START hub prefers containing hubs whose floor exists at the unit's
     level. (BF1's locate is 3-D — RE_LOG — so this is the same-level test
     the addendum named, done with the collision ground instead of the
     authored y the discs do not carry on POL1.)
  4. **Jump arcs.** BF2 arc `flags` bit 2 (jet_jump [H]) is excluded for
     units without a jet pack in `locate`, `next_hop` and `route_bfs`; bit
     1 (jump) stays routable for soldiers (a hop). PROVISIONAL which bit is
     which until the RE names them; count both per map in ROUND_VERBOSE.
  5. Diagnostics: per map the level-rejected legs, jump-arc exclusions and
     sweep hits; ROUND_VERBOSE prints the waypoint's ground verdict.
  **Acceptance:** the sweep (gate 32 of 32, two runs identical), the
  stuck_max table before/after, the player replays byte-identical
  (`push_out` untouched; the recorded push does not alter motion),
  `--bench` at 16 a side and 64 within the "not more than double" rule, and
  for every map still stalled the ROUND_VERBOSE evidence — then stop.

- 2026-09-01 (Fable session 8): **Phase A.1 landed — local steering** (Opus
  build from "PHASE A.1 DESIGN" + its addendum, lead-verified: ci green,
  player replays byte-identical, two sweeps identical). `CollisionWorld::
  wall_probe`, `NavGraph::{leg_crosses_barrier, detour}` (the PS2's corner
  walk: A\* over the expanded silhouette corners of enabled barriers in the
  search band), the follower's sub-waypoints, the sweep-and-fan steering
  with a remembered slide side, the pad built from the WORLD move direction
  (`lx = +X` at yaw 0, `ly = −Z`, measured against `Soldier::step`), engaged
  units walking on until `HOLD_RANGE`, `ADVANCE_CONE` deleted, `Barrier.
  enabled` (always true today — the hook for BF2's Enable/DisableBarriers).
  12 new tests. **Sweep: 26 of 32** (24 before): DEA1, BF2 KAS2 and MUS1
  finish; TAN1 regressed; POL1, UTA1, KAM1, GEO1 and BF1 YAV2 still stall.
  `stuck_max` median 2,280 → 1,980 over all maps, unchanged over the
  finished ones (1,680 → 1,710) — the gain is entirely at the jammed end
  (DEA1 53,220 → 180; KAS2 58,620 → 2,760); the design's 300-tick target is
  met on three finished maps. **The failure shape changed:** at HEAD the
  stalls were units pinned at zero velocity; now every deep stall is a unit
  running at full speed that never closes on its waypoint — (1) the probe
  capsule is translated 1 m ahead, so a thin wall between the unit and the
  probe position is never touched while the wall pass pushes the unit back
  each tick; (2) waypoints whose ground is NO_GROUND or another floor
  (`hub_waypoint` = `ground_under(hub.pos)` from the hub's AUTHORED y: POL1's
  189 hubs at y 0 under floors 13–33 m up, UTA1 a waypoint at y −75 under a
  unit at 1.1, KAM1 a platform-to-platform leg the geometry does not join);
  TAN1 is a standoff, not a stall (6 v 4 alive, every LOS `BLOCKED world`,
  no bleed — the reposition/hunt gap Phases B/C own). Blocked-forward ticks
  separate the populations: 1.0–1.6 M unit-ticks on the six stalled maps
  against 1,860 on TAT2 — the one-probe-per-moving-unit cost holds
  wherever the AI is getting somewhere. **Deviations, all measured and
  recorded by the build:** the slide side is remembered (`AiState::slide`,
  hashed — recomputing it flipped near head-on; TAN1 stuck_max 48,600 →
  23,940); the probe skips the bottom `STEP_HEIGHT` (a kerb is not a
  wall); the side-step offsets the move TARGET 5 m (the PS2's
  `FUN_0026aba8` shape); progress is measured to the current sub-waypoint
  (the PS2's rule; the single biggest win — MYG1/TAN1/YAV1 went from stalls
  to finishing); `arrived_at_post` needs 1.0 m XZ of the post (BF1 NAB1
  froze sixteen attackers in a ring at the capture cylinder's edge); a
  stuck re-plan drops the corner path (NAB2 unit 25 sat 6.6 m from the
  same corner for 49,440 ticks); no separate "last push" state; the
  `battle_replay` test now builds TAT1 with its collision world and graph
  (on a nav-less TAT1 both teams swept the ring in the same direction and
  never met — the placeholder objective rule, not the steering). The build
  reports that its deviations 5 and 6 bought NAB1/NAB2/MUS1 and cost GEO1
  and TAN1 (an intermediate build measured 25 of 32 with a better median);
  kept, both grounded in the RE — the trade is recorded and reversible.
  **Cost:** `--bench` TAT2 sim tick p99 0.448 → 0.461 ms at 16 a side
  (+2.9 %), 1.863 → 1.931 ms at 64 (+3.7 %); on the jammed maps the fan
  runs up to 10 probes a tick (POL1 mean tick 0.48 → 1.28 ms, 3.8 % of the
  budget) and the 32-map sweep takes ~300 s. PROVISIONAL constants (all
  labelled): `PROBE_DIST` 1.0 m, `CORNER_ARRIVE` 1.0 m (the PS2's node
  radius), `DETOUR_RADIUS` 1.0 m, `SIDESTEP_OFFSET` 5.0 m, `FAN_STEP` 30°,
  `DETOUR_MAX` 6, `CORNER_CLEARANCE` 0.5 m, `DETOUR_BAND_MIN` 25 m,
  `DETOUR_MAX_DEPTH` 32, `DETOUR_MAX_BARRIERS` 24. Tier-4: `engine/
  artifacts/ai_nav/` — Dagobah's cave post captured (team 0 from 1 to 4
  posts by tick 5,396 in the idle capture), Bespin's units climbing from
  the streets. Next: "PHASE A.2 DESIGN".

- 2026-09-01 (Fable session 8): **Phase C hypotheses closed** (Fable RE agent
  → `analysis/re-ai-difficulty.md` §9; §8 items 1, 4, 8 struck). (1) **The AI
  view cone is a 77.5° half-angle [V] in both games**: `FUN_00539ed0` fills
  the BSS sin/cos table (2048 pairs, 2π/2048), `FUN_00539ff8` is its cosine,
  `… the test is `d² < 100`
  (10 m omni) else `dot > 0 && d²·c² < dot²`. BF1 has the same cone with NO
  10 m omni and a scan radius clamped 75..350 m. The Phase C design's [D]
  becomes [V]; 12.5° is ruled out. (2) **The level-3-only multiplier is the
  ODF key `HitLocation` (fnv1a 0xced77479, `"%s %f"`, per collision node,
  default 1.0)** — shipped only as vehicle critical-hit prims ×4 on 15 BF2
  vehicle classes (AT-AT `p_neck_soldier`, AT-ST `p_crit_hit`, AAT/hailfire/
  IFT/AT-TE `p_crithit` …; BF1 ships only `p_crithit 0`); the gate
  `FUN_001e0580`: a multiplier > 1 applies to a HUMAN attacker always and to
  an AI attacker only at level 3, multiplied into damage before friendly
  fire, shields and armour. So the PS2's Elite lever is "AI may score
  vehicle criticals", NOT a damage buff against humans — Phase C's
  per-human damage multiplier (1.15 / 1.3) is entirely ours, PROVISIONAL.
  Corrects re-blaster-damage §6.2's "no per-bodypart multiplier" (there is
  one, per collision node, vehicles only on the discs). (3) **The PS2 BF2
  shell passes the flag: `ifs_difficulty` compiles to
  `ScriptCB_SetDifficulty(CurButton, true)` [V]**, which sets `bAutoAim`,
  `bAimAssist`, `bStickyReticule = (level < 3)` — Elite on the PS2 turns the
  human's aim assist OFF; BF1's binding takes one arg and touches no aim
  flag. Design consequence for Phase C: the originals' only per-human
  pressure at Elite is losing aim assist; ours keeps the assist slider
  (PLAN §Movement) and adds the pressure layer — worth offering "Elite
  disables aim assist" as the PS2-faithful default of that slider, the maintainer's
  call. (4) "dodge = roll" is NOT supported ([D]: `FUN_0020a940` resets the
  movement sub-object, never the roll dispatch); `FUN_002aba38`'s codes are
  engagement-suitability modes, not movement.

- 2026-09-01 (Fable session 8): **Phase A.2 landed — the swept probe, same-
  level waypoints, jump arcs** (Opus build from "PHASE A.2 DESIGN", lead-
  verified: ci green, player replays byte-identical, two sweeps identical).
  `CollisionWorld::wall_sweep` = parry's linear `cast_shapes` of the capsule
  along the move direction (read in parry's source: support-map cast via
  `gjk::directional_distance`, sqrt-only; `wall_probe` deleted), with
  `SWEEP_CONTACT_DIST` = 1e-3 m (PROVISIONAL, new: a cast from a touching
  pose reports a float-zero hit in every non-separating direction, so
  inside that distance the A.1 point test decides); `Unit.wall_push`
  recorded by the wall pass and HASHED; `hub_waypoint` resolves the floor
  from the unit's level and falls back to the authored height; a leg with
  no floor either way is rejected and re-routed (next_hop with the arc
  excluded → BFS over floored arcs → the PS2's random-other-arc → straight,
  counted); BF2 arc bit 2 (jet_jump [H]) excluded for units without a jet
  pack (`UnitSpec::jet`, always false today), bit 1 (jump) routable.
  **Sweep: 27 of 32** (26 before): BF1 YAV2, BF2 GEO1 and TAN1 finish; BF1
  NAB2 and BF2 MUS1 regress; POL1, UTA1, KAM1 still stall. Median
  `stuck_max` 1,980 → 1,770 (1,710 → 1,260 over the maps that finished
  before); the biggest wins GEO1 −49,680 and YAV2 −44,700 ticks. **The
  contact cue (design decision 2) is recorded but NOT wired into the
  steering**, on the strength of a four-way ablation over the full sweep:
  sweep only 24 / 32; sweep + level + jet (shipped) 27; + side-step side
  from the push 25 (median 2,640); + the full cue 24 (median 3,300); the
  full cue without the level rule 22. Cause: `push_out` reports ANY
  penetration, and the push is non-zero on ~60 % of unit-ticks on the
  jammed maps — not what a side-contact accumulator means; the module
  header carries the numbers so the rule can be retried against a real
  accumulator. Two more measured deviations: the literal "unit's level"
  rule rejected 3,894 legs on TAT1 in 3,000 ticks because `ground_under`
  calls a terrain sheet more than `TERRAIN_BURY` above the feet "buried"
  (same waypoint: floor 16.9 for a unit at y 18.0, NO_GROUND for one at
  11.6) — hence the authored-height fallback; and the two-floor test is
  written the way the ground query behaves (it looks DOWN, so the level
  test catches a floor above and not one far below). **Per-map counters
  (32 maps): 5 jet-jump arcs excluded, 17 jump arcs kept, 1,756 level-
  rejected legs (622 unrouted; POL1 1,153 / 252), 5.86 M sweep hits.**
  **Evidence for the five stalls:** BF1 NAB2 — six units at y −49 whose
  waypoint floor is 11 m BELOW them, `steer fwd` (the sweep says clear)
  with `push 0.30` every tick: **a sweep bug** — the floor filter skips the
  whole BODY on its first (floor) hit, so a wall on the same body is never
  seen ("one hit per body"), the case the contact cue would have caught;
  BF2 POL1 — units sliding along corridor walls 8 m from waypoints that
  resolve a level below (1,153 rejections) plus a firefight that cannot
  see itself; BF2 UTA1 — a chokepoint the fan cannot get round (three
  units at one spot, `push 0.4–0.5`); BF2 KAM1 — two units 3.9 m apart
  `BLOCKED world` fanning against each other on a platform (a standoff +
  chokepoint); BF2 MUS1 — a corridor the steering cannot solve, no
  routing failure. **`--bench`** TAT2: 16 a side p99 0.654 → 0.663 ms
  (+1.3 %), 64 a side 2.256 → 2.502 ms (+10.9 %). Noted, untouched:
  `NavGraph::components` is jetless connectivity (a jet class needs a
  second array); `next_hop`'s budget subtracts every barred alternative
  (identical to the PS2's `31 − arrival weight` while slots are unique per
  entry); TAT1 still ends rounds with units at NO_GROUND; DEA1 has 132
  unrouted legs yet finishes faster; **the 32-map count is a high-variance
  metric** — intermediate variants flipped the winner on 5–8 maps for
  small changes; the median / `>10k` / sum columns are the steadier
  scoreboard and the sweep should print them. **Next (A.3, small):** the
  per-body floor skip in `wall_sweep` (take the first NON-floor hit per
  body, not the first hit), and run the sweep's maps in parallel threads
  (each map's sim is single-threaded and independent) so a sweep takes
  seconds, not five minutes — the A.2 build spent an hour on thirteen
  sequential sweeps for its ablations.

### 2026-09-02 — A.3, the mirrored wall pass, and the honest 23

- 2026-09-02 (Fable session 9): **The 148 "untextured draws" are not a
  lookup bug** (Opus analysis agent, read-only; the lead reproduced the
  148 total on four maps with `--scene-report`, the empty texture slots on
  the raw bytes of BF2 KAS2.LVL — `kas2_prop_dirt` seg 0: MTRL 0x10001,
  RTYP 0, four `TNAM` slots all empty — and the dangling `yav_prop_ivy`
  with a `tex_`-chunk scan over every LVL of both discs: 0 hits, control
  `kam1_water` 1). Corrections to the census entry: twelve maps are
  non-zero, not seven (tat1 3, BF2 YAV1 3, BF1 YAV1 3, TAN1 1, COR1 1 were
  missing from the list; the total is right), and only 84 of the 148
  render grey — BES1's 64 are terrain chunks the runtime already drops.
  Causes: **A (75 draws, 27 model/segment pairs, 8 maps)** the disc segment
  names NO texture — 1–18-triangle stubs (effect anchors, `*_collision_*`
  placeholders, `kas2_prop_dirt` ×31) painted flat grey by `viewer.rs`'s
  `None` arm; A2 (1) `tan4_bldg_engine_b` seg 1, empty slot but a `COL0`
  vertex-colour set; **B (8 draws, 3 names)** `yav_prop_ivy` (6),
  `Cor1_bldg_flowers` (1), `noIcon` (1, a `geo_sky_explosions` sky shell)
  exist in no `tex_` chunk on either disc — dangling on the disc
  (`yav_prop_ivy` IS a `tex_` in the PC install, a classic-plus question
  for HOLD 8, not a fix); **C (64)** BES1's terrain `LTEX` names
  `kam1_water`, only in KAM1.LVL, dropped correctly already. Case folding,
  suffixes, truncation and the nested-`lvl_` `tex_` chunks were checked and
  account for zero. **Fix queued (Opus, small, brief written):** skip
  disc-untextured and disc-dangling segments instead of painting them grey
  (PROVISIONAL — what the PS2 draws for them is untraced: HOLD FOR FABLE,
  new item), report them on their own scene-report lines, and print the
  dropped terrain chunks apart from "untextured draws". Full list:
  scratch `untextured/list.csv` (session-local).

- 2026-09-02 (Fable session 9): **Build specs released, not yet built
  (the tree is free; briefs and code maps under `engine/artifacts/briefs/`,
  local).** In queue order after A.4: (1) the untextured-draws fix (Log
  above; skip disc-untextured and disc-dangling segments, report them on
  their own scene-report lines; goldens must not move). (2) **ALPHA RULE
  v2 build decisions (Fable, PROVISIONAL):** D1 `RTYP` parsed into
  `mungefs::model::Segment` (BF1 string / BF2 u32, pinned on `kam_crate`'s
  four segments `Scroll/Normal/Normal/Specmap` with flags
  `0x410/0x05/0x01/0x31`); D2 the verdict computed per (model, segment) in
  the bake from the PS2 alpha stats (`mid`/`a0`/`amax` of mip 0, never the
  PC tier's channel) and written as glTF `alphaMode` (+ `alphaCutoff` 0.5
  on MASK) with the inputs in `extras`, materials keyed by (texture,
  mode); D3 the KTX2 keeps its alpha unless EVERY draw of the texture is
  OPAQUE (the 902 material-less textures bake opaque), and a texture with
  any MASK/BLEND draw EXCLUDES the PC tier (its alpha is another channel);
  D4 the manifest's `transparent` list kept, derived (any BLEND draw) and
  unread; D5 the runtime routes per draw from the material — `DrawLayer::
  Mask` between Opaque and Blend, three pipelines (the discard becomes
  pipeline-selected, the literal `base.a < 0.5` kept for the bake↔shader
  pin), the texture bind group no longer decides the pass; D6 the
  side/soldier bake stays per-texture (deferred). Acceptance as the spec
  entry says, plus a `glb_verdicts.py` census over the GLBs; the build
  STOPS before `UPDATE_GOLDENS` for the lead's sheet review. (3) **Phase B
  build decisions:** the ten raw ATK/DEF hashes read in `layout` into
  `CommandPostDef` (faction rows), resolved PER TEAM into `CpDef` at
  `setup_conquest` because `core` has no faction notion; `CpDef::value_for
  (team, owner)` = DEF/ATK/average; `core::commander::TeamPlan` × 2 hashed
  after `waves`; `NavGraph::hop_distances` (a BFS distance field, `route_
  bfs`'s loop) for the 1/(1 + hops) term; greedy assignment in unit index
  order with the design's scoring, hysteresis 1.2×, noise from a `route_
  rand`-style hash; `retarget` reads the assignment; squads bookkept.
  (4) Phase C from its design entry with the §9 numbers; (5) Phase D.

### 2026-09-03 — A.4, the collision layers, ALPHA RULE v2, sky placement, Phase B

- 2026-09-03 (Fable session 10): **A.4 (2/3) — collision LAYERS: the soldier
  world is each class's `SoldierCollision` subset (+ the BF2 prim `MASK`),
  and the session-7 name rule is gone (b112661, 1fb9595, 960fdac). Sweep 22
  → 26 of 32.** Fable RE (agent, every load-bearing claim re-run by the
  lead: `analysis/re-collision-layers.md`, RE_LOG "Collision LAYERS"), Opus
  build from `brief_a4_layers.md`, lead-verified: ci green at HEAD, the
  runtime module read, both sweeps re-run identical, replays and goldens
  checked. **What the PS2 does** [V]: neither ELF parses a primitive's name;
  layer membership is per entity CLASS from the entc PROPs `SoldierCollision`
  / `VehicleCollision` / `BuildingCollision` / `TerrainCollision` /
  `OrdnanceCollision` / `TargetableCollision` / `VehicleCollisionOnly`
  (fnv1a of the lowercased key; a value names a prim or `collisionmesh`, or
  `none` / `clear` for an empty subset); with no property every body is in
  the five subsets; a property REPLACES its own layer's subset with the
  accumulated named bodies and touches no other; `VehicleCollisionOnly`
  adds to vehicle and removes from soldier + ordnance. BF2 adds a per-prim
  `MASK` byte (s 0x01 v 0x02 b 0x04 t 0x08 o 0x10 — the PC exporter's
  `p_-so-…` letters, measured on all 21 BF2 LVLs) whose loader never
  clears its temp, so an unmasked prim after a masked one in the same
  chunk inherits the byte (209 prims; replicated). `nab2_bldg_b_19` on
  both discs: `SoldierCollision = collisionmesh`, `OrdnanceCollision =
  collisionmesh`, `VehicleCollision = p_cube, p_cube1` — the plaza's
  boxes were never soldier collision. **The port:** `mungefs::coll::
  Primitive.mask/mask_own`; `ManifestClass.collision: Vec<(layer, body)>`
  in PROP order and `ManifestPrim.mask/mask_own` (all defaulted; rebake
  11 s, manifests differ only by the new fields); `game::collision_layers::
  layer_bodies` (the rule as a pure function, nine tests plus the six
  ground-truth classes) and `Scene::collision_world` keeps the SOLDIER
  subset; `prim_blocks_soldiers` deleted. Census (`examples/layer_census`,
  lead-reconciled): 1,016 instance-primitives leave the soldier world in
  the base worlds (+118 when common merges: BF1 `com_weap_gunturret`'s
  `p_base`), 965 by an authored BF2 MASK, 0 by an inherited one, 169 by
  class properties; 2 enter (`kas1_bldg_hut_grand p_vehicle_2`, KAS1/KAS2);
  no coll mesh leaves. (The lead's own join said 1,056: it counted
  mode-layer instances the base world does not place.) DEFERRED, in the
  module doc: the ORDNANCE subset for `CollisionWorld::ray` (bolts, AI
  LOS) — on the discs 58 ordnance-only and 21 soldier-only BF2 prims, the
  Hoth bridges' `p_ordinance*`, BF2 common's `com_inv_col_*`
  (`OrdnanceCollision = none`); the vehicle subset for P7; the engine's
  `VehicleCollisionOnly` also REPLACES the vehicle subset with the
  accumulator where the port only adds (vehicle layer unused today); 28
  undecoded-kind prims the bake drops (16 geometries). **Sweep: 26 of 32**
  (22 before): median 1,500 (finished-only 1,080), >10k 5, sum 302,400;
  two runs identical; 17 hashes moved, 15 maps bit-identical (their worlds
  did not change). BF1 NAB2 (34,980 → 3,720), BF1 YAV2 (21,180 → 5,040),
  BF2 MYG1 (17,940 → 2,580) and BF2 UTA1 (46,440 → 780) now FINISH; POL1
  57,240 → 33,840; GEO1 9,720 → 1,200; no finished map regressed. Player
  replays byte-identical (c425fcf50d2bea6b / 0695bdbb7738eb71 /
  24988d2da84964b8; the P3 logs touch none of the dropped bodies); the 42
  goldens unchanged (`scripts/gpu_tests.sh`). `--bench` TAT2 (lead, median
  of five, sim tick p99): 1.010 → 0.946 ms at 16 a side, 3.783 → 3.947 at 64 (run-to-run noise is ±0.4 ms; fewer bodies, no measurable change). **Still stalled (6):** BF2
  HOT1 (four units 0.37 m inside `HOTH_bldg_bunker`'s mesh, jammed against
  each other and the wall, waypoint 2.5 m below through it), tat2 and BF2
  MUS1 (both: one unit oscillating over 0.8 m at `slide` / `fan` with push
  0.000 — the per-tick traces show A.1's REMEMBERED SLIDE SIDE never
  flipping when the slide direction is itself blocked, and the stuck
  side-step's side never reaching it: A.4 (3/3a), brief written), POL1,
  BF1 BES2, BF2 TAN1 (not yet re-read). HOLD 13 is answered for subset
  SELECTION and stays open for the collision RESPONSE (step, slope,
  push-out) and the layer-id constructor stores.

- 2026-09-03 (the maintainer, in session 10): **Two locked-feature-set additions** (§LOCKED
  FEATURE SET, dated): (1) BF2's shell, menus and HUD are THE UI on every map
  of both games — look and feel BF2, content free to modernise; BF1-only
  weapons/grenades/items show their BF1 icon in the BF2 HUD; (2) unit
  behaviour is UNIVERSALISED across the blend, one behaviour per class on
  every map, recorded per class — first: the droideka ball jumps everywhere
  (BF1's rule). Both are design lines for the lead; the icon fallback is an
  Opus-safe bake+HUD item once the P7/heroes rosters name the BF1-only
  weapons.

- 2026-09-03 (the maintainer, in session 10): **Render quality is a runtime toggle and
  never a physics change** (§Classic-plus assets, dated): both visual tiers
  baked side by side and switched in the in-game menu (MCC-style); the sim
  reads only the PS2 bake (collision, placements, missions, nav); the gate is
  `--sim-hash` and the replays identical across the switch; texture creative
  liberties allowed later on the visual tier only. Consequence for the bake:
  `bake::tier` must stop REPLACING the PS2 KTX2 and write the override beside
  it (an Opus-safe change once ALPHA RULE v2 lands, since v2's D3 already
  keeps the PS2 alpha as the truth).

- 2026-09-03 (the maintainer, in session 10): render-tier refinements — the toggle is in
  the PAUSE menu during play (hot swap, sim hash unmoved across it); override
  meshes only ever replace the same-named mesh at the same placement, never
  add/remove/move instances, and the bake rejects one whose bounds exceed the
  PS2 mesh's (so visuals never sit where collision is not). §Classic-plus.

- 2026-09-03 (Fable session 11): **ALPHA RULE v2 landed — the alpha mode is a
  property of the DRAW, written by the bake as glTF `alphaMode` per segment
  and routed by the renderer through three pipelines (3f8c38b, 23bc7d4,
  f7a411f, dda89b4; goldens c787beb).** Opus build from
  `brief_alpha_v2.md` (+ the session-11 addendum), lead-verified: ci green at
  every commit, the rule function and the routing diff read, the census
  re-run by the lead (`glb_verdicts.py`), both sheet sets viewed, the sweep
  re-run by the lead and byte-identical to the session-10 baseline (collision
  untouched), `scripts/gpu_tests.sh --render` green. **What landed:** D1
  `mungefs::model::Segment.render_type` (`RenderType::{Bf1(String), Bf2(u32),
  Absent}`, pinned on `kam_prop_crate`'s Scroll/Normal/Normal/Specmap with
  0x410/0x05/0x01/0x31; the build's survey over all 243 LVLs, 14,746 segments:
  BF1 payloads are 5/7/8-byte strings beside a 4/12-byte MTRL, BF2 exactly a
  u32 beside 25/44 bytes — no overlap). D2 `bake::alpha::verdict(flags, rtyp,
  stats)` — the three clauses verbatim — with `AlphaStats` per texture from the
  PS2 mip 0; every GLB primitive carries `alphaMode` (+ `alphaCutoff` 0.5 on
  MASK) and `extras {flags, rtyp, mid, a0, amax, rule}` (the rule as a JSON
  string, one material per primitive — two build deviations, both accepted).
  D3 the KTX2 keeps alpha iff some draw is MASK/BLEND; the PC tier is excluded
  for 49 such textures (`tier::resolve_keeping_ps2_alpha`, a sibling of
  `resolve`). D4 `transparent` derived and unread (19 → 34 entries). D5
  `DrawLayer::Mask`, `fs_main` (no discard) / `fs_mask` (the literal `base.a <
  0.5`), a mask pipeline identical to the opaque one, `draw_key` led by the
  layer so the pass switches once; `TextureDesc::blend` is gone — one cached
  texture serves every mode; the SKINNED pipeline stays on `fs_mask` because
  D6 leaves the side bake on the per-texture OR (the four unit goldens are
  bit-identical, which is the pin). **D7 (new, PROVISIONAL, the lead's
  amendment after the first sheet review):** the SKY path is out of scope —
  the sky pipeline composites by alpha and never tests, so sky-shell textures
  keep the PRE-v2 KTX2 alpha state (`alpha::keep_alpha`); without it Bespin's
  cloud sea (`sky_bespin1_clouds_2`, one rule-3 OPAQUE draw) baked solid and
  Mustafar's 94 %-transparent base dome composited over the desert-blue clear
  (37 % and 28 % of those goldens). Six sky textures on five maps return to
  their pre-v2 state; `sky_naboo2` is the only texture both a shell and a
  world model draw. **The census under the rule (6,260 draws over 34 baked
  dirs):** OPAQUE 5,276 / MASK 797 / BLEND 187; by clause: rule 1 (amax ≤
  128) 54 BLEND, rule 2 (modulation) 708 OPAQUE + 133 BLEND, rule 3 4,430
  OPAQUE + 797 MASK, no texture 138 OPAQUE; 91 (dir, texture) pairs carry
  mixed verdicts across their own draws — the information the per-texture
  OR could not hold. All 38 named checks pass (`kam_crate` body OPAQUE by
  rule 3, its Scroll/Specmap strips OPAQUE by rule 2; leaves and bamboo MASK
  on their 0x07 segments only; the Theed banners, the rope, the deck edges,
  the grate MASK; the fountains, waterfalls, mist, aurora, shields, screens
  BLEND; `myg1_mountains` BLEND by rule 1 — it drew nothing before). KTX2:
  511 textures lost an alpha they never used (the material-less 902 took
  `unwrap_or(ALPHA_IS_MEANINGFUL)` before), 23 gained one they needed. **Data
  that argues with the thresholds, reported, not acted on:**
  `tat2_bldg_jawa_junky` a0 = 0.0479 against `a0 ≥ 0.05` (the PLAN named the
  Jawa junk as class D; the rule as written misses it by 0.002);
  `kas2_prop_light` (a0 0.24, mid 0.00 — a textbook cutout) goes OPAQUE
  because bit 0x10 alone puts it under rule 2; `cor1_bldg_warroom05` has no
  sub-31 alpha at all. Class D reads 13 MASK / 3 OPAQUE. **Goldens: 28 of 42
  moved, viewed on before/after sheets by the lead** (`alpha/sheets_v3`,
  session-local): the Kamino crates solid, the Kamino ocean a sheet instead
  of alpha-shredded speckle, the antenna poles back, Kashyyyk Docks' water
  visible, Mygeeto's mountains on the horizon, the console screens
  composited, Polis Massa's asteroid shell on PS2 pixels; the four unit
  goldens, both Hoths, NAB1, RHN1, TAT1, the shell, HUD and loadscreens
  unchanged. `--scene-report` identical before and after (0 / 76 / 8 / 64).
  29 PNGs regenerated by `scripts/goldens.sh` (the 28 plus `tat1_overview.png`, which the sheet could not measure and which moved by 2 pixels of 921,600); `scripts/gpu_tests.sh` fully green, re-run by the lead. Rebake 10.8 s; `gpu_tests` 23 s. Tier-4: `engine/artifacts/alpha_v2/`. **Not done, with a home:** the side bake
  (D6, `bake::soldier` module doc); the side-by-side two-tier KTX2 for the maintainer's
  runtime toggle (next Opus item; `bake::tier`'s layout untouched); the
  aurora has no golden camera (`rhn_prop_northern_lights` is BLEND in the
  census but outside RHN2's frame). PROVISIONAL throughout: HOLD 10(g) (what
  the flag word means) stands; the rule is the census's, not the ELF's.
  **Seen while viewing, not this change — the maintainer's report of the same hour:**
  BF2 MUS1's sky carries a hard horizontal seam from any high camera (the
  gallery overview, a fly camera). Root-caused by the lead with one-shell
  renders and the mesh: `mus1_sky_volcano` is two overlapping ring bands of
  one 512×128 texture (rings at mesh y −3.69/3.69 and −1.78/5.98, u shifted
  0.5), each band transparent at its top and opaque lava at v = 1.0, so the
  upper band's bottom edge is an opaque line over the lower band; under the
  port's placement (Offset +100 m, the first shell normalised to 1000 m)
  it sits at +0.63° elevation — predicted screen rows 86 / 149 for the seam
  and the shell's lower edge at the overview camera's 70° fov, measured 89 /
  155–161. Ground cameras hide it behind the terrain skyline; the committed
  golden had it before this change. Whether the PS2 places the shell the
  same way (`DomeModel` `Offset` / `MovementScale` / dome scale / wrap mode)
  is a Fable RE pass in progress; the fix follows its answer.

- 2026-09-03 (Fable session 11): **Sky dome placement RE'd — the Mustafar seam
  is the port's lift, not the disc's** (`analysis/re-sky-dome-placement.md`,
  RE_LOG "Sky dome placement"; Fable RE agent, the lead re-read the three
  load-bearing citations in both decomps and corrected the doc's model
  mix-up). [V] in both ELFs: a dome shell's X/Z are the camera's and its Y
  is `cam.y + (Offset − cam.y)·(1 − MovementScale)·S` (BF2 `FUN_003cb918`
  425830–425837, BF1 `FUN_002f2308` 280238–280240), S = 1.0 [D], the mesh at
  authored size (~20 m around the eye). The authored `Offset` reaches the
  shell only through `(1 − MovementScale)`: MUS1's volcano shell (Offset
  100, MS 0.995) lifts 0.16 m on the PS2 where the port lifts it 100 m, so
  the double band's opaque edge sits at −4.9° on the PS2 (below every ground
  horizon) and at +0.63° in ours. [V] too: each shell is drawn with its
  MATERIAL's GS state (`FUN_0058ed70` 723411–723480 — bit 0x04 blended, bit
  0x02 alpha-tested with AREF 0x60 [D], else opaque with Z write); the port
  composites every shell. Brief written for the next Opus build after Phase
  B (`brief_sky_placement.md`: the ELF's placement rule with the port's
  angle-invariant scale, per-material opaque / tested / blended sky draws;
  goldens will move on the ~17 maps with offset shells; sheets viewed first).
  After the fix the gallery's orbital overview still sees the asset's
  underside (the seam moves below the horizon, not away) — the PS2 has no
  such camera. **HOLD FOR FABLE (new):** whether `FUN_0058ed70`'s material
  rule is the general model draw path — if so it is the first ELF reading
  of HOLD 10(g) (0x04 = blend, 0x02 = test at ≈ 0.75, 0x40 additive) and it
  disagrees with ALPHA RULE v2's census heuristic on the Kamino crate (0x05
  → blended by the ELF's rule, OPAQUE by ours); reconcile against what the
  PS2 shows before touching the rule. Also held: the sky texture wrap mode,
  the far-scene pass's depth range vs a 20 m shell, the MTRL → segment copy.

- 2026-09-03 (Fable session 11): **Sky placement landed — shells sit where
  the ELF puts them and draw with their material's GS state; the maintainer's
  Mustafar seam is below every ground horizon (a937199, f01f45d; goldens
  072d783).** Opus build from `brief_sky_placement.md`, lead-verified: ci green,
  the placement and mode code read against the RE, the sheets of every
  moved golden viewed (21 of 42), the regeneration run by the lead (the
  agent's permission layer refused `UPDATE_GOLDENS`; a stale test binary
  from the agent's scratch worktree had to be rebuilt first — a shared
  target dir bakes the worktree's manifest path into golden tests),
  `scripts/gpu_tests.sh` and `./ci.sh` green after it. **What landed:**
  `render::Placement::{Dome {offset_y, movement_scale}, Object {offset}}`
  — a dome's translation is `eye + (0, (Offset − eye.y)·(1 − MovementScale)
  ·k, 0)` with the port's angle-invariant scale `k` and no horizontal lag
  (RE §3/§4 [V]); a sky object keeps `eye + offset`. `render::SkyMode::
  from_flags` (0x04 → blend, else 0x02 → alpha-test at the shader's 0.5
  (the PS2's AREF 0x60 is [D], noted, no second threshold), else opaque —
  RE §5 [V]) from the raw MTRL word the bake already writes as
  `extras.flags`, read by `assets::load_glb` beside the `DrawLayer`; six
  sky pipelines (`[fogged] × [opaque, mask, blend]`, two extra fragment
  entry points); depth unchanged. Tests: the dome formula (68 / 100 / 0.995
  / k 50.25 → 8.04 m, ms 1.0 → 0), the MUS1 edge at −4.9° (was +0.63°), the
  mode table, a GPU referee (one half-transparent shell painted / tested /
  composited), a GLB round-trip carrying both `alphaMode` and flags.
  **Sim untouched:** `--sim-hash 600` 9f8c6f7dccc24191, walk_demo
  1c1959ed3fd15936, aim_demo 45dea6557b35f773, both `motion.csv`
  byte-identical, the sweep byte-identical to Phase B's. **The seam
  (the lead's row-jump instrument):** at the ground eye no seam in frame
  before or after; at the gallery's orbital overview it moves from row 87
  to row 174 of 720 — the asset's underside, still visible from a camera
  the PS2 never has. **Per map:** every parallax shell drops to the
  horizon (Cloud City and its cloud shells, Kamino's city ring 80 m, Naboo's
  waterfalls and mountains −40 m, Tatooine's rock spires 100/110, Endor's
  satellite/trees/Death Star 95–130, Felucia's tree line 190 with ms 1.0 —
  the ELF gives that Offset no path at all, Hoth's mountains 80 and cannon
  28, Geonosis's arena/spire/rim, Rhen Var's clouds); all capital-ship
  flights and most base domes paint opaque (TAT1's four flights carry flags
  0x0/0x1); Hoth's sky is the dome's own blue with mountains instead of a
  white composite (whether the PS2 fogs the base dome stays HOLD 10(a));
  Mygeeto's mountains (ALPHA RULE v2's rule-1 texture) survive MASK at 0.5
  and read more solid than the blend did. BF1 NAB1 `sky_water` is 0x411
  (bits 0x400/0x10 unmodelled → opaque). 21 maps whose only sky is a
  zero-offset opaque dome are bit-identical. Tier-4:
  `engine/artifacts/sky_placement/` (MUS1 ground/overview before/after, all
  sheets with a per-golden "what moved it" column, the 34-dir shell census).
  Not built (RE §8): `RotationSpeed` (the bake does not parse it) and
  `Reflect`.

- 2026-09-03 (Fable session 12): **The combined sky / fog / alpha RE — item 1 of the maintainer's order — is closed at
  the ELF level, four questions in one parallel pass (four Fable agents, every load-bearing citation re-read by
  the lead; `analysis/re-sky-pass.md`, `re-fog.md`, `re-material-flags.md`, `re-texture-wrap.md`; RE_LOG
  entries of the same date).** (a) **The sky pass:** `cam+0x140`/`+0x90` is the camera's ZOOM divisor, not a
  world scale; one projection per frame (near = camera near, far = 10000 in `.data`); the far-scene range
  `[80·S, 2000]` reaches only CPU culls; every sky draw takes TEST 0x51001 (Z-test, NO Z write) into a 16-bit
  far target sharing the main Z, composited over the near scene by a depth LUT — so "shells scaled about the
  eye, depth off, drawn first" reproduces the PS2 angles and ordering and the session-11 placement is no longer
  PROVISIONAL on that count. CORRECTION to session 11: under the sky flag a material with bit 0x02 BLENDS
  (ALPHA 0x44), it is not alpha-tested — `mus1_sky_volcano` and `myg1_sky_mountains_01` composite on the PS2
  (built on `sky-wrap` as an addendum before its goldens). (b) **Fog (HOLD 10(a) answered):** not GS fog but a
  second VU1 program (`fogspread`) with a linear per-vertex ramp; BF2 enables it iff `SkyInfo.FogRange` has
  near < far (alpha ignored, black legal), far-scene draws only, never the sky; BF1 NEVER PARSES
  `SkyInfo.FogColor/FogRange/FogRamp` — the placeholder maps were dead data all along — and fogs the world from
  `LowResTerrain.FogColor/FogNear/FogFar` with the colour's 4th component stretching the far distance
  (`far' = near + (far − near)·255/a`), models in both scenes, terrain in the far scene, never sky. Both Hoths
  fog (203,201,203) 100..1000 and fog their sky on neither game — the port's session-5 parallax-shell fog is
  the deviation that made BF2 Hoth's mountains white. ONE rule per game replaces `fog_from`'s three
  heuristics (brief next). (c) **The MTRL flag word (HOLD 10(g), CPU level):** `FUN_0058ed70` is the general
  per-segment draw for every `RedModel` in BF2 (36 callers) and tests the on-disc word verbatim: 0x04 (or
  colour alpha ≠ 0xff) → blend 0x44 without Z write, else 0x02 → alpha-test at AREF 0x60 (= 96/255), else
  opaque; 0x40 additive, 0x08 half-alpha, 0x80 specular pass, 0x100 env pass, 0x0f000000 UV animation; BF2
  drops `RTYP` entirely while BF1 picks one of four segment CLASSES from the flag bits (0x20 Specular, 0x400
  Scroll, else Normal) and only the Normal class honours 0x02 (with AFAIL FB_ONLY) or 0x04. Texture alpha
  always reaches the blend (TCC = RGBA, TFX = MODULATE). **It contradicts ALPHA RULE v2 on the Kamino crate
  (ELF blends it by its grime alpha), the Theed banners / class D (ELF opaque, alpha ignored) and Mygeeto's
  mountains (ELF tests at 96/255); it agrees on leaves, bamboo, fountains, mist, the Scroll/Specmap strips.**
  Read literally it ALSO contradicts what the PS2 is known to show (Endor's leaves as solid cards under BF1's
  FB_ONLY test without ABE), and the GS's 0x80-is-opaque alpha arithmetic is unreconciled with the discs'
  full-range CLUT alpha (lead: 2,156 of 2,182 baked textures max at 255) — the VU1 side or the CLUT upload
  still hides something. **Decision (lead, per the maintainer's decision 3): ALPHA RULE v2 stands; the ELF rule is
  recorded, not adopted; the reconciliation needs a GS-level ground truth (a PCSX2 GS dump of END1/KAM1/NAB2,
  HOLD 7 — the maintainer's rig). Brought to the maintainer below.** (d) **Wrap:** GLOBAL REPEAT for every model draw in both
  games (the CLAMP_1 data is a texture-record qword the loader zeroes unconditionally; `tex_` carries no wrap);
  the PS2 samples Hoth's v = 1.001 ring wrapped too. The maintainer's V clamp (item 0) is therefore a DELIBERATE
  DEVIATION for the eye, per segment where the UVs do not tile (lead's census `engine/artifacts/sky_wrap/
  dome_uv.txt`: POL1's and TAN1's base domes tile in V and stay REPEAT). **For the maintainer:** (1) the alpha rule —
  keep v2 (the eye's rule) until a GS dump exists, or A/B the ELF rule behind a bake flag now on the crate /
  banners / Mygeeto and pick by eye? Recommendation: keep v2, build the A/B flag only when the rig exists.
  (2) the fog rule lands next with the two Hoths side by side; expect BF1 maps to GAIN fog (BES1 gold-brown,
  GEO1 orange, TAT1/TAT2 pale haze, KAS1/KAM1 black, KAS2 a faint red horizon — the disc's own numbers) and
  the sky shells to lose it everywhere.

## Part 2 — HANDOFF-OPUS.md, the superseded generations



## What next — value order (2026-09-09 evening, after session 18 — the loop is RUNNING; the paste-in prompt for the next Fable window is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
2. **Fable RE left (unchanged from s17, plus one):** the **`modl` LOD switch rule** (HOLD 20 — GEO1's wreck; the material path is fully [V] and does NOT explain it: `analysis/re-vu1-cull.md` §4 carries the two-station PCSX2 capture note for the maintainer, which settles it); the **water VU1 microcode** (wave displacement, the LUT lookup, per-vertex alpha) and **KAM1's FFT ocean** (HOLD 21, `analysis/re-water.md` §8 — whether to build the ocean at all is a design call); **P7 vehicles**, which is NEXT now that Phase D is complete (the broadphase's `bodies` field goes private with it); **B1** (`Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change, lead-run, with a re-baseline; phase-d 1a pinned `--sim-hash 100` = `17e1b946ff0e821c` in a test that says a failure is a lead decision); BES1's ring — whether the PS2 draws an effect into it (HOLD 4, low value). **NEW (2026-09-09 evening, 17B's observation):** whether the AI-shed loop in `tick_bleed` is reachable from bleed at all — 17B measured `max overhang [0, -427]` over its configuration and argues from the two gates that it cannot fire (`apply_bleed` needs `alive < tickets` to arm, and one tick at the Elite handicap moves the pool by ~0.014), but it is not proved in general; if it truly is unreachable, the port pays for a safety net on every tick of every round. A lead call, not a queue item.
3. **Decisions open for the maintainer:** the **PCSX2 GS dump** — three questions: (a) the D-pad menu's second material layer (the 0x89 / PRIM 0x58 / TEST 0x51001 entry passed whenever a texture is bound — does the microcode draw it), (b) the wreck's LOD at the golden's ~475 m framing (which `modl` is kicked), (c) the far water fog pass's look; the alpha-rule reconciliation stays CLOSED (`analysis/re-hud-icons.md` citation 3, the `pal_` loader's `(A·0x81)>>8` CLUT scale). **NEW 2026-09-09 evening — BF2 NAB2's black distance:** item 6's fade takes a BF2 map with no far layer to the SKY colour, and `sky_naboo2` is black on the disc (HOLD 10(c)), so Theed's distance now fades to black. It is faithful; whether the port keeps it is the maintainer's eye (`bf2_nab2` is one of the 11 regenerated goldens, `lead_tmp/s18/sheets_view/`). **Whether tier 2 ships on disk at all** for a build the maintainer plays (+388 MB for the maps, +4.9 MB for the side textures; T2R and T2S are built either way — the toggle is the question). **Tier-4 checks:** the squad README + captures (Phase D's deliverable for the maintainer), and the Elite feel check (D12's reinforcement bias, declined until then).

## State at handoff (2026-09-09 evening, Fable session 18 COMPLETED — the nine parked branches are landed, the rebake and the goldens are done, the sweep is re-baselined, the queue is refilled and the loop is RUNNING)
- **Process rules learned (keep — the cumulative list):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before removing it (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout, and a BACKGROUND Bash call resets the harness cwd the same way (s17); a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped; view a regenerated golden as a COMMITTED-vs-REGENERATED pair (`golden_pairs.py`), never the new image alone, and **quantify its diff (mean / max / changed-pixel fraction + bbox) BEFORE viewing it**; a MAP golden can move when only the SIDE bake changed (s17) — and when only the WATER bake changed (s18); two [V] facts from two different RE agents can COMPOSE into a port rule neither of them stated (s17); a paused loop never sees STOP — kill it (AUTOLOOP.md); after any edit to `autoloop.sh` the live lanes are stale — kill and relaunch; launch the loop from a plain terminal or with every `CLAUDE*` env var stripped and run `--probe` first; after ANY queue generation check that `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists every item; **never `pkill -f` a pattern that is in your own command line**. **New this window (s18):** `cmp_bake.sh` EXITS 1 whenever DIFF≠0 — an expected-family diff is not a failure, read the families not the exit code; a branch that re-baselines a gate lands LAST, so every earlier branch still measures against the old baseline; and a re-baselining item should ship the escape hatch that reproduces the old value (`SWEEP_SLOT0=player`) so the move can be proved inert.


## What next — value order (2026-09-09, after session 17 — the loop is STOPPED and the maintainer relaunches it; the paste-in prompt for the next Fable window is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
2. **Fable RE left:** the **`modl` LOD switch rule** (HOLD 20 — GEO1's wreck; the material path is now fully [V] and does NOT explain it, so the LOD-distance rule is the candidate: `analysis/re-vu1-cull.md` §4 carries the two-station PCSX2 capture note for the maintainer, which settles it); the **water VU1 microcode** (wave displacement, the LUT lookup, per-vertex alpha) and **KAM1's FFT ocean** (HOLD 21, `analysis/re-water.md` §8 — whether to build the ocean at all is a design call); **P7 vehicles**, which is NEXT now that Phase D is complete (the broadphase's `bodies` field goes private with it); **B1** (`Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change, lead-run, with a re-baseline; phase-d 1a pinned `--sim-hash 100` = `17e1b946ff0e821c` in a test that says a failure is a lead decision); BES1's ring — whether the PS2 draws an effect into it (HOLD 4, low value).

## State at handoff (2026-09-09, Fable session 17 COMPLETED — the seven parked branches are landed, Phase D is COMPLETE, the rebake and the goldens are done, the queue is refilled and the loop is STOPPED for the maintainer to relaunch)
- **Process rules learned (keep — the cumulative list):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before removing it (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout, **and a BACKGROUND Bash call resets the harness cwd the same way (s17)**; a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped; view a regenerated golden as a COMMITTED-vs-REGENERATED pair (`golden_pairs.py`), never the new image alone; a paused loop never sees STOP — kill it (AUTOLOOP.md); after any edit to `autoloop.sh` the live lanes are stale — kill and relaunch; launch the loop from a plain terminal or with every `CLAUDE*` env var stripped and run `--probe` first; after ANY queue generation check that `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists every item; **never `pkill -f` a pattern that is in your own command line**. **New this window (s17):** a MAP golden can move when only the SIDE bake changed — the map sheet's right-hand panel draws a side unit, so a [REBAKE] side change is a [GOLDENS] map change too; **quantify a moved golden's diff (mean / max / changed-pixel fraction + bbox) BEFORE viewing it** — that is what tells noise from a real move; and two [V] facts from two different RE agents can COMPOSE into a port rule neither of them stated (the CLUT scale × the AREF → queue item AT) — the lead writes the composition as [D] with both citations.


## What next — value order (2026-09-08 late night, after session 16 — the loop runs it; the paste-in prompt for the next Fable window is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
2. **Fable RE left:** water (HOLD 10(f)/19(c): the water pass takes no fog; the level vs the authored floors — YAV1's plaza, KAS2's beach post; the flat sea gradient). **The maintainer's look at the regenerated goldens (2026-09-08 late night):** the KAS2 red horizon band, KAM1's black band and END1's dome trees over a flat grey plane are the RECORDED defects 1/3 (queue 5 — a lane is building it now — then 6; KAM1's flat sea and the water half of the KAS2 band are the water RE above). NEW: BF2 GEO1's Techno Union wreck reads as incomplete — the forward hull draws as bare rafters. Measured: the geometry is fully baked (`segdump GEO1.LVL geo_bldg_technounion_hull` 2645/2645 indices over 8 segments, LOWD 895/895), four segments are MASK by alpha rule 3 (`geo_bldg_wrecked_02`, `geo_bldg_rafters`, a0 0.21–0.26) and every segment is single-sided; whether the PS2 alpha-tests that plating and draws it two-sided is the alpha-rule question (HOLD 10(g)) — Fable's, and the PCSX2 GS dump would settle it. The `PrimaryStance` question is CLOSED and BUILT (phase-d 0b); the RE agent's three un-re-read side findings (re-stance-roll §3.2's swapped blocks, BF1 `FUN_002450b8`, BF2 `FUN_002d53c8`) are PROVISIONAL and cheap to verify when next in the stance code; BF1's `SecondaryStance` setter stores `v & 7` with no `v > 0` guard (RE §2; no disc authors 8 — the PORT GUARD at `stance_mask` is labelled PROVISIONAL). BES1's ring — whether the PS2 draws an effect into it (HOLD 4, low value).







## State at handoff (2026-09-08, Fable session 14 CLOSED — everything parked is landed; the loop queue is refilled; nothing is in flight)
- **RE this session (lead-verified, RE_LOG 2026-09-08 ×3):** `analysis/re-ai-sniper.md` (the float floor/cap; the sniper table — HOLD 19(e)/(g) ANSWERED; `re-ai-difficulty.md` §4.1 and `re-blaster-damage.md` §4.3 corrected; `brief_phase_c.md` LEAD ADDENDUM 4), `analysis/re-lowr.md` (HOLD 10(i)/19(f) ANSWERED — a same-footprint far LOD; complements item 5; `brief_lowr.md`), `analysis/re-terrain-cutters.md` (HOLD 19(a) ANSWERED — the `PTCH` replacement mesh; `brief_terrain_patch.md`). `re-fog.md` §2.2's `FUN_0048b9c0` row relabelled (water low-res pass). `walkabout_defects/README.md` carries the correction.



## State at handoff (2026-09-03, Fable session 12 CLOSED — items 0 and 1 landed; three builds parked unmerged)
- **Session 12 closed (the maintainer, context full):** all five agents finished; their reports are saved VERBATIM under
  `engine/artifacts/lead_tmp/s12/reports/` (layer_content.md, fog_rule.md, roster.md, droideka_NOTE.md,
  sound_global_NOTE.md); branches `layer-content` / `fog-rule` / `roster` are UNMERGED and NOT lead-verified;
  the droideka and sound RE reports (`lead_tmp/s12/re/`) are NOT lead-verified. `NEXT_SESSION_PROMPT.md` is the
  handoff.

## State at handoff — earlier (2026-09-01 after Fable session 7)















## What next — value order, with the Fable/Opus call made (2026-09-01, after session 7 — superseded above, kept for the item text)

1. **AI base — DECIDED 2026-09-01 (the maintainer, Fable session 8): openbf's OWN AI**
   (PLAN Log "AI decision"): modern architecture on the discs' planning
   graphs/hints/barriers, four difficulties on a three-layer model, squad
   commands (follow/hold/disperse/clear, ≤ 6 bots within 20 m). Phase A
   (navigation) is Fable-designed; the formats are being pinned (RE_LOG
   "`plan` / `Hint` / `BARR`"). Do NOT tune `core::ai` on Opus; each
   implementation step is released as a spec when ready.


## NEWLY OPUS-SAFE (Fable, 2026-09-01, session 7)

6. **Single-file game build (the maintainer, 2026-09-01 — personal-use only, never
   shared or committed).** Two commits: (a) resolve the default `--assets`,
   `--missions` and `--classes` paths relative to `std::env::current_exe()`
   (falling back to the current defaults when the exe-relative dirs are
   absent), so the exe runs from any directory next to its data; (b)
   `scripts/pack.sh`: build release, `strip` a COPY of the binary (the
   workspace profile keeps `debug = true` — do not change it), append one
   uncompressed archive of `assets_baked/` + `missions/` + a 16-byte footer
   `[magic][u64 pack offset]`, and teach `game::assets` to open its own exe,
   check the footer, and read assets from the embedded archive when present
   (directories always win when both exist). No new dependencies unless a
   tiny archive format needs one — a homemade `[path len][path][u64 len]
   [bytes]` table is fine and keeps mmap-friendly offsets. Acceptance: the
   packed file copied to /tmp launches the shell and loads BF1 KAM1; ci
   green; the pack script refuses to run if the output would land anywhere
   inside a git-tracked path.




## Part 3 — PLAN.md Log, 2026-09-07 to 2026-09-09: the Fable landing windows (sessions 13–19)

- 2026-09-07 (Fable session 13, closing): **CORRECTION to the defect-hunt entry above, and two RE reports parked
  NOT lead-verified.** The Hoth-terrain RE agent (`lead_tmp/s13/re/hoth_terrain.md`) found that the lead's "no
  terrain cutter within 296 m of either room" and "17/35 cutters with a NEGATIVE sphere radius" were the LEAD's
  misread of the manifest: `terrain_cutters[].sphere` is stored RADIUS-FIRST `(r, cx, cy, cz)` — exactly as
  `mungefs/terrain.rs` decodes the disc's `CUTR INFO` (`u32 planeCount + f32[4]`) and as `core::collision::
  is_cut_by` reads it (`c.sphere[0]` = r); the lead's containment script read `(cx, cy, cz, r)`. Re-read that way:
  0 cutters have r ≤ 0 on any map, and BOTH outpost rooms sit inside two cutters each on BOTH discs (BF1 40/41 at cp3,
  14/15 at cp4; BF2 11/12, 2/3; the `hoth_bldg_bunker` model's volumes). So the SIM already cuts there — the defect
  is the RENDER: the agent reports [V, cited loaders BF1 `FUN_00481608` 546287–546333 / BF2 `FUN_0056bbf0`
  703562–703628, builders 548317–548700 / 705673–706065] that the PS2 never tests cutters when drawing; affected 8×8
  patches ship a PRE-CLIPPED replacement mesh inside their `PTCH` record (`INFO` flag + `STRP`/`POSI`/`COLR`/`ALPH`;
  BF1 HOT1 patches 588 and 490 under the two rooms carry it, their open-snow neighbours do not; 23 patches on BF1
  HOT1, 30 on BF2 HOT1, 1–16 on eleven other terrains, 0 on nineteen) and `bake/src/terrain_glb.rs` emits the full
  procedural grid instead. NOT closed by the agent: whether the replacement mesh removes the +4.8 m crest (the
  cutter's cap is +0.41 m). The LOWR RE agent (`lead_tmp/s13/re/lowr.md`) reports [V, BF1 `FUN_00481608`
  546096–546279 / BF2 `FUN_0056bbf0` 703386–703534] that `LOWR` is a coarse TRIANGLE-STRIP MESH of the terrain
  (`INFO` grid/res/extent/ymin/ymax; per `LPTC` a box, u16 `POSI`, u16 `STRP` with bit 15 = restart; planar UVs
  tiled by key 0x6d9b436e), drawn in BF1's far pass iff `MaxDistance > 0` (default 2000 m when absent), Z-tested and
  written, fog pass always appended, never sky; absent `LOWR` draws nothing; and that BF2 has the SAME mechanism and
  keys (the lead's premise that BF2 lacks the scope was wrong: BF2 END1/GEO1/HOT1/KAS2/POL1/TAT2/YAV1 author it).
  NOT closed: whether the sheet's extent equals the terrain width (a far LOD) or exceeds it (an outside fill) — the
  decisive census. Both reports are the next window's first RE verification; HOLD 19(a)(b) is rewritten
  accordingly in HANDOFF. Phase C: see HANDOFF §State for where its build stopped.

- **2026-09-08 — Fable session 14: the four parked branches landed (the gate values moved), three RE answers lead-verified (the aim-error floor is a FLOAT; the sniper table decoded; LOWR is a same-footprint far LOD; the Hoth `PTCH` mesh removes the crest), the loop queue refilled.** the maintainer's rule for this window: Fable does only what needs Fable; everything with a clear plan goes to the autoloop.
  **Landed on main (an Opus lander under the lead's brief `lead_tmp/s14/briefs/landing.md`; report `lead_tmp/s14/reports/landing.md`; ci logs `lead_tmp/s14/logs/`):** `cleanse-render` 0e36b15+70d7dd1, `cleanse-game-tests` 3664198+39c0265, `cleanse-mungefs` b6015c6…8884e42 (six), `phase-c` commit 1 658c6af (D1+D2+D3, `--difficulty` + the Skill vector, no behaviour change), then `Scripts: sim_gates.sh` 3f8d5c2 and `scratch_bake.sh` + `cmp_bake.sh` (the byte-identical bake gate CLEANSE_PLAN Appendix 1 said was missing; `scratch_bake.sh` refuses an OUT under `assets_baked`). Each branch: rebased, ci exit 0 in its worktree AND on main (one ci at a time), fast-forwarded; `gpu_tests.sh` 0 with no golden moved. **Lead-reproduced on main:** the sweep pair `cmp` exit 0 INCLUDING the hash column; footer `finished 25/32  stuck_max median 1200 (finished-only 900)  >10k 5 maps  sum 241740`; per-map lines identical to `s13/sweep_final1.txt` once the hash is stripped and the four-part filter (`' done ('`, `^roster `, `mix team`, `^mission `) is applied to both sides; `scripts/sim_gates.sh` → walk_demo 8d9d661ebcdd5245 (407), aim_demo c12fdeb33d06ce48 (120), `--sim-hash 600` 537c1c6235c1f47e, both `motion.csv` byte-identical to the pre-phase-c motion (`lead_tmp/s14/lead_gates/`). **BASELINES NOW = `lead_tmp/s14/baselines/SUMMARY.txt`**; the s13 values (43d2…/242f…/20fd…) are superseded — the hash FORMAT grew by the difficulty byte and `Unit.skill`, the trajectories did not move. Worktrees and branches removed (the `cleanse-audit` branch had 0 commits); the per-branch target dirs (~200 GB under `lead_tmp/*_target`) are left for C2's `scripts/cleanse_delete.sh`. **Cleanup done 2026-09-08 (the maintainer's go):** `scripts/cleanse_delete.sh` (e18ed6b; dry-run by default, `--yes` deletes; re-runnable — it recomputes dead `*_target` dirs from git state) removed 36 paths, 179 GB logical (~57 GB physical; the filesystem compresses). C2's §F2 job is therefore done; C2 keeps §F1 (`git rm`) and the docs.
  **RE, lead-verified (analysis/RE_LOG.md 2026-09-08 ×3):** (1) `analysis/re-ai-sniper.md` — HOLD 19(g): `FUN_002a2df8` has NO integer conversion; Ghidra's *(decompiler detail omitted)* is the EE `max.s` (word 46010028 at 0x2a2e28, read from the ELF by the lead), so the error floor is `max(0, lerp(1.6, 0.4))` as a float — 1.6 m at skill 1, 0.97 at 11, 0.59 at 17, 0.4 at 20 — and the cap `max(0, lerp(2.2, 1.2))` likewise; the consumer clamps to [floor, 2.0] (B2:233314–233320), no-target floor 1.5 m. `re-ai-difficulty.md` §4.1's "1 m below skill 12, then 0" and "cap 2 m / 1 m" were WRONG (corrected today), and so is Phase C commit 1's `int(lerp)` — fixed by Phase C addendum 4 in commit 3. HOLD 19(e): the AI sniper distance table is fully decoded on both games — predicate `YawSpread == 0 && AI shooter && TriggerSingle && ZoomMax >= 6.0` (inclusive; `imp_weap_inf_sniperrifle` is exactly 6.0) with a current target; a step table on the SQUARED distance (400/1600/3600/6400/10000/14400 → 0.0997/0.0499/0.0333/0.025/0.02/0.0167 else 0.0143 rad) × the skill multiplier, ADDED in the YawSpread slot (`max(s,0)` is not replaced — `re-blaster-damage.md` §4.3 corrected); `ZoomMax` = fnv1a("zoommax") 0x2a96e8b4, loader coerces 0 → 1.0, ctor default 1.0 (all re-read by the lead). D9 is UN-STRUCK: `brief_phase_c.md` LEAD ADDENDUM 4 carries keys, predicate, table, insertion point, tests, and the lead's D10 ruling (`err` persists across targets; a target-less tick clamps it into [1.5, 2.0]). (2) `analysis/re-lowr.md` — HOLD 10(i)/19(f): the session-13 draft had 25/31 claims right and 5 WRONG — chiefly "the real terrain is also drawn in the far pass": `FUN_0048b1e0` is the WATER's low-res pass (WATR/WMAP → `DAT_005648dc`; `re-fog.md` §2.2 relabelled). The sheet is the ONLY terrain either PS2 draws past `NearSceneRange[2]`, to `MaxDistance` (default 2000 both games; zeroed by every tern load, so BF2 DAG1/NAB2 ship one and never draw it). Census (lead re-run identical): 16/32 terrains carry `LOWR`, extent/W = 1.000 on all — a same-footprint far LOD, not an outside fill; 6 of the 9 razor-horizon defect maps have NO sheet, so the razor-horizon group's fix is queue item 5's far cull (terrain clip at `NearSceneRange[2]`, PROVISIONAL — the far-plane line was read, not traced) plus item 6's fade; LOWR complements it (queue item L, `brief_lowr.md`). (3) `analysis/re-terrain-cutters.md` — HOLD 19(a): 18/18 claims verified; the flagged `PTCH` replacement mesh is the heightfield clipped exactly by the cutter's 3-D volume (lead re-run byte-identical): holes at BF1/BF2 cp3 and BF2 cp4, BF1 cp4 covered 1.72 m BELOW the floor; in-room crest +6.8/+7.9 m above the floors today. The bake's only error is emitting the flagged patches' quads → queue item H (`brief_terrain_patch.md`, one bake commit, [REBAKE], HOT1 goldens move; 13 terrains carry flagged patches). Not re-read by the lead: the BF1-side lines of the sniper RE, the LOWR loader/format lines, the Hoth loader/builder lines (the session-14 Fable verifier's, cited in each header).
  **Decisions (lead):** Phase C commits 2–5 become a multi-session LOOP item (PC) rather than a lead-serialised build — the brief + addenda are complete and the gates are mechanical; Phase D (PD) follows it in the queue; `bankdiag.rs` is deleted by C2; the `net` crate stays as a placeholder; item 3 also fixes M-B2 (ci writes `assets_baked/tat1/sounds/names.ron`) and T1 (silent golden skips → fail); item 16 makes the bake byte-deterministic (M-B3/M-B4). The loop prompt gained the `RESUME FROM` rule, `sim_gates.sh`, the scratch-bake gate and the M-B2 note. **Not done:** water (HOLD 10(f)/19(c)) — the remaining Fable RE; no walkabout regeneration (nothing visual landed). **Process:** a harness hook denied a heredoc whose TEXT mentioned worktree-removal/rebase-abort commands (the destructive-op guard, not `claude_guard.sh`) — briefs go through the Write tool; the LOWR verifier could not write its own report — saved verbatim by the lead (`lead_tmp/s14/re/REPORT_lowr_hoth.md`). Evidence index: `lead_tmp/s14/`.

- **2026-09-08 (evening) — Fable session 15: the ten branches the two-lane loop parked are LANDED; Phase C is COMPLETE; the gate values moved by design; the queue is refilled with a drained-queue backoff; the bloat is gone.** the maintainer's rule held: Fable did only what needs Fable — two Opus landers (briefs `lead_tmp/s15/briefs/landing_{A,B}.md`, reports `lead_tmp/s15/reports/landing_{A,B}.md`, ci logs `lead_tmp/s15/logs/`) did the mechanics; the lead read every loop report, read C2's whole diff, reproduced the load-bearing numbers on main, and took the decisions. A Fable RE agent answered the one RE question (below).
  **Landed on main, in this order — each rebased onto main in its worktree, ci exit 0 there AND on main (one ci at a time under `flock engine/artifacts/loop/ci.lock`), fast-forwarded, worktree and branch removed:** `cleanse-docs` (C2, 13 commits, 4fcc3ca — the lead's whole-diff check: every non-blank line removed from PLAN.md/HANDOFF is in the new file or verbatim in `analysis/PLAN_LOG_2026.md` — 4,175 of its 4,281 lines are contiguous runs of the originals and the 71 others are its own headers and pointers — bar the deliberate rewrites: §WHERE THINGS STAND, the droideka line, the parry line, the topic index, HOLD 14, two pointers; decisions: `scripts/scan_tables.py` KEPT (the only path from an ELF back to the binding tables), HANDOFF's second copy of the baselines is now a pointer to PLAN §WHERE THINGS STAND, HOLD 14 pruned to a one-liner); `cleanse-bake` (7, 8841778 — the lander's FRESH scratch bake from the rebased branch: 15,340 of 15,375 files byte-identical, the 35 others all `sounds/names.ron` + `audio/*.ogg`, the M-B3/M-B4 nondeterministic family; decisions: B-B3's two `baked_root` definitions STAY (the shipped binary must not link `bake`), B-R7 and B-M1 stay open in the C3 entry); `cleanse-core-nav` (7, 88f9430; `226f31a` `ron` → dev-dependency KEPT; its bug B-CN1 → queue item 17); `phase-c2` (12, fc077bb — **PHASE C COMPLETE**: commit 2 perception (contact memory, cone + view distance + ordnance LOS, reaction, hearing, target choice), 2a the FLOAT error floor/cap (addendum 4a), 3a the aim tracker + error radius, 3b the AI sniper distance table (addendum 4b — the `ZoomMax >= 6 && YawSpread == 0 && TriggerSingle` predicate; measured over the sweep: 39,235 of 1,411,120 AI fire presses came from a table class, exactly three distinct values each reproducing table × the skill multiplier, no sniper shot past 60 m), 3c fire discipline (bursts, the pause, charge weapons, dodge-on-hit), 4 hints in combat (`PrimaryStance` → a stance wish, the `ROLL_STANCE` tap controller — 18.6 % of think-ticks carry a wish, 0.3 % of unit-ticks give up movement to stand, 11,368 presses in the whole sweep), 5a D12 (bot-on-human damage × `human_damage`, the Elite ×1.25 bleed against the lone human's side), 5b the arena (`game --example arena`); every gate moved by design and is re-baselined below); `walkabout-rgb` (2, 56fa51a — RGB walkabout PNGs (219/219 lost their alpha plane, 0 differ in RGB; the set is 1.1 GB → 880 MB) and the 25° vantage pitch cap: the floor lifted exactly two vantages, COR1 stays inside the cap, UTA1 goes 31.8° → 25.0° with visible sky 15.0 → 23.0 %; ONE golden moved, `goldens/maps/bf2_uta1.png`, regenerated by the lead after viewing the parked sheet; decision: the sinkhole rim standing above UTA1's horizon is geometry, not a pitch defect — no new item); `test-scratch-dirs` (2, c8cccf2 — 12 of the 18 fixed temp-dir names carry the pid, T1's two tests fail loudly instead of skipping; PARTIAL: the 5 files parked branches owned are its resume, item 3; decision: extend T1 to the map goldens too); `bes1-sliver` (1, 15f239d — **item 8 CLOSED as "reproduces the disc"**: `bes1_bldg_main02` has an open ring between its ceiling slab (top 106.97) and the parapet over an outdoor ledge (103.50), 5,492 of 5,492 declared indices consumed, the 65 skipped instances are all `com_*` markers, the collision has the same opening; the lead re-ran the branch's `segdump` example on four LVLs — BF1 BES1 37, BF1 HOT1 55, BF2 TAT2 90, BF2 HOT1 101 models, 0 mismatches — and corrected `analysis/a2-bake-formats.md` §Models: the LAST `modl INFO` u32 is the model's total index count, not a face count, and the +60 word `face_count` reads is unidentified and unconsumed); `mat-invest` (0 commits, worktree removed — item 10 all three ANSWERED, `engine/artifacts/mat_invest/README.md`: (a) BF1 YAV1's ribbon quads are `yav_bldg_ruins_column2` seg1, MTRL 0x31 RTYP `Specmap`, ALPHA RULE v2 rule 2 → OPAQUE, which IS the PS2's base pass — the port lacks the additive specular pass `re-material-flags.md` §4 [V] describes; 84 primitives / 25 textures port-wide → folded into item 9; (b) the RHN streaks are in the texture (V spans 0.00–0.81, anisotropy 1.2, no slot dropped — the crisp neighbour is detail-compressed); (c) DEA1's red grid is a second surface, one flat texel RGBA (189,45,46,0) under the MASKed grating, red on the disc too); `parry-determinism` (1 + Cargo.lock, 120e7e0 — the three lock edges read: parry3d +indexmap, simba +libm, indexmap +serde/serde_core; nothing moved on this box because libm's `sqrtf` IS `sqrtss`; the proof is one cross-OS run, backlog); `collision-broadphase` (3, 1c2eb89 — a 16 m XZ grid in CSR over the bodies' AABBs, a SUPERSET prefilter (every exact test still runs, ascending order kept, full-scan fallback when `bodies.len()` changes), cell size measured across five sizes; lead-reproduced TAT2 ai=64 sim tick mean 1.362 → 0.368 ms, p99 2.743 → 1.217 ms; the hazard — a body replaced or moved in place keeps the length, so the grid stays live — is P7's to close by making the field private).
  **Lead-reproduced on main `1c2eb89` — BASELINES NOW = `lead_tmp/s15/baselines/SUMMARY.txt` (§WHERE THINGS STAND carries the canonical copy):** the sweep pair `cmp`-identical raw INCLUDING the hash column (stdout captured alone); footer `sweep: finished 26/32  stuck_max median 900 (finished-only 540)  >10k 5 maps  sum 271740` — 25 → 26 of 32: BF1 KAM1 and BF2 UTA1 finish since commit 3c, BF1 HOT1 stopped finishing at commit 4 (team 0 down to the IDLE PLAYER alone — the artefact the maintainer's decision 2 removes, queue S0), the four Phase B standoffs (BF1 KAS2, BF2 KAM1/MUS1/POL1/TAN1 minus one) unchanged in kind; `scripts/sim_gates.sh` walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, both `motion.csv` byte-identical to the s14 (and pre-phase-c) trajectories — the hash FORMAT grew at commits 2, 3a, 3b, 3c and 4 and the trajectories never moved; the arena `flat 3000 42` twice identical (kills/min 45.49 / 122.33 / 166.89 / 200.00, hit rate 0.181 / 0.362 / 0.456 / 0.588, `monotonic: kills yes  hits yes`) and `tat2 9000 42` twice identical (kills/min 29.0 / 29.8 / 40.0 / 33.0 — NOT monotonic; hit rate 0.139 / 0.243 / 0.300 / 0.299; ttfk 18.5 / 17.3 / 15.4 / 14.5 s IS); Seeds 43 and 44 (lander A, `s15/logs/arena_{tat2,flat}_seed4{3,4}.txt`): the flat field is monotonic on kills and hits in all three seeds; tat2's hit rate flattens at ~0.30 for Hard and Elite in all three (kills/min monotonic on seed 44 only), and time-to-first-kill is monotonic on every seed and both fields. `scripts/gpu_tests.sh` 0 with the one designed golden moved and regenerated; ci 542 tests. **No rebake was due** — no landed branch changed a baked byte (cleanse-bake proved it; the others touch no bake file).
  **Decisions (lead):** (1) **`PrimaryStance` 3:** NOT prone — lead-verified RE (RE_LOG 2026-09-08 evening; `lead_tmp/s15/re/primary_stance.md`): the value is a BITMASK (bit0 stand, bit1 crouch, bit2 prone) and the PS2 draws one stance uniformly among the set bits at hold entry; BF2 masks `& 3` (its AI never prones — the go-prone slot is a stub), BF1 keeps the byte (prone only from 4–7). The port rule is the mask + a seeded draw, BF2-sourced maps `& 3`, BF1-sourced raw → Phase D commit 0b; Phase C's 1/2/3 mapping stands PROVISIONAL until then. (2) **D12's reinforcement bias: NOT built.** The Elite handicap is the ×1.25 bleed alone until the maintainer's Tier-4 Elite feel check says it is too weak; if it is ever built it is the bot-only side's reinforcement pool ×1.25 (PROVISIONAL) behind the same host option — a second lever on the same axis is not worth a visible ticket-count asymmetry today. (3) **tat2's non-monotonic kills/min: ACCEPTED** — one seed on a map where cover and post pressure compete with the tier; the flat field (the acceptance field) and time-to-first-kill are monotonic; seeds 43/44 confirm the shape — the MAP field caps Hard/Elite hit rate at ~0.30 (cover and post pressure), the tier table itself is monotonic; a Phase-D-era arena run on a second map is the next look, not a tuning now. (4) The `WeaponClass::bf1_rifle()` fixture keeps the disc's `zoom_max` 2.5 (addendum 4b's "1.0 in both constructors" applies to `Default` only); the fire-cycle carryover stays COUNTED (`NavStats.carry_shots`) — ending the cycle on trigger release is a weapon change that moves the hash. (5) The hash-test gap PC left (no per-field assertion for D9–D11's eleven hashed fields) is Phase D's commit 0. (6) B-CN1 — `Battle::on_death` drives `reinforcements` negative against [V] `re-tickets-spawn.md` and the raw field is hashed — is queue item 17: a designed hash-column move, `battle.rs` only, lead re-baselines. (7) **The additive pass:** ONE `BLEND_ADD` pipeline serves item 9's strips AND the Specular class's second draw (mask = the base texture's alpha, PROVISIONAL — the ELF samples a highlight texture; item 9 reads §3/§4 first). (8) T1 extends to `every_baked_map_matches_its_golden` (item 3's resume). (9) B-B3, B-R7, B-M1, C10 as written in the C3 entry.
  **Loop:** the drained-queue backoff — a session that finds no eligible item touches `engine/artifacts/loop/DRAINED` and exits (one journal line, no report; `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md`), every lane sleeps `PROBE_INTERVAL` while the file exists (`scripts/autoloop.sh`, replaced by rename while the lanes ran), the lead removes it at refill. Script fixes on main: `scripts/scratch_bake.sh` creates `classes/` (the `classes` group died on a scratch tree); `scripts/cleanse_delete.sh` Tier B now covers the branch scratch copies (`bake_after`, `before`, `c2_*`, `goldens_committed`, `nd`, `base`, `spot1..6`) and the superseded s13 scratch bake + pre-s13 walkabout set. **Cleanup (the maintainer, 2026-09-08 evening, "delete unnecessary files too — the folder is ~242 GB"):** `cleanse_delete.sh --yes` twice this window — 245 (28 + 74 + 143 GB logical, per the script's own du) GB in 49 paths (the dead per-branch cargo targets, the scratch bakes and copies); the repo is now 76 GB, of which the maintainer's inputs are iso 8.8, `PC_swbf2_exe` 9.6, `community_mods` 13, `extracted` 2.2, `tools` 2.0, and the live build `engine/target` 18 — nothing else large is regenerable. **Queue refilled** (`QUEUE.md`; the GATES block = the s15 values, `BASE` absolute, stdout-only sweeps): H → 15 → 16 → 11 → 5 → 6 → 9 (+ the additive pass) → L → PD (+ commit 0) → S0 (the slot-0 bot, after PD) → 17 → 13 → 4 → 7 → 3 (resume) → C3 row 6 (after PD/S0). The walkabout set regenerated at close (RGB, the cap): 219 images over 32 maps in 18.9 s, 889 MB, every PNG RGB, `INDEX.md` rebuilt; the vantage panels carry the 25° cap (`lead_tmp/s15/logs/walkabout_regen.log`). **Not done:** water (HOLD 10(f)/19(c)) — the Fable remainder; P7's vehicle RE. **Process:** the lander briefs carry the flock rule and the stdout/stderr split; a Fable-window command run inside a worktree moves the harness's cwd there — use absolute paths from the main checkout; the guard's `writes_leadfile` false positive (a lead-file name inside a sed replacement) is still open; `cleanse_delete.sh` is safe while landers run (it skips live names).

- **2026-09-09 — Fable session 17: the seven parked branches are LANDED and Phase D is COMPLETE (D5–D14); ONE rebake; 31 goldens regenerated after every pair was viewed; four Fable RE questions answered and verified; the queue refilled; NO gate value moved.** **The maintainer's new standing rule, for this and every future Fable window (recorded in HANDOFF §Rules and PLAN §Working mode):** Fable is used SPARINGLY — a Fable window does only what needs Fable (RE verification, decisions, viewing sheets and goldens, writing the briefs and the queue items), it is SHORT, it spawns Fable subagents for the Fable-only tasks and OPUS subagents for everything else (landers, doc patches, scripts, measurements), it keeps its own context small (it reads reports, never logs; subagents write to files and return one paragraph), and its product is a QUEUE full of disjoint, precisely gated items so that Opus can run autonomously through `scripts/autoloop.sh` for as long as possible — and **The maintainer starts the loops HIMSELF; a Fable session never launches `autoloop.sh`.**
  **Landed on main in queue order, each rebased in its worktree, ci exit 0 there and on main under `flock engine/artifacts/loop/ci.lock`, fast-forwarded** (Opus lander, brief lead_tmp/s17/briefs/landing.md, report lead_tmp/s17/reports/landing.md (170 lines), logs lead_tmp/s17/logs/; lead checked by grep: the eight ci totals, all eight sweeps raw-cmp-identical to s16/baselines/sweep_main_1.txt, all seven sim_gates logs green with the three s16 hashes and motion.csv identical, the four cmp_bake summary lines; one deviation: a PIPESTATUS=141 SIGPIPE misread on bake-ogg's merge, re-verified as exit 0):
  **`test-scratch-dirs-2` (3, 2 commits f050cb1/4df338e, `4df338e`)** — the last five fixed scratch names take the process id, and bug M-B2 is fixed: a ci run stops rewriting the shipped `tat1/sounds/names.ron`; +0 tests; a fresh scratch bake vs the shipped tree DIFF=1, that same `names.ron`.
  **The item-3 leftover `t1_map_patch.diff` (`fd2f02a`)** — committed directly on main by the lander after T2R landed: every committed map golden must be rendered.
  **`bake-ogg` (OGG, 2 commits 311b5f6/a1ff75f, `a1ff75f`)** — one Ogg page walk, in `bake::ogg`; the beep stops carrying its own; +0; scratch bake vs the shipped tree DIFF=1 (`names.ron`) — the three beep OGGs byte-identical.
  **`global-sound-runtime` (GS, 1 commit `35c6eed`)** — the runtime plays the streamed VO and the resident CORE bank; +4 (587).
  **`two-tier-runtime` (T2R, 2 commits 0d1a5ec/16a2e49, `16a2e49`)** — two-tier textures at runtime: tier 2 in play, tier 1 in the goldens (default `Pc`); +5 (592); `gpu_tests.sh` exit 0 with `OPENBF_TEXTURE_TIER=2` in the environment too — the golden tests force the PS2 tier by construction.
  **`two-tier-soldier` (T2S, 1 commit `51f7d80`)** — the side bake writes both texture tiers and the base is ALWAYS the disc's; +1 (593); scratch bake vs the shipped tree SAME=21547 DIFF=22 ONLY_IN_A=21 (the 21 `side_bf2` stems).
  **`far-scene-cull` (5, 4 commits 26ed37d/5005440/3a42452/b203619, `b203619`, rebased clean over the identical `OPENBF_WALKABOUT_OUT` hunk)** — the far-scene cull behind `OPENBF_FAR_CULL`, DEFAULT OFF (the mechanism only; the look is judged with item 6); +3 (596); scratch bake vs the shipped tree SAME=21515 DIFF=54 ONLY_IN_A=21 (`names.ron` + 21 side textures + 32 manifests, no `common.ron`).
  **`phase-d` (PD, 6 commits 31ccd5f..`cef083b` = D5–D14 — PHASE D IS COMPLETE)** — +19 (615); the sweep twice = s16, both arenas = s16, `--sim-hash 100` = `17e1b946ff0e821c`, the squad probe 32/32 zeros, `508/61/32 stance_want` with no Prone; the worktree and the branch are gone.
  ci across the window: 583 → 583 → 583 → 587 → 592 → 592 → 593 → 596 → 615. **No sim value moved on any branch** — measured by the lander per rebased branch and reproduced by the lead on final main. Worktrees at close: the seven branch worktrees + the two throwaway tsd2-b/tsd2-main; git worktree list = main alone; <repo parent>/openbf-wt/ empty. **The CODE SHA is main `cef083b`** (main before the window: `273ac43`).
  **The ONE rebake (`lead_tmp/s17/rebake_and_cmp.sh, log lead_tmp/s17/logs/rebake_and_cmp.out`):** the pre-rebake scratch bake of main is 21590 files; against the SHIPPED tree `SAME=21515 DIFF=54 ONLY_IN_A=21 ONLY_IN_B=0 — tat1/sounds/names.ron 1 + side_bf2/textures/*.ktx2 21 + */manifest.ron 32; ONLY_IN_A = 21 side_bf2 *.tier2.ktx2; nothing else`. After `scripts/rebake.sh` on main the same scratch bake vs the rebaked shipped tree is `SAME=21590 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0` over 21590 files — **`engine/assets_baked` IS a fresh bake of main `cef083b`**.
  **Goldens (lead_tmp/s17/goldens_regen.sh + golden_pairs.py (path-agnostic since s17)):** 31 moved and nothing else — 14 (bf2_cis_inf_{bdroid,magnaguard,sbdroid}, bf2_rep_inf_{arctrooper,clonecommander,ep3armoredpilot,ep3heavytrooper,ep3jettrooper,ep3sniper,ep3trooper,feluciatrooper,jettrooper,pilot,trooper}) — and 17 (the 16 BF2 maps + tat2) — NOT expected in advance: their pixel diff is one 70×197 px strip at x 1411–1481 / y 189–386 on every one, mean 0.006–0.018, 0.3–0.7 % of pixels = the map sheet's right-panel soldier close-up, a BF2 REP/CIS unit drawn with the side_bf2 textures T2S made the disc's; the BF1 maps (BF1 soldier) did not move. ONE cause (the disc's soldier textures); every pair viewed (lead_tmp/s17/sheets_view/golden_pairs_00..15.png + mapdiff_bf2_{cor1,hot1}.png): same geometry and poses, softer disc textures; committed as `1628519` by path; 90 goldens, 74 MB in `engine/goldens`.
  **Lead-reproduced on final main `1628519` — BASELINES NOW = `lead_tmp/s17/baselines/SUMMARY.txt` (§WHERE THINGS STAND carries the canonical copy), and every value is IDENTICAL to s16's — nothing moved this window (lander-measured per branch, lead-reproduced on final main):** `scripts/sim_gates.sh` walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, `--sim-hash 100` `17e1b946ff0e821c`, both motion.csv identical to s16 (walk 079c8dbca95a380d0fd91484b7f2d2d0, aim 492299f958b70d6dc956b6386950f765); the sweep pair `cmp`-identical raw INCLUDING the hash column and identical to s16's (pair 0, vs s16 0 (md5 bca8aedd215f0a2f3cba94869a4f45b2 = s16's)), footer `sweep: finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480`; `arena 3000 42 flat all` kills/min 45.49 / 122.33 / 166.89 / 200.00 (606262bc6865a689 / 09c7449831956e5d / b2f6343d64f93c51 / b8ab0f137168eb26), monotonic: kills yes  hits yes; pair 0, vs s16 0; `arena 9000 42 tat2 all` kills/min 27.40 / 37.20 / 34.60 / 36.60 (02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81), monotonic: kills no  hits no; pair 0, vs s16 0; `scripts/gpu_tests.sh` exit 0; ci 615 tests (lead_tmp/s17/final_gates.sh, log lead_tmp/s17/logs/final_gates.out). The walkabout set regenerated on the rebaked tree: 219 images / 32 maps in 20.3 s, 890.8 MB, 256 PNGs all RGB (tier 2 ON in the set).
  **The four Fable RE answers (four Fable subagents, each re-verified by the lead before it entered the Log; rulings `lead_tmp/s17/rulings.md`):**
  **(1) HUD icon compositing** (`analysis/re-hud-icons.md`) — lead-verified (the ALPHA 0x44/TEST 0x31001 stores, the (A*0x81)>>8 CLUT scale, the bake's keep_alpha): every HUD bitmap on both discs is a `RedBitmapElement` drawn source-over by its CLUT alpha (ALPHA 0x44, TEST 0x31001 — no cutout, no Z), the `pal_` loader scales every CLUT alpha `(A·0x81)>>8` at upload so disc 0xff IS the GS's 1.0, and the disc icons ARE cutouts that the BAKE forces opaque (`alpha::keep_alpha` returns false for any non-sky texture no MASK/BLEND draw names). The three hops the lead re-read: the BF1 `FUN_004bbe90` ALPHA/TEST stores (B:580213–580427), the CLUT scale at B:571177–571201, and `alpha.rs:336–342` → `map.rs:1010–1021`. → queue item SM (the squad marker on, [REBAKE]: an unreferenced texture keeps its disc alpha; the marker blends, no depth; ON by default). HOLD 4 unchanged by this one; the PS2's own placement of these sheets (a 2D D-pad command menu at HUD (−120,−50)) is recorded as the PROVISIONAL deviation.
  **(2) GEO1's wreck / the segment flags** (`analysis/re-vu1-cull.md`) — the CPU side agrees with re-material-flags §2 [V]; the VU1 cull reading is the agent's; no alpha-rule/cull change; the LOD switch rule is the new HOLD. MTRL `f&0x1` → prim 0x4000 = VU1 per-vertex lighting; `f&0x2` → TEST 0x5060b (alpha test GEQUAL AREF 0x60, Z written, no blend); `f&0x10000` suppresses `p|=0x800800`, where VU1 reads 0x800 = backface-cull enable and 0x800000 = cull-side flip — so 0x10000 = two-sided, the cull bits ride ONLY on 0x800800, and plain opaque segments are never culled on either console; the port's one pipeline is `cull_mode: None` (`render/lib.rs:1496`), already two-sided everywhere. **VERDICT: no ALPHA RULE v3 and no cull item from GEO1; the wreck's "bare rafters" is NOT in the material path.** The strongest remaining candidate is [H]: the port draws LOD0 forever while the LOWD's rafters texture is 100 % opaque, and the PS2's `modl` LOD switch rule is unread — **NEW HOLD FOR FABLE 20**, with a two-station PCSX2 capture note for the maintainer in `re-vu1-cull.md` §4.
  **(3) The three PROVISIONAL stance side findings** (`analysis/re-stance-side.md`) — A verified (labels swapped), B and C dead code (C's stub re-read in the raw ELF: 03e00008 0000102d); no port impact. A: `re-stance-roll.md` §3.2's two block labels ARE swapped — block A (`soldier+0x3c8..+0x3e0`, `FUN_0020aad0`) is the AI controller path (vtable 0x5375e8, built in the `UnitAgent` ctor), block B (`+0x2ee..+0x2f1`) is the `PlayerController` update from pad bits 2/3 (tap-release = crouch toggle, double-tap = prone); the doc is CORRECTED. B: BF1 `FUN_002450b8` = `UnitCombatAgent::OnEnterState` does push prone directly at state 6, but state 6 is only the initial state for order kind 3 and no reachable site issues kind 3 — dead code (the lead re-read the kind→state ladder at 161150–161158; the reachability claim is the agent's). C: BF2 `FUN_002d53c8` is coded as a third hint-stance behaviour but its hint lookup needs `FUN_002deec0 != 0`, and the lead re-read the raw ELF words at 0x2deec0 — `03e00008 0000102d` = `jr ra; daddu v0,zero,zero`, a `return 0` stub — so its stance states never run. **PORT IMPACT: none; the mask rule stands.**
  **(4) WATER** (`analysis/re-water.md`) — lead-reproduced: the disc `fx__` census on KAM1/KAS2/YAV1 (`fxwater.py`, read-only) — KAM1 `MainTexture('kam1_water_2')`, `FoamTexture('kam1_water')`, `OceanEnable(1)`, `Tile(4,4)`; KAS2 `OceanEnable(0)`, `Tile(2,2)`, `MainTexture('kas2_water')`; YAV1 `Tile(2,2)`, `OceanEnable(0)`, `DisableLowRes()`, `Velocity(0.1,0.02)`, `MainTexture('yav_water')` (rerun on `YAV1.LVL` by the lead). Lead re-read: the port's `mungefs/src/water.rs:201` `height: le::f32_at(rest, 4)`; the BF1 `CPAT` tile loop 551346–551359 (a patch REPLACES the flat template, it does not add); the far distance `DAT_00522a7c * FarSceneRange` at 551290–551293 (the 0.9 constant is the agent's ELF read). **VERDICT: lead-verified on the hops above; the VU-side claims (the LUT, the speckle pass, the FFT) are the agent's [V] over the decomp, not re-read — the RE_LOG entry says which.** **HOLD 10(f) ANSWERED:** `CPAT` REPLACES its cell's flat quad; `WMAP` bit 3 = "an `LPTC` follows"; `LAYR`'s colour word and its three undecoded fields are DROPPED by both ELFs (the byte-order question is moot). **HOLD 19(c) ANSWERED:** the near water takes no fog on the PS2 either; BF1's far water is fogged by a geometry sub-pass out to 0.9·`FarSceneRange` (only without `DisableLowRes`), BF2's never (its `SpecularEnable` gate is unauthored); the level is right — the plane is `LAYR.height` — so the YAV1/KAS2 look is the tint (W2) or the port's floor (WL). **HOLD 4 AMENDED:** the `fx__` chunk's `Effect("Water")` records are RELEASED to Opus (the `sky_` reader shape, the 27 keys in `re-water.md` §5); every other `fx__` effect stays held. **STILL HELD (NEW HOLD 21):** the water VU1 microcode (wave displacement, the LUT lookup, per-vertex alpha) and KAM1's FFT ocean — building the ocean is a design call; also the far fog pass's look, KAM1's horizon past 640 m, and key 0xa015e2ff. → queue items W1, W2, W3, WL; item 5's open water-cull question is answered inside W2.
  **Decisions on the parked branches (taken before landing):** item 5's mechanism lands with `OPENBF_FAR_CULL` OFF and its 44 sheets are NOT viewed yet (they are judged with item 6's); T2R's default stays `Pc` and the +388 MB on disk stays the maintainer's; T2S accepted (`tier.rs` outside the file set) and its 14 unit goldens regenerated by the lead after viewing the pairs; OGG's rename accepted; GS's `audio` deviation ACCEPTED (`Audio::decode` = `load_sound` without the mixer, additive) and `has`/`play` seeing the resident CORE bank accepted; PD's D5(b) "current objective" = nearest post STANDS (PROVISIONAL), "a Hold has no release" stays PROVISIONAL → queue item HR, and the squad marker stays OFF until SM; the two `OPENBF_WALKABOUT_OUT` hunks are byte-identical (the lead `cmp`ed the two diffs) so T2R's lands first and far-scene-cull rebases clean; the phase-d worktree and branch removed after the ff (Phase D BUILT).
  **The refill (`lead_tmp/s17/gen_queue.py` over `s17/QUEUE.md.before` — every kept item verbatim; `queue_spec.json` is the input, `queue_spec_template.json` the reusable pattern):** 3/OGG/GS/T2R/T2S/5/PD DONE; order now 6 → W1 → S0 → W2 → W → 4 → SM → VO → T2C → WL → L → AT → 9 → W3 → 7 → 13 → HR → C3 row 6 → 17B → ONCE → RT. New items: W1 (water config from fx__ Effect("Water") [REBAKE][GOLDENS]), W2 (tint/alpha/Z/fog rule [GOLDENS]), W3 (speckle pass [GOLDENS]), WL (YAV1/KAS2 floor measurement), SM (the squad marker on — unreferenced textures keep disc alpha [REBAKE]), AT (MASK cutoff 192/255 [GOLDENS]), VO (per-map VO streams [REBAKE]), T2C (soldier tier-2 runtime test), HR (a Hold releases, PROVISIONAL), 17B (post-outcome pool test), ONCE (roster OnceLock), RT (render/tests common). The GATES block carries the s17 baselines (identical values, new paths) and the bake-bytes gate is 21590 files DIFF=0 with no noise family left.
  **Cleanup:** `scripts/cleanse_delete.sh --yes` freed 160 GB (26 paths — the lander's four scratch bakes + bake_main_pre, bake-ogg/{bake,bake2}, far-scene-cull/bake, test-scratch-dirs-2/bake, two-tier-soldier/{bake_before,bake_after,bake_nopc}, two-tier-runtime/before, phase-d/before, and the nine dead *_target dirs; scripts/cleanse_delete.sh gained the s17 line); the repo is 89 GB.
  **Loop:** STOPPED for the whole window (the maintainer's `STOP` at the root, 07:58) and left stopped — **The maintainer relaunches it himself**: `rm -f STOP PAUSE && scripts/autoloop.sh --probe && LANES=2 nohup scripts/autoloop.sh > engine/artifacts/loop/autoloop.log 2>&1 &   (from the maintainer's own terminal; with every CLAUDE* env var stripped if run from a Claude session)`. `engine/artifacts/loop/DRAINED` removed at the refill.
  **Process rules learned this window (keep):** a map golden can move when only the SIDE bake changes — the map sheet's soldier panel draws a side unit, so a [REBAKE] side change is a [GOLDENS] map change too (s17); quantify a moved golden's diff (mean/max/changed-pixel fraction + bbox) before viewing, it tells noise from a real move; a Fable RE agent's two [V] facts can compose into a port rule neither stated (the CLUT scale × the AREF) — the lead writes the composition as [D] with both citations; a background Bash cwd reset moves the harness cwd — absolute paths.
  **Still Fable-only:** the water VU1 microcode and KAM1's FFT ocean (HOLD 21); the `modl` LOD switch rule (HOLD 20); the vehicle RE for P7, which is next after Phase D; B1 (`Sim::hash` omits `assist` and most of `Bolt`); BES1's ring (HOLD 4, low value). The maintainer: a PCSX2 GS dump (the alpha-rule reconciliation is now CLOSED by the CLUT scale — what is left is the D-pad menu's second layer, the wreck's LOD and the water fog look); the Tier-4 Elite feel check.

- **2026-09-09 (night) — Fable session 19: the FIVE branches the evening loop parked are LANDED; NO rebake; 7 map goldens regenerated after every pair was viewed; the SWEEP and the TAT2 ARENA re-baselined BY DESIGN (queue item 4); the water tint scale verified in the ELF; the queue refilled.** the maintainer's standing rule held: Fable did only what needs Fable — an Opus lander did the landing mechanics, an Opus toolsmith ported the close-out kit into `lead_tmp/s19/`, an Opus doc agent typed this entry; the lead read the five loop reports' "For the lead" sections, checked the lander's load-bearing numbers against its own logs by grep, re-read the diffuse-LUT hops in the ELF itself, viewed every golden pair, took the decisions, regenerated the goldens and reproduced every baseline on final main.
  **The loop's evening run:** after session 18's unpark (21:07) the maintainer's two lanes ran 21:17–22:51 — seven sessions (lane 1 ×3, lane 2 ×4), est. (cost omitted); FIVE branches PARKED with every gate green (W2, 4, 13, HR, WS); DRAINED at 22:27/22:51 (everything left waited on W2 or 4). The maintainer: "go" — land them now so the loop runs overnight; the lead parked the lanes with `PAUSE` (no lock held) at 23:00 for the landing.
  **Landed on main in the order 13 → HR → WS → W2 → 4** (item 4 LAST, so every earlier branch's sweep and arena still matched the s18 baselines), each rebased in its worktree, ci exit 0 there AND on main under `flock engine/artifacts/loop/ci.lock`, fast-forwarded, the worktree and branch dropped afterwards (Opus lander, brief `lead_tmp/s19/briefs/landing.md`, report `lead_tmp/s19/reports/landing.md`, logs `lead_tmp/s19/logs/`; lead checked by grep: the five ci totals, four sweeps raw-`cmp`-identical to s18 plus item 4's own pair/loop-after `cmp`s and its 22 moved lines, all five sim_gates logs green with the three hashes, the `cmp_bake` line, W2's failing-golden lines):
  **`ron-twins` (13, 1 commit `6d87a77`)** — the two RON twins become one definition each, with byte-for-byte round-trip tests over every shipped file: +6 → 652; the reader had drifted (`odd_frequency`/`streamed` dropped, an invented `extras`); a full scratch bake vs the shipped tree `SAME=21651 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0` (lander-reproduced).
  **`hold-release` (HR, 1 commit `6c5cd4a`)** — a holder shoved off its anchor is released, on Follow's own rule: +1 → 653; PROVISIONAL; the D14 scripted-order capture reproduces `sim hash after 6000 ticks: ebb933882466ff06` with all 60 `SQUAD` trace lines at `releases=0` — the new rule is present and never fires in any automated referee.
  **`walk-spawn-always-post` (WS, 1 commit `3f7a0f2`)** — `game --map` starts at a friendly post on every map, not only where the ground is missing: +1 → 654; lander-reproduced bf2_cor1 y=3.000 and bf2_hot1 unchanged (all 16 of its posts are team 2); tat1 keeps its authored start, so the recorded replays do not move.
  **`water-tint-fog` (W2, 2 commits 9637294/`5e047c2`)** — the water's own pass: the authored tint at the GS's 0..128 scale, depth-written and translucent, no fog inside the near range, the far rule per game: +4 → 658; `gpu_tests.sh` 101 BY DESIGN on exactly five water maps (mean abs bf1_kas2 13.785, bf2_dag1 1.958, bf1_kas1 1.935, bf2_kas2 1.626, bf2_kam1 0.739; bf1_nab2 0.011 and bf1_yav1 0.008 moved a hair and did not fail; every other map 0.000).
  **`era-class-db` (4, 3 commits 7da1e1e/152625e/`542d7bc`)** — a map's classes AND its soldier models come off the MAP's own disc, not the faction's, and the roster warnings go in the map's own block instead of a shared stderr (roster warnings 233 → 3, fallbacks 36 → 0): +4 → 662; **THE SWEEP AND THE TAT2 ARENA MOVE BY DESIGN** — both reproduce the loop session's own files at `cmp` 0, 22 lines moved vs s18 (18 map rows + 3 relocated roster lines + the footer).
  ci across the window: 646 → 652 → 653 → 654 → 658 → 662. **No sim value moved on any branch** — the three replay hashes, `--sim-hash 100` and both `motion.csv` are unmoved after five branches; the only gates that moved are item 4's sweep and tat2 arena, by design. Worktrees at close: all five removed; `git worktree list` = main alone; `<repo parent>/openbf-wt/` empty. **The CODE SHA is main `542d7bc`** (main before the window: `2be0358`); the goldens commit `5d18752` and the docs sit on top.
  **NO rebake this window:** no branch changes a bake output byte, and item 13's full scratch bake against the shipped tree is `SAME=21651 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0` — not one path in either direction — so `engine/assets_baked` is still a fresh bake of main at 21,651 files.
  **Goldens (`lead_tmp/s19/goldens_regen.sh` + `golden_pairs.py`):** 7 moved and nothing else — bf1_kas1, bf1_kas2, bf1_nab2, bf1_yav1, bf2_dag1, bf2_kam1, bf2_kas2 — W2's five past the limits, quantified before viewing (bf1_kas1 1.935 / 16.5 %, bf1_kas2 13.785 / 31.3 %, bf2_dag1 1.958 / 15.9 %, bf2_kam1 0.739 / 7.2 %, bf2_kas2 1.626 / 2.5 %) plus the two hairline ones the check did not fail (bf1_nab2 0.011, bf1_yav1 0.008 mean), all seven water maps; every pair viewed (`lead_tmp/s19/sheets_view/golden_pairs_00..03.png`): the water is darker and translucent — the pier legs and pool bottoms show through, Dagobah's swamp water goes near-black at eye level, Kamino's sea a shade darker; nothing but water moved; committed by path as `5d18752`, with no rebake preceding the regeneration.
  **Lead-reproduced on final main `5d18752` — BASELINES NOW = `lead_tmp/s19/baselines/SUMMARY.txt` (§WHERE THINGS STAND carries the canonical copy); the SWEEP and the TAT2 ARENA re-baselined by item 4, every other value identical to s18:** `scripts/sim_gates.sh` walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, `--sim-hash 100` `17e1b946ff0e821c`, both motion.csv identical to s18 (walk 079c8dbca95a380d0fd91484b7f2d2d0, aim 492299f958b70d6dc956b6386950f765); the sweep footer is now `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000` (pair 0; vs the loop's `era-class-db/final/sweep_1.txt` 0; 22 lines moved vs s18, md5 be656ec867f7a3d1492ed48863b7b859, s18's 3a3d47ec…; NO escape hatch — the old numbers were the fallback's); `arena 3000 42 flat all` kills/min 45.49 / 122.33 / 166.89 / 200.00 (606262bc6865a689 / 09c7449831956e5d / b2f6343d64f93c51 / b8ab0f137168eb26), pair 0, vs s18 0; `arena 9000 42 tat2 all` kills/min 25.40 / 38.60 / 39.80 / 37.20 (afc35bc1ad1547fa / 39b7566c1cd0081b / 5b7f8c1e4c3bb14b / 5136f94879f8f310), `monotonic: kills no  hits yes`, pair 0, vs the loop's own after-arena file 0, vs s18 DIFFERS by design; `scripts/gpu_tests.sh` exit 0; ci 662 tests (`lead_tmp/s19/final_gates.sh`, log `lead_tmp/s19/logs/final_gates.out`). The walkabout set was regenerated on the UNCHANGED s18 bake (the water tinted and depth-written by W2, every map starting at a friendly post): 219 images over 32 maps in 20.5 s (4.5 s of that PNG encoding), 882.1 MB in <repo>/engine/crates/game/../../artifacts/walkabout.
  **THE WATER TINT SCALE, LEAD-VERIFIED IN THE ELF (rulings `lead_tmp/s19/rulings.md`, item W2 — the one RE fact of the window):** the queue item spelled `/255`, the lane landed that and then flagged the water as DARK and asked. The lead re-read the hops: the diffuse LUT builder (BF1 `FUN_0048d068` 551904–551970, the entries at 551945–551960) lerps the authored Border/Min/Max colour bytes and stores each as `byte_lerp * 0.003921569 + 65536.0` — the PS2's float-to-fixed trick: for a float in [65536, 131072) the mantissa's unit is 1/128, so the low mantissa bits hold `round(byte/255 × 128)`, i.e. the GS's own 0..0x80 scale where 255 → 128 = 1.0. The alpha table (`FUN_0048cec8` 551854–551903, the packs at 551896–551898) packs Min.a/Max.a the same way (188 → 94 = 0.74 blend). **So `/255` IS the PS2's reading:** an authored 80 modulates the texture by 0.31, and the water is dark by the disc's own numbers — which is also what the RE predicted (`analysis/re-water.md` §2/§3: "≤ half-bright", "on the PS2 the water is dark"), so the picture is not evidence against it. RULING: `/255` stands → [V]; W2's midpoint-of-the-LUT simplification stays PROVISIONAL (the faithful form is the per-vertex random index). Recorded in `analysis/re-water.md` §3 and `analysis/RE_LOG.md`; HOLD 10(f)'s tint clause is CLOSED.
  **Decisions on the five (taken before landing, `lead_tmp/s19/rulings.md`):** **W2** LANDS with the tint scale above; its FOAM finding is item **SM**'s (`kam1_water` reads alpha 255 on every texel in today's bake because the bake forces every texture no MASK/BLEND segment names opaque — the `keep_alpha` artefact; SM restores the disc alpha, W2's `texture_has_alpha` predicate then draws the foam by itself and KAM1's golden moves then); W2's correction ACCEPTED — KAS2's "white bar above the red line" is the sky dome's bottom edge plus the fog-coloured clear strip between the terrain edge and the dome, which is item **L**'s. **4** LANDS LAST and re-baselines BOTH the sweep and the tat2 arena, with no escape hatch (the old numbers were the fallback's); its deviations accepted (`examples/arena.rs` and `src/soldier_vis.rs` outside the file set; BF1's CIS default list has four classes; "21 BF2 GCW missions" measures 16); its two `spawn:` prints → C3 row 6. **13** LANDS; the THIRD twin (the class RON — `bake::classes::BakedClass` vs `game::classdb::ClassDef`) → new queue item **CT**; the fourth (`GlobalSoundRon` vs `GlobalRon`) is a documented partial read and stays; its one guard denial — a READ of `extracted/` made through the bake binary — is a FALSE POSITIVE and goes to the HANDOFF backlog. **HR** LANDS, PROVISIONAL: a design line, unreachable in every automated referee we have (only a Tier-4 hand-made capture would show it — a Tier-4 capture row, not a queue item), and PLAN's "a Hold has no release" deviation is struck where it lived, in the Phase D squad spec; `core/src/ai.rs` outside the file set accepted. **WS** LANDS: tat1's authored-start exception STANDS (a branch that edits its own referee proves nothing; pinning `--spawn` in `sim_gates.sh` is a lead change for another day), bf2_hot1 is left alone (every one of its 16 posts is team 2), and bf2_pol1's friendly post sitting ~27 m under its own terrain (26.86 m measured, `--ai` too) goes to item **SK** as a lead, untested; `layout.rs` +5 accepted.
  **The refill (`lead_tmp/s19/gen_queue.py` over `s19/QUEUE.md.before`, `queue_spec.json` the input):** the five are DONE; order now **OC → SM → L → AT → SK → 9 → W3 → 7 → CT → C3 row 6 → ONCE**. One new item: **CT** (the third RON twin, the class RON — the same defect item 13 fixed twice over, and it waited on item 4's files). The GATES block carries the s19 baselines — the new sweep footer, the new tat2 arena rows, ci 662, bake bytes 21,651 files DIFF=0.
  **Cleanup:** `scripts/cleanse_delete.sh --yes` freed 104 GB (12 paths — the five dead `*_target` dirs, `s19/bake_ron-twins`, and the raw renders of W2 and WS with their sheets kept; `scripts/cleanse_delete.sh` gained the s19 lines); the repo is 94 GB.
  **Loop:** the two lanes ran the evening and drained; the lead parked them with `PAUSE` for the landing and removed `PAUSE` and `engine/artifacts/loop/DRAINED` at 23:58, before this docs commit — **the loop is RUNNING on the refilled queue and needs no relaunch.**
  **Still Fable-only:** the water VU1 microcode and KAM1's FFT ocean (HOLD 21 — the tint-scale half is now CLOSED); the `modl` LOD switch rule (HOLD 20); the vehicle RE for P7, which is next; B1 (`Sim::hash` omits `assist` and most of `Bolt`); BES1's ring (HOLD 4, low value); the `tick_bleed` AI-shed reachability question (17B's observation). The maintainer: a PCSX2 GS dump (the D-pad menu's second layer, the wreck's LOD, the far water fog look); whether NAB2 keeps its faithful black distance (HOLD 10(c)); the Tier-4 Elite feel check.

# ARCHIVED FROM PLAN.md 2026-09-25 (restructure: goals/specs stay in PLAN.md; status -> STATUS.md; history here)

## WHERE THINGS STAND (updated 2026-09-24 by the cleanse, lane 1 — true of main after KW and the cleanse; what each landing did is in the Log and in QUEUE.md's DONE rows, not repeated here. The loop IS the lead and IS running; Fable is off, so it does its own RE)

Read this before the Log — the Log is chronological and long, and this is what
it adds up to. `analysis/RE_LOG.md` is the lab notebook (one section per
subsystem, every finding tagged [V]/[D]/[H] with its citation);
`HANDOFF-OPUS.md` is the current session brief and the HOLD list;
`engine/artifacts/loop/QUEUE.md` is the queue the autoloop works through.

- **The AI is ours (the maintainer, 2026-09-01) and it is the port's biggest deliberate
  departure.** Both discs ship the same authored AI data on every map (`plan`
  graphs, `Hint` nodes, `BARR` barriers — measured); neither original's
  decision code is ported. Graph navigation + local steering, a per-team
  utility commander, behaviour tree, perception, a skill vector; four
  difficulties on a three-layer model; squad commands. Full decision and
  evidence: §AI in the LOCKED FEATURE SET and the Log entry "AI decision";
  formats in RE_LOG; the difficulty/feel constants of both originals in
  `analysis/re-ai-difficulty.md` (its §4.1 aim-error table was CORRECTED
  2026-09-08 — the floor is a float, `analysis/re-ai-sniper.md`).
- **Known open defects — grouped by CAUSE, with the fix each waits on**
  (the maintainer's walkabout defect hunt, 2026-09-07;
  `engine/artifacts/walkabout_defects/README.md` has the crops, and the
  2026-09-08 correction at its head):
  1. *Outside the level* — the razor horizon and the band under the sky
     shells. Three pieces are built: the far-scene CULL (queue 5, `b203619`)
     behind `OPENBF_FAR_CULL`, the BF2 no-far-layer FADE into the sky colour
     (queue 6, `19148e6` — on by default; Felucia's and Endor's fog walls are
     gone), and the `LOWR` far-LOD SHEET (queue L, `cf7b642`) behind
     `OPENBF_LOWRES`. **Both flags stay OFF** (L's landing, Log 2026-09-17
     evening): the sheet is faithful where one exists — 14 of 32 maps,
     recovering 61–77 % of the cull's change — but on the 18 maps that draw no
     sheet (16 ship no `LOWR`; BF2 DAG1 and NAB2 ship one with no texture) the
     cull leaves the clear colour (BF1 END1 loses 11.778 % of its
     pixels, BF1 RHN2 5.197 %). What the default waits on is what the disc
     draws past the cull on those 18 maps — no queue entry holds it yet
     (HANDOFF §What next 2). `LOWR` is a same-footprint far LOD, not the razor
     horizon's fix (`analysis/re-lowr.md`). KAS2's red far band is in both
     halves of its sheet, so it is queue KASF's (the `255,0,0` fog).
  2. *Terrain drawn where the PS2 hides it* — the Hoth outpost rooms: FIXED
     2026-09-08 night (queue H landed, `50c7993`): the bake writes the disc's
     flagged `PTCH` replacement mesh for the 122 flagged patches on 13 terrains
     (`analysis/re-terrain-cutters.md`) and the viewer chunks the baked
     triangles; the lead viewed all 101 A/B sheets (the Hoth rooms show their
     console banks, grate and doorways; TAT1's buried staircase is back).
  3. *Fog and colour* — the BF2 "no far layer" maps are **FIXED 2026-09-09
     evening** (queue 6, `19148e6` — bullet 1; six goldens regenerated,
     `bf2_nab2` fades into the disc's own BLACK `sky_naboo2`, which is HOLD
     10(c) and the maintainer's eye). The water fog question is **ANSWERED 2026-09-09**
     (HOLD 19(c), `analysis/re-water.md`
     §1, lead-verified on the cited hops): the NEAR water takes no fog on the
     PS2 either, and the FAR water is fogged only in BF1, only on maps without
     `DisableLowRes`, by a geometry sub-pass out to 0.9·`FarSceneRange` — BF2
     gates that sub-pass on `SpecularEnable`, which no map authors but which
     DEFAULTS TO 1 (corrected 2026-09-25, `analysis/re-water-flags.md` [O]), so
     it runs on BF2 too — and shows nothing on any shipped BF2 map (KAS2's fog
     is off; the rest have no far layer or `DisableLowRes`). The port rule
     LANDED 2026-09-09 night (queue W2, `5e047c2`): no fog inside the near
     range, the far rule per game (BF2's flat-water half landed 2026-09-25 as
     queue WSE, `bb6b0d2`; BF2's ocean stays unfogged — its renderer is not
     the gated path).
  4. *Materials* — BF2 COR1's sky-lane strips are an alpha-tested card, fixed
     by queue 9 (landed 2026-09-24), and the additive SPECULAR second pass is
     drawn on world models since queue SPEC (landed 2026-09-25; soldiers and
     weapons are SPEC2) — item 10 had found BF1 YAV1's ribbon quads to be the
     PS2's own opaque base pass of a `Specmap` 0x31 material missing that
     pass; the RHN "smears" are in the texture; DEA1's red
     grid is a second, deliberately red surface behind the grating —
     `engine/artifacts/mat_invest/README.md`); `kam_prop_crate`-class
     grime/specular alphas still test to shreds (the mask classifier is
     specified in §DESIGN SPECS, unbuilt). **BF2 GEO1's Techno Union wreck
     (added 2026-09-08 late night, the maintainer's look at the regenerated goldens —
     the forward hull draws as bare rafters) is NOT a material bug** (RE
     2026-09-09, `analysis/re-vu1-cull.md` §2, the CPU side lead-verified
     against `re-material-flags.md` §2; the VU1 microcode reading is the
     agent's): the five hull segments carrying MTRL 0x02 are alpha-tested at
     AREF 0x60 on the PS2 = the port's MASK in kind, and backface culling is
     VU1-side and rides only on 0x800800, which the EE sets solely for
     `(f&6) && !(f&0x10000)` — so plain opaque segments are drawn two-sided on
     both consoles, exactly as the port's one `cull_mode: None` pipeline does.
     No ALPHA RULE v3 and no cull item come out of it. The strongest remaining
     candidate is the LOD switch: the port draws LOD0 forever while the LOWD's
     rafters texture is 100 % opaque, and the PS2's `modl` LOD switch rule is
     unread — **HOLD FOR FABLE 20** (`analysis/re-vu1-cull.md` §4 carries the
     two-station PCSX2 capture note that settles it). **Soldiers** draw by
     the disc's own per-segment rule since SKA (2026-09-24,
     `analysis/re-side-alpha.md`): no alpha test on any of them, and the one
     0x04 segment per game — `all_inf_wookie`'s fur — blended. What the ELF
     reading has settled in the world's alpha rule without a GS dump (AT's
     test, R1U's COR1 doors and DEA1 walls drawn solid, KW's Kamino glass) is
     HOLD 10(g)'s list.
  5. *Water* — answered (`analysis/re-water.md` §1–§5, §3a, §3b;
     `analysis/re-water-alpha.md`) and built: the plane and the floor are the
     disc's (WL); the `fx__ Effect("Water")` texture, tiling and scroll (W1);
     the water's own pass — the authored tint at the GS's 0..128 scale,
     depth-written, unfogged in the near range, the far rule per game (W2);
     Kamino's FFT ocean at the disc's unnormalised metre scale and one
     resolution, and the flat-water bob (OC, OCB, OCL); and each map's own water
     sheets keep their disc alpha (WA, 2026-09-23), so bays and swamps show
     their beds and BF2 Kamino's foam layer draws; and that foam is the disc's
     per-vertex byte, stepped on the crests (OCF, 2026-09-23); the speckle
     layer is on both games' flat water and FEL1's water is still (W3 and WSE,
     2026-09-25, `analysis/re-water-flags.md` [O]). LEFT: the foam
     layer's own `FoamTile(5,5)` and the ocean's vertex RGB (both unread — HANDOFF
     §What next 2), and HOLD 21's
     flat-water VU1 microcode (the phase nibble, the LUT, Av, the speckle's
     colour), which needs a GS dump.
  6. *Missing instances* — BF1 BES1's sky sliver: ANSWERED 2026-09-08 (item 8,
     `engine/artifacts/bes1_sliver/`) — the disc's `bes1_bldg_main02` HAS that
     opening (an open ring between the ceiling slab and the parapet over an
     outdoor ledge; every declared index consumed, the collision agrees). Not a
     defect; whether the PS2 draws an effect into the ring is HOLD 4 (`fx__`).
  Standing, with their HOLD numbers: BF2 Theed's sky dome is black because the
  disc ships a black texture (HOLD 10(c)); ~10 % of both Hoth goldens clips to
  pure white (HOLD 11(a)); `*_collision*` proxies are in the drawable list
  (HOLD 11(b)); untextured and dangling segments are skipped (HOLD 18); the
  mission RONs' member pools are PROVISIONAL where PC and PS2 differ (the PS2
  wins).
- **Stated deviations from the PS2 — the one list** (they used to be scattered
  across ~20 documents). These are deliberate and known, not defects:
  1. **The AI is openbf's own**, not either original's (above).
  2. **Classic-plus rendering**: modern lighting/shadows/post ON by default,
     strict-faithful as a toggle (§Architecture 5).
  3. **The sky-shell V clamp** is per segment where the shell's UVs do not
     tile; the PS2 samples the ring wrapped (`analysis/re-texture-wrap.md`,
     RE_LOG "DELIBERATE DEVIATION"; it removed the maintainer's Mustafar seam).
  4. **The fog's reach**: the port fogs every draw from the fog's near
     distance, where BF2 fogs only far-scene draws through a depth LUT and BF1
     fogs near-scene models too (`analysis/re-fog.md` §2.2/§2.5; the
     "NOT reproduced" list on `bake::lighting::fog_from`). A BF2 map with NO far
     layer is faithful since queue 6 (`19148e6`): its near scene fades into the
     sky and its authored fog colour is never drawn (`game::viewer::far_fade_from`).
  5. **ALPHA RULE v2** classifies the alpha mode per draw from the flag word,
     render type and histogram; the reconciliation with what the GS actually
     shows needs a dump (HOLD 10(g)).
  6. **Universalised unit behaviour** where the two games differ (the droideka
     jump, above) — one behaviour for the whole blended game.
  7. **Aim assist is never off by default at any difficulty**, and it is
     BF1-style passive magnetism only on foot (§Movement & feel, the maintainer; vehicles,
     flyers and space: §Vehicles & space).
  8. **A fresh weapon-balance pass** over the baked ODF stats, which are kept
     in data for reference (§Combat).
  9. **Heroes are always killable** and use BF2's hero health pool (the disc's own
     values) — it shrinks ONLY from damage taken, never with time, and still
     regenerates on kills (the maintainer, 2026-09-23; reverses the earlier flat-pool call,
     §Heroes).
  10. **Unit counts** default to per-source-game authentic caps (BF2 16 bots,
      BF1 25–32) with a slider, not BF2's single 16 (§AI).
  11. **No Lua VM, no mod ecosystem** — a mod worth having is ported to native
      Rust (§Architecture 3).
  12. **Untextured / dangling segments are skipped** rather than guessed
      (HOLD 18), and the movement/step constants named above are ours.
  13. **The squad marker is drawn IN THE WORLD, over the bot, and keeps a
      depth test** — the PS2 draws those two icon sheets as a 2D D-pad menu at
      HUD (−120, −50) with no Z (TEST 0x31001); the art and its alpha are the
      disc's (`game::squad_vis` header, `analysis/re-hud-icons.md` §2;
      `OPENBF_SQUAD_MARKER=0` hides it).
  14. **Kamino's still water plane is drawn past the ocean's far range** (sunk
      under each frame's deepest trough, and without foam once the sea steps) —
      the PS2 draws no water there at all; it is what keeps the sea from opening
      onto the seabed while `OPENBF_FAR_CULL` is off (`game::water_anim` header).
  15. **Kamino's foam starts zeroed and settled** — the disc clears ONE byte of
      its foam map (a shipped bug) and lets the heap's garbage decay; the port
      zeroes all of it and pre-runs `FOAM_WARMUP` = 192 steps (PROVISIONAL) so
      the first frame shows the settled sea (`game::water_anim` header).
- **Everything unproven is labelled PROVISIONAL in code and here** — **190
  occurrences across 56 files** (`grep -r PROVISIONAL engine/crates
  --include='*.rs'`, re-counted 2026-09-24 by the cleanse — OCF's 191 less `WrapV`'s, settled as stated deviation 3; the splits spread them over ten more files). Anything needing the
  decompiled C is on the HOLD FOR FABLE list in `HANDOFF-OPUS.md` — kept under
  that name so old greps find it. **Since 2026-09-17 (the maintainer) the loop does that
  RE ITSELF** (`OPUS_LOOP_PROMPT.md` §5, §Working mode below): Fable is off,
  `$OPENBF_FABLE_MAX` is 0, and the session's own reading is disciplined by the
  four self-checks and one Opus verifier agent, lifted into `analysis/` as
  `[O] UNAUDITED` with its constants PROVISIONAL.

## Audits & research
- A2. Bake-format recon: PS2 LVL chunk variants vs LibSWBF2/swbf-unmunge PC
  handling. **DONE 2026-08-30** → `analysis/a2-bake-formats.md`: PC-identical
  container w/ FNV-1a hashed tags; PS2 texture RLE codec cracked+verified;
  quantized model buffers mapped; terrain heights (HGT8/HEXP) were A2's top
  open question — RESOLVED at P1 (`analysis/re-terrain-heights.md`).
  A2's own errata are marked inline in that file (modl INFO, HEXP layout). LibSWBF2 has no PS2 support; swbf-unmunge
  covers tex/models only.
- R1. Survey community GC + graphics mods on PC ports (ideas for GC economy, classic-plus
  look targets). **Graphics half largely RESOLVED 2026-08-30** by the Harrisonfog
  collection (see §Classic-plus assets); GC-mod survey still open.


## Working mode (overrides memory defaults)
- **Parallel work (the maintainer, 2026-09-03):** research and RE agents run in parallel
  freely. Code changes may run in parallel ONLY as separate git worktrees on
  branches with DISJOINT file sets, each with its OWN cargo target dir under
  `engine/artifacts/lead_tmp/<branch>_target/` (never under /tmp, never the
  shared `engine/target`), merged to `main` by the LEAD only — since 2026-09-14
  that is the loop's LANDING session, one at a time under the
  `engine/artifacts/loop/locks/MAIN` claim (rebase, `./ci.sh` green on the
  branch AND on the merged tree, a fast-forward only, then the
  sweep/replays/goldens run on main). Anything that rebakes, touches
  `manifest`/`Cargo.lock`, `core::sim`'s hash, or the goldens is serialised
  through that claim. Priority one is correct, clean code and a look that
  matches the discs under our stated deviations — speed never buys a
  weaker gate.
- **The Opus loop is the lead; Fable does RE only (the maintainer, 2026-09-14 — replaces the
  2026-09-09 "Fable sparingly / Fable windows" rule).** The project runs on
  `scripts/autoloop.sh` with the standing prompt `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md`.
  Its Opus sessions build, land (rebase → gates → fast-forward → rebake → goldens,
  quantified and viewed first → re-baseline → docs), refill the queue and make the
  calls, fully autonomously: a call is taken (faithful, reversible), recorded as a
  `DECIDED` Log line and in HANDOFF §For the maintainer, and the work goes on. **Fable is OFF on
  this project since 2026-09-17** (the maintainer needs that budget for his day job; `FABLE_MAX`
  = 0): the session does the reverse-engineering itself under `OPUS_LOOP_PROMPT.md` §5
  — [V]/[D]/[H] tags, then four mandatory self-checks (name the enclosing function of
  every citation; a search, not a reading, behind every absence claim; the twin open
  for every "BF2 does the same"; the ELF bytes for every constant), then one Opus
  verifier agent that sees the claims and citations but not the reasoning; the result
  is lifted into `analysis/` as `[O] UNAUDITED` and its constants stay PROVISIONAL.
  Sessions run at `EFFORT=medium`, and a session that picks the RE job restarts itself at max (the maintainer, 2026-09-24 — see the Log). **A CLEANSE job falls due every 12 landings (landings only, no clock; the maintainer, 2026-09-23), capped at about 250 turns and 2 sub-agents**
  (prompt §6b; checklist `.claude/commands/cleanse.md`): docs and artifacts
  consolidated every time — one live copy of each fact, superseded text archived to
  `analysis/PLAN_LOG_2026.md`, dead scratch deleted — and one code area per rotation
  (core → game → render → bake → mungefs → manifest → scripts) for dead code,
  duplication, stale PROVISIONALs, weak tests and a bug hunt over what landed since
  the last one; it never changes behaviour, and what it cannot finish becomes a
  `CLN<n>` queue entry. That is how the project stays small and true as it grows. The evidence behind the discipline is the 2026-09-14 blind trial
  (`engine/artifacts/opus_re_trial/report.html`: Opus on four Fable RE briefs scored
  83 % of the key claims but made four false [V] claims). **The maintainer starts the loop
  HIMSELF; no session ever launches `autoloop.sh`.**





## Log

- 2026-08-30 (the maintainer, decisions): **(1) the Lua mod ecosystem is NOT a goal** — a
  mod worth having is ported to native Rust (§Architecture 3); **(2)** the PC
  SWBF2 install supplied at `PC_swbf2_exe/` (gitignored); **(3)** the PC textures
  measure larger for about a third of them (79/79 name matches on DEA1), so a
  PC-stock tier is taken ONLY where larger; **(4)** DLSS is the wrong tool (a
  temporal frame upscaler, no offline image mode) — offline upscaling stays
  Real-ESRGAN; **(5)** per-map `lght`/`sky_`/fog and the colour-space questions
  released to Opus. Full text: archive, same section.

- 2026-09-01 (Fable session 8, with the maintainer) — **THE AI DECISION: openbf writes
  its OWN infantry AI; the discs supply the data, the RE supplies the feel
  constants. Deferred design #1 resolved.** the maintainer's direction: the AI may
  behave differently from either original; FOUR difficulties (Easy / Normal /
  Hard / Elite) with old-BF2-Elite ≈ our Normal; EVERY bot improves with
  difficulty, and the game must still get harder for the human in any lobby;
  BF1's teammate commands come back. **The architecture (decided, still the
  plan):** six layers — navigation on the authored `plan` graphs, a per-team
  commander, a per-bot behaviour tree, perception (cone + LOS + memory +
  reaction), a skill vector applied through the shared weapon dispersion, and
  squad commands — every bot driving the SAME soldier code as the player
  through a synthesised pad. **Difficulty is three layers:** symmetric
  COMPETENCE, per-HUMAN pressure, and a team handicap that applies only
  against bot-only teams; DDA is excluded from anything competitive. The
  evidence: both discs ship the same three AI data sets on every ground map
  (hints, barriers, a `plan` graph), so BF1-vs-BF2 is an ENGINE difference —
  BF1 a connectivity-graph follower, BF2 a goal arbiter. Phases A–E and their
  acceptance gates were set here; A/A.1/A.2/A.3/A.4 and B are built, C is
  building, D is briefed. Detail: `analysis/PLAN_LOG_2026.md` §2026-09-01.

- 2026-09-01 (Fable session 8, the maintainer's request) — **the texture alpha census,
  and ALPHA RULE v2.** 2,182 baked textures joined to 14,746 `segm` materials
  over 243 LVLs (the join reproduces `bake::map`'s `tex_flags` on all 2,182
  rows — the instrument was checked before anything was read off it):
  **303 textures are alpha-tested to shreds today (44 on drawn instances) and
  16 real cutouts render OPAQUE** (Theed's banners hang as solid rectangles
  on both discs). First data on HOLD 10(g): bit 0x02 is the real cutout flag
  (P(false cutout) 0.10) while 0x10/0x40/0x80/0x400 and RTYP Scroll/Specmap
  run 0.56–1.00 — alpha as a MODULATION channel, not a mask — and the
  per-texture OR is lossy, so the rule must be per DRAW. That produced ALPHA
  RULE v2 (built 2026-09-03) and the still-unbuilt classifier refinement in
  §DESIGN SPECS. Two artifacts the maintainer asked about were root-caused here: BF1
  Kamino's "whiteish boxes" are `kam_prop_crate` eroded by the old rule, and
  the white streak under Kashyyyk's treeline is the outside-the-level gap
  (HOLD 10(i)), not a texture. `engine/artifacts/texture_census/`. Detail:
  `analysis/PLAN_LOG_2026.md` §2026-09-01.

- 2026-09-03 (Fable session 10) — **A.4: the honest world's last wall bugs,
  and the collision LAYERS answer.** Four builds and one ablation pass, each
  lead-verified, took the sweep 23 → 28 of 32 with median `stuck_max` 2,580
  → 720 and nothing tuned. (1) A body the capsule ALREADY penetrates is
  judged by penetration DEPTH, not by parry's cast — measured, not assumed:
  parry 0.22.1's `cast_shapes` from an overlapping pose returns `t = 0` in
  EVERY direction with the same normal, so the A.2 touching branch decided
  everything (6907b83; the behaviour is pinned by a test so a parry upgrade
  fails loudly). (2) **Collision LAYERS [V], and the session-7 name rule is
  gone** (`analysis/re-collision-layers.md`; b112661, 1fb9595, 960fdac):
  neither ELF parses a primitive's NAME — membership is per entity class
  from the entc PROPs `SoldierCollision` / `VehicleCollision` / … plus BF2's
  per-prim `MASK` byte (whose loader never clears its temp, so an unmasked
  prim inherits the previous one's byte — replicated). 1,016 instance-prims
  leave the soldier world; Naboo's plaza boxes were never soldier collision.
  HOLD 13 is answered for subset SELECTION and still open for the RESPONSE.
  (3) A blocked slide FLIPS the remembered side and the stuck side-step
  chooses it (caab253) — the 13-tick oscillation on tat2 and MUS1. (4) The
  wall pass uses the same stepped capsule as the probe, so a kerb below
  `STEP_HEIGHT` is climbed, not a wall (dda2afa) — POL1's pair, and after it
  no unit on any examined map carries a non-zero push. (5) The two A.1/A.2
  decisions that rested on mirrored-world numbers were re-ablated over four
  sweeps: the contact cue stays UNWIRED (it improves no column) and the
  probe lift stays (removing it costs GEO1), but the lift's recorded
  justification did not reproduce and `Capsule::stepped`'s doc comment now
  says so. Also this session: the 148 "untextured draws" are SKIPPED and
  reported on their own lines (eb73e0d; 0 on all 32 maps, what the PS2 draws
  for them is HOLD 18), and the maintainer added the two locked-feature-set items and
  the render-tier rules that are now §LOCKED FEATURE SET and §Classic-plus
  assets. Detail: `analysis/PLAN_LOG_2026.md` §2026-09-03.

- 2026-09-03 (the maintainer, end of Fable session 11) — **seven decisions on the
  session's report.** (1) Aim assist is never off by default: a user toggle,
  default ON, profile-saved, no tier touches it — it replaced the Phase C
  brief's D4 and is §Movement & feel. (2) The sweep's idle human is the
  artefact behind the BF2 `alive < tickets` standoffs: the sweep's slot-0
  player becomes a bot (the ticket rule stays) — still to build, after Phase
  D. (3) The ELF's material → GS-state rule is RE'd together with the sky
  pass and fog in ONE pass before any alpha or fog rule changes; the two
  Hoths must look the same afterwards. (4) The class roster is built BEFORE
  Phase C (done, session 13). (5) Squad commands and their voice lines are
  universal (§Presentation). (6) The order: sky V-wrap → the combined RE →
  roster → Phase C → Phase D → two-tier KTX2, the droideka RE, the HOT1
  bridge gap. (7) Parallel work per §Working mode. Detail:
  `analysis/PLAN_LOG_2026.md` §2026-09-03.

- 2026-09-03 (Fable session 12) — **the combined sky / fog / alpha RE, and
  the sky V clamp.** Four Fable agents in one parallel pass, every
  load-bearing citation re-read by the lead (`analysis/re-sky-pass.md`,
  `re-fog.md`, `re-material-flags.md`, `re-texture-wrap.md`). (a) Every sky
  draw takes Z-test with NO Z write into a 16-bit far target composited by a
  depth LUT — the session-11 placement is no longer provisional on that
  count, and under the sky flag bit 0x02 BLENDS rather than tests. (b)
  **HOLD 10(a) ANSWERED:** fog is a second VU1 program, BF2 enables it iff
  `SkyInfo.FogRange` has near < far, **BF1 never parses `SkyInfo` fog at
  all** and fogs from `LowResTerrain.FogColor/FogNear/FogFar` with the
  colour's alpha stretching the far distance; neither game fogs its sky —
  so the port's parallax-shell fog was the deviation that made BF2 Hoth's
  mountains white. (c) **HOLD 10(g) at the CPU level:** the flag word is
  tested verbatim (0x04 blend without Z write, else 0x02 test at AREF 0x60,
  0x40 additive, 0x08 half-alpha, 0x80 specular, 0x100 env; BF1 picks a
  segment class from 0x20/0x400). It contradicts ALPHA RULE v2 on the
  Kamino crate, the Theed banners and Mygeeto's mountains, and read
  literally it contradicts what the PS2 is known to show — **decision: v2
  stands, the ELF rule is recorded not adopted, and the reconciliation
  waits on a PCSX2 GS dump** (the maintainer's decision, re-confirmed 2026-09-08).
  (d) Wrap is GLOBAL REPEAT in both games, so the V clamp shipped in item 0
  (b459146, d86edab, ded500c, 840a701) is a DELIBERATE DEVIATION, applied
  per segment only where the UVs do not tile; the same commit replaced the
  gallery's orbital overview with the fogged `vantage_camera`. Measured
  while doing it: at a standing eye the V wrap is nearly invisible (Hoth 58
  px of 921,600) — the maintainer's grey band was in the orbital panel, and Kamino's
  is the dome's own skirt. Detail: `analysis/PLAN_LOG_2026.md` §2026-09-03.

- 2026-09-14 — **The maintainer: the Opus autoloop is the lead; Fable does reverse-engineering only.** the maintainer's decision, this date: the project runs on `scripts/autoloop.sh` overnight, fully autonomously — its Opus sessions build, land, rebake, regenerate goldens, re-baseline, refill the queue and make the calls without stopping, and they spawn Fable agents only for RE. There is no Fable lead window any more. **Evidence:** the blind trial the same day (`engine/artifacts/opus_re_trial/report.html`; the record in `engine/artifacts/opus_re_trial/record/`): five sandboxed headless Opus runs at max effort redid the four session-17 Fable RE briefs (HUD icons, the stance side claims, the GEO1 wreck, water twice) from the pre-answer snapshot `1628519`, graded against an answer key frozen before any output was read — **37.5 / 45 key claims (83 %)**, (cost omitted), 14–28 min a run, isolation audited (515 tool calls, none outside each run's copy) — but **four claims tagged [V] were false**: both water runs took BF2's ocean renderer `FUN_00575670` for the flat one (the flat far fog is gated, D:708831, and its range is 0.9, D:708702); the HUD run's "BF2's squad command UI is text only" against `FUN_00391ac0`'s two sheet hashes; the stance run's "nothing outside reads +0x30" against the agent tick at D:254274. Every one was an absence claim or a BF2-twin claim — so RE stays with Fable, and the loop verifies each answer on exactly those two kinds of claim (the pre-registered verdict, B). The same runs found a live port bug Fable had marked verified: `bake::water_glb::covered_by_patch` removes **17 / 32 / 2** wet cells where the PS2 replaces **4 / 8 / 1** (BF1 NAB2 / BF2 NAB2 / BF2 DEA1, measured on main) — queue entry **WP**.
  **What changed (docs and scripts only, no engine code):** `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md` rewritten — one job per session (finish → land → build and land → RE → refill), the landing procedure, the `locks/MAIN` claim for anything that changes the main checkout, the Fable budget and the verification rules, `DECIDED` records, `DRAINED` only when everything left needs the maintainer; `scripts/claude_guard.sh` — loop sessions may now rebake, regenerate goldens and rebase, merges must be `--ff-only`, checking out main and shell writes to PLAN/HANDOFF/analysis stay denied (the offline test `engine/artifacts/lead_tmp/s13/guard_test.sh` passes 50/50); `scripts/autoloop.sh` — `FABLE_MAX` (default 2, exported as `OPENBF_FABLE_MAX`) and the comments; `scripts/AUTOLOOP.md`; QUEUE.md's rules and marks (statuses TODO / IN PROGRESS / PARKED / BLOCKED / DONE); `engine/artifacts/loop/START_HERE.md`; `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md` (now only an interactive-steering prompt); HANDOFF (§For the maintainer added; §Rules, §What next and §HOLD FOR FABLE redefined); this file's §WHERE THINGS STAND, §Working mode, §Subagent orchestration and §Escalation policy. The superseded texts are kept under `engine/artifacts/lead_tmp/s19/` (`*_before_2026-09-14*`, `NEXT_SESSION_PROMPT_s20_fable.md`).
  **State:** nothing is running — `STOP` sits at the repo root and `engine/artifacts/loop/DRAINED` is left from the last run, and the maintainer's start recipe (START_HERE §Start) clears both. Six entries PARKED since 2026-09-10 (OC, SM, AT, SK, CT, ONCE) are the first landings; then L → 9 → W3 → 7 → C3 row 6 → WP, and the loop refills from HANDOFF §What next (P7's RE first). No gate value moved: the s19 GATES block stands.

- 2026-09-17 — **The maintainer: openbf goes ALL Opus — no Fable at all — at max effort, with a cleanse on a timer.** Three days after the 2026-09-14 change the loop had still not been started: the maintainer needs his Fable budget for his day job and cannot run Fable for work and for this project. Decision: **Fable is OFF here** (`FABLE_MAX` = 0) and the loop does the reverse-engineering itself; sessions run at `EFFORT=max` so they think longer on RE and on code; and a **CLEANSE job** falls due every 8 landings or 24 h so the docs, the code and the artifacts stay consolidated as the project grows. His framing: try full automation for a few days, and readdress Fable if the output is poor.
  **The RE discipline that replaces the Fable audit** (`engine/artifacts/briefs/OPUS_LOOP_PROMPT.md` §5, built from the 2026-09-14 trial's own failure modes): every claim tagged [V]/[D]/[H]; four mandatory self-checks before anything counts as [V] — name the enclosing function of every citation (both water runs in the trial cited a real line in the WRONG function), a search rather than a reading behind every absence claim, the twin open for every "BF2 does the same", the ELF bytes for every constant; then ONE `model: "opus"` verifier agent given the claims and citations but never the reasoning, briefed to falsify them. Results are lifted into `analysis/` under `# OPUS RE … [O] UNAUDITED`, every load-bearing claim tagged `[O]` (`grep -rn '\[O\]' analysis/` is the audit list) with constants kept PROVISIONAL, so a Fable sweep can be bought later if the budget allows.
  **The cleanse cadence** (§6b; checklist `.claude/commands/cleanse.md`): docs and artifacts every time — one live copy of each fact, superseded generations archived VERBATIM into `analysis/PLAN_LOG_2026.md`, `scripts/cleanse_delete.sh --yes` — plus one code area per rotation (core → game → render → bake → mungefs → manifest → scripts) for dead code, duplication, stale PROVISIONALs and weak tests, and a bug hunt over everything landed since the previous cleanse. No behaviour change: every gate value stays put. Findings are worked to zero or queued as `CLN<n>`; the stamp is `engine/artifacts/loop/LAST_CLEANSE`.
  **Changed:** `OPUS_LOOP_PROMPT.md` (§5 rewritten, §6b added, job B2 in §2), `scripts/autoloop.sh` (`EFFORT` default `max`, `FABLE_MAX` default 0), `scripts/AUTOLOOP.md`, QUEUE.md's rules and its [RE] mark, `engine/artifacts/loop/START_HERE.md`, HANDOFF (§Rules, §HOLD FOR FABLE → the RE list, §What next item 2), and this file (§WHERE THINGS STAND, §Working mode, §Subagent orchestration). The Fable-agent procedure is kept verbatim at `engine/artifacts/lead_tmp/s19/OPUS_LOOP_PROMPT_with_fable_re_2026-09-14.md` §5 — setting `FABLE_MAX` above 0 restores it. Still nothing running: `STOP` at the repo root and a stale `DRAINED`, both cleared by the maintainer's start recipe.

- 2026-09-17 — **The water MOVES: queue item OC landed on main as `b8ebe59` (the Opus loop's first landing as lead; lane 2, landing report `engine/artifacts/loop/reports/20260917_1610_land_OC.md`).** Two commits: the build `2650d07` (rebased from `aafb452`, unchanged — main's only new commits since are docs and `scripts/`, and `git diff --stat 5d18752..d042acc -- engine/` is empty) and `b8ebe59`, the lead's `main.rs` wiring that the build had to park because another lane held that file. Kamino authors `OceanEnable(1)` and now gets a 16×16 Tessendorf FFT patch per 160 m cell (`re-water.md` §3 [V]), instanced over the 1,618 cells of both KAM1 bakes out to the 640 m the PS2 draws water to, full divisions inside `NearSceneRange.b` and half past it; every flat-water plane bobs by `0.06·sin(1.885·t)`; `run_bench`, `run_headless` and the window loop step it at `water_anim::clock(r.time)`, with `OPENBF_WATER_TIME` pinning the phase for a reproducible capture and the PROVISIONAL per-vertex ripple still off.
  **Gates, lead-reproduced on main, nothing moved:** ci EXIT=0 **673** passed / 0 failed (662 + OC's 11); `scripts/gpu_tests.sh` EXIT=0, 24 passed, `git status --short engine/goldens` EMPTY; `sim_gates.sh` GATES_RC=0 at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both motion.csv identical to the s19 base; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp` 0 and 0 against `s19/baselines/sweep_main_1.txt` at `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`. **No re-baseline, no golden regenerated, no rebake owed** (the branch touches no `bake`/`mungefs`/`manifest` file) **and no walkabout owed** (`animate_water` is reachable only from `main.rs` and `examples/water_frames.rs`; `walkabout.rs` goes through `gallery::walkabout`, which renders straight off `upload_scene`). The one golden the game BINARY writes, `tat1_overview.png`, is byte-identical with the wiring in place — TAT1 has no water.
  **DECIDED (loop, 2026-09-17, lane 2): the ocean keeps `re-water.md` §3's 1/NM normalisation and its amplitude stays PROVISIONAL** — lead-measured rms **0.0125–0.0190 m**, peak −0.0385…+0.0415 m, bound **0.1461 m** over t ∈ {0, 0.5, 1, 10, 100}. §8's "metre-scale" is a [D] estimate that matches no normalisation §3 allows (the build measured ×16 → 0.22 m rms and ×256 → 3.52 m with a 37.4 m bound), and guessing metres would put that bound under the still far plane on an estimate. *Why:* §3 documents the transform the code implements; §8 documents nobody's transform. *Reverse:* new queue entry **OCA**, an [RE] job on the `h̃0` scaling, the iFFT's normalisation and the VU1 displacement (HOLD 21's remaining half) — the constant then changes in one place.
  **DECIDED (loop, 2026-09-17, lane 2): OC's "water mean abs > 1" gate was mis-specified, and it is met in substance.** At the golden vantage the sea moves by **0.002** mean abs over 75,271 water pixels — because W2's faithful tint leaves it near-black there, not because it is still. At a low vantage 4 m above the sea, t0 → t3.0 changes **22.62 %** of the frame (117,272 px, max 42; 13.58 % and max 81 at a second camera), bbox **y 254..539 — entirely below the horizon**, and the viewed changed-pixel mask shows coherent diagonal wave fronts with the platform pillars punched out as unchanged silhouettes. Nothing outside the water moves (0 px at the golden vantage). *Reverse:* OCA; if the disc's sea really swells, the amplitude is the fix, not the gate. Frames and the mask: `engine/artifacts/lead_tmp/water-moves/land_gates/lowcam/`.
  **DECIDED (loop, 2026-09-17, lane 2): the `--bench` cost is ACCEPTED at +0.030 ms.** Nine alternated cross-process pairs on bf2_kam1 give a median **+15.8 % mean / +17.6 % p50** (spread −8.3…+36.8 %), i.e. 0.190 → 0.220 ms — past the entry's 10 %, which was written before anyone had measured a frame. The in-process interleaved harness attributes **0.0128 ms** of it to the step itself (FFT + two uploads + the draws), so the added work is bounded, CPU-side and 0.09 % of a 60 fps budget; on a slower GPU it is a smaller fraction, not a larger one. *Reverse:* revert `b8ebe59` alone and the ocean stops being stepped while the code stays.
  **DECIDED (loop, 2026-09-17, lane 2): `main.rs` wired at this landing rather than after SM.** The parked patch applies clean and its hunks (1087–2470) cannot collide with SM's single hunk at line 380, so SM's rebase is unaffected; unwired, OC lands as dead code and the water does not move. *Reverse:* the wiring is its own commit.
  **Process:** the branch was gated once rather than twice (the rebase provably changed nothing under `engine/`), and `scripts/goldens.sh` was not run in its regenerating form — OC declares no golden move, so `goldens.sh --check` inside `gpu_tests.sh` is the byte referee for exactly that claim, on both the branch and main.

- 2026-09-17 — **Queue item SK landed on main as `f6cbf90` (lane 2; landing report `engine/artifacts/loop/reports/20260917_1637_land_SK.md`, build report `engine/artifacts/loop/reports/20260910_0105_stuck-census.md`).** One commit, one file, `game/examples/round_to_outcome.rs` (+367 −3): `STUCK_CENSUS=1` (or `=<n>` for another threshold than 5,000 ticks) makes the sweep list, on STDERR after the worker join, every alive unit whose `Route::stuck_run` exceeds the threshold at the last tick — team, class, position, the leg's from/to/dest hubs with the next hub's AUTHORED position, arc and route source, the distance to the leg's sub-point, the same-level `locate` and the re-plan hub, the steering's own wall probe, a soldier in that probe's path, the terrain above the feet, and one verdict (not-walking / off-graph / no-arc / wall / body / open) — plus a per-map summary naming the unit that set `stuck_max`, a verdict tally, the legs' next hubs and the three biggest position clusters. It only reads the sim. Parked 2026-09-10 on main `417a8cd`; the rebase onto `2522818` was clean (disjoint files — OC touched `game/src/`, not the examples).
  **Gates, lead-reproduced on main `f6cbf90`, nothing moved:** ci EXIT=0 **673** passed / 0 failed; `scripts/gpu_tests.sh` EXIT=0, 24 passed, `git status --short engine/goldens` 0 lines; the sweep pair `cmp` 0 and `cmp` **0** against `lead_tmp/s19/baselines/sweep_main_1.txt` (md5 `be656ec867f7a3d1492ed48863b7b859`, the s19 value), footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`; a census-ON run's stdout `cmp` **0** against that sweep — the census cannot move stdout, and the diff bears it out: the two added writes are `eprintln!`/`eprint!` and nothing else prints; `sim_gates.sh` GATES_RC=0 at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both motion.csv identical; `--sim-hash 100` `17e1b946ff0e821c`. Nothing rebaked, regenerated or re-baselined: no bake, render, manifest or arena code is touched, and `arena.rs` uses `game::walk::WalkSim`, not this file. The census stderr on main is byte-identical (timing lines stripped) to the branch run AND to the build report's own run of 2026-09-10, and its instrument holds on **32/32** maps (`longest run` == `nav_stats.stuck_max`).
  **What it measured (the four no-outcome maps, seed 42, 60,000 ticks — all lead-reproduced):** **bf2_mus1** 10 alive units over 5,000 ticks of 33 (team 0: 9) — verdicts wall 8, off-graph 1, body 1; nine team-0 units at z 88–92 walking SOUTH to hubs 31 (z 60) and 19 (z 54), 30–36 m short, on authored TABLE legs, seven of them in one knot at (−47.3,68.0,90.8) touching at 0.00–0.10 m; ZERO shots all round, tickets 150/150 at 60k and at 120k. **bf2_pol1** 8 of 32 (team 1: 7) — five at x ≈ −39..−40 on the 25 m floor beside their own post `cp2con`, walking EAST on arcs 0 and 1 against a face with normal −X, stuck since the round's first ~500 ticks. **bf2_tan1** ONE stuck spot, 4.9 m short of hub 102, units 14 and 15 taking turns in it; ZERO shots. **bf2_kam1** **0** units over 5,000 — not a stuck map at all, which corrects S0's grouping. A fifth no-outcome map, **bf1_kas2**, is stuck-driven (2 team-0 units, 23.5 m from hub 42).
  **DECIDED (loop, 2026-09-17, lane 2): the bleed gate at `core/src/cp.rs:293` is FAITHFUL — the tickets = alive stall is the disc's own rule, not a port defect.** KAM1 (16 tickets / 16 alive at 60k, 13/13 at 120k), TAN1 (12/12 at 120k) and bf1_kas2 (12/12) never finish because nobody dies, and `apply_bleed` stops while `alive() >= tickets()`. `analysis/re-tickets-spawn.md` §1.3 [V] quotes the PS2 Lua bytecode's own bleed timer doing exactly that — `if GetNumTeamMembersAlive(team) < GetReinforcementCount(team) then … else SetBleedRate(team, 0); StopTimer(timer) end` — so the clamp stays as written and SK's question 2 is answered from `analysis/`, without new RE. What is left open is the ONE exit the port does not implement, the time limit, and that is queue entry **TL**. *Why:* the citation is verified disc behaviour, so changing the clamp would be the deviation, not keeping it. *How to reverse:* if a PCSX2 capture ever shows a round draining past its alive count, change `cp.rs:293` and re-baseline the sweep in the same landing.
  **DECIDED (loop, 2026-09-17, lane 2): `Route::stuck_run` STAYS a `u16` and keeps saturating at 65,535.** SK found the saturation in 120,000-tick runs (`ai.rs`'s `saturating_add`). *Why:* the field is hashed, so widening it moves all three replay hashes, `--sim-hash 100` and the sweep's hash column — a full re-baseline for a diagnostic that only matters past 65,535 ticks, while every gate runs 60,000. *How to reverse:* a queue entry that widens the field and declares that re-baseline in its own landing.
  **DECIDED (loop, 2026-09-17, lane 2): the MUS1/POL1 walls are an RE question, not an Opus guess — queued as SW.** The census hands the RE exact positions, face normals, arcs and legs; the three candidate rules are a BF2 door the mission Lua opens (`core/src/ai.rs`'s barrier comment already asserts it, untested), a collision primitive the disc does not apply to soldiers, or an arc the disc's path data does not offer. *How to reverse:* drop SW from the queue. SK's own file set stays with C3 row 6, which is now free of SK (it still waits on SM).

- 2026-09-17 — **The water lattice yields ONE cell per patch, not a 3×3 block: queue item WP landed on main as `7ef36a8`, its golden as `c754d13`** (the loop's lane 1; report `engine/artifacts/loop/reports/20260917_1645_water-patch-cells.md`). `bake::water_glb` dropped every wet `WMAP` cell whose CENTRE fell inside a `CPAT` patch's AABB — and an `LPTC INFO` box is its own cell padded by half a cell on every side, so its edges run through the eight neighbours' centres and the rule took the whole block: **17 / 32 / 2 wet cells removed where the disc marks 4 / 8 / 1** (BF1 NAB2 / BF2 NAB2 / BF2 DEA1, which drew no lattice cell at all), holes in the water beside every patch. The disc's own answer is `WMAP` bit 3 — decoded 2026-09-09 (`analysis/re-water.md` §4 [V]) and never read by this code. `mungefs::water::Water::has_patch` is the accessor; `covered_by_patch` and its AABB test are gone; `bake/tests/water_patch_cells.rs` (new, disc-gated) pins the counts, one yielding cell per patch, `triangle_count == 2 × kept + the patches' own`, and — exactly, on all 13 shipped patches — that each `LPTC` box IS its bit-3 cell padded by half a cell, which proves the patch-to-cell pairing geometrically.
  **Gates, all lead-reproduced (numbers in the report's §2/§2b):** ci EXIT=0 **676** passed / 0 failed on the branch and again on main (673 before the merge: OC's 673 + SK's 0 + WP's 3); `scripts/gpu_tests.sh` EXIT=0 24 passed with `git status --short engine/goldens` empty on both sides of the golden commit; bake bytes before the merge `SAME=5301 **DIFF=6**` — exactly `bf1_nab2/{water.glb,manifest.ron}`, `bf2_nab2/{…}`, `bf2_dea1/{…}`, the manifest delta ONE field each (`triangles` 120→146, 480→528, 12→14 = 2 × the 13 / 24 / 1 recovered cells) and the other nine water maps' `water.glb` byte-identical; after `scripts/rebake.sh maps` a fresh scratch bake of main is **SAME=5307 DIFF=0 ONLY_IN_A=0**; `sim_gates.sh` GATES_RC=0 on main AFTER the rebake (`bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both motion.csv identical), `--sim-hash 100` `17e1b946ff0e821c`, the sweep pair `cmp` 0 and `cmp` 0 against `lead_tmp/s19/baselines/sweep_main_1.txt` (footer `finished 27/32 … sum 213000`) — run on the branch, on the rebased branch and again on main once the new manifests were in the shipped tree.
  **ONE golden moved, quantified before it was viewed:** `goldens/maps/bf1_nab2.png` — mean abs **0.0045**, max 34, **298 px = 0.029 %**, bbox x 510..686 y 217..256, the changed pixels darkening (RGB 82.5/64.0/35.6 → 59.3/48.8/26.8) where the water plane now covers a wedge of bright stone beside Theed's green dome. bf2_nab2's and bf2_dea1's goldens did not move and no undeclared golden moved in the 90. The walkabout was regenerated for the three maps: 5 of 22 panels move, the largest 0.0093 mean (bf1_nab2 `vantage`), 17 byte-identical. Sheets and plan views: `engine/artifacts/lead_tmp/water-patch-cells/{pairs_00.png,sheets/}`.
  **DECIDED (loop, 2026-09-17, lane 1): `has_patch` reads the `WMAP` nibble rather than a patch→cell index.** The parser reads every `LPTC` under `CPAT` without recording which cell it belongs to, so "the nibble has bit 3" and "a patch is linked here" are two counts of the same thing — equal on all 14 shipped chunks (`mungefs/tests/fx_lvl.rs`), and the new disc test re-asserts it per map. *Why:* carrying `(x, z)` on `WaterPatch` is a `mungefs` API change this entry does not own, and the nibble is what the PS2's parser itself walks. *How to reverse:* add the field and switch the one predicate.
  **DECIDED (loop, 2026-09-17, lane 1): the sub-tolerance golden is regenerated anyway** (the move is inside `game/tests/common/mod.rs`'s 1.5 / 1 % limits, so no test failed either way) — the committed PNG should be what the code draws, and the entry declares these three maps. *How to reverse:* revert `c754d13`.
  **RE, lifted into `analysis/re-water.md` §4 as a CORRECTION tagged `[O]` (UNAUDITED — Fable is off):** the patch→tile match is an EXACT INDEX equality, `z*grid + x` against the patch record's own pair, and the packet reaches `tile+0x24` only on equality — BF1 `FUN_004770a8` 539306–539324, BF2 `FUN_00561b58` 696884–696892, each read with its enclosing function printed. §4's port remark ("`water_glb.rs`'s rule is right") was too kind to the port and now says so. An Opus verifier agent, briefed with the citations only and asked to falsify, supported the match in both games and **did not support §4's "an either/or in every sub-pass"**: the flat-water draw is a **three-way** (`FUN_0048b9c0` 551345–551358 — `+0x24 == 0` falls back to `DAT_005649c0`/`DAT_005649c4` before the flat template) and **the OCEAN twin never reads `+0x24` at all** (`FUN_0048b280` 550922–551180, reached from `FUN_0048b1e0` when `OceanEnable` is set), so "a patch replaces its cell" is the flat-water path's rule only. Both were re-read by me before being written down, and **neither touches the port**: no shipped ocean map ships a `CPAT` (both KAM1s: 0 patches, asserted by the new test). Also corrected there: `tile+0x24` holds a DMA packet pointer (Ghidra types it `float` at the store, `int` at the load), and the cull's walk is a merge-join whose correctness rests on the parser's row-major append order (538776–538781).
  **Measured and left open:** BF2 NAB2 gains 48 triangles and moves **not one pixel** of its golden, its 7 walkabout panels or a straight-down plan view — yet **9.5 %** of the frame changes from an eye 2 m over its water plane, so the cells are drawn and simply live under Theed's plaza. That is the same shape as queue **WA**'s open question about bf1_kam1, and it is noted there. `--cam-overview` renders an empty frame on bf2_nab2 and bf2_dea1 (measured in passing; §What next item 4 already carries the `--cam-overview` question).

- **2026-09-17 (evening) — CLEANSE (loop, lane 2; the first under `OPUS_LOOP_PROMPT.md` §6b): the live docs now hold ONE copy of each fact, and the queue costs every session half of what it did.** Job B2, claimed as `locks/CLEANSE`; `LAST_CLEANSE` was missing, so the code-area rotation starts at its head, `core`. Findings file `engine/artifacts/loop/cleanse/20260917_core.md`, report `engine/artifacts/loop/reports/20260917_1900_cleanse_core.md`. Three Opus gathering agents (docs↔reality, `analysis/`↔code, the `core` crate) ran read-only while lane 1 landed WP; every finding quoted below was re-verified by the lead before it was acted on.
  **Consolidation (nothing about behaviour moved; no gate was run and none could move):** `QUEUE.md` 96,711 → 52,183 bytes — the 31 DONE entries are one-line rows (label, date, sha, report) and their full text is verbatim in `engine/artifacts/loop/QUEUE_DONE_ARCHIVE.md`; both rewrites were compare-and-swap guarded on the file's md5 because lane 1 was editing its own entries at the same time (its WP edit arrived between the two and survived). `START_HERE.md` 229 → 105 lines — eight superseded status sections and a 2026-09-07 cleanse-items section moved verbatim to `START_HERE_ARCHIVE.md`, and the live status now says the loop IS running. HANDOFF's §State is one live snapshot again.
  **DECIDED (loop, 2026-09-17, lane 2): the gate VALUES live in exactly one place — `QUEUE.md`'s GATES block.** PLAN §WHERE THINGS STAND and HANDOFF §What next / §State now point at it instead of restating it. Why: three copies existed, two of them claiming to be canonical, and HANDOFF carried `ci 662 tests` for a day after ci reached 673 — the GATES block is the copy a landing rewrites mechanically (§3.9), so it is the one that cannot drift; this Log stays the durable record of each value at the moment it moved. Reverse: `git show 834b40a:PLAN.md` has the old block verbatim.
  **DECIDED (loop, 2026-09-17, lane 2): superseded text in TRACKED files is deleted, not copied into `analysis/PLAN_LOG_2026.md`.** Git is the archive for tracked files (`git show <sha>:HANDOFF-OPUS.md`); the `analysis/` archive earns its keep only for text that would otherwise be lost, and copying 6 KB of s19 snapshot into it would have grown the live tree, not shrunk it. Untracked files keep getting real archive copies (`QUEUE_DONE_ARCHIVE.md`, `START_HERE_ARCHIVE.md`). Reverse: paste from git.
  **Truth fixes, each re-verified by the lead before the edit:** PROVISIONAL is **189 occurrences across 46 files**, not 171/42; PLAN said a loop session gets its RE from a Fable agent and "never derives PS2 behaviour from the decomp on its own" — false since 2026-09-17, it does §5 RE itself; P6.5's phase line carried no DONE markers though A–D are landed; 18 Log topic-index rows pointed at entries that live in the archive with no `→ archive` marker and eight of those quoted titles that exist nowhere (now the 2026-08-31 summary); HANDOFF's standing backlog listed the broadphase, parry `enhanced-determinism` and the RON twins as work when all three are built; HOLD 10(i) (`LOWR`) was listed open although `analysis/re-lowr.md` answered it on 2026-09-08; `scripts/gpu_tests.sh` "the ten render referees" is 24 tests today.
  **`analysis/` errata (the lead read both sides of each):** RE_LOG's "each patch covers 3–24 % of its tile" now carries WP's correction inline (the box is the cell padded by half a cell on every side, so coverage over the cell it replaces is 13–96 %); `re-water.md`'s §6 draft paragraph and RE_LOG's copy of it are marked RETRACTED where they still said `water_glb`'s either/or rule was verified; §8's "metre-scale waves [D]" carries OC's measurement (1.25–1.90 cm rms under §3's normalisation) and the OCA pointer; the file's LEAD VERIFICATION header now scopes itself to the 2026-09-09 body, with §4's correction marked `[O]`; W1/W2 are LANDED, not TODO; `re-stance-roll.md`'s `0x11 stand_getupfront` is corrected to **0x14** inline (the ELF table at 0x5fde48; 0x11 is `thrown_bouncebacksoft`); `re-sky-dome-placement.md` §5's alpha-tested volcano is marked superseded by its own §0 LEAD CORRECTION 2. Citations that named `mungefs/src/water.rs:201` now name the symbol — `7ef36a8` moved it to :241.
  **Artifact READMEs (what the maintainer has been shown):** `texture_tiers/README.md` said tier 2 is not loaded at runtime — it has been since T2R (`OPENBF_TEXTURE_TIER`, 363 files); `gallery/README.md` said 42 committed sheets and 4 unit sheets (90 and 52); `global_sound/README.md` put the CORE banks under `audio/`; three READMEs named raw-render directories the delete script pruned. All fixed in place.
  **`core` (the rotation's first area) came back CLEAN on the things that matter** — zero `#[allow(dead_code)]`, no unused dependency, no `std::env` read, no wall clock, no `HashMap` in production code, no `mungefs` dependency, one `unwrap` and it is provably guarded, 251 tests with no assertion-free body, and 27 commits since 2026-09-08 re-read for equivalence with no defect found. Six findings, none of them behaviour: the broadphase's `pub bodies` + length-only staleness stamp (already P7's to close, now stated in HANDOFF's backlog item 1); a `commander::noise` doc comment that names the wrong sibling and would invite a hash-moving "cleanup"; the `fmath` host-libm ban missing `hypot`/`exp2`/`log2`/`cbrt`/…; two copy-paste twins in `squad`/`nav`; and five stale line numbers in `core/Cargo.toml`'s justification for the `ron` dev-dep.
  **Artifacts:** `scripts/cleanse_delete.sh --dry-run` has **nothing left to take** — the repo's 195 GB is 118 GB of `lead_tmp`, and 94 GB of that is the four parked entries' cargo caches (~20 GB each), correctly kept while their branches live. Disk is at 30 % of 929 GB. No orphan branch or worktree; `git ls-files` holds no asset, ELF, decomp or baked path. After the cleanse's own branch was landed and removed, `cleanse_delete.sh --yes` took 12 GB and the repo is **176 GB**.
  **The cheap fixes landed as `9edff01` (comment-only, through §3 like any branch):** twelve comments the code had outgrown, `git diff main...HEAD` with **0 non-comment lines**, ci EXIT=0 **676** passed / 0 failed on the branch AND on main, `scripts/gpu_tests.sh` EXIT=0 **24 passed** with `git status --short engine/goldens` EMPTY (run because the script's own header was one of the twelve). No sim, sweep, bake or golden gate can move from a comment, and none was run beyond those two.
  **A flake the cleanse caught by accident, and it matters:** the FIRST `./ci.sh` on main after the merge FAILED — `game --test roster`, `roster.rs:256` panicking with `kam1 rosters: parse classes.ron` / `4114:27: Expected end of string`, i.e. a half-written `classes.ron`. The test then passed **3/3** on an immediate re-run and the next full ci was green at 676/0. That is the in-process same-file bake race queue **ONCE** closes (PARKED, branch `roster-oncelock`): **ONCE is a FLAKE FIX, not a speed-up, and should land early** — the evidence is on its queue entry. Two new queue entries: **CLN1** (the `fmath` host-libm ban misses `hypot`/`exp2`/`log2`/`cbrt`/… — a guard gap with no live violation, and its gate must prove the guard can fail) and **CLN2** (`main.rs`'s header documents 2 of the 6 env vars it reads; waits on SM, whose file it is). Declined with reasons: the `squad` Follow/Hold release twins and `nav`'s inlined `dist_to_segment` — duplication in hashed code with no failure scenario, to be folded only when that code is touched for a real reason.

- **2026-09-17 (evening) — RE (loop, lane 1, job D, queue OCA): Kamino's sea is METRE-scale — the PS2's inverse FFT is unnormalised, and nothing between the spectrum and the vertex scales it.** No branch, no code change, no gate run and none could move: this entry only reads the discs and writes `analysis/`. Lifted into `analysis/re-water.md` **§3a** under the `# OPUS RE … [O] UNAUDITED: no Fable pass` header with every load-bearing claim tagged `[O]`; RE_LOG pointer entry added; HANDOFF §HOLD FOR FABLE 21 amended; scratch, scripts and measurements under `engine/artifacts/lead_tmp/re/water-amplitude/`; report `engine/artifacts/loop/reports/20260917_1745_re_OCA.md`.
  **The three answers OCA asked for.** (b) `FUN_004cea50` (BF1 593638–593672, BF2 `FUN_005c4488`) divides by `N·M` **only when its 4th argument is non-zero**, and the water passes ZERO — read in the bytes, not the decompiler: `00477b00: 0000382d daddu a3,zero,zero` before `00477b04: jal 0x004cea50` (BF2 `00562598` / `0056259c`). The direction is a hardcoded `+1` in the wrapper's delay slot (`004cea7c: addiu a3,zero,1`, against the forward wrapper's `-1` at `004cea3c`), and the 1-D butterfly (`FUN_004cece8`, Numerical Recipes `four1`) scales nothing. **§3's "1/NM-normalised" was wrong and is corrected inline; §8's metre-scale [D] was right.** (a) Nothing scales `h̃0`: the mesh writer `FUN_00477cd0` hands the VU a **unit cube** (`y = (h − min)/(max − min)`, 539780) and the only metres in the draw are the per-tile record's two corners, `(x0 − cell/2, base+bob+min, z0 − cell/2)` and that `+ (2·cell, max − min, 2·cell)` (`FUN_004770a8` 539327–539330) — the engine's generic vertex-dequantisation box (`FUN_0039bd98`, 57 call sites, 31 of them beside a `FUN_00398928` mesh submission — the same convention `mungefs/src/model.rs:237–250` already implements for PS2 models). (c) The VU1 therefore **cannot** rescale the displacement — and the ocean does not even run the water microcode: `FUN_0048b280` references neither VU program `0x515410`/`0x514ec0` nor the LUT block `0x754a30` (3 hits each, all in the flat-water path). Its per-vertex alpha is written on the EE — the FOAM byte, halved (550727) — which answers the ocean half of §8's "exact per-vertex alpha".
  **Measured, on the disc's own formulas re-implemented hop for hop** (`disc_ocean.py`/`amplitude_stats.py`; the hand-written transform and numpy agree to 2.6e-14 m, and the model reproduces the port's own OC numbers to three digits). KAM1's authored inputs are identical on both discs (`WATR INFO` cell 160 m, INFO5 1; `PatchDivisions(16,16)`, `WindSpeed(25)`, `WindDirection(0.2,1)`, `PhillipsConstant(1e-05)`, `LODDecimation(2)`, `LAYR` height −8.0 — re-read 2026-09-17). Over 320 frames: peak-to-trough **median 19.23 m** (p5 13.0, p95 30.3, max 41.9), rms **4.145 m**, max |h| 23.5 m, `bound` 37.0 m — against the port's **0.0751 m / 0.0162 m / 0.1445 m**. An independent check settles it without the VU: the disc's foam byte (`FUN_00477cd0` 539941–539985, threshold 3.0 read as ELF `0x40400000` at `004782a4`) fires on a median **13.3 %** of the mesh points unnormalised and on **0.00 %** under the port's `1/(N·M)` — at the port's scale KAM1's whole foam mechanism is dead code.
  **DECIDED (loop, 2026-09-17, lane 1): the port's `1/(N·M)` is a defect, not a reading — but it is fixed in its own entry, not here.** Why: OCA declares "no code change; every gate value stays where OC left it", and the fix moves two KAM1 goldens, so it needs its own before/after sheets. Queued as **OCB** with the two lines that must move together (`water_anim.rs:332` `Ocean::inverse` and `:368` `Ocean::bound` — changing only the first leaves the far-plane sink 256× too shallow) and the `drop` re-derivation. How to reverse: OCB is one constant; §3a records both numbers, so restoring `1/(n·n)` restores today's sea.
  **What stays [H] (and what it would take):** the generic mesh VU program is unread, so the dequantisation itself is [D] — a PCSX2 GS dump of KAM1 would close it, and with it why the box is two cells wide (a horizontal choppy displacement would want exactly that half-cell margin) and whether the far LOD is displaced the same way. HOLD 21(a) keeps the flat-water LUT, the phase nibble and the far LOD.
  **The verifier agent (§5's falsification half; one Opus pass, citations only, no reasoning shown to it) SUPPORTED every load-bearing claim** — both discs' normalisation flags and `±1` directions in the bytes, the unit cube, the packing, the tile record, the `OceanEnable` gates (it derived the FNV-1a key hashes itself and matched them to the dispatcher), the Phillips formula, every ELF immediate, and the two `KAM1.LVL` payloads read byte for byte. **It falsified the WORDING of two absence claims** (the foam map is touched in four functions and the transform buffer in three, counting the alloc/free/pointer-init sites I had not listed — neither alters a value) and corrected four line ranges, all folded into §3a before the commit. It also caught two things I had missed: `FUN_00478bd8` stores THREE floats, so the 0x30 tile record is two PADDED vec3s; and **the disc clears its foam map with `memset(ptr, 0, 1)`** (539465, ELF `004776ec: addiu a2,zero,1`) although it allocates `(N+1)·(M+1)` bytes — a shipped bug that the −4/frame decay hides within ≤64 frames.
  **What the RE enables, queued:** **OCB** (the amplitude fix above) and **OCF** — the ocean is TWO passes on the disc (opaque base `kam1_water_2` at `Tile(4,4)`, then `FoamTexture('kam1_water')` at `FoamTile(5,5)` blended by the per-vertex foam byte) where the port draws one, with the base sheet baked and never drawn and `color: [255; 4]` in `water_anim.rs:411`. That pass is now fully specified EE-side, so it is buildable; it depends on OCB because the foam rule cannot fire at today's amplitude.

- 2026-09-17 — **The squad marker draws by its own alpha and is ON: queue item SM landed on main as `f21c2db`** (lane 1, job B; branch `squad-marker-on`, build report `engine/artifacts/loop/reports/20260910_0037_squad-marker-on.md`, landing report `20260917_1815_land_SM.md`). The branch had been parked since 2026-09-10 waiting on a scope call and then on the MAIN claim; it rebased **clean** onto `f3c2097` as `ba3a5b8` + `0c6c145` + `f21c2db` (of its five files only `game/src/main.rs` had moved under it, in OC's water wiring `b8ebe59`, and not at the marker's lines). Five files, +231 −48.
  **What it is.** The bake forced every texture that no top-level or mode-layer model segment names OPAQUE, so the two HUD sheets the squad marker draws arrived alpha-255 everywhere and an alpha-tested quad drew the whole tan square — which is why the marker had shipped behind `OPENBF_SQUAD_MARKER=1`, default off, since Phase D. `bake::alpha::keep_alpha` now takes an `unnamed` input and lets such a sheet keep its DISC alpha; `game/src/squad_vis.rs` draws the marker `DrawLayer::Blend` at element alpha 1.0; `game/src/main.rs` turns it ON unless `OPENBF_SQUAD_MARKER=0`. The PS2 fact behind it is `analysis/re-hud-icons.md` §2 (lead-verified 2026-09-09): every HUD bitmap on both discs is composited source-over by its CLUT alpha (ALPHA 0x44, TEST 0x31001 — no cutout, no Z) and the icon's shape IS that alpha.
  **DECIDED (loop, 2026-09-17, lane 1 — taken before the landing, executed as commit `f21c2db`): the unnamed-alpha clause is the COMMON bakes' only.** `bake_content` takes a `Content` (`Common` | `Map`) and only a common bake asks the question. *Why:* the [V] evidence is the HUD's and those sheets live in COMMON.LVL / INGAME.LVL; applied to every bake the same rule also freed 194 map-directory files and 8 tier-2 files, and on the water maps the water pass composites by texture alpha — four map goldens went past their limits (bf1_kas2 2.854 mean / 26.9 % strong, bf2_dag1 2.269 / 11.4, bf2_kam1 0.875 / 5.0, bf2_kas2 0.753 / 2.5) on a rule whose governing PS2 fact is UNREAD: `re-water.md` §3 tags As "[V table, D blend]" and §8 lists the exact per-vertex alpha as unread VU1 (HOLD 21). *How to reverse:* pass `Content::Map` through the same predicate — one boolean at two call sites. The map half is queued as **WA**, an [RE] entry that reads the VU1/TEX0 question first and then decides.
  **Gates on main, every exit code read directly, nothing taken on the build report's word.** ci **EXIT=0, 679 passed / 0 failed / 24 ignored** on the rebased branch AND again on main (676 + SM's three named tests, all three seen in the log). `scripts/gpu_tests.sh` **EXIT=0, 24 passed** on the branch and on main, `git status --short engine/goldens` EMPTY both times. **[REBAKE]:** on the branch a FULL scratch bake vs the shipped tree gave **SAME=21363 DIFF=288 ONLY_IN_A=0 ONLY_IN_B=0** — 201 `common_bf1/textures/*.ktx2` + 86 `common_bf2/textures/*.ktx2` + `common_bf2/textures/map_mask.tier2.ktx2`, every path a `.ktx2` in a common directory and **not one map, side or map-tier-2 path** (`kam1_water` byte-identical, so KAM1's foam stays off until WA); the census printed on exactly 2 bakes (204 names → 201 files; 86). Then `scripts/rebake.sh common` on main and a full scratch bake of main against the result: **SAME=21651 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0** — the shipped tree is a from-scratch bake of main's code over all 21,651 files. **[GOLDENS]: `scripts/goldens.sh` regenerated every golden on main and NOTHING moved** (`== changed goldens ==` empty, `git status --short engine/goldens` empty) — the option-(b) declaration held exactly, so there was no pair sheet to build and none to view.
  **The two load-bearing RE numbers re-measured by the lander from the BAKED BYTES** (a 40-line KTX2 level-0 reader, `lead_tmp/squad-marker-on/land/ktx2_alpha.py`, not the crate's own test): `squad_cmd_highlight.ktx2` 64×64 goes from **0 alpha-0 texels of 4,096 to 1,594**, `squad_cmd_cross.ktx2` 128×128 from **0 of 16,384 to 7,732** — exactly `re-hud-icons.md` §2's CLUT counts, so the RE's central claim is now proven end to end from disc CLUT to shipped KTX2.
  **DECIDED (loop, 2026-09-17, lane 1): the sim gates were run although §3.9 does not owe them, and no walkabout was regenerated.** *Why:* the merged diff holds no sim file (`main.rs`'s change is the marker's default flag and its doc), but the landing rewrote 288 baked files, and a rebake is exactly the thing that could move a hash by accident — so sim_gates (**GATES_RC=0**, walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, both `motion.csv` identical to base), `--sim-hash 100` (**`17e1b946ff0e821c`**) and the D14 capture (**`ebb933882466ff06`** over 6,000 ticks, the same two `command …,6` events) were all run and all unmoved. The sweep pair and the arenas were NOT run: no sim or AI file is in the diff and the three replay hashes plus the 6,000-tick capture already cover the rebake's reach. The walkabout was not regenerated because zero map bake files moved and `game/examples/walkabout.rs` references neither `squad_vis` nor any common sheet (grepped). *How to reverse:* both are one command.
  **The look, VIEWED and ACCEPTED on main** (`lead_tmp/squad-marker-on/land/d14_landed_walk_0{2,4}000_before_after.png`, 2× crops against the pre-landing capture): **17,462 pixels** of `walk_02000.png` differ — the build report's own figure, reproduced — and they are the squares. The tan backing behind each green arrow is gone and sky and rock show through the alpha-0 texels; at tick 4,000 the cross loses its blue-grey square and the vehicle seats show through it. Deviation kept and recorded: the marker still takes the depth TEST (`DrawLayer::Blend` tests depth, writes none), so a wall hides a marker, where TEST 0x31001 has no Z at all — the PS2 draws these two sheets as a 2D D-pad command MENU at HUD (−120,−50) and never over a bot, so our in-world placement is already the PROVISIONAL deviation and an occluding wall is the coherent reading of it; reversing it means a no-depth `DrawLayer` variant in `render/src/lib.rs`, a file other entries own today.
  **DECIDED (loop, 2026-09-17, lane 1): the GATES block's ci count moves 676 → 679 as this landing's own declared move, and CLN1's predicted 677 is now 680.** *Why:* the block is rewritten mechanically by each landing (§3.9) and CLN1 was parked against the pre-SM count; leaving it would hand the next lander a wrong expectation for the second time this evening. *How to reverse:* one line in `engine/artifacts/loop/QUEUE.md`. Also fixed in place: `engine/artifacts/squad/README.md`'s closing section still told the maintainer the marker was off by default because the sheets carry alpha 255 on every texel — both halves untrue since this commit.

- 2026-09-17 — **The MASK cutoff is the PS2's: queue item AT landed on main as `a55688f`**, with the goldens `565d2e0` and the comment fix `74ee140` (lane 1, job B; branch `mask-cutoff`, build report `engine/artifacts/loop/reports/20260910_0104_mask-cutoff.md`, landing report `20260917_2015_land_AT.md`). It was the oldest PARKED entry — parked 2026-09-10 and deferred by three sessions because it moves goldens — and it was the one blocking four TODO entries (L, 9, W3, 7) on the render files. It rebased CLEAN onto `9ebabea`: neither `render/src/shader.wgsl` nor `bake/src/detail.rs` had moved on main since the branch's base `417a8cd`.
  **What it is.** `fs_mask` discarded below 0.5 because that is glTF's default, not because anything on the disc says so. The disc's own test, composed from three [V] facts (`analysis/RE_LOG.md` "Alpha-test cutoff in disc terms", 2026-09-09): a 0x02 cutout segment is tested ATST GEQUAL at AREF 0x60 = 96 on the GS's 0..0x80 scale with TFX MODULATE and an opaque vertex colour (Av = 0x7f), and the `pal_` loader uploads every CLUT alpha as `(A·0x81)>>8` — so a texel survives iff `((A·129>>8)·127>>7) >= 96`, i.e. **disc alpha ≥ 193**. `MASK_CUTOFF = 0.753` sits between 192/255 and 193/255, ~1000 ULPs clear of the lower one; `bake::detail::ALPHA_TEST` is the same number in 8 bits and its test now pins shader-keeps == PS2-keeps == `a >= ALPHA_TEST` for all 256 alphas.
  **DECIDED (loop, 2026-09-17, lane 1): the disc's cutoff is the WORLD's only — skinned draws keep the port's own 0.5, through new `fs_mask_skinned` / `fs_mask_skinned_fade` entry points** (commit `a55688f`, which is the landing's own commit on top of the build's). *Why:* the composition is derived for 0x02 CUTOUT segments, and a soldier draw has no verdict to be 0x02 — the side bake is not on ALPHA RULE v2 (design D6) and `bake::skin_glb` writes no `alphaMode` at all (verified: all 97 materials in the shipped `side_bf*/soldiers/*.glb` carry none), so ONE test serves every segment of every side model. The branch as built moved that test too, silently contradicting `render/src/lib.rs:926`'s own stated intent ("skinned draws keep exactly the test they have always had"), and the cost is measured: at 193 BF2's `all_inf_wookie` keeps **835 of its 65,536 texels** (4,299 at 0.5) — its alpha is a smooth fur channel, not a cutout mask — while BF1's `wookie` loses 0.5 % of its kept texels. So the port would have shipped a nearly invisible unit on an unproven extension of a proven rule. *How to reverse:* one constant in `shader.wgsl`. The question itself is queued as **SKA** ([RE] then build — D6's unanswered half), and it is why **no unit golden moved in this landing at all**.
  **Gates on main, every exit code read directly.** ci **EXIT=0, 680 passed / 0 failed / 24 ignored** on the rebased branch and again on main (679 + the shader-split test), and a third time after the comment commit. `scripts/gpu_tests.sh` on the branch **EXIT=101 BY DESIGN** — the declared [GOLDENS] failure, 5 of 32 map goldens past the limits and the render referees 17/17 green; the four golden binaries cargo skips after that failure run clean separately (**4/4, EXIT=0**). On main after the regeneration: **EXIT=0, 24 passed**, `git status --short engine/goldens` EMPTY. `sim_gates.sh` **GATES_RC=0** — `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both motion.csv identical to base — and `--sim-hash 100` **`17e1b946ff0e821c`**; run as insurance, not because §3.9 owes them (no sim file in the diff). **No rebake, and none was owed:** `ALPHA_TEST`'s only reader is `tier::Resolved::alpha_test_deletes_it`, whose only callers are its own two tests (grep over `engine/crates`), so no baked byte can move.
  **[GOLDENS]: 27 regenerated, every one quantified BEFORE it was looked at** (`lead_tmp/mask-cutoff/land/quantify.txt`): bf1_end1 3.188 mean / 11.69 % strong, bf2_kas2 2.333 / 5.19, bf1_nab2 0.971 / 1.95, bf2_end1 0.849 / 3.33, bf1_yav1 0.785 / 1.99 — the five past the test's limits are exactly the five the branch declared — then 22 under them, down to three tat1 goldens at 0.000 mean and 17 changed pixels. **VIEWED as committed-vs-regenerated pairs, all 14 sheets** (`land/pairs_*.png`, plus bbox crops at 3×): END1's fir canopy goes from dense cones to lacy spindles with the mist through it, KAS2's canopy and platform rims thin, YAV1's bare trees keep their branches and lose the twig speckle, Theed's railings and planters thin, GEO1's rafter lattice thins. The strongest single loss is TAT2's `tat2_bldg_crashed_spaceship3` grille, which reads as a few scratches where it was a dark lattice.
  **The look was accepted on evidence, not taste: the thinning is the disc's arithmetic on the disc's own data.** The mip chain is the LVL's own (`mungefs::texture` asserts `mips.len() == info.mip_count`) and the PS2 samples it — `analysis/re-texture-wrap.md:67,154` reads TEX1 = MXL `(mips−1)<<2` | MMAG linear | MMIN 4/5 on the same VU1 register list that carries TEST_1 — and the fraction of kept texels sitting in the newly discarded band GROWS with the mip level (`end_prop_tree_leaves`: mip0 26 %, mip3 64 %, mip5 100 %; `tat2_bldg_crashed_spaceship3`: mip0 15.1 %, mip2 56.8 %). At 512×448 the disc picks a coarser mip than these 1920-wide panels do, so the disc's foliage is at least this thin and probably thinner. The walkabout was regenerated in full (219 images over 32 maps, 20.6 s): bf1_end1 moves 2.87–4.63 mean on all six panels, and two tat2 panels came back byte-identical as the determinism control.
  **DECIDED (loop, 2026-09-17, lane 1): rule 1's `NEVER_TESTS` (128) stays where it is and becomes queue NT.** *Why:* it was the shader's threshold and is now decoupled from it; raising it to 193 would re-verdict every texture whose alpha max lands in 129..192 from MASK to BLEND, which is a rule decision that moves baked bytes — measured at this landing, no MASK-verdict texture has its alpha max in that band, so nothing is lost by waiting. Its doc comment and `AlphaMode::Mask`'s (both said "0.5") were corrected in `74ee140`. *How to reverse:* drop the NT entry.
  **`analysis/` corrected where this landing made it untrue:** RE_LOG's cutoff entry said "Ported: no" and now records the sha, the world-only scope and the mip evidence; `re-material-flags.md` §6 (c)/(d) stated AREF 0x60 in DISC alpha terms as 0.376 — written before the CLUT scale was known — so `myg1_mountains` (alpha max 115 → GS 57) would pass NOWHERE, not "where α ≥ 96", and that erratum now sits inline beside the claim. The GS-side readings in that section are unaffected.

- **2026-09-17 (evening) — RE (loop, lane 2, job C/D, queue SW): the wall that pins MUS1's team 0 and POL1's team 1 is a DOOR, and BF2 doors open by themselves.** No branch, no code change, no gate run and none could move: this entry reads the discs and writes `analysis/` only. Lifted into `analysis/re-bf2-doors.md` under the `# OPUS RE … [O] UNAUDITED: no Fable pass` header with every load-bearing claim tagged `[O]`; RE_LOG pointer added; HANDOFF §HOLD FOR FABLE 4 amended and §What next item 2's SW line closed; scratch, scripts and measurements under `engine/artifacts/lead_tmp/re/stuck-wall/`; report `engine/artifacts/loop/reports/20260917_1835_re_SW.md`.
  **The answer, against SW's own three candidates.** (a) a BF2 door — **YES for the door, NO for the Lua**: BF2 has an engine entity class `door` (label at VA 0x60fb68, registered by the class-label factory `FUN_00173d10` with a 0x380-byte class object built by `FUN_001ec428`), and its tick `FUN_001ebb80` — reached only through the thunk `FUN_001ec850` at +0x0c of the secondary vtable 0x61aff8 — opens it whenever an object whose collision LAYER ID is 2 (the soldier layer) enters a sphere of the ODF's `AnimationTrigger` radius around a named hardpoint; the mission Lua never OPENS one — it only locks one (`IsLocked`), and the four conquest scripts of these two maps contain no `SetProperty` of any kind. (A scripted doorway is a different mechanism: POL1's conquest binds `PlayAnimation("Airlockclose")` to a CP's capture on `Pol_prop_airlock_door`, whose entc BASE is `prop`, not `door`, and whose two instances sit 185–225 m from the stuck loci.) (b) a primitive the disc does not apply to soldiers — **NO**: the door authors no `SoldierCollision`, so by `re-collision-layers.md` §0.3 its box IS in the disc's soldier subset; it is the box that MOVES. (c) an arc the disc does not offer — **NO**: every stuck leg is an arc of the disc's own `plan` chunk, and `MUS/mus1c_con.lua:18` calls `UnblockPlanningGraphArcs("Connection74")` at mission start — the very arc the seven-unit knot stands on.
  **Measured this session, not taken from any report.** All **13 `wall` verdicts** on bf2_mus1 (8) and bf2_pol1 (5) are units pressed against one of FIVE door instances, with a 14th queued behind them on a `body` verdict; each of them stands **1.14–2.72 m** from the hardpoint whose ODF radius is 3.0–5.0 m, i.e. inside the sphere that would have opened the door on the PS2. In the port's own world a 0.5 m-radius flood-fill through a 5 m corridor at chest height is **impassable at all five doorways with the baked bodies and passable at all five without them**. The disc's `open` clip translates the bone the collision box hangs off by **3.965 m in 1.00 s** on `mus1_prop_door_tri` (31 frames, MINA CRC 0xd02e3793 = crc32_lower("open"), track 2 = `bone_door`), **±2.250 m** for POL1's two leaves and **3.444 m** for the garage door, which animates `boneroot` itself. The port bakes that box at frame 0 — `game/src/assets.rs:468-489` composes a prim's XFRM straight onto the instance matrix, "PRNT node assumed identity" — so **50 of 66 door instances over 5 BF2 maps (156 collision bodies) stand as permanent walls within 1.5 m of an authored planning arc.** BF1 has no door ENTITY CLASS: **zero** case-insensitive "door" bytes in `SWBFRONT.ELF`; a class-label table (`FUN_00172ab0`, 35 labels at 0x527110–0x527340) that runs `armedbuildingdynamic → trap` exactly where BF2's runs `armedbuildingdynamic → door → trap`; and **0 of 788** BF1 entc classes declaring BASE "door" against 26 of 1347 on BF2 — while BF1 does ship 6 door-NAMED classes, every one `BASE = prop`.
  **DECIDED (loop, 2026-09-17, lane 2): the port's stand-in for a door is a FLAG, queued as DR1, and not folded into this job.** *Why:* §5 makes RE a job of its own, and the real fix (DR2) needs a body that MOVES in `CollisionWorld` — whose broadphase is built once — plus prop animation playback, neither of which exists yet. DR1 takes `base: "door"` bodies out of the SOLDIER subset behind `OPENBF_DOORS_SOLID` (default = open) and **leaves them in ORDNANCE** so a door still stops bolts and AI line of sight: with no door state, bots shooting each other through shut doors is the worse deviation. It is marked [SWEEP] and lands after every other PARKED entry, because unpinning 13 units moves the sweep footer and very likely the three replay hashes. *How to reverse:* `OPENBF_DOORS_SOLID=1`, or drop DR1.
  **The verifier agent (§5's falsification half; one Opus pass, citations only, never the reasoning) SUPPORTED every load-bearing claim** — the label bytes, the six property hashes and their slots, the eight fnv1a preimages, the tick and its only thunk, the trigger loop, the rig, the prim's parent, the clip CRC and the decoded travel, the Lua lines and counts, and all three BF1 searches, which it re-ran itself (finding and decompressing a BF1 LVL that is a ZIP rather than a ucfb, and searching that too). It **falsified two wordings and tightened four**, every one folded in before this commit: "BF1 has no door at all" → no door ENTITY CLASS (BF1 ships 6 door-named `prop`s); "the Lua only locks a door" → POL1's conquest DOES close a doorway, by `PlayAnimation` on a `prop`, so BF2 has TWO doorway mechanisms and this RE covers the engine-class one; plus the `EntityDoor` name proven by the module's static init instead of vtable adjacency, the two sounds shown to be EDGE-triggered by a latch bit, a second gated query path in the trigger loop, and six exactness fixes. It strengthened two: `FUN_001ec5d0`'s address occurs exactly ONCE in the ELF (slot +0x20 of the class vtable 0x61afc8) and `FUN_001ebb80` has no pointer anywhere — the thunk at 0x61b004 is its only entry — so "the door class's handler" and "the door's tick" are proven, not inferred.
  **What stays [H]:** the name behind PROP key 0xa1c9cbc5 (`"metal_foley"`); which vtable slot the door toggles on the room portal it registers with (`FUN_005beba8`, the manager the `wrld` loader hands every `port` chunk to); and whether the PS2's own compiled MISSION.LVL Lua matches the PC modtools script source used for the Lua half (that half is tagged [D], not [V]).
  **What the RE enables, queued:** **DR1** (the flag, [SWEEP]), **DR2** (the door entity: trigger sphere, `open` clip, moving box, `IsLocked`) and **DR3** (`plnp`'s blocked-group route patches and the `Block/UnblockPlanningGraphArcs` pair the port ignores — low value while SP conquest asks for the all-open table the port already uses). Noted on the way, not a claim: bf2_tan1 has 16 doors and every one stands on an arc, and it is one of SK's three no-outcome maps where a team fired ZERO shots all round.

- **2026-09-17 (evening) — RE (loop, lane 1, job C, queue WA's [RE] half): the PS2's water DOES composite by its TEXTURE alpha — and the near speckle deliberately does not.** No branch, no code change, no gate run and none could move: this job reads the discs and writes `analysis/` only. Lifted into `analysis/re-water-alpha.md` under the `# OPUS RE … [O] UNAUDITED: no Fable pass` header with every load-bearing claim tagged `[O]`; RE_LOG pointer added; `re-water.md` §3 and §8 amended; HANDOFF HOLD 21(a) narrowed and §What next item 2's water line closed; scratch, census binaries and raw measurements under `engine/artifacts/lead_tmp/re/water-alpha/`; report `engine/artifacts/loop/reports/20260917_2045_re_WA.md`.
  **The answer.** Every BLENDED water layer takes `As = At × Av >> 7` with `At` the sheet's own alpha: the TEX0 the water binds carries **TCC = 1 (RGBA) and TFX = 0 (MODULATE)**, built for every non-32-bpp texture by the engine's single texture path (BF1 `FUN_0049c5c8` 562034, reached from the flat-water renderer `FUN_0048b9c0` 551303/551306 → `FUN_0049ee68` modes 10/8 → `FUN_0049de40` → the builder; BF2 `FUN_00582c90` 717181 by the identical chain from `FUN_00575fd8` 708713/708716), and the near main (0x69c390 / 0x76c170), far main (0x69c3a0 / 0x76c180) and ocean foam (0x69c3e0 / 0x76c1c0) records all carry ALPHA 0x44. The exceptions are exact and both are self-consistent with the disc's own data: the ocean BASE is not blended at all (0x4100, prim 0x18 without ABE) and its sheet `kam1_water_2` is the ONLY water sheet on either disc authored alpha-255 on every texel; and the near cross-faded SPECKLE has its TCC bit **explicitly cleared** — *(decompiler detail omitted)*, whose only zero bit is 34 — at BF1 551846 / BF2 709258, behind a gate (`DAT_00522a64` / `DAT_0060c1e4`) that reads 1 in both ELFs and is never written, which is exactly why BF1's 25 `water_specularmask_NN` frames are authored alpha-255.
  **Measured on the disc this session, with the port's own `mungefs`** (`lead_tmp/re/water-alpha/disc_water_alpha.txt`): all 14 water sheets are 4- or 8-bpp paletted, so every one takes the TCC=1 branch, and **11 of the 12 `MainTexture`s carry real alpha** — `kas2_water` 96 flat, `yav_water` 96 flat, `nab2_water` 89..190, `fel1_water` 80..171, `dag1_water` 138..203, `dea1_water` 137..240, `kas1_water` 142..255, BF2 `kas2_water` 158 flat, the ocean foam `kam1_water` 0..218 (mean 62, 11.1 % at zero). So **`fs_water`'s `base.a` multiply is the disc's arithmetic** and is inert only because `bake::alpha` forces those sheets opaque; the bake is the defect, and giving them their alpha back is not a look judgement.
  **The port question the item carried is answered too, and it is not a water path: BF1 KAM1's water is under the map's own terrain.** BF1 KAM1 ships a real heightfield (256×256 at 40 m) that sits at y = 0.00 over **every one of its 1,618 wet cell centres — 8.00 m above the −8 water plane** — where BF2 KAM1's `tern` is a STUB with no heightfield at all (`kam1_terrain_vs_water.txt`). Same sheet, same plane, same cells: that is why bf1_kam1's golden and all 8 of its walkabout panels read 0.000 when the sheet flipped while bf2_kam1's golden moved 5 %. The committed goldens show it — BF1 KAM1's "sea" is the flat grey terrain plate, BF2 KAM1's is real water between the platforms.
  **DECIDED (loop, 2026-09-17, lane 1): WA's build half is scoped NARROW — only the sheets a map's own `fx__ Effect("Water")` names get their disc alpha back, not every unnamed map texture.** *Why:* the [V] evidence reaches the WATER draw and stops there; the wider "no segment names it" clause queue SM measured would free bird/fish/mist/foliage/terrain DTEX on the same warrant, and those are model-drawn sheets whose verdicts belong to ALPHA RULE v2, not to this reading. *How to reverse:* the predicate is one clause in `bake::alpha` — widening it later is the same clause with `unnamed` restored. Consequence recorded in the entry: SM's measured golden deltas are an UPPER bound and the build half re-measures its own.
  **The verifier agent (§5's falsification half; one Opus pass, citations only, never the reasoning) falsified exactly one claim and it was the speckle's** — the lead had read `FUN_0048c848`'s TCC=1 and stopped at that line; the agent read to the function's end and found the clear 58 lines later. It was **reproduced by the lead before acceptance** (the decomp line, the mask's single zero bit, the gate's ELF value and its two read-only occurrences, and the machine code at va 0x48ce5c-0x48ce90 that builds the mask and applies it with `ld/and/sd` to the first qword). Four wordings tightened with it: the TEX0 guard is "not 32-bpp" rather than "4/8-bpp"; 562033/562038 assign the CLD bit with the OR at 562040; `FUN_0049c5c8` has a second TEX0 path that bakes TCC=1 into its TH constants; and both games carry an alternate near record (0x69c380 / 0x76c160) their selectors never reach. Everything else was SUPPORTED with the enclosing function named.
  **What stays open:** `Av` itself — the Min.a→Max.a value the VU1 phase nibble animates — is still HOLD 21's, untouched by this job; and the RE turned up one erratum candidate in `re-material-flags.md` §2, whose blanket "TEX0 TCC=1 for every texture in both games" is looser than the code at the 32-bpp site (queued as **TCC32**; no water sheet is 32-bpp, so nothing today turns on it).

- **2026-09-17 (evening) — LANDED (loop, lane 1, job B, queue OCB): Kamino's sea is metres, not centimetres — the PS2's inverse FFT is UNNORMALISED.** Branch `ocean-metre-scale` (built 2026-09-17 17:55 by lane 2 on main `936b08c`), rebased onto main `8acedd9` and fast-forwarded as **`20ae155`** + **`bc1d9d1`** + the landing's own comment fix **`efab925`**; landing report `engine/artifacts/loop/reports/20260917_2010_land_OCB.md`, build report `20260917_1805_ocean-metre-scale.md`. `Ocean::inverse` drops its `1/n²` and `Ocean::bound` drops it with it — `analysis/re-water.md` §3a [V] [O]: `FUN_004cea50` divides by `N·M` only when its fourth argument is non-zero and the water's call site passes zero (`00477b00: daddu a3,zero,zero`; BF2 `00562598`), nothing between the spectrum and the vertex scales `h̃0`, and the VU cannot — it gets a unit cube plus one per-tile world box. Only the `(−1)^(x+z)` sign stays; it is the −n/2 wave-number offset, not a normalisation. **KAM1's sea went from 7.5 cm peak-to-trough to 19.2 m.** +142/−46 over three files (`game/src/water_anim.rs`, `game/tests/water_anim.rs`, `game/examples/water_frames.rs`).
  **The rebase was free:** `git log 936b08c..main` over those three files is EMPTY across the **19 commits** the branch sat parked behind, and the rebased diff is byte-identical to the parked one (`sha256 67d8135d…` on both sides).
  **Gates, every exit code read directly from its own file, on the branch AND again on main:** ci **EXIT=0, 685 passed / 0 failed / 24 ignored** on both — exactly what the GATES block predicted (683 + OCB's 2). gpu **EXIT=0 with 24 referees** and `git status --short engine/goldens` **EMPTY** on both sides. sim_gates rc=0 with walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b` and both motion.csv identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp`-identical RAW to each other, to the branch's own pair AND to `s19/baselines/sweep_main_1.txt`, footer `finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`. No rebake (no bake file in the diff), no golden committed, no walkabout regenerated.
  **The build report's load-bearing numbers reproduced here rather than taken on its word:** a throwaway probe over 2000 frames of KAM1's own sea (removed, and the tree confirmed clean, before the merge) measured rms median **4.3295 m**, peak-to-trough median **19.1866 m**, worst |h| **17.1606 m** and `bound()` **37.4139 m** — and those same four numbers divided by `n² = 256` are 0.016912 / 0.074948 / 0.146148, which IS the sea the normalised transform drew, so the factor is exactly 256 and not approximately so. §3a's independent model of the disc's own pipeline gives 4.145 m and 19.23 m, 4.5 % and 0.2 % away, from different code and a different generator.
  **DECIDED (loop, 2026-09-17, lane 1): the entry's `[GOLDENS]` tag is re-declared UNMOVED — proven, not argued.** *Why:* `scripts/gpu_tests.sh` ends in `scripts/goldens.sh --check`, which runs the golden tests WITHOUT `UPDATE_GOLDENS`, so the committed PNGs are compared against freshly rendered ones and a green check means a regeneration would write identical bytes; it was green with an empty goldens tree on the branch and again on main. Structurally the same: `game/src/gallery.rs` imports `upload_scene` and never `animate_water` or `WaterMotion`, and `game/examples/walkabout.rs` contains no water reference at all, so neither the golden nor the walkabout path ever steps the sea. OCA wrote the `[GOLDENS]` expectation without checking that path; OCB's build caught it and this landing confirmed it twice over. *How to reverse:* `scripts/goldens.sh` on main regenerates at any time.
  **DECIDED (loop, 2026-09-17, lane 1): the goldens were CHECKED, not regenerated.** *Why:* `--check` is the stronger evidence (it proves byte-equality of a fresh render against the committed PNG), and a full regeneration that no entry declares also re-runs the game binary's own `tat1_overview.png` capture path, inviting noise into a landing for no information. Same call, same reason, as the CT and CLN1 landings. *How to reverse:* run `scripts/goldens.sh` without `--check`.
  **DECIDED (loop, 2026-09-17, lane 1): the build's per-frame `water_anim::sink_offset` is KEPT over `Ocean::bound() + 1 cm`.** The still plane is all there is past `WaterRanges::far`, so every metre it sinks is a step at that range. `bound()` is the triangle inequality — every mode at its crest at once — and would sink it **37.4 m**; the frame's own deepest trough sinks it a median **9.3 m** (worst 17.2 m) and is exact rather than bounded. The step is a port-only artefact either way: the PS2 draws no water past its far pass at all. The disc rescans its field's minimum every frame too (§3a, `FUN_00477cd0` 539732–539748). *How to reverse:* `let drop = ocean.bound() + 0.01;` back in `OceanDraw::new` and drop the per-frame re-apply.
  **DECIDED (loop, 2026-09-17, lane 1): one comment-only commit was added to the branch during the landing (`efab925`).** The new test `kam1s_sea_is_metres_not_centimetres` documented itself as "Measured here over 2000 frames" while its loop runs 200; the 2000-frame numbers are real but came from the build's scratch, not from the test. The doc now says which is which and carries the landing's own re-measured figures. *Why:* a comment that states false provenance for a load-bearing number is the thing this project's discipline exists to prevent, and it is two lines inside the entry's own file set. *How to reverse:* revert `efab925`; it touches no production code and ci is 685/0/24 either side of it.
  **The look call, taken by the lander rather than inherited:** `engine/artifacts/lead_tmp/ocean-metre-scale/sheets_bright/lowC_t0.png`, `lowA_t3.0.png` and `farD_t0.png` viewed. At 4–18 m vantages the sea carries a real rolling swell; the pillars' waterlines, all one ruler-straight line before, now sit metres apart; occlusion against the pillars is correct and no platform deck is breached at crest height. The high vantage shows neither a LOD seam nor a far-range step. **Judgement: land it** — this is the look change OCA predicted, and it is the disc's own scale rather than a tuned one.
  **Unblocked by this landing:** `OCF` (the ocean's second, foam pass — its byte rule only fires at this amplitude; at the old one it decayed to zero on every point of every frame, and it now fires on 13 % of them) and `OCL` (the near/far LOD seam, which this landing widened from 8.9 mm to **2.27 m** and which is a consequence of the port's PROVISIONAL decimation, not of the amplitude).

- **2026-09-17 (evening) — LANDED (loop, lane 1, job B, queue L): the `LOWR` far-LOD terrain sheet, bake and render, behind `OPENBF_LOWRES=1`.** Branch `lowr-sheet` (built 2026-09-17 20:10 by lane 2 on main `2d8cc7d`, four commits) rebased onto `8a1a8f2` and fast-forwarded as **`cf7b642`**. Landing report `engine/artifacts/loop/reports/20260917_2035_land_L.md`; build report `20260917_2010_lowr-sheet.md`. `mungefs::terrain` parses `tern → PCHS → LOWR`, `bake::lowres_glb` writes one `lowres_<terrain>.glb` per sheet with `Manifest.lowres_glb` naming it, and `viewer::upload_scene` draws it opaque at identity with the map's fog from the heightfield's own end out to `LowResTerrain.MaxDistance`. 16 of the 32 baked maps ship a sheet; 14 of them draw one.
  **The rebase was free:** `git log 2d8cc7d..main` over the branch's 18 files is EMPTY across the 11 commits it sat parked behind, and the diff against main is byte-identical on both sides of the rebase (`sha256 f8373e163700e3f178b33a725610083c6648566cb041a9ac9afcaecd32044854`).
  **Gates, every exit code read directly from its own file, on the branch AND again on main:** ci **EXIT=0, 690 passed / 0 failed / 24 ignored** on both — exactly what the GATES block predicted (685 + L's 5), block rewritten 685 → 690; gpu_tests **EXIT=0 with 24 referees** and an empty `git status --short engine/goldens` both sides. **[REBAKE] `tat1 maps`**: the branch's scratch bake against the shipped tree gave **SAME=5407 DIFF=31 ONLY_IN_A=16** with the branch's own `check_diffs.sh` re-run at **DECLARED=47 UNDECLARED=0** (all 31 DIFFs a manifest.ron with exactly `+1/-0` and that line `lowres_glb:`, all 16 new files `lowres_*.glb`); after `scripts/rebake.sh tat1 maps` on main, a fresh scratch bake of main against the result gave **SAME=5454 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=16213** — 5407 plus exactly the 47 declared. The behaviour set was run on main although the entry carries no [SWEEP] and touches no sim file (CT's shape): sim_gates **GATES_RC=0** at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both motion.csv identical to base, `game --sim-hash 100` `17e1b946ff0e821c`, the sweep pair `cmp`-identical RAW to each other **and to `s19/baselines/sweep_main_1.txt`** at the GATES footer, and both arena tables (flat 3000/42 and tat2 9000/42) at their eight GATES hashes and kills/min. No gate value moved.
  **"Both flags default OFF" was measured at landing scale, not argued.** After the rebake moved all 31 manifests and added 16 GLBs, a **full walkabout regeneration wrote 256 of 256 PNGs byte-identically** (sha256 manifest before vs after, `diff` empty; 219 images over 32 maps), and **`scripts/goldens.sh` was run as a full REGENERATION rather than a `--check`** — all 90 goldens rewrote byte-identically, including the game binary's own `tat1_overview.png` capture, `git diff --stat engine/goldens` EMPTY. Nothing was staged or committed under `engine/goldens`.
  **The build report's load-bearing numbers reproduced here rather than taken on its word:** `strp_bits.py` re-run over both discs gives bit 14 set in **0 of 415,277** shipped LOWR indices across 21 sheets / 2,893 patches with 0 masked indices overrunning their POSI count — so re-lowr §1's "0x4000 on every second vertex" is the VU word the loader builds, not the disc's bytes, and `unstitch_strips`' `& 0x7fff` is exact; and the look table re-measured on main over six maps in three configurations — `OPENBF_LOWRES=1` **alone** is pixel-identical to the default everywhere (mean 0.000, max 0), the cull alone costs BF1 END1 **4.640 mean / 11.778 % of its pixels** and BF1 RHN2 5.197 %, and the sheet recovers **76.5 %** of that change on BF1 KAS2 (5.049 → 1.186), **74.3 %** on TAT1 (3.137 → 0.805) and **60.7 %** on BF1 HOT1 (1.668 → 0.656), with BF2 DAG1 — which ships a sheet its sky authors no scope for — at 0.000 in every configuration as the control.
  **DECIDED (loop, 2026-09-17, lane 1): `OPENBF_FAR_CULL` stays OFF by default, and `OPENBF_LOWRES` with it.** This is the re-judgement HANDOFF §What next 1 had parked on this item, and it is re-taken on the lander's own measurement and eye rather than inherited. *Why:* the pair is faithful only where a sheet exists — 14 of 32 maps. Where one does, the far ground becomes real terrain and 61–77 % of the cull's change comes back. Where none does, the cull deletes the world and leaves the frame's clear colour behind it. *How to reverse:* `OPENBF_FAR_CULL=1 OPENBF_LOWRES=1` turns the pair on today; the default now waits on queue items 5/6 (the no-sheet maps), not on this item.
  **DECIDED (loop, 2026-09-17, lane 1): the goldens were REGENERATED, not merely checked** — the opposite call to OCB's landing that afternoon, for a reason that did not apply there: this landing's rebake moved 31 manifests inside the tree the goldens render from, so a `--check` would have proved less than a regeneration that writes identical bytes. It wrote nothing. *Reverse:* none needed — no golden is staged or committed by this landing.
  **DECIDED (loop, 2026-09-17, lane 1): the full behaviour set and both arena pairs were run on main** though the entry carries no [SWEEP] and no `core/` or sim file changed. *Why:* cheap, and it makes "no gate value moved" a measurement rather than an argument. *Reverse:* n/a.
  **The build's other four decisions are ADOPTED as landed** (the build session could not write them here, MAIN being held all evening): `lowres_glb` goes at the END of `Manifest` per the manifest crate's own rule; the sheet's near edge is the port's `NearSceneRange[2]`, not the disc's `[1]`, because the PS2's far scene is a separate buffer and the port has one depth buffer; `OPENBF_LOWRES` REQUIRES `OPENBF_FAR_CULL` as a type dependency, since the coarse LOD stands 4–58 m above the heightfield in places and pokes through it without the cull; and a sheet is baked whenever the `tern` ships one, including BF2 DAG1/NAB2 whose sheet the disc never draws — the bake stays a transcription and the manifest already says which are drawn.
  **The look call, taken by the lander:** four sheets viewed (`engine/artifacts/lead_tmp/lowr-sheet/land/sheets_land/`). TAT1's flat tan plain becomes dunes with real ridge lines and a mesa row; BF1 HOT1's featureless white plane becomes a drifted snowfield with shadowed hollows; BF1 KAS2's near-black olive plane becomes a lit shoreline and bay. BF1 END1 — the map with no sheet — is the one that keeps the default OFF: with the cull on, the forest floor and most of its trees past the near scene are replaced by a flat grey-mauve void. **KAS2's hard red band is present in BOTH halves of its sheet**, above the sheet's own horizon, so this item does NOT close it — that stays queue `KASF` (the maintainer's flythrough item 3).
  **One file outside the entry's declared set:** `game/tests/collision.rs` (+2 lines), forced by `Manifest` and `Scene` gaining a field. That is queue `DR1`'s file; DR1 is TODO with no branch, so nothing conflicted, and its entry already carries the flag.

- **2026-09-17 (evening) — RE (loop, lane 2, job C→§5, queue OCL): the PS2 draws its ocean at ONE resolution, and the port's 2.27 m sea-seam is the port's own.** No branch, no code change, no gate value can move; docs only. Result `analysis/re-water.md` §3b (`[O] UNAUDITED`), RE_LOG pointer, report `engine/artifacts/loop/reports/20260917_2033_re_OCL.md`, scratch `engine/artifacts/lead_tmp/re/ocean-lod/`. OCL's step (1) was an RE question, so §5 made it a job of its own; the build half is re-spec'd and left for the next session.
  **The answer, both halves at once.** The ocean DOES take a decimated grid: its per-frame mesh writer ends in `FUN_0048ac90` (BF1 539979, inside `FUN_00477cd0`; BF2 697487, inside `FUN_00562760`), which rotates a triple buffer and builds the mesh **twice** — stride 1 into `DAT_005649c0` and stride `LODDecimation` into `DAT_005649c4` (550796–550797; BF2 708144–708145). But **only the stride-1 mesh is ever issued.** `FUN_0039b488(dl, handle)` appends a DMAtag `0x50000000` — id 5, a DMA **call** — whose ADDR is the handle (387615–387616; BF2 `FUN_00496eb0`, instruction-for-instruction the same), so a mesh whose handle is never passed to it is never transferred. The decimated handle is read by **exactly one instruction in each ELF** — `0048baac` / `005760dc` — both inside the FLAT-water renderer, which the dispatcher (`FUN_0048b1e0` 550906 / `FUN_005755b0`) runs only when `OceanEnable == 0`. So the drawn ocean is **full `PatchDivisions` at every distance**: there is no near/far mesh join on the disc and no seam to reproduce.
  **The absence is byte-exhaustive, not a reading**, which is what §5 self-check 2 demands: a scan of every load/store/addiu word in each whole ELF for the handle's offset returns 3 sites for the far handle (2 writes, 1 read) and 5 for the near one on BOTH discs, matching the decompiler's own occurrence counts exactly; and the ocean renderer's entire 464-instruction body (0x48b280–0x48b9bc) contains no reference to the far handle at all.
  **The decompiler had hidden the decisive argument.** Its ocean loop reads *(decompiler detail omitted)* (551113, 551169) with one argument, while the flat renderer's reads *(decompiler detail omitted)*. The bytes put it back: `0048b5d0 lw a1,0x49c0(s6)` / `beq a1,zero` / `nop` / `jal 0x0039b488` / `daddu a0,s0,zero` — nothing touches `a1` between the load and the call. Ghidra dropped it because the load also feeds the branch, and it drops the same argument in §4's `CPAT` arm (551359). Put side by side, the ocean's tile loop is simply the flat loop with the LOD-selected handle replaced by the stride-1 one and the `CPAT` either/or removed.
  **DECIDED (loop, 2026-09-17, lane 2): OCL's build half is re-spec'd to option (2b) — delete the port's far ocean patch rather than implement a join.** *Why:* there is no join on the disc to implement. *Reverse:* one paragraph in §3b and the entry's build list; `Ocean::patch(every)` and its test stay, so restoring a far patch is a three-line change.
  **DECIDED (lane 2): the disc's second, decimated mesh is deliberately NOT ported.** The PS2 builds it every frame and throws it away; copying work nothing can observe buys no fidelity. *Reverse:* recorded in §3b and in the module header the build will rewrite.
  **DECIDED (lane 2): the RE was done as its own job and the build was NOT started**, per §5, even though the budget allowed both. *Reverse:* the entry is TODO and fully spec'd.
  **DECIDED (lane 2): MAIN was not claimed while lane 1 held it for the L landing**; the lift was prepared verbatim in scratch and committed only after L released it. *Why:* L's landing owed a rebake and a 31-map walkabout and was editing HANDOFF and PLAN itself. *Reverse:* n/a.
  **Two corrections this RE makes to the entry's own numbers**, both reproduced rather than inherited: dropping the decimation **costs** 384 more triangles per far cell (128 → 512 at `PatchDivisions(16,16)`/`LODDecimation(2)`), not the 192 the entry estimated, which had counted quads; and the gate it proposed cannot be a ci test, because `WaterMotion::step` takes a concrete `&mut render::Renderer` — the seam's guard should be the TYPE (a single `MeshHandle` instead of a pair), which makes the crack unrepresentable.
  **Confirmed where the port already had it right:** the ocean's far multiplier is the inline immediate `0x3f4ccccd` = 0.8 against flat water's `DAT_00522a7c` = `0x3f666666` = 0.9 (`viewer.rs:31–32`, cited there from the decomp) — re-read out of the file bytes here, and they hold.
  **Verification, and what it cost:** one Opus verifier agent on the claim list and citations only. It attacked the three absence claims with a stronger search than mine — full effective-address tracking over all 967,919 `.text` instructions, wide `ld/sd/lq/sq` spans, the `$gp` path (`ri_gp_value = 0`, so none exists), literal-pointer scans of the data sections, and a scan for any `lui`+`addiu`/`ori` chain that could *form* the address in a register (0 results) — and **all three survived**. Every technical assertion was SUPPORTED. What it broke was four CITATIONS, three of which I had already caught by re-reading every cited range (550661→550687, 551110→551113, and 708487→**697487**, a transposition that pointed at the flat-side neighbour — exactly the ocean/flat confusion self-check 1 exists to catch), plus one it caught that I had not (551290→551292). All four are corrected in §3b. It also added five refinements now recorded there, the sharpest being that `DAT_005647b8` is a **tri-state** written by two different fx keys, and that no `fx__` key name is a string in either ELF — every key identification rests on the FNV-1a hash and should be read as [D], not [V].
  **HOLD 21(a) is half retired:** "whether the far LOD tiles are displaced" is answered for the OCEAN (there are no far-LOD tiles); only FLAT water still needs the maintainer's GS dump for it.

- 2026-09-17 — **The disc alpha-tests NO soldier: queue SKA's `[RE]` half answered, docs only (lane 1, job D; no branch, no code change, no gate value could move).** Result `analysis/re-side-alpha.md`
  (`[O] UNAUDITED`), RE_LOG pointer appended, report `engine/artifacts/loop/reports/20260917_2145_re_SKA.md`, scratch and measurements `engine/artifacts/lead_tmp/re/side-alpha/`. This closes design
  **D6**'s unanswered half — the question AT's landing deliberately left open when it kept skinned draws on the port's own `SKINNED_MASK_CUTOFF` = 0.5.
  **The answer is not a threshold.** Measured over every `modl` in all 10 BF1 + 18 BF2 SIDE LVLs (2,581 segment rows; the census tool instrument-checked against the two `MTRL` vectors pinned in
  `mungefs/src/model.rs:436`/`:455`), **not one segment of the 52 soldier models or 29 weapon models the port bakes carries the cutout bit 0x02** — BF1 39 + 16 distinct segments, BF2 58 + 20, zero in all
  four sets — and **exactly ONE segment per game carries the transparent bit 0x04**: `all_inf_wookie`'s fur (BF1 seg 2 word `0x5`, 261 verts; BF2 seg 1 word `0x10005`, 288 verts). Both discs BLEND that
  one (GS `ALPHA 0x44`, `TEST 0x51001` = Z-tested, no Z write, `As` = sheet alpha × colour alpha) and draw every other soldier segment with `TEST 0x50000`, i.e. **the sheet's alpha ignored outright**.
  The verdict is per SEGMENT, not per texture: all three wookie segments name the same sheet and two of them are opaque.
  **There is no skinned-specific material path in either ELF** — the soldier's own chain was walked (`soldier` label at VA 0x60fb78 → class ctor `FUN_00250488` → factory `FUN_00255558` → instance ctor
  `FUN_00234a68` → draw thunk `FUN_00255db8` → `FUN_00239bb0` → `RedModel` slot +0xc, the ELF word at 0x64febc = `FUN_0057cf60`), all three of that function's segment branches reach `FUN_0058ed70`,
  and inside it the bone palette is referenced twice and consumed by one DMA call (`FUN_004975c0`, 724020). BF1 mirrors the answer through a different structure (a segment CLASS per flag bit,
  `FUN_004aa9f8`; the mode word `FUN_004abde0` 568939–568954; `TEST` `0x50000`/`0x5100b`/`0x51001` in `FUN_004cf6f0`; palette → `FUN_0039bca0`, 594499). The colour a soldier draw passes is a grey
  brightness byte with alpha = the draw's float × 255, and the main LOD loop passes the literal `1.0f` (`FUN_005ace40`, 743999) — so a normal soldier's colour alpha is `0xff` and only the distance fade
  lowers it, which is the disc's own version of the port's `fs_mask_skinned_fade`.
  **The port's shipped defect, measured and viewed:** `SKINNED_MASK_CUTOFF` = 0.5 is inert on the 31/32 (BF1) and 40/41 (BF2) side base sheets the bake force-opaques, and on the one sheet per game that
  keeps its disc alpha it deletes **93.44 %** of `all_inf_wookie`'s 65,536 texels (BF2, mean alpha 30.7) and **8.46 %** of `wookie`'s (BF1, mean 233.6) — reproducing the AT landing's 4,299 and 835 kept
  texels from the same files. `engine/goldens/units/bf2_all_inf_wookie.png` is a cloud of fur fragments around an intact weapon (viewed). **DECIDED (loop, 2026-09-17, lane 1): the constant was NOT
  touched in this RE job** — *why:* an RE job is not a build (`OPUS_LOOP_PROMPT.md` §5), and a one-line change would have moved two committed goldens outside any entry's declared gates; the fix is queue
  **SKA**'s build half, re-spec'd with this answer (delete the skinned test, give 0x04 segments a BLEND pass, expected rebake diff exactly 2 GLB files and exactly those 2 unit goldens moving).
  *How to reverse:* nothing to reverse — no code changed.
  **Self-checks and their limit (§5):** every citation re-read with its enclosing function printed — which is how this RE corrected `re-material-flags.md` §1, whose 712870/712920/712952 labels
  "rigid/skinned/bone-attached" are wrong in the middle: 712920 passes 0 and 0 for the palette (posed rigid) and 712952 is the softskin branch. Every absence claim is a search with counts (call-site
  counts, data-word scans of both ELF images — `0x0058ed70` appears in 0 data words — and corpus-wide constant counts: `0x5000b` 4× in BF2, `0x5100b` exactly 2× in BF1, `0x5000b` 0× in BF1, and neither
  word anywhere in BF1's Specular class draw). The BF1 twin was opened function by function. Every load-bearing constant was read from the ELF bytes, including the TEST/ALPHA immediates straight out of
  the instruction stream (`0058efa4 lui 0x0005`, `0058f028/38 ori 0x1001`, `0058f03c ori 0x000b`; `004cfb4c/68/90/94`, `004cfd3c addiu 0x0600`).
  **The verifier agent returned after the first commit and is folded in as `5c1aeab`: 24 of 25 claims SUPPORTED, and the one it broke was real.** BF1's model-draw vtable base is `0x00558fe8` — the
  address two constructors store at object+0xc0 (`FUN_00496068:557008`, `FUN_004960e8:557038`) — so that draw is slot **+0xc**, identical to BF2's, not the +0x14 this RE first read off the zero words
  preceding the table; both lines were re-read by the lead before the correction was accepted, and the doc's §4 and §7 now carry it. Every ABSENCE claim survived an instruction-level attack stronger than
  the textual counts they rested on (3 `jal 0x0058ed70` and **zero** occurrences of that word anywhere in the 5.28 MB BF2 image; the palette's two stack slots touched by 5 and 4 instructions in total;
  `0x5000b` built in 3 BF2 code sites and 0 in BF1; `0x5100b` in exactly 2 BF1 sites; neither word formed anywhere in BF1's Specular draw, which sets `0x50600` with ATE clear). It also fixed three line
  numbers, sharpened BF1's translucent-word guard (*(decompiler detail omitted)* forces it, `& 0x20000000` suppresses it — neither is set on a soldier draw), found four more dropped-argument sites of the
  `re-water.md` §3b kind, and noted that BF2's posed branch masks `~0x01000000` where BF1's masks `~0x00800000`. **DECIDED (loop, 2026-09-17, lane 1): the verdicts were folded in under `STOP` rather than
  left for a session that will not run** — *why:* the docs had been committed saying no agent verdict existed, which stopped being true the moment it arrived, and one of the verdicts was a correction to a
  [V] claim. *How to reverse:* `git revert` the amend commit; the agent's report is quoted in the session report.

- **2026-09-23 — LANDED (loop, lane 1, job B, queue WA): each water map's own sheet composites by its disc alpha — 12 sheets rebaked, 8 goldens, and BF1 Kamino's "sea" turns out to be its terrain.** Branch `water-sheet-alpha` (built 2026-09-17 21:08 by lane 1 on `8822b86`, one commit `5a0ecb0`, parked six days behind OCL and the SKA RE) rebased clean onto `173c432` as **`19975d0`**, plus a comment-only fix **`2d5e4a4`** made on the branch before the merge (its report-line comment said "2 on the KAM1s"; the bake prints one kept sheet per wet map, 12 lines), fast-forwarded; goldens **`7409964`**. Landing report `engine/artifacts/loop/reports/20260923_1905_land_WA.md`; build report `20260917_2108_build_WA.md`. `bake::alpha::keep_alpha` gains `water_named` beside SM's `unnamed`: the lower-cased `MainTexture` + `FoamTexture` of THIS map's own `fx__ Effect("Water")`, asked only of a MAP bake — the NARROW rule the WA RE decided (`analysis/re-water-alpha.md` §0, `[O]`).
  **Gates, every exit code read from its own file, on the branch AND on main:** ci **EXIT=0, 692 / 0 / 24** on both (690 + WA's 2; GATES block 690 → 692); gpu_tests **EXIT=0, 24 referees**, `git status --short engine/goldens` empty on the branch against the SHARED bake (the code alone moves no picture) and again on main after the golden commit. **[REBAKE] `maps`:** the branch's scratch bake against the shipped tree **SAME=5310 DIFF=12 ONLY_IN_A=0** — exactly the 12 declared `<name>_water.ktx2` (no manifest, no tier-2, no common/HUD path, no other map sheet); after `scripts/rebake.sh maps` on main, a fresh scratch bake of main against the result **SAME=5322 DIFF=0 ONLY_IN_A=0**. **Behaviour set on main** (not a [SWEEP] entry; the sweep is the one gate that reads the rebaked maps): sim_gates **GATES_RC=0** at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both motion.csv identical, `--sim-hash 100` **`17e1b946ff0e821c`**, the sweep pair `cmp` 0 to each other and to `s19/baselines/sweep_main_1.txt`, footer unchanged. The arenas were not re-run: `flat` is synthetic and `tat2` is not in the `maps` group (the build ran both against the changed bake, eight hashes unmoved). No gate value moved.
  **Goldens — 8 moved, quantified before viewing, all four pair sheets viewed** (`lead_tmp/water-sheet-alpha/land/pairs_0{0..3}.png`), rgb mean / max / changed %: bf1_kas2 **2.856** / 38 / 31.358, bf2_dag1 **2.271** / 62 / 16.466, bf2_kam1 **0.875** / 30 / 7.270, bf2_kas2 **0.819** / 116 / 2.773, bf1_kas1 **0.546** / 5 / 16.479, bf1_nab2 0.009, bf1_yav1 0.008, and bf1_kam1 **0.000 RGB with its stored alpha moving on 7.632 %**. **All 8 are byte-identical to the PNGs the build parked before the rebase**, so OCL's change moved neither KAM1 golden (the build's "re-measure after the rebase" is done; the gallery never animates water). **Walkabout regenerated IN FULL** (a partial run APPENDS duplicate INDEX sections): **46 of 219** images moved, all on 11 of the 12 water maps, the 20 dry maps byte-identical, and the build's top five reproduced exactly (`bf1_yav1/cp5_bazaar` **17.592**, `bf2_yav1/cp4_bazaar` **15.833**, `bf2_fel1/cp1_cp6_1` 5.118, `bf2_dag1/cp0_cp2` 4.976, `bf1_kas2/vantage` 4.894); INDEX.md moved only in the two KAM1 sections, +1 draw per panel = the foam draw now issued.
  **The build's open question ANSWERED — bf1_kam1's stored alpha is its TERRAIN, not the foam.** The water pipeline blends alpha OVER (`wgpu-types` 27.0.1, `ALPHA_BLENDING.alpha = BlendComponent::OVER`), which cannot lower a destination alpha. The baked `terrain_kamino1.glb` names one material, `kam1_water` (the `tern`'s LTEX layer 0), over y −20.16..0.00, and draws through the opaque pipeline (blend None, `render/src/lib.rs:1700`), where `fs_main` → `shade` hands the texel's alpha to the target while colour ignores it: the new alpha is 0..197, median 60 over 79,130 pixels, tracking the sheet's own 0..218 / mean 62.2. The ocean (`water.glb`, `kam1_water_2`, y = −8) and its new foam draw both sit under that heightfield. Nothing reads destination alpha (0 `DstAlpha` factors under `engine/crates/`) and the swapchain takes `caps.alpha_modes[0]` (Opaque where supported), so it is invisible in play. Two build claims corrected: "the only code that reads it is the foam gate" (the terrain reads it too) and "255 → 0..7" (0..197). So BF1 Kamino shows no foam at all; queue OCF's capture gate must be bf2_kam1. `[H]`, unread: whether the PS2's terrain pass takes LTEX layer 0's texture alpha (`re-water-alpha.md` §5 addendum).
  **One RE count corrected:** "11 of the 12 `MainTexture`s carry real alpha" is **10 of 12** — the other two are the KAM1s' ocean base `kam1_water_2`, which the census table had grouped into one row (12 of all 14 sheets carry alpha) — fixed in `re-water-alpha.md` §4, RE_LOG and HANDOFF.
  **DECIDED (loop, 2026-09-23, lane 1): the look change is accepted on the sheets** — bays, ponds and swamps stop being opaque slabs and show their beds, Yavin's flooded plaza becomes a film over its paving, and BF2 Kamino's sea gains its disc foam layer. *Why:* every move is the water composited by exactly the alpha its own sheet carries, which is the disc's arithmetic (`re-water-alpha.md` §0, `[O]`). *How to reverse:* the `water_named` clause in `bake::map::bake_content`, `rebake.sh maps`, and revert `7409964`.
  **DECIDED (loop, 2026-09-23, lane 1): bf1_kam1's alpha-only golden was committed with the other seven.** *Why:* it is the declared change's own consequence, the referee compares RGB only, and leaving it out would show as a phantom move on every later regeneration. *How to reverse:* revert that one PNG.
  **DECIDED (loop, 2026-09-23, lane 1): the whole walkabout was regenerated rather than the 12 water maps one by one.** *Why:* `examples/walkabout.rs` APPENDS to INDEX.md, so a partial run would have left 12 duplicate sections; the full run also proves that the 20 dry maps did not move. *How to reverse:* n/a (the old INDEX is kept as `lead_tmp/water-sheet-alpha/land/walk_INDEX_before.md`).
  **Staging note:** PLAN.md carried the maintainer's own uncommitted 2026-09-23 PCSX2 edit (a §Environment rewrite, a KICKOFF Q1 rewrite and its own Log entry, marked "the maintainer's to review and commit"). This landing committed only its own two hunks, by writing the index copy directly (HEAD's text plus this entry and the §WHERE THINGS STAND name), and left the maintainer's edit unstaged in the working tree.

- **2026-09-23 — CLEANSE (loop, lane 2, job B2; the second under `OPUS_LOOP_PROMPT.md` §6b): docs consolidated, code area `game` with CLN2 folded in, a bug hunt over the nine landings since `9edff01`, and `tat2` rebaked.** Code landed as `fac5c88` + `25676a5` (branch `cleanse-game`, 27 files, +237/−253, comment/test-only apart from three error paths that are identical on valid data); report `engine/artifacts/loop/reports/20260923_1945_cleanse_game.md`. **Nothing that runs changed, and the gates say so:** ci **691 / 0 / 24** on the branch and on main (692 − one shadowed test deleted; the GATES block now says 691); `scripts/gpu_tests.sh` exit 0 with the goldens tree clean on the branch and on main; `sim_gates.sh` at the three baseline hashes (`bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`) with both `motion.csv` identical. The sweep and the arenas were not run: no file under `core/` changed except one comment, and `game/`'s changed code (sound-bank error reporting, the camera's triangle filter, a bench message) is not on the sim's path — `--sim-hash 100` is pinned inside ci itself. **`tat2` rebaked:** its manifest had never received L's `lowres_glb` key (`tat2` is its own rebake group; L rebaked `tat1 maps`, WA `maps`) — a scratch `tat2` group from main read SAME=197 **DIFF=1** (that one line) before and SAME=198 DIFF=0 after; the GATES block's bake count is corrected 21,651 → 21,667 (L's 16 sheets). **What shrank:** HANDOFF-OPUS.md 596 → 360 lines / 89 → 35 KB (§For the maintainer one line per call, nothing cleared; §What next and §State rewritten; answered HOLD items cut to a line each); QUEUE.md 124 → 74 KB (twelve DONE entries verbatim to `QUEUE_DONE_ARCHIVE.md`, WA's and CLN2's included; three entries added); this Log's 2026-09-07..09 stretch (11 entries, 86.5 KB) verbatim into `analysis/PLAN_LOG_2026.md` **Part 3**, byte-checked by `lead_tmp/cleanse-game/verify_log_move.py`, three dated summaries in its place; PLAN.md 327 → 246 KB. **Untrue text fixed:** "only the maintainer can supply a PCSX2 capture" in HANDOFF, QUEUE, `scripts/AUTOLOOP.md` and §Working mode (lifted today; what is missing is an in-map capture — HOLD 7); HOLD 6 (the mission Lua was answered 2026-09-01); "the far-cull default waits on items 5/6" (both landed 2026-09-09 — it waits on something no entry holds, flagged for the next refill; and 18 maps DRAW no sheet, 16 SHIP none); stated deviation 4 (the fog's reach), and deviations 13–14 added (the in-world squad marker; Kamino's still plane past the far range); §Phases P9; defect groups 1 and 5; the baselines note (no `loop_baselines/` exists yet); a code citation to a PLAN "HOLD FOR FABLE" line that never existed (now HOLD 10(j)); seven artifact READMEs that described output the tree no longer makes; and 28 `analysis/` statements contradicted by later work, each given a dated inline erratum — among them `re-bf2-doors.md`'s "bolts pass through those doors" (**wrong on both sides**: a name miss writes nothing, BF2 `FUN_00216288` 128707-128710, so bolts stop there, as in the port) and `re-lowr.md`'s "MaxDistance is inherited across maps" (contradicted: `Enable(1)` constructs a fresh sky object, `FUN_002e44d8` → `FUN_002e2810`). **Code (22 findings, all closed):** four tests that could not fail on what they named now can or claim only what they pin; the water-floor test and example merged the common collision from the wrong directory and now do it right, still passing; a shadowed test and two tautological asserts deleted; `--bench`, the sound index and `Bank::load` report load failures instead of hiding them; the camera blockers range-check GLB triangles; ~35 falsified comments fixed (the 0.5 MASK test in `render`/`bake`, CLN2's env block — nine variables, two had been documented — BF1 chatter "silent until GS", a BF2 door "the mission Lua opens"); `RUN_SPEED` and `CORPSE_SECS` labelled PROVISIONAL (BF1's table says a 4 s corpse; values unchanged); PROVISIONAL is now 190 across 46 files. **Queued:** CLN5 (the `LOWR` sheet's flag-gated draw path: END1 invisible but depth-writing, LIT against its unlit rule, a circular alpha-255 measurement, TAT1 capped at the 2000 m far plane), CLN6 (the libm referee's blind spots — path forms, glam, `.powi(`), CLN7 (`golden_tat1` compares two different scenes; a literal-vs-literal water-fog test); notes left on SKA and OCF. **DECIDED (loop, 2026-09-23, lane 2): HANDOFF §For the maintainer is one line per call, grouped into look calls / only-you / calls made, and nothing was cleared** — *why:* the rule was always one line, and 93 lines of paragraphs had buried the look calls — *reverse:* `git show 7498317:HANDOFF-OPUS.md`. **DECIDED (same): the 2026-09-07..09 Log stretch moved to the archive's Part 3** — C2's own pattern, byte-verified — *reverse:* move it back. **DECIDED (same): `tat2` rebaked inside the cleanse** — a stale shared bake is repo hygiene, the move is one manifest line no runtime reads differently (`lowres_glb: None`), and the next landing's full scratch bake would otherwise have tripped on it — *reverse:* none needed. **DECIDED (same): the camera's `Aabb` twin of `core::collision::Aabb` and the 5× scene-load sequence are DECLINED** — no defect, and the camera is golden-visible; the two copies that had diverged were fixed instead — *reverse:* a queue entry. Next cleanse: area `render`, due after 8 landings or 24 h (`engine/artifacts/loop/LAST_CLEANSE`).

- 2026-09-23 (evening) — **DECIDED (the maintainer): heroes use BF2's standard hero health, minus the time drain — the earlier flat-pool decision is reversed.** the maintainer had set heroes in normal conquest games (when enabled) to spawn with ordinary health rather than BF2's special hero health, which in his words "grows and shrinks over time and depending on what you do"; he wants the BF2 hero health back as the default, "for now, when we get to that" (P8). The rule: the hero's health **shrinks only when it takes damage, never with time passing**, and it **still regenerates for killing enemies**. He will add more options and tinker with it later, so this default is deliberately simple. PLAN's §Architecture item 9, the §Heroes health bullet and the P8 phase line are rewritten to match, and "Lightsaber-drain hero health" is removed from the "Dropped outright (never RE these binary regions)" list — the hero health pool and the kill-heal values are now RE targets (§5), while the time drain stays off. *Open, for that RE:* the disc's own hero pool size and kill-heal amounts, and whether BF2's hero health has a time component at all (the rule stands either way). This is a LOCKED FEATURE SET change made on the maintainer's word in an interactive session (Sonnet, in-editor), not a loop decision; nothing in the code implements heroes yet, so no gate can move. *How to reverse:* the four edited spots above, in git.

- 2026-09-23 (night) — **DIRECTION (the maintainer, tentative): aiming splits by context — on foot stays BF1 passive magnetism; in vehicles, flyers and space the port will "likely need" BF2's lock-on-target mechanic.** Said as "probably", so it is recorded as the working direction to confirm when P7/P8's RE reaches it, not as a locked call. Edited to match: §Movement & feel and deviation #7 now say "on foot"; §Vehicles & space gains the aiming bullet with its `RE:` list (what the disc's vehicle/space lock-on is, and whether it shares anything with the on-foot X-lock); and "BF2 X-lock aim mechanic" in the "Dropped outright (never RE these binary regions)" list is narrowed to the **on-foot** X-lock, because the old wording would have barred the loop from reverse-engineering the vehicle lock-on. Open, for the RE: whether BF2's vehicle/space lock-on and the on-foot X-lock are one mechanism or two (nothing in `analysis/` says yet — `re-aim-assist.md` covers BF1 soldiers only). Interactive session (Sonnet, in-editor); no code implements vehicles or space yet, so no gate can move. *How to reverse:* the four edited spots above, in git.

- 2026-09-23 (night) — **DECIDED (the maintainer): the cleanse falls due every 12 landings — landings only, no time clause — and is capped at about 250 turns and 2 sub-agents.** Replaces "every 8 landings or 24 h". *Why:* two cleanses in six days cost about (cost omitted) and (cost omitted) against about (cost omitted) for a typical session — roughly a third of the spend since the all-Opus switch (API-equivalent estimates) — and the 24 h clause would make the first session after any gap pay for a full cleanse after a landing or two. Its value is real (the second cleanse caught a libm guard with fifteen holes, four tests that could not fail, ~35 falsified comments, two wrong RE claims, and shrank HANDOFF and the queue by about 40 %), but its code-size effect was nil (+237/−253 lines). Edited to match: `OPUS_LOOP_PROMPT.md` B2 and §6b (with the cap), `LAST_CLEANSE`, `START_HERE.md`, the QUEUE rules line, HANDOFF's cadence bullet and PLAN's Working-mode line. Interactive session (Sonnet, in-editor). *How to reverse:* set the number back and restore the clause in those places.

- **2026-09-23 (night) — LANDED (loop, lane 1, job B, queue OCF): KAM1's foam is the disc's per-vertex byte — it gathers on the crests instead of covering the sea — and the port's own far filler no longer wears the whole sheet.** Branch `ocean-foam` (built 2026-09-23 19:30 by lane 1 on `7498317`, parked green when the maintainer's STOP arrived) plus ONE landing commit for the build's open look call, rebased onto `41d00ca` and fast-forwarded as **`42f85e2`** (the build) + **`0b203e7`** (the far ring). Landing report `engine/artifacts/loop/reports/20260923_2215_land_OCF.md`; build report `20260923_1930_build_OCF.md`; evidence `engine/artifacts/lead_tmp/ocean-foam/` (the landing's under `land/`).
  **What:** `water_anim::Foam` keeps the disc's byte per point of the `(n+1)²` patch and steps it by `FUN_00477cd0`'s rule (539941–539985, `re-water.md` §3a [V][O]) — +6 where `(h − h₋₁) − (h₊₁ − h)` exceeds 3.0 along either periodic axis, else −4, saturating — and the foam draw gets its OWN copy of the patch whose vertex alpha is `round(((b+1)>>1)·255/128)` (`FUN_0048a6c8` 550727/550747). A copy because the base and foam templates shared one patch mesh and the shader multiplies vertex alpha into both: a shared alpha would have made the opaque base translucent. The map is zeroed in full (the disc's `memset(map, 0, 1)` is a shipped bug; deviation in the module header and §WHERE THINGS STAND deviation 15). The build read every constant as BF1 ELF bytes (`004782a0/a4` = 3.0, `c.lt.s` strict per axis, `sltiu 250` / `addiu +6` / store 255, `sltiu 4` / `addiu −4` / store 0; the packing `lbu` / `addiu 1` / `srl 1` / `sll 24` at `0048a9d4..0048aa30`), which re-confirms §3a's `[O]` claims; no new RE claim beyond the [D] rate below. The stale foam comment in `viewer.rs` (false since WA) was rewritten at the cleanse's request (its finding C1), comment-only.
  **The far ring (the build's one open look call, taken here):** past `WaterRanges::far` (640 m) the port's own sunk still plane (deviation 14) kept its FOAM draw at the baked white vertex alpha — the whole sheet — while the stepped sea in front of it foams on its crests only, so a light ring showed at 640 m. `0b203e7` gives that still foam draw instance alpha 0 once the ocean steps (the plane is flat, and on a flat field the disc's rule decays every byte to 0). bf2_kam1 horizon (`--cam 0,12,-420,0,-4`, `OPENBF_WATER_TIME=10`, 960×540): rows 248–256 mean luminance 17.3 → 4.9 (parked) became 6.5 → 3.5–4.8, against the near sea's 3.1–5.6; 0.693 % of the frame changed, all in those rows; the sky rows identical (`lead_tmp/ocean-foam/land/horizon/ring_fix_sheet.png`, viewed). From the golden vantage the landed stepped frames differ from the parked build's only in rows 166–235 (the sea past 640 m, 4.8–5.1 % of the frame, max 18 levels), and the close-up only in rows 40–64 (`land/far_plane_sheet.png`, viewed).
  **Gates, on the rebased branch and again on main:** ci **699 / 0 / 24**, exit 0, on both (691 + the build's 8 foam tests, all 8 named and passing in both logs; the ring fix adds none — `WaterMotion::step` needs a live `Renderer`, so the capture is its proof); `gpu_tests.sh` **EXIT=0, 24 referees**, `git status --short engine/goldens` empty on both; sim_gates **exit 0** at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both `motion.csv` identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp` 0 to each other and to `s19/baselines/sweep_main_1.txt` on both (4570 B, footer `finished 27/32 … sum 213000`; the two stderr files differ only in the wall-clock `done (N/32, s)` lines). Arenas not run (render-only; the sim reads no water). **Nothing to rebake and no golden or walkabout panel can move**, reproduced: `water_frames`' `still.png` and `nowater.png` from the landed binary are byte-identical to the pre-OCF build's on both KAM1s, and only `main.rs` (bench, capture, play) and the `water_frames` example ever call `animate_water` (grep) — the goldens and the walkabout never step the water. bf1_kam1's stepped frames are byte-identical to the parked build's (its sea sits under its terrain, far ring included). Cost, re-measured on the landed binary (`water_frames bf2_kam1 240` ×2): the step alone **0.0157–0.0163 ms** mean, against the build's 0.0164–0.0170 after and 0.0116–0.0121 before — about +5 µs, inside the accepted +0.030 ms.
  **DECIDED (loop, 2026-09-23, lane 1): the foam steps on a 30 Hz grid of the WATER clock, each step over the heights at its own `k/30` s, not once per rendered frame.** *Why:* the disc steps once per frame (§2, the water tick at B1 135276) and renders one frame per two NTSC vblanks (`a5-tickrate.md`, the audit `core::TICK_HZ` rests on) — [D] from two [V] facts — so stepping per rendered frame would run the foam 2–5× fast at 60–144 Hz; the grid also makes a capture's foam independent of how it was reached (test `the_foam_is_the_same_at_any_frame_rate`). *How to reverse:* `FOAM_RATE` / `Foam::advance`.
  **DECIDED (loop, 2026-09-23, lane 1): a cold start pre-runs `FOAM_WARMUP` = 192 steps (PROVISIONAL).** *Why:* the first frame and every capture should show the settled foam, not a build-up the PS2's player never saw (its first frames pass under the load screen); 192 is the fewest of 96/128/160/192 whose foam matched a 2000-step run byte for byte at all 1,200 times sampled (128 missed 69 of them, by ≤ 2 of 289 bytes). *How to reverse:* set it to 0 for a zero start.
  **DECIDED (loop, 2026-09-23, lane 1): the foam layer is a second patch buffer inside `water_anim`, not a render draw-layer variant.** *Why:* `render/src/lib.rs` and `shader.wgsl` are shared with queue 9, W3, 7, CORB, SKA and SPL6, and a per-frame copy of the one patch keeps OCL's one-resolution guard; it relies on the water pass ordering runs by mesh handle (`render::draw_key`), which the code says where it uploads. *How to reverse:* delete `OceanDraw::foam_mesh`.
  **DECIDED (loop, 2026-09-23, lane 1): the look change is accepted on the capture sheets** — the foam leaves the flat sea and gathers on the crests (`lead_tmp/ocean-foam/close/sheet_close_x4.png`, ×4 gain). The straight foam-free bands 80 m apart are the model's, not a seam: one patch per cell (§3) and `P(k) = P(−k)` give standing-wave node lines, the time-mean foam repeats every 8 points, and the dips MOVE with the seed (`lead_tmp/ocean-foam/probe/`). *How to reverse:* revert `42f85e2` and `0b203e7`.
  **DECIDED (loop, 2026-09-23, lane 1): the port's still plane past the far range carries NO foam once the ocean steps (`0b203e7`), rather than accepting the ring.** *Why:* the plane is the port's own filler (the PS2 draws no water there), so its only job is not to show; the disc's rule on a flat field gives exactly 0 foam, and the measured ring is gone. *How to reverse:* delete the `if foam { d.color[3] = 0.0 }` in `WaterMotion::step`.
  **Left for a refill (HANDOFF §What next 2):** the foam layer's `FoamTile(5,5)` (parsed by `mungefs::fx`, absent from `manifest::ManifestWaterConfig`, so the copy keeps the base's Tile 4 — [RE] for the foam texture matrix, then [REBAKE]) and the ocean's vertex RGB (`DAT_0069c420..422` halved, writer unread; the port tints the ocean with the flat-water LUT midpoint that §3a (c) says the ocean path never uses).

- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue SKA's build half): the disc tests no soldier's alpha — the skinned path loses its alpha test, and the one 0x04 segment per game, `all_inf_wookie`'s fur, blends.** Branch `skinned-alpha` off `77792be`, two commits, the rebase a no-op, fast-forwarded as **`ff71352`** (the build) + **`62e824a`** (the crate map names where a soldier's alpha is decided; `scripts/gpu_tests.sh` stops counting its tests); `side` rebaked on main; goldens **`8277709`**. Report `engine/artifacts/loop/reports/20260924_0245_build_land_SKA.md`; evidence, tools and every gate log in `engine/artifacts/lead_tmp/skinned-alpha/` (the landing's under `land/`). Built on the 2026-09-17 RE (`analysis/re-side-alpha.md`, `[O]`): the PS2 draws a side model's segment with the GS state its own `MTRL` word asks for, no soldier segment on either disc carries the cutout bit 0x02, and exactly one per game carries 0x04, which both discs BLEND (`ALPHA 0x44`, `TEST 0x51001` — Z-tested, no Z write).
  **What:** `bake::soldier::segment_blends` reads a skinned segment's `MTRL` word — 0x04 → `blend`, neither bit → opaque — and REFUSES what the skinned path cannot draw (0x02, and 0x04 with 0x08 / 0x40 / 0x80); `bake::skin_glb` writes `"alphaMode": "BLEND"` on that segment's material and nothing on the rest. `game::assets::CpuSkinnedMesh::submeshes` carries the pass, and `upload_skinned`, `SoldierVisuals`, the gallery and `pose_preview` hand it on. `render::SkinnedDrawCmd` gains `layer` (Opaque or Blend; any other layer is refused); `SKINNED_MASK_CUTOFF`, `skinned_mask_test`, `fs_mask_skinned` and its fade twin are deleted — the skinned pipelines take the world's `fs_main` / `fs_fade` / `fs_blend_fade`, and a new skinned blend pipeline and its fade twin draw the blended segments after every opaque skinned draw, far to near, through `sort_far_to_near`, which the world's blend pass now shares. +548/−206 over 17 files.
  **Gates, reproduced by this session:** ci **702/0/27** on the branch (twice) and on main (699 − the shader-grep test the 2026-09-23 cleanse asked the build to delete + four new: `sort_far_to_near`, `segment_blends`, the wookie's fur on both discs, the per-segment GLB round trip; three new GPU referees); `scripts/gpu_tests.sh` on main exit 0 (27 passed: 20 render referees, three of them new, + 7 golden tests) with the goldens tree clean. **Bake:** a scratch `side` bake from the unchanged binary read SAME=283 DIFF=0 against the shipped tree (the instrument), the branch's read SAME=281 **DIFF=2** — exactly `side_bf1/soldiers/all_inf_wookie.glb` and `side_bf2/soldiers/all_inf_wookie.glb`, whose BIN chunks and JSON outside `materials` are byte-identical — and after `rebake.sh side` on main a scratch `side` group of main read SAME=283 DIFF=0; 21,667 files. A census over the soldier GLBs reproduced the RE's count independently: BF1 21 GLBs / 39 materials, BF2 31 / 58, one `alphaMode` in each game. **Goldens:** `scripts/goldens.sh` on main moved exactly `units/bf1_all_inf_wookie.png` (RGB mean |d| 0.2609, max 112, 3.18 % of pixels, all four panels) and `units/bf2_all_inf_wookie.png` (5.2596, max 137, 11.73 %), quantified before viewing, then viewed as pairs (`land/pairs_00.png`) and full-size RGB crops (`view/`): BF2 goes from a cloud of fur fragments to a whole wookie with a soft fur fringe; BF1 loses its alpha-test holes and hard strand edges. Both PNGs byte-identical to the branch's candidates, and every other golden byte-identical. **Behaviour, unmoved on the branch and on main:** `sim_gates.sh` `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both `motion.csv` identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp`-identical and equal to the s19 baseline (md5 `be656ec8…`, footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`); the arenas twice each, pairs and s19 baselines `cmp`-identical (flat 45.49 / 122.33 / 166.89 / 200.00, tat2 25.40 / 38.60 / 39.80 / 37.20).
  **RE correction, [V] by `segdump`:** BF1's wookie segments name `wookieface02` / `wookie` / `wookie` (words `0x1` / `0x1` / `0x5`), so the RE's "all three segments name the same sheet" holds for BF2 only; an opaque and a blended segment share a sheet in both games, so the per-segment verdict stands (`re-side-alpha.md` §6 item 3 erratum, RE_LOG pointer marked ported).
  **DECIDED (loop, 2026-09-24, lane 1): a skinned segment whose word asks for a state the skinned path cannot draw fails its bake** — 0x02 (the disc's alpha test) or 0x04 with 0x08 / 0x40 / 0x80 (another blend) — *why:* no baked soldier has one (the rebake of all 52 proved it), but P7's vehicles and the hero units carry both, and silently drawing them opaque or with the wrong blend is the defect SKA exists to remove — *how to reverse:* make `segment_blends` return `false` for them.
  **DECIDED (same): the golden's alpha plane keeps the fur sheet's alpha under the opaque segments,** as every opaque world draw already writes its own sheet's alpha — *why:* forcing alpha to 1 on the opaque passes would move every golden's alpha plane for no visible change; a compositing image viewer shows the BF2 body pale, so HANDOFF points the maintainer at the RGB crops — *how to reverse:* an RGB-only write mask on the opaque skinned pipelines, and the goldens it moves.
  **DECIDED (same): the gate's "a render test that the skinned shader declares no cutoff constant" is met by behavioural GPU referees** (an alpha-0.25 sheet draws solid on the opaque skinned pass and composites on the blended one; the blend pass runs after the opaque one and is Z-tested; any other layer is refused) — *why:* the cleanse flagged the old grep test as unable to fail if skinned draws were routed back to a test — *how to reverse:* add a grep test.
  **DECIDED (same): no walkabout regeneration** — *why:* the change touches no map, and the 32 map goldens, which draw the same reference soldier through the same skinned pipelines, came back byte-identical — *how to reverse:* regenerate it.
  **Queued:** SKO — every skinned draw is drawn after the world's blend pass, so a soldier behind a translucent world surface paints over it (after SPL6, which moves `render_frame`). **Docs:** QUEUE SKA → DONE, archived verbatim; the GATES block's count 699 → 702 (27 ignored); a note on SPL6 (FREE; `render/src/lib.rs` 2,208 lines); this file's §WHERE THINGS STAND soldier line; HANDOFF §For the maintainer (a look call, a calls-made line), §What next, §State and HOLD 1; `analysis/re-side-alpha.md` §6 and `RE_LOG.md`'s SKA pointer; `LAST_CLEANSE` 8 of 12.
- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue NT): rule 1's `NEVER_TESTS` stays at 128 — it does not follow the world's alpha test, by decision, and its doc now says why.** Branch `never-tests` off `00062f3`, two commits, the rebase a no-op, fast-forwarded as **`3698060`** (the constant's doc and rule 1's call-site comment in `bake/src/alpha.rs` — comment-only) + **`21a06bc`** (`engine/CRATES.md`'s two counts it moved); report `engine/artifacts/loop/reports/20260924_0345_build_land_NT.md`; the census scripts, the experiment's bake and every gate log in `engine/artifacts/lead_tmp/never-tests/`.
  **The question** (queued by AT's landing, 2026-09-17): `bake::alpha::NEVER_TESTS` (128) was the old 0.5 test and is now decoupled from the world's MASK test (disc alpha 193 since AT) — should it follow? AT predicted a raise would move nothing, because no MASK-verdict texture had its alpha max in 129..192.
  **Measured — the prediction was wrong in kind.** Rule 1 runs BEFORE rules 2 and 3, so it re-routes every draw in the band whatever its verdict, not only MASK draws. A census of the shipped bake's GLB `extras` (`band_census.py`): 35 materials in 129..192 — 11 already BLEND, 1 MASK (`mus1_sky_dome`'s 0x10001 word, a sky shell whose material `upload_sky` never reads; AT's count had missed it), 23 OPAQUE. An uncommitted experiment (`NEVER_TESTS = ALPHA_TEST − 1`) and a full scratch bake: **SAME=21633 DIFF=34** — 23 GLBs, 7 KTX2 (four sheets keeping their disc alpha, RGB byte-identical) and 4 manifests (one `transparent` name each), exactly the census's families. By `analysis/re-material-flags.md` §2's reading of each word, the disc blends 8 of the 23 (`kam_bldg_window_01`'s 0x04 words, drawn OPAQUE today by rule 3's 0x04 clause — §6 point 1's known disagreement) and draws 15 opaque: DEA1's `dea1_bldg_conference_room_wall01` ×6 (alpha ≤ 31 on 88 % of its texels), the probe droid ×6 on both discs, UTA1's `uta1_ground_fog` (a flat alpha 165), one Kamino window word and MUS1's dome 0x10010 word.
  **DECIDED (loop, 2026-09-24, lane 1): `NEVER_TESTS` stays 128, decoupled from the renderer's test on purpose.** *Why:* following the test moves 15 draws away from the disc to move 8 toward it, and those 8 belong to rule 3's 0x04 clause, not to rule 1; the disc has no rule 1 at all (§6 point 4); and a MASK draw in the band draws nothing on BF2's disc as well (0x02 → AFAIL KEEP). *How to reverse:* `NEVER_TESTS = crate::detail::ALPHA_TEST - 1`, then `scripts/rebake.sh maps common` — the 34 files above are all in those two groups.
  **Gates, reproduced by this session:** ci **702/0/27** exit 0 before each commit and on main; a FULL scratch bake of the branch **SAME=21667 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0** — the first full-tree proof since 2026-09-17 that main's code reproduces the shipped bake (41 s); `scripts/gpu_tests.sh` exit 0 (27) with the goldens clean on the branch and on main; `scripts/goldens.sh` on main rewrote all 90 goldens byte-identical; on the branch `scripts/sim_gates.sh` `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both `motion.csv` identical, `--sim-hash 100` `17e1b946ff0e821c`, and the sweep pair `cmp`-identical to each other and to the s19 baseline (footer `finished 27/32 … sum 213000`) — `game` does not link `bake` (a dev-dependency only), so none of these could move; `size_report.sh --check-map` 0 stale. No rebake, golden, walkabout or re-baseline.
  **Found and queued — R1U, placed next:** rule 1 already misfires at 128. Of the 55 materials it catches, 37 carry a word with neither 0x02 nor 0x04, which the disc draws OPAQUE (§2; §6 point 4) — BF2 `Cor1_Bldg_security_roomDoor` ×24 on nine COR1 room models (alpha max 53), BF2 `deal1_bldg_reactor_room02` ×7 on four DEA1 models (78), BF1 `kam_bldg_antenna` ×4 on BES1's anti-aircraft chunks (116) and GEO1's `geo_sky` ×2 (a sky shell). The decoded sheets show a GLOW mask, not a cutout (`lead_tmp/never-tests/tex/rule1_two_sheets.png`), so the port draws those doors and walls at most 21 % / 31 % / 45 % opaque. Across all 6,155 materials the rule decides, 602 differ from the disc's reading (`v2_vs_disc_all.txt`); the rest are §6's capture-dependent disagreements (the crate, class D).
  **Docs:** QUEUE NT → DONE, archived verbatim; R1U added; HANDOFF §What next, §State and §For the maintainer (a calls-made line); a dated line in `analysis/re-material-flags.md` §6 (point 4 counted); `LAST_CLEANSE` 10 of 12.
- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue R1U): rule 1 composites only a word that asks for its alpha — COR1's security-room doors and DEA1's reactor walls draw solid.** Branch `rule1-unflagged` off `afa3dbc`, two commits, the rebase a no-op, fast-forwarded as **`0c3b5c3`** (the rule, its docs and tests in `bake/src/alpha.rs`) + **`e7577e1`** (`engine/CRATES.md`'s two counts); `maps` rebaked on main; goldens **`47cd6c8`**. Report `engine/artifacts/loop/reports/20260924_0430_build_land_R1U.md`; the prediction, tools, sheets and every gate log in `engine/artifacts/lead_tmp/rule1-unflagged/` (the landing's under `land/`). Built on `analysis/re-material-flags.md` §2 [V] (a word with neither 0x02 nor 0x04 draws OPAQUE on both discs, its texture alpha only written to the frame buffer) and §6 points 3 and 4.
  **What:** in `bake::alpha::verdict`, rule 1 (`amax <= NEVER_TESTS` → BLEND) fires only when `flags & ALPHA_IS_MEANINGFUL != 0`; rule 3's class-D clause (unflagged, a0 ≥ 0.05, mid ≤ 0.10 → MASK) also needs `amax >= detail::ALPHA_TEST` (193) — below it that MASK drew nothing. The words rule 1 used to catch now fall to rules 2 and 3 like any other unflagged word, and the GLB's `extras.rule` names the clause that decides (`"2"` or `"3"`). `NEVER_TESTS` stays 128. Tests: the rule-1 boundary on a 0x04 word; the class-D boundary 192/193; a new test over the COR1 door's four words, DEA1's wall, BES1's antenna, the door's stats past the test (MASK), the Falcon's shadow 0x05 at 92 and `myg1_mountains` 0x03 at 115 (both still rule 1); `mus1_sky_dome`'s 0x10001 fixture reads OPAQUE.
  **Gates, reproduced by this session:** ci **703/0/27** exit 0 before each commit, at the head and on main (702 + the new test). **Bake:** the file set was written down BEFORE the build (`PREDICTION.md`: 28 files — the entry named 26, 18 GLBs + 5 KTX2 + 3 manifests; reading `map.rs` added GEO1's manifest, whose `transparent` list is not sky-carved, and MUS1's dome GLB, whose 0x10001 word is class D at max 173); a full scratch bake of the branch read **SAME=21639 DIFF=28 ONLY_IN_A=0 ONLY_IN_B=0**, exactly that set: 19 GLBs (BIN chunks and JSON outside `materials` byte-identical; 38 materials — the 37 rule-1 draws + the MUS1 dome word), 5 KTX2 (the COR1 door and DEA1 wall base + tier 2, BES1's `kam_bldg_antenna`; header and RGB identical on every mip, alpha → 255), 4 manifests (one `transparent` name each: BES1, COR1, DEA1, GEO1). The head's binary baked byte-identical to it. After `rebake.sh maps` on main, a FULL scratch bake of main read **SAME=21667 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0** against the rebaked tree. NT's census: ('1','BLEND','OPAQUE') **37 → 0**; disagreements with §2's reading **602 → 564**. **Walkabout:** the branch's before-capture reproduced the committed set (38/38 byte-identical); regenerated in full on main, **210 of 219 panels byte-identical** and `INDEX.md` unchanged — the 9 that moved (COR1 ×4, DEA1 ×5) are byte-identical to the branch's after-captures, each quantified and viewed: the doors, once see-through to the Coruscant skyline, draw as solid grooved panels with their blue light strip; the reactor walls, once see-through to the red nebula, draw solid with their green light bars (`view/bf2_cor1_cp5_cp6.png`, `view/bf2_dea1_cp3_cp3.png`; COR1 cp5_cp6 17.1 % of pixels, DEA1 cp3_cp3 15.8 %); BES1, GEO1 and MUS1 byte-identical. **Goldens:** `scripts/goldens.sh` on main rewrote all 90, exactly `maps/bf2_dea1.png` moved — RGB mean 0.0000, **max 1**, 0.004 % of pixels, bbox (341,203)–(357,209), alpha unchanged; quantified, then viewed (`land/pairs_00.png` and a 6× crop): the sliver where a distant reactor wall meets the floor at the horizon; byte-identical to the branch's candidate. `scripts/gpu_tests.sh` exit 0 (27) with the goldens clean on the branch and on main. **Sim, on main after the rebake:** `sim_gates.sh` `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both `motion.csv` identical, GATES_RC=0; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp`-identical to each other and to the s19 baseline (4,570 bytes each; footer `finished 27/32 … sum 213000`) — the sweep plays COR1 and DEA1, and no sim path reads `alphaMode` or `transparent`. Arenas not run: flat is synthetic, and `tat2` and `side` were not rebaked. `size_report.sh --check-map` 0 stale.
  **DECIDED (loop, 2026-09-24, lane 1): the new OPAQUE path is labelled by the clause that decides it (`"2"` or `"3"`), not a new `Rule` value.** *Why:* the unflagged words simply stop being rule 1's and go through rules 2 and 3 exactly as every unflagged word with more alpha already does; the GLB `extras` still carry flags, rtyp, mid, a0 and amax, so every verdict re-derives from the file, and the census scripts keep their vocabulary. *How to reverse:* a `Rule::NeverTestsUnflagged` variant and its string.
  **DECIDED (same): the class-D condition is `amax >= ALPHA_TEST`, as the entry wrote it, so `mus1_sky_dome`'s 0x10001 word (max 173) goes MASK → OPAQUE too** — one of the two files the entry's own count (26) missed, both predicted before the build. *Why:* the reason is the same at 173 as at 53 — no texel passes the world's test, so the MASK drew nothing, and the disc draws the word opaque (§6's MUS1 row); nothing drawn moves (the sky path never reads the material, MUS1's panels byte-identical). *How to reverse:* test against `NEVER_TESTS` instead.
  **Found and queued — KW, placed next:** with R1U in, NT's reason for keeping `NEVER_TESTS` at 128 no longer holds. An uncommitted experiment on top of R1U (`NEVER_TESTS = ALPHA_TEST − 1`, a full scratch bake against R1U's): **DIFF=17** — exactly the 8 `Kam_bldg_window_01` draws whose word carries 0x04 turn OPAQUE → BLEND, which is §2's reading, the window sheet keeps its alpha (123..183) and KAM1's manifest gains it in `transparent`; the other 11 moved materials keep BLEND and change only `extras.rule` 2 → 1. `NEVER_TESTS`' doc now says NT measured before R1U. KW names both builds (the raise, or a narrower 0x04 clause) and leaves the choice to its builder.
  **Docs:** QUEUE R1U → DONE, archived verbatim; KW added after it; the GATES block's count 702 → 703; HANDOFF §For the maintainer (a look call and a calls-made line), §What next and §State; a PORTED line in `analysis/re-material-flags.md` §6 and in `RE_LOG.md`'s material-flags entry; `LAST_CLEANSE` 11 of 12.
- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue KW): Kamino's windows draw as glass — rule 3's 0x04 clause composites a sheet no texel of which passes the world's test.** Branch `kam-windows` off `ee84878`, two commits, the rebase a no-op, fast-forwarded as **`bc069b9`** (the clause, its docs and tests in `bake/src/alpha.rs`) + **`684cf5f`** (`engine/CRATES.md`'s counts); `maps` rebaked on main; goldens **`47efeff`**. Report `engine/artifacts/loop/reports/20260924_0500_build_land_KW.md`; the prediction, tools, sheets and every gate log in `engine/artifacts/lead_tmp/kam-windows/` (the landing's under `land/`). Built on `analysis/re-material-flags.md` §2 [V] (BF2 `FUN_0058ed70`: every 0x04 word → ALPHA 0x44, TEST 0x51001 — Z-tested, no Z write) and §6 point 1.
  **What:** in `bake::alpha::verdict`, rule 3's 0x04 clause returns BLEND when the texture's alpha max is below `detail::ALPHA_TEST` (193) and keeps its histogram above it (mid ≤ 0.30 MASK, else OPAQUE). `Kam_bldg_window_01` decodes as glass — a smooth pale blue-grey gradient whose alpha ramps 183 → 123 from top to bottom, every texel in the mid band, no cutout shape (`lead_tmp/kam-windows/tex/window_sheet_rgb_alpha_comp.png`: RGB | alpha | over a checkerboard) — and its three 0x04 words (0x10005, 0x205, 0x10205: 8 draws on five KAM1 models) had read as grime (mid 1.0 > 0.30) and drawn OPAQUE. Its unflagged word 0x10001 stays OPAQUE (R1U). `NEVER_TESTS` stays 128 and its doc says why. Tests: the rule-1 boundary on a 0x02 word (128 BLEND, 129 MASK) and on a 0x04 word (129 now BLEND by rule 3); the 0x04 clause's 192/193 boundary; `kaminos_window_composites_by_its_0x04_words` (0x10005 / 0x205 / 0x10205 → BLEND, 0x10001 → OPAQUE, the same stats past the test → OPAQUE).
  **Gates, reproduced by this session:** ci **704/0/27** exit 0 before each commit and on main (703 + the new test). **Bake:** the file set was written down BEFORE the build (`PREDICTION.md`, from the shipped GLBs' own `extras` by `tools/predict.py`: 8 files); a full scratch bake of the branch read **SAME=21659 DIFF=8 ONLY_IN_A=0 ONLY_IN_B=0**, exactly that set — 5 GLBs (BIN and the JSON outside `materials` byte-identical; 8 materials OPAQUE → BLEND, `extras.rule` stays "3"), the window's base + tier-2 KTX2 (header, index and RGB identical on every mip; alpha 255 → 123..183), `bf2_kam1/manifest.ron` (`transparent: []` → `["kam_bldg_window_01"]`). After `rebake.sh maps` on main, a FULL scratch bake of main read **SAME=21667 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0** against the rebaked tree, and the rebaked tree equals the branch's scratch bake. NT's census: ('3','OPAQUE','BLEND') **68 → 60**, ('3','BLEND','BLEND') **0 → 8**; disagreements with §2's reading **564 → 556**. **Walkabout:** the branch's before-capture of `bf2_kam1` reproduced the committed set (7/7 byte-identical); regenerated in full on main, **212 of 219 panels byte-identical** — the 7 `bf2_kam1` panels moved, each byte-identical to the branch's after-capture, quantified and viewed: the central chamber's window band and the column's glass panes composite, so the cloning tanks and pedestals in the next room show through (`view/cp0_cp1_BL_band.png`, `view/cp0_cp1_TL_band.png`; cp0_cp1 13.6 % of pixels, max 63); the other six move 0.008–0.29 % (exterior door panes and distant window strips, now faintly translucent). `INDEX.md` moved only in `bf2_kam1`'s draw-call counts (vantage 164 → 170; the culled counts unchanged): the blend pass issues one draw per instance where the opaque pass instances a run (`render/src/frame.rs` 376–379, 413–431). **Goldens:** `scripts/goldens.sh` on main rewrote all 90, exactly `maps/bf2_kam1.png` moved — RGB mean 0.7968, **max 55**, 6.305 % of pixels, bbox (297,0)–(1920,486), alpha max 16 on 0.019 %; quantified, then viewed (`land/pairs_00.png`; RGB crops `view/golden_kam1_right_top.png`, `view/golden_kam1_left_x4.png`); byte-identical to the branch's candidate. `scripts/gpu_tests.sh` exit 0 (27) with the goldens clean on the branch and on main. **Sim, on main after the rebake:** `sim_gates.sh` `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both `motion.csv` identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp`-identical to each other and to the s19 baseline (4,570 bytes each; footer `finished 27/32 … sum 213000`). Arenas not run: neither `tat2` nor `side` was rebaked. `size_report.sh --check-map` 0 stale (the workspace total in `CRATES.md`, stale since SPL6, corrected to 63,046).
  **DECIDED (loop, 2026-09-24, lane 1): the narrow clause, not the raise.** KW offered two builds that move the same 8 draws. *Why:* NT's two other reasons still stand — the 8 are rule 3's 0x04 clause's to answer (the disc reads the bit as BLEND, §2), and the disc has no rule 1 (§6 point 4), so the port-only clause should not widen where the disc's own reading covers the case; the raise would also composite a 0x02 word in 129..192, which BF2's disc draws nothing of (AFAIL KEEP) — what MASK draws now (none ships). And the clause completes R1U's pattern: below the world's test rule 3's histogram has nothing to cut, so each word takes the disc's reading of its bits — unflagged OPAQUE (R1U), 0x04 BLEND (KW), 0x02 the test itself. It is also the smaller change: no relabels, no `common` rebake. *How to reverse:* drop the clause and set `NEVER_TESTS = crate::detail::ALPHA_TEST - 1` (the raise: the same 8 flips + 11 relabels; `rebake.sh maps common`), or drop the clause alone and `rebake.sh maps`.
  **Docs:** QUEUE KW → DONE, archived verbatim; the GATES block's count 703 → 704; HANDOFF §For the maintainer (a look call and a calls-made line), §What next and §State; a PORTED line in `analysis/re-material-flags.md` §6 and in `RE_LOG.md`'s material-flags entry; `LAST_CLEANSE` 12 of 12 — the cleanse (job B2, area `render`) is due next.

- **2026-09-24 — LANDED (loop, lane 1, job B, queue SKO): a soldier behind a translucent world surface is drawn under it — every opaque draw, world and skinned, goes before ONE blend pass that sorts the world's and the skinned BLEND draws far to near together.** Built by iteration 14 (branch `skinned-order` off `de7f771`, report `engine/artifacts/loop/reports/20260924_0615_build_SKO.md`), landed by the next session (`engine/artifacts/loop/reports/20260924_1915_land_SKO.md`): fast-forwarded as **`662ba97`** (`render/src/frame.rs`'s `render_frame` + `blend_order`, `draw.rs`'s Blend docs, two GPU referees in `render/tests/skinned.rs`) + **`2e88ace`** (`engine/CRATES.md`) + **`b29b9d5`** (the landing's comment-only citation fix: `blend_order` cites `analysis/re-fog.md` §2.2 for line 723381). Built on `analysis/re-side-alpha.md` §3 [V][O] (one function draws every model segment, skinned included) and `re-material-flags.md` §2 [V] (a 0x04 word → bucket 0xc); the bucket ORDER stays [H] (§8; `re-side-alpha.md` §7 now carries the PORTED note).
  **Reproduced by the landing:** ci **707 / 0 / 29** exit 0 on the branch and on main (the GATES block moves 706 → 707 — `frame::tests::the_blend_pass_interleaves_world_and_skinned_draws_far_to_near` — and 27 → 29 ignored, the two GPU referees); `scripts/gpu_tests.sh` exit 0, 29 passed, on the branch and on main; with main's `frame.rs` swapped into the branch the two new referees FAIL (`[218, 0, 0, 255]`, `[0, 160, 117, 255]` — the report's values); `scripts/goldens.sh` on main regenerated **0 of 90**; `--sim-hash 100` `17e1b946ff0e821c`; `size_report.sh --check-map` 0. Taken from the build report, not re-run (render-only, nothing a sim or bake gate reads changed): the walkabout 219/219 byte-identical and the KAM1 glass view (2,594 px moved, all the soldier's).
  **DECIDED (loop, 2026-09-24, lane 1): the skinned opaque draws stay AFTER the water runs.** *Why:* the water is no `RedModel` draw (`re-material-flags.md` §1) and where the disc draws it against the model buckets is not in `analysis/`, so a soldier under water stays as it was. *How to reverse:* split the run walk at the Water boundary and draw the skinned opaque loop there.
  **DECIDED (loop, 2026-09-24, lane 1): one blend pass; a distance tie draws the world draw first.** *Why:* the disc draws both kinds through one function; the stable sort keeps world-then-skinned input order. *How to reverse:* chain the skinned draws first in `blend_order`.
  **DECIDED (loop, 2026-09-24, lane 1): no golden or walkabout regeneration owed; the [GOLDENS] mark does not apply.** *Why:* 0/90 goldens and 219/219 panels byte-identical — no committed frame has a soldier behind a BLEND surface. *How to reverse:* n/a.
  **DECIDED (loop, 2026-09-24, lane 1): the far-fade band keeps the world's own approximation.** *Why:* a fading soldier (a depth-writing composite) now goes down before the world's blends, so a blend surface behind him is depth-rejected under his silhouette, exactly as the world's `_fade` draws already are; the disc's fading soldier is a no-Z-write translucent draw (`re-side-alpha.md` §5 [D][O]) and neither order is the disc's. *How to reverse:* move the fade-band skinned draws into the blend pass.
- **2026-09-24 (evening) — RE (loop, lane 1, job D at max, queue TL): stock PS2 SP conquest sets NO time limit, so a round that stalls at tickets = alive has no timer exit on the disc either; the timer is an Instant Action option, Off at every power-on.** No branch, no code change, no gate run and none could move: docs only. Lifted into **`analysis/re-time-limit.md`** under the `# OPUS RE … [O] UNAUDITED: no Fable pass` header with every load-bearing claim tagged `[O]`; `re-tickets-spawn.md` gains a pointer §6 (and its §1.2 "no-op" and §1.3 time-limit lines point at it); RE_LOG pointer added; HANDOFF §What next and §HOLD FOR FABLE's open-[RE] list amended; scratch, scripts and every output under `engine/artifacts/lead_tmp/re/timelimit/`; report `engine/artifacts/loop/reports/20260924_2035_re_TL.md`. Done in two sessions: iteration 3 read the hops and drafted, then ended while WAITING on a background verifier (its verdicts were lost); this restart re-ran the verifier in the foreground and added four claims (C23–C26).
  **What:** all 33 BF2 `*_con` missions pass `multiplayerRules = true` and no `timeLimit`, so `Objective:Start` (objective.luac `main.f7` pc 29–41) takes `ScriptCB_GetCONMaxTimeLimit()` = `int[0x5fa8c8 + 4·rules] × 60` (0x19b8b0, decomp 43020), a table that is all 0 in the ELF (file off 0x49b8c8) — so `loseTimer` is never created (pc 108–131), and no conquest mission runs a round timer of its own (only TAN1's 0.3 s console one-shot and DEA1's compactor cycle create timers at all). The option is `ifs_instant_options`' `con_timer` (Off, 5..60 min in 5s), written only through `ScriptCB_SetNetGameDefaults` (SHELL.LVL ×2; the table's only code writer); when set, `GameOptionsTimeLimitUp` gives the round to more tickets and a tie to both teams. BF1: no conquest time limit found (two searches, [H]).
  **Verified:** the four self-checks (every citation's enclosing function printed; every absence claim searched — an immediate superset scan and a lui-resolving scan over the option block, 0 `$gp` accesses in BF2's `.text`, a whole-image pointer scan, every name in all 33 `_con` missions and all 269 BF2 `.luac`, all 186 LVLs, every BF1 file; BF1's cull opened against BF2; every constant read from the file). One `opus` verifier given the claims only: **25 SUPPORTED, 1 PARTLY, 0 NOT SUPPORTED** (C18's BF1 file count was 153, is 148 — corrected); the lead reproduced its load-bearing counts (`lead_repro.py`).
  **Found on the way:** the `_con` ticket percentage is a player option (10–500 %; `re-tickets-spawn.md` §1.2's "no-op" holds for the stock options only), and **Death Star conquest takes a SECOND ticket from team 1 per team-1 death** (`dea1{c,g}_con` `ScriptPostLoad` pc 25–28: `OnCharacterDeathTeam(…, 1)` → `AddReinforcements(1, -1)`) — the port's `bf2_dea1_con{,_cw}.ron` carry neither handler.
  **Queued:** **TLO** (the Instant Action conquest timer + the percentage row, Off / 100 % by default, no gate moves) and **DSD** (DEA1's second ticket, [SWEEP] — it re-baselines the sweep, so it lands after every other PARKED entry).
  **DECIDED (loop, 2026-09-24, lane 1): the port owes no DEFAULT conquest time limit, and the sweep's three NO-OUTCOME maps are not a rules defect.** *Why:* the stock disc's value is 0 = Off and its only exits are the port's (0 tickets, 20 s domination); whether the disc's AI breaks those stalls is an AI question. *How to reverse:* a default limit would be a LOCKED FEATURE SET change — the maintainer's call; TLO adds the option Off.
  **DECIDED (loop, 2026-09-24, lane 1): TLO's shell takes the disc's reinforcement PERCENTAGE row (10–500 %, step 10, default 100 % of each team's own count) in place of Shell v0's absolute count.** *Why:* it is the disc's own option (`re-time-limit.md` §5), and the absolute row seeds BOTH teams from team 1's count, flattening `bf1_kam1_con`'s 200 / 220 to 200 / 200; PLAN §Presentation (2026-09-03) lets the menu content change either way. *How to reverse:* keep the absolute row and seed it per team (`game/src/main/windowed.rs`).
- **2026-09-24 (night) — LANDED (loop, lane 1, job C, queue item 9): BF2 COR1's sky-lane strips are not additive, they are an alpha-TESTED card that ALPHA RULE v2 drew opaque — a word the disc alpha-tests is now a cutout whatever its `mod` bits.** Built and landed in one session (branch `cor1-sky-lanes`): `c263b99` (`bake/src/alpha.rs`: `disc_alpha_tests` + `BF1_NON_NORMAL_CLASS`, rule 2's `mod` term yields to it; one test), goldens `7f7b1bd`. **What the strips are:** `cor1_prop_buldings_sky{,LOWD}` seg `cor1_sky_cars`, BF2 word **0x1000002** (0x02 + UV-animation type 1, no 0x40), a white 128x32 card whose alpha is the cars' silhouettes (a0 0.925); rule 2's `mod` term (0x1000000) drew it OPAQUE, so the white showed as bands. BF2 alpha-tests every 0x02 word without 0x04 (`re-material-flags.md` §2 BF2 draw table, `FUN_0058ed70` 723478–723479 *(decompiler detail omitted)*, re-read this session and the enclosing function printed); BF1 only in its Normal class (§0.4). Port-wide census (`lead_tmp/cor1-sky-lanes/census_r2_cutout.py`): 15 draws move OPAQUE → MASK, all BF2 — COR1's sky cars (2), MUS1's lava fall `mus1_main_3` (6) and `mus1_bldg_control06` (4), YAV1's `blinking lights` (2), `com_icon_republic` (1, drawn by nothing); no BF1 draw.
  **Reproduced by the landing:** ci **708 / 0 / 29** exit 0 on the branch and on main (GATES 707 → 708: `alpha::tests::a_word_the_disc_tests_is_a_cutout_whatever_its_mod_bits`); `gpu_tests.sh` exit 0, 29 passed, on the branch and on main; full scratch bake of the branch SAME=21647 DIFF=20 (the 15 model GLBs and 5 textures of those draws, no manifest); `rebake.sh maps common` on main, then a scratch bake of `maps common` → SAME=5951 DIFF=0, and the branch's full scratch bake against the shipped tree SAME=21667 DIFF=0. **Goldens:** 2/90 moved, both declared — `maps/bf2_cor1.png` mean 0.782 max 181, 1.34 % px, bbox x0-1487 y36-132 (the white bands gone); `maps/bf2_mus1.png` mean 0.138 max 125, 0.70 %, bbox x21-1919 y0-332 (the lava falls cut where their alpha fails the test); viewed (`lead_tmp/cor1-sky-lanes/pairs_00.png`, `goldens_after/*_pair.png`). Walkabout regenerated for bf2_cor1/mus1/yav1: cor1 `cp1_cp2` 2.0 % and `vantage` 2.6 % (the strips), mus1 four panels ≤ 1.3 %, yav1 a handful of pixels (`lead_tmp/cor1-sky-lanes/sheets/`). No sim code: `--sim-hash 100` pinned by its ci test. Report `engine/artifacts/loop/reports/20260924_2130_build_land_9.md`.
  **DECIDED (loop, 2026-09-24, lane 1): item 9 takes the rule fix, not an additive pipeline; the Specular second pass (item 9's 2026-09-08 addendum) becomes its own entry SPEC.** *Why:* the strips carry no 0x40 — the entry's own "if not, report what they are" branch — and the spec pass needs RE first (the highlight texture from the power, `FUN_00589ac0`). *How to reverse:* n/a (SPEC is queued).
  **DECIDED (loop, 2026-09-24, lane 1): the fix is general (every word the disc tests), not a COR1 special case, and MUS1's lava falls are accepted cut.** *Why:* the ELF evidence is the same for all 15 draws; the lava fall's crust alpha (63 % of texels ≥ 193) now shows as gaps in the fall, which is what TEST 0x5000b does at the port's cutoff. Open [H]: a COL0 vertex alpha other than the material's would move the disc's effective cutoff for that word — the port's MASK uses one cutoff for every draw, as before. *How to reverse:* drop `&& !disc_alpha_tests(..)` in `bake::alpha::verdict` and `rebake.sh maps common`.
- **2026-09-24 (night) — RE (loop, lane 1, job D at max, queue SPEC's [RE] half): the PS2's specular pass is a reflection-vector lookup into ONE shared grey table, not a texture built from the power; it is flat-coloured and added by the base sheet's alpha.** No branch, no code change, no gate run and none could move: docs only. Done by the escalated restart after iteration 8 died about 5 minutes in, leaving four scratch files.
  Where it went:
  - lifted into **`analysis/re-specular-pass.md`** (`[O]` header, every load-bearing claim `[O]`);
  - correction notes in `re-material-flags.md` §2 (the 0x80 row, BF1's Specular bullet) and `re-texture-wrap.md` §1.3 (its vi1/vi2 open point, resolved);
  - RE_LOG pointer; HANDOFF §What next, §HOLD FOR FABLE and §For the maintainer amended;
  - scratch, tools and every output under `engine/artifacts/lead_tmp/re/specular/`, including a fresh VU1 decoder, `vu1dis.py`. The s12/s17 decoder mislabels every A = 0x100·k instruction and swaps `SQ` operands.
  - report `engine/artifacts/loop/reports/20260925_0010_re_SPEC.md`.

  One `opus` verifier, run in the FOREGROUND with the claims only: **23 SUPPORTED, 7 PARTLY, 0 NOT SUPPORTED** (`lead_tmp/re/specular/verify/VERDICTS.md`), all folded in. The lead reproduced its one load-bearing new finding (C4's alpha byte) from the bytes, 0x58fdc0–0x58ff2c.
  **Queued:** SPEC's build half (the entry rewritten with the recipe, `re-specular-pass.md` §9) and **SPEC2** (the same pass on the side models, skinned, after SPEC lands).
  **DECIDED (loop, 2026-09-24, lane 1): the SPEC build reproduces the disc's MIRRORED light (d = 2(X·L)(X·R) − L·R) by default, behind a flag for the consistent d = L·R.** *Why:* PS2 fidelity is the port's rule. The frame mix is in both games' code (both camera inverses and both VU view builders read from the bytes), and whether it shows on screen needs an in-map capture the rig cannot make yet (HOLD 7). *How to reverse:* flip the flag's default.
  **DECIDED (loop, 2026-09-24, lane 1): BF2's translucent (0x04|0x80) spec pass is not drawn.** *Why:* on the disc it adds Cs·As with As = 1/128, at most 2/255 — invisible (`re-specular-pass.md` §6). *How to reverse:* bake those 22 words' `spec_pass` with a 1/128 weight.
  **DECIDED (loop, 2026-09-24, lane 1): SPEC is split — SPEC builds the world pass, SPEC2 the side models.** *Why:* the skinned path is a different bake (`bake::soldier`) and draw. SPEC2 reuses SPEC's pipeline, table and MTRL parse. *How to reverse:* n/a (queue order).
- **2026-09-25 — LANDED (loop, lane 1, job C, queue SPEC): the specular second pass on world models — an additive lookup into the disc's `specular` table (`analysis/re-specular-pass.md`, every claim `[O]`, so PROVISIONAL throughout).** Built and landed in one session: build `e58ede7`, goldens `d99e15c`; report `engine/artifacts/loop/reports/20260925_0100_build_land_SPEC.md`.
  **What:**
  - `mungefs::model` parses the MTRL's specular fields (BF2 spec RGBA at 8 + power at 12; BF1 A at 4, B at 8).
  - `bake::spec` writes `extras.spec_pass = {power, rgb | null}` on BF2 0x80-without-0x04 and BF1 class-0x20 segments (map and common bakes; the side bake is off until SPEC2). `null` = the map's sun colour, resolved at draw time as `sun_color·255/128`, because the lighting is read later in the LVL than the models.
  - `bake specular <CORE.LVL> <outdir>` bakes the table into `common_bf{1,2}/specular.ktx2` (the `common` group; the two discs' files are byte-identical). The runtime uploads level 0 only, linear, REPEAT, bilinear.
  - `render`: `DrawLayer::Spec` with ONE additive pipeline (One/One on colour, destination alpha kept, `LessEqual`, no depth write), sorted between the masked and the water runs; `vs_spec` does the tex-gen per vertex (R = 2(N·Î)N − Î, d = 2(X·L)(X·R) − L·R, uv = (power/255, 0.5 + 0.5·d)); `fs_spec` adds `min(1, T·rgb)·Ad` in the GS's gamma units by recomputing the base shade and returning the linear difference. `render/src/specular.rs` holds the table upload, the per-frame `SpecParams` and `spec_texcoord` (the CPU twin).
  - `game::viewer::scene` draws each spec segment a second time, installs the table (`OPENBF_SPEC=0` turns the pass off; `OPENBF_SPEC_MIRROR=0` takes d = L·R), and culls BF2's pass at `NearSceneRange` c.
  **Gates, reproduced by the landing:**
  - `./ci.sh` 0 on the branch and on main: **714 / 0 / 32** (+6 unit tests, +3 GPU referees in `render/tests/specular.rs`: the add of a known texel × colour × mask over a known base, in gamma; the depth and no-table cases; the tex-gen against `spec_texcoord` in both light modes). `scripts/gpu_tests.sh` 0 on both.
  - Scratch bake: DIFF = 119 model GLBs (21 map dirs, `tat2`, `common_bf{1,2}`), ONLY_IN_A = the 2 tables, `side` untouched. All 222 spec materials are OPAQUE, so no pass sits on a masked or blended base. After `rebake.sh maps tat2 common` on main, a scratch bake of main cmp'd DIFF=0 (6,151 files), and the branch's full scratch bake read SAME=21669 against the rebaked tree.
  - `--sim-hash 100` = `17e1b946ff0e821c`, unchanged (render/bake only).
  - The walkabout rendered with the pass OFF on the new bake was byte-identical to the committed set (29/29 panels over yav1, kam1, dea1, tan1); every move with it ON brightens only. Walkabout regenerated for the 17 touched maps.
  **Goldens moved (8, all declared candidates, 0 darker pixels):** bf1_kam1 mean 5.02 (51.7 % of pixels: the deck and domes, BF1 white B → the sun colour raw at low power, a broad lobe), bf2_kam1 mean 1.08 max 231 (one dome's sheen), bf1_kas2, bf2_uta1, bf1_yav1, bf2_dea1, bf2_tan1, bf2_yav1 (≤ 0.5 % of pixels each).
  **DECIDED (loop, 2026-09-25, lane 1): the landed look is accepted as the disc's.** *Why:* every move is the `[O]` recipe applied as written, and the brightest moves (KAM1's broad deck glow, BF2 KAM1's dome sheen) are what low powers and BF1's raw sun colour predict (§2, §5). An in-map PCSX2 capture is what would settle it. *How to reverse:* `OPENBF_SPEC=0`, or revert `e58ede7` + `d99e15c`. Sheets: `engine/artifacts/lead_tmp/spec-pass/pairs_0{0..3}.png`, `sheets/`.
  **DECIDED (loop, 2026-09-25, lane 1): the specular add is done in gamma space by recomputing the base's colour in `fs_spec`, and the fog and the far fade apply to both sides of the difference.** *Why:* the GS adds in its own units; a plain linear add on the sRGB target under-shoots a mid-grey base by about half. Whether BF1 fogs a Specular segment is unread ([H], §7). *How to reverse:* return `add` linearised from `fs_spec` instead of the difference.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue SPEC2): the specular second pass on side models — soldiers skinned, their weapons rigid (`analysis/re-specular-pass.md` §1, `re-side-alpha.md` §3; every claim `[O]`, so PROVISIONAL).** Built and landed in one session: build `1583ac0`, crate map `54d9560`, goldens `c231edb`; report `engine/artifacts/loop/reports/20260925_0200_build_land_SPEC2.md`.
  **What:**
  - `bake::skin_glb::SkinnedSegment::spec` writes a soldier segment's `extras.spec_pass` exactly as `model_to_glb` does (`bake::spec::spec_pass`, BF2 0x04 still skipped). The weapon bake takes the pass too, so `model_to_glb`'s `spec` flag (true everywhere now) is gone.
  - `render`: `SkinnedDrawCmd::specular` (layer `Spec`), `vs_spec_skinned` (R from the SKINNED normal; it and `vs_spec` share `spec_varyings`), one skinned specular pipeline on SPEC's layout. It draws right after the skinned opaque run and before the blend pass, and is skipped without a table, as the world's is.
  - `game`: `CpuSkinnedMesh::spec`. `SpecPass::rgb_under` is now the one copy of the BF1 sun rule (scene.rs's `spec_rgb` is folded into it). `soldier_vis::part_spec` resolves each part's pass against the sun already on the renderer; `with_spec` pushes the base plus its pass for soldiers, and the weapons take the rigid pass. The gallery's reference soldier and unit sheet do the same. `install_spec_table` now takes the baked tree and the game, so the unit sheet installs the side's table.
  **Gates, reproduced by the landing:** `./ci.sh` 0 on the branch and on main, **715 / 0 / 34** (+1 GLB round trip; +2 skinned GPU referees: a skinned pass adds exactly as the rigid one, keeps its base's depth and needs a table; a posed skinned quad draws the same frame ±1 as the rigid quad at the same matrix, in both light modes, under a ramp table). `scripts/gpu_tests.sh` 0 on both. Scratch bake: DIFF = 17 side GLBs only, SAME=21652, ONLY 0/0 (BF2: 10 soldiers with 12 spec segments + 2 weapons; BF1: 3 soldiers with 4 segments + 2 weapons; all 17 spec materials OPAQUE). After `rebake.sh side` on main, a scratch bake of main's `side` group read SAME=283 DIFF=0, and the branch's full scratch bake read SAME=21669 DIFF=0 against the rebaked tree. `--sim-hash 100` = `17e1b946ff0e821c` (no sim code). No walkabout regeneration: neither reference soldier's GLB changed.
  **Goldens moved (28 unit sheets, 0 map sheets, all declared candidates, byte-identical to the branch's, 0 darker pixels):** the 13 soldiers with a spec segment (biggest: bf1_cis_inf_sbdroid mean 4.65, bf1_imp_inf_darktrooper 3.69, bf2_imp_inf_atatcommander 2.47, bf2_imp_inf_darktrooper 2.37), and 15 Republic units whose only move is the rifle's glint (`rep_weap_inf_rifle`, mean 0.003).
  **DECIDED (loop, 2026-09-25, lane 1): the side-model look is accepted as the disc's.** *Why:* the moves are the `[O]` recipe applied as written. The brightest are what it predicts: BF1's white B takes the sun colour raw over 128 (about 2×) at power 4, so the droids and the dark trooper turn chrome; BF2 `imp_inf_atatcommander`'s power-1 record reads the table's column 0 and washes the helmet and vest near-white, the "≥ +50 %" glow SPEC's RE foresaw. An in-map capture would settle it. *How to reverse:* `OPENBF_SPEC=0`, or revert `1583ac0` + `c231edb` and `rebake.sh side`. Sheets: `engine/artifacts/lead_tmp/spec-side/pairs_{00..13}.png`.
  **DECIDED (loop, 2026-09-25, lane 1): a side part's sun-coloured pass is resolved when the soldiers LOAD, from the sun `upload_scene` has already put on the renderer; the gallery's unit sheet, which has no map, takes the renderer's default sun and installs the side's own table.** *Why:* the side bake is map-independent and every caller loads the soldiers after the scene, so no frame-time plumbing is needed. The unit sheets then show the pass as the regression net for it. *How to reverse:* drop the unit sheet's `install_spec_table` call (its 28 goldens move back).
  **Not built:** the first-person weapon. The port has no first-person view; the census's `*_1st_*` sheets are the third-person weapon GLBs this entry does cover.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue W3): the water's additive speckle layer on BF1's flat water** (`analysis/re-water.md` §1/§3 [V], `re-water-alpha.md` §7.1 `[O]`). Build `6405cd4`, crate map `0c5d9ae`, goldens `3085b01`; report `engine/artifacts/loop/reports/20260925_0145_build_land_W3.md`. `render`: `DrawLayer::Speckle`, `DrawCmd::speckle` and one additive pipeline (`fs_speckle`; (One, One), depth `LessEqual`, no depth write — the PS2's record 0x69c3b0, ALPHA 0x48, TEST 0x51001), sorted after every water run; the sheet's alpha is not read (the near speckle's TCC is cleared); faded by the water's fog ramp and far fade; `TextureDesc::uv_scale` (read only by `fs_speckle`); the additive blend state now shared with the specular pass. `game`: `viewer::look::speckle_look` + `speckle_frames`, `SceneGpu::speckle` stepped in `animate_water` (it rides the water's draw range, so the bob and ripple carry it); `OPENBF_WATER_SPECKLE=0` turns it off. Read this session (BF1 `FUN_0048b9c0` 551399–551402, the config switch 550402–550408): the speckle's UV is the cell's 0..1 × `SpeckleTile` + `frac(t·SpeckleScrollSpeed)` — NOT × INFO5, which scales only the main layer (538831–538838) — so against the mesh UV the multiplier is `SpeckleTile · metres_per_repeat / cell` (KAS2 0.625: 25.6 m per repeat).
  **Gates, reproduced by the landing:** `./ci.sh` 0 on the branch and on main, **718 / 0 / 34** (+3 unit tests: the speckle's sort position, `speckle_look` on KAS2's numbers and its BF2/ocean/no-frame refusals, the flipbook's wrap and cross-fade). `scripts/gpu_tests.sh`: on the branch every render GPU referee passed and the goldens check failed on bf1_kas2 alone (declared); on main after the golden commit, exit 0. `--sim-hash 100` = `17e1b946ff0e821c` (no sim code). No bake change. KAS2 at the golden vantage (`water_frames`, `lead_tmp/water-speckle/frames_{on,off}_bf1_kas2/`): water mean abs 6.42 between the 0 / 0.5 / 1 s frames with the layer on, 0.005 off; vantage frame time 0.146 vs 0.144 ms mean (median 0.138 vs 0.136) on vs off — inside the 5 % gate. Walkabout regenerated for bf1_kas1, kas2, nab2, yav1.
  **Goldens moved (4, all BF1 speckle maps, alpha untouched, every changed pixel brighter):** bf1_kas2 mean 4.673 (31.4 % of pixels), bf1_kas1 0.675 (16.5 %), bf1_nab2 0.004 (a fountain pool), bf1_yav1 0.001 (a corner sliver); every other map and unit sheet mean 0.000.
  **DECIDED (loop, 2026-09-25, lane 1): the speckle's colour is `SpeckleSpecularColor` flat — rgb linearised like every authored colour, its alpha /255 as the add weight — and the KAS2 look (a moving grey caustic shimmer over the whole near sea) is accepted.** *Why:* the queue entry names that colour, and the PS2's real one is per vertex from the unread VU1 program (`re-water.md` §3 cit. 7, §8: it also holds `SpeckleAmbientColor` and a light vector), so any other flat colour would be as much a guess; the specular colour is the brightest reading and may overstate the layer away from the sun's glint. The add is linear where the GS adds in gamma (dimmer over dark water than the disc's). *How to reverse:* `OPENBF_WATER_SPECKLE=0`, or revert `6405cd4` + `3085b01`. Sheets: `engine/artifacts/lead_tmp/water-speckle/pairs_0{0,1}.png`, `kas2_crop_sheet.png`.
  **DECIDED (loop, 2026-09-25, lane 1): the PS2's one cross-faded draw is two additive draws weighted `1 − w` and `w`, and BF2 stays gated on `SpecularEnable` as `re-water.md` §1 reads it.** *Why:* two adds of one surface sum to the cross-faded add without a second texture slot in the renderer. The BF2 gate is PROVISIONAL: the 2026-09-24 effort A/B's graders found `DAT_006615f9` reset to 1 at B2:695896, which would make every BF2 flat-water map draw both the speckle and the far `fogspread` — a claim this session lead-grepped (four lines name the flag) but did not put through §5, so it is queued as **WSE** ([RE] first, next in line). *How to reverse:* the gate is one clause in `speckle_look`.
- **2026-09-25 — RE (loop, lane 1, job D at max, queue WSE's [RE] half): BF2's `SpecularEnable` and `OscillationEnable` both default to 1, so BF2's flat water draws the speckle layer and the far `fogspread` sub-pass, and BF2 FEL1 — the one map that authors `OscillationEnable(0)` — has still water.** No branch, no code change, no gate run and none could move: docs only. Where it went:
  - lifted into **`analysis/re-water-flags.md`** (`[O]` header, every load-bearing claim `[O]`);
  - corrections in `re-water.md` §1, §5, cit. 8, §6 and §7's W3 line; `re-fog.md` §2.1 and §2.2's flat-renderer row;
  - RE_LOG pointer; HANDOFF §HOLD FOR FABLE, §What next and §For the maintainer; PLAN §WHERE THINGS STAND items 3 and 5;
  - scratch, tools, `claims.md` and the verifier's `verify/VERDICTS.md` under `engine/artifacts/lead_tmp/re/wse/`;
  - report `engine/artifacts/loop/reports/20260925_0210_re_WSE.md`.

  **What (all [V] [O]):**
  - **The gate.** BF2's flat renderer `FUN_00575fd8` tests `DAT_006615f9` before both its far speckle + `fogspread` block (708831; ELF 0x576848) and its near cross-faded speckle (708897; 0x5766dc); BF1's `FUN_0048b9c0` tests nothing (0x48bf98 splits on argument 1 alone), and the SpecularEnable hash is nowhere in the BF1 ELF.
  - **The default.** The byte is `.sbss` and has exactly two writers: the Water effect's level reset `FUN_005601a8` stores 1 (`sb` at 0x56027c) and the parser arm for 0xd4ddec5f stores `v != 0` (0x561124). Searched by every load/store width, lui-formed pointers (39, each read), gp (0 gp-based accesses in `.text`), data words and the decomp names.
  - **The order.** The reset is reached from vtable 0x63a6b8 +0x24 ← the effect manager `FUN_00450cf8` ← `FUN_00174b28` ← the "mission" state's enter (0x175af8), which calls ScriptInit — where the map's world LVL and its `fx__` are read — right after (0x175b1c → 0x175b24).
  - **The disc.** 0xd4ddec5f occurs in 0 of 186 BF2 and 0 of 137 BF1 LVLs, positive controls hitting. So the near speckle draws on all six BF2 flat-water maps and the far speckle on KAS2; the far fog shows nothing on any shipped map (KAS2's fog is off; NAB2/DEA1/FEL1 have no far layer; YAV1/DAG1 author `DisableLowRes`).
  - **The layer.** BF2's speckle record (0x76c190 = {0x4148, 0x51001, 0x58}), its near TCC clear and its switch words equal BF1's, so W3's pipeline is the right one for BF2.
  - **The bob.** `OscillationEnable` (0x12e39af3 → `DAT_006615f8`, reset to 1 at 0x5602a4) gates the 6 cm bob in `FUN_005603e0`; FEL1 authors it 0; BF1 has no such key.
  - **Corrections.** `re-water.md` cit. 8's LAYR/CPAT/INFO lines are in `FUN_005609f0` (not `FUN_00560f40`); `re-fog.md` §2.1's "fog off → rows zero" holds for the first map after boot only (the rows have no clearer).

- **2026-09-25 — WSE landed (loop, lane 1, job C at medium, built and landed in one session): BF2's two Water flags as the PS2 reads them.** `bb6b0d2` (build) + `a8b2f41` (the ocean fix below) + `52c84a5` (golden) + `4c3553d` (crate map); report `engine/artifacts/loop/reports/20260925_0315_build_land_WSE.md`.
  - **What:** `bake::water_glb::manifest_config` writes `specular_enable` and a NEW `oscillation_enable` as `authored.is_none_or(|v| v != 0)` (the level reset's 1, `re-water-flags.md` §2/§7 [O]); `manifest::ManifestWaterConfig` gains `oscillation_enable`, both with a serde default of true; `game::viewer::look` — BF2's speckle and far-water fog follow the flag (fog only on FLAT water); `game::water_anim::{oscillates, flat_lift}` — FEL1's plane no longer bobs; `render::lighting`'s `water_fog_begin` doc corrected.
  - **Gates reproduced:** ci 718 → **720** / 0 / 34 on the branch and on main (+2 tests: the bake flags, the still bob; the `look.rs` tests were extended in place). Scratch bake `maps` on the branch: **DIFF = exactly the 12 water `manifest.ron`s** (each: `specular_enable` false → true, `oscillation_enable` added, false on `bf2_fel1` only); `rebake.sh maps` on main, main scratch bake DIFF=0 (SAME=5322, 21,669 total). gpu_tests 0 on the branch against the OLD bake (no golden moved) and on main after the golden. `--sim-hash 100` `17e1b946ff0e821c`. Frame time, `water_frames` on BF2 KAS2, vantage stepping: 0.110 ms mean / 0.108 median speckle on vs 0.109 / 0.107 `OPENBF_WATER_SPECKLE=0` (+0.9 %; 194 vs 192 draws). Walkabout regenerated for bf2_kas2 / nab2 / dea1 / fel1 / yav1.
  - **Moved:** golden `maps/bf2_kas2.png` only — mean abs 0.044, max 10, 2.77 % of pixels, bbox x 93..999 y 136..445, every changed pixel brighter, alpha untouched: a faint grey speckle on the pool between the platforms (`lead_tmp/water-flags/pairs_00.png`, `kas2_crop.png`). **The entry's prediction was wider:** nab2, dea1, fel1 and yav1 did not move — their golden framings show no water (viewed nab2 and fel1; an additive layer on visible water would have moved bytes); dag1 as predicted (black speckle).
  - **Deviation — caught on the branch:** the first commit fogged BF2 KAM1's sea (its golden moved on an overlay preview). `re-water-flags.md` §1/§5 puts the gate in the FLAT renderer `FUN_00575fd8`; the ocean `FUN_00575670` is the driver's other branch. `a8b2f41` restricts the BF2 fog gate to `!ocean`; `bf2_kam1` then regenerated byte-identical. Also outside the entry's file set: the one doc comment in `render/src/lighting.rs`.
  **DECIDED (loop, 2026-09-25, lane 1): a BF2 ocean (KAM1) stays unfogged, as the port always drew it — the `SpecularEnable` fog gate applies to BF2 flat water only.** *Why:* the gate is read in the flat renderer and BF2's ocean renderer is unread for fog; keeping the ocean as it was moves nothing unproven. *How to reverse:* drop `!c.ocean &&` in `water_look`'s `gated_on` (moves `bf2_kam1`); an RE of `FUN_00575670`'s fog would settle it.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue DR1, built and landed in one session; a DECLARED [SWEEP] re-baseline): a BF2 door stops being a permanent wall.** `bbf281a`; report `engine/artifacts/loop/reports/20260925_0340_build_land_DR1.md`. `Scene::collision_world` now delegates to `collision_world_with(doors_solid)`: every body of an instance whose class BASE is `door` loses `Layers::SOLDIER` unless `OPENBF_DOORS_SOLID=1`, and keeps its ORDNANCE bit, so bolts and AI line of sight still stop on a door. This is a documented DEVIATION standing in for DR2's door entity (`analysis/re-bf2-doors.md` §2, §3 [V][O]: the disc's door opens itself for a nearby soldier and its box rides the `open` clip out of the doorway). Door classes are on five maps: bf2_mus1, pol1, tan1, kam1 and dea1. `main.rs`'s env list gains the flag, a file outside the entry's set (two doc lines).
  - **Gates reproduced:** ci 722 → **724** / 0 / 35 on the branch and on main (+2 in `game/tests/collision.rs`: a door is out of the soldier subset and still in the ordnance one, with ray checks both ways; a non-door class is unchanged by the rule). gpu_tests 0 on the branch and on main, `engine/goldens` clean; no bake, no golden. UNMOVED: `sim_gates.sh` GATES_RC=0 (`bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, motion.csv identical), `--sim-hash 100` `17e1b946ff0e821c`, and both arenas (cmp-identical to DET1's files). **MOVED:** the sweep (branch ×2 and main ×1, all cmp 0) went from 26/32, median 1020 (960), >10k 4, sum 195180 to **29/32, median 960 (960), >10k 2, sum 107100**. Exactly the five door maps' rows moved: bf2_dea1, bf2_mus1 and bf2_tan1 now finish (team 1 at tick 10731, team 1 at 12590, team 0 at 9437), while bf2_kam1 and bf2_pol1 still end with NO OUTCOME. **Attribution, one variable:** `OPENBF_DOORS_SOLID=1` on the branch reproduces DET1's sweep file byte for byte. **Census** (`STUCK_CENSUS=1`): bf2_mus1 went from 10 stuck (8 wall) to **0**. bf2_pol1 went from 8 (5 wall) to **3** (0 wall): 2 off-graph under-terrain at (93.5,13.1,−60) and 1 no-arc at (−92,21.7,−128), both positions that were already stuck before. No other map's count rose. Baselines are in `lead_tmp/loop_baselines/20260925_0335_DR1/` (SUMMARY, the sweep, the arenas, both censuses). The GATES block was rewritten (the sweep file, the footer, the ci count).
  **DECIDED (loop, 2026-09-25, lane 1): accept the re-baseline, and keep the door open by default as the entry specified.** *Why:* every `wall` verdict on the two stuck maps cleared, and the flag reproduces the old world exactly; DR2 replaces the flag with the moving door. DSD's bf2_dea1 row moved again (it now finishes), so DSD measures against this baseline. *How to reverse:* `OPENBF_DOORS_SOLID=1`, or revert `bbf281a` and point the GATES block back at `lead_tmp/loop_baselines/20260925_0314_DET1/`.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue DR2, built and landed in one session; [REBAKE] maps + a DECLARED [SWEEP] re-baseline): BF2 doors are entities — they open by themselves.** `4921d3b` + crate map `5f92c19`; report `engine/artifacts/loop/reports/20260925_0400_build_land_DR2.md`. The disc's rule (`analysis/re-bf2-doors.md` §2 hop 5, §3 hops 7–9 [V][O]): a `door`-based class opens while a soldier is inside one of its ≤ 3 `AnimationTrigger` spheres, its parameter moving by `dt·30/(frames−1)` toward 1 (and back to 0 when nobody is), and its collision prims ride the bone the `open` clip animates. (1) `bake::door` (new) bakes each door class's rig into `ManifestClass::door` (additive, skipped when `None`, so only the five door maps' `manifest.ron` changed): the triggers' model-space hardpoints and, per moving prim, its `PRNT` bone's delta `B(f)·B(0)⁻¹` per clip frame. All 13 door classes rig; MUS1's hangar door reproduces the RE's 3.965 m to (0, 3.707, 1.409), the garage 3.444 m and POL1's leaves ±2.25 m. (2) `core::collision::CollisionWorld::with_movers` + `move_body`: a mover is filed in the broad phase under the envelope of every pose, so moving it inside that envelope needs no rebuild. (3) `core::door` + `Sim::doors`, ticked in `Sim::step` before movement and hashed only when non-empty (BF1 maps and fixtures hash as before). (4) `Scene::install_collision` at the six sim set-up sites; DR1's soldier-subset deviation is GONE.
  - **Gates reproduced:** ci 724 → **731** / 0 / 35 on the branch and on main (+7). Bake: the branch's full scratch bake DIFF = exactly the 5 door maps' `manifest.ron` (SAME 21,664); on main `rebake.sh maps`, then a scratch `maps` bake SAME=5322 DIFF=0, and the whole shipped tree SAME=21,669 DIFF=0 against the branch's bake. gpu_tests 0 on the branch and on main; `goldens.sh` moved nothing. Walkabout regenerated for the five door maps (the renderer does not read the rig). UNMOVED: `sim_gates.sh` GATES_RC=0 (`bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, motion.csv identical), `--sim-hash 100` `17e1b946ff0e821c`, both arenas (cmp-identical to DR1's files). **MOVED:** the sweep (branch ×2 and main ×1, all cmp 0) from DR1's 29/32, sum 107100 to **27/32, median 960 (960), >10k 3, sum 166020** — only the five door maps' rows: bf2_mus1 (team 1 won at 12590 → NO OUTCOME 70/13, stuck_max 540), bf2_dea1 (team 1 won at 10731 → NO OUTCOME, stuck_max 55680), bf2_tan1 (team 0 wins at 8809, was 9437), bf2_kam1 and bf2_pol1 still NO OUTCOME. **Attribution:** `OPENBF_DOORS_SOLID=1` (no door entity) reproduces DET1's pre-DR1 sweep file byte for byte, which also proves the movers' broad phase changes no collision answer. **The entry's gate:** the five §1 stuck positions clear — the census shows bf2_mus1 0 stuck and none of bf2_pol1's 3 at a door (all `off-graph [under-terrain]` at (93,13,−60), stuck before DR1 too); a scratch example (`lead_tmp/door-entity/tools/door_cross.rs`) walks one soldier stick-forward through each of the five §1 doorways: the live door costs **+4 to +7 ticks (0.13–0.23 s)** over the same door pinned open (≤ the clip's 1.00 s), and a door that never opens stops him exactly at the §1 stuck position. bf2_dea1's no-outcome is one unit on a `dea1_prop_grate` 27 m from any door (queued as STK with pol1's knot). Baselines `lead_tmp/loop_baselines/20260925_0355_DR2/`; the GATES block was rewritten (the sweep file, the footer, the ci count). `bake/src/map.rs` 1,598 → 1,617 lines (+19, the rig hook), already under SPL7. New entries DRV (the drawn door and its sounds) and STK (the three knots left).
  **DECIDED (loop, 2026-09-25, lane 1): accept the re-baseline — the port now shuts doors nobody stands near, as the disc does.** *Why:* DR1's always-open door was a stand-in; the real rule costs 0.13–0.23 s per doorway and the sweep's two lost finishes are not door walls (mus1 ends 70/13 with 0 stuck; dea1 on a grate). *How to reverse:* revert `4921d3b` and point the GATES block back at `lead_tmp/loop_baselines/20260925_0335_DR1/`.
  **DECIDED (loop, 2026-09-25, lane 1): the door's unread details, PROVISIONAL `[O]` in `core/src/door.rs`** — the trigger tests each live unit's FEET; the hardpoint sits where the clip's frame 0 puts it (not re-resolved through the animated model); a pose between two clip frames interpolates them (lerp / nlerp); the tick runs before movement; the shut pose is frame 0, not the `skel` rest. *Why:* each is the plainest reading the RE leaves open (`re-bf2-doors.md` §8), and none moved a gate beyond the declared sweep. Consequence noted [H]: `dea1_prop_door`'s 1 m trigger centred 1 m up is never entered by feet, and `dea1_prop_garbage_room_door`'s clip moves none of its prims, so both stay shut. *How to reverse:* edit `Door::occupied` / `bake::door::rig`.
  **DECIDED (loop, 2026-09-25, lane 1): `OPENBF_DOORS_SOLID=1` now means "no door ever opens"** (the pre-DR1 port, byte for byte) instead of DR1's "doors block soldiers". *Why:* it is the one-variable reversal of the whole door rule. *How to reverse:* `game/src/assets.rs` `collision_and_doors`.

- **2026-09-25 — LANDED (loop, lane 1, job C, queue DR3): BF2's `plnp` route patches apply — the blocked-group mask at 0 by default, the missions' start-of-round blocks behind `OPENBF_PLNP_MISSION=1`** (RE_LOG 2026-09-01 "`plnp`" [V]; the hops re-read this session: `FUN_002ec5f8` restores the current record, stores the mask at +0x2c and XORs the new record on; `FUN_002ec818` walks the patch per hub / five types / rows; the three Lua verbs are `|` `FUN_002ebe28`, `& ~` `FUN_002ebea8`, `^` `FUN_002ebda8`; the name → group lookup is `FUN_002ebcc0`; the mask's initial value is the constructor `FUN_002ead18`'s *(decompiler detail omitted)*, SWBFRNT2 decomp line 275526, and `FUN_002ec900` builds a fresh object per plan load). Build `5cbe3af`, crate map `2778eb8`. `core::route_patch` (new, 289 lines): the bake's `plnp_<key>.bin` attaches to a `NavGraph`, validated once against the table so a swap never half-applies; `set_blocked_groups` / `block_arcs` by lower-case fnv1a arc name. `layout::nav_graph` attaches it; `gen_missions.py` walks `ScriptPostLoad` and the script functions it calls (DEA1 unblocks through `PlayAnimExtend()`) and writes `arc_blocks` into 10 BF2 mission RONs (DEA1, KAM1, KAS2, MUS1, TAN1, both eras; the other 47 regenerate byte-identical); `MissionDef::apply_arc_blocks` replays them in `battle.rs` and the sweep example only with the flag. Tests (+5): MUS1 `Connection74` is plnp group 2 and blocking it gives the authored table XOR the disc record (36 rows, 264 entry bytes), unblocking restores it; all 10 baked base-world plnp sets attach and round-trip; every mission arc name resolves to a group. **Gates, reproduced on the branch and again on main:** ci **EXIT=0, 736 / 0 / 35**; `scripts/gpu_tests.sh` **0**, no golden moved; sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, both motion.csv identical; `--sim-hash 100` **`17e1b946ff0e821c`**; the sweep pair `cmp`-identical and identical to DR2's `sweep_main_1.txt` (27/32, sum 166020); both arena pairs identical, TAT2 identical to DET1's file. Nothing rebaked (the sidecars were already baked). `core/src/nav.rs` 1,693 → 1,722 (+29: the patch hooks and a `RouteTable::row` accessor; already under SPL7). **Measured with the flag on** (`lead_tmp/plnp-patches/gates/sweep_flag.txt`): only DEA1 (stuck_max 55,680 → 1,560, still no outcome), KAS2 (win at tick 10,939, was 16,549) and TAN1 (10,649, was 8,809) move; sum 166020 → 112980. New entry DR3B (the default, [SWEEP]).
  **DECIDED (loop, 2026-09-25, lane 1): the start-of-round route blocks stay OFF by default (`OPENBF_PLNP_MISSION=1` turns them on).** *Why:* the entry's gate is byte-identity at mask 0, and the port models none of the events that UNBLOCK those groups on the PS2 (KAS2's gate panel and wooden walls destroyed, DEA1's `grate01` killed), so turning them on shuts routes for the whole round that the disc reopens mid-round; DR3B makes that call with the measurement above. Also [H], not modelled: `FUN_002ec5f8` posts an event (`FUN_002b6258`, type 10) after the swap — presumably the units' re-plan; the port's units re-read the table at their next chooser step. *How to reverse:* `MissionDef::apply_arc_blocks` (drop the env test).
- **2026-09-25 — LANDED (loop, lane 1, job C, queue DR3B): the missions' start-of-round `plnp` route blocks ON by default** (`OPENBF_PLNP_MISSION=0` turns them off; DR3's mechanism, RE_LOG 2026-09-01 "`plnp`" [V]). Build `cf5b23b`: `MissionDef::apply_arc_blocks` tests `== "0"` instead of `!= "1"`; the env doc in `main.rs`, the comments and the two log lines (`route groups … blocked (plnp)`). **Gates, reproduced on the branch and again on main:** ci **EXIT=0, 736 / 0 / 35** (no test added — the replay is DR3's tested path); `scripts/gpu_tests.sh` **0**, no golden moved; sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, motion.csv identical; `--sim-hash 100` **`17e1b946ff0e821c`**; both arena pairs byte-identical to the GATES block's files (the arena applies no mission). **DECLARED re-baseline, the sweep:** the pair `cmp`-identical, branch == main, and byte-identical to DR3's measured flag-on run; only bf2_dea1 (stuck_max 55,680 → 1,560, still no outcome, tickets 24/16), bf2_kas2 (win 16,549 → 10,939) and bf2_tan1 (8,809 → 10,649) moved; footer `finished 27/32 … >10k 2 maps  sum 112980` (was 3 maps, 166020). `OPENBF_PLNP_MISSION=0` reproduces DR2's `sweep_main_1.txt` byte for byte. `STUCK_CENSUS=1`: DEA1's `wall` knot at the grate (unit 29, STK's first knot) 1 → 0; POL1's three under-terrain units and BF1 KAS2's one unchanged. Baselines `lead_tmp/loop_baselines/20260925_0450_DR3B/`; the GATES block rewritten.
  **DECIDED (loop, 2026-09-25, lane 1): option (a) — the blocks go on by default, with no unblock events.** *Why:* (b) is impossible today — the sim has no destructible objects (no `grate01`, `gatepanel` or wall death to hook; `grep -rn destruct` over `core/src` and `game/src` finds nothing), and the port's barriers are solid, indestructible collision for the whole round (DR2's census found DEA1's no-outcome unit pinned 0.6 m from a `dea1_prop_grate`). So the PS2's pre-destruction state — barrier up, route group blocked — is the only state the port ever reaches, and blocking keeps the planner's graph consistent with the world; (c) would leave units routing into walls the port never removes. When destructibles exist, their death events must call `NavGraph::block_arcs(.., false)` (the scripts' `Unblock…` calls). *How to reverse:* `OPENBF_PLNP_MISSION=0`, or flip the test in `MissionDef::apply_arc_blocks`. Supersedes DR3's "off by default" call.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue DRV): the drawn door and its sounds follow the sim's door** (`analysis/re-bf2-doors.md` §2 hop 5, §3 hop 9 [V][O]). Build `5eeaab0`, crate map `7ebeb48`. **Bake** (`bake::door`, [REBAKE] maps): a census of the 13 rigged classes' models found every segment hard-bound by `BNAM` and none `bone_space` or soft, so each drawn submesh (GLB primitive = non-empty segment, in order) whose bone the clip moves gets that bone's `D(f)` — the prims' own rule — as `ManifestDoor::parts`; the ODF's `OpenSound` / `CloseSound` (fnv `0x323F7450` / `0x4FCF4562`, a test pins both) ride along; all additive and skipped when empty. All 26 names resolve in their own map's sound bank (`door_*` ×9 classes, `big_door_*` DEA1 blast + KAM1 podroom, `blast_door_*` TAN1, `window_*` MUS1). **Sim** (`core::door`): the disc's sound latch — bit 0x21 of `this+0x5f2`, `FUN_001ebb80` B2:99659–99687, read this session with its enclosing function checked — fires on the TRIGGER's edges (first occupied tick → `OpenSound`, first empty tick → `CloseSound`), not on the parameter leaving/reaching 0 as the entry said; `Sim::tick_doors` pushes `Event::DoorOpened` / `DoorClosed`. The latch and the events are output-side and NOT hashed (a test flips the latch and compares hashes). `Door` carries its instance, origin and sound names as setup data; `door::sample` is the frame interpolation the bodies and the draw share. **Game:** `SceneGpu::pose_doors` sets each moving submesh's draws (base + specular) to `instance · D(open)`, called by the windowed loop, the replay captures and the bench — never by the goldens or the walkabout, which still draw every door shut; the windowed game plays the sounds at the door; the replay's events CSV gets `door_opened` / `door_closed` rows. **Gates, reproduced on the branch and again on main:** ci **EXIT=0, 736 → 738 / 0 / 35** (+2: the latch's edges, the sound keys); `scripts/gpu_tests.sh` **0** on both, `goldens.sh` on main moved nothing; the branch's full scratch bake DIFF = exactly the 5 door maps' `manifest.ron` (SAME 21,664); on main `rebake.sh maps`, a scratch `maps` bake SAME=5322 DIFF=0, the whole shipped tree SAME=21,669 DIFF=0 against the branch bake. UNMOVED on both: sim_gates `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, motion.csv identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp` 0 and `cmp` 0 against DR3B's `sweep_main_1.txt` (27/32, sum 112980); both arenas identical to the GATES block's files. Walkabout not regenerated — it never poses a door, so its pictures cannot move. **Captures** (scratch `lead_tmp/door-visual/tools/drv_door.rs`, the sim's own `Door::advance` with a unit's feet in the trigger, the reference soldier planted there): MUS1 `hangardoor` shut / 15 ticks (open 0.5) / 30 ticks (1.0) — the panel slides up its tilted frame and the corridor shows behind (`cap/sheet_hangar_z8.png`); `door_cont_3` from both sides (`cap/sheet_cont3.png`); KAM1 `fdl-1` (`cap/sheet_blast_kam.png`). **Found:** DEA1's `dea1_prop_door_blast` — its bones rest at x = ±5 and the −x leaves hang off `bone_rightdoors` (+x), so read as model space (the port's reading for the prims since DR2 and now the drawn leaves) the shut door has a 1.4 m slit and opening slides the leaves ACROSS the doorway; read in the bone's rest frame they meet on a diagonal seam and slide apart. MUS1's tri door (rest rotated 20.8°) fits its frame only in model space. New [RE] entry **BNR**.
  **DECIDED (loop, 2026-09-25, lane 1): the drawn door moves with the collision door's rule, DEA1's blast door included** — *why:* what a soldier sees then matches what blocks him (DR2 already slides that door's prims across the doorway), the rest-placement question is shared by both and needs RE (BNR), and fixing only the picture would split them; *how to reverse:* stop calling `SceneGpu::pose_doors` (four call sites) or drop `ManifestDoor::parts` for the class.
- **2026-09-25 — MEASURED (loop, lane 1, job C, queue STK; no code): POL1's last stuck knot is an unreachable defend hint.** Report `engine/artifacts/loop/reports/20260925_0600_measure_STK.md`, evidence `lead_tmp/stk-census/` (a read-only trace of the sweep's own round: the bf2_pol1 row and hash `98ea331aa00a84d5` reproduced byte for byte). DEA1's grate knot had already gone with DR3B. POL1 units 2, 3, 6 (unscoped riflemen defending cp6con, floor 13.1) take Cover `hintnode153`, 8.5 m ABOVE the post — `commander::nearest_hint` picks the nearest usable hint within 25 m by 3D distance with no level test, and `ai::route` writes it into the last leg as a straight line from the post's hub. Each unit climbs the ledge at z ≈ −64, slides west and drops into `pol1_prop_crate_stack_14`'s cluster, where the disc's barrier 81 contains it at `DETOUR_RADIUS` and is ignored for the leg (FUN_00268f68's end-point rule), so the wall probe holds it for the rest of the round (53,520 / 42,720 / 28,500 ticks). Two same-floor Cover hints were in range. Across the maps, 65 of 174 posts with a hint in range have one more than 3 m off the post's level. The census's `under-terrain` flag is a label on POL1 (the asteroid heightfield is 33 m over the interior floor), not a cause. New entry **HDL** [SWEEP]: a defender's hint must stand on its post's level.
  **DECIDED (loop, 2026-09-25, lane 1): STK closes with ONE entry (HDL), not one per symptom** — *why:* the ledge climb, the drop and the pocket all follow from the hint pick in all three traces; *how to reverse:* if HDL's census still shows a unit boxed in a prop cluster, queue the pocket on its own.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue STK2): the two knots HDL left, measured — and a hunter's last leg now walks at the enemy, not its hub** (`ai::route::HUNT_AT_ENEMY`, PROVISIONAL — a port rule under the commander's Hunt, which is itself the port's design, D6). Build `9beb056` (`core/src/ai/route.rs`, +1 test that fails with the flag off). **The trace** (seed 42, the sweep's round, `lead_tmp/stk2-census/`, scratch `stk_trace` reproducing the sweep rows' hashes): (1) **BF1 YAV1** — team-0's last unit (8) had walked off the `yav_bldg_aquaduct` deck at T10380–10500 (a re-plan on arc 110 past its midpoint starts from hub 83 and walks straight at hub 84, 17.5 m off the deck's corner) and slid for the rest of the round between the piers 10 m below; team-1's last hunter (24) reached hub 83 — the quarry's hub by XZ locate — and stood on it, because a Hunt's last leg aimed at the HUB; units 18/22 sat in two barriers' margins near hub 17. (2) **BF2 POL1 unit 1** — HDL's same-level hint `hintnode159` is 3.8 m from `crate_stack_14`; the Defend capture-volume flap re-plans leg 32→35 through the stack from there and `leg_ignores` drops barrier 81 (the unit stands in its `DETOUR_RADIUS` margin); the stuck recovery re-plans the identical leg. **Gates, branch and main:** ci **EXIT=0, 740 / 0 / 35**; `scripts/gpu_tests.sh` **0**, `goldens.sh` nothing moved; sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, motion.csv identical; `--sim-hash 100` **`17e1b946ff0e821c`**; both arena pairs byte-identical to HDL's files. **DECLARED re-baseline, the sweep:** pairs `cmp` 0, main == branch == the scratch experiment; 14 rows moved (bf1 bes2/kam1/nab1 hash only, bf1 end1/hot1/rhn1/yav1, bf2 end1/geo1/kam1/kas2/nab2/tan1, tat1); footer `finished 29/32  stuck_max median 960 (finished-only 840)  >10k 2 maps  sum 84240` (was 27/32, 900/780, 3 maps, 118560) — BF1 YAV1 NO OUTCOME → team 1 wins 28,168, BF2 KAM1 NO OUTCOME → team 0 wins 23,349, BF2 TAN1's winner flips (team 0 at 11,225 → team 1 at 9,808). `STUCK_CENSUS=1`: YAV1 1 → 0; POL1 unit 1 and BF1 KAS2 unit 7 unchanged. Baselines `lead_tmp/loop_baselines/20260925_0557_STK2/` (the clock stamp sorts before HDL's label; it is the later); GATES block rewritten. **New entries:** BPO ([RE] first — the PS2's barrier push-out `FUN_002689d8`, RE_LOG:178, for a unit in a barrier's margin), AQD (BF1's 3-D hub locate, RE_LOG "Arrival and locate [V]", so no unit walks off YAV1's aqueduct deck), CEN (the census misses a unit that slides in place — three of YAV1's four knots read `stuck_run` ≤ 1,080). Report `engine/artifacts/loop/reports/20260925_0620_build_land_STK2.md`.
  **DECIDED (loop, 2026-09-25, lane 1): build the Hunt rule inside STK2 and land it as a re-baseline** — *why:* the entry allowed one small rule; the traced cause of YAV1's NO OUTCOME is the hunter's waypoint, the rule is four lines, and the measured sweep improves (2 more maps finish, sum −34,320); the other two mechanisms are not small and have their own entries; *how to reverse:* `ai::route::HUNT_AT_ENEMY = false` (the only code path added — gives back HDL's sweep, derived, not measured).
- **2026-09-25 — LANDED (loop, lane 1, job C, queue DSD): Death Star conquest takes a second ticket from team 1 per team-1 death.** Build `aed8710` (`analysis/re-time-limit.md` §6 [V][O]): `scripts/gen_missions.py` now reads `ScriptPostLoad`'s `OnCharacterDeathTeam(handler, t)` whose handler is straight-line code whose one call is a literal `AddReinforcements(team, -n)` into a new RON field `death_drains: [(on, team, tickets)]` (port teams) — regenerating wrote ONLY `bf2_dea1_con.ron` and `bf2_dea1_con_cw.ron` (every other RON byte-identical; a pre-change regeneration was clean); `game::mission::MissionDef::apply_death_drains` loads it into `core::battle::Battle::death_drain[dying][paying]`, which `Battle::on_death` spends on top of its own ticket, floored at 0 (PROVISIONAL [O]: the filter's subject [D], the floor [H]); both setups apply it (`make_walk`, the sweep). Not in `Sim::hash` (fixed setup; what it spends lands in `reinforcements`). A drain can leave the paying team below its live count for one tick; the next `tick_bleed` sheds the AI, as a bleed does. **Gates, reproduced on the branch and on main:** ci **EXIT=0, 742 / 0 / 35** (+2 tests); `scripts/gpu_tests.sh` 0, no golden moved; sim_gates (walk `bb860216c775e788`, aim `b489b1b3aa14084f`, 600 `fb19c1d2c811788b`, motion identical), `--sim-hash 100` `17e1b946ff0e821c`, both arena tables byte-identical to the STK2 baseline files; the sweep pair `cmp` 0, main == branch, and against STK2's baseline ONLY bf2_dea1's row moved, as declared: `tickets 60/16 alive 16/16 y 50..52 … stuck_max 1260 hash 31cb8d35c6bc385c` → `tickets 10/16 alive 10/16 y 50..58 … stuck_max 1560 hash a8908fdf9582b0f7`, still NO OUTCOME; footer `finished 29/32 … sum 84240` → `sum 84540`. (The entry's "today team 1 wins at tick 33945" predated DR3B/HDL/STK2's re-baselines; the row it moved was already NO OUTCOME.) DECLARED re-baseline: `lead_tmp/loop_baselines/20260925_0640_DSD/`, GATES block rewritten. No rebake (the RONs are read at runtime). Report `engine/artifacts/loop/reports/20260925_0640_build_land_DSD.md`.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue TLO): the Instant Action conquest options, the TIME LIMIT (Off, 5..60 min) and the reinforcement PERCENTAGE (10–500 %).** Build `e76156a` (`analysis/re-time-limit.md` §1 hop 9 and §5 [V][O]): `Sim::time_limit: Option<u32>` seconds (None = Off, the default; an INPUT, not hashed) — `update_cps` checks it after the 0-ticket and domination rules, and the step that brings `tick` to `limit × 30` gives the round to the team with more tickets, or `OUTCOME_DRAW` = 2 on equal counts (`MissionVictory({1,2})`: both win), with `Event::TimeUp`; `core::sim::scale_reinforcements` = `count * pct / 100` in integers. The shell's "Reinforcements" row is now the % (showing both scaled counts) and a "Time limit" row follows it (the disc's clamps); a shell round takes the MISSION's own per-team counts scaled by the %, no longer team 1's count for both. HUD: an m:ss countdown under the tickets while a limit is set, and a "TIME UP — BOTH TEAMS WIN" banner; `game --time-limit <min>`; a draw counts as a win in the career, the events CSV logs `time_up`. **Gates, reproduced on the branch and on main:** ci **EXIT=0, 745 / 0 / 35** (+3 tests in `core/tests/battle_outcome.rs`: expiry with more / fewer / equal tickets → 0 / 1 / draw at exactly tick 60 of a 2 s limit; Off and an unexpired 1 h limit never end a round and hash alike; the % at 10/100/500 on 150 and 220 → 15/150/750 and 22/220/1100); sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, motion identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp` 0 and `cmp` DSD's baseline 0 (footer `finished 29/32 … sum 84540`); both arena pairs identical to the baseline files; main == branch on every file. **The entry's scratch round** (a temporary, uncommitted `TLO_LIMIT` hook in `round_to_outcome`, seed 42): bf2_tan1 with a 300 s limit ends at **tick 9000 (300 s), team 1 wins, tickets 11 / 17**; without it the row is the baseline's (`team 1 wins at tick 9808`; the entry's "NO OUTCOME" for tan1 predated the DR/HDL/STK2 re-baselines). **Goldens:** only `shell.png` moved (mean abs 4.891, max 232, 6.53 % of pixels, bbox 340,120–939,600: the new row, the % text, the panel one row taller); the regenerated file is byte-identical to the viewed branch candidate; `scripts/gpu_tests.sh` 0 after. Deviations: four files outside the entry's list — `core/src/sim.rs` (the exports), `game/src/main/headless.rs` (the draw in the hash line, the CSV row), `game/tests/golden_{shell,hud}.rs` (the models' new fields). Report `engine/artifacts/loop/reports/20260925_0710_build_land_TLO.md`.
  **DECIDED (loop, 2026-09-25, lane 1): TLO lands with the `shell.png` golden move the entry did not declare.** *Why:* the entry's own build adds a shell row, so the shell golden must move; the move is exactly that row and the % text (sheet `engine/artifacts/lead_tmp/time-limit/sheet/out/shell.png`), nothing else moved. *How to reverse:* revert `939ed21` with `e76156a`.
- **2026-09-25 — PXC batch 1 (loop, lane 1, job B3 at max): the PCSX2 rig reaches a level on both games, and every save state gives the camera pose.** No tracked code (everything lives under the gitignored `engine/artifacts/pcsx2/`). Boot → menus → in level, unattended, on BF1 Kashyyyk: Docks and BF2 Kashyyyk — the key paths and the menu rule (≥ 1.1 s between presses, or both games drop every other one) are in `engine/artifacts/pcsx2/README.md`; save states at side select, the spawn map and the spawns (`ref/states/`). A PCSX2 state is a zstd zip holding the whole EE RAM and its own 640×480 frame: BF1's active camera is `*DAT_00522ac8` (`FUN_0049c3d0`, BF1 decomp 561582; matrix rows right / up / back / eye at +0, projection at +0x80), BF2's `DAT_0060c240` (`FUN_00582c60`; matrix at +0x30, projection at +0x130 — the offsets `analysis/re-sky-pass.md` names); both games' RAM coordinates are the port's (BF1 KAS2 cp11 and BF2 KAS2's four posts read with the port's own signs, no flip) — `pcsx2/pose.py`. An untracked scratch example (`pcsx2/tools/pxc_pose.rs`, built in a detached scratch worktree, never committed) renders the port at a state's pose and FOV, and the pairs line up to a few pixels. Findings (`engine/artifacts/pcsx2/ref/FINDINGS.md`, each with both PNG paths): F1 BF1 KAS2's water light green-teal and see-through on the PS2, dark warm grey in the port; F2 no red band on the PS2 at four dock poses where the port draws 4 grey-white rows over 4–5 rows of (255,0,0) — KASF's question (a), for those poses; F3 / F7 the PS2's sun glare and BF2 KAS2's near-white horizon haze; F5 the PS2 soldier camera is 48.0° × 37.69° (BF1) and 65.0° × 52.06° (BF2), h × v, zoom 1, against the 70° vertical `WalkSim::camera` takes from `Camera::default`; F6 BF1 KAS2's sand slope behind Dock Two is grass in the port; F8 BF2 KAS2's sea is lighter. Queued CFOV, WCOL, SUNG, KTEX ([RE] first) and a KASF note; HANDOFF HOLD 7 and What next 2(a) rewritten (the in-level step is built). Report `engine/artifacts/loop/reports/20260925_0945_b3_PXC.md`.
- **2026-09-25 — PXC batch 2 (loop, lane 1, job B3 at max): the PCSX2 library covers nine water maps, and the pairs find eight new defect families.** No tracked code (everything under the gitignored `engine/artifacts/pcsx2/`; the pose renderer stays an untracked scratch example in a detached worktree, removed). Captured with poses, port renders at the PS2's exact pose and FOV, and pairs: BF1 KAM1 (2 posts), YAV1 (2), NAB2, KAS1, KAS2's high ground (Republic side — `Right K` proven), BF2 KAM1, NAB2, DAG1, FEL1, plus BF2 MYG1 and Space Kashyyyk from two menu miscounts (BF2's list holds Mos Eisley and the space maps). 63 new pairs; menu states for both games (`ref/states/bf1_menu_iamaps.p2s`, `bf2_menu_iaplanets.p2s`) so a map costs one load. **The instrument was checked first**: PCSX2 warns "Blending Accuracy is Basic, recommended High", so F1's and F7's states were re-rendered under HW Basic, HW High and the SOFTWARE renderer — water and haze patches agree within 4 levels (frame mean abs 1.8–3.2). Findings (`engine/artifacts/pcsx2/ref/FINDINGS.md` F9–F23, both PNG paths each, read under the maintainer's new `ref/COMPARISON_RULES.md`): BF1 KAM1's sea is dark teal under foam crests on the PS2 and a flat textureless slate plate in the port, the y = 0 terrain `re-water-alpha.md` §5 left open (F9 → KOC); a dark near-camera wall in the port where the PS2 sees through — the port culls nothing (`render/src/pipelines.rs` 528–529 `cull_mode: None`, the only one), BF1's words carry no two-sided flag (F10, F23 → BFC); rain and lightning on Kamino (F11 → WTHR); instances the port skips as "no drawable model" that the PS2 draws — the CP console, Theed's shrubs and vines, Yavin's grass (F12 → SKIPC); YAV1's flooded bazaar and BF2 KAM1's ocean green-teal on the PS2, neutral in the port (F13, F18 → WCOL notes); **BF1 KAS1's lake mirrors the sky** — every sky shell of BF1 KAS1 and KAS2 is authored `Reflect` (the hash 0x92c778aa sits on `kas1_sky_dome` / `kas1_mountains` / `kas_moon` and on KAS2's two shells, and nowhere else on either disc), the draw `re-sky-dome-placement.md` §8 said to ignore until a map needs it (F16 → REFL); bare terrain where the PS2 carpets the ground with plants — Dagobah, Felucia, BF1 KAS1 / KAS2 / YAV1 — and the port parses no `FOLG` (F22 → FOLG); terrain layer colours on four more maps (F14, F17, F21 → KTEX note); the port ~60 % brighter on BF2 MYG1 and on Theed's shaded stone (F15, F20 → LITE); a specular hot spot on a BF2 KAM1 pad (F19 → SPHOT); 0 red pixels on BF1 KAS2's high ground in both, but the bamboo hides the horizon, so KASF's "never red" stays [H]. The §5 verifier (Opus, foreground, claims and citations only) SUPPORTED all ten claims and sharpened two: the `Reflect` hits parsed as DomeModel properties, and BF2's `FOLG` chunks are only 2–8 KB. Report `engine/artifacts/loop/reports/20260925_1130_b3_PXC.md`.
  **DECIDED (loop, 2026-09-25, lane 1): the reference library keeps PCSX2's default hardware renderer at Basic blending, not High or the software renderer.** *Why:* the three agree within 4 levels on the water and haze patches the findings rest on (FINDINGS "Batch 2", `engine/artifacts/lead_tmp/re/pxc2/ctl/`), and HW Basic is the fastest. *Reverse:* set `accurate_blending_unit = 3` or `Renderer = 13` in `~/.config/PCSX2/inis/PCSX2.ini` (the pre-batch copy is `lead_tmp/re/pxc2/PCSX2.ini.before_pxc2`) and repeat the control on any finding that turns on a blend mode.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue CLN5): the `LOWR` far sheet's flag-gated draw path, four defects.** Build `50285f8`, behind `OPENBF_FAR_CULL=1` + `OPENBF_LOWRES=1` (both still OFF by default). (1) `viewer::lowres_range` returns no sheet on a `FarSceneRange(0)` map: BF2 END1 had drawn its sheet through the fade pipeline, invisible but depth-writing; `re-fog.md` §2.5 [V] (the far-buffer composite is skipped) and re-lowr.md §5's own list of maps without a drawn sheet agree, so no new RE was needed [D]. (2) New `render::DrawLayer::FarSheet` / `fs_far_sheet`: UNLIT (the disc's base colour 0x80808080), fogged, own pipeline, sorted after Mask. (3) The "every sheet texture is alpha 255" census had read the BAKED files, which the bake forced opaque. On the DISC (`lead_tmp/cln5-lowr/sheet_alpha_disc.txt`, every `tex_` via `mungefs`) BF1 KAS1 `kashyyyk1` has 62,571 / 65,536 mip-0 texels below 16 and BF1 KAS2 `kas2` 27,482; the other twelve sheets are 255 everywhere. `bake::map` now keeps the named sheet texture's alpha (`keep_alpha`'s `water_named` became `consumer_named`), and the pass tests at the AREF 0x10 composed to disc A ≥ 32 (PROVISIONAL [D]). (4) TAT1's 3000 m MaxDistance stays capped by the camera's 2000 m far plane — recorded in `lowres_range`, not changed. `re-lowr.md` §5 corrected. **Gates, reproduced on the branch and on main:** ci **EXIT=0, 745 / 0 / 35** both; `scripts/gpu_tests.sh` 0 both; `goldens.sh` on main: NO golden moved; branch scratch bake `tat1 maps` SAME=5452 **DIFF=2** — exactly the two KTX2s, as declared here; `rebake.sh maps` on main, then a main scratch bake `maps` SAME=5322 DIFF=0; walkabout regenerated for bf1_kas1/bf1_kas2 (flag off): all 15 images byte-identical in RGB. No sim code touched. Flag-ON before/after sheets (`lead_tmp/cln5-lowr/sheets/`, viewed): END1 identical (the invisible sheet hid nothing at the 7 poses); TAT1 and BF1 HOT1 lose the lit tint (vantage 51 % / 28 % of pixels); KAS1 and KAS2 gain holes over their water. **Through those holes the port shows the CLEAR colour** (black on KAS1, KAS2's red fog colour), where the PS2 shows its far water pass (re-lowr.md §2.3): one more thing `OPENBF_FAR_CULL`'s default waits on (HANDOFF §What next 2(b)). `bake/src/map.rs` is now 1,630 lines (SPL7). Report `engine/artifacts/loop/reports/20260925_1200_build_land_CLN5.md`.
  **DECIDED (loop, 2026-09-25, lane 1): CLN5's two open calls — no sheet on a `FarSceneRange(0)` map, and TAT1's far-plane cap kept and recorded.** *Why:* (1) is the reading both analysis docs already support; (4) raising every camera's far plane would change what the default renders past 2000 m, for a sheet that is off by default. *Reverse:* (1) drop the `far_scene_range == Some(0.0)` early return in `lowres_range`; (4) give `render::Camera` the map's max(MaxDistance, FarSceneRange) as `far`.
- **2026-09-25 — PXC batch 3 (loop, lane 1; captures at max, interrupted after its FINDINGS/README at 13:03; the queueing and report finished by the next session, job A at medium).** Every one of the port's 32 maps now has a PCSX2 spawn post, three turned views and a pitched-up view with port pairs (`engine/artifacts/pcsx2/ref/README.md`'s table); the port's `tat2` is BF2's Mos Eisley, so the BF1 capture is refiled as `bf1_tat2_mos_eisley/`. FINDINGS F24–F33: **LEAF** (new, [RE]) — the PS2 hangs `leafpatch` sprite clusters on trees' branch hardpoints (`AttachOdf`/`AttachToHardpoint`; both ELFs register the class — BF2 decomp 16469–16470, string 0x60fd08 re-read from the ELF), the port draws bare branches (BF2 YAV1 canopy box PS2 (24,31,8) vs port (136,141,143), reproduced); **GLOW** (new, [RE]) — BF2 Utapau's panels, Coruscant's windows and Mustafar's lava are self-lit on the PS2 and dark in the port; notes on WCOL (three more flat waters: BF1 NAB2, BF2 FEL1, BF2 YAV1), KASF (Beach Head: 0 red pixels on the PS2 in 5 poses, the port up to 5042), LITE (a BF2 effect: port/PS2 luminance median 1.30 over 17 BF2 maps, 0.93 over 15 BF1 — a crude box census [D]), FOLG (both Endors' fern carpets), SUNG (two more glare frames), BFC (BF1 BES2's camera inside a mesh); F31 confirms CORB's call on the PS2. Batch 3's own [V] RE reads (the leafpatch properties, the factories) have not been through a §5 verifier — LEAF's RE session owes it. No code, no gate run or moved. Report `engine/artifacts/loop/reports/20260925_1415_a_PXC3.md`.
- **2026-09-25 — WCOL's [RE] half (loop, lane 1, job D at max): the flat water's teal is a VU1 view-angle term the port lacks.** `analysis/re-water-colour.md` (pointer `re-water.md` §3c, RE_LOG entry), `[O]` UNAUDITED. The "microcode 0x515410" is a DMA chain that uploads a 63-instruction VU1 overlay (BF1 VU 0x74e, BF2 0x758, word-identical); its routine B colours the MAIN layer per vertex, **unlit**: LUT colour + `SpecularColor`·F and LUT alpha + `SpecularColor.a`·F, F = (1 − max(0, V̂.y))³ with V to the eye shifted 10 m along the light's horizontal direction; the LUT runs Min → 2·Max − Min over the DIVU-picked indices 5..15, so its mean is MaxDiffuse, not the port's midpoint; routine A gives the speckle `SpeckleAmbient + SpeckleSpecular·lobe³²` around the sun's azimuth (the port's is flat — its white mottle); the ocean's vertex RGB is the engine's global white halved = unity (HANDOFF What next 2(d) answered); a reversed-scroll second pass is dead (flags 0, no writer). **Measured on the PS2 itself** — save states with the `Effect("Water")` bytes patched in EE RAM, booted in the isolated PCSX2 rig: zeroing `SpecularColor` removes +13..+27 G/B levels, the teal body, on BF1 KAS2 / NAB2 / BF2 FEL1; the port's own model run on the PS2 draws the port's grey mottle. The port taken apart at the same poses with a scratch instrument (a detached worktree, never main; the no-knob render equals the library frames byte for byte) moves the same way. The rest of the gap is the **bed** under the water (KAS2 sand vs the port's grass — KTEX; NAB2/FEL1 beds 2–2.5× too bright in the port — LITE). **Side finding: the port's baked sun is not the PS2's** — BF1 (−L.z, L.y, L.x) of `FUN_004931e0`'s vector from the same `SunInfo.Angle` (all 14 BF1 maps with states), BF2 (L.x, L.y, −L.z) of its global light (16 maps); the PS2's sun glare sits where its own vector says. One foreground Opus verifier: 11 SUPPORTED, 2 PARTLY (folded in), 0 NOT SUPPORTED. Queued **WCOLB** (the build, [GOLDENS] the 11 wet-map goldens) and **SUND** ([RE] first; relights every map); notes on KTEX, LITE, SUNG. No code, no gate run or moved. Report `engine/artifacts/loop/reports/20260925_1545_re_WCOL.md`.


# `git show bdbacd9:HANDOFF-OPUS.md` is the page before the 2026-09-24 consolidation, `7498317` before 2026-09-23's)

This file is the brief every **Opus** loop session reads. **Since 2026-09-14 (the maintainer) the autoloop is the project's
lead** — its sessions land, rebake, regenerate goldens, refill the queue and make the calls (§Rules; the standing
prompt is `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md`). **Since 2026-09-17 (the maintainer) they also do the
reverse-engineering themselves: Fable is OFF on this project** (his Fable budget goes to his day job), so §HOLD FOR
FABLE is simply the RE list, and `OPUS_LOOP_PROMPT.md` §5's four self-checks plus an Opus verifier agent replace the
Fable audit. **Since 2026-09-23 (the maintainer) the loop may also boot a disc in PCSX2 and screenshot it itself**
(`engine/artifacts/pcsx2/`, §HOLD 7).


## For the maintainer — calls the loop made without you, and what only you can supply

- DECIDED 2026-09-25 lane 1 (DR2, `reports/20260925_0400_build_land_DR2.md`): BF2 doors are real entities now —
  shut until a unit's feet enter the trigger, then the box rides the `open` clip (+0.13–0.23 s per doorway against
  a door pinned open); the sweep re-baselined 29/32 → 27/32 (mus1 and dea1 no longer finish; neither is a door wall).
  The unread details are PROVISIONAL `[O]` (feet, frame-0 hardpoint, lerp between frames); two DEA1 door classes
  therefore never open [H]. The drawn door followed in DRV. `OPENBF_DOORS_SOLID=1` = no door ever opens.
  Reverse: revert `4921d3b`, GATES back to `loop_baselines/20260925_0335_DR1/`.
- DECIDED 2026-09-25 lane 1 (SPEC, `reports/20260925_0100_build_land_SPEC.md`): the Specular second pass is on —
  crisp glints on DEA1's panel edges, a broad sun-tinted glow over BF1 KAM1's deck and domes (mean +5 on its golden,
  half the pixels), one bright dome sheen on BF2 KAM1 — accepted as the `[O]` recipe applied as written. Sheets:
  `lead_tmp/spec-pass/pairs_0{0..3}.png`, `lead_tmp/spec-pass/sheets/`. Reverse with `OPENBF_SPEC=0` (A/B in play) or by
  reverting `e58ede7` + `d99e15c`; `OPENBF_SPEC_MIRROR=0` shows the consistent light in place of the disc's mirrored one.
  An in-map PCSX2 capture of Kamino would settle how bright the deck really is.
- 2026-09-24 lane 1 (item 9, `reports/20260924_2130_build_land_9.md`): COR1's sky-lane strips were an alpha-TESTED
  card of cars (BF2 0x1000002), not additive — now cut to the cars, the white bands gone; the same disc rule cuts
  MUS1's lava falls where their crust alpha fails the test (gaps in the fall) — `lead_tmp/cor1-sky-lanes/pairs_00.png`
  and `sheets/bf2_cor1_cp1_cp2.png`. Two DECIDED in PLAN's Log (the Specular pass split off as SPEC; the fix is general
  and the lava's cut is accepted, reverse by dropping `disc_alpha_tests` in `bake::alpha::verdict`).
- 2026-09-23 lane 1 (WA, `reports/20260923_1905_land_WA.md`): the water on 12 maps is translucent by its own sheet's
  alpha — bays, ponds and swamps show their beds, BF2 Kamino's sea gains its foam layer.
  `lead_tmp/water-sheet-alpha/land/walk_sheets/bf1_yav1__cp5_bazaar.png` (+ `bf2_dag1__cp0_cp2`, `bf2_kam1__vantage`),
  the goldens `…/land/pairs_0{0..3}.png`. BF1 Kamino's visible "sea" is the TERRAIN (`re-water-alpha.md` §5, [H]).
  Reverse: the `water_named` clause in `bake::map::bake_content`, `rebake.sh maps`, revert `7409964`.

## What next — value order (rewritten 2026-09-24 by the cleanse, lane 1)
0. **The loop runs the project** (the maintainer, 2026-09-14), all Opus since 2026-09-17. The maintainer starts it himself, from his own
   terminal (`engine/artifacts/loop/START_HERE.md` has the commands); no session ever launches `autoloop.sh`. Each
   session does ONE job (`OPUS_LOOP_PROMPT.md` §2). **The queue, its order and every entry's status live in
   `engine/artifacts/loop/QUEUE.md`, and the gate VALUES only in its GATES block** — this page restates neither (it
   went stale twice doing so).
1. **Next, in queue order** (SKO and item 9 landed, TL answered — `analysis/re-time-limit.md` — and SPEC landed
   2026-09-25 on its RE, `analysis/re-specular-pass.md`, SPEC2 — the same pass on soldiers and weapons — right after,
   then W3, BF1's water speckle, and WSE — RE and build — 2026-09-25, `analysis/re-water-flags.md`, and item 7, the walkabout statistics referee, and CLN3, the `/tmp` scratch leak, and CLN4, the sweeper's structural bake walk, and DET1, the spawn yaw off the host libm — a re-baseline, and DR1, BF2 doors open to soldiers — another, and DR2, the real door entity — a third, then the fourth cleanse, and DR3, `plnp` route patches applied at mask 0, and DR3B, the missions' start-of-round route blocks ON by default — a re-baseline, and DRV, the drawn door and its sounds): CORB, the RE entries KASF / MUSS / UTAS / TCC32 (TL's follow-ons have both landed 2026-09-25: DSD, DEA1's second ticket per team-1 death — a re-baseline, DEA1 still no outcome — and TLO, the Instant Action conquest timer + percentage rows, Off / 100 % by default, no sim gate moved, `shell.png` moved), the cleanse rows CLN6, CLN7 and CLN9
   (CLN5, the `LOWR` sheet's flag-gated draw path, LANDED 2026-09-25 — CLN6 the libm referee's blind spots, CLN7 two game referees that
   test the wrong thing, CLN9 KAM1's untested foam routing and its wrapping clock) and SPL7 (the four files just over
   1,500 lines, split or waived — the six earlier splits' lessons are `engine/artifacts/briefs/SPLIT_RECIPE.md`
   §10–§15), then HDL (a defender's hint must stand on its post's level) and STK2 (a hunter's last leg walks at the enemy —
   both re-baselines) — STK2's three follow-ons: BPO ([RE] first: the PS2's barrier push-out `FUN_002689d8` for a unit
   standing in a barrier's margin — POL1 unit 1's knot), AQD (BF1's 3-D locate, so YAV1's units stop walking off the
   aqueduct deck; after BPO, same files); CEN (the census's no-progress line) landed 2026-09-25 and found DWS (DEA1's
   13 units wall-sliding on two legs), measured the same day: they stand against blast doors their own presence
   holds open, whose leaves close the doorway — so BNR ([RE]: where a door's bone-bound geometry sits at rest) is
   now DEA1's no-outcome cause and the most valuable RE entry. **The PCSX2 fix list comes before those (prompt §2
   B3, the maintainer 2026-09-25):** WCOL (its RE answered 2026-09-25, `analysis/re-water-colour.md`: the flat water's teal is a
   VU1 view-angle term the port lacks — its build **WCOLB** is next, job C), REFL, KOC, LITE, SKIPC, FOLG, CFOV, then
   KASF, SUNG, WTHR, BFC, KTEX, SPHOT and PXC
   batch 3's LEAF (leaf patches on tree hardpoints) and GLOW (self-lit surfaces) — batch 3 covers all 32 maps
   (`engine/artifacts/pcsx2/ref/FINDINGS.md` F24–F33); PXC captures now run one session in six. The next cleanse falls due 12 landings after the 2026-09-25 one
   (area `mungefs` next; the count is in `engine/artifacts/loop/LAST_CLEANSE`). WCOL's RE also queued **SUND** — the
   port's baked sun is not the PS2's direction on any map (BF1 rotated 90° in azimuth, BF2 mirrored in z;
   `re-water-colour.md` §4) — [RE] first; its build relights every map, so it lands alone after WCOLB.
2. **For the next REFILL (job E) — things no queue entry holds yet:** (a) the in-map PCSX2 step is BUILT (queue PXC
   batch 1, 2026-09-25 — in level on both games, poses from states; PXC's later batches fill the library), so every
   "a capture settles it" note (HOLD 20, 21, the `analysis/` [D]s) can now become a capture job — PXC batch 1's own
   findings are queued as CFOV, WCOL, SUNG, KTEX and a KASF note, batch 2's (nine water maps, 2026-09-25) as REFL,
   FOLG, BFC, SKIPC, KOC, WTHR, LITE, SPHOT ([RE] first, in that value order) and notes on WCOL, SUNG, KTEX, KASF;
   PXC now alternates with C/D; (b) what the disc draws past the
   far cull on the 18 maps that draw no sheet (16 ship no `LOWR`; BF2 DAG1 and NAB2 ship one with no texture) — the
   one thing `OPENBF_FAR_CULL`'s default now waits on — with, since CLN5, the far WATER pass (re-lowr.md §2.3), which
   the PS2 shows through the holes BF1 KAS1's and KAS2's sheets cut, where the port shows the clear colour; and two from OCF's landing (2026-09-23): (c) the foam
   layer's `FoamTile(5,5)` — `mungefs::fx` parses it, `manifest::ManifestWaterConfig` lacks it, so the foam copy keeps
   the base's Tile 4 — [RE] the foam texture matrix first, then a [REBAKE] `maps`; (d) the ocean's vertex RGB — the
   disc packs `DAT_0069c420..422` halved and what writes those bytes is unread ([RE]); the port tints the ocean with
   the flat-water LUT midpoint, which `re-water.md` §3a (c) says the ocean path never uses — **ANSWERED 2026-09-25
   (WCOL, `re-water-colour.md` §1.9): the bytes are the engine's global white (`FUN_00492598`), so the ocean base draws
   at unity, unlit; WCOLB builds it.** P7 vehicles (HOLD 1) is
   the next phase, RE first.


## Standing backlog and standing facts (lifted 2026-09-08 by cleanse C2; pruned 2026-09-23)
**Work items no queue entry holds** (a refill turns them into entries):
1. **`CollisionWorld.bodies` is still `pub`** while the broadphase's staleness stamp is only `bodies.len()`, so an
   in-place edit keeps a stale grid — P7 closes it by making the field private (cleanse 2026-09-17, C1).
2. **An AI-battle replay capture for the maintainer** (Tier-4): the sweep proves rounds end, not that a battle looks like one.
3. **Pad-driven spawn-select state in `core`** — the spawn screen still spawns ONE class per faction.
4. **The A.1/A.2 constants re-read** — briefed (`engine/artifacts/briefs/brief_a4_constants.md`, a five-sweep
   budget) and never run.
5. **The single-file game build** (the maintainer, 2026-09-01, personal use only, never shared or committed): exe-relative
   asset paths, then `scripts/pack.sh` appending an archive + a 16-byte `[magic][u64 offset]` footer that
   `game::assets` reads from its own exe. Not started; the spec is in `analysis/PLAN_LOG_2026.md` (Part 2, §NEWLY
   OPUS-SAFE 2026-09-01 #6).
6. **The lead's older rendering-defect order** — `LOWR` (built, L) → `ALPH` (HOLD 10(d)) → the Hoth clip (built,
   H) → Theed (a disc fact, HOLD 10(c)) → `*_collision` (HOLD 11(b)).
7. **B1:** `Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change, i.e. a declared [SWEEP]
   re-baseline (`--sim-hash 100` = `17e1b946ff0e821c` is pinned by a test in `core/src/sim/state.rs`).
8. **Is `tick_bleed`'s AI-shed loop reachable from bleed at all** (17B's observation)? If not, every tick of every
   round pays for a safety net — RE first.
9. Small: the era/sound/HUD-icon fallbacks; the `--cam-overview` fog look call; the `missions_root()` /
   `extracted_root()` literals; one cross-OS determinism run (the parry flag + `fmath`; DET1 closed the known leak);
   the tat1 `--spawn` pin (pin `--spawn -140,230` in `scripts/sim_gates.sh` and delete `HISTORIC_WALK_START_MAP` —
   the hashes stay put); the guard's false positives (`writes_leadfile`, a READ of `extracted/` through the bake
   binary). (`impl Default for TextureDesc` was DECLINED by the 2026-09-24 cleanse: four literal constructions in the
   workspace, and `mips` — the texel data — has no valid default.)

**Measured facts that justify current behaviour:**
- **The sky V clamp is PER SEGMENT, not blanket** (`engine/artifacts/sky_wrap/dome_uv.txt`): most shells end at
  v ≈ 1.00 (Hoth 1.0010, COR1 1.0107, MUS1 1.0767, NAB1 water 1.13) while BF2 POL1's and TAN1's base domes genuinely
  tile in V (v −1.7..2.7 / −0.5..1.5) — clamping everything would break those two (PLAN stated deviation 3).
- **BF2's `lvl_` mode layers carry CONTENT** — `entc`/`modl`/`skel`/`gmod`/`coll`/`prim`/`tex_`/`expc` inside the
  hashed sub-container (BF2 HOT1 39 entc / 58 modl over four wrappers; POL1 15/15, COR1 11/15, DEA1 7/6, MUS1 5/8,
  MYG1 5/15, UTA1 2/15; BF1 has none) — the census that says which maps a layer regression would hit
  (`layer-content`, `0b93bb1`).
- **The bake carries 363 tier-2 textures** — 342 listed by map manifests plus 21 `side_bf2` stems no manifest lists;
  the test pins `on_disk == manifest.texture_tier2.len()` per directory and `stems.len() == 21`.
- **`GlobalSoundRon` vs `game::sounds::GlobalRon` is a deliberate PARTIAL read**, not a fourth RON twin (the three
  twins are closed: queue 13 `6d87a77`, CT `a2cb9cf`).
- **parry runs with `enhanced-determinism`** (`engine/Cargo.toml`; PLAN §Tech stack says why the version matters).


## The autoloop (added 2026-09-03, end of session 12; shaken down 2026-09-07, session 13 — the maintainer's operator page is `engine/artifacts/loop/START_HERE.md`)
`scripts/AUTOLOOP.md`: `scripts/autoloop.sh` runs unattended Opus sessions on `engine/artifacts/loop/QUEUE.md`.
**Since 2026-09-14 (the maintainer) those sessions are the lead:** each does one job — finish an interrupted job, land a
parked branch, build and land an entry, answer an RE question itself (§5; Fable is off since 2026-09-17), run the
periodic cleanse, or refill the queue — and writes its report and journal paragraph under `engine/artifacts/loop/`.
Guardrails are harness hooks (`.claude/settings.json` → `scripts/claude_guard.sh`), enforced in every session. An
Opus session started by the loop MUST follow `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md` (its standing prompt) —
the rules below still apply.

## Rules for the Opus session (from the maintainer)
- **Fable is OFF; the RE is yours (the maintainer, 2026-09-17).** `$OPENBF_FABLE_MAX` is
  0: spawn no Fable agent. Answer PS2-behaviour questions yourself from the
  decompiled corpora, the ELFs, VU1 microcode and the disc, under
  `OPUS_LOOP_PROMPT.md` §5 — tag every claim [V]/[D]/[H], run the four
  self-checks (name the enclosing function of every citation; a search, not a
  reading, behind every absence claim; the twin open for every "BF2 does the
  same"; the ELF bytes for every constant), then have ONE `model: "opus"`
  verifier agent try to falsify the claim list. Everything else — building,
  landing, docs, the queue, measurements, goldens, decisions — is yours or an
  Opus agent's too. PCSX2 ground truth is yours too since 2026-09-23 (§5; read
  `engine/artifacts/pcsx2/README.md` first, every time).
- Every claim about how the PS2 behaves must cite an existing `analysis/`
  document. If the answer is not already in `analysis/`, **do not guess** — do
  the RE as its own job (`OPUS_LOOP_PROMPT.md` §5), with the four self-checks
  and the verifier agent. The absence claim and the "BF2 does the same" claim
  are the two kinds that were false in the 2026-09-14 blind trial
  (`engine/artifacts/opus_re_trial/report.html`), so they carry the heaviest
  proof. What you could not verify stays [H] and PROVISIONAL; the result is
  lifted into `analysis/` under an `# OPUS RE … [O] UNAUDITED` header, and
  every load-bearing claim carries `[O]` so a later audit can sweep them.
- **Cleanse on a cadence (the maintainer, 2026-09-17).** Every 12 landings (landings only, no time clause — the maintainer, 2026-09-23) a
  session takes a CLEANSE job (`OPUS_LOOP_PROMPT.md` §6b; the checklist is
  `.claude/commands/cleanse.md`): docs and artifacts consolidated every time —
  one live copy of each fact, superseded generations archived verbatim into
  `analysis/PLAN_LOG_2026.md`, `scripts/cleanse_delete.sh` run — plus one code
  area per rotation for dead code, duplication, stale PROVISIONALs, weak tests
  and a bug hunt over everything landed since the previous cleanse. A cleanse
  never changes behaviour: every gate value stays put, and what cannot be fixed
  in the session becomes a `CLN<n>` queue entry.
- **The loop is the lead and it is ALL Opus (the maintainer, 2026-09-14, extended
  2026-09-17 — replaces the 2026-09-09 "Fable sparingly / Fable windows" rule;
  Fable is reserved for the maintainer's day job and is off here, RE included).** The project runs on
  `scripts/autoloop.sh`: its Opus sessions land branches, rebake, regenerate
  goldens (after quantifying and viewing every pair), re-baseline declared
  gate moves, refill the queue and keep PLAN/HANDOFF true, and they never stop
  to wait for the maintainer — a call is taken (faithful, reversible), recorded as a
  `DECIDED` line in PLAN's Log and in §For the maintainer, and the work goes on. Only
  what the maintainer alone can supply (Tier-4 feel checks, legal or guardrail
  questions, a LOCKED FEATURE SET change — PCSX2 captures left this list on
  2026-09-23) waits in §For the maintainer, and `engine/artifacts/loop/DRAINED` is
  written only when nothing else is left. **The maintainer starts the loop HIMSELF; no
  session ever launches `autoloop.sh`.**

## HOLD FOR FABLE — now simply the RE list (redefined 2026-09-17; the name is kept so old greps still find it)
Questions that need reverse-engineering over the decompiled corpora, the ELFs, VU1 microcode, an undocumented
format or the disc. **The loop does them itself** (Fable is off since 2026-09-17): one RE job per question under
`OPUS_LOOP_PROMPT.md` §5 — [V]/[D]/[H] tags, the four self-checks, one Opus verifier agent — lifted into `analysis/`
as `# OPUS RE … [O] UNAUDITED` with every load-bearing claim tagged `[O]`, then this entry amended and the build
queued. It never GUESSES: an unverified hop is [H] and its constant PROVISIONAL. A new question goes here and into
the PLAN Log as a "HOLD FOR FABLE:" line. An answered item keeps its number, so old references resolve, and one line
pointing at its answer — the history it carried is in git (`git show 7498317:HANDOFF-OPUS.md`). Open `[RE]` entries
with their own specs live in the QUEUE, not here: TCC32, KASF, MUSS, UTAS (TL was
answered 2026-09-24: stock SP conquest sets no time limit — `analysis/re-time-limit.md`; SPEC the same night: the
0x80 / class-0x20 specular pass — `analysis/re-specular-pass.md`; WSE 2026-09-25: BF2's `SpecularEnable` and
`OscillationEnable` default to 1, so BF2's flat water speckles and FEL1's does not bob — `analysis/re-water-flags.md`).

4. **Chunks and formats `mungefs` does not parse:** `fx__` effects (only `Effect("Water")` is released —
   `analysis/re-water.md` §5; whether the PS2 draws an effect into BF1 BES1's ring is this item, low value), `coll`'s
   runtime RESPONSE (the format is [V] in `a2-bake-formats.md`), `plan`/`path` AI use, `shdw`, `font`, `bnd_`,
   `CLTH`, vehicle `zaa_` banks. Partly answered 2026-09-17 (`analysis/re-bf2-doors.md`): a `prim` body's `PRNT`
   node can be an ANIMATED bone — every BF2 door — which the port treats as identity (queue DR1/DR2).
7. **The PCSX2 rig.** FIRST TIER LIVE 2026-09-23: `engine/artifacts/pcsx2/pcsx2_capture.sh` boots a disc headless
   and screenshots it, and the loop may use it (read its README first, every time; hold `ci.lock` for the call).
   **IN-LEVEL since 2026-09-25 (queue PXC batch 1):** `pcsx2_session.py` drives both games from boot to a spawn (the
   key paths are in the README), save states hold the spot, and `pose.py` reads the camera pose out of any state, so
   the port can render the same view (`tools/pxc_pose.rs`) — the reference library is `engine/artifacts/pcsx2/ref/`
   (coverage table, `FINDINGS.md`). Still to build: a GS-register dump. The dump's three standing questions: (a) the D-pad menu's second
   material layer (the 0x89 / PRIM 0x58 / TEST 0x51001 entry passed whenever a texture is bound — does the
   microcode draw it); (b) the wreck's LOD at the golden's ~475 m framing (item 20); (c) the far water fog pass's
   look (item 21).
10. **Raised by measurement 2026-08-31, reconciled 2026-09-01:** (a) ANSWERED 2026-09-03 — the fog rule
    (`analysis/re-fog.md`). (b) the `SunInfo` BACK light (`BackAngle`/`BackColor`/`BackDegree`) — every BF1 map
    authors one, it is parsed, nothing renders it. (c) BF2 Theed's sky is BLACK because the disc's `sky_naboo2` is
    (all 256 palette entries in 0..3, read from the raw `pal_` bytes), so item 6's fade takes Theed's distance to
    black — faithful; the maintainer's eye. (d) terrain `ALPH` — `COLR` is cracked at element size 2, `ALPH` falls out at no
    element size, nor do the per-patch `PTCH -> COLR/ALPH` overrides. (e) the `sky_ SkyObject` bearing — count,
    altitude band and distance are authored, azimuth is not; `InDirectionFactor` and the unnamed `0xb4c88558` are
    parsed and unused; the even spread is ours. (f) ANSWERED 2026-09-09 — water `CPAT`/`WMAP`/`LAYR`
    (`analysis/re-water.md` §3/§4). (g) the `MTRL` flag word is ANSWERED at the CPU level
    (`analysis/re-material-flags.md`: 0x04 blend, 0x02 test at AREF 0x60, 0x40 additive, 0x08 half-alpha, 0x80
    specular — its pass answered 2026-09-24 in `analysis/re-specular-pass.md`, queue SPEC/SPEC2 — 0x100 env) and
    its CLUT-alpha half is closed (`re-hud-icons.md` citation 3); still held: the
    reconciliation with what the GS shows — ALPHA RULE v2 stands until a GS dump (item 7), except where the ELF
    reading needs no capture: AT took the disc's test (193), SKA the soldiers' rule, R1U (2026-09-24) stopped
    rule 1 compositing a word with neither 0x02 nor 0x04, and KW (the same day) composites a 0x04 word whose sheet
    has no texel past the test (Kamino's windows); a 0x04 sheet past the test is the crate question, still held. (h) sound samples whose
    `frequency` is outside 4–48 kHz (1.8 % of every bank). (i) ANSWERED 2026-09-08 — `LOWR` (`analysis/re-lowr.md`;
    built as queue L). (j) duplicate sound-property definitions — a SOUND LVL defines a property once per sublevel
    and 63 properties over the 32 maps pair an audible definition with an empty one; which one the PS2 plays is
    unread, and the port keeps the last audible one (PROVISIONAL, `game::sounds::merge_props`; added 2026-09-23 by
    the cleanse — the code had cited a PLAN line that never existed).
19. **ANSWERED 2026-09-08/09** — the walkabout defect hunt's (a)–(g): `re-terrain-cutters.md` (queue H),
    `re-ai-sniper.md`, `re-lowr.md` (queue L), `re-water.md` §1/§2 (W1/W2/WL); the droideka decided 2026-09-08
    (the maintainer: jumps everywhere, BF2's numbers — the build is P7-class). Nothing held.


## How the last session verified things (keep doing this)
- **Process rules learned (keep — the cumulative list):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before removing it (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout, and a BACKGROUND Bash call resets the harness cwd the same way (s17); a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped; view a regenerated golden as a COMMITTED-vs-REGENERATED pair (`golden_pairs.py`), never the new image alone, and **quantify its diff (mean / max / changed-pixel fraction + bbox) BEFORE viewing it**; a MAP golden can move when only the SIDE bake changed (s17) — and when only the WATER bake changed (s18); two [V] facts from two different RE agents can COMPOSE into a port rule neither of them stated (s17); a paused loop never sees STOP — kill it (AUTOLOOP.md); after any edit to `autoloop.sh` the live lanes are stale — kill and relaunch; launch the loop from a plain terminal or with every `CLAUDE*` env var stripped and run `--probe` first; after ANY queue generation check that `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists every item; **never `pkill -f` a pattern that is in your own command line**; `cmp_bake.sh` EXITS 1 whenever DIFF≠0 — an expected-family diff is not a failure, read the families not the exit code (s18); a branch that re-baselines a gate lands LAST, so every earlier branch still measures against the old baseline (s18); and a re-baselining item should ship the escape hatch that reproduces the old value where one is possible (`SWEEP_SLOT0=player`) so the move can be proved inert. **New this window (s19):** one branch can re-baseline TWO gates at once and there is not always an escape hatch — when there is none, the reproduction is a raw `cmp` against the LOOP SESSION's own after-files (item 4's sweep and tat2 arena both `cmp` 0 against `lead_tmp/era-class-db/final/`), so a loop session must park those files; and a report's "extra" gate can name a GENERATOR rather than a referee (`examples/squad_log` writes the log and prints no hash) — the sim hash comes from the artifact README's own capture recipe, so read the README before reproducing an extra.
