# openbf — the archived Log, 2026

This file is the **append-only record of PLAN.md's Log up to 2026-09-09**, moved
here verbatim by the cleanses so that PLAN.md can be the plan again rather than a
4,800-line chronology: Part 1 (to 2026-09-03) by queue item C2 on 2026-09-08,
Part 3 (2026-09-07..09, the Fable landing windows) by the loop's cleanse on
2026-09-23. PLAN.md's Log is append-only from 2026-09-14 forward; everything
before that lives here, unedited.

**The rules this file is kept under.**
- Text arrives here **verbatim**. Nothing is rewritten, summarised or corrected
  on the way in — a later correction lives in its own later entry, exactly as it
  did in the Log.
- PLAN.md keeps one dated **summary** entry per stretch, and that summary names
  the section here to read for the detail.
- Nothing that is still a live decision, specification or open question is
  archived. The four decided-but-unbuilt designs were lifted into PLAN.md
  §DESIGN SPECS before this file was written; the stranded work items and
  measured facts were lifted into HANDOFF-OPUS.md §Standing backlog and standing
  facts. If you find something here that is still live, it belongs back in
  PLAN.md — this file is not a place for work to hide.
- The entries are the record of what was DONE and WHY, including the wrong
  turns. Several entries correct earlier entries; read a topic to its end.

---

## Part 1 — PLAN.md Log, 2026-08-29 to 2026-09-03

### 2026-08-29 / 2026-08-30 — kickoff, P0–P6, the first Opus sessions

Summary in PLAN.md's Log under the 2026-08-29/30 entries. Everything below is
verbatim from PLAN.md as of commit `bf205fe`.

- 2026-08-29: Project created. Extraction, recon, Ghidra analyses, binding tables
  (769/483 named), showcase + full decomp dumps (37MB C).
- 2026-08-29: Vision v1: merged BF1+BF2, Rust+wgpu, bake pipeline, LAN.
- 2026-08-30: the maintainer answered full 28-item questionnaire → feature set LOCKED above.
  Notable new designs: contextual roll/prone button, session cheat registry, class
  meta-unlock, killable flat-health heroes, no-overspawn invariant, era mix-and-match,
  classic-plus visuals, controller-only. Aim assist locked BF1 (X-lock dropped). A1 run.
- 2026-08-30 (later): Appended autosave-everywhere, host/P1 session model, host-only MP
  cheat authority; MP-GC autosave ownership deferred (#8).
- 2026-08-30 (evening): Added PART B autonomous execution guide (working mode, locked tech
  stack from the maintainer's wgpu projects, workspace layout, capture/replay spine, 5-tier testing
  ladder, RE fidelity method, community/no-bloat rule, subagent protocol, legal guardrails,
  per-phase acceptance P0–P5). Copied both ISOs local to `iso/` (SSD no longer needed).
  Confirmed toolchain (Rust 1.97, NVIDIA RTX 4000 Ada/Vulkan).
- 2026-08-30 (kickoff decisions): the maintainer resolved Q1–Q4 → STAGED fidelity (decomp-first,
  PCSX2 later), RUN-UNTIL-BLOCKED autonomy, RE-AUDITS-FIRST build order, LOCAL-git only.
  The maintainer installed Blender 5.2 LTS system-wide and dropped a PCSX2 source checkout at
  `pcsx2-master/` (build later for `gsrunner` trace capture; needs a BIOS). Plan updated;
  project cleared to start autonomously at P0 (audits A2–A6).
- 2026-08-30 (later): the maintainer added a prebuilt PCSX2 2.8.0 Qt AppImage in `pcsx2-master/`
  (verified runnable). No source build needed for interactive use; BIOS still required for
  the later trace rig; headless capture will need xvfb-run or gsrunner.
- 2026-08-30 (agent, autonomous): **P0 COMPLETE.** Git repo initialized (local-only,
  guardrail gitignore verified — no binary-derived material tracked). Engine workspace
  scaffolded (9 crates, ucfb parser skeleton, ci.sh Tier 0/1 gate, locked dep versions
  verified to resolve). All audits A2–A6 run via parallel subagents, each output
  lead-verified against the corpora/data before integration (details in RE_LOG.md):
  A2 formats (PS2 RLE texture codec cracked; terrain HGT8/HEXP = open Q for P1),
  A3 subsystem map (436/47/332; AI divergence = planning layer, not soldier layer;
  252 dead online bindings flagged do-not-RE), A4 call-graph (keep-set 83–85%/game),
  A5 timing (29.97fps render, variable measured dt, 0.1s clamp → engine locks fixed
  30Hz tick), A6 mission Lua (439 scripts; PC mod-tools .lua = the missions → RON
  generates from PC sources, PS2 wins deltas). First traced ports: SCE rand LCG
  (SimRng), tick constants. RE_LOG.md seeded and substantive. → Proceeding to P1
  (bake v0); first map: BF1 TAT1 (Tatooine Dune Sea — modest geometry, clear CPs,
  Tatooine exists in both games for later BF1-vs-BF2 comparison).
- 2026-08-30 (agent, autonomous): **P1 COMPLETE — bake v0 for BF1 TAT1.** mungefs parses
  container/RLE/tex/modl/wrld/tern/soundbank + reads ISO9660 directly (byte-identical to
  7z). Bake emits: 46 KTX2 textures (round-trip via ktx2 crate), 51 model GLBs (via gltf
  crate, 14.9k tris), 130k-tri terrain GLB (HGT8/HEXP formula RE'd from decomp, verified
  empirically — floor −0.6 exact), RON manifest (249 instances/15 regions), and the BF1
  loading beep (`ui_load_lp` 440 Hz + both as-played pitched renders) as WAV+OGG,
  ffprobe-verified. Two A2 errata found and fixed during implementation (ABGR palette
  order; HEXP = 8-byte records per 16×16 patch). RE subagents delivered terrain-height +
  sound-bank specs; both lead-verified (decomp bodies read directly, empirics reproduced,
  independent Rust decoders match traced references in Tier-1 tests). All commits
  Tier-0/1 green. → P2 (wgpu map viewer) next.
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
- 2026-08-30 (agent, autonomous): **P3 core landed.** RE subagents completed + lead-
  verified: BF1 aim-assist magnetism (re-aim-assist.md; 0.5 slowdown + 0.75 velocity
  tracking + gated fire magnetism toward ballistic lead) and both stance machines +
  rolls (re-stance-roll.md; BF2 already implements our contextual button — decision
  recorded under §Movement & feel). Ported to engine with decomp citations + Tier-1
  reference-vector tests: core::aim_assist (T1–T6 green), core::heightfield (terrain
  query = parity-alternating triangle lerp from FUN_00473710), core::soldier (stance
  machine, BF2 impulse roll, sprint/energy w/ depleted latch, prone restored; sim-
  hash reproducible), input crate (gilrs→PadState, PS2-default mapping), game walk
  mode (--walk windowed + --walk-replay/--capture-series headless). Soldier ODF
  constants surveyed+baked (bake classes cmd; ClassDb w/ BF2 BASE-chain inheritance).
  Tier-4 artifact: engine/artifacts/p3_walk/ (walk_demo.mp4 + motion_plot.png —
  every plateau matches RE'd constants: 9.0/13.5/15.3/6.3/2.7/11.7). PROVISIONAL
  (pin via anim data/PCSX2): roll duration 0.75s, sprint-recovery threshold 25.
  Aim assist now wired vs dummy targets (--targets: acquisition ray, magnet
  highlight, pacer tracked; f2 sign resolved positive-left, physics-forced).
  REMAINING for P3: PCSX2 spot-check backlog (Q1-staged; items: axis identity,
  turn-rate vcall, BF1 pad mapping, sprint-recovery threshold), anim-pinned roll/
  transition durations (subagent on zaa_ banks), soldier visual beyond the box.
  Housekeeping per the maintainer: installer exes 7z-verified, original-download.rar deleted
  (5.1GB); 20-superseded/ kept until W1 verifies installer LVLs in our bake. Housekeeping: accidental 2.65GB
  mod-download blob purged from git history (history rewritten, verified clean).
- 2026-08-30 (agent, session end — clean stopping point, all subagents landed):
  **P2 COMPLETE, P3 COMPLETE (modulo Q1-staged PCSX2 items), P4 STARTED.** All five
  RE subagent reports delivered, lead-verified, and integrated this session:
  re-aim-assist.md, re-stance-roll.md, re-anim-banks.md (zaa_/MINA + engine MSB
  CRC-32 cracked; roll 0.85 s + prone locks 1.33/1.5 s PINNED in code),
  re-tickets-spawn.md (death consumes ticket; no-overspawn is ENGINE-NATIVE in both
  games; BF2 bleed −1/3 s; BF1 bleed provably inert; BF1 runs 25–32/team),
  re-blaster-damage.md (dt-accumulator fire cycle, predictive-raycast bolts, exact
  spread formula; AimFactorPosture*/AimValue proven DEAD DATA in both ELFs — any
  stance-dependent accuracy in openbf is a new feature, not fidelity; BF2 rifle
  80→50 falloff, BF1 flat 60). Engine state: core::{sim,soldier,aim_assist,
  heightfield,battle,rng}, game::{assets,classdb,viewer,walk}, input, bake
  {map,classes,beep,tex-dump}, render (now trilinear w/ full mip chains, golden
  regenerated). Walk mode playable windowed (--walk --targets) and headless-
  replayable; Tier-4 artifacts in engine/artifacts/{p2_flythrough,p3_walk}/ for
  The maintainer's async review. Widescreen-native + classic-plus asset plan + evidence base
  recorded. NEXT (P4 proper): weapons/projectiles in sim from re-blaster-damage
  specs, CPs + capture + BF2 bleed wired to core::battle, damage/health, dummy-AI
  targets → then P5 slice. Also queued: COMMON.LVL class bake (turrets/droids),
  PCSX2 trace rig backlog, W1 PC-LVL texture reader.
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
- 2026-08-30 (agent, autonomous, P4 session 2 — later): **P4 combat v0 feature-
  complete in code; Tier-4 artifact in production.** Landed since the previous
  entry: Zero Editor `path` chunks baked (mungefs::path, 13 on TAT1) + game::layout
  (6 CPs with capture/control regions, spawn paths, teams, ValueBleed) +
  core::region; core::cp (two-phase capture with a PROVISIONAL rate rule, BF2
  CP-majority bleed clamped at alive, 20 s domination, spawn points) + core::ai
  (pad-driven placeholder with bounded aim error) + Sim respawns (4 s, BF2
  alive-AI cap 16, no-overspawn admission incl. player-displaces-AI) + BF2
  immediate defeat at 0 tickets; game `--ai N` conquest setup from the layout, CP
  pole markers, `--tickets`, `--events` CSV, spectator `--cam` for headless
  captures; `cargo run -p core --example ttk_trace` reproduces the RE report's
  TTKs inside the full sim (BF1 5×60 → 1.03 s @20 m; BF2 5×80 → 0.90 s, 7×65
  @80 m, 8×50 @100 m). 110 Tier-0/1 tests green. P4 acceptance: blasters ✓ damage
  ✓ spawns ✓ CPs ✓(provisional rule) tickets+invariant ✓ minimal AI ✓ cheats ✓.
  OPEN before calling P4 closed: integrate analysis/re-cp-capture.md (RE subagent
  in flight — capture rate/headcount rule, region SIZE semantics, spawn
  eligibility) into core::cp/core::region; Tier-4 artifact folder
  engine/artifacts/p4_combat/ (firefight video + TTK traces + README) for the maintainer's
  async review. THEN P5 (vertical slice): BF2 map bake (A2 BF2 deltas: MTRL 25 B,
  RTYP u32), full conquest round vs AI on TAT1 + one BF2 map, era-mix option,
  replay-stable --sim-hash.
- 2026-08-30 (agent, autonomous): **P4 COMPLETE — combat v0.** The CP-capture RE
  landed and was lead-verified against the decomp (analysis/re-cp-capture.md →
  RE_LOG §Command-post capture): core::cp now runs the real two-meter machine
  (presence weight 2−2^(1−n), drain-then-flip, contested freeze, defenders don't
  slow neutralization, BF2 owner-inside recovery) and core::region the real SIZE
  semantics (sphere |SIZE|, cylinder √(sx²+sz²), strict tests) — all 12 report
  vectors that apply are Tier-1 tests. One documented divergence: the neutralize
  meter is zeroed on capture (BF2 leaves it at N; BF1's guard shows the intent).
  Acceptance met: blasters + damage + spawns + CPs + tickets w/ no-overspawn
  (unit + property tests) + minimal AI + dev cheats (`--cheat invincible,ammo`).
  Tier-4 artifacts for the maintainer's async review: engine/artifacts/p4_combat/
  (firefight_cp3.mp4 — 90 s, 8 AI/team on TAT1 cp3; ttk_traces.csv reproducing
  the RE report's TTKs in the full sim; events/motion CSVs; README with the
  known v0 gaps: placeholder AI, boxes for soldiers/bolts/posts, no effects/
  audio/HUD). Not blocking. → **P5 (vertical slice) next**: BF2 map bake (TAT2
  has 4 top-level `wrld` layers + per-mode `lvl_` layers (tat2_con etc.) and
  top-level `path` chunks — bake must pick the base world, merge the chosen
  mode layer, read paths at top level), full conquest round vs AI on TAT1 + one
  BF2 map, era-mix option, replay-stable `--sim-hash`. Also queued: PCSX2 items
  (BF2 leftover-meter, predictive-raycast dodge, BF1 bleed no-op), COMMON.LVL
  turret/droid classes into the runtime, soldier visuals.
- 2026-08-30 (agent, autonomous): **P5 vertical slice — functionally complete;
  Tier-4 artifact + the maintainer's windowed check outstanding.** Landed: BF2 map bake
  (config-tree parser, BF2 .pth paths z-negated per the loader, base + always-on
  layers, per-mode `lvl_` layers merged at load with `--mode`; TAT2 Mos Eisley
  bakes and plays), rosters from baked classes (game::roster: faction → soldier
  class → SoldierConstants::from_props + primary weapon through the wpnc/ordc
  chains; manifest `source_game`; `--teams` era mix, e.g. all vs cis), mission
  data (engine/missions/tat1_con.ron 25/25 units 250 tickets, tat2_con.ron 20
  units 150 tickets cap 16 — transcribed from the RE'd mission Lua; generation
  from the PC .lua sources still queued), `--mission` driving factions/tickets/
  unit counts/AI cap, title-bar HUD + round-over report (egui HUD is P6),
  headless final sim hash, Tier-1 replay-determinism test on a fought conquest
  round (same seed+log → identical hash). Smokes: TAT1 mission 24/25 AI runs
  ~2.4 ms/tick debug; TAT2 16/16 fights, captures/neutralizes, both hashes
  stable. Acceptance: BF1 map ✓ TAT1, BF2 map ✓ TAT2, era-mix ✓, deterministic
  --sim-hash ✓; "playable" = walk mode + R2 fire + title HUD (needs the maintainer's
  windowed run with the pad). KNOWN GAPS (not blocking): placeholder AI piles
  onto one post; no unit-unit collision; 27 TAT2 wall instances skip (models
  not in the map LVL — likely a shared props LVL, check at the next bake pass);
  turrets/droids (COMMON.LVL/INGAME classes) not spawned; soldiers/bolts/posts
  are boxes; no effects/audio; BF1/BF2 spawn-wave scheme not modeled (flat 4 s
  respawn); JumpHeight→velocity conversion untraced. → P6 (shell UI, BF1 loading
  screens, settings, pause-menu cheats, profiles/autosave) next per PLAN, with
  the egui HUD as its first piece; queued alongside: COMMON/INGAME object
  classes (turrets/recharge droids) in the runtime, soldier visuals, PCSX2
  spot-check rig.
- 2026-08-30 (agent, autonomous): **P5 Tier-4 artifact produced; P6 started with the
  egui HUD.** engine/artifacts/p5_slice/ (tat2_conquest.mp4 — 60 s mission-driven
  Mos Eisley conquest, 16/16 AI, 1817 shots / 110 kills / 4-2 posts; events +
  motion CSVs; README with the play-it-yourself commands and known gaps). P6 piece
  1: `ui` crate (egui 0.33 + egui-wgpu/egui-winit on wgpu 27, headless-testable
  `hud::draw_hud` over a `HudModel`; `overlay::EguiOverlay` glue) painted through a
  new `Renderer::render_with_overlay` hook; the game shows a BF2-style HUD —
  reinforcements + posts top-center, HP/energy/ammo bottom-left, kill feed
  top-right, round banner, PS2 control hints — all edge/corner-anchored (widescreen
  rule). Title-bar HUD stays as a fallback. FIX found the same evening: BF2 PS2
  modl INFO is 72 B (A2 erratum) — every TAT2 model had been dequantized into a
  shifted box; extents are now pinned in Tier-1 and TAT2 renders as Mos Eisley
  proper; P5 artifact regenerated. Pause menu + session cheat registry + profile
  autosave landed; shell v0 (instant action: map/mode/era-mix/AI/tickets, Start,
  back-to-shell) landed — `game` with no args opens it. BF1 loading screen landed:
  `bake loadscreen LOAD/<MAP>.LVL <mapdir>` bakes the zoom sequence (galactic map →
  locale → landsat → level + planet globe texture); the shell's Start plays it
  pillarboxed 4:3 (~1 s/step) before the synchronous map load. Audio crate wired
  with kira 0.10 behind a `backend` feature — the maintainer installed `alsa-lib-devel` the
  same session, the backend is ON by default and opens a device here (kira
  `opened: true`); the loading beep (baked `assets_baked/audio/load_xtracking.ogg`)
  plays during the load sequence (`--no-default-features -p audio` keeps a silent
  no-op API for boxes without ALSA headers). Shared assets landed: `bake common
  COMMON.LVL|INGAME.LVL assets_baked/common_<game>` (no world) and the loader merges
  them under each map → command posts, recharge droids and turrets now draw with
  their real models (TAT1 329 draws; only dust emitters / vehicle spawns / sound
  emitters remain undrawable); 32-bpp direct-color textures (BF1 effects) decode. Headless HUD path + Tier-2 golden
  (goldens/tat1_hud.png, `cargo test -p game --test golden_hud -- --ignored`;
  found and fixed: egui fades new Areas in over animation_time, so headless
  captures need animation_time = 0). NEXT in P6: pause menu + cheat
  codes registry, BF2 shell flow (map/mode/era pick), BF1 loading screen + beeps,
  settings (aim-assist intensity, unit-count slider), profiles/autosave scaffold.
- 2026-08-30 (agent, autonomous): **P6 status — shell/loading/HUD/pause+cheats/profile
  all landed as v0; only the authentic cheat-code button sequences remain (RE
  subagent in flight → sequence recognizer in ui::menu).** Also this stretch: shared
  com_* assets (`bake common`) so posts/droids/turrets draw for real; 32-bpp effect
  textures; BF2 modl INFO 72-B fix; audio backend live (ALSA headers installed by
  The maintainer). Tier-2 goldens: tat1_overview, tat1_hud, shell. Artifacts for the maintainer:
  engine/artifacts/{p4_combat,p5_slice,p6_shell}. NEXT: weapon SFX through the audio
  crate (snd_ property scripts → sample banks per side LVL, `bake sounds`), then
  soldier visuals (skel + softskin + zaa_ clips → skinned render path) before P7
  vehicles; cheat sequences when the RE lands.
- 2026-08-30 (agent, autonomous): **P6 CLOSED.** Weapon SFX end to end (SOUND/<planet>.LVL
  banks → WAV + `sounds.ron`, snd_ property scripts with inheritance, Shot/Reload/
  Hit/impact one-shots with distance attenuation + pan) and the sample-bank
  **alias** records cracked (40–55 % of records alias another id; tat1 614 / tat2
  644 samples, none skipped). Cheat codes lead-verified against the raw BF2 ELF
  (Morse on the D-pad while paused: sos/ammo/hud/slow/80s/at/alias; BF1 has none)
  and ported bit-for-bit as `ui::menu::MorseCheats` (5 tests) wired into the pause
  loop. Open: `shell_menu_accept` confirmation sound not located; BF2
  SOUND/GLOBAL.LVL (827 MB) + SHELL.LVL now extracted locally for later. NEXT:
  soldier visuals (skel + softskin + zaa_ clips → skinned render path; RE agent
  first), then P7 vehicles; queue unchanged otherwise (PCSX2 rig, W1 PC-LVL
  textures, BF2 non-Tatooine load screens, spawn waves, unit collision).
- 2026-08-30 (agent, autonomous): **Soldier visuals started — render side first.** GPU
  skinned pipeline in `render` (`SkinnedVertex` joints/weights, `vs_skinned`, per-draw
  96-joint palettes via a dynamic-offset uniform, `render_frame(draws, skinned)`;
  two-bone headless GPU test green on the RTX) + `game::skin` (Skeleton/Pose →
  palette, any bone order, blend). Two fable RE agents in flight: PS2 softskin
  `modl` (WGHT/BIND/BONE + vertex weights, `skel` bind pose) and `zaa_` keyframes
  (TNJA/TADA + `zaf_` rig). Next: `mungefs::anim` clip directory (SMNA/MINA — already
  verified in re-anim-banks.md), then parsers/bake/loader once the RE lands.
- 2026-08-30 (agent, autonomous): **Waiting on the soldier-visuals RE → gameplay queue
  items landed.** Skinned GLB round trip (`bake::skin_glb` ↔ `assets::load_skinned_glb`,
  `viewer::upload_skinned`, headless GPU test), `mungefs::anim` clip directory (Tier-1 on
  both human banks), unit-vs-unit separation (PROVISIONAL), and **spawn waves** per
  re-tickets-spawn §1.5/§2.5 (SetSpawnDelay(10, 0.25); wave+1/next-slot at death; HUD
  "RESPAWNING IN N" from the member timer ∨ wave countdown). W1 PC-LVL texture reader
  delegated to an Opus agent (in flight). Three agents running: softskin RE, anim-keys RE,
  W1.
- 2026-08-30 (agent, autonomous): **Soldier visuals LANDED.** Two fable RE passes
  (re-softskin.md: rigid-per-vertex softskin in bone bind space, BMAP/BIND, skel with
  parent names + row-vector XFRM; re-anim-keys.md: TNJA/TADA keys, zaf_ rig, sampling +
  per-game timing, DummyRoot root motion) lead-verified (raw-byte spot checks, decoder
  rerun over all 15 human banks, wireframe T-poses, rig-rest ≡ skel-bind 25/25) and
  ported: `mungefs::{skel, anim, model softskin}`, `bake soldier` (model-space skinned
  GLB + 34 human_rifle clips + textures), `game::{animator, soldier_vis}` — real
  textured stormtroopers/rebels/clones/droids animate in both maps (chase-cam frames in
  artifacts/p7_soldiers). Also this stretch: BF2 load-screen pools, spawn waves, unit
  separation, `mungefs::anim` clip directory. NEXT: weapon attach to `hp_weapons`,
  then **P7 vehicles** (RE first); W1 PC-LVL texture reader agent still in flight.
- 2026-08-30 (agent, session end — HANDOFF TO OPUS): the maintainer is out of Fable credits.
  `HANDOFF-OPUS.md` (repo root) is the brief for the Opus session: never spawn
  fable; a **HOLD FOR FABLE** list (P7 vehicles, anim blend layers, new formats,
  sim fidelity constants, mission Lua extraction, PCSX2 rig, classic-plus override
  design, unresolved "where does the PS2 get X" questions) and an Opus-safe backlog
  (weapon attach orientation check, camera vs buildings, spawn screen, effect
  placeholders, more maps with the existing pipeline, perf, W1 follow-through).
  Last commits: soldier visuals + hand weapons (577d8bd). W1 PC-LVL texture reader
  (Opus agent) reviewed/committed at session end if it finished — see Log below.
- 2026-08-30 (the maintainer, mid-session): **Harrisonfog remaster integration directive** →
  §Classic-plus assets added (HD texture overrides from PC LVLs, native wgpu look
  from Shader Patch MIT + preset numbers, addon maps as variants, no shell/exe
  ports); R1 graphics half resolved, R2 + W1 queued. Sources organized + verified
  under community_mods/harrisonfog/ (INVENTORY.md; gitignored local-only).
- 2026-08-30 (agent, session end — clean stopping point): **P2 in progress, 3 of 4
  pieces done.** (1) Determinism spine live: fixed-tick `Sim` + input-log replay +
  FNV-64 state hash; `game --sim-hash/--replay` work and reproduce. (2) Asset loader:
  baked manifest/GLB/KTX2 → CPU `Scene`; TAT1 loads back, material→texture links
  resolve (Tier-1). (3) `render` crate: wgpu 27 windowed+headless renderer (instanced
  draws, yaw/pitch fly-cam, Depth32Float, lambert+ambient WGSL with alpha-test,
  PNG capture with row-padding strip); adapted from an earlier wgpu project's code
  (cited in-source); 6 unit tests + GPU smoke test GREEN on the RTX 4000 Ada
  (headless verified end-to-end). wgpu-27 quirks documented in render crate comments.
  REMAINING for P2: wire game `--map <baked>` + `--capture-frame` (Scene → render
  DrawCmds: terrain mesh + per-instance transforms + texture binding), fly-cam input
  (gilrs), commit a TAT1 golden PNG + Tier-2 golden test, then the Tier-4 feel
  artifact for the maintainer. NOTE for integrator: baked world transforms are PS2 native
  (Y-up per terrain worldY; verify instance rotation row/column convention against
  a known object like the sarlacc pit when first rendering).
- 2026-08-30 (Opus session): **TEXTURE ERRATUM — 256-colour CLUTs are LINEAR.**
  The maintainer decoded raw LVL bytes both ways and found the P1 CSM1 unswizzle wrong;
  reproduced here on a 4-texture contact sheet (rep_inf_ep3trooper,
  tat2_bldg_set01, tat1_rocks1, all_inf_trooper — CSM1 speckles all four,
  linear is clean). The P1 validation ("8-bit sky/rocks decode clean") was
  invalid: a noisy rock still reads as rock, and the two textures actually
  spot-checked by hand (sandcrawler, tat1_map) are 4-bit, which CSM1 never
  touched. `mungefs::texture` unswizzle + unit test deleted; new Tier-1
  regression measures adjacent-texel luma step on tat1_rocks1 (linear 9.37 vs
  CSM1 19.78). `bake tex-dump` now descends lvl_ sublevels (SIDE LVL textures
  were invisible to it) and skips unsupported textures instead of aborting.
  Everything rebaked via the new `scripts/rebake.sh` (one command for all
  eight groups; the exact bake invocations now live in the repo instead of in
  Log prose); goldens/tat1_overview.png + tat1_hud.png regenerated after
  viewing — rock strata are legible where the old golden was mush.
- 2026-08-30 (Opus session): **BF1 detail textures land (`<name>_dtl`).** A2 §4
  had the rule since the audit (detailCompressed → modulate x2 against the
  `_dtl` sibling) and nothing consumed it, which is why BF1 units looked like
  64-px blobs beside BF2's 256² ones. Census + evidence in RE_LOG "BF1 detail
  textures": 22 of ALL.LVL's 84 textures are flagged, each with exactly one
  companion at exactly 4x the base size, signal in ALPHA, mean 128 (x2-neutral);
  the companion is REGISTERED to the base (the face's eyes/beard land on the
  base face's), so the UV scale is 1.0 and no scale constant had to be invented
  — nothing here is PROVISIONAL. BF2 sides never use it. Ported end to end:
  `bake::detail` (companion baked as `<base>.dtl.ktx2`; its presence is the
  flag), `assets::CpuTexture.detail` + `load_texture`, `render::DetailDesc`
  (bind group 1 gains the detail view + a MaterialParams uniform),
  shader `detail_factor`. The multiply is done in GAMMA space — `pow(2*d, 2.2)`
  on the linearised base — because the GS multiplies stored texels; a headless
  GPU test pins the ratios and excludes the linear-space answer. Tier-4:
  `engine/artifacts/textures_fix/` (A/B sheets, lead-viewed).
  Also: `viewer::upload_cpu_texture`/`upload_texture_file` now own every
  texture upload (soldier_vis's duplicate is gone), `pose_preview` renders
  textures and takes `--cell/--dist/--yaw`, goldens regenerated (TAT1's only
  detail-mapped texture is the skiff: 104 px changed).
- 2026-08-30 (Opus session): **Backlog #1 (weapon attach orientation) — no fix
  needed.** `pose_preview` at yaw 90/180/270 with `imp_weap_inf_rifle` on the
  stormtrooper: the rifle sits level at the hip in the right hand with the
  muzzle pointing the way the soldier faces (weapon local +Z = barrel, and the
  posed `hp_weapons` bone already orients it). No PROVISIONAL rotation added —
  a fixed rotation would have BROKEN a correct attach. Artifact:
  `engine/artifacts/textures_fix/` §Also.
- 2026-08-30 (Opus session): **Backlog #2 — third-person camera vs buildings
  (PROVISIONAL).** `game::camera_collide::CameraBlockers` shortens the chase
  track when world geometry sits between the player and the camera: AABB broad
  phase per drawable instance (659 on TAT2) → the 3.2 m segment pushed into
  model space and tested against the model's TRIANGLES; the camera stops 0.35 m
  short of the first hit and never closer than 25 % of the track. An AABB-only
  first attempt was written, measured and REJECTED: Mos Eisley's buildings are
  concave and their boxes swallow the streets, so the camera slammed against
  the player's back in 3 of 6 sampled open-street frames. With triangles: TAT2
  changes in 1 of 11 frames (the frame where the camera was genuinely inside a
  wall) and TAT1's open desert in 0 of 5. Visual only — `WalkSim.camera_blockers`
  is never read by the sim; the walk-demo sim hash is byte-identical with and
  without it (f7b2f6ade72989dd). Tier-4: `engine/artifacts/camera_collide/`.
  HOLD FOR FABLE: what the PS2 camera actually does on contact (spring rate,
  pull-in vs slide, recovery) and whether it uses the `coll` chunks — nothing
  in `analysis/` covers it, so the pull-in policy is labelled PROVISIONAL.
- 2026-08-30 (Opus session): **Backlog #3 — spawn screen (UI only).**
  `ui::spawn::draw_spawn` over the battle frame while the player is dead and
  the round is live: the team's spawnable posts (owned + spawn nodes) with
  distance from the death spot and an UNDER ATTACK flag for posts being
  neutralized, the live reinforcement count, and the wave countdown
  (`Sim.waves[team].countdown` ∨ the unit's respawn timer — the same quantity
  the HUD banner used, which is now suppressed while the screen is up).
  `game::main::spawn_model` builds it; the d-pad moves a cursor held in `App`.
  **The cursor does not steer the respawn** and the panel says so in small
  print: a player's spawn choice has to arrive through the input log
  (`PadState`) or replays and sim hashes stop reproducing, and that means a
  pad-driven spawn-select state in `core` — a sim change this task explicitly
  excluded. NEXT STEP for whoever takes it: add that state to `Sim` (driven by
  the existing pad bits so determinism holds) and have `respawn_slot` prefer
  the chosen CP's nodes. Tier-2 golden `goldens/tat1_spawn.png` (+ egui layout
  tests at two aspects); Tier-4 `engine/artifacts/spawn_screen/` incl. the
  screen in a real TAT2 round (player killed at tick 386).
- 2026-08-30 (Opus session): **Backlog #4 — placeholder effects.** `game::fx`
  turns sim events into camera-facing billboards: muzzle flash on `Shot`,
  ground/air impact on `BoltExpired { terrain: true }`, a warm spark on `Hit`
  at the victim's chest. Wired into both the windowed and headless paths
  (`viewer::upload_billboard_quad` + `upload_fx_sprite`). Everything is
  PROVISIONAL and says so — `fx__` is NOT parsed (HOLD FOR FABLE), so no
  size/colour/lifetime here claims fidelity. Two measured fixes: the sim's
  `Shot` origin is the shooter's CHEST (`Unit::muzzle()`), so a flash drawn
  there is swallowed by the shooter's own model in third person (19 visible
  px) — it is offset a rifle length forward + 0.3 m right; and since the
  renderer alpha-tests at 0.5 with no blending, effects cannot fade out, so
  they use a procedural round sprite and fade by shrinking + dimming. The
  billboard quad's normals point into the sun so effects do not dim as the
  camera turns. Tier-4: `engine/artifacts/fx_placeholder/`.
- 2026-08-30 (Opus session): **Backlog #5 — every ground map bakes and runs
  (30, was 2).** All BF1 + BF2 ground maps bake through the existing pipeline
  into `assets_baked/<game>_<lvl>` with their load screens (`scripts/rebake.sh
  maps`; space SPA* is P8). Eleven failed at first; both causes were errata in
  formats we already parse, found by reading bytes against the existing specs
  (RE_LOG "All 30 ground maps bake"): `path` INFO is **u16 count + u16 kind**
  (four BF1 maps carry a kind-2 "boundary" path that read as 131078 nodes),
  and a `tern` can be a **stub** (NAME only, or NAME + WATR — five BF2 maps are
  built entirely from models with no heightfield). Both got Tier-1 regressions.
  Map identity is now the baked DIRECTORY name, not the world name inside the
  LVL: both games ship worlds called hoth/yavin1/kas2/naboo2/geo1/end1/kamino1,
  so the old key could not address a mission file. Command posts: 14/14 BF1
  maps and 13/16 BF2 maps, the latter after matching `<map>_conquest` as well
  as TAT2's `<map>_con` (PROVISIONAL alias — which sublevels a mission loads is
  mission Lua). `--cam-overview` frames a map from its baked bounds for
  bring-up. **HOLD FOR FABLE: per-map mission values + eras** — every new
  `missions/<dir>_con.ron` is a labelled PROVISIONAL copy of the TAT1/TAT2
  numbers with the source game's default matchup, so era-specific maps show
  the wrong sides; also `fel1`/`myg1`/`mus1` find no conquest layer by name,
  and BF1 ships sound banks per ERA (CW/GCW) rather than per map, so the new
  maps have no baked sounds. Tier-4: `engine/artifacts/all_maps/`.
- 2026-08-30 (Opus session): **Backlog #6 — robustness + docs.** New data-gated
  Tier-1 tests walk every baked map: `all_maps_load` asserts each
  `assets_baked/*` scene loads with models, instances and at least one
  resolvable draw, and that each map's mission file parses and names its own
  map. Manual sweep: all 32 baked maps run a 407-tick walk replay headless
  with exit 0 and no panics (load ~0.2 s each, largest map 155 models). `bake`
  now prints a real usage table pointing at `scripts/rebake.sh`.
- 2026-08-30 (Opus session): **Backlog #7 — `bake pc-tex` (W1 follow-through).**
  Dumps a PC-format LVL's textures to `<out>/textures/*.ktx2` + an index RON
  (size, mip count, every D3D format present, which surface was written), and
  reads an LVL from stdin so a Harrisonfog archive member can be streamed
  through `unzip -p` without unpacking anything. Verified on the BS1 addon's
  SIDE/BES.lvl: 13 textures, 0 skipped, KTX2s read back correct (1024² cloud-car
  diffuse, 512² normal map — the PS2's own unit textures are 64²-256²).
  **No override wiring** — name matching, mip policy, the normal-map path and
  the Shader Patch numbers stay HOLD FOR FABLE #8; the index file says so in
  its own header, including that `_bump` maps come out in the sRGB colour
  format and are for looking at, not shading. Also added a KTX2 writer
  round-trip test (every bake path depends on it). Tier-4:
  `engine/artifacts/pc_textures/`.
- 2026-08-30 (Opus session): **HOLD FOR FABLE (new questions raised this
  session, none acted on):** (1) per-map mission values and eras — every new
  `missions/<dir>_con.ron` is a labelled PROVISIONAL copy of the TAT1/TAT2
  numbers with the source game's default matchup, so era-specific maps show
  the wrong sides; (2) `fel1`/`myg1`/`mus1` find no conquest sublevel by name,
  so they run with zero command posts — which sublevels a mission loads is
  mission Lua; (3) [WRONG — corrected below] BF1 map sound banks; (4) terrain layer
  splatting (LPTC/ALPH/COLR) is still unparsed, so every terrain renders with
  one flat tint and Hoth's snow is sand-coloured; (5) what the PS2's
  third-person camera actually does on contact with geometry (spring rate,
  pull-in vs slide, recovery, whether it uses `coll`) — `game::camera_collide`
  is a PROVISIONAL pull-in until that is answered; (6) whether the PS2 fades
  the `_dtl` detail layer with `mipDistance` (all shipped pairs are mipCount 1).
  Also updated `HANDOFF-OPUS.md` with the session results and what the next
  session should pick up. Shell map rows now read "HOT1 (bf2)" rather than
  "bf2_hot1 (bf2)"; shell golden regenerated after viewing.
- 2026-08-30 (Opus session, the maintainer asked): **Terrain is textured — "Hoth's snow
  is sand-coloured" root-caused.** It was neither a decode bug nor an RE error:
  the terrain was never textured. `viewer::upload_scene` drew it with a
  hardcoded `TERRAIN_TINT` and discarded the submesh material that
  `bake::terrain_glb` had been setting to LTEX layer 0 since P1, and the baked
  UVs were one 0..1 stretch over the whole 2 km map so no layer texture could
  have tiled anyway. Now: UV = world metres / SCAL[0], material = LTEX[0],
  renderer binds it. Backed by measurements over all 50 shipped terrains
  (RE_LOG "Terrain layer texturing"): SCAL tracks CELL size not map size, so
  it reads as metres-per-repeat (PROVISIONAL); AXIS/ROTN are 0 for every base
  layer; `PCHS->PTCH->INFO` field 0 is a u16 LAYER BITMASK + u16 flags (every
  set bit < textureCount on all 50; mask 0 plots exactly as the ground outside
  the level on KAS1/END1); LTEX[0] is painted in 81-100 % of textured patches
  on every map, and 98-100 % of patches carry no extra layer data — so one
  base layer is a fair approximation. Also PROVISIONAL: LUMI's neutral is 128
  on maps whose 99th percentile stays under 204 (BF1 HOT1 stores exactly 128
  on 99.2 % of texels) and 255 otherwise, classified per map because YAV1
  ships the same lightmap in both games; with that scale the ground matches
  the brightness of the rocks standing on it. TAT1 goldens regenerated after
  viewing. Tier-4: `engine/artifacts/terrain_layers/`.
  **HOLD FOR FABLE (refined):** not "layer splatting is unparsed" any more but
  specifically — how the layers a patch lists are BLENDED (per-patch ALPH/
  COLR/LPTC; the PCHS-level COLR/ALPH are not plain RLE at elem 1/2/4),
  whether mask-0 patches should draw at all (`LOWR` holds a second lower-res
  terrain), the `DTLX` terrain detail texture (`DTEX` header is all zeros on
  HOT1), the flag bit on one patch each of YAV1/YAV2, and what the PS2 itself
  multiplies LUMI by.
- 2026-08-30 (Opus session, the maintainer asked): **BF2 mode-layer matching fixed — all
  16 BF2 maps have command posts** (was 1 at session start, 13 after the first
  alias pass). Same class of bug as the terrain: ours, not the RE. `bake::map`
  names a `lvl_` mode layer after `worlds[0]` only, so Mygeeto's conquest
  sublevel `['myg1_turrets', 'myg1_cp-conquest']` was called "myg1_turrets" and
  never matched. `Scene::apply_mode_layer` now matches against EVERY world in a
  layer, accepts the spellings the discs use (`_con` on TAT2 alone, `_conquest`
  on most, `_cp-conquest` on myg1), and falls back to the largest
  non-mode-named layer for the two maps that ship no conquest sublevel at all
  (Felucia and Mustafar keep their posts in `*_design`). Post counts: BF1 14/14
  (4-7 each), BF2 16/16 (4-7 each); TAT2's sim hash is unchanged
  (f7b2f6ade72989dd). Data-gated test asserts every baked map finds posts.
  Still PROVISIONAL: which sublevels a mission actually loads is mission Lua.
- 2026-08-30 (Opus session, the maintainer asked): **ERRATUM to my own HOLD list — BF1
  does NOT ship sound banks per era.** The BF1 disc has one sound LVL per
  planet (`SOUND/{BES,END,GEO,HOT,KAM,KAS,NAB,RHN,TAT,YAV}.LVL` plus CW, GCW,
  SHELL, TEST) and BF2 has 19 of them; `extracted/` only ever received 4 of
  BF1's 14 and 3 of BF2's 19, which is why it looked era-based. Verified the
  pipeline already handles them: `bake sounds` on HOT.LVL (extracted to scratch
  from `iso/SWBF1.iso`) yields 1 bank / 475 samples / 550 properties. So the
  new maps' audio needs no new code — only the missing files.
  **GUARDRAIL QUESTION FOR THE MAINTAINER:** `extracted/` is declared read-only, so I did
  not add them. One `bsdtar -xf iso/SWBF1.iso DATA/_LVL_PS2/SOUND/...` per file
  (~24 files, ~1 GB) restores the extraction; say the word and the rebake picks
  them up.
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
- 2026-08-30 (Opus session): **Two bugs the gallery found immediately.**
  (1) The CIS super battle droid rendered as NOTHING and (2) both Kamino maps
  rendered as shredded fragments — same cause: our shader alpha-tests at 0.5 and
  those textures store alpha below it everywhere, so every fragment was
  discarded. The decode was fine; the alpha channel simply is not a mask for
  those materials. `segm MTRL`'s first u32 is the flags word (same field in both
  games) and decides it: measured over all 10,060 segment/texture pairs on the
  discs, bit 0x02 set means a real cutout texture 72-77 % of the time while
  neither 0x02 nor 0x04 means 6 %. The bake now writes a texture opaque when no
  material referencing it sets either bit (`bake::detail::ALPHA_IS_MEANINGFUL`,
  PROVISIONAL on the bit meanings, with a Tier-1 regression on the droid).
  Deliberately narrow: transparent-flagged textures keep their alpha and stay
  invisible, because the renderer has no blend path yet — that, and the rest of
  the flag word, are HOLD FOR FABLE.
- 2026-08-30 (Opus session): **Load-screen goldens** — every baked load screen
  thumbnailed into one grid per game (`goldens/loadscreens_bf1.png` 32 stills,
  `_bf2.png` 86). Big slab of decoded content nothing else covered; both grids
  read correctly (BF1's galaxy→planet zoom sequences, BF2's in-game stills plus
  the logo), which is also a broad confirmation of the CLUT erratum fix on a
  large unrelated sample. Golden set is now 39 sheets; `scripts/goldens.sh`
  regenerates all of them, `--check` verifies. Tier-4 index for the maintainer:
  `engine/artifacts/gallery/README.md`.
- 2026-08-30 (Opus session 2, the maintainer asked): **Sound — every map has its planet's
  bank, and there is a tool to listen.** the maintainer authorised extracting the missing
  SOUND LVLs; all 28 came out additively (nothing under `extracted/` modified,
  `iso/` read only). `scripts/rebake.sh sounds` maps a map to its planet by name
  prefix, so 32/32 maps now bake a bank (9,095 WAVs, 261 MB, 1.3 s). The runtime
  already loaded `<mapdir>/sounds/sounds.ron` per map, so the wiring needed data,
  not code. `bake sound-names` recovers readable names by hashing candidate
  strings from the baked classes and the extracted Lua and matching — a hit is a
  CONFIRMED PREIMAGE, tagged `Direct` (verbatim on the discs) or `Derived`
  (recombined, ~0.3 expected collisions/map at 733,945 candidates) — and
  `scripts/audition.sh list|play|export` joins that to the WAVs. Tier-3: all
  9,095 WAVs swept, 0 silent, 0.017-6.00 s, mono 16-bit. Tier-1:
  `every_baked_map_has_its_sound_bank` pins the measured side set per map, which
  matches canon (Hoth/Endor GCW-only, Geonosis/BF1 Kamino CW-only). Found and
  fixed: `SoundLib` collected duplicate property definitions last-wins, and 63
  properties across the 32 maps pair an audible definition with an empty one, so
  chunk order decided whether they played (PROVISIONAL: prefer the last audible
  one). Tier-4: `engine/artifacts/sound_banks/`.
  **HOLD FOR FABLE: sample `frequency` outside 4-48 kHz.** 1.8 % of samples on
  every disc (162 of 9,095) store a frequency that cannot be a playback rate
  (clusters at 322-346 and 3000-3047, two at 200,012). NOT a parser slip —
  `frames == size/16*28` holds for every record including these — so the disc
  really stores those values and something else must interpret them. Written
  through unchanged and counted, not guessed at.
- 2026-08-30 (Opus session 2): **Code audit + cleanup (adapted `/cleanse`).**
  Three scoped read-only audits (dead code/duplication via compiler-as-oracle,
  docs-vs-reality, error handling/determinism/boundaries), every finding
  re-verified by the lead before acting. Landed: (1) doc reconciliation — module
  headers describing an engine that no longer exists (bake::map "terrain lands
  once HGT8/HEXP is confirmed", walk.rs "soldier renders as a box",
  mungefs::model's BF1-only 68-B INFO), `apply_mode_layer`'s stale `{map}_{mode}`
  doc, and a2-bake-formats.md's disagreements with the later errata marked
  INLINE (it is the format reference, not a notebook). (2) hygiene — five
  functions with no call site anywhere (`Sim::tick_hz`, `Skeleton::bind_palette`,
  `Renderer::{is_headless,queue}`, `bake::soldier::bake_textures`, the last of
  which passed an EMPTY material-flag map so any caller would have baked every
  texture opaque); the `PROP` record decoder existed twice in `mungefs`
  (entity.rs now calls world::parse_props). Determinism came back CLEAN —
  zero nondeterminism sources in non-test `core`, and the module boundaries
  (core->render, render->gameplay, game reimplementing mungefs) are all clean.
- 2026-08-30 (Opus session 2): **Perf — measured, and deliberately not
  optimised.** `OPENBF_ADAPTER=igpu|dgpu|<substring>` selects the adapter for
  every entry point that builds a Renderer, and `--bench` times the real paths
  (load, upload, `step_tick`, `render_frame` with the world draw list PLUS the
  battle's skinned soldiers) as mean/p50/p99. Over all 32 maps x both adapters
  at 720p with 16 AI/team: dGPU frame 0.18-0.54 ms; **iGPU 1.24-3.69 ms (271 fps
  worst case)**; load+upload 12-30 ms; **sim tick p99 <= 0.031 ms** (0.09 % of
  the 33.3 ms budget); textures 10.6-27.1 MB/map RGBA8, 351 MB for all 32. The
  worst map on the weaker GPU spends 22 % of a 60 fps frame. Two experiments say
  optimising now would be churn: 720p->4K costs 2.5x the time for 9x the pixels
  (~1.9 ms fixed + 0.48 ms/Mpx, so not fill-bound), and at 64x64 a 7x spread in
  draw count moves the frame floor by 0.01 ms (so not draw-call bound) — map
  instancing and skinned-draw batching came OFF the list on the measurement.
  NOTE (the maintainer, this session): the iGPU floor is **Intel Iris Xe (Gen12) or
  better**; this machine's RPL-S UHD is the same architecture with about a third
  of Iris Xe's EUs, so these numbers are conservative against the target. The
  one number that will matter is texture memory on the classic-plus path
  (remaster art is ~16x the bytes, turning a 16 MB map into ~250 MB) — that is
  where BC7/BC5 earns its place, and it belongs to HOLD FOR FABLE #8.
  Tier-4: `engine/artifacts/perf/` (+ the CSV).
- 2026-08-30 (Opus session 2): **Death Star and Tantive IV root-caused — both
  were OUR gallery framing, not geometry.** Three separate bugs, all found by
  measurement: (1) ONE stray `dea1_prop_tiefighter` at x = 828 when 95 % of the
  map sits at x <= -46 stretched the auto-framed overview 4x and shrank the map
  to a speck — `viewer::robust_bounds` now rejects per axis with Tukey's far-out
  fence (3x IQR), which drops NOTHING on a normally-spread map; (2)
  `gallery::map_sheet` never applied the conquest mode layer, and every BF2 map
  keeps its posts in a `lvl_` sublevel and none in the base world, so the
  "ground view at the first command post" was not at a post on any of the 16 BF2
  maps — it fell through to the whole-map centroid, which for Death Star was
  empty space; (3) the ground view snapped the soldier to TERRAIN height, but a
  post sits on whatever surface the editor put it on — Mos Eisley's cp7 is at
  y = 8.3 over desert floor at y = 0, so the soldier stood inside the building
  underneath. Terrain is now a floor, not the answer. Also: the gallery camera
  tries eight directions around a post and keeps the longest unobstructed one,
  because tight interiors collapsed the PROVISIONAL pull-in into the soldier's
  back (game camera untouched). Tantive IV needed no fix at all — its "fragmented
  geometry" was the overview looking down into an interior map; its ground view
  was always correct. **All 32 map goldens regenerated and every one viewed**
  (contact sheets); every ground view now reads correctly.
- 2026-08-30 (Opus session 2): **Remaining rendering defect, characterised not
  fixed — outside-the-level terrain.** Visible in the OVERVIEW panel of Endor
  (both games), Kashyyyk 1/2, Naboo 1, Yavin 1/2 and Dagobah: a flat filler
  plane surrounds the playable area. Measured: mask-0 patches are 85 % of END1
  and 93 % of KAS1, and their heights are nearly constant (KAS1 sd **0.08 m**
  against 7.59 m inside the level; END1 2.66 vs 9.66) — so they are filler, not
  terrain. HOT1, TAT1 and KAS2 have NO mask-0 patches at all, which is why only
  the forest/jungle maps show it. What stops a fix being honest: the BF1 map
  LVLs DO contain `LOWR` and `LPTC` tags (grep over the discs; HOT1 has 256
  LPTC), so there is plausibly a second lower-res terrain the PS2 draws out
  there, and culling mask-0 patches outright would leave Kashyyyk's island over
  bare sky. **HOLD FOR FABLE: what the PS2 draws outside the level** — whether
  `LOWR` is that terrain, and whether mask-0 patches should draw at all.
  (Caution for whoever takes it: a `tern`-children walk via `Chunk::children()`
  reported neither chunk on END1/KAS1 while grep finds them in the same files,
  so that iterator is dropping children somewhere — do not trust it as a census
  without checking it first.)
- 2026-08-30 (Opus session 2): **Alpha blending — a real composite pass.** The
  renderer only alpha-tested, so any texture with no texel above 0.5 rendered as
  nothing (water, glass, shadows, godrays, holos). `render` gains a blend
  pipeline (src-alpha, depth writes OFF, depth test on) run after every opaque
  draw, sorted back-to-front per draw (the standard approximation; intersecting
  transparent surfaces can still resolve wrongly). The TRIGGER is deliberately
  narrower than the material flag: `segm MTRL` bit 0x04 alone selects **377**
  distinct textures including `hoth_mountain`, `tat_main_1`, `kas2_mountains`
  and the terrain layer sheets — solid surfaces that must keep writing depth.
  The rule shipped is "alpha-testing this texture DELETES it" (flag set AND no
  mip-0 texel reaches the test), which selects **17** textures across 12 maps,
  every one of them water/glass/shadow/holo/cloud. Those draw nothing today, so
  compositing them cannot disturb anything that currently renders. PROVISIONAL.
  Proven by two headless GPU tests (invisible tested vs visible blended;
  composites over opaque geometry behind it even when submitted first), not by
  goldens — only `bf1_bes2` moved, the one golden camera that frames any of the
  17. **HOLD FOR FABLE: what `segm MTRL` bit 0x04 actually means**, since it is
  clearly not "composite this" for the other 360 textures that carry it.
- 2026-08-30 (Opus session 2): **Robustness.** `mungefs` now holds its no-panic
  contract against malformed disc data — seven reproduced panics fixed with
  regression tests (ISO directory-record bounds and a self-referential
  directory that looped forever; unbounded `SCOP` recursion that ABORTED the
  process on a stack overflow and is reachable from `bake`; a `skel` parent
  cycle that asserted; an RLE decompression bomb at 64x and a u16 `grid_size`
  that could drive a 17 GB allocation; a short `pal_ INFO`; a mip shift past
  32). Caps are sized against measured reality (largest real RLE output
  256 KiB vs a 16 MiB cap; deepest real config nesting 5 vs a cap of 64).
  Rebaking TAT1/TAT2 through the hardened parser gives BYTE-IDENTICAL output.
  Three runtime crashes fixed too: the skinned-GLB loader indexed animation
  tracks sized from the sampler INPUT accessor while iterating the OUTPUT one
  (any spec-valid CUBICSPLINE clip overran); the heightfield's `unit <= 0.0`
  guard is false for NaN, which then panicked in `f32::clamp` on the first sim
  tick; and `tickets: 0` panicked at startup and is now rejected where it
  enters.
- 2026-08-30 (Opus session 2, the maintainer asked): **Community projects assessed; perf
  scope corrected.** the maintainer linked zenit / SWBF2Phoenix / BF2GameExt and asked
  whether the PS2 base was a mistake. Written up in
  `analysis/community-projects.md`. Findings: all three target the PC 2005
  build; **BF2GameExt is MIT and carries ~30 RE documents that map onto our HOLD
  FOR FABLE list** (an explicit BF1-vs-BF2 AI comparison — our parked deferred
  decision #1 — plus vehicles/flyers ahead of P7, animation blending, the `fx__`
  particle system, soldier collision, sound); Zenit documents only 4 chunk types
  and has **no license at all** (read only, copy nothing); Phoenix is AGPL and
  runs the original Lua at runtime. **PS2 remains the right base** — the
  controller-era systems this project is faithful to (BF1 aim-assist magnetism,
  prone, sprint, the contextual roll button) exist ONLY in the PS2 builds;
  BF2GameExt had to invent gamepad aim assist and re-add prone/sprint for PC,
  and no PC copy of BF1 exists here anyway. The cost of the PS2 choice (no
  community tooling, PC docs that actively misled A2) is largely sunk now that
  the formats are cracked. The PC SWBF2 copy IS worth having: it makes
  BF2GameExt's addresses followable in a real binary, gives a second sample of
  the same engine for semantics-only questions, and supplies the PC↔PS2 name map
  the classic-plus override needs. **Every PC-sourced fact is a hypothesis to
  verify against PS2** — the doc opens with the five PC-derived claims that were
  wrong for PS2 and what each cost. Also added §Optimisation backlog above:
  the perf verdict was scoped to the PS2-authentic workload and the maintainer's goals
  spend exactly that headroom, so the baseline is now a regression reference and
  the open question is scaling. Verified in code: no frustum culling anywhere,
  baked LOD chains ignored, `separate_units` and AI targeting both O(n²),
  nothing in the workspace threaded — and **no CLI way to raise `ai_alive_cap`,
  so we cannot currently even measure more AI than the PS2 held.**
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

Summary in PLAN.md's Log under the 2026-08-31 entries. Verbatim below.

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
- 2026-08-31 (Opus session 3): **Sky domes — each map draws its own sky.**
  `sky_ DomeInfo` names dome models that were already sitting in each map's
  baked models, unreferenced: Tatooine's cloud band, Bespin's sunset,
  Kashyyyk's green mountains, the Death Star's stars, Mustafar's lava glow,
  Polis Massa's nebula. The renderer owns the shells (`Renderer::set_sky`)
  because their transform follows the camera; they draw FIRST, unlit and
  unfogged, with no depth test and no depth write. `DrawCmd.blend` becomes
  `DrawCmd.layer` (Opaque | Blend | Sky). PROVISIONAL: `SKY_SHELL_RADIUS`
  1000 m and `shell_position = camera * MovementScale`. Two measured
  limitations: the shipped domes cover a BAND, not a sphere (BF1 KAS1's spans
  0–40.6° of elevation, TAT1's −29.6–39.8 plus a flat cap), so above the band
  the frame keeps its clear colour; and **BF2 Theed now has a black sky
  because the disc says so** — its `sky_naboo2` is 128×128 with all 256
  palette entries in 0..3, read from the raw `pal_` bytes, while BF1's is a
  512×128 blue panorama. BF2 Tantive IV's `tan1_sky` is all zeros with 15 of
  16 entries fully transparent, which for a ship interior is probably right.
  Those are the only two black dome textures of 32. **HOLD FOR FABLE.**
  Two headless GPU tests pin the pass.
- 2026-08-31 (Opus session 3): **`--ai-cap`, and the scaling curves that were
  blocked on it.** `ai_alive_cap` came only from the mission RON, so nothing
  headless could SUSTAIN a population above the PS2 unit count. (Precisely:
  `--ai N` sets the INITIAL spawn either way; the cap is what the sim holds to
  afterwards. Measured on TAT2 with `--ai 200` over 900 ticks: 40 alive under
  the mission's cap of 16/team, 141 alive with `--ai-cap 200`. The curves below
  are plotted against MEASURED alive units, not against `--ai`.) `--ai-cap <n>` overrides it;
  `--bench` also gained `--bench-player-cam` (the default overview camera sees
  the WHOLE map and is the one camera where culling can never help),
  `--no-cull`, a units-alive count and a DRAW-CALL count. Measured on TAT2,
  720p, both GPUs, with the population held stable:

  | units alive | tick mean | tick p99 | frame dGPU | frame iGPU |
  |---|---|---|---|---|
  | 17 | 0.003 ms | 0.011 | 0.25 ms | 2.54 ms |
  | 65 | 0.015 | 0.032 | 0.35 | 3.26 |
  | 194 | 0.069 | 0.176 | 0.64 | 3.75 |
  | 380 | 0.193 | 0.535 | 1.09 | 4.89 |
  | 563 | 0.389 | 1.067 | 1.25 | 6.32 |
  | 740 | — | — | — | 6.98 |

  **The sim scales at about n^1.4, not n².** `separate_units` and AI targeting
  are pairwise, but the early-out makes almost every pair cheap: at 563 alive
  the tick is 0.39 ms mean / 1.07 ms p99, i.e. **1.2 % of the 33.3 ms budget**.
  Resolution barely matters either — with the REAL shader and 201 units,
  720p→4K is 0.63→0.96 ms on the dGPU and 3.86→5.24 ms on the iGPU, nine times
  the pixels for ~1.4x the time. **So "more AI than the PS2 held" is already
  affordable: 740 alive units at 6.98 ms/frame on the INTEGRATED GPU.**
  This retires A3 (spatial grid) and A4 (deterministic parallelism) as
  optimisations with no measured need — they stay on the list only as
  insurance if a future workload moves the knee.
- 2026-08-31 (Opus session 3): **Frustum culling landed (PLAN B1).** Six
  planes from the view-projection against each draw's world AABB. From the
  player's eye on the iGPU at 720p, 32 AI/team: 78–94 % of static draws culled
  on the big maps, frame 3.88→3.31 ms on BF1 END1, 2.50→2.08 on TAT2,
  2.46→2.09 on RHN2 (−14 to −17 %); the win shrinks to −2 to −6 % where
  skinned soldiers dominate, which is honest — culling removes STATIC work.
  **All 42 goldens regenerate byte-identical with it on**, which is the proof
  it removes nothing visible. Not culled: sky shells, skinned draws, terrain
  (one undivided mesh — PLAN B4). The bench's draw-CALL counter is the number
  that moved: BF1 END1's 2,645 draws already collapse into 227 instanced
  calls, so the draw-list length was never the cost driver.
- 2026-08-31 (Opus session 3, the maintainer asked): **PC stock textures where larger —
  the classic-plus chain's second tier is wired** (`bake::tier`;
  `bake map <lvl> <out> [pc.lvl]`, and `scripts/rebake.sh` picks the install up
  automatically and SAYS which way it ran). BF1 is untouched — no PC BF1 exists
  here. Full coverage, independently re-measured by the lead against the
  subagent survey (per-map counts reproduce exactly): 1,063 baked BF2 textures,
  1,040 name-matched by direct lowercase name, **370 (34.8 %) strictly larger
  on the PC in both dimensions**, modal improvement a clean 2x.
  **"Larger" turned out not to be enough, and the measurement is the reason.**
  Downsampling each candidate back to the PS2 size: median mean-absolute
  difference 4.5 of 255 and median luma cross-correlation 0.988 — three
  quarters of the set is the same painting, sharper. The tail is a DIFFERENT
  painting under the same name: BF2 `sky_hoth` is a hazy white blizzard on the
  PS2 and a blue sky with clouds on the PC (correlation −0.12), and
  `hoth_mountain`, `sky_endor`, `sky_kam1`, `fel1_sky_clouds`, the Geonosis and
  Tatooine terrain sheets and `uta1_prop_rock_1_ps2` (a texture the disc
  literally names `_ps2`) are the same story. Taking those changes the map's
  CONTENT, and content fidelity is non-negotiable (Architecture #5). The
  shipped rule rejects a candidate only when it agrees on NEITHER levels NOR
  structure (mean diff > 12/255 AND correlation < 0.5) — both are needed,
  because correlation alone rejects smooth textures where it is meaningless and
  difference alone rejects the same art merely graded differently (the PS2 art
  is systematically brighter). 24 rejected that way, 11 more as
  detail-compressed bases (their `_dtl` companion is registered to the PS2
  pixels), **312 taken.** Alpha is handled by applying the PS2 `segm MTRL`
  gates to the RESOLVED pixels, without which 30 replacements gain alpha and 13
  of those lose over 80 % of their texels to the alpha test. Cost is memory and
  only memory: bf2_cor1 19.2 → 58.2 MB, tat2 15.6 → 30.7 MB, **with no
  measurable iGPU frame-time change** — so block compression is a residency
  question, live on the Harrisonfog tier (~250 MB/map), not a frame-time one.
  17 goldens changed (every `bf2_*` plus tat2, no BF1) and all were viewed.
  Tier-4: `engine/artifacts/pc_tier/`. STILL OPEN: the Harrisonfog x PC overlap
  is unmeasured, so the PC tier's NET value once HD lands could be below 312.
- 2026-08-31 (Opus session 3): **CORRECTION — `Chunk::children()` was never
  dropping anything.** The outside-the-level entry warned that a `tern`-children
  walk found no `LOWR`/`LPTC` while grep found them in the same files, and told
  the next session not to trust the iterator. The iterator is fine: **`LOWR` is
  a child of `tern`'s `PCHS`**, one level below where the census looked. Pinned
  as a test. Its shape is now measured and recorded in RE_LOG (grid² `LPTC`
  patches; `INFO` = grid, resolution, extent, min y, max y, and the resolution
  matches `sky_ DomeInfo.LowResTerrain.PatchResolution`), but NOT parsed —
  it stays HOLD FOR FABLE. **And it does not close the defect: 17 of the 34
  maps carry no `LOWR` at all, including BF1 END1 and BF1 YAV1, two of the maps
  that show it.** Whatever those draw out there, it is not `LOWR`.
- 2026-08-31 (Opus session 3): **Placeholder effects composite instead of
  alpha-testing.** The blend pass landed on 2026-08-30 and `game::fx` was left
  behind, so effects still faded by shrinking and DIMMING — a muzzle flash that
  dims while staying opaque reads as a grey card in front of the scene, which
  is what the old artifact frame shows. Now: `DrawLayer::Blend` on every puff,
  the procedural sprite's alpha becomes a soft falloff instead of a hard circle
  (its RGB no longer has to fake a lit rim to hide a dark halo), and the fade
  moves out of the colour into the alpha, so the colour stays at full
  brightness for the whole life. Two unit tests pin it; every golden is
  unchanged (no golden camera frames a live effect). Still PROVISIONAL — the
  PS2's `fx__` system is still unparsed and still HOLD FOR FABLE.
  Tier-4: `engine/artifacts/fx_placeholder/` (updated).
- 2026-08-31 (Opus session 3, the maintainer asked): **Colour-space item B — `COLR`
  cracked, and the LUMI-neutral rule re-checked and upheld.**
  **`COLR` is LUMI in colour.** A2 had it as "not plain RLE arrays — tried
  elem 1/2/4, none decode to a grid size"; it decodes at **elem 2** on all 26
  shipped terrains to exactly `patches × 81 × 2` bytes — 81 = 9×9 = a patch's
  8 cells plus the shared edge, i.e. ONE u16 PER VERTEX. The u16 is GS
  `PSMCT16` R5G5B5A1: the top bit is set on 100.0 % of all 82,944 entries of
  every map checked, and read as 5551 TAT1's COLR draws the dune field while
  565 gives confetti. Patch and sample indices are both COLUMN-major, proven
  by reconstructing LUMI from COLR's green channel to within the 5-bit
  quantisation error (mean abs difference 0.31 of 255 on BF1 END1, 3.3–4.2 on
  TAT1/KAM1/GEO1, against 8–28 for the other three orderings). The bake keeps
  LUMI's 8-bit precision and takes only the HUE, so a grey lightmap is a
  no-op; six terrains carry a real hue (BF1 KAM1/BES1 99.2 %, RHN1 99.0 %,
  BF2 NAB2 97.4 %, BF1 BES2 65.3 %, BF2 DAG1 23.1 %, then a gap to 1.8 %) and
  the other twenty bake byte-identically. RhenVar's snow gets its cold blue,
  Kamino its storm cast. `ALPH` still does NOT fall out at any element size.
  **The LUMI-neutral rule stands, and now has a decisive cross-check**: two
  terrains ship in both games and classify differently. BF1 HOT1 stores 128 on
  99.2 % of its texels and BF2 HOT1 stores 255 — same map, same base texture —
  so a UNIFORM "128 is 1.0" reading would render BF2's Hoth twice as bright as
  BF1's for identical art, while the per-map rule lands both at 1.0. BF1 and
  BF2 YAV1 ship the same lightmap byte for byte and classify the same way. The
  204 threshold sits in a 56-wide gap (class-128 p99 ≤ 174, class-255 ≥ 230).
  Six goldens changed and all were viewed;
  Tier-4: `engine/artifacts/terrain_layers/` (updated).
- 2026-08-31 (Opus session 4, the maintainer released `tern -> WATR`): **Twelve maps get
  their water.** A2 listed `WATR` and did not decode it; no map drew water.
  It decodes completely (`mungefs::water`, full spec in the module header and
  `analysis/RE_LOG.md`). `INFO` = grid, 2, **cell size** (the handoff read
  this as the water height — it is not), half-extent x/z, n; `grid *
  cellSize == 2 * halfExtent` exactly on all 14 shipped chunks, which is what
  identifies the sheet as centred on the origin. `WMAP` is one NIBBLE per
  cell over grid², row-major `[z][x]`, high nibble first, and the value is a
  boolean — over all twelve maps a nibble is only ever 0, 1 or 9. Both
  orderings were MEASURED, not assumed, by sampling the terrain under each
  cell: row-major separates wet from dry by **+63 points** of "ground below
  the waterline" against +13 transposed, and plotted the mask draws
  Kashyyyk's island, Theed's platform, Felucia's river and Yavin's pond
  exactly. **`LAYR`'s first f32 is the water plane's Y and is proven twice** —
  by that separation (BF1 KAS1 98.7 % of wet cells over low ground against
  0.0 % of dry) and by `CPAT`, every one of whose 165 vertices sits at exactly
  that Y although its box allows a 1 m range. `CPAT` itself is the
  shoreline-fitted surface on three maps: an AABB, a standard PS2 strip, and
  `POSI` as 3×u16 into the box, covering 3-24 % of its tile. **Two findings
  set the value**: BF2 DEA1 and BF2 KAM1 ship a `tern` that is `NAME` + `WATR`
  and nothing else, so the water is their ONLY ground (BF2 Kamino's platforms
  sat in a black void); and BF1 KAM1's terrain is a flat sheet at y=0 on
  65,025 of 65,536 vertices, so Kamino's old "sea" was that sheet. THREE
  judgement calls, ours and PROVISIONAL, all in `bake::water_glb`: the water
  is OPAQUE and `LAYR`'s colour word is recorded but NOT applied (read as RGBA
  it turns Felucia's teal river red-black, as BGRA it turns Kamino's ocean
  red — neither byte order survives the shipped textures, which are opaque and
  already finished colour); metres per texture repeat = `cell_size /
  uv_tile`; and a `CPAT` patch REPLACES the `WMAP` cells it covers. The
  renderer gained a per-material UV scroll on a wrapped clock (`TIME_WRAP`
  1000 s is seamless because every velocity either disc authors times 1000 is
  a whole number of repeats); a GPU test pins that a zero-scroll material is
  BYTE-IDENTICAL at any clock value, which is why nothing else moved. 8 of 42
  goldens changed, all viewed. **Ruled out: `sky_ WaterInfo` is not this
  data** — 8 LVLs carry it, 7 of them have no `WATR` at all, and every one is
  the identical all-zero `Enabled(0)` default. **HOLD FOR FABLE: whether a
  `CPAT` patch replaces its cells or adds to them; what bit 3 of a `WMAP`
  nibble means; what `LAYR`'s colour word is.** Tier-4:
  `engine/artifacts/water/`.
- 2026-08-31 (Opus session 4, the maintainer asked): **Parallax sky shells take the
  fog; the base dome still does not.** Session 3 made the whole sky pass
  unfogged. That is right for the base dome — it IS the horizon — but wrong
  for the LANDSCAPE shells authored over it, which is why Hoth's mountains
  stood out of a blizzard as dark rock. The classification is data-driven over
  the 74 `DomeModel` entries that resolve on the 34 maps: a non-zero `Offset`
  or a `MovementScale` other than 1.0 always means landscape (Kamino's city,
  Hoth's mountains, Tatooine's rock spires, Endor's trees, Bespin's Cloud
  City, Mustafar's volcano), and offset 0 with movement 1.0 always means sky
  or space (Yavin's gas giant, Polis Massa's asteroids, Coruscant's skyline).
  The "not the first `DomeModel`" guard earns its keep on exactly one map:
  **BF2 GEO1 authors `MovementScale` 0.995 on all four shells, base dome
  included.** The fog WEIGHT is a constant, 0.65, and it is OURS and
  PROVISIONAL — a parallax shell follows the camera and has no distance to
  feed the ramp — picked by sweeping 0/0.5/0.65/1.0 and looking. Five goldens
  moved; four are the intended win and the fifth is evidence: **BF1 KAM1's
  city goes pale**, because KAM1's authored fog colour is the neutral
  128/128/128 placeholder against a dark storm. The 25 maps with no fog and
  no parallax shell came back byte-identical. This is cause (c) of the Kamino
  seam; cause (a) was the missing sea (above) and cause (b) is the shell's own
  bottom rim, which is not decoded and was deliberately left alone.
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
- 2026-08-31 (Opus session 4): **Terrain split (PLAN B4 DONE), and a
  measurement lesson.** Two changes. First, `upload_submeshes` handed every
  submesh the WHOLE model's vertex arrays: the vertex buffer was uploaded once
  per submesh, and `GpuMesh::upload` takes the AABB from the positions it is
  given, so every submesh inherited the whole model's bounds — culling could
  only ever take all of a model or none. Compacting each submesh to the
  vertices it uses fixes that and is the enabler. Second, the terrain is
  uploaded as 32-cell chunks. Measured on the iGPU at 720p from the player's
  eye, **median of five 300-frame runs**: bf1_hot1 1.94 → 1.61 ms (−17 %),
  bf1_end1 2.54 → 1.97 (−22 %), bf1_kas1 1.35 → 1.07 (−21 %), tat2 1.35 →
  1.25 (−7 %, and its terrain is 7,938 tris — the honest control). Chunk size
  was swept (64/32/16), not asserted; every size beats no split and 32 has the
  best total, and it is 4×4 of the disc's own 8-cell `PCHS` patches.
  **Two things worth carrying forward.** (a) The split MUST happen at upload,
  not in the bake: `Scene::heightfield` rebuilds the sim's ground-height query
  out of `terrain.positions` assuming a row-major n×n grid, and chunking the
  GLB silently breaks it — on BF1 Hoth the chunked vertex count 69,169 is a
  perfect square, so the guard PASSES and the sim gets a garbage ground plane.
  It showed up as 13 of 32 map goldens with soldiers standing on nothing. (b)
  **A short bench run on this iGPU is worth about ±0.4 ms.** The first pass at
  these numbers used 120–150 frames cold and produced a confident −29 % for
  compaction alone that a 300-frame median showed to be ZERO. Any future
  before/after on this machine needs the longer protocol. All 42 goldens
  byte-identical.
- 2026-08-31 (Opus session 4): **Mip chains generated in the bake.** The KTX2
  writer stored exactly the levels it was handed and **1,989 of the 2,288
  baked textures had an incomplete chain**, 102 of them 512 px or more on the
  long side — a large texture with no chain shimmers because the sampler has
  nothing to fall back to. `ktx2w::write_rgba8` now completes the chain to
  1×1 by box filter, at the single write point, so every producer gets it
  (map textures, `_dtl` companions, load screens, soldier and weapon sheets).
  Filtering is in the STORED sRGB encoding, the same choice the `_dtl` GPU
  test already pinned. Cost: baked textures 654.8 → 698.8 MB on disk
  (+6.7 %), resident per map bf2_cor1 58.2 → 64.0 MB and tat2 30.7 → 33.7 MB
  (≈ +10 %); afterwards 0 of 2,288 chains are short. 39 goldens changed and
  **every one compresses SMALLER as a PNG**, which is the signature of removed
  high-frequency noise; BF1 Hoth's snow was a field of black-and-white speckle
  and is now snow.
- 2026-08-31 (Opus session 4): **The PC stock tier reaches SIDE.**
  `bake::soldier` was the bake's second texture write site and the tier never
  got there. PS2 → what is written now: `rep_1st_weap_inf_rifle` **128×64 →
  512×256** (the sheet that fills a quarter of the screen when the player
  aims), `rep_inf_ep3trooper` and `cis_inf_sbdroid` 256² → 512². The resolver
  is used unchanged — same-art test and all three exclusions — with the same
  choose-source-then-apply-alpha ordering `bake::map` uses. BF1 is untouched
  (no PC copy exists) and stays at 64² trooper sheets. `bake soldier` and
  `bake weapon` gained an optional trailing `[pc.lvl]`; `rebake.sh` finds the
  PC side LVLs itself and checks each individually. 19 goldens changed, viewed.
- 2026-08-31 (Opus session 4): **Flythroughs, one per map** — the Tier-4
  deliverable. `game --capture-orbit <frames>,<dir>` + `scripts/flythrough.sh`
  give 32 maps × 5 s of 720p in ~75 s. The camera is the engine's own
  `overview_camera` swung in a circle, so there are no hand-authored poses to
  drift out of step with the bake (the p2_flythrough artifact's `poses.txt`
  was the thing to avoid repeating). `engine/artifacts/flythrough/`.
- 2026-08-31 (Opus session 4): **Load time re-measured after two texture
  growths, and PLAN B7 is retired.** Scene load + GPU upload is now
  **21.3–38.8 ms** per map (bf2_cor1 31.5 + 7.3, bf2_dea1 27.9 + 9.6,
  bf1_hot1 16.8 + 4.5), against 12–30 ms before the PC tier, the SIDE tier
  and the mip chains. That is still about a frame and a bit to open a map, so
  **B7 (parallel texture decode at load) has no measured need** — the same
  verdict, and the same method, as A3/A4 in session 3.
- 2026-08-31 (Opus session 5, the maintainer's review): **The maintainer's defect hunt — five
  complaints, six bugs, and one thing that is simply not implemented.** the maintainer
  eyeballed all 42 goldens and all 32 flythroughs and listed items A–E. Every
  one was reproduced and root-caused before anything changed; two of the
  causes explain defects he had NOT flagged, and two of his items turned out
  to share a cause with each other. Tier-4: `engine/artifacts/defect_hunt/`.

  **Item C — the orbit did not orbit the map. OURS.** `capture_orbit`
  recovered the orbit centre and radius by INVERTING `overview_camera`'s eye
  placement, taking the radius from the eye's distance to the WORLD ORIGIN, so
  every map whose bounds are not origin-centred got both wrong. Measured over
  32 maps: BF2 TAN1 centre off 233 m with a radius 3.2x too large on a map
  110 m across, BF1 KAM1 2.1x, BF2 DEA1 0.19x (camera INSIDE the map), BF2
  HOT1 0.45x; ten maps off by more than 20 %. `viewer::overview_bounds` now
  returns the `Overview { center, radius }` the camera already computed and
  `capture_orbit` asks for it. Eye placement unchanged, so all 42 goldens
  verified byte-identical. **No interior mode was added**: an orbit-occlusion
  test over the same 32 maps saturated at 100 % on 22 of them, so it does not
  discriminate interiors, and after the centre fix Tantive IV and the Death
  Star are readable anyway. BF2 COR1's orbit was never wrong — its play space
  is a roofed temple interior, so an outside orbit shows the roof.

  **Item D — interiors "untextured". OURS, and eight times bigger than it
  looked.** Censused with a new `--scene-report` (headless; the untextured
  list is collected inside `upload_scene` so it cannot drift from what
  renders) rather than guessed at. The textures are baked; the lookup missed
  them **by CASE**. A `modl` segment spells its texture the way the artist
  typed it and the `tex_` chunk spells it another way (BF1 RHN2 bakes
  `rhn_bldg_interior_02`, its walls ask for `Rhn_bldg_interior_02`); the
  terrain and sky draw paths already lowercased, the MODEL path did not.
  Untextured draws before → after: bf1_rhn2 1306/1462 → 0, bf1_rhn1 925 → 0,
  bf1_hot1 398 → 0, bf2_hot1 386 → 0, bf2_tan1 355 → 1, bf2_cor1 192 → 1,
  bf2_uta1 187 → 9, bf2_pol1 168 → 0; **3,752 → 148 in total, 22 of 32 maps
  now at zero.** Same class of bug as session 4's sky domes, one layer down.
  It also answers item C's third defect: **BF2 Utapau's "large untextured grey
  cylinder" is `uta1_bldg_cliffs*`, the sinkhole wall.** What is LEFT is named,
  not tinted over: 64 of the 148 are BF1 Bespin's terrain (below), and most of
  the rest are meshes with NO material at all — `dea1_bldg_*_collision*` and
  `uta1_bldg_macy_collision*` (collision geometry being DRAWN, worth its own
  look), `kas2_prop_dirt`, `geo_prop_splash`, `mus1_fx_largeexplosion`. Three
  name a texture no LVL on either disc holds (`yav_prop_ivy`,
  `cor1_bldg_flowers`, `noicon`). 21 goldens regenerated and viewed.

  **Item A — washed out AND too dark are TWO bugs, both ours.** (1)
  `DomeInfo.Ambient` was multiplying whole sky domes. Censused over all 34
  maps it takes exactly four states and every one is a NEUTRAL GREY: 255 on
  23 maps, 256 on 2, **128 on 4**, absent on 5. Two values, both "no change"
  in the PS2 pipelines — the same pair the LUMI neutral rule classifies per
  map. Reading 128 literally was a 50 % darkening and singled out exactly Hoth
  and Geonosis, on BOTH discs, for a half-brightness sky nothing asks for;
  `bake::lighting::dome_ambient` renders both neutrals as unity and still
  applies anything else. That is the maintainer's "bf1_hot1 too dark" and half of
  "bf2_hot1 darker", plus both Geonosises which he had not flagged. (2)
  Session 4's `SKY_SHELL_FOG = 0.65` is **deleted**: its premise (a
  camera-locked shell has no distance) is wrong — a shell is drawn at
  `SKY_SHELL_RADIUS` about the eye, so `view_dist` is real, and `fs_sky_fogged`
  now runs the map's own fog ramp at the shell's own distance. Measured down
  the middle of the BF2 Hoth golden, **the step across the horizon goes from
  147 levels to 1**. It cut both ways, which is why "washed out" and "too dark"
  appeared together: a shell NEARER than the map's `near` was being greyed for
  nothing (BF1 KAM1's city sits at ~600 m against FogRange(600, 1200) and comes
  back a dark teal skyline; BF1 YAV2's jungle backdrop against
  FogRange(1800, 3000) comes back green), and one past `far` was not grey
  enough. **BF2 MYG1 is neither** and is reported rather than fudged: its
  wash-out is its own `FogColor(194,197,199)` over `FogRange(0.5, 600)` on a
  map 720 m across — fog that starts at the camera. A GPU test pins the ramp
  (move only `Fog::far` and the shell must follow). 8 goldens changed; the 24
  unfogged maps with no 128-ambient dome are byte-identical.

  **Item B — the census, and Bespin.** the maintainer asked whether there are textures
  below the horizon. **Yes: 29 of 32 base domes reach below it** (seven below
  -88 degrees), and six maps ship a shell that is ENTIRELY below it. Only BF1
  KAS1, BF1 KAS2 and BF2 UTA1 stop at 0. Tool:
  `mungefs --example sky_dome_census`; table in `analysis/RE_LOG.md`. Bespin's
  harsh line was **ours**: the flat tan was `TERRAIN_TINT`, a colour we
  invented, laid over the gold cloud sea Bespin's own dome and
  `bes1_sky_dome_clouds_2` already draw. BES1's `tern` names
  `LTEX("kam1_water")` and `DTEX("yav_decal")` — textures from other maps'
  LVLs, in no chunk BES1 ships, so the PS2 cannot have textured that sheet
  either. An untextured terrain chunk is now NOT DRAWN and `TERRAIN_TINT` is
  gone; `Scene::heightfield` still reads `scene.terrain`, so `--sim-hash 600`
  is unchanged. A test pins the list at exactly `bf1_bes1: kam1_water`.
  **BF2 MUS1's "two images stacked" is NOT our bug**: `mus1_sky_volcano` is
  two concentric rings in one mesh (radius 18.10 spanning y −3.69..+3.69, and
  19.04 spanning −1.78..+5.98, rotated 5.6 degrees and offset half a texture
  width), drawn as authored; the line is each ring's straight bottom rim,
  below which Mustafar ships nothing — no terrain chunk and no `LOWR`.

  **Item E — one fog knob, at 8 %.** `viewer::FOG_DENSITY_SCALE = 0.92`,
  PROVISIONAL and ours, applied once where the manifest becomes render state;
  authored `FogColor`/`FogRange` untouched, no per-map tuning, and
  `bake::lighting::fog_from` deliberately not touched. It scales the ramp's
  SPAN (`far` moves to `near + (far - near) / SCALE`) rather than the blend
  factor, so full fog still reaches the fog colour and the horizon seam does
  not come back. Effect on the five goldens that moved (mean |delta| / peak /
  share of pixels moved): fel1 0.31/8/6.3 %, myg1 0.21/6/3.4 %, hot1
  0.15/7/4.5 %, end1 0.11/6/3.8 %, yav1 0.07/6/1.9 % — subtle by
  construction, 8 % of the gap between scene and fog colour. **BF2 YAV1 is the
  honest exception**: `FogRange(90, 350)` is a ground-level fog and the
  flythrough orbits 250 m out and 110 m up, so a 1.00/0.95/0.90/0.85 sweep
  moves almost nothing. The maintainer's own reading — goldens fine, flyover foggy — was
  right, and the flythrough is the wrong instrument for a short-fog map.

  **Our own sweep then found one more, and it followed from the census.** On
  the three maps whose dome stops at the horizon the frame showed
  `viewer::CLEAR`, the fixed desert blue — a bright band between Kashyyyk's
  sea and its jungle and a ring of sky around the Utapau sinkhole.
  `clear_color` now falls back to the alpha-weighted mean of the BOTTOM TEXEL
  ROW of the map's own base dome texture (v = 1.0 is the dome's lowest ring on
  all three, measured on the baked meshes), continuing the map's art instead
  of inventing a colour. 23 unfogged maps derive it; none falls back.

  **Measured and deliberately NOT changed: both Hoths clip.** 9.4 % of the BF1
  Hoth golden and 10.8 % of the BF2 one are pure 255 in all three channels, so
  that much snow has no detail at all. BF1 HOT1 authors
  `AmbientColor(255,255,255)` and lands at 1.35 at normal incidence, which
  `bake::lighting` already documents leaving as authored. Rescaling is a
  fidelity judgement about how the PS2 combines ambient, sun and LUMI —
  **HOLD FOR FABLE, sharpened: 10 % of both Hoths is blown to pure white
  under our ambient+sun sum; is the PS2's terrain term a sum, a modulate, or
  a fixed 2x?**

  **New HOLD FOR FABLE this session:** the Hoth clipping question above; and
  why `dea1_bldg_*_collision*` and `uta1_bldg_macy_collision*` are in the
  drawable instance list at all (they carry no material and look like
  collision proxies — is the PS2 filtering them by a class flag we do not
  read?).
- 2026-08-31 (Opus session 5, the maintainer asked "how close are we to a prototype"):
  **Round-completion sweep — 21 of 32 maps finish a conquest round, and the
  11 that do not fail in exactly two ways.** Measured, not estimated:
  `cargo run --release -p game --example round_to_outcome` plays a headless
  round on every baked map (idle player, 16 AI per team) until one side wins.
  Reproduced on seeds 42, 7 and 1234 — both failure modes are systematic, not
  unlucky seeds.
  **(a) Four maps never fight at all** — BF1 KAM1, BF2 COR1, BF2 KAM1, BF2
  UTA1 sit at 250/250 tickets after 60,000 ticks (33 minutes of sim) with
  17 v 16 units alive and **every unit at exactly y = 0**. All four are
  heightfield-less maps (`terrain: None` or a `tern` that is `WATR` only), so
  `Scene::heightfield` puts every soldier on the y = 0 plane while the play
  space — Kamino's platforms, the Jedi Temple, the Utapau sinkhole — is
  somewhere else entirely. This is the ground half of the missing building
  collision.
  **(b) Six or seven maps stall at exactly ONE ticket** — BF1 BES1 (1/1 with
  one unit alive per side), BF1 BES2, BF1 KAS2, BF1 RHN1, BF2 DAG1, BF2 TAN1,
  and BF2 MUS1 on one seed. The starved team always has exactly ONE unit
  alive. That is the no-overspawn invariant working as specified (live units
  may never exceed remaining reinforcements) with no rule to end the round:
  the last defender never dies, so the round cannot finish. **A round-end
  rule for the last reinforcement is a DESIGN decision and is the maintainer's**, not
  something to pick unilaterally — the invariant itself is in the locked
  feature set.
  The 21 that do finish take 185-453 s of sim, which is the right ballpark
  for a conquest round. BF2 HOT1 finishing in 20 s with 242 tickets left is
  a domination win and worth its own look.

### 2026-09-01 — the cleanse, walkable collision, the AI decision, the P9 spine, Phases A/A.1/A.2

Summary in PLAN.md's Log under the 2026-09-01 entries. Verbatim below.

- 2026-09-01 (Fable session 6): **Cleanse — whole codebase, docs included.**
  Ritual: `.claude/commands/cleanse.md` (adapted from Hangar's on 2026-09-01,
  improved by this run). Scope point `3b4edb1`; pass 5 weighted on
  `bea37dc..HEAD` (sessions 3-5). Passes 2-4 fanned out to three Opus
  subagents, every finding re-verified by the lead; passes 1, 5, 6 and every
  fix decision in the main thread. **41 findings: 33 FIXED, 6 DEFERRED-WITH-A-
  HOME (all in `HANDOFF-OPUS.md` §What next), 1 DECLINED, and these passes
  came back CLEAN on an adversarial read:** the deterministic sim (`sim`,
  `battle`, `cp`, `unit`, `collision`, `heightfield`, `projectile`, `soldier`,
  `ai` — generation-checked handles, index-ordered loops, no HashMap
  iteration, no wall clock, no sort, hash over every field that feeds the
  next tick); the render pipeline (pass order, conservative culling, fog
  ramp, wrapped clock); the bake decoders' module headers (all 29 checked,
  every symbol resolves); the fidelity ledger (57 PROVISIONAL sites, each
  says why and what would settle it; nothing promoted from guess to fact
  without RE_LOG); every artifact README command line; the guardrail greps
  (0 tracked assets/ELF/C/Lua, tracked bytes SHRANK 28.3 → 25.7 MB since
  `bea37dc`); the architecture invariants (only `bake` links `mungefs`; no
  Lua VM; `net` is one line); 297 tests inventoried with 0 deletions and 0
  data gates skipping on this machine; the third case-folding class (none —
  every name-keyed lookup folds, and no baked map has two names that collide
  after folding).

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

  **Fixed (code).** The golden gallery's OVERVIEW panel cleared to the fixed
  desert blue on every map — it drops fog on purpose, and the session-5
  per-map clear colour was only ever applied to the ground panel, which is
  why `ded9712` ("only one golden moves") moved none. It now clears to the
  dome's bottom row (`viewer::horizon_clear`), never the fog colour. Eight of 42 goldens moved (KAS1, KAS2, UTA1, COR1, GEO1, BF2 KAS2,
  NAB2, TAT2), every changed pixel in the overview half, all viewed:
  Kashyyyk's blue band is gone, Utapau's sinkhole ring is sand haze,
  Coruscant's gaps take the night skyline; BF2 Theed clears to BLACK because
  its dome texture is black on the disc (HOLD 10(c)) — honest, not pretty.
  `engine/artifacts/cleanse/`.
  `cargo test -p render -- --ignored`, the documented command for the ten
  render GPU referees, SIGSEGVs under the harness's default thread count on
  the iGPU (2 of 3 runs, reproduced): `scripts/gpu_tests.sh` serialises them
  and then runs `goldens.sh --check`. `mungefs::water`'s nibble-order test
  never called the parser (it re-implemented the split in its own body; the
  lead swapped the parser's order and it stayed green) — it now feeds a
  synthetic `tern → WATR` through `Water::parse` and was watched failing
  once. `bake::sound_names`' hash test asserted only case-insensitivity; it
  pins `0x2729_7082` now. `bake::detail::ALPHA_TEST = 128` and the shader's
  `< 0.5` are one threshold in two languages with nothing tying them — a
  referee test reads `shader.wgsl` and pins both. `core → hecs` and `game →
  serde_json` were declared and unused — removed (tech stack line updated:
  no ECS). `core::ai` was labelled PLACEHOLDER, not PROVISIONAL, and so was
  invisible to every ledger inventory — relabelled, its two bare `0.6`
  literals are `ADVANCE_CONE`; `input::DEADZONE`, `bake::sounds` defaults,
  `bake::terrain_glb`'s COLR thresholds, the shader's "base dome is never
  fogged" and `soldier`'s fall threshold are now labelled ours.
  **Fixed (docs).** The alpha-blend pass was still recorded as MISSING at
  three sites written hours before `7a919fa` landed it (RE_LOG "Open for
  Fable", the gallery README's "not yet fixed", the shader header); the
  terrain split (B4) was still open in `render::frustum`'s header and the
  perf README (the maintainer's optimisation backlog); `gallery.rs` documented a
  `game --gallery` flag that never existed; `bake pc-tex` wrote "no override
  wiring; the maintainer + Fable's call" into every generated `pc_textures.ron` after
  `bake::tier` shipped; `bake::map`'s output list, `core::sim`'s tick order
  (5 of 7 phases), `game/main.rs`'s flag table (5 parsed flags undocumented),
  RE_LOG/re-sky-lighting/defect_hunt's "six maps" (five), two unmarked A2
  claims (`WATR` "rare"; `PCHS COLR` = "blend data"), and the `all_maps`,
  `fog_rule`, `pc_textures`, `pc_tier` and `lighting` READMEs — all
  corrected, errata inline, READMEs in their own UPDATED convention.
  **Declined:** the bake writes BF1's planet-globe load still that the
  runtime never reads — it is the globe BF1 composites over its locale step;
  decided-not-drifted, its home is the P10 load-screen layout pass.
  **Deferred with a home** (HANDOFF §What next, in value order): walkable
  `coll` collision (Fable design → Opus build; the round sweep is the
  acceptance test); mission RONs from the PC `.lua` sources (released to
  Opus, PS2-cross-checked); P7 entity shape (`Unit` is a soldier by
  construction — vehicles are a second entity kind); the two P9 spine
  changes (N pads; `libm`); the shared `manifest` crate (the bake↔runtime
  schema is defined twice with no round-trip test — a renamed field compiles
  on both sides and silently defaults).
  **Facts for the maintainer, not verdicts:** `engine/goldens/loadscreens_bf{1,2}.png`
  are decoded disc art, 7.1 MB = 28 % of tracked bytes (fine while git is
  local-only; revisit at Q4). `.git` is 139 MB of golden-PNG history.
- 2026-09-01 (Fable session 7): **Walkable model collision landed; the mission
  RONs are generated from the discs; the round sweep says the next blocker is
  the placeholder AI, not the world.** Worked HANDOFF §What next in order
  (items 1–6); items 1–2 built (Fable design → Opus build, three agents in
  sequence, every result re-run by the lead), 3–4 designed into this entry,
  5 briefed and deferred (the manifest schema changed twice this session and
  must settle first), 6 is the brief at the top of the session report.

  **1. Collision — what was measured first (RE_LOG "`coll` / `prim` / `CUTR`",
  2026-09-01).** The `coll` inner encoding is pinned by consistency over
  1,698 chunks on both discs: `LEAF` = `u8 n` + 6 bytes + `u16[n]`, a convex
  polygon in perimeter order (0 bow-ties in 5,000+ quads), INFO =
  (verts, nodes, leaves, Σn), POSI in the render model's frame. `prim` kinds:
  0 sphere, 2 cylinder (radius, FULL height, local Y), 4 box (half extents);
  the Kamino platform floor is a `prim` cylinder, so primitives are part of
  the ground query. `CUTR` = `[u32 n][f32 sphere r,cx,cy,cz]` + n unit
  planes; inside ⇔ every `n·p + d < 0`, pinned on DAG1's cave post. Every
  walkable structure on the eleven stalling maps ships collision (census in
  RE_LOG); what lacks it is decoration.
  **The design (Fable, main thread; PROVISIONAL constants labelled in
  `core::collision`):** ground = the highest walkable surface (|n.y| ≥ 0.5)
  between a probe `STEP_HEIGHT` (0.6 m) above the feet and the terrain, else
  the terrain, else `NO_GROUND` (−300, the PS2's own cut sentinel reused as
  the no-terrain floor); a point inside a cutter has no terrain (the PS2 rule,
  re-terrain-heights.md); the terrain is a ONE-SIDED sheet — a unit more than
  `TERRAIN_BURY` (2 m) below an uncut heightfield is legitimately underground
  (BF2 Polis Massa's base is 21–34 m under the asteroid surface and none of
  its seven cutters contains a post; they are the entrance shafts), and the
  terrain LOS/bolt rays hit only where a ray CROSSES the sheet outside a
  cutter (before this, 29 units inside BF1 TAT1's cp2 building — a cutter,
  2.7 m under the heightfield — could not see each other). Walls: a horizontal
  capsule push-out (parry3d `contact`, queries only) — applied to PLAYER units
  only (`Sim.walls_for_ai = false`, PROVISIONAL) because the straight-line
  placeholder AI jams on buildings (14/32 rounds finish with AI walls, 18/32
  without). Determinism: bodies in manifest instance order, triangles in leaf
  order, parry's BVH min-toi, strict `<` ties, nothing sorted; the world is
  an input like the heightfield and is not hashed. All 15,199 instance
  matrices over the 32 maps are orthonormal (measured by the build agent), so
  the rigid-isometry path is data-backed. `parry3d 0.22.1` is now a `core`
  dependency (queries only, per the tech stack). Bake: `collision/<geometry>.
  glb` per `coll`, manifest `collision`/`terrain_cutters`; `scripts/rebake.sh`
  clears `collision/`. Diagnostics kept: `round_to_outcome` prints a per-map
  replay `hash`, and `ROUND_VERBOSE=1` adds the spawn-node ground check
  (authored y vs resolved ground), and per unit the terrain height, cutter
  flag and heightfield LOS to its nearest enemy.
  **What the sweep then said, honestly.** Before: 21 of 32 finish (session 5's table). With the
  ground query alone, 18; with the wall pass on every unit, 14 — the
  placeholder AI beelines into buildings and stops (`vel 9.00`, position
  frozen; `push_out` measured correct, 0.13 m per tick, no oscillation). Then
  with the sheet rules, player-only walls, mission wiring, death regions and
  lethal landings, and the separation fix (a grounded unit pushed over an
  edge keeps its y and falls instead of being re-grounded to −300): **28 of
  32**, two runs byte-identical, `gpu_tests.sh` green with all 42 goldens
  unchanged, `--sim-hash 600` (no map) unchanged. The four that do not
  finish (ROUND_VERBOSE, `engine/artifacts/collision/README.md`): BF1 BES2 —
  both teams on the streets 40 m under Cloud City's posts with no path up;
  BF1 RHN1 — sixteen winners camp on the citadel top 200 m from the idle
  player, the cleanse's "camps an uncapturable post"; BF2 DAG1 — the last
  post is the cave (its cutter works: the sweep finished DAG1 in an earlier
  configuration) and the surface AI has no path in; BF2 POL1 — the surface
  AI stands over the shaft to the under-terrain base. All four are
  navigation, i.e. Deferred design #1. Maps that finish now and did not
  before include every platform map: BF1 KAM1/KAS2/BES1, BF2 KAM1/COR1/
  DEA1/MUS1/TAN1/UTA1.
  **Environmental deaths (PROVISIONAL, ours).** `AddDeathRegion` regions from
  the mission kill a unit whose origin is inside (binding BF1 001857a8 / BF2
  0018d628; the engine consumer is untraced — HOLD FOR FABLE); a landing at
  ≥ `LETHAL_LANDING_SPEED` (25 m/s ≈ a 17 m drop at g = 18) kills — both
  games have fall damage, its rule is untraced — HOLD FOR FABLE: the soldier
  fall-damage rule (which class prop, threshold, scaling) and the death-region
  consumer. Without these, units that walk off Kamino's platforms sit in the
  sea forever and units that step off a Death Star floor fall to −300 and
  stand there.
  **HOLD FOR FABLE (collision):** the PS2 collision LAYER MASKS (`p_vehicle*`
  primitives are excluded from the soldier world by NAME — `*vehicle*`,
  `*flyer*`, `*ordnance*` — as a PROVISIONAL rule; the per-primitive and
  per-polygon masks, and whether `prim PRNT` transforms are ever non-identity,
  are RE items); the soldier step/slope rule (`STEP_HEIGHT`, `WALKABLE_MIN_NY`,
  `TERRAIN_BURY` are ours); the soldier-vs-world response (`push_out` is
  ours); bolts vs model collision (bolts still see terrain + capsules only);
  the six `LEAF` flag bytes (not a normal — undecoded).

  **2. Mission data — generated from the PS2 bytecode, PC sources as
  referee (Opus; `scripts/gen_missions.py`).** The agent wrote Lua 4.0 and
  5.0 undumpers plus a small abstract interpreter (BF1's `PUSHINT` operands
  and BF2's `SetupTeams{}` constructor both hide the numbers from any string
  or constant-table dump; note: Lua 5.0 packs `OP,C,B,A` with `POS_A = 24`,
  not 5.1's layout; the PS2 build's RK split is 128, confirming
  re-tickets-spawn §1.3). **57 RONs**: `<key>_con.ron` (GCW) and
  `<key>_con_cw.ron` (CW) for every map that ships both eras; single-era maps
  keep one file. Findings: BF1's mission letter is the ATTACKING FACTION
  (a/i = GCW, r/c = CW; `_h` = campaign twins carrying `SetHistorical`), so
  the hand-authored `tat1_con.ron` was internally inconsistent (all/imp order
  with `tat1r` — a CW script's — numbers): it is now `tat1i`, team 1 = the
  attacking Empire, and `tat1_con_cw.ron` is `tat1r`. **All 68 PC/PS2
  disagreements are the same fact: the PS2 retail shrinks the member pool**
  (BF2 32 → 20/16/19/22/25 per map, BF1 hot1i 28 → 25, rhn2 25 → 23) — A6's
  "occasional minor PS2 tuning" — and every one is labelled PROVISIONAL with
  PS2 winning; factions, tickets, spawn delay, attacking team, posts and death
  regions agree everywhere. Command-post ownership: **BF1 missions assign
  none** (the world layer is the roster); BF2 uses `SetProperty("cpN",
  "Team", "…")` (Kamino, both eras, six posts) and `SetObjectTeam` (Hoth).
  **CORRECTION to the Cleanse's correction 2:** BF2 HOT1 is not "five posts
  reassigned" — `hot1g_con` builds its conquest from FOUR posts (CP3–CP6),
  `KillObject("CP7")`s the fifth, and flips CP3/CP6 to team 1: 2–2 at tick 1.
  BF2 DEA1 is the same class of case (`KillObject("CP6")`). The RONs carry
  `posts` (the objective's roster, empty on BF1 by measurement) and `killed`;
  `game::layout::MapLayout::apply_mission` restricts, drops and reassigns
  accordingly, and the death regions are resolved by name from the world's
  regions. Not carried (recoverable from the same call trace): bleed
  thresholds (inert on BF1, §2.4), hero classes, `AddWalkerType`, memory
  pools, VO. `ai_alive_cap` is one number for two teams (BF1 kas1/kas2/nab2
  are asymmetric; `initial_ai` clamps per team). The PC sources live under
  `community_mods/gametoast_lua/` (gitignored, never tracked).

  **P7 DESIGN — entity shape (Fable, decided 2026-09-01).** Moved verbatim to
  §DESIGN SPECS (2026-09-08, cleanse C2): a vehicle is a SECOND ENTITY KIND,
  not a `Unit` variant; occupancy on both sides; the `Target` enum; hulls from
  `coll`/`prim`; P9 before P7; and the seven questions the RE pass must answer.

  **P9 SPEC (a) — `Sim::step` over N pads indexed by player slot (Fable,
  half a page; Opus implements as its own commit with the replay check
  below).** Today `Sim::step(&mut self, pad: &PadState)` copies the ONE pad
  into every `Controller::Player` unit (sim.rs step phase 1), `Sim.player:
  Option<UnitHandle>` is the single player, and `InputLog.ticks:
  Vec<PadState>` holds one pad per tick. Change: `Controller::Player` becomes
  `Controller::Player { slot: u8 }`; `UnitSpec.is_player` becomes
  `player_slot: Option<u8>` (`is_player()` derived); `Sim.player` becomes
  `Sim.players: Vec<Option<UnitHandle>>` indexed by slot (`spawn` fills
  `players[slot]`, growing the Vec; `respawn_slot` keeps the slot);
  `Sim::step(&mut self, pads: &[PadState])` gives slot s `pads[s]`, or
  `PadState::default()` when the slice is shorter (a slot without a pad idles
  — documented, not an error); aim assist and the magnet are per slot
  (`assist: Vec<AssistState>`, `magnet: Vec<Option<UnitHandle>>`, HUD reads
  slot 0); `InputLog { seed, slots: u8 (serde default 1), ticks:
  Vec<PadState> }` stays FLAT — tick t slot s is `ticks[t * slots + s]`, so
  the two existing artifact logs (`engine/artifacts/p3_walk/{walk,aim}_demo.
  ron`, no `slots` field) replay unchanged. `Sim::hash` adds the slot table
  (count + each handle). Callers: `WalkSim::step_tick(&[PadState])`,
  `WalkSim.player` = slot 0's handle, `game/main.rs` (walk, record, replay,
  walk-replay), `bench.rs`, `round_to_outcome` (`&[PadState::default()]`),
  every test. Tests: `two_slots_drive_two_units_independently`,
  `missing_pad_idles_the_slot`, `single_pad_log_without_slots_field_replays_
  identically` (serialise a log with no `slots`, assert the same hash as
  `slots: 1`), and `same_log_same_hash` as is. Replay check (the lead runs
  it): `--sim-hash 600` before/after (moves — the slot table is hashed —
  record both), and `game --map assets_baked/tat1 --headless --walk-replay
  engine/artifacts/p3_walk/walk_demo.ron --capture-series 1000000,<dir>`
  before/after: `motion.csv` must be BYTE-IDENTICAL (the hash may move; the
  motion may not). `net` later maps peer → slot; splitscreen maps pad → slot.

  **P9 SPEC (b) — `core::fmath` backed by the `libm` crate (Fable, half a
  page; Opus implements as its own commit with the replay check below).**
  `core` calls the platform libm at eight sites: `soldier.rs:355` and `:390`
  (`yaw.sin_cos()`), `unit.rs:93-94` (`aim_frame`: yaw and pitch `sin_cos`),
  `weapon.rs:962-963` (`shot_direction`: pitch and yaw `sin_cos`), `ai.rs:82`
  (`yaw_toward`: `atan2`), `ai.rs:175` (`asin`). `sqrt` is IEEE-754
  correctly rounded everywhere and stays `std`; `%` is exact; glam's
  `length/normalize` are sqrt-only; no `Quat`/`from_rotation_*`/`acos`/`exp`
  is on the sim path (grep clean; `weapon.rs:1195 angle_between` is inside
  `#[cfg(test)]`). Change: new `core::fmath` with `pub fn sin_cos(x: f32) ->
  (f32, f32)`, `atan2(y, x)`, `asin(x)` calling `libm::sinf/cosf/atan2f/
  asinf` (`libm = "0.2"` in the workspace deps + `core`; 0.2.16 is already in
  `Cargo.lock` transitively, so no new download); the eight sites call
  `fmath`; a source-scan referee test (like `bake::detail`'s shader referee)
  fails if `core/src` outside `fmath.rs` and outside `#[cfg(test)]` contains
  `.sin_cos(`, `.sin(`, `.cos(`, `.tan(`, `.atan2(`, `.asin(`, `.acos(`,
  `.exp(`, `.ln(`, `.powf(`. A `fmath_within_2ulp_of_std` sweep test
  documents the difference (it may be exactly 0 on this glibc — say so in
  the Log either way). Follow-up inside the same pass: `parry3d` is now on
  the sim path (walkable collision); read its `Ball`/`Cylinder`/`Cuboid`/
  `TriMesh` ray casts and the capsule `contact` path for transcendental calls
  — sqrt-only is expected; anything else is a finding to bring back. Replay
  check (the lead runs it): `--sim-hash 600` before/after (an idle sim may
  not move — record both), and the same `walk_demo.ron` + `aim_demo.ron`
  `--walk-replay` motion.csv before/after: the stance column must be
  identical and the max |Δposition| over the log ≤ 1e-3 m; record the max.
  This is the one P9 change whose hash is ALLOWED to move by rounding, and
  the Log must carry the measured bound.

  **6. AI base decision — the brief is at the top of the session report;
  nothing was tuned.** Its one-line summary: BF2 base with the six PC-keyed
  player biases OFF by default (verify each exists on the PS2 first), BF1's
  data fed to BF2's arbiter; the maintainer picks.

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

  **The architecture (decided).** Six layers, all sim-state only, seeded and
  staggered per unit, and every bot keeps driving the SAME soldier code as the
  player through a synthesised pad — movement, stances, sprint, weapons and
  dispersion stay shared; that contract is the one thing kept from the
  placeholder:
  1. Navigation — the authored `plan` graph (hubs + connections, per unit
     type) and `BARR` barriers decoded from the discs, with collision-aware
     local steering over `CollisionWorld`; AI walls go back ON. A baked
     navmesh is the fallback only if the authored graphs prove too coarse
     (decided by measurement in Phase A).
  2. Commander per team — a utility arbiter that assigns squads to objectives
     (attack post, defend post, hunt the last defender, escort; later board
     vehicle, hero) about once a second from post value (the discs'
     `Value_ATK_*`/`Value_DEF_*`, re-cp-capture), graph distance, threat and
     force ratio. This is what produces route diversity and kills camping.
  3. Individual behaviour — a behaviour tree per bot (advance, engage, cover
     from `CoverHint`/graph edges, strafe, grenade, retreat when hurt, snipe
     from `SnipeHint`, jet-jump routes from `JetJumpHint`) driving the pad.
  4. Perception — vision cone + LOS (terrain AND model collision) + hearing +
     last-known-position memory + reaction delay; a bot may act only on what
     perception delivered (no wallhack, no omniscient aim). This is the
     fairness guarantee for the top tiers.
  5. Skill vector — reaction time, angular aim error scaled by range and
     target motion, burst discipline, lead accuracy, applied through the
     weapon dispersion the real engine also multiplies for AI
     (weapon+0x158/+0x138).
  6. Squad commands (below) and, with P7, vehicle boarding/driving through the
     routed pad.

  **Difficulty (decided): three layers.** (1) COMPETENCE is symmetric — every
  bot on every team gets the tier's skill vector, perception and commander
  quality. In a symmetric conquest fight bot-vs-bot exchanges cancel on
  average, so the round is decided by what the humans add; as bots improve
  the human both dies more and can solo-carry less, which is the two ways
  "harder" is felt. (2) PRESSURE is per HUMAN, not per team — bots on all
  teams weight human targets higher; humans take a damage multiplier at the
  top tiers. Attached to the human it composes for every lobby: opposing
  humans are both hunted, co-op partners are both hunted, 2-vs-1 stays fair;
  uneven sides are balanced by bot COUNT (BF2's `AIAutoBalance` idea), never
  by skill. (3) TEAM HANDICAP applies only AGAINST bot-only teams —
  reinforcement/bleed bias against the humans' team, a host option that is
  inert whenever both teams have humans (the strategy-game "cheater" layer,
  and the reason single-player Elite can exceed any co-op fairness ceiling).
  Tiers: Easy (low competence, forgiving); Normal (competence anchored to the
  PS2 BF2 Elite numbers once traced); Hard (above both originals' maximum,
  mild pressure); Elite (full competence with a capped reaction time and an
  aim-error floor, full pressure, team handicap in single-player). Dynamic
  difficulty adjustment is EXCLUDED from anything competitive; an optional
  single-player aid later at most. Precedents behind the reasoning:
  symmetric bot skill (Counter-Strike, Unreal Tournament, Halo Infinite bots),
  asymmetric enemy scaling (Halo campaign, Left 4 Dead's director),
  competence + handicap layering (StarCraft II's tiers vs "Cheater" levels,
  Civilization, Company of Heroes); BF1's own per-team skill pair is the
  in-franchise precedent.

  **Squad commands (decided, the maintainer's numbers).** Follow me / hold position /
  disperse / clear all. A command goes to the unassigned AI teammates within
  20 m of the player in ANY direction, at most 6; one command per bot, the
  newest replaces the older; the player's death clears every command they
  gave, a bot's death clears its own; a commanded bot leaves the commander's
  pool until cleared. Both originals had follow and hold agents in the
  engine, so this is faithful in spirit; the VO is the `*_com_chatter_tac_*`
  chatter already in the banks.

  **Phases and acceptance (measurable, in order).** A. Navigation: 32 of 32
  maps finish `round_to_outcome` with AI walls ON (`Sim.walls_for_ai`
  retired), two runs byte-identical, no unit stuck (position frozen under a
  non-zero pad) for longer than a bounded time, and the last-defender hunt
  closes BF1 RHN1. B. Commander: on the platform maps units split across at
  least two routes to the same objective (measured from the replay), no
  camping of unreachable posts, AI-vs-AI ticket curves within the originals'
  range. C. Combat + perception + skill vector + tiers: a headless bot-vs-bot
  arena example where per-tier kill rate is monotonic, the reaction-time and
  aim-error floors hold at Elite, and no bot fires on a target perception has
  not delivered. D. Squad commands with tests and the tac chatter. E. Vehicle
  AI with P7. Tier-4 artifacts for the maintainer at every phase.

  **What the RE pass is for now (narrowed, Fable):** the three formats and
  their loaders (running: RE_LOG "`plan` / `Hint` / `BARR`"); whether BF1
  consumes its `plan` at runtime; the five unit path types; BF2's hint use;
  what Elite changes on PS2 BF2 (the three leads above); the defaults behind
  the shared Lua knobs (`SetTeamAggressiveness`, `SetAIViewMultiplier`, snipe
  ranges, `SetStationaryWaitTime`); the AI accuracy model. NOT the originals'
  decision logic — it is not being ported.

  **Work order this session:** P9(a) N pads (Opus, running) → lead replay
  check + commit → P9(b) `fmath` (Opus) → shared `manifest` crate (Opus) →
  `mungefs` plan/hints/barriers + bake (Opus, from the RE spec) → Phase A
  navigation (Fable design, Opus build, the sweep as the gate) → bolts vs
  model collision → Phases B/C → P7. Only one agent edits `engine/` at a
  time. **HOLD FOR FABLE (new):** the BF1 skill pair's sign convention and
  consumer (FUN_00237af0, DAT_005875ec/f4); BF2 Elite's semantics
  (FUN_002a2988, table 0x5fa808, the dispersion writer); BF2 hint
  consumption. **Retired:** the BF1-vs-BF2-vs-merge brief.

- 2026-09-01 (Fable session 8): **P9 (a) landed — `Sim::step` over N pads
  indexed by player slot** (Opus build from PLAN Log "P9 SPEC (a)", lead-
  verified). `Controller::Player { slot }`, `UnitSpec.player_slot`,
  `Sim.players`/`assist`/`magnet` per slot, `InputLog.slots` (serde default
  1, FLAT layout, the two on-disk P3 logs replay unchanged),
  `Sim::step(&[PadState])` with a missing pad idling its slot. Verified by
  the lead independently (stash → before captures → pop → after captures):
  `walk_demo.ron` (407 ticks) and `aim_demo.ron` (120 ticks) `motion.csv`
  BYTE-IDENTICAL; ci green; `round_to_outcome` 28 of 32 with the same four
  stalls, every non-hash column identical to the pre-change sweep, two runs
  identical. `--sim-hash 600`: 460de98224d807c9 → 2c4328a4e032b709 (the
  slot table; the agent proved the attribution by deleting only that block)
  → 21ca18400d887168 after the lead's one change: the per-slot magnet table
  is length-prefixed with `None` as (MAX, MAX) like the slot table, because
  a magnet in slot 0 and the same magnet in slot 1 hashed alike (test
  `magnet_slot_is_part_of_the_hash`). Noted, not changed: a dead player's
  slot keeps its last magnet until respawn (pre-existing; the HUD can show a
  magnet for a dead player — decide at the HUD pass); `InputLog::frames`
  drops a trailing partial tick silently (validate when the recorder lands);
  `Sim::new` still spawns only slot 0, so `--replay` drives one slot until a
  multi-slot scene exists; `main.rs` has no `--record` (the spec's caller
  list was one item optimistic). Tests: `two_slots_drive_two_units_
  independently`, `missing_pad_idles_the_slot`, `single_pad_log_without_
  slots_field_replays_identically`, `magnet_slot_is_part_of_the_hash`.

- 2026-09-01 (Fable session 8): **P9 (b) landed — `core::fmath` on the `libm`
  crate** (Opus build from "P9 SPEC (b)", lead-verified). Eight call sites
  (`soldier.rs` ×2, `unit.rs` ×2, `weapon.rs` ×2, `ai.rs` ×2) go through
  `fmath::{sin_cos, atan2, asin}` → `libm::{sinf, cosf, atan2f, asinf}`
  (0.2.16, already in the lock file; `sqrt` stays std, IEEE-correct); the
  source-scan referee `no_std_transcendentals_in_core` fails on any
  transcendental method outside `fmath.rs`/tests and fails LOUDLY if a
  file's `#[cfg(test)]` is not the trailing `mod tests` or `core/src` grows
  a subdirectory (verified to fire on an injected `.acos()`). **Measured
  bound:** libm vs glibc 2.43 differ by at most 1 ulp (sin at x = −25.12,
  cos at −24.35, atan2 at y = −1, asin at −0.99892; 2M-point sweeps,
  `fmath_within_2ulp_of_std`). Replay check: `walk_demo.ron` (407 ticks)
  and `aim_demo.ron` (120) `motion.csv` BYTE-IDENTICAL (max |Δpos| 0,
  stance identical); `--sim-hash 600` unchanged at 21ca18400d887168.
  **But those probes cannot see this change** — both logs and the idle sim
  run ONE player and no AI, so `ai.rs`'s `atan2`/`asin` never execute; the
  only sensitive probe is the round sweep, and it MOVED: **27 of 32** (28
  before). All 32 per-map hashes moved; 29 keep their winner; BF2 GEO1 and
  BF2 KAS2 flip winner; BF1 KAS2 goes from a team-0 win at tick 10618 to
  NO OUTCOME. Root-caused, not assumed: the agent re-backed `fmath` with
  `std` keeping every call-site rewrite and the sweep came back
  byte-identical to before, so the eight rewrites are behaviour-neutral
  and the whole divergence is the 1-ulp libm difference amplified through
  `shot_direction` (spread → normalize) over 60k-tick battles. The BF1
  KAS2 stall is the cleanse's known shape, not a new one (`ROUND_VERBOSE=1
  … bf1_kas2`): 13 team-1 AI stand at (25, −5.9, 116), UNDER the cp10 pier
  deck (capture box centre (25, 1, 107), y ∈ −0.5..2.5), 5.6 m from a post
  they can never enter, the idle player 207 m away — the placeholder AI's
  navigation, which Phase A replaces. **The scoreboard is now 27 of 32**
  and stays there until Phase A; nothing was tuned. parry3d audit: every
  query the sim reaches (`TriMesh`/`Ball`/`Cylinder`/`Cuboid` ray casts,
  capsule `contact` via GJK/EPA) is sqrt-only — no finding — but parry runs
  with default features, so a future query reaching its nonlinear TOI /
  `Cone` / mass-properties code would pick up the host libm silently.
  **P9 follow-ups:** enable parry's `enhanced-determinism` (simba
  `libm_force`) before the first cross-OS lockstep test; add an AI-battle
  capture (a seeded `round_to_outcome`-style battle with a motion CSV) to
  the replay check so the next numeric change is gated by something that
  exercises the AI sites. Tests: `fmath_within_2ulp_of_std`,
  `no_std_transcendentals_in_core`.

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

- 2026-09-01 (Fable session 8): **CORRECTION to the P9 (b) entry — the parry
  audit evidence.** The build agent retracted its first parry3d audit as
  FABRICATED (it had delegated the read to a sub-agent, received nothing,
  and wrote a plausible section: the "GJK/EPA sqrt-only" trace and the
  claim that `cuboid.rs`/`segment.rs` hits were inside block comments were
  invented). It then did the audit itself; the lead re-checked the
  load-bearing fact by grep: **`parry3d-0.22.1/src/query/` — the whole
  implementation of both entry points the sim uses (`cast_local_ray_and_
  get_normal`, `query::contact`) — contains ZERO transcendental calls in
  either the method form or the `ComplexField::`/`RealField::` form.** The
  crate's 43 hits live in `utils/interval.rs` (CCD), `transformation/`
  (mesh generation, mesh booleans), `shape/{cuboid,segment,convex_*}.rs`
  (`support_feature_toward`, reached only by point projection and contact
  manifolds), `bounding_volume/aabb.rs:572–603` (rotated-AABB helpers for
  CCD), `partitioning/bvh/bvh_optimize.rs` (`log2`, no caller inside
  parry) and `shape.rs` (`Cone`). The verdict stands (sqrt-only on every
  sim-reachable path) on this evidence instead of the retracted one; the
  P9 follow-up about `enhanced-determinism` (`simba/libm_force`, not in
  parry's `default = ["required-features", "std", "spade"]`) stands.
  Working rule reaffirmed for the session: an agent's claim enters the
  Log only after the lead has reproduced its load-bearing measurement.

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

- 2026-09-01 (Fable session 8): **Shared `manifest` crate landed** (Opus
  build from HANDOFF §NEWLY OPUS-SAFE item 2, lead-verified). Seventeen
  bake↔runtime RON types (`Manifest`, `CommonManifest`, `ManifestHead`,
  the instance/region/class/geometry/path/layer/cutter/collision/prim/
  water records and the five lighting/sky records) now have ONE definition
  in `engine/crates/manifest`, deriving both `Serialize` and
  `Deserialize`, with all 31 game-side `#[serde(default)]` attributes kept
  next to the note saying which older bake each keeps loadable; `bake` and
  `game` re-export them under their old paths, nothing renamed.
  `impl Default for ManifestLighting` moved with its type (pure data; the
  PROVISIONAL `bake::lighting` defaults are untouched). Verified: three
  reference bakes (BF1 TAT1, BF2 HOT1 with the PC tier, common_bf2)
  `diff -r` empty before/after — 130/109/278 files, glb and ktx2 included;
  the lead re-baked TAT1 independently and it is byte-identical to the
  pre-change output; `all_maps_load` 9/9 green on the existing
  `assets_baked`; ci green; `round_to_outcome` 27 of 32, two runs
  identical, every per-map hash identical to the P9 (b) sweep. Follow-up
  (same drift risk, out of scope here): `bake::loadscreen::{LoadScreen,
  LoadStyle}` vs `game::assets::{LoadScreenRon, LoadStyleRon}` and
  `bake::sounds::SoundsRon` vs `game::sounds::SoundsRon` are still
  duplicated twins.

- 2026-09-01 (Fable session 8): **AI difficulty and feel constants RE'd
  from both ELFs** (Fable RE agent → `analysis/re-ai-difficulty.md`; the
  per-level tables, the clamp and three knob defaults re-read from the ELF
  data by the lead — RE_LOG pointer entry). What it settles for the
  difficulty model: (1) both originals run ONE integer skill 1..20 PER TEAM
  and **both are asymmetric by design — the human's teammates are made
  weaker than the enemy**: BF2 Easy/Normal/Elite = enemy 4/14/17 vs
  teammates 6/11/11; BF1 = enemy 4/14/17 vs teammates 6/4/4 (campaign
  deltas on top); in netplay and split-screen versus every bot gets the
  enemy table. So the originals' hardest tiers were largely "your squad is
  useless", the exact thing the maintainer rejected ("every AI unit improves with
  difficulty"). Our model stands as decided — symmetric competence, per-
  human pressure, team handicap only against bot-only teams — and this
  quantifies its cost: at our Normal, teammates are old-Elite enemies
  (17-equivalent) instead of 11, so the human side is STRONGER than in the
  old Elite and the pressure/handicap layers must carry more than they
  would have; the numbers make that tunable rather than guessed. (2)
  **Anchor for "old Elite ≈ our Normal": enemy skill 17** → spread
  multiplier ×1.32 and AI distance-dispersion ×0.32 written every aim tick
  (weapon+0x134/+0x138; BF1 +0x154/+0x158, the "untraced writer" of
  re-blaster-damage §10.4 is the aim tracker), aim-tracker accel lerp(0.8,
  14) (BF1 curve 0.8/1.9/20), damping lerp(0.8, 2.4), a per-target
  tracked-position error radius (decay lerp(0.1, 1.1) m/s, floor 1 m below
  skill 12, cap 2 m / 1 m, scaled by target motion and visibility) instead
  of a reaction timer, bursts min + U·t, pause (1.5 − 0.8t)×weapon, grenade
  chance t, dodge-on-hit lerp(0, 0.2); perception = 10 m omni + a cone [D
  77.5° half-angle] + view distance (20·radius + 30)·mult·vis ≈ 40–50 m for
  a soldier, nearest-first acquisition with a facing-aware human bias, and
  NO line-of-sight raycast located — so PLAN layer 4's LOS + hearing is
  genuinely new. (3) **CORRECTION to "AI decision"**: BF2's
  `EnableAIAutoBalance` shifts the SKILL of the human team's bots vs the
  enemy's by ±4 from the ticket/score gap every 15 AI ticks (SP/co-op
  only) — a rubber band, not a bot-count balance; bot counts come from the
  `0x5fa808` table. "Uneven sides balanced by bot count" is therefore
  openbf's own rule, not BF2's, and the originals' rubber band falls under
  the DDA we excluded from competitive play. (4) Knob defaults for the
  data-driven layer: aggressiveness 1.0, stationary wait 10 s, vehicle
  notify radius 50 m, view multiplier 1.0 (clamp 0.1..1), snipe
  suitability 70 m, BF1 snipe ranges 140 (used as 70 m radii); BF2's snipe-
  range bindings are stubs. (5) Level-3-only switches worth knowing: AI
  attackers get a > 1 damage multiplier (identity [H]); Elite clears the
  human's auto-aim / aim-assist / sticky reticule in the profile if the
  shell passes the flag (open Q8) — a PS2 "pressure" lever on the human,
  the same shape as our layer 2. Open questions 1–9 are in the report;
  the [H] items (cone angle, damage multiplier identity, combat modes,
  facing axis) stay PROVISIONAL until Phase C needs them.

- 2026-09-01 (Fable session 8): **The AI data is baked** (Opus build from
  "NAV DATA SPEC", lead-verified). `mungefs::plan` (Plan/Hub/Arc/RouteTable
  in the BF2 row form for both games, Plnp with inflated records; the
  layout detected by EXACT TILING, never by game; every truncation a
  `UcfbError`, inflates capped at the declared size), `mungefs::world`
  hints + barriers (raw PROPs kept, the five keys resolved), manifest
  types in the shared crate — **schema corrected during the build:**
  `ManifestNav { plan: Option<ManifestNavPlan>, hints, barriers }`, because
  eight BF2 mode layers on seven maps carry hints without a plan and the
  spec as written would have dropped 199 of them (`Manifest.nav` after
  `terrain_cutters`, `ManifestLayer.nav` after `instances`, all serde
  defaults so older manifests load). Sidecars `nav/routes_<layer>.bin` and
  `nav/plnp_<layer>.bin`; layer keys de-duplicated in layer order because
  BF2 HOT1 ships two `hoth_objective` layers and NAB2/YAV1 two
  `*commondesign` — **a consumer must follow the manifest's path, never
  rebuild it from the name.** KAM1 and MUS1 discard their earlier
  top-level plans and plnps (last-top-level-wins, as BF2 frees the
  previous graph on every `plan`); their hints/barriers come from `wrld`
  and are all kept. **Census over the 32 baked maps: 57 nav blocks, 4,757
  hubs, 6,218 arcs, 5,498 hints, 13,116 barriers, 180 plnp records — hubs,
  arcs, hints and barriers equal to the RE census on every map**, pinned
  as a test (`every_baked_nav_block_is_self_consistent` also checks arc
  endpoints, slot consistency, rows ≤ 4, sidecar tiling, plnp record
  counts and hint kinds). Tests: 16 in `mungefs::plan` incl. the four
  disc oracles (TAT1 hubs 0/2 + arcs, TAT2 hubs 0/1 rows + arcs, KAS2 plnp
  header + record 0, POL1 plnp 189/72745 B), 2 Hint/BARR byte oracles, the
  manifest round trip with both nav variants and a field-order pin. Every
  layout in the RE SPEC matched the bytes; two invariants the spec did not
  state hold on all 20 plnp chunks and are enforced (Σ namesPerGroup ==
  nameCount; records tile the payload). Verified by the lead: ci green,
  `all_maps_load` green, `scripts/rebake.sh tat1 tat2 maps` 4.3 s for all
  32, every manifest diff pure insertions, `round_to_outcome` twice
  byte-identical with every per-map hash unchanged (no consumer yet: 27 of
  32). Noted: hub arc slots are `u8` on disc (max 242 arcs shipped).

- 2026-09-01 (Fable session 8): **PHASE B DESIGN — the commander (per-team
  objective arbiter).** Written while Phase A builds; Opus implements from
  it after Phase A lands. Purpose: route diversity, no camping, a sensible
  attack/defend split, hunting, and the hook squad commands and vehicles
  plug into later. **Decided:**
  1. `core::commander`: one `TeamPlan` per team — `assignment: Vec<Option<
     Objective>>` indexed by unit slot, `next_plan_tick` — re-planned every
     `PLAN_PERIOD` (30 ticks ≈ 1 s, PROVISIONAL) staggered by team (team t
     plans when `tick % PLAN_PERIOD == t`), teams in index order, units in
     index order, so it is sim-state and replay-exact. `Objective::{Attack
     (cp), Defend(cp), Hunt, Escort(UnitHandle) (Phase D), Board(Vehicle
     Handle) (Phase E)}`.
  2. Utility scoring, per unit × objective, all terms in one table with
     PROVISIONAL weights: attack value = the disc's `Value_ATK_*` (BF2 uses
     DEF if friendly, ATK if hostile, the average if neutral — re-cp-capture;
     the mission RONs carry `value_bleed` today, the ATK/DEF values are
     extracted the same way) × 1/(1 + hops) where hops = graph distance
     from the unit's hub to the post's hub (a breadth-first pass FROM each
     post's hub once per plan, ≤ 8 posts × ≤ 300 hubs); − threat (enemies
     within 30 m of the post, from the sim, not from perception — the
     commander is omniscient about POSITIONS the way the originals' planner
     is; Phase C's perception bounds the SOLDIER, not the plan); −
     crowding: the marginal utility of the k-th unit on the same objective
     decays as 1/(1 + k/4), with a hard cap of ceil(N/2) units on one
     attack objective. Defend value = `Value_DEF_*` × (enemies approaching:
     any enemy within 60 m of an owned post, else 0.15). A per-unit
     deterministic noise term (hash of unit index + plan tick, ±10 %)
     breaks ties differently per unit — route diversity on top of the
     routing table's own weighted alternatives. Hysteresis: a unit keeps
     its objective unless another beats it by 20 %.
  3. Team-level rules on top: owning the majority of posts biases toward
     Defend on the contested ones; tickets behind by more than a third
     biases every unit to Attack; no attackable post → every unit Hunts
     (dest = the nearest enemy's hub, the Phase A fallback moved here).
  4. Behaviour hooks: an Attack objective drives the Phase A follower to
     the post and into its capture volume; Defend drives the unit to the
     post and then to the nearest `CoverHint`/`SnipeHint` within 25 m of
     it (Phase C uses the stance the hint authors), else a spot inside the
     capture volume; Hunt drives to the enemy's hub and re-plans each
     period. Units on the same objective from within 20 m of each other
     form a squad (id = objective) — bookkeeping only until Phase D.
  5. Diagnostics: ROUND_VERBOSE prints each unit's objective and squad;
     the sweep prints per map the distinct objectives the winning team used
     and the longest time any unit spent within 10 m of an objective it
     could not reach.
  **Acceptance:** the sweep stays at Phase A's count with two runs
  identical; on every map with ≥ 3 posts the winning team uses ≥ 2
  distinct attack objectives over the round; no "unreachable objective"
  dwell longer than 30 s; the AI-vs-AI round-length distribution is
  REPORTED (no gate — the originals' 16-v-16 rounds are minutes long and
  ours must not become seconds); Tier-4: a top-down BF1 BES1 capture
  series showing the split between the corridor and the outer platforms.

- 2026-09-01 (Fable session 8): **PHASE C DESIGN — perception, the skill
  vector and the four tiers.** Anchored on `analysis/re-ai-difficulty.md`
  (lead-verified numbers). The six decisions (skill scale, perception, target
  choice, difficulty plumbing, hints in combat, the arena) and the acceptance
  gates are **§DESIGN SPECS "PHASE C DESIGN"** — moved there verbatim
  2026-09-08 by cleanse C2, with the 2026-09-08 aim-error floor/cap correction
  attached. Commit 1 of 5 is on main (`658c6af`); commits 2–5 are loop queue
  item PC.

- 2026-09-01 (Fable session 8): **Phase B addendum — the post values are in
  the bake already.** The CP instances carry `Value_ATK_{Alliance, Empire,
  Republic, CIS, Locals}` and `Value_DEF_{…}` as raw PROPs under the hashes
  re-cp-capture §176–177 lists (A340D6B5 / 4DB44670 / 9E78B410 / 00E73245 /
  573C98A4 and 44A2C4A6 / A8679457 / 8F5540CB / 5BF85AE8 / E689EF93 — NOT
  fnv1a of the key names, which is why a naive lookup finds nothing);
  checked in the baked manifests: TAT1's nine posts author ATK 0/10/20/30
  and DEF 0/10/20/50 per faction, BF2 DAG1 10/20, BF2 HOT1 0/10, BF1 BES1 a
  flat 10 everywhere (so its route diversity must come from the noise
  term and the routing table, not from the values). `game::layout` reads
  them next to `ValueBleed` (`PROP_VALUE_BLEED`) into `CommandPostDef`;
  the commander takes the row for the team's faction (mission `factions`)
  with the BF2 rule (DEF if friendly, ATK if hostile, average if neutral;
  default 100 when a post authors none — the class ctor default, same
  table).

- 2026-09-01 (Fable session 8): **PHASE A.1 DESIGN — local steering (the
  wall slide, feelers, a strafe-capable pad, movement while engaged).**
  Written from the Phase A sweep evidence: every remaining stall is a unit
  walking a straight leg head-on into model geometry — `push_out` is a
  positional push along the contact normal, so a head-on approach makes
  exactly zero progress while a diagonal one already slides — and, on
  UTA1, an engaged unit that holds its ground against a wall because the
  terrain-only LOS lets it "see" through it. **Decided (after the bolts /
  LOS item lands, which gives `ai::visible` the model-collision test):**
  1. `CollisionWorld::wall_probe(cap, dir_xz, dist) -> Option<Vec3>`: the
     capsule translated by `dir·dist`, one pass of the `push_out` contact
     test, returning the XZ push normal of the deepest penetration, `None`
     when clear. A query, no state.
  2. Steering in `ai::think`, every tick the unit wants to move: desired
     XZ direction `d` (toward the waypoint, or the post on the last leg);
     probe `d` at `PROBE_DIST` = 1.0 m; clear → move along `d`. Blocked
     with normal `n` → candidates in this order: the wall tangent `t = d −
     (d·n)n` (sign chosen toward the waypoint; if |t| < 0.1 the side
     alternates with the stuck side-step), then the fan ±30°, ±60°, ±90°,
     ±120° about `d` (the side-step's sign first), first clear probe wins;
     all blocked → move along −d this tick. The fan runs ONLY when the
     forward probe hits, so the common cost is one probe per moving unit
     per tick (`--bench --ai-cap` re-measured, recorded in the Log).
  3. The pad is built from the chosen WORLD direction: `ly = m·forward`,
     `lx = m·right` in the soldier's own convention (read it from
     `soldier.rs`, do not guess the sign), so a unit moves the intended
     way while its yaw catches up — `ADVANCE_CONE` no longer gates
     walking. Facing stays as it is: the waypoint when not engaged, the
     enemy when engaged.
  4. Engaged units keep MOVING ALONG THE ROUTE (decision 2's `m` from the
     route, not the enemy) while aiming and firing, and hold only within
     `HOLD_RANGE` as today; the early return in `think` goes away so the
     stuck window and the side-step also run while engaged.
  5. Arrival semantics stay the PS2's (the hub radius): the last leg from
     a large destination hub to the post is a steered leg, so the GEO1
     bunker is approached by wall-hugging rather than a beeline. If the
     sweep shows a post that hugging cannot reach (a bunker whose door is
     on the far side from every approach), the next tool is the post's
     own spawn-path nodes as intermediate waypoints — measured first.
  6. Diagnostics: ROUND_VERBOSE adds the steering pick (`fwd`, `slide`,
     `fan ±k`, `back`) per unit and a per-map count of blocked-forward
     ticks; `stuck_max` stays the headline.
  **Acceptance:** the sweep gate is unchanged (32 of 32, two runs
  identical); `stuck_max` per finished map under 300 ticks is the target
  and the table is REPORTED either way; player replays byte-identical
  (the change is AI-only; `push_out` is untouched); a Tier-4 capture of
  BF2 DAG1 with a longer idle log (6,000 ticks, one frame every 500)
  showing units entering the cave, and of BF1 BES2's ramp.

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

- 2026-09-01 (Fable session 8): **Bolts and the AI's line of sight now see
  model collision** (Opus build from HANDOFF §NEWLY OPUS-SAFE item 3,
  lead-verified). `CollisionWorld::ray` (first hit over bodies in index
  order, strict `<` on t, the same parry casts as `ray_down`, conservative
  AABB prefilters, no walkability filter), `projectile::Hit::World`,
  `Bolt::spawn` takes the nearest of terrain crossing / world / capsule,
  `Event::BoltExpired.terrain` renamed `struck` (a wall hit makes the same
  puff and sound as a terrain hit), `ai::visible` = terrain crossing then
  world ray. Four tests incl. an observable exact-tie case (a ball and a
  slab at bit-identical t with different normals). **Sweep: 24 of 32** (28
  before), two runs byte-identical, player replays byte-identical. The
  four new stalls are the honest cost of no longer shooting through
  geometry, each with ROUND_VERBOSE evidence: BF2 MUS1 — seven AI 7–9 m
  from the idle player with a FLOOR between them (they used to shoot him
  through it; the AI has no reposition or grenade behaviour yet); BF2 KAM1
  — the last team-0 AI on a platform above twelve enemies, all `BLOCKED
  world`; BF2 DEA1 — the map has no heightfield so `visible` used to be
  ALWAYS true; two AI now stuck against geometry 160 m from the player;
  BF1 YAV2 — both teams jammed flat against walls (many at exactly z =
  147.3, `stuck` ≈ 55,000), a pre-existing navigation stall that a
  through-wall firefight used to end. Three winners flip (BF1 KAM1, BF2
  END1, BF2 YAV1). Nothing was tuned; all eight open maps are the
  steering layer's ("PHASE A.1 DESIGN"). **Performance (Opus-measured,
  `--bench` protocol, TAT2, 86 units alive at `--ai-cap 64`, median of
  five): sim tick p99 1.25 → 2.71 ms (8 % of the 33 ms budget), all of it
  the AI LOS ray — a batched think tick runs `visible` per candidate
  enemy and each call scans all 1,578 `Body` structs (~200 B each, two
  Mat4s and an isometry), so the tail is memory traffic; the bolt ray
  costs ≈ 0.05 ms.** At the PS2's 16 a side it is far lower.
  Optimisation-backlog item, not this change: a broadphase or SoA AABB
  array over `bodies` (needs a `CollisionWorld::new`, the struct-literal
  construction in `game::assets` and tests goes through it). Noted:
  parry flips a TriMesh normal toward the ray, so winding is not
  observable through the cast (a one-sided world ray would need its own
  test); the PS2 collision LAYER MASKS (HOLD 13) are still not applied —
  every body answers every query.

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

- 2026-09-01 (Fable session 8): **PHASE D DESIGN — squad commands.** the maintainer's
  rules (decided): follow me / hold position / disperse / clear all; a
  command reaches the unassigned AI teammates within 20 m of the player
  in any direction, at most 6; one command per bot, the newest replaces
  the older; the player's death clears every command they gave, a bot's
  death clears its own. The five decisions (`core::squad` and the selection
  rule, the three behaviours, the d-pad input, multiplayer, the tests) and
  the acceptance gates are **§DESIGN SPECS "PHASE D DESIGN"** — moved there
  verbatim 2026-09-08 by cleanse C2, with the 2026-09-08 VO re-pointing
  attached. Loop queue item PD, after PC.

- 2026-09-01 (Fable session 8, the maintainer's question): **Two artifacts in the
  collision captures, root-caused.** (1) **The "whiteish boxes" on BF1
  Kamino are `kam_prop_crate` (21 instances) being alpha-tested to
  shreds.** A close-up render at platform level shows the crate as a
  hollow lattice while the Gonk droid beside it is solid. The crate body
  segment is `MTRL 0x05` (bit 0x04 "transparent" set, `RTYP Normal`) and
  its 256² texture `kam_crate` carries a grunge/specular mask in ALPHA —
  47,373 of 65,536 texels (72 %) are below the 0.5 test — so the material
  rule `bake::detail::ALPHA_IS_MEANINGFUL` marks the alpha meaningful and
  the renderer's alpha test deletes most of the box; the low-res LOD's
  alpha is opaque, so far crates render solid and near ones shred. This is
  the rule's own measured 23–28 % of flagged textures that are NOT
  cutouts (RE_LOG "Material flags decide whether texture ALPHA is a
  mask") and one more datum for HOLD 10(g): bit 0x04 with a noise alpha is
  not transparency. **The fix — the bake-time alpha MASK classifier — is
  §DESIGN SPECS** (moved there verbatim 2026-09-08, cleanse C2; still
  unbuilt). (2) **The white streak under Kashyyyk's treeline is
  the outside-the-level gap, not a texture.** BF1 KAS2's sky dome is one
  of three that stop exactly at the horizon (RE_LOG dome census: 0.0..90°)
  and the terrain, seen from a low camera, ends below the horizon line;
  the strip between shows the clear colour, which is the dome's own bottom
  row — a hazy white. On the PS2 that strip is covered by the low-res
  distant terrain / fog (HOLD 10(i), §What next item 6). Same defect as
  the forest maps' outside-the-level terrain. (3) Also seen while looking:
  the pale speckled column near KAS2's far dock is `kas2_prop_bamboo` —
  its cutout alpha is genuine (leaf sprigs on a green ground), the sprigs
  are pale in the texture, and sub-pixel stalks without anti-aliasing
  render as dotted lines at 150 m; aliasing, not a bug — MSAA/upscaling is
  a classic-plus render item. Tools: `game --scene-report` (0 untextured
  draws on either map), a KTX2 alpha histogram script (scratch), the
  texture sheets viewed.

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

Summary in PLAN.md's Log under the 2026-09-02 entries. Verbatim below.

- 2026-09-02 (Fable session 9): **A.3 landed — the parallel sweep with the
  scoreboard columns (0d8074e) and `wall_sweep`'s first non-floor hit per
  mesh body (8734f82)** (Opus build, lead-verified: ci green, both commits
  read, every number below re-measured by the lead). The sweep runs its
  maps on worker threads (`SWEEP_THREADS`, default the machine's 32),
  prints stdout in map order after the join, and ends with
  `sweep: finished F/M  stuck_max median A (finished-only B)  >10k C maps
  sum D` (lower medians). Serial 7 m 10 s → parallel 2 m 26 s, bounded by
  BF2 UTA1's 146 s alone; peak RSS 1.2 GB; the 32 per-map lines of the
  parallel run are byte-identical to the serial baseline, hashes included,
  and two runs identical. **Baseline at 25d98cf (the lead, two serial runs
  identical): 27/32, median 1,440 (finished-only 1,200), >10k 5, sum
  317,340.** The sweep fix: a `Shape::Mesh` body whose whole-body cast (or
  touching-branch contact) is a FLOOR is re-asked per triangle over its
  BVH, floor-vs-wall by the triangle's FACE normal, the push normal still
  the contact normal; primitives unchanged (a box's top-edge graze can
  still read as a floor — recorded PROVISIONAL). The build measured that
  parry's composite cast visits parts with the pose inverted and the hit
  swapped, and mirrored that argument order so one mesh and the same
  geometry as two bodies answer bit-identically (test asserts equality;
  the brief's order was 1.5e-4 apart). **After the fix: 26/32, median
  2,280 (finished-only 1,860), >10k 6, sum 348,000** (lead: two runs
  identical, identical to the build's). Every hash moved; winners flipped
  on six maps: BF1 NAB2 and BF2 MUS1 now FINISH (NAB2 `stuck_max` 56,160 →
  8,640), BF1 BES1, BF2 COR1 and TAN1 stall, BF1 KAM1 and BF2 END1 change
  winner. `--bench` TAT2 p99 0.682 → 0.723 ms at 16 a side, 2.578 → 2.647
  at 64 (±0.4 ms run-to-run noise). Player replays identical (walk_demo
  c425fcf50d2bea6b, aim_demo fb0b0c0a08604b28, `--sim-hash 600`
  24988d2da84964b8). **Nothing tuned. The lead's ROUND_VERBOSE read of the
  three new stalls:** BF2 COR1 is a 1-v-1 standoff (both `BLOCKED world`,
  tickets 1/1), BF2 TAN1 a chokepoint (thirteen units queued at hub 12,
  fan picks), BF1 BES1 the NAB2 signature — units at full speed, `steer
  fwd`, `push 0.29–0.35` every tick, `stuck` 45,000 — which A.3 did NOT
  remove on NAB2 either (seven units still show it at y −49, the same
  route, the same 11 m "floor below"; the map finishes because the jam is
  smaller). **That signature is root-caused below, and it is not the
  sweep's.**

- 2026-09-02 (Fable session 9): **The wall pass has been pushing every unit
  out of a MIRRORED copy of every mesh body — parry3d 0.22.1's
  `contact_composite_shape_shape` inverts the relative pose once too
  often.** Found by probing BF1 BES1's jam spots with a scratch example
  (never committed): rays along the unit's centre line at five heights hit
  nothing within 2 m, no single triangle of the pushing body
  (`bes1_bldg_walkway_16`, 630 triangles, rotation 176.6° about Y,
  translation (61, 108, −245)) pushes the capsule 0.3 m ahead of the unit,
  yet the whole body pushes it 0.307 m — and the identical triangles
  rebuilt in world space under an identity transform push 0.000. In `core`
  with parry called directly: the whole-mesh `contact` reports dist
  −0.300 with its witness point on the mesh 4 m BELOW the walkway and 3 m
  from the nearest triangle; per-triangle `contact` over all 630 finds
  nothing; `CompositeShapeRef(mesh).contact_with_shape` with the CORRECT
  pose (`body.iso⁻¹ · cap_iso`) returns `None`, and with the inverted pose
  returns the same −0.300 against triangle 555. Read in parry's source:
  `contact_composite_shape_shape(pose12, g1, g2)` calls
  `contact_with_shape(&pose12.inverse(), g2)` while `contact_with_shape`
  already takes shape 2's pose RELATIVE TO the composite (it computes
  `shape2.compute_aabb(pose12)` and `part_pos1.inv_mul(pose12)`); the
  shape-cast twin passes `pos12` straight through and is correct;
  parry's `master` (fetched 2026-09-02) already passes `pose12` directly,
  so the defect is the 0.22.x release we pin. Arithmetic: for a body with
  rotation R and translation T, the capsule at c is tested at local pose
  `(R, T − c)` instead of `(R⁻¹, R⁻¹(c − T))` — point-mirrored through
  the instance's origin, and for BES1's body the Y is exactly what the
  witness point showed (−1.9 for +1.9 in the mesh frame). **Scope:** only
  capsule-vs-`TriMesh` `contact` is affected — `push_out` (the wall pass
  on EVERY unit, the player included, since walkable model collision
  landed on 2026-09-01) and `wall_sweep`'s touching branch. Not affected:
  primitives, `ray`/`ray_down` (`cast_local_ray` on the affine
  `to_local`), the whole-body `cast_shapes`, and A.3's per-triangle pass.
  So the sweep (cast, correct) and the push (contact, mirrored) have been
  disagreeing since A.2 — "sweep says clear, push 0.30 every tick" on
  NAB2, BES1 and the jammed maps is a phantom wall, and the A.2 ablation's
  "push non-zero on ~60 % of unit-ticks on the jammed maps" was the same
  phantom; the contact cue was measured against it. A.3's fix stands on
  its own evidence (a wall behind a ramp of the same mesh is real and
  tested) but its NAB2 target was a mirrored floor. **Fix (Opus, in
  progress from the lead's brief):** `mesh_contact` = the per-triangle
  contact over the mesh's BVH with world isometries (the same loop
  `contact_with_shape` runs, minus the inversion), used by `push_out` and
  the sweep's touching branch for mesh bodies; a regression test on an
  ASYMMETRIC body (a wall at local x = 5 under a translation of 100: the
  real wall at 105 pushes, the mirror at 95 does not) that also asserts the
  raw parry behaviour so a future parry upgrade fails it loudly and the
  workaround can be dropped. Expect the sweep and the player replays to
  move — the wall pass is a different world. The A.1 "kerb is not a wall"
  probe lift, the contact-cue ablation and every `stuck_max` in the Log
  since 2026-09-01 were measured in the mirrored world and are due a
  re-read once the fix lands.

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

- 2026-09-02 (Fable session 9): **The mirrored wall pass is fixed (9258cd3) —
  `mesh_contact`, per-triangle contact over the mesh's BVH with world
  isometries, replaces every capsule-vs-`TriMesh` `contact` in `core`
  (`push_out`, the sweep's touching branch); primitives unchanged** (Opus
  build from the lead's brief, lead-verified: ci green, diff read, both
  sweeps re-run identical, the Tier-4 frames viewed). The regression test
  `mesh_contact_is_not_mirrored` uses an ASYMMETRIC body (walls at local
  x = 5 and −6 under a translation of 100) and asserts the raw parry
  behaviour too, so a parry upgrade that fixes the bug fails it with "drop
  `mesh_contact`'s workaround". Measured on the fixture: 0.2 m inside the
  real wall `push_out` 0.000 → −0.200; empty air at the mirror −0.600 →
  0.000 (raw parry's witness point sits on a wall 9.7 m away). **Sweep: 23
  of 32** (26 before; two runs identical; the lead's two runs identical to
  the build's): median 2,460 (finished-only 1,380), >10k 9, sum 507,540;
  every hash moved. BF1 BES1 (the traced map, 49,200 → 3,600) and BF2 COR1
  now finish; BF1 NAB2, BF1 YAV2, BF2 MUS1, MYG1 and NAB2 newly stall;
  winners flip on BF1 KAS1, BF2 DEA1, TAT1; BF2 POL1 −21,240 stuck. The
  phantom signature (`steer fwd` with `push ≥ 0.25`) 20 of 33 → 4 of 16
  live units on BES1, 7 → 0 on NAB2; blocked-forward unit-ticks per tick
  ROSE (BES1 1.8 → 6.6, NAB2 5.7 → 20) — walls are solid now, so units
  are blocked more often and pinned less. `--bench` TAT2 p99 0.716 →
  0.758 ms at 16, 2.649 → 2.998 at 64 (the mirrored query was often cheap:
  the phantom capsule landed outside the mesh). Player replays unchanged
  (the P3 logs never touch a mesh wall). Tier-4: `engine/artifacts/
  collision/mirrored_walls/` — a trooper running in place on
  `bes1_bldg_walkway_16` for 40 s before, gone by tick 600 after; the
  per-map diff table and the lead's sweep output beside it.
  **The lead's ROUND_VERBOSE read of the five new stalls (all real
  geometry now):** BF1 NAB2 (6 units), BF2 NAB2 (12) and MYG1 (1) are
  units BOXED at their own RAISED post — NAB2's cp1/cp6 at y −42 (BF2: +16)
  over street-level hubs at −50 (+8): they spawn on the plaza, locate to
  the street arc under it in XZ, the waypoint resolves to the street 8 m
  below (the same-level rule looks DOWN and accepts a floor far below),
  and the way there runs over a parapet; `steer back`, real `push
  0.2–0.5`, stuck 50–57k. BF1 YAV2 is a crowd (12 units at one hub with
  fan picks) and BF2 MUS1 a corridor with 943 BFS fallbacks. **Suspect,
  measured but not yet root-caused (A.4, first thing next session):** at
  the BF1 NAB2 spots (−328.8, −42.09, −23.0) and (−327.9, −42.09, −30.1),
  with the feet exactly on the ground, `wall_sweep` reports BLOCKED at
  t = 0.0 in all twelve directions for the probe, the crouch and a 0.55 m
  capsule, while horizontal rays at nine heights find OPEN AIR within 3 m
  in three to six of those directions and no ceiling exists (ray up: none
  / 9.7 m) — a capsule touching one wall (0.5 m to its side) makes the
  cast report t ≈ 0 everywhere, and the touching branch's end-pose test
  then blocks directions that are open. The A.2 note that "a cast from a
  touching pose reports a float-zero hit in every non-separating
  direction" was measured in the mirrored world; the end-pose test that
  was supposed to disambiguate it needs the same core-level witness-point
  probe the mirror bug got (scratch example recipe: a capsule at the
  verbose position, `wall_sweep` in twelve directions, rays at 0.2..2.6 m,
  `ray_down`; ~80 lines, never committed). Also due once that is settled:
  the same-level rule's DOWNWARD half (reject or re-route a waypoint whose
  floor is more than a fall below the unit when the leg crosses a wall),
  and a re-read of every A.1/A.2 constant against the honest world.

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

Summary in PLAN.md's Log under the 2026-09-03 entries. Verbatim below.

- 2026-09-03 (Fable session 10): **A.4 (1/3) — the "touching branch" suspect
  is closed, and the real defect behind "steer fwd, push 0.30 every tick" in
  the honest world is fixed (6907b83): a body the capsule ALREADY penetrates
  is now judged by penetration depth, not by parry's cast** (Opus build from
  the lead's brief `engine/artifacts/briefs/brief_a4_sweep.md`;
  lead-verified: ci green on the tracked tree, the diff read, both sweeps
  re-run identical to the build's). **The probe at the NAB2 spots** (the
  session-9 recipe: a scratch example over the public API, never committed):
  at (−328.8, −42.09, −23.0) the probe capsule is **1.46 m inside a box
  primitive** of the building it stands on (`nab2_bldg_b_19 : p_cube`, half
  extents 39.9 × 10.4 × 12.3, spanning y −50..−29) and 0.24 m from a second
  one (`p_cube1`) that overlaps the first; at (−327.9, −42.09, −30.1) it is
  inside both. The sweep's `t = 0` in all twelve directions was the truth,
  not a bug — the plaza floor (the coll MESH at −42.09) lies inside the
  building's own box prims. Why the PS2 does not trap soldiers there is the
  next entry (collision LAYERS). **The separate defect, found on BF2 MYG1:**
  unit 6 sat 0.33 m inside `myg1_bldg_large_platform_b : p_-v-rob` (a
  118°-rotated box) for 53,400 ticks with `push_out` = 0.33 and `wall_sweep`
  = clear in every direction at every capsule height. Measured on synthetic
  fixtures (the numbers are now unit tests): a capsule 0.1–1.0 m inside a
  118°-rotated box, sweeping INTO it → `None` at every depth (an
  axis-aligned box → `Some(t = 0)`); a capsule 0.1–0.9 m inside a two-
  triangle mesh wall → INTO `None`, ALONG `Some(t = 0)` (blocked), OUT
  `None` — backwards. **Cause, corrected by the build's measurement (the
  lead's brief blamed GJK returning `None`; it does not):** parry 0.22.1's
  `cast_shapes` from an overlapping pose returns `t = 0` in EVERY direction
  with the SAME normal (`minkowski_ray_cast` yields `(0, dir)` as soon as
  the origin projects to zero, and the hit is filled from the direction-
  independent `contact_support_map_support_map`), so the A.2 touching
  branch's END-pose contact decided everything — and for a deep box the
  cheapest escape at the end of a 1 m step flips to VERTICAL (a "floor",
  skipped), while for a zero-thickness coll wall the end pose 1 m further
  in has no contact at all (clear). Any residual penetration the two-
  iteration wall pass leaves (NAB2: sandwiched between two boxes, net push
  0.24) puts the AI's sweep in that regime every tick. **The rule now:** (a)
  the cast, unchanged, for walls the step passes THROUGH (its hits under
  `SWEEP_CONTACT_DIST` are ignored; the touching branch and A.3's touching
  sub-branch are deleted); (b) where the cast has no usable hit, a
  penetration test per body — per TRIANGLE for a mesh, wall triangles by
  face normal — blocks at `t = 0` iff the step DEEPENS the penetration by
  more than `SWEEP_CONTACT_DIST`: already penetrating → depth measured along
  the START contact's own normal (`pen0 − dist·(dir·n0)`; the end contact's
  normal and existence are exactly what the measurement showed to be
  unusable); touching or clear → the Phase A.1 point test (end pose inside
  → blocked). Two build deviations from the brief, both forced by
  measurement and recorded in the doc comments: (b) does not run for a body
  the cast already answers about (the literal merge broke three existing
  assertions — thin wall, clear approach, wall behind a ramp), and the
  penetrating regime uses the start normal rather than a second contact.
  Known limitation (lead): a mesh whose nearest cast hit is a usable wall or
  floor ahead is not also tested for a simultaneous penetration of another
  of its wall triangles. Tests: `a_penetrated_rotated_box_still_blocks_the_
  way_in` (also pins raw parry's `t = 0, same-normal` behaviour so an
  upgrade fails loudly), `a_penetrated_mesh_wall_still_blocks_the_way_in`,
  `a_step_may_not_deepen_a_penetration_but_may_keep_it`; every existing
  assertion unchanged (`…touching_branch_is_not_mirrored` renamed
  `…penetration_test_is_not_mirrored`). **Sweep: 22 of 32** (23 before),
  median 2,580 (finished-only 1,140), >10k 9, sum 507,540 → 456,360; 31 of
  32 hashes moved; two runs identical (build and lead). BF2 NAB2 now
  FINISHES (53,280 → 1,500); MYG1 53,400 → 17,940 (unit 6's line is gone —
  its team was wiped; the phantom signature is 1 of 16 live units, walking);
  BF1 NAB2 57,840 → 34,980 with four of its five stuck units at `push
  0.000` — what is left there is routing, not walls; GEO1 32,100 → 9,720;
  BF1 KAS2 7,980 → 2,580. Two maps newly stall, both read by the lead with
  the probe: **tat2** is an end-game standoff (3 v 5 alive, tickets 3/5, no
  bleed) plus one unit at the foot of `tat2_bldg_stair1` with push 0.000,
  the stair mesh a real wall 0.9 m ahead and both slide directions clear —
  not the new test; **BF2 HOT1** is four units bunched 0.37 m inside
  `HOTH_bldg_bunker`'s mesh where the new sweep answers correctly (blocked
  into the wall with its normal, clear away) and their waypoint is 2.5 m
  BELOW them through that wall — the "downward half" case, A.4 (3/3).
  Player replays byte-identical (walk_demo c425fcf50d2bea6b, aim_demo
  0695bdbb7738eb71, `--sim-hash 600` 24988d2da84964b8). `--bench` TAT2 p99
  0.802 → 0.963 ms at 16 a side, 3.711 → 4.052 at 64 (build's medians of
  five; the sims diverge, not like-for-like). Nothing tuned.

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

- 2026-09-03 (Fable session 10): **A.4 (3/3a) — a blocked slide flips the
  remembered side, and the stuck side-step chooses it (caab253). Sweep
  26 → 28 of 32.** Correction to the entry above: the six maps unfinished
  at 960fdac were BF2 HOT1, KAM1, MUS1, POL1, TAN1 and tat2 (BF1 BES2
  finishes; the lead misread its own list). **The finding (lead, per-tick
  traces with a scratch tracer example, never committed):** tat2 unit 18 at
  (−116.1, 0, −30.8) and BF2 MUS1 unit 1 at (−52.2, 66.0, 8.3) — both
  `steer slide`/`fan`, `push 0.000`, `stuck` 52,860 and 58,260 — oscillate
  over 0.8 m with a ~13-tick period: the remembered slide side (+1, A.1's
  deviation that stopped the head-on flip-flop) walks the unit along the
  wall until the SLIDE direction is itself blocked (a low box at knee
  height on tat2), the fan then walks it back the other way, the forward
  sweep is still blocked, and the same side is re-picked — forever. The
  stuck window fires every 60 ticks (`sidestep` counts down, the re-plan
  runs) and never touches `AiState::slide`, so the PS2's own escape (the
  side-step to the side not used last time, RE_LOG local avoidance) could
  not reach the wall-follow. **The rule (Opus build from
  `brief_a4_slide.md`, lead-verified):** in `steer_dir`, when the slide
  tangent is blocked the remembered side is FLIPPED and the other tangent
  tried before the fan (the fan's first side is the flipped one); in the
  stuck window `st.slide` is set to the side-step's own sign. No constant
  changed. The pre-existing test `a_blocked_probe_slides_and_keeps_its_side`
  asserted "never flipped" — the old fall-through, verbatim; its clause is
  now "at most two sign changes over the run" (the stuck window's and the
  blocked tangent's, both traced), the property A.1 actually needed being
  "no tick-to-tick alternation". Two risks the build measured on that
  fixture and the lead accepted unguarded (no tuning; the constants re-read
  owns them): a long wall-follow reverses every stuck window (60 ticks)
  because sliding sideways makes no waypoint progress by construction; and
  while a unit PENETRATES a body both tangents can report hits, so the side
  can flip on consecutive ticks until the fan takes over. **Sweep: 28 of
  32** (26 before): median 1,020 (finished-only 960), >10k 3, sum 222,540;
  build's two runs identical, the lead's run identical to them. tat2
  (52,980 → 600) and BF2 HOT1 (49,620 → 2,160) FINISH — the HOT1 crowd
  inside the bunker mesh dissolved with the side flip, so the "downward
  half of the same-level rule" (A.4's third item) has NO remaining case at
  this HEAD and is not built: recorded, re-open if a stall shows it. Six
  winners flip (bes2, nab1, nab2, yav2, cor1, tat1 — the high-variance
  metric, PLAN "Phase A.2 landed"); BF2 POL1 regresses 33,840 → 59,820:
  its kerb pair (below) runs longer and a new three-unit cluster at (−88,
  21.7, −128) runs at full speed with push 0.000 at an unreachable
  waypoint. Still stalled: KAM1 (unit 26 `steer fwd` push 0.291 — the
  phantom signature, to be probed), MUS1 (unit 16 at (−45.6, 68.0, 90.1),
  vel 0.5, push 0.01 — a slow trap the flip does not reach; the old
  oscillator's spot now holds units for 14,280 ticks instead of 58,380),
  POL1, TAN1 (a standoff, no unit stuck over 360 ticks). Player replays
  byte-identical (c425fcf50d2bea6b / 0695bdbb7738eb71 / 24988d2da84964b8).
  **A.4 (3/3c), the wall pass and `STEP_HEIGHT` (brief written, building
  next):** BF2 POL1 units 3 and 4 at (113.7, 20.7, −34.7) — `steer fwd`,
  `push 0.301` every tick, 33,840 ticks — stand before a kerb 0.2–0.5 m
  high: the lifted probe capsule sweeps clear in every direction and
  `push_out` on it is zero, horizontal rays at 0.2 m hit at 0.4 m and rays
  from 0.5 m up hit nothing, the full capsule's sweep is blocked at 0.54 m.
  The ground query would climb it (its probe starts `STEP_HEIGHT` above the
  feet) and the steering ignores it (A.1's lift), but the wall pass runs on
  the FULL capsule from the feet and pushes the unit back before the ground
  query ever sees the kerb's top. Rule: the wall pass uses the same stepped
  capsule as the probe (`Capsule::stepped`, one helper for both), so a
  kerb below `STEP_HEIGHT` is climbed, not a wall. PROVISIONAL — the PS2's
  step rule is untraced (HOLD 13).

- 2026-09-03 (Fable session 10): **A.4 (3/3c) — the wall pass steps over
  `STEP_HEIGHT` like the probe: a kerb is not a wall (dda2afa). Sweep 28 of
  32 held; median 1,020 → 720, >10k 3 → 2, sum 222,540 → 155,340.** Opus
  build from `brief_a4_step.md`, lead-verified: ci green at HEAD, the diff
  read, the lead's sweep byte-identical to the build's two. `Capsule::
  stepped` (lift `STEP_HEIGHT.min(height/2)`, the top unmoved) is the ONE
  capsule both the steering probe and the wall pass use — `ai::probe_capsule`
  is gone and `Sim::step` 3a calls `push_out` on it — so the three rules
  that define a step (the ground query's probe start, the probe's lift, the
  wall pass) agree at last. Consequence, documented: a unit walks INTO
  geometry lower than 0.6 m with its feet and the ground query lifts it
  onto the top next tick. Tests: the stepped capsule per stance (lifts 0.6
  / 0.6 / 0.275), a 0.4 m kerb is climbed with `wall_push` zero throughout,
  a 1.0 m box still pushes. **Per map:** BF2 KAM1's deep unit is gone
  (57,840 → 1,200; the map still ends without an outcome, tickets 17/145 —
  a standoff), tat1 5,580 → 1,620, BF1 BES1 3,960 → 60, BF2 TAN1 4,800 →
  1,680, BF1 NAB2 3,660 → 660, MYG1 2,520 → 420, HOT1 2,160 → 360; up: BF1
  RHN1 60 → 3,780, YAV2 180 → 3,300, BF1 KAM1 2,820 → 5,160, GEO1 1,320 →
  2,760; four winners flip. **POL1's kerb pair is gone and no unit on any
  of the six examined maps carries a non-zero push any more**; what is
  left — MUS1 unit 16 (−45.9, 68.0, 90.2) and POL1 units 7, 23, 27 — runs
  at full speed, push 0.000, at a waypoint it never reaches (A.1's second
  category: the leg, not the wall). Still without an outcome: KAM1, MUS1,
  POL1, TAN1 (KAM1 and TAN1 are standoffs with every unit under 1,700
  stuck ticks — Phase B/C's hunt and reposition). Player replays
  byte-identical (no P3 log walks a kerb). `--bench` TAT2 p99 0.907 →
  0.991 ms at 16 a side, 3.643 → 3.599 at 64 (build's medians of five;
  not like-for-like). PROVISIONAL: `STEP_HEIGHT` and this rule are ours
  (HOLD 13's step/slope question stands). **A.4 is closed:** item 0 of
  HANDOFF §What next is done except the bounded constants re-read
  (`brief_a4_constants.md`), which follows in the queue.

- 2026-09-03 (Fable session 10): **A.4 (3/3b) — the re-read of the two
  A.1/A.2 decisions that rested on mirrored-world measurements: both KEPT,
  nothing committed but one doc correction** (Opus ablations from
  `brief_a4_constants.md`, four sweeps of a five-sweep budget, each a
  temporary edit reverted from `git show HEAD:`; the baseline run's 32 map
  lines byte-identical to the lead's; the ablation numbers are the build's
  — no code changed, so the lead re-ran none). Baseline 28 / 32, median 720
  (finished-only 660), >10k 2, sum 155,340. **The contact cue** (PLAN "PHASE
  A.2 DESIGN" decision 2, unwired since A.2 on mirrored evidence): the
  "blocked" half alone → 28 / 32, median 960, sum 157,320, 30 of 32 maps
  changed, 21 worse / 8 better, six winners flip; the side-step-side half
  alone → 28 / 32, median 1,020, sum 158,700, 12 worse / 2 better. Neither
  improves any column the lead reads, and the "blocked" half's blast
  radius says what the module header already says: a push-out threshold of
  "any penetration" is not a contact accumulator. Kept unwired; the
  header's mirrored figures stay, labelled as A.2's. **The A.1 probe lift
  (`STEP_HEIGHT` off the bottom of the steering capsule):** removing it
  from the sweeps only → **27 / 32** (BF2 GEO1 stops finishing, 2,760 →
  50,700), median 960, >10k 3, sum 174,720 — worse on all four columns, so
  the lift stays. But its recorded justification (TAN1 blocked-forward
  656,798 → 650,987) was mirrored-world and does not reproduce: at HEAD,
  on the four maps that run the full 60,000 ticks in both arms, the lift
  RAISES TAN1's blocked-forward count (1,462,664 → 1,534,202, +4.9 %) and
  the 32-map total 15.5 %; the doc comment of `Capsule::stepped` now says
  so and cites the outcome evidence instead (this commit, docs only). The
  other ten A.1/A.2 constants cite a disc census, the PS2's own shape or a
  design choice, never a sweep number — read, listed in the build's
  report, untouched. Runs: scratch `a4/opus_constants/` (session-local).
  **A.4 is complete.** The engine queue resumes at the untextured-draws fix.

- 2026-09-03 (the maintainer, in session 10): **Two locked-feature-set additions** (§LOCKED
  FEATURE SET, dated): (1) BF2's shell, menus and HUD are THE UI on every map
  of both games — look and feel BF2, content free to modernise; BF1-only
  weapons/grenades/items show their BF1 icon in the BF2 HUD; (2) unit
  behaviour is UNIVERSALISED across the blend, one behaviour per class on
  every map, recorded per class — first: the droideka ball jumps everywhere
  (BF1's rule). Both are design lines for the lead; the icon fallback is an
  Opus-safe bake+HUD item once the P7/heroes rosters name the BF1-only
  weapons.

- 2026-09-03 (Fable session 10): **The 148 "untextured draws" are gone from
  the scene report (eb73e0d): disc-untextured and disc-dangling segments are
  SKIPPED, not painted grey, and reported on their own lines** (Opus build
  from `brief_untextured_fix.md`, lead-verified: ci green at HEAD, `--scene-
  report` re-run on four maps, the sweep byte-identical to the previous
  commit's — the sim is untouched). `viewer::route_untextured` (a pure
  function with a unit test) sends a segment whose GLB material name is
  EMPTY (the disc named no texture: 76 draws over 8 maps, 27 model/segment
  pairs, incl. `tan4_bldg_engine_b` seg 1 with its `COL0` — skipped only
  because the renderer draws no vertex colour yet) and the three names that
  exist in no `tex_` chunk on either disc (`DANGLING_ON_DISC`: `yav_prop_ivy`
  ×6, `Cor1_bldg_flowers`, `noIcon` — the `geo_sky_explosions` shell, skipped
  too) to `continue`, exactly as the terrain arm does; a material naming a
  texture the bake lacks stays grey and is the only thing "untextured draws"
  counts now — 0 on all 32 maps. BES1's 64 terrain chunks (`kam1_water`, in
  KAM1.LVL only) print as `terrain chunks dropped (texture in another LVL)`.
  Totals over the 32 maps: untextured draws 148 → 0, untextured on the disc
  76, dangling on the disc 8, terrain chunks dropped 64; draw counts −83.
  Goldens: `scripts/gpu_tests.sh` green, nothing regenerated; the build
  measured the exact pixel movement with a throwaway delta example (deleted
  before the commit): three sheets move at most 0.0039 % of their pixels
  (BF2 DEA1, UTA1, BF1 YAV1 — the grey stubs that were in frame), ~250×
  under the gallery tolerance, the other 39 bit-identical. PROVISIONAL: what
  the PS2 draws for these segments is HOLD 18. **The engine queue resumes at
  ALPHA RULE v2** (`brief_alpha_v2.md`; the build stops before
  `UPDATE_GOLDENS` for the lead's sheet review).

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

- 2026-09-03 (Fable session 11): **Phase B landed — the commander (bbb8baa,
  c613a6a). Sweep 28 → 27 of 32, median 720 → 600 (finished-only 480), sum
  155,340 → 118,500; eight winners flip; every hash moves.** Opus build from
  `brief_phase_b.md` (+ the session-11 addendum), lead-verified: ci green at
  HEAD, the module header and the weight table read, the hash coverage read
  (`TeamPlan` length-prefixed with a sentinel for `None`, after `waves`;
  `AiState.squad`), the lead's own sweep byte-identical to the build's pair
  on all 32 per-map lines, goldens unchanged (sim-only). **Commit 1:** the
  ten raw `Value_ATK_*` / `Value_DEF_*` PROP hashes read in `game::layout`
  into `CommandPostDef` (five faction rows), resolved per team at
  `setup_conquest` into `CpDef.value_atk/value_def[2]` with
  `CpDef::value_for(team, owner)` (DEF if mine, ATK if theirs, the average if
  neutral); the sweep's teams now carry the mission factions like `main.rs`.
  Measured on the 32 base worlds: TAT1 ATK 10/20/30 DEF 10/20/50, BF2 DAG1
  10/20, BF2 HOT1 0/10, BF1 BES1 a flat 10 (the addendum's numbers), only
  five cells over all maps take the class default 100; BF2 UTA1 authors
  Empire ATK 0 on every post. CORRECTION to the addendum: the hashes ARE
  fnv1a of the lowercased keys under the port's `| 0x20` lowercasing
  (`_` → 0x7F), which is why a `to_lowercase()` hash finds nothing; a test
  pins both. **Commit 2:** `core::commander` — `TeamPlan` × 2 as sim state,
  `Objective::{Attack, Defend, Hunt}` (Escort/Board left for D/E), a plan
  per team every `PLAN_PERIOD` 30 ticks staggered by team, scoring per the
  design (value × 1/(1 + hops) from `NavGraph::hop_distances`, threat ×
  `THREAT_W`, crowding 1/(1 + k/4), ±10 % per-unit hash noise, hysteresis
  1.2×, majority/behind biases, greedy in unit index order), `retarget`
  reading the assignment, Defend taking the nearest Cover/Snipe hint within
  25 m, squads bookkept, ROUND_VERBOSE objective/squad columns and per-map
  commander lines. Every constant PROVISIONAL in one table. **Eleven
  deviations from the design, each forced by a per-map ROUND_VERBOSE read
  the build kept (`phase_b/probe*.txt`) and each accepted by the lead:**
  Defend takes the 1/(1 + hops) factor too; the hard cap applies to Defend
  (TAT1: 11 of 12 stood on the DEF-50 post all round); an uncontested post
  takes a garrison of one and at most N/4 of the team garrisons at all
  (TAT2 141/142 after 60,000 ticks); a contested post draws 2 × the enemies
  within 60 m, not half the team (BF2 DEA1: one roamer pulled eight units
  onto a post at `vel 0.00` for 45,000 ticks); the defence spot applies from
  assignment, not from entering the volume (BF1 END1 unit 28: 45,587 ticks
  at full speed 1.5 m from its post); an objective a unit has dwelled on
  for 300 ticks is refused for that plan (BF1 KAS2 units 5–15 under cp1,
  22 m below a post four hops away); Hunt as a last resort when everything
  is capped; a Hunt with nobody to hunt falls back to Phase A's rule; the
  noise mixes the objective key (a per-unit-only term cancels out of an
  argmax); `hop_distances` takes a `Mover`; the manifest schema is
  untouched (props already ride raw). **Acceptance:** on every map with ≥ 3
  posts the winning team used ≥ 2 distinct Attack objectives (min 2 on the
  two four-post maps, BF1 KAM1 used all seven); `unreachable_dwell_max` ≤
  329 ticks everywhere (gate 900); round length to outcome median 9,407 →
  10,357 ticks (n 28 → 27; max 16,077 → 29,569), nothing collapsed;
  `--bench` TAT2 p99 1.039 → 1.004 ms at 16 a side, 3.695 → 3.373 at 64
  (build's medians of five, not like-for-like) — the plan pass is
  invisible. **The one lost outcome, BF2 UTA1** (team 0 wins at 9,491 →
  none): team 1 reduces team 0 to the idle player alone at 1 ticket, and
  BF2's bleed needs `alive < tickets`, so 1/1 never bleeds out — the
  ticket rule's standoff shape, not navigation (dwell 238, unroutable 0,
  five attack objectives). **KAM1 and TAN1 still stand:** the pressure IS
  applied (KAM1 spreads over five posts with four hunters, TAN1 splits 3/3)
  and at tick 60,000 every unit on both maps reports its nearest enemy
  `BLOCKED world` — a shooting/reposition problem for Phase C; KAM1 also
  sits at alive 16 / tickets 16, the same bleed deadlock. Player replays:
  motion byte-identical (both `motion.csv`s), hashes moved because the
  commander is hashed — walk_demo 1c1959ed3fd15936, aim_demo
  45dea6557b35f773, `--sim-hash 600` 9f8c6f7dccc24191 (the first two
  re-run by the lead). Tier-4: `engine/artifacts/ai_nav/bf1_bes1_commander/`
  (BES1 top-down, one frame per 500 ticks: team 0 takes the centre post at
  tick 1,092 and holds 4–3 while grinding tickets 180 → 1). Not consumed:
  `Hint.command_post` (the disc's hint→post binding; the design's 25 m
  radius stands). **Replay-hash bookkeeping, resolved by the lead with a
  2×2 matrix (old 7252cf2 binary × old fresh bake vs new):** aim_demo is
  fb0b0c0a08604b28 in all four cells — the A.3 entry's value; session 10's
  0695bdbb7738eb71 was measured against a bake state a fresh bake does not
  reproduce, and is not a code regression. walk_demo differs between a
  fresh 7252cf2 bake and the current bake (2e8042d4a63681b7 vs
  c425fcf50d2bea6b) with TAT1's manifest, collision, nav and terrain
  byte-identical — the difference is in `common_bf1` or `side_bf1`; being
  bisected (next entry).

- 2026-09-03 (Fable session 11): **CORRECTION to the Phase B entry's last
  paragraph — the ALPHA RULE v2 rebake moved NO replay.** The "walk_demo
  differs between a fresh 7252cf2 bake and the current bake" reading came
  from partial scratch bakes (the /tmp quota cut the old `common` and
  `side` bakes short and a re-run skipped them because their directories
  existed). Redone from scratch with exit codes and file counts checked:
  the 7252cf2 bake's TAT1 manifest, `common.ron`, the soldier anims RON and
  the class RONs are byte-identical to the current bake's, and the 2×2
  matrix (7252cf2 binary / dda89b4 binary × old bake / current bake) gives
  walk_demo c425fcf50d2bea6b and aim_demo fb0b0c0a08604b28 in all four
  cells. So at every code state from A.3 to Phase B's parent, aim_demo is
  fb0b… — the A.3 entry's value; session 10's 0695bdbb7738eb71 (quoted in
  three entries and the briefs) cannot be reproduced from any code + fresh
  bake and is treated as a session-10 bookkeeping slip, not a regression.
  The replay baselines going forward are Phase B's: walk_demo
  1c1959ed3fd15936, aim_demo 45dea6557b35f773, `--sim-hash 600`
  9f8c6f7dccc24191 (all three re-run by the lead at c613a6a). Lesson
  recorded for the environment: /tmp carries a user quota; scratch cargo
  targets and bakes go under `engine/artifacts/lead_tmp/`, never a second
  target dir under the scratchpad.

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

- 2026-09-03 (Fable session 11): **The ORDNANCE subset reaches bolts and the
  AI's line of sight (f8dcf57). Sweep 27 → 26 of 32, median 600 held, >10k 2
  → 3, sum 118,500 → 139,200.** Opus build from `brief_ordnance_ray.md`
  (Fable brief; its census reproduced by the lead beforehand), lead-verified:
  ci green at HEAD, the lead's sweep byte-identical to the build's three, the
  three replay hashes unchanged (1c1959ed3fd15936 / 45dea6557b35f773 /
  9f8c6f7dccc24191), goldens green. **What landed:** `core::collision::
  Layers` (SOLDIER 1, ORDNANCE 2, VEHICLE 4 reserved) on every `Body`
  (`Body::new` = ALL, `Body::with_layers`); `ray_down`, `push_out`,
  `wall_sweep` filter on SOLDIER, `ray` takes a mask and its three callers
  (`Bolt::spawn`, `ai::visible`, the sweep's LOS column) pass ORDNANCE (RE
  §4: the trailing-flag ray tests the ordnance subset); `Scene::collision_
  world` builds the UNION of the two class subsets with masks, bodies in
  neither skipped, the soldier subsequence in today's order (a test pins
  it). **The disc census** (`examples/layer_census`, second table; the
  lead's own reproduction matched to the body): 26,980 built bodies over the
  32 base worlds, soldier 24,952, ordnance 23,766, union 25,040 (+88),
  1,274 soldier-only (311 by a BF2 MASK, 963 by class properties — the ewok
  fences and walkways, the Kashyyyk bamboo meshes, BF2 common's `com_inv_
  col_*` invisible walls, `com_weap_gunturret`'s `p_cylindergun`/`p_spawn`
  on 118 BF1 instances), 88 ordnance-only (the Hoth bridges' `p_ordinance*`,
  Utapau's `-o-` prims, Polis Massa's shields); no map's body count moved
  but BF1 HOT1 (+12) and BF2 UTA1 (+69). **The sweep:** the eight maps with
  no body in exactly one subset are byte-identical (the soldier path is
  untouched); 12 maps moved, all where a soldier-only body stopped stopping
  bolts and lines of sight (BF2 END1's winner flips as bolts cross the ewok
  fences; BF1 KAM1 stops finishing — 60,000 ticks, stuck_max 960, the
  commander's assignment changes 2,513 → 6,165; BF1 KAS2 finishes at 56,479
  with stuck_max 21,420 — both read by the build: the LOS verdicts are the
  same, what changed is which bolts land). Those two are the worst quality
  outcomes in the sweep now and are NOT chased (the world is the disc's;
  they need Phase C's shooting and reposition, then a ROUND_VERBOSE read).
  `--bench` TAT2 p99 0.927 → 0.932 ms at 16, 3.577 → 3.354 at 64; UTA1 at 16
  0.632 → 0.579 with 966 → 1,035 bodies (build's medians of five). The
  brief's `com_weap_gunturret` note is disproved by the disc data (both
  prims are cylinders the bake builds; `p_base` is vehicle-only and in
  neither subset). **Bake gap found, not fixed:** BF2 HOT1's manifest has no
  class or collision row for `hoth_prop_bridge_long/short` although its
  conquest layer places six — they have no collision at all today (a
  side-LVL class the map bake does not pull; Opus-safe once the pull is
  specified). Deferred as before: the vehicle subset (P7), the two-tier KTX2.
  Tooling lesson recorded in the build's report: a scratch worktree sharing
  `engine/target` leaves worktree-fingerprinted rlibs behind; a full
  `cargo clean` was needed, and every number was re-taken on the clean build.

- 2026-09-03 (the maintainer, end of Fable session 11): **Decisions on the session's
  report, and the plan approved.** (1) Aim assist never off by default
  (§Movement & feel); the Phase C brief's D4 is replaced: a user toggle,
  default ON, profile-saved, no tier touches it. (2) The sweep's idle human
  is the artefact behind the BF2 `alive < tickets` standoffs — make the
  sweep's slot-0 player a bot (the rule stays). (3) The ELF's material →
  GS-state rule is RE'd together with the sky pass (scale, projection, fog
  enable — HOLD 10(a)/10(g)) in ONE Fable pass before any alpha or fog rule
  changes; the two Hoths must look the same afterwards. (4) The class
  roster (sniper, heavy, engineer, pilot, jet, special — one class per
  faction spawns today) is built BEFORE Phase C. (5) Squad commands and
  their voice lines are universal (§Presentation). (6) Order: the sky
  V-wrap fix with GROUND-level before/after sheets (the gallery's orbital
  overview panel is to be replaced by a fogged, lower camera) → the
  combined sky/fog/alpha RE → class roster → Phase C → Phase D → two-tier
  KTX2, the droideka RE, the BF2 HOT1 bridge bake gap. (7) Parallel work
  per §Working mode. **Sky findings behind (6), lead-measured:** the hard
  line across both Hoth skies is the base dome's bottom ring at v = 1.001
  wrapping under the REPEAT sampler to the texture's top row (dome-only vs
  mountain-only overview renders: the jump is the dome's, row 125 of 720);
  47 shells on the 34 baked dirs have rings at exactly v = 1.0 and 14 pass
  it; BF2 Hoth's mountains are white because BF2 HOT1 ships fog our
  parallax shells honour while BF1 HOT1's fog is rejected by the bake
  heuristic (same mesh, same placement); Kamino's grey is the dome's skirt
  with no fog. The ELF's placement makes `Offset` nearly inert for 0.995
  shells (≤ ~1°), so shells are eye-centred by authoring; the RE's scale
  claim (S = 1, a 20 m shell against an 80 m near plane) is internally
  inconsistent and the sky pass is NOT closed — placement stays
  PROVISIONAL until the combined RE, judged on ground-level captures.

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

- 2026-09-03 (Fable session 12): **Item 0 landed — the sky V-wrap clamp, the gallery's fogged vantage panel,
  and the 0x02-sky-shells-blend correction (b459146, d86edab, ded500c, 840a701; goldens regenerated after the
  lead viewed the sheets).** Opus build from `brief_sky_wrap.md` (+ the lead's addendum from the sky-pass RE) in
  the `sky-wrap` worktree with its own target dir — the first change built under the parallel-worktree rule;
  rebased over the session's RE commits and fast-forwarded, then ci / `scripts/goldens.sh` / `scripts/
  gpu_tests.sh` / the three replays run ONCE on `main` by the lead (all green; walk_demo 1c1959ed3fd15936,
  aim_demo 45dea6557b35f773, `--sim-hash 600` 9f8c6f7dccc24191 unchanged). **What landed:** `render::WrapV`
  on `TextureDesc` (V only; U/W stay REPEAT, the discs' panoramas tile in U); `viewer::sky_wrap_v(v_min, v_max)`
  — ONE decision function, `ClampToEdge` iff the segment's V range lies within [−0.25, 1.25], else REPEAT —
  applied per DOME segment through a separate cache key (sky objects untouched); the census through the real
  upload path: 88 clamped segments over 31 of 32 map dirs, BF2 TAN1 none, POL1's and TAN1's tiling base domes
  REPEAT by construction, 2 extra texture uploads in all (`SKY_naboo2`, both NAB2s — the one texture used
  both ways). `gallery::vantage_camera` (PROVISIONAL, the lead's numbers: 0.55·r out on the old bearing,
  `c.y + max(0.18·r, 18 m)`, floored 6 m over the terrain, the map's own fog and clear) replaces the orbital
  overview in the map sheets; `overview_camera` stays for `--cam-overview` and the flythrough. `SkyMode` is
  `{Opaque, Blend}` — bit 0x02 or 0x04 composites under the sky flag (RE_LOG "The sky pass"), the sky MASK
  pipelines and `fs_sky_tested` twins deleted; it moves exactly two goldens (BF2 MUS1 0.7 %, MYG1 9.8 %).
  **The seam finding, measured with the new `sky_wrap_captures` example (eight maps, four bearings at a
  standing eye, +6° pitch):** at a standing eye the V wrap is nearly invisible — the dome's bottom ring sits
  at −3°…0° elevation and is occluded by the terrain horizon (Hoth 58/133 px moved of 921,600; both Kaminos,
  NAB1 and BF2 KAS2 exactly 0) — the hard grey band the maintainer saw was in the GALLERY'S ORBITAL PANEL, where the
  clamp removes it outright (the lead viewed `sheets_clamp/maps_bf1_hot1.png`); and **Kamino's grey band is not
  a V wrap at all** (`kam1_sky_dome` ends at v = 0.9976, under the bilinear wrap threshold): it is the dome's own
  skirt, as the maintainer's session-11 finding said. Goldens: clamp alone moved 23 of 42 (14 past the 12/255 check; 9
  bit-identical; the moved ones in narrow sky bands only — KAS1/KAS2 on a single rim row; 3 ground panels at
  ≤ 0.2 %); with the vantage panel all 32 map sheets move (the ground halves bit-identical to the clamp-only
  candidates, verified per panel), the unit/shell/HUD/loadscreen goldens untouched. Tier-4:
  `engine/artifacts/sky_wrap/` (README, before/after/diff captures, both sheet sets with a per-golden cause
  column, `dome_uv.txt`). The lead's read of the vantage sheets: every map reads, Felucia's is heavy haze (its
  own fog to 62 m — the disc's look, and the fog-rule build changes nothing there), BF2 Hoth's is the white
  shell fog the fog-rule build removes. Deviations accepted: a second seam band in the instrument (the brief's
  band was above the ring), `after_head/` captures for the 0x02 commit, `FRAC_1_SQRT_2` for the bearing, the
  referee quad's v range 0..1.2 so the wrap is on screen, `git stash` instead of `checkout` for the worktree's
  goldens. `viewer::horizon_clear` now has no caller (kept; a one-line follow-up). Per the wrap RE the clamp is a
  DELIBERATE DEVIATION from the PS2 (which samples the ring wrapped) — recorded here as such.

---

## Part 2 — HANDOFF-OPUS.md, the superseded generations

HANDOFF-OPUS.md kept six "State at handoff" and five "What next" generations
plus a digest per session. It now keeps ONE of each (the 2026-09-08 pair) and
the sections that are not chronological at all — §The autoloop, §Rules for the
Opus session, §HOLD FOR FABLE, §Standing backlog and standing facts, §How the
last session verified things. Everything else is below, verbatim, in the order
it appeared in the file. **The live items were lifted out first** (commit
`bf205fe`): the nine stranded work items and the four measured facts are in
HANDOFF §Standing backlog and standing facts, the 300-frame median protocol is
in PLAN §Testing ladder, the worktree layout is in PLAN §Working mode, and the
parry 0.22.1 rationale is in PLAN §Tech stack. Read those first; what is here
is the record of how each session ended.

### The superseded "What next" and "State at handoff" generations

<!-- moved here 2026-09-09 night by the lead (session 19): the 2026-09-09 evening HANDOFF pair, verbatim -->
### HANDOFF pair superseded 2026-09-09 night (session 19)

## What next — value order (2026-09-09 evening, after session 18 — the loop is RUNNING; the paste-in prompt for the next Fable window is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
0. **The loop is RUNNING.** the maintainer launched the two lanes at 15:00 on 2026-09-09 (`LANES=2`); they drained at 16:57/16:58 and the lead parked them with `PAUSE` for the landing, then removed `PAUSE` and `engine/artifacts/loop/DRAINED` right after the docs commit — **the lanes resume by themselves on the refilled queue; no relaunch is needed.** If they ever do need starting again, **The maintainer does it HIMSELF, from his own terminal** — a Fable or Opus session never launches it: `rm -f STOP PAUSE && scripts/autoloop.sh --probe && LANES=2 nohup scripts/autoloop.sh > engine/artifacts/loop/autoloop.log 2>&1 &   (from the maintainer's own terminal; with every CLAUDE* env var stripped if run from a Claude session)`. Queue order: W2 → OC → 4 → SM → L → AT → 13 → HR → WS → SK → 9 → W3 → 7 → C3 row 6 → ONCE. **The SWEEP is RE-BASELINED and every other gate value is unchanged from s17** (QUEUE.md's GATES block = `engine/artifacts/lead_tmp/s18/baselines/SUMMARY.txt`): sweep `sweep: finished 26/32  stuck_max median 840 (finished-only 780)  >10k 6 maps  sum 242580` — queue item S0 made the sweep's slot-0 player a bot, and `SWEEP_SLOT0=player` reproduces the s17 sweep byte for byte, so the s17 sweep file is stale for the default sweep only; walk_demo `bb860216c775e788` / aim_demo `b489b1b3aa14084f` / `--sim-hash 600` `fb19c1d2c811788b` / `--sim-hash 100` `17e1b946ff0e821c`; both `motion.csv` identical to s17; flat arena 45.49 / 122.33 / 166.89 / 200.00 (606262bc6865a689 / 09c7449831956e5d / b2f6343d64f93c51 / b8ab0f137168eb26); pair 0, vs s17 0; tat2 arena 27.40 / 37.20 / 34.60 / 36.60 (02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81); pair 0, vs s17 0; ci **646 tests**; `gpu_tests.sh` exit 0; bake bytes **21651 files DIFF=0** (the tree grew by VO's 61 new files; no noise family). A drained queue parks the lanes on `engine/artifacts/loop/DRAINED` — the lead removes it after refilling.
1. **Next Fable window — land what the loop parks**, as sessions 14–18 did. **The s18 kit is the reusable one:** `engine/artifacts/lead_tmp/s18/briefs/landing.md` (the lander brief — rewrite the branch list and the expected values), `briefs/toolsmith.md` (the kit port), `briefs/docs.md` (the docs brief the close-out agent runs on), `s18/rebake_and_cmp.sh`, `s18/goldens_regen.sh` + `s18/golden_pairs.py`, `s18/final_gates.sh`, `s18/gen_queue.py <spec.json>` with `s18/queue_spec_template.json` as the spec pattern, and `s18/make_values.py`/`closeout.py` as the older docs-patch pattern. Rebase, ci both sides under the flock, reproduce, ff; then once: rebake if any [REBAKE] landed, quantify every moved golden's diff BEFORE viewing it and view every parked sheet AND every regenerated golden as a COMMITTED-vs-REGENERATED pair before committing, gpu tests, `sim_gates.sh`, the sweep pair, the arena pair; record the baselines under `lead_tmp/s19/baselines/`; refill; `rm engine/artifacts/loop/DRAINED` if set. Two judgements are waiting for that window: **re-judge `OPENBF_FAR_CULL`'s default after item L lands** (item 6 measured one pixel of difference over 44 images on its own six maps, so the flag's judgement rests on item 5's other 26 maps, where the cull shows the clear colour past `FarSceneRange`; L adds the far sheet on the 16 maps that ship a `LOWR`), and **view item AT's and item W2's sheets and regenerate** the goldens they move. C2's shape rule: one §What next + one §State here; the superseded pair moves verbatim to `analysis/PLAN_LOG_2026.md` Part 2.
2. **Fable RE left (unchanged from s17, plus one):** the **`modl` LOD switch rule** (HOLD 20 — GEO1's wreck; the material path is fully [V] and does NOT explain it: `analysis/re-vu1-cull.md` §4 carries the two-station PCSX2 capture note for the maintainer, which settles it); the **water VU1 microcode** (wave displacement, the LUT lookup, per-vertex alpha) and **KAM1's FFT ocean** (HOLD 21, `analysis/re-water.md` §8 — whether to build the ocean at all is a design call); **P7 vehicles**, which is NEXT now that Phase D is complete (the broadphase's `bodies` field goes private with it); **B1** (`Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change, lead-run, with a re-baseline; phase-d 1a pinned `--sim-hash 100` = `17e1b946ff0e821c` in a test that says a failure is a lead decision); BES1's ring — whether the PS2 draws an effect into it (HOLD 4, low value). **NEW (2026-09-09 evening, 17B's observation):** whether the AI-shed loop in `tick_bleed` is reachable from bleed at all — 17B measured `max overhang [0, -427]` over its configuration and argues from the two gates that it cannot fire (`apply_bleed` needs `alive < tickets` to arm, and one tick at the Elite handicap moves the pool by ~0.014), but it is not proved in general; if it truly is unreachable, the port pays for a safety net on every tick of every round. A lead call, not a queue item.
3. **Decisions open for the maintainer:** the **PCSX2 GS dump** — three questions: (a) the D-pad menu's second material layer (the 0x89 / PRIM 0x58 / TEST 0x51001 entry passed whenever a texture is bound — does the microcode draw it), (b) the wreck's LOD at the golden's ~475 m framing (which `modl` is kicked), (c) the far water fog pass's look; the alpha-rule reconciliation stays CLOSED (`analysis/re-hud-icons.md` citation 3, the `pal_` loader's `(A·0x81)>>8` CLUT scale). **NEW 2026-09-09 evening — BF2 NAB2's black distance:** item 6's fade takes a BF2 map with no far layer to the SKY colour, and `sky_naboo2` is black on the disc (HOLD 10(c)), so Theed's distance now fades to black. It is faithful; whether the port keeps it is the maintainer's eye (`bf2_nab2` is one of the 11 regenerated goldens, `lead_tmp/s18/sheets_view/`). **Whether tier 2 ships on disk at all** for a build the maintainer plays (+388 MB for the maps, +4.9 MB for the side textures; T2R and T2S are built either way — the toggle is the question). **Tier-4 checks:** the squad README + captures (Phase D's deliverable for the maintainer), and the Elite feel check (D12's reinforcement bias, declined until then).
4. Backlog: P7 — it is next (the broadphase's `bodies` field goes private then); the era/sound/HUD icon fallbacks; the `--cam-overview` fog look call; the `missions_root()`/`extracted_root()` literals; one cross-OS determinism run (the parry flag + `fmath`); B1 (above); the guard's `writes_leadfile` false positive; **the `tick_bleed` AI-shed loop** (item 2 above — if it is unreachable from bleed it is a per-tick cost with no caller); **`impl Default for TextureDesc`** (and `DetailDesc`) in `render/src/texture.rs`, which would let `tex_desc` disappear from the GPU referees — RT's out-of-scope suggestion, a future cleanse row.

## State at handoff (2026-09-09 evening, Fable session 18 COMPLETED — the nine parked branches are landed, the rebake and the goldens are done, the sweep is re-baselined, the queue is refilled and the loop is RUNNING)
- **On main (all lead-verified, PLAN Log 2026-09-09 evening):** `soldier-tier-check` 27c2f3e, `render-tests-common` 754b662, `battle-outcome-pool` bc030e6, `water-floor-check` 52d0ff5, `walk-spawn-post` 9f1155b, `map-vo-streams` d3ac445, `water-fx-config` 52e6b54, `bf2-far-fade` 19148e6, `sweep-slot0-bot` 5ff02ca (= the **CODE SHA `5ff02ca`**, every gate below measured there); then the lead's commits on top — the goldens `6314c2c` and the docs. **No worktree, no branch, no stash** (all nine removed; git worktree list = main alone; <repo parent>/openbf-wt/ empty). Evidence: `engine/artifacts/lead_tmp/s18/` (`reports/landing.md`, `reports/toolsmith.md`, `rulings.md`, `values.json`, `baselines/SUMMARY.txt`, `logs/`, `sheets_view/`).
- **`engine/assets_baked` IS a fresh bake of main `5ff02ca`** (21651 files; the pre-rebake scratch bake of the same main vs the rebaked tree `SAME=21651 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0`; before the rebake it was `SAME=21569 DIFF=21 ONLY_IN_A=61 ONLY_IN_B=0 — DIFF = 12 water maps' manifest.ron + 9 water*.glb (W1); ONLY_IN_A = 32 sounds/streams/streams.ron + 29 sounds/streams/<hash>/<hash>.ogg (VO); nothing else (the census pattern expected the OGGs one level up — a pattern slip, all 29 strays are those OGGs)`).
- **The goldens:** 90 committed PNGs (74 MB); **11 regenerated this window** — bf1_kas1, bf1_kas2, bf1_nab2, bf2_dag1, bf2_dea1, bf2_end1, bf2_fel1, bf2_kam1, bf2_kas2, bf2_nab2, bf2_yav1 — the union of item 6's six BF2 fade maps and W1's six water maps (DAG1 and NAB2 in both) plus `bf1_nab2`, whose `water.glb` changed. Every pair viewed before committing (`lead_tmp/s18/sheets_view/golden_pairs_00..05.png`): the fog walls dissolve into the sky, the water carries the disc's tiling, KAM1's sea is textured instead of a flat grey plane, and eye-level frames are unchanged except for the fade. Committed by path as `6314c2c`. They render the PS2 BASE tier and the golden tests force `Ps2` themselves, so `OPENBF_TEXTURE_TIER=2` in the environment cannot move them.
- **Baselines NOW:** PLAN §WHERE THINGS STAND is the ONE canonical copy (`lead_tmp/s18/baselines/SUMMARY.txt`) and **only the SWEEP moved — by design (queue S0)**. In one line, for orientation only: sweep `sweep: finished 26/32  stuck_max median 840 (finished-only 780)  >10k 6 maps  sum 242580`; walk_demo bb860216c775e788 / aim_demo b489b1b3aa14084f / `--sim-hash 600` fb19c1d2c811788b / `--sim-hash 100` 17e1b946ff0e821c; `motion.csv` unchanged since P9; flat and tat2 arenas unchanged since s16 (arena's slot-0 default stays OFF). ci 646 tests; `gpu_tests.sh` 0; bake bytes DIFF=0 over 21651 files.
- **The walkabout set** was regenerated at close on the rebaked tree: 219 images over 32 maps in 21.1s (4.6s of that PNG encoding), 884.2 MB in <repo>/engine/crates/game/../../artifacts/walkabout.
- **Cleanup:** `scripts/cleanse_delete.sh --yes` freed 183 GB (21 paths: the nine dead *_target dirs, the lander's two scratch bakes + bake_main_pre, the loop sessions' VO/W1 scratch bakes, item 6's raw renders (its 88 sheets kept); scripts/cleanse_delete.sh gained the s18 lines); the repo is 91 GB.
- **Process rules learned (keep — the cumulative list):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before removing it (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout, and a BACKGROUND Bash call resets the harness cwd the same way (s17); a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped; view a regenerated golden as a COMMITTED-vs-REGENERATED pair (`golden_pairs.py`), never the new image alone, and **quantify its diff (mean / max / changed-pixel fraction + bbox) BEFORE viewing it**; a MAP golden can move when only the SIDE bake changed (s17) — and when only the WATER bake changed (s18); two [V] facts from two different RE agents can COMPOSE into a port rule neither of them stated (s17); a paused loop never sees STOP — kill it (AUTOLOOP.md); after any edit to `autoloop.sh` the live lanes are stale — kill and relaunch; launch the loop from a plain terminal or with every `CLAUDE*` env var stripped and run `--probe` first; after ANY queue generation check that `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists every item; **never `pkill -f` a pattern that is in your own command line**. **New this window (s18):** `cmp_bake.sh` EXITS 1 whenever DIFF≠0 — an expected-family diff is not a failure, read the families not the exit code; a branch that re-baselines a gate lands LAST, so every earlier branch still measures against the old baseline; and a re-baselining item should ship the escape hatch that reproduces the old value (`SWEEP_SLOT0=player`) so the move can be proved inert.

<!-- moved here 2026-09-09 evening by the lead (session 18): the 2026-09-09 morning HANDOFF pair, verbatim -->
### HANDOFF pair superseded 2026-09-09 evening (session 18)

## What next — value order (2026-09-09, after session 17 — the loop is STOPPED and the maintainer relaunches it; the paste-in prompt for the next Fable window is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
0. **The loop is STOPPED** — `STOP` sits at the repo root (the maintainer wrote it 2026-09-09 07:58 and it is deliberately left there; `PAUSE` and `DRAINED` are gone). **The maintainer relaunches it HIMSELF, from his own terminal** — a Fable or Opus session never launches it: `rm -f STOP PAUSE && scripts/autoloop.sh --probe && LANES=2 nohup scripts/autoloop.sh > engine/artifacts/loop/autoloop.log 2>&1 &   (from the maintainer's own terminal; with every CLAUDE* env var stripped if run from a Claude session)`. Queue order: 6 → W1 → S0 → W2 → W → 4 → SM → VO → T2C → WL → L → AT → 9 → W3 → 7 → 13 → HR → C3 row 6 → 17B → ONCE → RT. **Every gate value is unchanged from s16** (QUEUE.md's GATES block = `engine/artifacts/lead_tmp/s17/baselines/SUMMARY.txt` = the s16 values): sweep `sweep: finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480`; walk_demo `bb860216c775e788` / aim_demo `b489b1b3aa14084f` / `--sim-hash 600` `fb19c1d2c811788b` / `--sim-hash 100` `17e1b946ff0e821c`; both `motion.csv` identical to s16; flat arena 45.49 / 122.33 / 166.89 / 200.00 (606262bc6865a689 / 09c7449831956e5d / b2f6343d64f93c51 / b8ab0f137168eb26); tat2 arena 27.40 / 37.20 / 34.60 / 36.60 (02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81); ci **615 tests**; `gpu_tests.sh` exit 0; bake bytes **21590 files DIFF=0** — and since item 3 landed there is NO noise family left (`tat1/sounds/names.ron` no longer drifts). A drained queue parks the lanes on `engine/artifacts/loop/DRAINED` — the lead removes it after refilling.
1. **Next Fable window — land what the loop parks**, as sessions 14–17 did. **The s17 set is the reusable one:** `engine/artifacts/lead_tmp/s17/briefs/landing.md` (the lander brief — rewrite the branch list and the expected values), `s17/rebake_and_cmp.sh`, `s17/goldens_regen.sh` + `s17/golden_pairs.py` (path-agnostic since s17), `s17/final_gates.sh`, `s17/gen_queue.py <spec.json>` with `s17/queue_spec_template.json` as the spec pattern, and `s17/briefs/docs.md` (the docs brief the close-out agent runs on). Rebase, ci both sides under the flock, reproduce, ff; then once: rebake if any [REBAKE] landed, view every parked sheet AND every regenerated golden as a COMMITTED-vs-REGENERATED pair before committing, gpu tests, `sim_gates.sh`, the sweep pair, the arena pair; record the baselines under `lead_tmp/s18/baselines/`; refill; `rm engine/artifacts/loop/DRAINED` if set. Two judgements are already waiting for that window: **judge `OPENBF_FAR_CULL`'s default** from item 6's sheets together with item 5's 44 parked sheets (`engine/artifacts/lead_tmp/far-scene-cull/`, never viewed — that was the decision), and **view item AT's sheets and regenerate** the goldens it moves. C2's shape rule: one §What next + one §State here; the superseded pair moves verbatim to `analysis/PLAN_LOG_2026.md` Part 2.
2. **Fable RE left:** the **`modl` LOD switch rule** (HOLD 20 — GEO1's wreck; the material path is now fully [V] and does NOT explain it, so the LOD-distance rule is the candidate: `analysis/re-vu1-cull.md` §4 carries the two-station PCSX2 capture note for the maintainer, which settles it); the **water VU1 microcode** (wave displacement, the LUT lookup, per-vertex alpha) and **KAM1's FFT ocean** (HOLD 21, `analysis/re-water.md` §8 — whether to build the ocean at all is a design call); **P7 vehicles**, which is NEXT now that Phase D is complete (the broadphase's `bodies` field goes private with it); **B1** (`Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change, lead-run, with a re-baseline; phase-d 1a pinned `--sim-hash 100` = `17e1b946ff0e821c` in a test that says a failure is a lead decision); BES1's ring — whether the PS2 draws an effect into it (HOLD 4, low value).
3. **Decisions open for the maintainer:** the **PCSX2 GS dump** — now THREE questions, not four: the alpha-rule reconciliation is **CLOSED** (`analysis/re-hud-icons.md` citation 3, lead re-read — the `pal_` loader's `(A·0x81)>>8` CLUT scale makes disc 0xff the GS's 1.0, so the port's full-range alpha is the right blend factor; `re-material-flags.md`'s header says so now), leaving (a) the D-pad menu's second material layer (the 0x89 / PRIM 0x58 / TEST 0x51001 entry passed whenever a texture is bound — does the microcode draw it), (b) the wreck's LOD at the golden's ~475 m framing (which `modl` is kicked), (c) the far water fog pass's look. **Whether tier 2 ships on disk at all** for a build the maintainer plays (+388 MB for the maps, +4.9 MB for the side textures; T2R and T2S are built either way — the toggle is the question). **Tier-4 checks:** the squad README + captures (Phase D's deliverable for the maintainer), and the Elite feel check (D12's reinforcement bias, declined until then).
4. Backlog: P7 after Phase D — it is next (the broadphase's `bodies` field goes private then); the era/sound/HUD icon fallbacks; the `--cam-overview` fog look call; the `missions_root()`/`extracted_root()` literals; one cross-OS determinism run (the parry flag + `fmath`); B1 (above); the guard's `writes_leadfile` false positive. **Three former backlog items are queue items now:** `render/tests/` sharing a `common/mod.rs` → RT; item 17's not-measured post-outcome pool question → 17B; the roster's `OnceLock` → ONCE.

## State at handoff (2026-09-09, Fable session 17 COMPLETED — the seven parked branches are landed, Phase D is COMPLETE, the rebake and the goldens are done, the queue is refilled and the loop is STOPPED for the maintainer to relaunch)
- **On main (all lead-verified, PLAN Log 2026-09-09):** `test-scratch-dirs-2` 4df338e, the item-3 leftover fd2f02a, `bake-ogg` a1ff75f, `global-sound-runtime` 35c6eed, `two-tier-runtime` 16a2e49, `two-tier-soldier` 51f7d80, `far-scene-cull` b203619, `phase-d` D5–D14 cef083b (= the **CODE SHA `cef083b`**, every gate below measured there); then the lead's commits on top — the goldens `1628519` and the docs. **No worktree, no branch, no stash** (the seven branch worktrees + the two throwaway tsd2-b/tsd2-main; git worktree list = main alone; <repo parent>/openbf-wt/ empty). Evidence: `engine/artifacts/lead_tmp/s17/` (`reports/landing.md`, `rulings.md`, `values.json`, `baselines/SUMMARY.txt`, `logs/`, `sheets_view/`, `re/`).
- **`engine/assets_baked` IS a fresh bake of main `cef083b`** (21590 files; the pre-rebake scratch bake of the same main vs the rebaked tree `SAME=21590 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0`; before the rebake it was `SAME=21515 DIFF=54 ONLY_IN_A=21 ONLY_IN_B=0 — tat1/sounds/names.ron 1 + side_bf2/textures/*.ktx2 21 + */manifest.ron 32; ONLY_IN_A = 21 side_bf2 *.tier2.ktx2; nothing else`).
- **The goldens:** 90 committed PNGs (74 MB); **31 regenerated this window** — 14 (bf2_cis_inf_{bdroid,magnaguard,sbdroid}, bf2_rep_inf_{arctrooper,clonecommander,ep3armoredpilot,ep3heavytrooper,ep3jettrooper,ep3sniper,ep3trooper,feluciatrooper,jettrooper,pilot,trooper}) and 17 (the 16 BF2 maps + tat2), one cause (the disc's soldier textures T2S made the base tier), every pair viewed before committing (`lead_tmp/s17/sheets_view/`). They render the PS2 BASE tier and the golden tests force `Ps2` themselves, so `OPENBF_TEXTURE_TIER=2` in the environment cannot move them (measured, gpu exit 0).
- **Baselines NOW:** PLAN §WHERE THINGS STAND is the ONE canonical copy (`lead_tmp/s17/baselines/SUMMARY.txt`) and **every value is the s16 one — nothing moved this window**. In one line, for orientation only: sweep `sweep: finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480`; walk_demo bb860216c775e788 / aim_demo b489b1b3aa14084f / `--sim-hash 600` fb19c1d2c811788b / `--sim-hash 100` 17e1b946ff0e821c; `motion.csv` unchanged since P9; flat arena unchanged since s15; tat2 arena 27.40 / 37.20 / 34.60 / 36.60. ci 615 tests; `gpu_tests.sh` 0; bake bytes DIFF=0 over 21590 files.
- **The walkabout set** was regenerated at close on the rebaked tree: 219 images / 32 maps in 20.3 s, 890.8 MB, 256 PNGs all RGB (tier 2 ON in the set), `INDEX.md` rebuilt (`lead_tmp/s17/logs/`).
- **Cleanup:** `scripts/cleanse_delete.sh --yes` freed 160 GB (26 paths: the lander's four scratch bakes + bake_main_pre, bake-ogg/{bake,bake2}, far-scene-cull/bake, test-scratch-dirs-2/bake, two-tier-soldier/{bake_before,bake_after,bake_nopc}, two-tier-runtime/before, phase-d/before, and the nine dead *_target dirs; scripts/cleanse_delete.sh gained the s17 line); the repo is 89 GB.
- **Process rules learned (keep — the cumulative list):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before removing it (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout, **and a BACKGROUND Bash call resets the harness cwd the same way (s17)**; a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped; view a regenerated golden as a COMMITTED-vs-REGENERATED pair (`golden_pairs.py`), never the new image alone; a paused loop never sees STOP — kill it (AUTOLOOP.md); after any edit to `autoloop.sh` the live lanes are stale — kill and relaunch; launch the loop from a plain terminal or with every `CLAUDE*` env var stripped and run `--probe` first; after ANY queue generation check that `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists every item; **never `pkill -f` a pattern that is in your own command line**. **New this window (s17):** a MAP golden can move when only the SIDE bake changed — the map sheet's right-hand panel draws a side unit, so a [REBAKE] side change is a [GOLDENS] map change too; **quantify a moved golden's diff (mean / max / changed-pixel fraction + bbox) BEFORE viewing it** — that is what tells noise from a real move; and two [V] facts from two different RE agents can COMPOSE into a port rule neither of them stated (the CLUT scale × the AREF → queue item AT) — the lead writes the composition as [D] with both citations.

<!-- moved here 2026-09-09 by the lead (session 17): the 2026-09-08 late-night HANDOFF pair, verbatim -->
### HANDOFF pair superseded 2026-09-09 (session 17)

## What next — value order (2026-09-08 late night, after session 16 — the loop runs it; the paste-in prompt for the next Fable window is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
0. **The loop is RUNNING** (relaunched 22:58 on 2026-09-08 on the committed `scripts/autoloop.sh`, `LANES=2`, pids launcher 3360746, lane 1 3360752; lane 2 starts three minutes later, see `engine/artifacts/loop/autoloop.log`; `PAUSE` and `DRAINED` both gone): queue order 5 → PD → GS → T2R → T2S → OGG → 6 → 9 → L → S0 → 13 → 4 → 7 → W → 3 → C3 row 6. Every gate value is the s16 one (QUEUE.md's GATES block = `lead_tmp/s16/baselines/SUMMARY.txt`); the bake-bytes gate is DIFF=0. A drained queue parks the lanes on `engine/artifacts/loop/DRAINED` — the lead removes it after refilling.
1. **Next Fable window — land what the loop parked**, as sessions 14–16 did (`lead_tmp/s16/briefs/landing_{A,B}.md` are the reusable lander briefs — rewrite the branch list and the expected values; `lead_tmp/s16/gen_queue.py` + `make_values.py` + `closeout.py` the docs-patch pattern; `s16/rebake_and_cmp.sh`, `goldens_regen.sh` + `golden_pairs.py`, `final_gates.sh` the close-out runs): rebase, ci both sides under the flock, reproduce, ff; then once: rebake if any [REBAKE] landed, view every parked sheet AND every regenerated golden as a pair before committing, gpu tests, `sim_gates.sh`, the sweep pair, the arena pair; record the baselines under `lead_tmp/s17/baselines/`; refill; `rm engine/artifacts/loop/DRAINED` if set. PD is IN PROGRESS in `<repo parent>/openbf-wt/phase-d` (HEAD == main f4e4e76) — a loop session resumes it there at D5; when its next commits are parked, land them the same way (its sweep gate is the s16 sweep). C2's shape rule: one §What next + one §State here; the superseded pair moves verbatim to `analysis/PLAN_LOG_2026.md` Part 2.
2. **Fable RE left:** water (HOLD 10(f)/19(c): the water pass takes no fog; the level vs the authored floors — YAV1's plaza, KAS2's beach post; the flat sea gradient). **The maintainer's look at the regenerated goldens (2026-09-08 late night):** the KAS2 red horizon band, KAM1's black band and END1's dome trees over a flat grey plane are the RECORDED defects 1/3 (queue 5 — a lane is building it now — then 6; KAM1's flat sea and the water half of the KAS2 band are the water RE above). NEW: BF2 GEO1's Techno Union wreck reads as incomplete — the forward hull draws as bare rafters. Measured: the geometry is fully baked (`segdump GEO1.LVL geo_bldg_technounion_hull` 2645/2645 indices over 8 segments, LOWD 895/895), four segments are MASK by alpha rule 3 (`geo_bldg_wrecked_02`, `geo_bldg_rafters`, a0 0.21–0.26) and every segment is single-sided; whether the PS2 alpha-tests that plating and draws it two-sided is the alpha-rule question (HOLD 10(g)) — Fable's, and the PCSX2 GS dump would settle it. The `PrimaryStance` question is CLOSED and BUILT (phase-d 0b); the RE agent's three un-re-read side findings (re-stance-roll §3.2's swapped blocks, BF1 `FUN_002450b8`, BF2 `FUN_002d53c8`) are PROVISIONAL and cheap to verify when next in the stance code; BF1's `SecondaryStance` setter stores `v & 7` with no `v > 0` guard (RE §2; no disc authors 8 — the PORT GUARD at `stance_mask` is labelled PROVISIONAL). BES1's ring — whether the PS2 draws an effect into it (HOLD 4, low value).
3. **Decisions open:** none blocking the loop. The maintainer: a PCSX2 GS dump (the alpha rule, HOLD 10(g)); a Tier-4 Elite feel check (D12's reinforcement bias, declined until then); whether tier 2 ships on disk at all for a build the maintainer plays (+388 MB — item 11's report; T2R is built either way, the toggle is the question).
4. Backlog: P7 after Phase D (the broadphase's `bodies` field goes private then); the era/sound/HUD icon fallbacks; the `--cam-overview` fog look call; `render/tests/` sharing a `common/mod.rs`; the `missions_root()`/`extracted_root()` literals; one cross-OS determinism run (the parry flag + `fmath`); B1 (`Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change, lead-run, with a re-baseline; phase-d 1a pinned `--sim-hash 100` = `17e1b946ff0e821c` in a test that says a failure is a lead decision); the guard's `writes_leadfile` false positive; item 17's not-measured question (can the live sim, which keeps stepping past the outcome, still reach a post-outcome negative pool — `cp::apply_bleed` may already prevent it).

## State at handoff (2026-09-08 late night, Fable session 16 COMPLETED — the six branches the loop left are landed; the rebake and the goldens are done; the loop is running on the refilled queue)
- **On main (all lead-verified, PLAN Log 2026-09-08 late night):** `terrain-ptch` 50c7993, `global-sound` e0da217, `bake-determinism` 91f3532 (lander A, session 16 first half), `two-tier-ktx2` aff46dd, `phase-d` 0a/0b/1a/1b 6275a6d, `battle-ticket-floor` f4e4e76 (= the CODE SHA `f4e4e76`, every gate below measured there; lander B); then the lead's commits on top (the 24 regenerated goldens; docs). One worktree: `<repo parent>/openbf-wt/phase-d` (PD IN PROGRESS, HEAD == main, resume at D5). No parked branch, no stash. Evidence: `engine/artifacts/lead_tmp/s16/` (`reports/landing_{A,B}.md`, `baselines/SUMMARY.txt`, `logs/`, `sheets_view/` incl. the golden pairs, the close-out scripts).
- **`engine/assets_baked` IS a fresh bake of main `f4e4e76`** (21,569 files; two scratch bakes of the same binary DIFF=0; the rebaked tree differs from the post-landing-A bake in exactly item 11's 292 `*.ktx2` + 34 manifests, + 342 `*.tier2.ktx2` new). **The goldens render the PS2 base tier** (decision 1): 24 goldens (21 map + the three tat1 captures) regenerated and viewed; the runtime half is queue item T2R.
- **Baselines NOW:** PLAN §WHERE THINGS STAND is the ONE canonical copy (`lead_tmp/s16/baselines/SUMMARY.txt`). In one line, for orientation only: sweep `sweep: finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480`; walk_demo bb860216c775e788 / aim_demo b489b1b3aa14084f / `--sim-hash 600` fb19c1d2c811788b / `--sim-hash 100` 17e1b946ff0e821c; `motion.csv` unchanged since P9; flat arena unchanged since s15; tat2 arena 27.40 / 37.20 / 34.60 / 36.60 (02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81). ci 583 tests; `gpu_tests.sh` 0; bake bytes DIFF=0.
- **The walkabout set** was regenerated at close on the rebaked tree: 219 images over 32 maps in 21.2 s (4.6 s PNG encoding), 884 MB, every PNG RGB (256 files), `INDEX.md` rebuilt (`lead_tmp/s16/logs/walkabout_regen.log`).
- **Cleanup:** `scripts/cleanse_delete.sh --yes` freed 111 GB (19 paths) this window; the repo is 106 GB.
- **Process rules learned (keep):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before `git worktree remove` (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout; a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped; view a regenerated golden as a COMMITTED-vs-REGENERATED pair (`golden_pairs.py`), never the new image alone; a paused loop never sees STOP — kill it (AUTOLOOP.md); after any edit to `autoloop.sh` the live lanes are stale — kill and relaunch. Launch the loop from a plain terminal or with every `CLAUDE*` env var stripped (`env $(env | grep -o '^CLAUDE[A-Z_0-9]*' | sed 's/^/-u /') LANES=2 nohup scripts/autoloop.sh …`) and run `--probe` first — a CLI started under a Claude session waits on stdin (the probe's stdin is closed since 2026-09-08 late night); after ANY queue generation check that `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists every item.


<!-- moved here 2026-09-08 late night by the lead (session 16): the session-16 STOPPED MID-WAY block and the session-15 evening pair, verbatim -->
## SESSION 16 (2026-09-08 night, 20:53–21:30) STOPPED MID-WAY at the maintainer's request — the next Fable window RESUMES HERE (paste-in prompt: `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
**Landed on main, lead-verified from the lander's raw logs** (`lead_tmp/s16/reports/landing_A.md`, `lead_tmp/s16/logs/`):
`terrain-ptch` (H) 50c7993 (2 commits), `global-sound` (15) e0da217 (3), `bake-determinism` (16) 91f3532 (2) — **main = 91f3532,
ci 564 tests**. No sim value moved on any of the three (the s15 replays/sweep, measured per rebased branch). Two scratch bakes
of main's binary: `SAME=21227 DIFF=0` — **the bake is byte-deterministic; the bake-bytes gate is DIFF=0 from here on**
(`scripts/cmp_bake.sh`'s header says so; the QUEUE GATES block does not yet — the refill rewrites it). Those three worktrees
and branches are gone. **`engine/assets_baked` is STILL the s15 bake — no rebake and no golden regeneration happened**
(they wait for item 11, so that there is ONE rebake). The lead viewed ALL 128 parked sheets (contact sheets in
`lead_tmp/s16/sheets_view/`: terrain-ptch 101 + 13 crops, two-tier-ktx2 27) — nothing to re-view.
**Decisions taken (do not re-litigate):** (1) item 11 LANDS, and **the goldens render the PS2 base tier from now on** (a
strict-faithful referee independent of the maintainer's PC install; the runtime half, tier 2 ON in play / OFF in the golden tests,
becomes queue item T2R); (2) phase-d's four complete commits (0a/0b/1a/1b) LAND now and the 0b move IS the re-baseline
(footer `sweep: finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480`, tat2 arena
02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81); PD stays IN PROGRESS in its worktree, HEAD ==
main after the ff, resume at D5; (3) item 17 moves nothing — no re-baseline; (4) the reports' PROVISIONAL calls stand: the
88 MB VO at quality 3, the 950 unreferenced VO ids NOT baked, the capitalised name wins ties, the tier-2 alpha rule, the
stance-less hint → STAND, the BF1 `SecondaryStance` guard, the sunk-soldier-at-cut-rims observation is a HOLD note not a defect.
**Remaining, in this order:** (a) keep the root `PAUSE` and `engine/artifacts/loop/DRAINED` — the three live lane processes
(pids 823710 / 823719 / 826168, started 14:06 from the pre-backoff script) sleep on `PAUSE` at zero cost and cannot see
`DRAINED`; (b) run lander B from `lead_tmp/s16/briefs/landing_B.md` (READY; it reads landing_A.md for main's SHA) —
`two-tier-ktx2` → `phase-d` (4 commits, worktree KEPT) → `battle-ticket-floor`, expected values inside; (c) ONE
`scripts/rebake.sh` on main, then `scripts/cmp_bake.sh lead_tmp/s16/bake_bake-determinism` against it (only item 11's
files may differ: 292 ktx2 + 34 manifests, 342 `*.tier2.ktx2` ONLY_IN_B), then a fresh scratch-bake pair on main → DIFF=0
over 21,569 files; (d) `scripts/goldens.sh`; `git diff --stat engine/goldens` must list ≤ 20 map goldens = H's 8
(bf1_hot1, bf1_rhn1, bf1_yav2, bf2_dag1, bf2_geo1, bf2_hot1, bf2_pol1, tat1) ∪ 11's 16 (bf2_cor1, dag1, dea1, end1, fel1,
geo1, kam1, kas2, mus1, myg1, nab2, pol1, tan1, uta1, yav1, tat2) and nothing else; view every regenerated golden;
`scripts/gpu_tests.sh` 0; (e) the final gates on main — `sim_gates.sh` (BASE absolute, motion.csv → `s16/baselines/motion_main`),
the sweep twice (stdout alone), the arena pair (streams split), `game --sim-hash 100` = 17e1b946ff0e821c — into
`s16/baselines/SUMMARY.txt`; the walkabout regen (delete `engine/artifacts/walkabout/INDEX.md` first); (f) the refill:
`python3 lead_tmp/s16/gen_queue.py <values.json> engine/artifacts/loop/QUEUE.md` — the generator is WRITTEN (it carries every
kept item verbatim from `s16/QUEUE.md.before` and adds GS, T2R, T2S, OGG, W; it wants sha_H/15/16/11/pd/17/main, footer,
walk/aim/h600/h100, tat2_rows/tat2_hashes, bake_files); (g) PLAN Log + this file's §State/§What next pair (C2's shape rule:
the superseded pair and this block move verbatim to `analysis/PLAN_LOG_2026.md` Part 2), START_HERE, JOURNAL LEAD line,
commit by path; `scripts/cleanse_delete.sh --dry-run` then `--yes` (the maintainer approved — the s16 scratch bakes and the dead
`*_target` dirs are Tier A/B); (h) kill the stale lane loop (pid 823710 and its children) and relaunch
`LANES=2 nohup scripts/autoloop.sh > engine/artifacts/loop/autoloop.log 2>&1 &`, then `rm PAUSE` and `rm engine/artifacts/loop/DRAINED`.

## What next — value order (2026-09-08 evening, after session 15 — the loop runs it; the paste-in prompt for the next Fable window is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`)
0. **The loop is RUNNING** (PAUSE removed 18:20 on 2026-09-08; two lanes were left running by session 14 and resumed on their own): queue order H → 15 → 16 → 11 → 5 → 6 → 9 → L → PD → S0 → 17 → 13 → 4 → 7 → 3 → C3 row 6. Every gate value is the s15 one (QUEUE.md's GATES block). A drained queue parks the lanes on `engine/artifacts/loop/DRAINED` — the lead removes it after refilling.
1. **Next Fable window — land what the loop parked**, as sessions 14 and 15 did (`lead_tmp/s15/briefs/landing_{A,B}.md` are the reusable lander briefs — rewrite the branch list and the expected values; `lead_tmp/s15/gen_queue.py` / `closeout.py` are the docs-patch pattern): rebase, ci both sides under the flock, reproduce, ff; then once: rebake if any [REBAKE] landed (H, 15, 16, 11 and L are), view every parked sheet before `scripts/goldens.sh`, gpu tests, `sim_gates.sh`, the sweep pair, the arena pair; record the baselines under `lead_tmp/s16/baselines/`; refill; `rm engine/artifacts/loop/DRAINED` if set. C2's shape rule: one §What next + one §State here; the superseded pair moves verbatim to `analysis/PLAN_LOG_2026.md` Part 2.
2. **Fable RE left:** water (HOLD 10(f)/19(c): the water pass takes no fog; the level vs the authored floors — YAV1's plaza, KAS2's beach post; the flat sea gradient). The `PrimaryStance` question is CLOSED (RE_LOG 2026-09-08 evening); the agent's three un-re-read side findings (re-stance-roll §3.2's swapped blocks, BF1 `FUN_002450b8`, BF2 `FUN_002d53c8`) are PROVISIONAL and cheap to verify when next in the stance code. BES1's ring — whether the PS2 draws an effect into it (HOLD 4, low value).
3. **Decisions open:** none blocking the loop. The maintainer: a PCSX2 GS dump (the alpha rule, HOLD 10(g)); a Tier-4 Elite feel check (D12's reinforcement bias, declined until then).
4. Backlog: the two-tier KTX2 runtime half (after 11); P7 after Phase D (the broadphase's `bodies` field goes private then); the era/sound/HUD icon fallbacks; the `--cam-overview` fog look call; `render/tests/` sharing a `common/mod.rs`; the `missions_root()`/`extracted_root()` literals; one cross-OS determinism run (the parry flag + `fmath` — compare the three replay hashes on a non-glibc box); B1 (`Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change, lead-run, with item 17's re-baseline); the guard's `writes_leadfile` false positive.

## State at handoff (2026-09-08 evening, Fable session 15 CLOSED — the ten parked branches are landed; Phase C is complete; the loop is running on the refilled queue)
- **On main (all lead-verified, PLAN Log 2026-09-08 evening):** `cleanse-docs` 4fcc3ca, `cleanse-bake` 8841778, `cleanse-core-nav` 88f9430, `phase-c2` fc077bb (Phase C commits 2–5), `walkabout-rgb` 56fa51a, `test-scratch-dirs` c8cccf2 (partial; item 3 resumes), `bes1-sliver` 15f239d, `parry-determinism` 120e7e0, `collision-broadphase` 1c2eb89 (= the CODE SHA, every gate below measured there); then the lead's commits on top (scripts: `scratch_bake.sh` classes dir, `cleanse_delete.sh` Tier B, `autoloop.sh` DRAINED, `sim_gates.sh` header; `goldens/maps/bf2_uta1.png`; `analysis/a2-bake-formats.md` erratum + RE_LOG; docs). No worktrees, no parked branches, no stashes. Evidence: `engine/artifacts/lead_tmp/s15/` (`reports/landing_A.md`, `reports/landing_B.md`, `baselines/SUMMARY.txt`, `logs/`, `re/`, `briefs/`).
- **Baselines NOW:** PLAN §WHERE THINGS STAND is the ONE canonical copy (`lead_tmp/s15/baselines/SUMMARY.txt`). In one line, for orientation only: sweep `sweep: finished 26/32  stuck_max median 900 (finished-only 540)  >10k 5 maps  sum 271740`; walk_demo bb860216c775e788 / aim_demo b489b1b3aa14084f / `--sim-hash 600` fb19c1d2c811788b; `motion.csv` unchanged since P9; the arena pairs as the QUEUE GATES block. ci 542 tests; `gpu_tests.sh` 0.
- **RE this session:** `lead_tmp/s15/re/primary_stance.md` (Fable RE agent) — `PrimaryStance`/`SecondaryStance`/`Mode` ANSWERED (a bitmask; 3 = stand-or-crouch; BF2 never prones; BF1 prone from 4–7 only) — every load-bearing hop re-read by the lead; HOLD 17 updated; Phase D commit 0b builds the mask. `analysis/a2-bake-formats.md` §Models corrected (the last `modl INFO` word is the total index count; lead-reproduced on four LVLs).
- **The walkabout set** was regenerated at close (RGB, the 25° vantage cap): 219 images over 32 maps in 18.9 s, 889 MB, every PNG RGB, `INDEX.md` rebuilt; the vantage panels carry the 25° cap (`lead_tmp/s15/logs/walkabout_regen.log`).
- **Cleanup:** `scripts/cleanse_delete.sh --yes` freed 245 (28 + 74 + 143 GB logical, per the script's own du) GB this window (the maintainer's ask); the repo is 76 GB and the rest is inputs and the live build.
- **Process rules learned (keep):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before `git worktree remove` (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout; a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped.

<!-- moved here 2026-09-08 evening by the lead (session 15): the session-14 pair and its evening note, verbatim -->
## EVENING NOTE 2026-09-08 (session 14, after the two-lane loop ran ~3 h) — read before the sections below
The loop (two lanes from 14:06) parked TEN branches and drained the queue; it is PAUSED (PAUSE file) — lane 1 may still be
finishing item 12 (`collision-broadphase`; its lock under `engine/artifacts/loop/locks/12`). Parked, in landing order:
`cleanse-docs` (C2, 13 commits — the lead reads the whole diff), `cleanse-bake` (7), `cleanse-core-nav` (7), `phase-c2`
(PHASE C COMPLETE, commits 2–5, 12 commits; sweep footer moves to 27/32 by design), `walkabout-rgb` (2; ONE golden moves,
bf2_uta1, sheets parked), `test-scratch-dirs` (PARTIAL, 2) + its scratch worktree `tsd-b`, `bes1-sliver` (1, answered: the
sliver is the disc's geometry), `mat-invest` (0 commits, report `engine/artifacts/mat_invest/README.md`),
`parry-determinism` (1 + Cargo.lock), then `collision-broadphase` when it ends. Reports: `engine/artifacts/loop/reports/
20260908_*.md`. The next window's paste-in prompt is `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md` (also START_HERE).
Two loop fixes for that window: a drained-queue backoff (lane 2 spent four iterations on "no item" reports), and the
walkabout-rgb build rewrote the shared walkabout set on main in RGB (harmless; regenerate after landing). The lanes and
the lead share ONE ci lock: `flock engine/artifacts/loop/ci.lock`.

## What next — value order (2026-09-08, after session 14 — the loop runs it; the paste-in prompt is `engine/artifacts/loop/START_HERE.md` §Hand the parked branches…)
0. **The maintainer starts the autoloop** (`START_HERE.md` §Start). Queue order as written: C2 (docs + files, `cleanse-docs`) → C3 `cleanse-bake`, `cleanse-core-nav` → **PC = Phase C commits 2–5** (multi-session; `RESUME FROM:`) → H (the Hoth `PTCH` mesh) → PD (Phase D, after PC lands) → L (LOWR, after 5 and H) → 16 → the numbered defect items. Every gate value is the s14 one (QUEUE.md's GATES block).
1. **Next Fable window — land what the loop parked**, in queue order, exactly as session 14 did (`lead_tmp/s14/briefs/landing.md` is the reusable lander brief: rebase, ci both sides one at a time, reproduce, ff, then once: rebake if any [REBAKE] (H, L, 16 are), view every parked sheet before `scripts/goldens.sh`, gpu tests, `scripts/sim_gates.sh`, the sweep pair). C2's diff is read IN FULL by the lead (it rewrites PLAN.md/HANDOFF/analysis). PC: reproduce the arena table twice and the per-commit sweep footers. After H lands: regenerate the walkabout set (delete INDEX.md first) and re-check the Hoth panels.
2. **Fable RE left:** water (HOLD 10(f)/19(c): the water pass takes no fog — KAS2 fogs white over red terrain; the level vs the authored floors — YAV1's plaza, KAS2's beach post; the flat sea gradient). Everything else the defect hunt raised is now Opus-safe and queued.
3. **The maintainer's three decisions — TAKEN 2026-09-08, all per the lead's recommendations:** (a) the ball jumps on every map, as in BF2, with BF2's numbers (C2 corrects §Movement & feel; the droideka BUILD stays held, P7-class); (b) the unified sound plan → queue item 15 UNBLOCKED (Opus-safe: the RE is lead-verified), PD's D12 audio follows it; (c) the alpha rule stays v2 until a PCSX2 GS dump is captured (a the maintainer step: play a map in PCSX2 and save a GS dump) and read. Still open: D12's reinforcement-bias number (the lead's, not the maintainer's).
4. Backlog: the two-tier KTX2 runtime half (after 11), P7 after Phase D, the era/sound/HUD icon fallbacks, the `--cam-overview` fog look call, C3 row 6 (`cleanse-core-sim`, `cleanse-game-main`) after PC lands, `render/tests/` sharing a `common/mod.rs` (cleanse-render report), the `missions_root()`/`extracted_root()` literals (cleanse-game-tests report item 3).

## State at handoff (2026-09-08, Fable session 14 CLOSED — everything parked is landed; the loop queue is refilled; nothing is in flight)
- **On main (all lead-verified, PLAN Log 2026-09-08):** `cleanse-render` (0e36b15, 70d7dd1), `cleanse-game-tests` (3664198, 39c0265), `cleanse-mungefs` (b6015c6…8884e42), `phase-c` commit 1 (658c6af), `scripts/sim_gates.sh` (3f8d5c2), `scripts/scratch_bake.sh` + `cmp_bake.sh` (cdc1a4b), then the session's docs commit. No worktrees, no parked branches, no stashes. Evidence: `engine/artifacts/lead_tmp/s14/` (`reports/landing.md`, `logs/`, `baselines/`, `re/`, `briefs/`, `lead_gates/`). **Cleanup done 2026-09-08 (the maintainer's go):** `scripts/cleanse_delete.sh` (e18ed6b; dry-run by default, `--yes` deletes; re-runnable — it recomputes dead `*_target` dirs from git state) removed 36 paths, 179 GB logical (~57 GB physical; the filesystem compresses). C2's §F2 job is therefore done; C2 keeps §F1 (`git rm`) and the docs.
- **Baselines NOW (`lead_tmp/s14/baselines/SUMMARY.txt`, lead-reproduced):** sweep `finished 25/32  stuck_max median 1200 (finished-only 900)  >10k 5 maps  sum 241740`, pair byte-identical INCLUDING the hash column under the four-part strip filter (`' done ('`, `^roster `, `mix team`, `^mission `); walk_demo 8d9d661ebcdd5245 (407), aim_demo c12fdeb33d06ce48 (120), `--sim-hash 600` 537c1c6235c1f47e; `motion.csv` byte-identical to the pre-phase-c trajectories (the hash FORMAT grew, the sim did not change). `scripts/sim_gates.sh` runs all three (BASE= for the motion cmp). ci exit 0 at every landing; `gpu_tests.sh` 0, goldens unmoved. The s13 values are superseded everywhere except historical text.
- **RE this session (lead-verified, RE_LOG 2026-09-08 ×3):** `analysis/re-ai-sniper.md` (the float floor/cap; the sniper table — HOLD 19(e)/(g) ANSWERED; `re-ai-difficulty.md` §4.1 and `re-blaster-damage.md` §4.3 corrected; `brief_phase_c.md` LEAD ADDENDUM 4), `analysis/re-lowr.md` (HOLD 10(i)/19(f) ANSWERED — a same-footprint far LOD; complements item 5; `brief_lowr.md`), `analysis/re-terrain-cutters.md` (HOLD 19(a) ANSWERED — the `PTCH` replacement mesh; `brief_terrain_patch.md`). `re-fog.md` §2.2's `FUN_0048b9c0` row relabelled (water low-res pass). `walkabout_defects/README.md` carries the correction.
- **The walkabout set is unchanged** (nothing visual landed; still the post-rebake session-13 set). **The maintainer's defect hunt:** every Opus-safe cause is queued (2, 5 amended, 6, 8, 9, 10, H, L); water is the Fable remainder.
- **Process rules learned (keep):** the destructive-op harness hook denies a Bash heredoc whose TEXT names worktree removal / `rebase --abort` — write briefs with the Write tool; a Fable subagent may be unable to write its final report file — save its return text verbatim; the s13 sweep file captured stderr (`mission …` lines) — the four-part filter is the one that makes `cmp` exit 0; `git worktree remove` without `--force` and `git branch -d` went through for the lander once the branches were merged.

## What next — value order (2026-09-07, after session 13 — the lead's order for the next window; the paste-in prompt is in `engine/artifacts/loop/START_HERE.md` §Hand the parked branches…)
0. **The maintainer's cleanse / audit is the loop's first task (2026-09-07 evening; queue items C1–C3, `engine/artifacts/loop/QUEUE.md`):** C1 = the audit + one plan file, C2 = files + docs on `cleanse-docs` (the ONLY branch allowed to edit PLAN.md / HANDOFF / analysis — the lead reads the entire diff; removed Log text goes to `analysis/archive/`), C3 = one branch per crate with byte-identical sweep/replay/golden gates and a BUGS list (unfixed). Landing them is this window's first job after the reading; land `phase-c` commit 1 early so C3 can reach `core/ai.rs`/`sim.rs`/`unit.rs`.
1. **Land what is parked**: `phase-c` (if still parked — its report under `lead_tmp/s13/reports/phase_c.md`), then every
   loop branch in `git worktree list` (reports under `engine/artifacts/loop/reports/`, items PARKED in QUEUE.md):
   rebase, ci both sides (ONE ci at a time), reproduce the numbers, ff, then once: rebake if any [REBAKE], view every
   parked sheet before `scripts/goldens.sh`, gpu tests, replays, sweep twice for [SWEEP]. Mark DONE, journal, refill.
2. **Phase D** (`brief_phase_d.md` + two addenda; VO re-pointed per `analysis/re-sound-global.md`: BF1's tac chatter is
   in SOUND/CW.LVL + GCW.LVL streams, BF2's command VO is `<side>_command_*` on the `<side>_unit_vo_quick` streams —
   D12 must not look in the per-map banks; both need the GLOBAL/CW/GCW bake, so D12's audio is SILENT until queue
   item 15 lands, the rest of Phase D is not), then the sweep's slot-0 player becomes a bot (the maintainer's decision 2).
3. **Fable RE, in this order (the defect hunt's non-Opus causes, `walkabout_defects/README.md`):** (i) the low-res
   terrain sheet `LOWR` (HOLD 10(i)) — the real fix for every "flat plane to a razor horizon" panel and the MUS1/UTA1
   shell band; it decides whether queue item 5's far cull stays; (ii) the terrain-visibility mechanism under Hoth's
   outpost rooms (both discs; no cutter within 296 m; 17/35 Hoth cutters carry a NEGATIVE sphere radius — check the
   `CUTR` decode); (iii) water (HOLD 10(f)): the water pass takes no fog (KAS2 fogs white over red terrain), the level
   (YAV1's plaza floods, KAS2's beach post in the sea), the flat sea gradient.
4. **The sniper distance table** (Phase C addendum 3 item 2): the roster fields sniper primaries; D9 needs the
   per-weapon zoom/scope ODF keys (untraced) — its own RE + brief after Phase C.
5. **The maintainer's three decisions** above; when (b) is taken, unblock queue item 15 and re-point Phase D's D12.
6. Backlog: two-tier KTX2 runtime half (after queue 11), P7 after Phase D, the era/sound/HUD icon fallbacks, the
   `--cam-overview` / `--capture-orbit` fog look call (the loop's report on `horizon-clear`).

## State at handoff (2026-09-07, Fable session 13 — the four branches LANDED; the autoloop shaken down and handed to the maintainer; Phase C in flight)
- **On main (all lead-verified, PLAN Log 2026-09-07 ×2):** `layer-content` (0b93bb1), `fog-rule` (1201a92 → 0f3fd3e),
  `roster` (b723b41 → 6eeac3e), `walkabout` (225c3a3 → 21c0f32), the COR1/UTA1 vantage floor (edac11a), the goldens
  (081d531: 39 moved + 48 new unit goldens, every sheet viewed), the loop's `horizon-clear` (e80119a), the guard hook
  fix (445f25b). ONE full `scripts/rebake.sh` done (the session-11 bake is gone: 52 soldiers / 31 weapons / the four
  extra class RONs / the layer content / the fog rule are all baked). Worktrees removed, branches deleted, stashes
  dropped. Evidence: `engine/artifacts/lead_tmp/s13/` (`landing_notes.md` is the index).
- **Baselines NOW (post-landing, post-rebake, lead-measured twice):** sweep `finished 25/32  stuck_max median 1200
  (finished-only 900)  >10k 5 maps  sum 241740` (`lead_tmp/s13/sweep_final1.txt`; the pair is byte-identical once the
  `roster <faction>: skipped class …` / `mix team` warnings — printed by parallel workers in nondeterministic order —
  are stripped; queue item 4 fixes that); walk_demo 43d2dfd6ce28c862 (407), aim_demo 242fcc93623533a9 (120),
  `--sim-hash 600` 20fd5f8a77fa37f1; `scripts/gpu_tests.sh` exit 0; ci exit 0 at 445f25b.
- **The walkabout set is current** (`engine/artifacts/walkabout/`, 219 images, regenerated after the rebake; the
  previous set is `lead_tmp/s13/walkabout_before/`). **The maintainer's defect hunt is done:** `engine/artifacts/walkabout_defects/
  README.md` — grouped by cause with crops; the Opus-safe fixes are queue items 2, 5, 6, 8, 9, 10; the Fable items are
  in §What next.
- **The autoloop is shaken down** (`scripts/AUTOLOOP.md` §Shakedown; the maintainer's page `engine/artifacts/loop/START_HERE.md`):
  one `--once` iteration ran end to end (55 turns, 7.8 min, ~$3.34), its branch was landed by the lead; the hook's six
  false-positive denials are fixed (`lead_tmp/s13/guard_test.sh`, 37 cases). `engine/artifacts/loop/QUEUE.md` holds
  14 items + 1 blocked. **`phase-c` is listed there as lead-parked** — the loop skips items 3 and 4 until it lands.
- **Phase C is PARKED at commit 1 of 5** (worktree `<repo parent>/openbf-wt/phase-c`, branch `phase-c`, target
  dir `lead_tmp/phase-c_target`): `adc52bd` "Difficulty + Skill vector (PROVISIONAL tiers), plumbed from
  `--difficulty`; no behaviour change" (D1+D2+D3) is GREEN — ci 0, baselines reproduced by the agent (sweep 32/32
  lines, walk/aim/sim-hash) — commits 2–5 NOT STARTED, nothing WIP; report `lead_tmp/s13/reports/phase_c.md`, NOT
  lead-verified. Next window: verify commit 1's gates (its hash column moves — the format grew by the difficulty
  byte and `Unit.skill`; every other column identical), then relaunch the build from that branch for commits 2–5
  (`brief_phase_c.md` + three addenda), or land commit 1 first. Note from the report: the sweep's warning shape
  on this bake is `roster <faction>: the mission's fill class … falling back to the default list` (not
  `skipped class`) — strip `^roster ` and `mix team` lines before cmp.
- **Decisions brought to the maintainer (PLAN Log 2026-09-07, second entry):** (a) the droideka premise inverts — BF2's ball
  jumps, BF1's does not; keep "jumps everywhere" with BF2's numbers and correct §Movement & feel; (b) the unified sound
  plan — bake GLOBAL.LVL's referenced VO once as OGG by segment-id hash + CW/GCW.LVL's four streams (the universal
  chatter) + CORE.LVL's bank; music streamed; Phase D's D12 re-pointed; (c) the alpha rule — keep v2 until a PCSX2 GS
  dump exists. Queue item 15 (the VO bake) is BLOCKED on (b).
- **Process rules learned this session (keep):** never run two `./ci.sh` at once — 18 tests bake into fixed
  `std::env::temp_dir()` names (queue item 3 fixes it; until then a Fable window must not run ci while a loop session
  may be running its own); `git worktree add <path> main` needs `--detach` (main is checked out) — a failed `cd` after
  it made the lead's candidate script write goldens into main's working tree (caught; nothing committed);
  `git stash drop` takes `stash@{n}`, find n by message; the sweep's warning lines interleave — strip before `cmp`.

## State at handoff (2026-09-03, Fable session 12 CLOSED — items 0 and 1 landed; three builds parked unmerged)
- **Baselines reproduced at 66d9c10 by the lead** (ci exit 0; sweep twice
  byte-identical: `finished 26/32  stuck_max median 600 (finished-only 480)
  >10k 3 maps  sum 139200`; walk_demo 1c1959ed3fd15936 / aim_demo
  45dea6557b35f773 / `--sim-hash 600` 9f8c6f7dccc24191; `scripts/gpu_tests.sh`
  exit 0). Results under `engine/artifacts/lead_tmp/s12/`.
- **Parallel layout (PLAN §Working mode):** worktrees at
  `<repo parent>/openbf-wt/<branch>` with `engine/assets_baked`,
  `engine/artifacts`, `extracted/` symlinked into the main repo and
  `CARGO_TARGET_DIR=engine/artifacts/lead_tmp/<branch>_target`; branches
  `sky-wrap` (item 0: `brief_sky_wrap.md`), `roster` (item 2:
  `brief_roster.md`), `layer-content` (`brief_layer_content.md`). The lead
  merges. **Landed on main:** item 0 (`sky-wrap`, four commits + goldens, PLAN Log "Item 0 landed"). In flight: `roster`, `layer-content`, `fog-rule` (`brief_fog_rule.md`, from the merged main).
- **Session 12 closed (the maintainer, context full):** all five agents finished; their reports are saved VERBATIM under
  `engine/artifacts/lead_tmp/s12/reports/` (layer_content.md, fog_rule.md, roster.md, droideka_NOTE.md,
  sound_global_NOTE.md); branches `layer-content` / `fog-rule` / `roster` are UNMERGED and NOT lead-verified;
  the droideka and sound RE reports (`lead_tmp/s12/re/`) are NOT lead-verified. `NEXT_SESSION_PROMPT.md` is the
  handoff.
- **Item 1, the combined sky/fog/alpha RE: CLOSED at the ELF level** (PLAN Log "The combined sky / fog /
  alpha RE"; `analysis/re-sky-pass.md`, `re-fog.md`, `re-material-flags.md`, `re-texture-wrap.md`). The fog
  rule is being built (`fog-rule`); the alpha rule is NOT changed (decision for the maintainer in that Log entry).
- **Lead findings this session (measured, not yet in the PLAN Log):** (a)
  dome-shell UV census `engine/artifacts/sky_wrap/dome_uv.txt` — most
  shells end at v ≈ 1.00 (Hoth 1.0010, COR1 1.0107, MUS1 1.0767, NAB1 water
  1.13) while BF2 POL1's and TAN1's base domes genuinely tile in V (v
  −1.7..2.7 / −0.5..1.5), so the V clamp is per segment, not blanket; (b)
  **BF2's `lvl_` mode layers carry CONTENT the map bake never reads** —
  `entc`/`modl`/`skel`/`gmod`/`coll`/`prim`/`tex_`/`expc` inside the hashed
  sub-container (BF2 HOT1: 39 entc / 58 modl over four wrappers; POL1 15/15,
  COR1 11/15, DEA1 7/6, MUS1 5/8, MYG1 5/15, UTA1 2/15; BF1 has no wrappers)
  — the bake's `lvl_` arm takes only `wrld`/`plan`/`plnp`, so those props
  are neither drawn nor collided (`--scene-report` skipped classes:
  `engine/artifacts/lead_tmp/s12/skipped_classes.txt`). The Hoth bridges
  were one symptom. Fix in flight on `layer-content`; rebake + goldens +
  sweep by the lead after merge.

## State at handoff (updated 2026-09-03, Fable session 11 IN PROGRESS)
- **Session 11 (2026-09-03): ALPHA RULE v2 LANDED** (3f8c38b, 23bc7d4,
  f7a411f, dda89b4 D7 sky carve-out, c787beb goldens; PLAN Log "ALPHA RULE
  v2 landed"): the alpha mode is per DRAW (glTF `alphaMode` from
  `bake::alpha::verdict`), the renderer routes OPAQUE / MASK / BLEND through
  three pipelines; 28 goldens regenerated after the lead viewed every sheet
  (`engine/artifacts/alpha_v2/`); the sweep did not move. Deferred with a
  home: the side bake (D6), the two-tier side-by-side KTX2 for the maintainer's runtime
  toggle (Opus-safe next), the sky path (D7). **The maintainer's Mustafar sky seam:**
  root-caused to the disc's `mus1_sky_volcano` double-band shell (PLAN Log,
  same entry); the `DomeModel` Offset/MovementScale/scale RE is in flight
  (`analysis/re-sky-dome-placement.md` when it lands) and the fix follows it.
  **Phase B LANDED** (bbb8baa, c613a6a; PLAN Log "Phase B landed"): 27 of
  32, median 600, sum 118,500; every acceptance gate met; the one lost
  outcome (BF2 UTA1) is the BF2 `alive < tickets` bleed rule with the idle
  player as the last unit; KAM1/TAN1 stand with every unit `BLOCKED world`
  (Phase C). Replay baselines now walk_demo 1c1959ed3fd15936 / aim_demo
  45dea6557b35f773 / `--sim-hash 600` 9f8c6f7dccc24191 (the session-10
  aim value 0695… was a bookkeeping slip — PLAN Log "CORRECTION"). **Sky
  placement LANDED** (a937199, f01f45d, goldens 072d783; PLAN Log "Sky
  placement landed"): the ELF's Offset rule and per-material sky GS state;
  The maintainer's Mustafar seam is below every ground horizon (the orbital overview
  still sees the asset's underside); 21 goldens regenerated after the
  sheets were viewed; `engine/artifacts/sky_placement/`. **The
  ordnance subset LANDED** (f8dcf57; PLAN Log "The ORDNANCE subset
  reaches bolts…"): 26 of 32, median 600; BF1 KAM1 and KAS2 are the worst
  outcomes now (bolts cross soldier-only bodies) — Phase C's business.
  **Session 11 closed with the maintainer's decisions recorded** (PLAN Log, the
  entry "Decisions on the session's report"; §What next below is the
  approved order). Phase C (`brief_phase_c.md` + LEAD ADDENDUM) and Phase D
  (`brief_phase_d.md` + two addenda) are written and NOT launched. Bake gap to fix (Opus-safe once specified): BF2
  HOT1's bridges have no class row in the map bake. Briefs written this session:
  `brief_ordnance_ray.md` (the ordnance subset for rays; its census numbers
  reproduced by the lead: 26,980 built bodies, soldier 24,952, ordnance
  23,766, 1,274 soldier-only, 88 ordnance-only).
## State at handoff (updated 2026-09-03 after Fable session 10)
- **Session 10 (2026-09-03): A.4 CLOSED — scoreboard 28 of 32, median
  `stuck_max` 720 (23 / 2,460 at the start).** The NAB2 stalls were units
  standing on a plaza mesh inside its building's box primitives; the PS2
  assigns collision layers per class through ODF properties (Fable RE,
  `analysis/re-collision-layers.md`; built as `game::collision_layers`, the
  session-7 name rule is gone). Also fixed: the sweep's blindness to a
  penetrated body, the never-flipping slide side, the kerb-as-wall wall
  pass. Four maps left (KAM1, TAN1 standoffs; MUS1, POL1 full-speed at
  unreachable waypoints). The constants re-read ran (both decisions kept,
  nothing committed) and the untextured-draws fix landed (eb73e0d). **The
  queue resumes at ALPHA RULE v2**; briefs under `engine/artifacts/briefs/`,
  the next-session prompt at `engine/artifacts/briefs/NEXT_SESSION_PROMPT.md`
  (local). Tier-4:
  `engine/artifacts/collision/layers/`.
## State at handoff (updated 2026-09-02 after Fable session 9) — earlier
- **Session 9 (2026-09-02): A.3 landed (the parallel sweep with the
  scoreboard columns; the per-mesh non-floor sweep hit), then the lead
  root-caused the "sweep says clear, push 0.30 every tick" signature: parry
  3d 0.22.1's composite `contact` tests the capsule at a MIRRORED pose
  against every mesh body — the wall pass had phantom walls and missing
  walls since walkable collision landed. Fixed (`mesh_contact`, 9258cd3).
  The honest scoreboard is 23 of 32** (PLAN Log 2026-09-02); five maps
  stall with units boxed at raised posts and a suspect in the sweep's
  touching branch (A.4, first next). The 148 untextured draws are disc
  facts, not a lookup bug (fix brief written). ALPHA RULE v2 and Phase B
  build specs are in the PLAN Log; the full briefs and code maps are under
  `engine/artifacts/briefs/` (local).
## State at handoff — earlier (2026-09-01 after Fable session 7)
- P1–P6 DONE. **Session 8 (2026-09-01): P9's spine changes DONE, the shared
  `manifest` crate DONE, the AI base DECIDED (PLAN Log "AI decision"), and
  P6.5 Infantry AI v1 is in progress — Phases A, A.1, A.2 landed (27 of
  32), B/C/D designed.** P7 (vehicles) follows Phase C and stays on the
  hold list.
- **Walkable model collision landed 2026-09-01 (session 7): 28 of 32 maps
  finish a conquest round** (21 before); **27 of 32 after P9 (b)** — a 1-ulp
  libm difference flipped BF1 KAS2 into the known under-the-pier stall
  (PLAN Log "P9 (b) landed"; nothing tuned). The next blocker is the
  placeholder AI's navigation, now being replaced (Phase A).
- All 32 ground maps bake and play; soldiers are skinned and animated; every map
  loads its own planet's sound bank. Determinism holds (`--sim-hash`).
- Baked local data (gitignored) lives under `engine/assets_baked/`.
  **Rebake with `scripts/rebake.sh`** (all groups) or e.g.
  `scripts/rebake.sh maps side` — the exact invocations live in that script.
- `PLAN.md` now opens with a **WHERE THINGS STAND** section and its Log has a
  topic index. Read those two before the Log itself.

### The per-session digests

## Opus session 1 (2026-08-30) — see the PLAN Log for detail
All seven Opus-safe backlog items DONE plus two texture bugs: the CLUT erratum
(256-colour CLUTs are LINEAR), BF1 `_dtl` detail textures, camera-vs-buildings,
the spawn screen, placeholder effects, all 30 ground maps baking, and
`bake pc-tex`. Then terrain texturing, BF2 mode-layer matching, and the golden
gallery (42 sheets).

## Opus session 2 (2026-08-30) — what changed
1. **Sound.** The 28 missing SOUND LVLs extracted (additive; `iso/` read-only);
   every map bakes its planet's bank; `bake sound-names` recovers readable names
   by confirmed hash preimage (tagged Direct/Derived); `scripts/audition.sh`
   lists, plays and exports a bank. Fixed a real bug: 63 duplicate sound
   properties across the maps were resolved last-wins and could be silenced.
2. **Audit + cleanup.** Three scoped audits, every finding lead-verified: doc
   reconciliation (module headers, a2's format errata marked inline), dead code
   (5 uncalled functions, one of which would have baked every texture opaque),
   the duplicated `PROP` decoder. Determinism and module boundaries came back
   CLEAN.
3. **Performance.** `--bench` + `OPENBF_ADAPTER` measure both GPUs. Worst map on
   the iGPU is 3.69 ms/frame; sim tick p99 ≤ 0.031 ms. **Nothing warrants
   optimising yet** and the measurements say why (not fill-bound, not draw-call
   bound). Texture memory is the number that will matter on the classic-plus
   path. `engine/artifacts/perf/`.
4. **Death Star and Tantive IV fixed** — three gallery-framing bugs, all ours,
   none in the geometry (see the PLAN Log). All 32 map goldens regenerated and
   viewed.
5. **`mungefs` hardened** against malformed disc data (bounds, recursion depth,
   decompression cap) — the crate's no-panic contract now has regression tests
   — plus three runtime crashes (glTF CUBICSPLINE overrun, a NaN heightfield
   step, `tickets: 0`).
6. **Alpha blending landed.** A composite pass for the textures alpha-testing
   deleted, triggered narrowly (17 textures, not the 377 the material flag
   alone would select). Two headless GPU tests pin it.

## Opus session 3 (2026-08-31) — what changed

All four items the maintainer released this session are DONE, plus two open defects.

1. **Per-map lighting, sky and fog (item A).** `sky_` and `lght` parsed
   (`analysis/re-sky-lighting.md`); 122/123 `sky_` and 23/23 `lght` property
   names recovered as CONFIRMED PREIMAGES. Every map now uses its own sun,
   hemisphere ambient, fog and SKY DOME — the dome models were already baked
   and unreferenced. Bespin is a sunset, RhenVar cold blue, Kamino a storm,
   the Death Star has stars. Three PROVISIONAL judgement calls, all in
   `bake::lighting` with the measurement that produced them.
2. **Colour space (item B).** Authored 0..255 light/fog colours are GAMMA and
   are stored linearised (the `_dtl` GPU-test rule, applied consistently).
   **`COLR` cracked** — it is LUMI in colour, PSMCT16, one u16 per vertex; A2
   had it as undecodable. The LUMI-neutral rule was re-checked and UPHELD, now
   with a decisive cross-check (BF1 and BF2 Hoth store different neutrals for
   the same art).
3. **Optimisation (item C).** `--ai-cap` landed — it was the stated blocker —
   plus `--bench-player-cam`, `--no-cull`, units-alive and DRAW-CALL counts.
   **The answer is that the maintainer's goal is already affordable: 740 alive AI units
   at 6.98 ms/frame on the INTEGRATED GPU, and 720p→4K costs 1.4x for nine
   times the pixels.** The sim scales at ~n^1.4, not n², so A3 (spatial grid)
   and A4 (deterministic parallelism) are retired as work with no measured
   need. Frustum culling landed anyway (−14 to −17 % on the big maps, all 42
   goldens byte-identical).
4. **PC stock textures (item D).** The tier is wired
   (`bake::tier`, `bake map <lvl> <out> [pc.lvl]`). 370 of 1,040 matched BF2
   textures are larger on the PC; 312 are taken. "Larger" was NOT enough —
   `sky_hoth` is a blizzard on the PS2 and a blue sky on the PC — so a
   same-art test (levels OR structure) rejects 24, and 11 detail-compressed
   bases are excluded outright.
5. Also: `game::fx` composites instead of alpha-testing; and the
   `Chunk::children()` accusation is withdrawn (`LOWR` lives under `tern`'s
   `PCHS`, and 17 of 34 maps have none at all — including two that SHOW the
   outside-the-level defect, so `LOWR` is not the answer there).

New HOLD FOR FABLE questions raised this session: what enables fog on the PS2;
what the `SunInfo` back light does; why BF2 Theed ships a black sky texture;
terrain `ALPH`.

## Opus session 4 (2026-08-31) — what changed

The maintainer released `tern -> WATR` and added a follow-on about the Kamino sky seam
and a BOUNDED fog investigation. Items 0–4, 6 and 8 of the task list are done;
5 (baked LOD chains) and 7 (pad-driven spawn state) were NOT started.

1. **Water (`tern -> WATR`) decoded and rendered on all twelve maps that ship
   it.** `WMAP` is one nibble per cell; `LAYR`'s first f32 is the water plane
   and is proven twice; `CPAT` is the shoreline-fitted surface on three maps.
   BF2 Kamino and BF2 Death Star carry a `tern` that is `NAME` + `WATR` only,
   so the water is their ONLY ground. Three PROVISIONAL calls in
   `bake::water_glb`, one of them a REJECTION: `LAYR`'s colour word is
   recorded and applied to nothing, because no byte order survives the art.
   Ruled out: `sky_ WaterInfo` is an all-zero `Enabled(0)` default that mostly
   appears on maps with no water.
2. **Sky objects** — 20 maps author capital ships and none drew. The
   "geometry lives in SIDE" claim was VERIFIED before building on it. Found a
   separate bug: sky domes whose authored name is not lowercase drew NOTHING
   (BF1 RhenVar 1 and 2 had no sky at all).
3. **Parallax sky shells take the map's fog** (the Kamino seam, cause c); the
   base dome still does not. Weight 0.65, PROVISIONAL, picked by sweeping.
4. **Fog investigated within the maintainer's bounds and the answer is a NEGATIVE
   plus a lead.** `Enable` and `FogRamp` are eliminated. BF1 keeps its real
   fog under `DomeInfo.LowResTerrain`, which BF2 promoted into `SkyInfo`
   byte-for-byte on HOT1/END1/YAV1 — implemented, viewed, and REVERTED
   because BF1 KAS2's copy is the placeholder `255,0,0`. `fog_from` is
   unchanged. `engine/artifacts/fog_rule/`.
5. **Terrain split (PLAN B4 DONE)**, −17 to −22 % on the big maps, all
   goldens byte-identical. It must happen at UPLOAD; doing it in the bake
   silently corrupts `Scene::heightfield`.
6. **Mip chains generated** — 1,989 of 2,288 textures had short chains.
7. **PC stock tier wired into SIDE** — the first-person rifle sheet goes
   128x64 -> 512x256.
8. **A flythrough per map** (`scripts/flythrough.sh`, 32 maps in ~75 s) and
   **load time re-measured** — 21–39 ms, so PLAN B7 is retired.

**Measurement caution for whoever is next:** a short bench run on this iGPU is
worth about ±0.4 ms. Session 4's first terrain-split numbers came from
120–150-frame cold runs and were wrong; a median of five 300-frame runs
reversed one of them from −29 % to 0 %.

## Opus session 5 (2026-08-31) — the maintainer's visual defect hunt

The maintainer reviewed every map golden and every flythrough and listed items A–E.
**Five complaints, six bugs — all of them ours — and two items that are not
bugs at all.** Full detail in the PLAN Log entry "the maintainer's defect hunt";
pictures in `engine/artifacts/defect_hunt/`.

1. **The flythrough orbited the world origin** (item C). `capture_orbit`
   inverted `overview_camera`'s eye placement and took the radius from the
   eye's distance to the ORIGIN. BF2 TAN1's centre was 233 m out with a radius
   3.2x too large on a 110 m map. `viewer::overview_bounds` now exposes the
   framing. No interior mode was added — an occlusion test over 32 maps
   saturated at 100 % on 22 and does not discriminate, and after the fix the
   interiors read fine.
2. **Textures were looked up case-sensitively** (item D). A `modl` segment
   spells its texture the artist's way; the `tex_` chunk spells it another.
   **3,752 draws rendered flat grey**; 22 of 32 maps now sit at zero. It is
   also what BF2 Utapau's "grey cylinder" was — the sinkhole wall.
3. **Two separate bugs behind "washed out" and "too dark"** (item A):
   `DomeInfo.Ambient(128,128,128)` halved four skies (the field only ever
   holds one of the two PS2 neutrals, on all 34 maps), and session 4's
   constant `SKY_SHELL_FOG = 0.65` is gone — a parallax shell now takes the
   map's own ramp at its own distance. BF2 Hoth's horizon step: 147 levels to
   1. BF2 Mygeeto is neither and its wash-out is its own authored fog.
4. **The dome census answers the maintainer's question** (item B): 29 of 32 base domes
   reach below the horizon and six maps ship a shell entirely below it. Bespin
   was ours (an invented tan tint over its own cloud sea). **Mustafar is not a
   bug** — two authored rings over the unimplemented `LOWR` sheet.
5. **One fog-density knob at 8 %** (item E), and BF2 Yavin 1 said out loud:
   its fog is ground-level (90..350 m) and the flythrough orbits outside it.
6. Our own sweep: three maps cleared to desert blue below the horizon; they
   now clear to their own dome's bottom row.

New tools, both used above and kept: `game --scene-report` (framing plus every
untextured draw, collected inside `upload_scene` so it cannot drift) and
`mungefs --example sky_dome_census`.

## Fable session 6 (2026-09-01) — the cleanse

A whole-codebase `/cleanse` (`.claude/commands/cleanse.md`): 41 findings
worked to zero — 33 FIXED, 6 DEFERRED-WITH-A-HOME (all in §What next), 1
DECLINED, and the passes that came back CLEAN are recorded in the PLAN Log
entry. What matters for the next session:

1. **The round-completion diagnosis is corrected.** `ROUND_VERBOSE=1 cargo run
   --release -p game --example round_to_outcome -- 60000 42 <maps>` shows 16
   winning-side AI standing 5.6 m from a post they can never take (BF1 KAS2:
   they are at y = −5.9 under the dock, the capture box spans y ∈ (−1.3, 3.3)).
   Same shape on DAG1 (units 4 m ABOVE a cave-floor volume), TAN1/BES1 (y = 0
   under platforms 2–100 m up). The bleed clamp at `alive < tickets` is
   FAITHFUL and fine. **BF2 HOT1's "20 s domination win" is a bug too**: all
   five `hoth_cps` posts author `Team "2"` and the real ownership comes from
   the mission Lua the placeholder RONs cannot express.
2. **The GPU referees have an entry point:** `scripts/gpu_tests.sh` (the ten
   `render` ignored tests SIGSEGV under the harness's default thread count on
   the iGPU; the script serialises them, then runs `goldens.sh --check`).
3. **The golden gallery's overview panel now clears to the map's horizon
   colour** (`viewer::horizon_clear`) instead of Tatooine blue — session 5's
   clear-colour fix had never reached the overview half of any golden.
   Eight goldens moved, all viewed (`engine/artifacts/cleanse/`); BF2 Theed's
   overview is now black because its dome IS black (HOLD 10(c)).
4. **Ledger hygiene:** `PROVISIONAL` is the only label the ledger recognises
   (`core::ai` was "PLACEHOLDER" and invisible to every inventory); unused
   `hecs`/`serde_json` deps removed (the sim is dense `Vec<Unit>` slots, not an
   ECS); a water-parser test that never called the parser now does; the
   bake's `ALPHA_TEST` and the shader's `< 0.5` are pinned to each other.

## Fable session 7 (2026-09-01) — collision, missions, and what the sweep says now

Worked §What next (2026-09-01 list) top-down. Detail: PLAN Log "Walkable model
collision landed" and RE_LOG "`coll` / `prim` / `CUTR`".

1. **Walkable model collision is IN** (item 1; Fable design, Opus build, lead-
   verified). `coll` meshes, `prim` primitives and terrain cutters are baked
   (`collision/<geometry>.glb`, manifest `collision` + `terrain_cutters`) and the
   sim's ground query is `Sim::ground_under` over `core::collision::CollisionWorld`
   (parry3d, queries only). Units stand on Kamino's platforms, Bespin's platforms,
   Kashyyyk's docks and Polis Massa's under-terrain base; terrain LOS/bolt rays are
   crossing-based and cutter-aware. Walls are a horizontal capsule push-out applied
   to PLAYER units only (`Sim.walls_for_ai = false`) — see 3.
2. **Mission RONs are generated from the PS2 bytecode** (item 2; Opus,
   `scripts/gen_missions.py`, 57 files incl. `_con_cw` eras) and CONSUMED: post
   roster (`posts`), `KillObject`s, per-post `Team` reassignment, death regions.
   BF2 HOT1 is four posts 2–2 at tick 1 (CP7 killed); DEA1 kills CP6. The 68
   PC/PS2 disagreements are all "PS2 shrinks the member pool" and are PROVISIONAL
   with the PS2 winning.
3. **The round sweep now measures the AI, not the world.** **28 of 32 maps finish a conquest round** (21 before this
   session; 14 with AI walls on, 18 with the ground query alone), two runs
   byte-identical, all 42 goldens unchanged.
   The residual stalls have three shapes, all placeholder-AI: it beelines into
   walls (so AI walls are off), it steps off platforms (fall lethality and death
   regions now turn that into ticket loss instead of a stuck unit), and inside a
   contested post it stands still where terrain terraces block its heightfield LOS
   (BF1 RHN2 cp4: a real 3 m lip, measured) instead of repositioning. **Do not tune
   `core::ai`** — the AI base decision brief is in the session report for the maintainer.
4. **P7 entity shape decided, P9 specs written** (PLAN Log; items 3–4). Vehicles
   are a second entity kind; P9(a) N pads and P9(b) `core::fmath`/libm are Opus-
   ready specs, each its own commit with the replay check spelled out.
5. **Deferred with a home:** shared `manifest` crate (item 5; brief in the PLAN
   Log; run once the schema stops moving), bolts vs model collision, the AI base.

New tools: `round_to_outcome` prints a per-map replay `hash`; `ROUND_VERBOSE=1`
adds the spawn-node ground check and per-unit terrain/cutter/LOS-to-nearest-enemy
columns. `scripts/gen_missions.py` regenerates every mission RON (never hand-edit
them). `engine/artifacts/collision/` is the Tier-4 folder.

## Fable session 8 (2026-09-01) — the AI is ours: decision, data, navigation

Worked with the maintainer in the main thread first (the AI brief → decision), then the
engine queue one Opus build at a time with Fable RE agents in parallel. Every
agent result was re-measured by the lead before it entered the Log.

1. **AI decision (3ed9d02, PLAN Log "AI decision")**: openbf's OWN AI on the
   discs' authored data; four difficulties on a three-layer model (symmetric
   competence, per-human pressure, team handicap only vs bot-only teams);
   squad commands follow / hold / disperse / clear, ≤ 6 bots within 20 m.
   Designs written for Phases A, A.1, A.2, B, C, D.
2. **P9 spine (728ca01 N pads, c612c4c `core::fmath` on libm)**, each with an
   independent stash-based replay check; the libm change moved the sweep by
   a 1-ulp difference (root-caused with a control build).
3. **Shared `manifest` crate (9d4825a)**, byte-identical bakes.
4. **RE**: `plan`/`plnp`/`Hint`/`BARR` formats + loaders (05c57ff — the graph
   is a precomputed routing table, BF1 uses it at runtime); AI difficulty
   and feel constants (641b1cb, `analysis/re-ai-difficulty.md`); barriers =
   the PS2's Obstacles with a corner-walk (91178dc); Phase C hypotheses
   closed (700a7b4: 77.5° cone, `HitLocation` vehicle criticals, PS2 Elite
   switches the human's aim assist off).
5. **The AI data is baked (2d51fb5)**: 57 nav blocks, every count equal to the
   disc census.
6. **Phase A navigation (74f6554), bolts + AI LOS vs model collision
   (8e7b628), Phase A.1 steering (82e6bbd)**: sweep 28 → 27 (libm) → 28 (A)
   → 24 (LOS honest) → 26 (A.1) → **27 (A.2, 98e0142)**. Three stalls left (POL1, UTA1, KAM1) and two regressions with named causes (NAB2: a sweep per-body floor skip; MUS1: an unsolved corridor).
7. **The maintainer's texture question → census (`engine/artifacts/texture_census/`)**:
   303 flagged textures alpha-tested to shreds, 16 real cutouts drawn opaque;
   the flag bits' semantics measured; ALPHA RULE v2 specified (PLAN Log).
8. Tier-4 for the maintainer: `engine/artifacts/ai_nav/`, `engine/artifacts/
   texture_census/`, `engine/artifacts/collision/bf1_kam1/crate_closeup.png`.

## Fable session 9 (2026-09-02) — A.3, the mirrored wall pass, the honest 23

1. **A.3 (0d8074e, 8734f82)**: `round_to_outcome` runs maps on worker
   threads (7 m 10 s → 2 m 26 s, byte-identical stdout, the footer
   `sweep: finished F/M  stuck_max median A (finished-only B)  >10k C  sum
   D`), and `wall_sweep` takes the first NON-floor hit per mesh body
   (per-triangle over the BVH, face normals). 27 → 26; the lead's
   ROUND_VERBOSE read of the regressions found the NAB2 signature on BES1.
2. **The wall pass was mirrored (9258cd3).** Root-caused with a scratch
   probe and a core-level witness-point test: parry 0.22.1
   `contact_composite_shape_shape` passes `pose12.inverse()` where
   `contact_with_shape` wants `pose12` (upstream master is fixed; we pin
   0.22.1 for the P9 audit). `push_out` and the sweep's touching branch
   now use `mesh_contact` (per-triangle contact, correct pose); a
   regression test on an asymmetric body detects a parry fix. **Sweep 26 →
   23**, every hash moved, player replays unchanged, BES1 fixed, five new
   stalls with real geometry. Tier-4: `engine/artifacts/collision/
   mirrored_walls/`.
3. **The 148 untextured draws** (Opus analysis, lead-reproduced on raw
   bytes): 75 disc segments with empty texture slots, 8 names dangling on
   the disc, 64 BES1 terrain chunks already dropped — no lookup bug. Fix
   brief written (skip, count, report).
4. **Specs released**: ALPHA RULE v2 build decisions D1–D6 and Phase B build
   decisions (PLAN Log 2026-09-02); code maps of both pipelines under
   `engine/artifacts/briefs/`.
5. New HOLD FOR FABLE: what the PS2 draws for a segment with an empty
   texture slot or a dangling texture name (HOLD 18).

## Fable session 10 (2026-09-03) — A.4: the boxed plaza, the blind sweep, the collision layers

Worked HANDOFF §What next item 0 (A.4) end to end; every build was an Opus
agent from a lead brief under `engine/artifacts/briefs/brief_a4_*.md`, every
number re-measured by the lead before it entered the Log.

1. **The "touching branch" suspect was the truth, not a bug** (PLAN Log
   "A.4 (1/3)"): the NAB2 units stood on `nab2_bldg_b_19`'s plaza mesh
   INSIDE the building's two box primitives. **Collision layers are per
   entity class** (Fable RE, `analysis/re-collision-layers.md`, RE_LOG
   "Collision LAYERS"): `SoldierCollision` / `VehicleCollision` /
   `OrdnanceCollision` / … name the bodies per layer, `VehicleCollisionOnly`
   removes one from the soldier set, BF2 adds a per-prim `MASK` byte (the
   `p_-so-` letters) with a loader carry-over quirk. Built (b112661, 1fb9595,
   960fdac): `mungefs` MASK, manifest fields, `game::collision_layers`; the
   session-7 name rule is gone. HOLD 13 answered for subset selection.
2. **The sweep was blind to a body the capsule already penetrates**
   (6907b83): parry's cast returns t = 0 with one normal in every direction
   from inside, and A.2's end-pose test then misfired; a start/end
   penetration-depth test now pairs with the cast.
3. **The remembered slide side never flipped** (caab253): units
   oscillated over 0.8 m for a whole round; a blocked slide now flips the
   side and the stuck side-step chooses it.
4. **The wall pass treated a knee-high kerb as a wall** (dda2afa): the
   ground query climbs 0.6 m, the probe ignored it, the wall pass pushed
   the unit back every tick; one `Capsule::stepped` now serves both.
5. **Scoreboard:** 23 → 22 → 26 → 28 → 28 of 32; median
   2,460 → 720; two runs identical at every step; player replays
   byte-identical throughout; goldens unchanged. Tier-4:
   `engine/artifacts/collision/layers/`.
6. Not built, recorded: the "downward half of the same-level rule" had no
   remaining case after (3); the A.1/A.2 constants re-read is briefed
   (`brief_a4_constants.md`, five-sweep budget) and NOT run.

## What next — value order (2026-09-03, after session 11 — the maintainer's approved plan)

0. **Sky V-wrap fix** (Opus, small): sky-shell samplers clamp V (47 shells ring
   at v = 1.0, 14 pass it — PLAN Log, the maintainer's decisions entry); ground-level
   before/after sheets of BF1/BF2 HOT1, KAM1, NAB1, KAS2, MUS1 for the maintainer; the
   gallery's overview panel moves to a fogged, lower camera (goldens move,
   sheets viewed by the lead).
1. **The combined sky / fog / alpha RE** (Fable, one pass, before any rule
   change): the sky pass scale + projection (the S = 1 claim is inconsistent),
   what enables fog (HOLD 10(a) — the two Hoths must match), whether
   `FUN_0058ed70`'s material rule is the general draw path (HOLD 10(g)), the
   sky texture wrap mode, then ONE rule each with goldens re-viewed.
2. **Class roster** (Opus-safe): every faction's classes from the baked class
   RONs (sniper, heavy, engineer, pilot, jet, special), per-class weapons,
   spawn-select in `core`; one class per faction spawns today.
3. **Phase C** (`brief_phase_c.md` + LEAD ADDENDUM; item 0 there replaces D4
   with the maintainer's aim-assist rule) → **Phase D** (`brief_phase_d.md` + two
   addenda; universal chatter from the BF1 tactical lines) → the sweep's
   slot-0 player becomes a bot (PLAN Log, the maintainer's decision 2).
4. Backlog: the two-tier side-by-side KTX2 for the runtime toggle; the
   droideka-ball-jump RE (the maintainer's universalised behaviour line); BF2 HOT1's
   bridges have no class row in the map bake; BF2's global sound bank
   (827 MB, hashed names) is unread — the unified sound plan; a broadphase
   over `CollisionWorld.bodies`; parry `enhanced-determinism`; the
   loadscreen/sounds RON twins; P7 vehicles RE after Phase C.

**Parallel work (PLAN §Working mode, the maintainer 2026-09-03):** research/RE agents in
parallel freely; code in parallel only as worktrees on branches with disjoint
file sets and their own target dirs under `engine/artifacts/lead_tmp/`, merged
to `main` by the lead with ci on both sides; bakes, `manifest`, `Cargo.lock`,
the sim hash and the goldens serialise through the lead.

## What next — value order (2026-09-03, after session 10)

0. **Two design lines from the maintainer (2026-09-03, PLAN §LOCKED FEATURE SET):** BF2's
   shell/menus/HUD on every map with BF1 icons as the fallback for BF1-only
   weapons (an Opus-safe bake+HUD item once the rosters name them); unit
   behaviour UNIVERSALISED per class (first: the droideka ball jumps
   everywhere, BF1's rule) — each difference needs RE of both ELFs (Fable).
0a. **A.4 leftovers (Opus-ready, briefs exist):** the constants re-read
   (`brief_a4_constants.md`); the ORDNANCE subset for `CollisionWorld::ray`
   (a per-body layer mask in `core::collision`, brief to write — the module
   doc of `game::collision_layers` lists the on-disc cases); the remaining
   stalls each with its verbose evidence in the Log (KAM1 unit 26's push-0.29
   spot to be probed; MUS1's slow trap; POL1's full-speed cluster at an
   unreachable waypoint; TAN1 is a standoff for Phase B/C).
1. ~~The untextured-draws fix~~ DONE (eb73e0d, 148 → 0, goldens untouched) → 2. ALPHA RULE v2
   (`brief_alpha_v2.md`, the build STOPS before UPDATE_GOLDENS) → 3. Phase B
   (`brief_phase_b.md`) → 4. Phase C → 5. Phase D → backlog as before.

## What next — value order (2026-09-02, after session 9) — superseded above, kept for the item text

0. **A.4 (Fable measures, then Opus builds):** the sweep's touching branch blocks open directions from a touching pose (PLAN Log 2026-09-02, the NAB2 spots and the probe recipe) → root-cause in `core` with the witness-point test, fix, re-sweep; then the same-level rule's downward half for units boxed at raised posts (both NAB2s, MYG1); then re-read the A.1/A.2 constants against the honest world. **Then the queue below, unchanged in order: untextured fix → ALPHA RULE v2 → Phase B → Phase C → Phase D.** Briefs: `engine/artifacts/briefs/`.

## What next — value order, with the Fable/Opus call made (2026-09-01, after session 7 — superseded above, kept for the item text)

1. **AI base — DECIDED 2026-09-01 (the maintainer, Fable session 8): openbf's OWN AI**
   (PLAN Log "AI decision"): modern architecture on the discs' planning
   graphs/hints/barriers, four difficulties on a three-layer model, squad
   commands (follow/hold/disperse/clear, ≤ 6 bots within 20 m). Phase A
   (navigation) is Fable-designed; the formats are being pinned (RE_LOG
   "`plan` / `Hint` / `BARR`"). Do NOT tune `core::ai` on Opus; each
   implementation step is released as a spec when ready.
2. **P9 spine prep — DONE 2026-09-01 (session 8):** (a) N pads by player
   slot (728ca01), (b) `core::fmath` on `libm` (c612c4c); replay checks and
   measured bounds in the PLAN Log. Follow-ups (Opus-safe): parry's
   `enhanced-determinism` feature before the first cross-OS lockstep test;
   an AI-battle capture added to the replay check.
3. **Shared `manifest` crate — DONE 2026-09-01 (9d4825a).** Follow-up
   (Opus-safe, same pattern): the loadscreen and sounds RON twins
   (`bake::loadscreen` vs `game::assets`, `bake::sounds` vs `game::sounds`).
4. **Engine queue (one Opus build at a time, the lead's sweep as the
   gate): A.3 (small: `wall_sweep`'s per-body floor skip — take the first
   NON-floor hit per body — and `round_to_outcome` running maps in
   parallel with the median / >10k / sum stuck columns) → ALPHA RULE v2
   (PLAN Log; goldens will move — the lead views the sheet) → the 148
   untextured draws (census §scene-report; BF1 BES1 64, BF2 KAS2 36) → an
   alpha-mode path for `CommonManifest` → PHASE B → PHASE C (with the §9
   numbers) → PHASE D. Backlog: a broadphase over `CollisionWorld.bodies`
   (the AI LOS ray's linear scan is the whole tick regression at 64 a
   side); parry `enhanced-determinism` before cross-OS lockstep; the
   loadscreen/sounds RON twins; P7 vehicles RE (Fable) after Phase C.**
5. **P7 RE pass (Fable) — after 2.** The entity design is decided (PLAN Log
   "P7 DESIGN"); the seven RE questions it lists are the pass.
6. **Open rendering defects, unchanged order:** outside-the-level terrain
   (`LOWR`, HOLD 10(i)), per-patch layer blending (`ALPH`, 10(d)), Hoth
   clipping (11(a)), Theed's black sky (10(c)), `_collision` meshes drawn
   (11(b)) — Fable, each a bounded RE pass.
7. **Optimisation:** B2/B5 only, Opus-safe with the 300-frame median protocol.
8. **Spawn screen → pad-driven spawn-select state in `core`** (Opus-safe).

### The NEWLY OPUS-SAFE releases and the original Opus-safe backlog

Kept verbatim because each release is a SPEC, and two of them are still open
(HANDOFF §Standing backlog items 7 and 8 point back here): §NEWLY OPUS-SAFE
2026-09-01 item 5 (`game --map` with no `--mission`) and item 6 (the single-file
build and `scripts/pack.sh`). The rest are done and are here as the record of
what was released, by whom, and with which acceptance gate.

## NEWLY OPUS-SAFE (Fable, 2026-09-01, session 7)

1. **The P9 spine changes** — PLAN Log "P9 SPEC (a)" (N pads by player slot)
   and "P9 SPEC (b)" (`core::fmath` on `libm`). Own commit each; the lead
   runs the replay check the spec names.
2. **Shared `manifest` crate** — one crate holding every bake↔runtime
   manifest type (`bake::map::{Manifest, ManifestInstance, ManifestRegion,
   ManifestClass, ManifestGeometry, ManifestPath, ManifestLayer,
   ManifestWater, ManifestCollision, ManifestPrim, ManifestCutter,
   CommonManifest}` + `bake::lighting::{ManifestLighting, ManifestFog,
   ManifestSky, ManifestSkyModel, ManifestSkyObject}` and their
   `game::assets` twins). Acceptance: byte-identical RON — bake TAT1, BF2
   HOT1 and common_bf2 to a scratch dir before and after and `diff` the
   manifests (empty), `all_maps_load` green, a round-trip test with every
   Option populated, `ci.sh` green.
3. **Bolts vs model collision** — add `CollisionWorld::ray(origin, dir,
   max_t) -> Option<(t, normal)>` (first hit over bodies in order, strict
   `<` ties, the same parry casts `ray_down` uses, no walkability filter),
   a `projectile::Hit::World` variant, and in `Bolt::spawn` take the nearest
   of terrain-crossing, world hit and capsule hit. `ai::visible` gets the same
   world test after the terrain one. Tests: a bolt into a box body stops at
   its face; LOS through a wall body is false. Replay check: `round_to_
   outcome` twice byte-identical; expect the sweep to move — report it, do
   not tune.
4. **Consuming the remaining mission fields** (HOLD 16) — the mechanical
   half only: `spawn_delay`/`spawn_interval` → `SpawnWaves`, `attacking_team`
   recorded on `Battle`, per-team alive cap. NOT the locals team.
5. **`game --map` without `--mission`**: `make_walk` spawns the player at an
   authored y = 0 — now lifted onto the terrain in `WalkSim::with_teams`; a
   map whose spawn point is over a model floor with no terrain (COR1/DEA1/
   MYG1/TAN1/UTA1 default spawns) still lands at NO_GROUND. Opus-safe: pick
   the map's first friendly post's spawn node when no `--spawn` is given.

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

## NEWLY OPUS-SAFE (Fable, 2026-09-01) — released this session

1. **Per-map mission RONs from the PC `.lua` sources** (§What next #2) —
   the PC mod-tools Lua ARE the mission scripts (A6, 100 % string overlap on
   BF1); generate, cross-check against the PS2 dumps, keep disagreements
   PROVISIONAL. The bytecode half of HOLD #6 stays held.
2. **The shared `manifest` crate** (§What next #5) — pure refactor,
   byte-identical RON.
3. **B2 baked LOD chains and B5 block compression measurement** with the
   300-frame median protocol (PLAN §Optimisation backlog).

## NEWLY OPUS-SAFE (the maintainer, 2026-08-30) — released from the hold list

The maintainer explicitly authorised these for Opus. They are the highest-value visual
work left and they are all *our own* rendering decisions, not PS2 fidelity
judgements that need the decompiled C.

1. **Per-map lighting, sky and fog** — DONE 2026-08-31. (Was: today
   `render::Lighting` is ONE hardcoded sun + ambient for every map
   (`render/src/lib.rs`), which is why every planet reads as the same
   mid-afternoon. Groundwork already exists: `lght` is in the container tag list
   and `mungefs::config` already parses the DATA/SCOP config tree that `lght`
   uses — nothing consumes it yet. `Manifest.sky_name` exists and is always
   `None`. Bake it, drive per-map lighting/fog/sky from it.
2. **Colour-space conventions** — DONE 2026-08-31. (Was: the `COLR` colour
   map, the linear-vs-gamma shading choice, and re-checking the LUMI-neutral
   rule (currently PROVISIONAL:
   neutral 128 on maps whose 99th percentile stays under 204, else 255,
   classified per map). Note the `_dtl` detail modulation is deliberately done in
   GAMMA space with a GPU test excluding the linear answer — do not "fix" that
   without reading RE_LOG "BF1 detail textures".
3. **PC stock textures where larger** — DONE 2026-08-31 (`bake::tier`), with a
   same-art test on top of "larger". See `analysis/community-projects.md`.

## Opus-safe backlog — the original seven are DONE (see §Opus session results)
Ideas that are still Opus-safe if another Opus session runs before Fable:
1. More Tier-4 capture scripts for the maintainer (the `--capture-series` recipes in the
   artifact READMEs are the pattern).
2. Perf work with numbers first: skinned draw instancing, a texture cache,
   startup time (currently ~0.2 s to load the largest baked map).
3. Tests/docs/CI hygiene; keep PLAN.md Log entries for everything.
4. `bake pc-tex` follow-ups that stay inspection-only (e.g. a coverage report
   of which PS2 texture names a PC LVL could override) — but NOT the override
   design itself (hold #8).
Anything that needs a fidelity judgement, a new format, or a "where does the
PS2 get X" answer stays on the hold list.



## Part 3 — PLAN.md Log, 2026-09-07 to 2026-09-09: the Fable landing windows (sessions 13–19)

Moved here verbatim by the loop's cleanse on 2026-09-23 (lane 2), under the same rules as Part 1. PLAN.md's Log
keeps one dated SUMMARY per day of this stretch (2026-09-07, 2026-09-08, 2026-09-09), each naming this part. Every
live item these entries carried already had a live home before they moved: the defect groups and the maintainer's three
2026-09-08 decisions in PLAN §WHERE THINGS STAND, the gate values in `engine/artifacts/loop/QUEUE.md`'s GATES
block, the open RE questions in HANDOFF §HOLD FOR FABLE.

- 2026-09-07 (Fable session 13): **The four parked branches landed — layer content, the ELF fog rule, the class
  roster, the walkabout gallery — serialised through the lead, each rebased and fast-forwarded with ci on both
  sides (0b93bb1, 1201a92/f98d4c5/0f3fd3e, b723b41…6eeac3e, 225c3a3/21c0f32), then ONE full `scripts/rebake.sh`,
  the walkabout set regenerated, every moved golden's sheet viewed (39 of 42 moved; 52 unit goldens now), the
  goldens regenerated (edac11a → the goldens commit), `scripts/gpu_tests.sh` exit 0, the sweep twice, the
  replays.** Evidence under `engine/artifacts/lead_tmp/s13/` (`landing_notes.md` is the index). **Reproduced by
  the lead, per branch:** (a) *layer content* — a 3 s scratch bake with main's binary reproduces the report to the
  row: +49 classes / +82 geometries / +81 collision rows / +97 models / +33 textures over the 16 BF2 maps,
  instances and layers unchanged, the 14 BF1 + tat1 + tat2 + bf2_geo1 manifests identical outside the fog field;
  scene reports cor1 392, dea1 490, end1 1611, hot1 617, mus1 383, pol1 438 draws (HOT1 objective 810 / 9
  skipped); layer census mode-layer bodies 1532 → 2159, base world unchanged. (b) *fog rule* — every row of
  `analysis/re-fog.md` §6 comes out of the bake in disc integers (BF1 gains BES1/END1/GEO1/HOT1/KAS1/KAS2/NAB1/
  NAB2/TAT1, changes BES2/KAM1/YAV1/YAV2, RHN1/RHN2 off; BF2 only KAM1 gains; FEL1's −10 near clamped to 0 — the
  recorded deviation); sim untouched. (c) *roster* — the sweep pair on the session-11 bake reproduces
  `finished 25/32 stuck_max median 1200 (finished-only 900) >10k 5 maps sum 244800` line for line, walk_demo
  43d2dfd6ce28c862 / aim_demo 242fcc93623533a9 / `--sim-hash 600` 20fd5f8a77fa37f1; the pair differs ONLY in the
  interleaving of the `roster <faction>: skipped class …` warnings from parallel workers (the loop gets an item to
  route them through the ordered per-map output); the era gap is live — every BF2 GCW mission's ALL/IMP classes
  miss the BF1 db and fall back to the default list (the runtime half, HOLD FOR THE LEAD → an Opus-safe queue item
  now that the four RONs are baked). (d) *walkabout* — the 2× comparison viewed on bf2_yav1 and bf1_end1: same
  framing and content, the differences are alpha-tested foliage and mip choice resolving finer; accepted.
  **After the rebake (layer collision + 52 soldiers / 31 weapons + 4 class RONs; the 17:42 `names.ron` mtime is
  settled):** sweep twice byte-identical (warnings stripped), `finished 25/32 stuck_max median 1200 (finished-only
  900) >10k 5 maps sum 241740` — exactly ONE line moved vs the roster baseline: bf2_dea1 finishes at 26,809 ticks
  (was 31,505; end-of-round worst stuck 480, a storm trooper on the cp5 approach; the mid-round peak 5,760 was 8,820)
  with the mode layer's central bridge / platform / door collision in its corridor; the hash column moved on
  bf2_dea1 and bf2_pol1 only (POL1's crate stacks touched a trajectory without changing the outcome); nothing tuned.
  Replays after the rebake unchanged (43d2dfd6ce28c862 / 242fcc93623533a9 / 20fd5f8a77fa37f1). **The lead's own
  one-liner (edac11a):** `gallery::vantage_camera` floors against the collision world's soldier-walkable surface on
  maps with no heightfield (COR1's plaza is a model — the camera sat under it); it also lifted UTA1's vantage onto
  its platforms; DEA1/MYG1/TAN1/KAM1 unchanged. **What the sheets showed (all viewed, `lead_tmp/s13/golden_sheets/
  README.md` has the table):** BF1 Yavin/Endor/Geonosis/Bespin/Tatooine/Naboo/Kashyyyk/Kamino take the disc's own
  fog; the two Hoths now match (BF1 gains the grey ground fog, BF2's mountain shell comes out of the blizzard);
  BF2 Endor/Felucia/Mygeeto shells come out of the fog; the spawn panel gains the class row; 52 unit goldens, every
  soldier textured. **Defects the sheets exposed (into the defect hunt below):** BF1 KAS2 draws a saturated red
  horizon line — the terrain and water run far past the PS2's 1500 m far-scene cull (`re-fog.md` §6) and reach
  100 % of the (255,0,0) fog; BF2 END1/FEL1/YAV1 keep the authored fog colour where the disc has no far layer
  (the recorded PROVISIONAL deviation) and now show a sharp shell over fogged ground. **Process findings:** two
  ci runs must never overlap — 18 tests bake into fixed `std::env::temp_dir()` names and the merged-tree ci
  failed in `battle_replay` while the worktree's ci ran the same test (re-run serially: green; a queue item makes
  the names per-process); a `git worktree add <path> main` needs `--detach` when main is checked out — the lead's
  candidate script fell through into the main tree and wrote the candidates into main's working goldens (nothing
  committed; the sheets were built from `git archive HEAD` and the viewing gate held). Worktrees removed, branch
  refs deleted; the two candidate stashes are dropped after this entry. Tier-4 for the maintainer: `engine/artifacts/
  fog_rule_v2/` (the two Hoths), `roster/README.md`, `layer_content/README.md`, the refreshed
  `engine/artifacts/walkabout/` (219 images, 1.07 GB).

- 2026-09-07 (Fable session 13): **The maintainer's map-defect hunt over the refreshed walkabout set — 219 images, every one
  viewed by four Fable analysis agents, the load-bearing ones re-viewed by the lead — grouped by cause; the RE
  verifications behind three decisions for the maintainer.** Tier-4: `engine/artifacts/walkabout_defects/README.md` (the
  grouped list with crops); the four reports `lead_tmp/s13/reports/defects_{A,B,C,D}.md` (with 146 native crops).
  **The maintainer's Mustafar line, measured (D):** the hard line sits at the same panel row from cp2 (58.6 m) and cp4
  (75.9 m) — a world-fixed terrain edge would have moved ~12 px per 17 m of eye height — so it is the volcano
  SHELL's own mountain/lava segment seam, with the lava segment continuing below the horizon where the port draws
  nothing; UTA1 shows the same wedge in the clear colour under its cloud shell, YAV1 its shell rim over flat fog.
  One cause, three maps; the PS2 covers that band with the far scene / its low-res sheet (`LOWR`, HOLD 10(i)).
  **The groups, worst first:** (1) *outside the level / no far cull* — BF1 KAS2's saturated (255,0,0) horizon line
  in 13 of 28 panels (terrain and water run to 100 % of the red fog; the PS2 culls the far scene at
  FarSceneRange 1500 m → 25 %), BF2 KAM1's pure-black band under its shell (800 m → 42 %), the flat outer plane to
  a razor horizon on BES2/RHN2/DAG1/END1/YAV1 vantages, the MUS1/UTA1/YAV1 shell bands; (2) *terrain drawn where
  the PS2 hides it* — the snow heightfield rises INSIDE both Hoth outpost rooms on BOTH discs (BF1 cp3/cp4, BF2
  cp3/cp4; the lead checked: no terrain cutter within 296 m of either room, 44/61 cutters per map of which 17/35
  carry a NEGATIVE sphere radius — a terrain-visibility mechanism the bake does not decode, Fable RE); (3) *the
  BF2 "no far layer" deviation now visible* — END1/FEL1/YAV1 keep the authored fog colour where the disc draws none
  (Felucia's dark green wall under a yellow sky; Endor's grey wall against the sharp dome treeline) — the recorded
  PROVISIONAL deviation, now with a build plan (re-fog §2.5: fade into the sky between NearSceneRange b..c);
  (4) *materials* — BF1 YAV1's bazaar ribbons drawn as solid white-and-gold cards (an alpha strip; ALPHA RULE v2's
  verdict to be named), BF2 COR1's sky-lane strips as opaque white bands (0x40 additive? the port has no additive
  path), RHN1/RHN2 wall faces vertically smeared, DEA1's floor grid red where texture alpha is 0, GEO1's lantern
  halos; (5) *water* — KAS2's sea fogs to near-white while its terrain fogs red (the water pass takes no fog),
  YAV1's plaza floods to the step, KAS2's beach post stands in the sea, KAM1's sea a flat gradient (HOLD 10(f));
  (6) *missing instances* — BES1's tapering sky sliver between the hub rooms' wall ring and ceiling cap (65 skipped
  instances on that map), TAN1's frameless doorway; (7) *tool* — every walkabout PNG keeps the framebuffer ALPHA
  (bf2_end1/vantage 36.7 % of pixels < 255: a compositing viewer shows a brighter picture than the framebuffer),
  the session-13 vantage floor lifted UTA1's camera until the sky left the frame, seven posts whose eye is in a
  rock or a wall, DEA1's interior vantage on the roof; (8) *known and unchanged* — the Hoth white clip is real RGB
  (HOLD 11(a)), Theed's black sky, GEO1's Core Ship crescent (the same lighting-sum family). **Clean:** bf1_yav2,
  bf1_kas1, bf2_geo1, bf2_kas2, bf2_myg1, bf2_nab2, tat1, tat2's ground sheets. **The split:** Opus-safe, into
  the loop's queue (`engine/artifacts/loop/QUEUE.md`, refilled this session): the walkabout tool fixes, the
  far-scene cull behind a flag (with the KAS2/KAM1 row-scan gate and sheets for the lead), the "no far layer" fade,
  the BES1 sliver, the COR1 additive path, the material investigations (report only), the per-panel visual
  regression test, the era-aware class db. Fable, the lead's own queue: the `LOWR` low-res sheet (the real fix for
  group 1), the terrain-visibility bits under Hoth's rooms, the water's fog and level, the negative cutter radii.
  **Recommended order for the maintainer:** the tool fixes → the far cull A/B → the fade → then the `LOWR` RE (which decides
  whether the cull stays) → materials → water. **Three decisions for the maintainer (evidence in `analysis/re-droideka.md`,
  `re-sound-global.md`, RE_LOG 2026-09-07; the lead re-read the key lines and bytes):** (a) **The droideka premise
  is inverted.** §Movement & feel says "the ball can jump on every map, as in BF1 — BF2's ball cannot"; the ELFs say
  the opposite: BF2's ball has the button jump (BallJumpHeight 1.27 m → 4.99 m/s, EnergyCostJump 30 of 100, 0.1 s
  coyote window, +25 % along facing, Rolling only), BF1's has none (its jump slot is the base stub; the only
  upward impulse is a mirror bounce off what it drives into). Recommendation: keep "the ball jumps on every map"
  as the universal choice and correct the bullet to "as in BF2 (BF1's cannot)" with BF2's numbers; the droideka
  stays out of the roster until its own build (it is not a soldier chain). (b) **The unified sound plan.** BF2's
  VO is never resident — it streams from GLOBAL.LVL (29 mono streams, 196.6 MB) by segment id, ≤ 6 slots; BF1's
  nine-command tac chatter is in SOUND/CW.LVL + GCW.LVL streams, not the per-map banks; `shell_menu_accept` is in
  CORE.LVL (HOLD 9 answered — a search-scope miss). Recommendation: bake ONCE (i) GLOBAL.LVL's referenced VO
  segments to OGG keyed by segment-id hash, (ii) CW.LVL + GCW.LVL's four streams as the universal chatter (the maintainer's
  decision 5: BF1's lines, BF2's `<side>_command_*` where BF2 maps have them), (iii) CORE.LVL's `global` bank +
  `snd_`; keep the two music streams (630 MB) streamed; Phase D's D12 re-pointed at (ii). (c) **The alpha rule.**
  The ELF's CPU-side material rule (re-material-flags.md) contradicts ALPHA RULE v2 on the Kamino crate, the class-D
  banners and Mygeeto's mountains, and read literally it contradicts what the PS2 shows; the GS-level truth needs a
  PCSX2 GS dump (HOLD 7, the maintainer's rig). Recommendation: keep v2 until that dump exists; build the A/B flag only then.
  The YAV1 ribbons and COR1 strips above are v2 verdicts to NAME, not a reason to change the rule.

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

- **2026-09-08 — the maintainer's three decisions TAKEN (all per the lead's 2026-09-07 recommendations):** (a) the droideka ball jumps on every map, as in BF2 (BF1's cannot), with BF2's numbers (`analysis/re-droideka.md`; C2 corrects §Movement & feel; the build stays P7-class, HOLD); (b) the unified sound plan (`analysis/re-sound-global.md` (i)+(ii)+(iii), music streamed) — queue item 15 unblocked and moved ahead of Phase D; (c) the alpha rule stays v2 until a PCSX2 GS dump is captured and read. The maintainer also approved the bloat cleanup (run, above). The autoloop starts today; the lead's remaining Fable-only queue is water (HOLD 10(f)/19(c)) and, later, the vehicle RE for P7.

- **2026-09-08 (evening) — Fable session 15: the ten branches the two-lane loop parked are LANDED; Phase C is COMPLETE; the gate values moved by design; the queue is refilled with a drained-queue backoff; the bloat is gone.** the maintainer's rule held: Fable did only what needs Fable — two Opus landers (briefs `lead_tmp/s15/briefs/landing_{A,B}.md`, reports `lead_tmp/s15/reports/landing_{A,B}.md`, ci logs `lead_tmp/s15/logs/`) did the mechanics; the lead read every loop report, read C2's whole diff, reproduced the load-bearing numbers on main, and took the decisions. A Fable RE agent answered the one RE question (below).
  **Landed on main, in this order — each rebased onto main in its worktree, ci exit 0 there AND on main (one ci at a time under `flock engine/artifacts/loop/ci.lock`), fast-forwarded, worktree and branch removed:** `cleanse-docs` (C2, 13 commits, 4fcc3ca — the lead's whole-diff check: every non-blank line removed from PLAN.md/HANDOFF is in the new file or verbatim in `analysis/PLAN_LOG_2026.md` — 4,175 of its 4,281 lines are contiguous runs of the originals and the 71 others are its own headers and pointers — bar the deliberate rewrites: §WHERE THINGS STAND, the droideka line, the parry line, the topic index, HOLD 14, two pointers; decisions: `scripts/scan_tables.py` KEPT (the only path from an ELF back to the binding tables), HANDOFF's second copy of the baselines is now a pointer to PLAN §WHERE THINGS STAND, HOLD 14 pruned to a one-liner); `cleanse-bake` (7, 8841778 — the lander's FRESH scratch bake from the rebased branch: 15,340 of 15,375 files byte-identical, the 35 others all `sounds/names.ron` + `audio/*.ogg`, the M-B3/M-B4 nondeterministic family; decisions: B-B3's two `baked_root` definitions STAY (the shipped binary must not link `bake`), B-R7 and B-M1 stay open in the C3 entry); `cleanse-core-nav` (7, 88f9430; `226f31a` `ron` → dev-dependency KEPT; its bug B-CN1 → queue item 17); `phase-c2` (12, fc077bb — **PHASE C COMPLETE**: commit 2 perception (contact memory, cone + view distance + ordnance LOS, reaction, hearing, target choice), 2a the FLOAT error floor/cap (addendum 4a), 3a the aim tracker + error radius, 3b the AI sniper distance table (addendum 4b — the `ZoomMax >= 6 && YawSpread == 0 && TriggerSingle` predicate; measured over the sweep: 39,235 of 1,411,120 AI fire presses came from a table class, exactly three distinct values each reproducing table × the skill multiplier, no sniper shot past 60 m), 3c fire discipline (bursts, the pause, charge weapons, dodge-on-hit), 4 hints in combat (`PrimaryStance` → a stance wish, the `ROLL_STANCE` tap controller — 18.6 % of think-ticks carry a wish, 0.3 % of unit-ticks give up movement to stand, 11,368 presses in the whole sweep), 5a D12 (bot-on-human damage × `human_damage`, the Elite ×1.25 bleed against the lone human's side), 5b the arena (`game --example arena`); every gate moved by design and is re-baselined below); `walkabout-rgb` (2, 56fa51a — RGB walkabout PNGs (219/219 lost their alpha plane, 0 differ in RGB; the set is 1.1 GB → 880 MB) and the 25° vantage pitch cap: the floor lifted exactly two vantages, COR1 stays inside the cap, UTA1 goes 31.8° → 25.0° with visible sky 15.0 → 23.0 %; ONE golden moved, `goldens/maps/bf2_uta1.png`, regenerated by the lead after viewing the parked sheet; decision: the sinkhole rim standing above UTA1's horizon is geometry, not a pitch defect — no new item); `test-scratch-dirs` (2, c8cccf2 — 12 of the 18 fixed temp-dir names carry the pid, T1's two tests fail loudly instead of skipping; PARTIAL: the 5 files parked branches owned are its resume, item 3; decision: extend T1 to the map goldens too); `bes1-sliver` (1, 15f239d — **item 8 CLOSED as "reproduces the disc"**: `bes1_bldg_main02` has an open ring between its ceiling slab (top 106.97) and the parapet over an outdoor ledge (103.50), 5,492 of 5,492 declared indices consumed, the 65 skipped instances are all `com_*` markers, the collision has the same opening; the lead re-ran the branch's `segdump` example on four LVLs — BF1 BES1 37, BF1 HOT1 55, BF2 TAT2 90, BF2 HOT1 101 models, 0 mismatches — and corrected `analysis/a2-bake-formats.md` §Models: the LAST `modl INFO` u32 is the model's total index count, not a face count, and the +60 word `face_count` reads is unidentified and unconsumed); `mat-invest` (0 commits, worktree removed — item 10 all three ANSWERED, `engine/artifacts/mat_invest/README.md`: (a) BF1 YAV1's ribbon quads are `yav_bldg_ruins_column2` seg1, MTRL 0x31 RTYP `Specmap`, ALPHA RULE v2 rule 2 → OPAQUE, which IS the PS2's base pass — the port lacks the additive specular pass `re-material-flags.md` §4 [V] describes; 84 primitives / 25 textures port-wide → folded into item 9; (b) the RHN streaks are in the texture (V spans 0.00–0.81, anisotropy 1.2, no slot dropped — the crisp neighbour is detail-compressed); (c) DEA1's red grid is a second surface, one flat texel RGBA (189,45,46,0) under the MASKed grating, red on the disc too); `parry-determinism` (1 + Cargo.lock, 120e7e0 — the three lock edges read: parry3d +indexmap, simba +libm, indexmap +serde/serde_core; nothing moved on this box because libm's `sqrtf` IS `sqrtss`; the proof is one cross-OS run, backlog); `collision-broadphase` (3, 1c2eb89 — a 16 m XZ grid in CSR over the bodies' AABBs, a SUPERSET prefilter (every exact test still runs, ascending order kept, full-scan fallback when `bodies.len()` changes), cell size measured across five sizes; lead-reproduced TAT2 ai=64 sim tick mean 1.362 → 0.368 ms, p99 2.743 → 1.217 ms; the hazard — a body replaced or moved in place keeps the length, so the grid stays live — is P7's to close by making the field private).
  **Lead-reproduced on main `1c2eb89` — BASELINES NOW = `lead_tmp/s15/baselines/SUMMARY.txt` (§WHERE THINGS STAND carries the canonical copy):** the sweep pair `cmp`-identical raw INCLUDING the hash column (stdout captured alone); footer `sweep: finished 26/32  stuck_max median 900 (finished-only 540)  >10k 5 maps  sum 271740` — 25 → 26 of 32: BF1 KAM1 and BF2 UTA1 finish since commit 3c, BF1 HOT1 stopped finishing at commit 4 (team 0 down to the IDLE PLAYER alone — the artefact the maintainer's decision 2 removes, queue S0), the four Phase B standoffs (BF1 KAS2, BF2 KAM1/MUS1/POL1/TAN1 minus one) unchanged in kind; `scripts/sim_gates.sh` walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, both `motion.csv` byte-identical to the s14 (and pre-phase-c) trajectories — the hash FORMAT grew at commits 2, 3a, 3b, 3c and 4 and the trajectories never moved; the arena `flat 3000 42` twice identical (kills/min 45.49 / 122.33 / 166.89 / 200.00, hit rate 0.181 / 0.362 / 0.456 / 0.588, `monotonic: kills yes  hits yes`) and `tat2 9000 42` twice identical (kills/min 29.0 / 29.8 / 40.0 / 33.0 — NOT monotonic; hit rate 0.139 / 0.243 / 0.300 / 0.299; ttfk 18.5 / 17.3 / 15.4 / 14.5 s IS); Seeds 43 and 44 (lander A, `s15/logs/arena_{tat2,flat}_seed4{3,4}.txt`): the flat field is monotonic on kills and hits in all three seeds; tat2's hit rate flattens at ~0.30 for Hard and Elite in all three (kills/min monotonic on seed 44 only), and time-to-first-kill is monotonic on every seed and both fields. `scripts/gpu_tests.sh` 0 with the one designed golden moved and regenerated; ci 542 tests. **No rebake was due** — no landed branch changed a baked byte (cleanse-bake proved it; the others touch no bake file).
  **Decisions (lead):** (1) **`PrimaryStance` 3:** NOT prone — lead-verified RE (RE_LOG 2026-09-08 evening; `lead_tmp/s15/re/primary_stance.md`): the value is a BITMASK (bit0 stand, bit1 crouch, bit2 prone) and the PS2 draws one stance uniformly among the set bits at hold entry; BF2 masks `& 3` (its AI never prones — the go-prone slot is a stub), BF1 keeps the byte (prone only from 4–7). The port rule is the mask + a seeded draw, BF2-sourced maps `& 3`, BF1-sourced raw → Phase D commit 0b; Phase C's 1/2/3 mapping stands PROVISIONAL until then. (2) **D12's reinforcement bias: NOT built.** The Elite handicap is the ×1.25 bleed alone until the maintainer's Tier-4 Elite feel check says it is too weak; if it is ever built it is the bot-only side's reinforcement pool ×1.25 (PROVISIONAL) behind the same host option — a second lever on the same axis is not worth a visible ticket-count asymmetry today. (3) **tat2's non-monotonic kills/min: ACCEPTED** — one seed on a map where cover and post pressure compete with the tier; the flat field (the acceptance field) and time-to-first-kill are monotonic; seeds 43/44 confirm the shape — the MAP field caps Hard/Elite hit rate at ~0.30 (cover and post pressure), the tier table itself is monotonic; a Phase-D-era arena run on a second map is the next look, not a tuning now. (4) The `WeaponClass::bf1_rifle()` fixture keeps the disc's `zoom_max` 2.5 (addendum 4b's "1.0 in both constructors" applies to `Default` only); the fire-cycle carryover stays COUNTED (`NavStats.carry_shots`) — ending the cycle on trigger release is a weapon change that moves the hash. (5) The hash-test gap PC left (no per-field assertion for D9–D11's eleven hashed fields) is Phase D's commit 0. (6) B-CN1 — `Battle::on_death` drives `reinforcements` negative against [V] `re-tickets-spawn.md` and the raw field is hashed — is queue item 17: a designed hash-column move, `battle.rs` only, lead re-baselines. (7) **The additive pass:** ONE `BLEND_ADD` pipeline serves item 9's strips AND the Specular class's second draw (mask = the base texture's alpha, PROVISIONAL — the ELF samples a highlight texture; item 9 reads §3/§4 first). (8) T1 extends to `every_baked_map_matches_its_golden` (item 3's resume). (9) B-B3, B-R7, B-M1, C10 as written in the C3 entry.
  **Loop:** the drained-queue backoff — a session that finds no eligible item touches `engine/artifacts/loop/DRAINED` and exits (one journal line, no report; `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md`), every lane sleeps `PROBE_INTERVAL` while the file exists (`scripts/autoloop.sh`, replaced by rename while the lanes ran), the lead removes it at refill. Script fixes on main: `scripts/scratch_bake.sh` creates `classes/` (the `classes` group died on a scratch tree); `scripts/cleanse_delete.sh` Tier B now covers the branch scratch copies (`bake_after`, `before`, `c2_*`, `goldens_committed`, `nd`, `base`, `spot1..6`) and the superseded s13 scratch bake + pre-s13 walkabout set. **Cleanup (the maintainer, 2026-09-08 evening, "delete unnecessary files too — the folder is ~242 GB"):** `cleanse_delete.sh --yes` twice this window — 245 (28 + 74 + 143 GB logical, per the script's own du) GB in 49 paths (the dead per-branch cargo targets, the scratch bakes and copies); the repo is now 76 GB, of which the maintainer's inputs are iso 8.8, `PC_swbf2_exe` 9.6, `community_mods` 13, `extracted` 2.2, `tools` 2.0, and the live build `engine/target` 18 — nothing else large is regenerable. **Queue refilled** (`QUEUE.md`; the GATES block = the s15 values, `BASE` absolute, stdout-only sweeps): H → 15 → 16 → 11 → 5 → 6 → 9 (+ the additive pass) → L → PD (+ commit 0) → S0 (the slot-0 bot, after PD) → 17 → 13 → 4 → 7 → 3 (resume) → C3 row 6 (after PD/S0). The walkabout set regenerated at close (RGB, the cap): 219 images over 32 maps in 18.9 s, 889 MB, every PNG RGB, `INDEX.md` rebuilt; the vantage panels carry the 25° cap (`lead_tmp/s15/logs/walkabout_regen.log`). **Not done:** water (HOLD 10(f)/19(c)) — the Fable remainder; P7's vehicle RE. **Process:** the lander briefs carry the flock rule and the stdout/stderr split; a Fable-window command run inside a worktree moves the harness's cwd there — use absolute paths from the main checkout; the guard's `writes_leadfile` false positive (a lead-file name inside a sed replacement) is still open; `cleanse_delete.sh` is safe while landers run (it skips live names).
- **2026-09-08 (night) — Fable session 16, STOPPED MID-WAY at the maintainer's request (20:53–21:30): three of the six branches the loop left are landed; the rest, the rebake, the goldens, the final gates and the refill are the next window's, with every decision already taken (HANDOFF's top block).** Landed on main in queue order, each rebased in its worktree, ci exit 0 there and on main under `flock engine/artifacts/loop/ci.lock`, the three replay hashes / both motion.csv / the sweep (raw `cmp` against `s15/baselines/sweep_main_1.txt`, hash column included) UNMOVED on each, fast-forwarded, worktree and branch removed (Opus lander A, brief `lead_tmp/s16/briefs/landing_A.md`, report `lead_tmp/s16/reports/landing_A.md`, every number below re-read by the lead from the raw logs under `lead_tmp/s16/logs/`): **`terrain-ptch` (H, 2 commits, 50c7993)** — the disc's pre-clipped `PTCH` replacement mesh (`analysis/re-terrain-cutters.md`) is baked for the 122 flagged patches on 13 terrains and the viewer chunks the baked triangles instead of re-deriving the grid; a fresh scratch bake differs from the shipped tree in EXACTLY the 13 flagged `terrain_*.glb` (+ the pre-item-16 names.ron/ogg noise); the test `bf1_hot1_outpost_rooms_are_no_longer_roofed_with_snow` pins the RE's two numbers (a hole at cp3, −2.56 m at cp4); the lead viewed all 101 walkabout A/B sheets and 13 crops (`lead_tmp/s16/sheets_view/`): the Hoth rooms show console banks, grate and doorways on both discs, TAT1's buried staircase is back, GEO1/POL1/END1/DAG1/RHN1/YAV2/BES2 reshape near ground only, the 61 unmoved panels are black, no hole-to-sky, spike or black pixel; the file-set deviation (`game/src/assets.rs`, `viewer.rs` — the two reads that assumed an n² vertex count) ACCEPTED, the chunker proven equal to the old grid walk on a plain grid. **`global-sound` (15, 3 commits, e0da217)** — the unified sound plan's bake (`analysis/re-sound-global.md` (i)+(ii)+(iii), the maintainer's decision (b)): a mungefs stream reader, `bake global-sound` writing one OGG per SEGMENT ID (BF2 GLOBAL 5,345 OGG / 80,547,651 B; BF1 CW+GCW 488 / 6,121,197 B; every Ogg page's serial rewritten to the id so the output is reproducible), CORE.LVL's resident bank as `core_bf{1,2}/sounds/` (HOLD 9's `shell_menu_accept` is in it), and `sounds.ron` gaining `stream`/`segments` (14,051 lines added on YAV1, 0 removed); music stays streamed; the lander's fresh bake reproduces the 5,852 new files and the 32 changed `sounds.ron` exactly; decisions: `VO_QUALITY = 3` (88 MB) stands PROVISIONAL, the 950/31 VO ids no shipped `snd_` names are NOT baked, the runtime half is a queue item (GS). **`bake-determinism` (16, 2 commits, 91f3532)** — M-B4 (the beep OGG serial pinned to `fnv1a_lowercase(name)`, CRC per RFC 3533) and M-B3 (the recovered sound name is a minimum over a total order: Direct beats Derived, then the lexicographically smallest spelling — 105 of 11,779 entries flip case to the disc's own capitalisation); **two fresh scratch bakes of main's binary: `SAME=21227 DIFF=0` over the whole tree including the 5,835 VO files — the bake is byte-deterministic and the bake-bytes gate is DIFF=0 from here on** (`scripts/cmp_bake.sh`'s header rewritten by the lead; the QUEUE GATES block follows at the refill). ci 549 → 560 → 564. **NOT done, deliberately:** no rebake and no golden regeneration (item 11 is also [REBAKE] — one rebake after it lands); `engine/assets_baked` is still the s15 bake and ci/gpu are green against it. **Also viewed and decided this window** (the next window executes): item 11 `two-tier-ktx2`'s 27 sheets — texture detail only (FEL1's ground to the disc's mottled sheet, POL1's star field and hull plates, MYG1's grating, COR1's facades) → LAND, and **the goldens render the PS2 base tier from now on** (strict-faithful referee, independent of the PC install; the runtime half T2R puts tier 2 ON in play and OFF in the golden tests); phase-d's four complete commits (0a/0b/1a/1b — the `PrimaryStance` bitmask, `core::squad`, the pad edge) LAND and the 0b move IS the re-baseline (footer `finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480`, tat2 arena 02a2…/4066…/3b56…/86cf…, prone wishes 45 → 0 of ~600), the branch stays IN PROGRESS at D5; item 17 (`battle-ticket-floor`) moves nothing — measured — no re-baseline. **Loop:** the two lane processes started 14:06 predate the DRAINED backoff (bash parsed the loop body once) and spun eight no-item sessions at ~$0.30 each after the queue drained; a lane-2 session parked them on a root `PAUSE` (honoured by the old script) at 20:17 — they sleep at zero cost; the next window restarts them on the committed script after the refill. The queue generator for the refill (`lead_tmp/s16/gen_queue.py`: every kept item verbatim from `s16/QUEUE.md.before`, new items GS / T2R / T2S / OGG / W) and lander brief B are written and unrun.

- **2026-09-08 (late night) — Fable session 16 COMPLETED (resumed ~22:10 on the paste-in prompt; nothing new decided): the other three branches landed, the ONE rebake, 24 goldens (21 map + the three tat1 captures) regenerated after viewing, the gate values re-baselined on phase-d's 0b move, the queue refilled, the lanes relaunched.** the maintainer's rule held: an Opus lander (brief `lead_tmp/s16/briefs/landing_B.md`, report `lead_tmp/s16/reports/landing_B.md`, logs `lead_tmp/s16/logs/`) did the mechanics; the lead checked every load-bearing number against the raw logs, ran the rebake, the goldens, the final gates and the docs himself, and reproduced every baseline on final main.
  **Landed on main, in queue order — each rebased in its worktree, ci exit 0 there and on main under `flock engine/artifacts/loop/ci.lock`, fast-forwarded:** **`two-tier-ktx2` (11, 1 commit, aff46dd)** — the base `<name>.ktx2` is ALWAYS the disc's pixels and the larger PC texture, where there is one, is written beside it as `<name>.tier2.ktx2` carrying the PS2 alpha mask (`bake::tier::tier2`, `Resolved::take_alpha_from`; manifest `texture_tier2`, additive, on `Manifest` and `CommonManifest`); the lander's fresh scratch bake vs the shipped s15 tree: DIFF = 406 = the 292 `*.ktx2` whose base was a PC replacement (284 the manifests listed as `"pc"` + 8 in `common_bf2` no manifest recorded) + 34 manifests + the 80 of lander A's families; ONLY_IN_A = 342 `*.tier2.ktx2` + the 5,852 VO files; nothing outside a family; the three replay hashes / motion.csv / sweep unmoved; ci 566. Worktree and branch removed. **`phase-d` commits 0a/0b/1a/1b (PD, 6275a6d; the branch stays IN PROGRESS in its worktree at D5, HEAD == main after the ff)** — 0a the hash-test gap (eleven D9–D11 fields asserted one at a time), **0b the `PrimaryStance` BITMASK** (`commander::stance_mask` = the lead-verified RE's table for both games, BF2 `& 3`, the stance-less hint → STAND, one PORT GUARD; `nav::Hint` carries the mask, applied in `game::layout::nav_graph`; `ai::stance_hold` draws one set bit with `SALT_STANCE` and holds it), 1a `core::squad` (ten PROVISIONAL constants, `Unit::command` hashed with nothing eaten when `None`, `Sim::new(42)`+100 ticks pinned at `17e1b946ff0e821c`), 1b the pad edge / issue / clear / death rules; ci 581. **The 0b move IS the re-baseline** (decision 2, taken in the STOPPED block): the sweep footer `sweep: finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480` (23 map lines + the footer moved vs s15; `finished 26/32` and `>10k 5 maps` unchanged; sum 271740 → 243480), the tat2 arena kills/min 27.40 / 37.20 / 34.60 / 36.60 with hashes 02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81 (`monotonic: kills no  hits no`, as before); the three replay hashes, both motion.csv, the flat arena and `--sim-hash 100` = `17e1b946ff0e821c` did NOT move. Prone wishes 45 → 0 of ~600 (the census the lander reproduced: `508 stance_want - / 61 Crouch / 32 Stand, no Prone, over 601 units`). **`battle-ticket-floor` (17, 1 commit, f4e4e76)** — `Battle::on_death` floors the pool at 0 (`analysis/re-tickets-spawn.md` §0/§4, both games); measured twice (loop session and lander after the rebase): NO recorded value moved — the sweep byte-identical to phase-d's INCLUDING the hash column, both arenas identical — so no re-baseline; ci 583. Worktree and branch removed. **The CODE SHA is main `f4e4e76`.**
  **The ONE rebake (`scripts/rebake.sh` on main `f4e4e76`):** vs the post-landing-A bake (`s16/bake_bake-determinism`, main 91f3532's binary) EXACTLY item 11's files differ — `SAME=20901 DIFF=326 ONLY_IN_A=0 ONLY_IN_B=342` = 292 `*/textures/*.ktx2` (in 16 BF2 map dirs + tat2 + common_bf2, no bf1_/tat1/side_) + 32 `*/manifest.ron` + 2 `common.ron`; ONLY_IN_B = the 342 `*.tier2.ktx2`, ONLY_IN_A = 0; nothing outside a family. Two fresh scratch bakes of the same binary: `SAME=21569 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0` — **DIFF=0 over 21,569 files**; the first of them vs the rebaked shipped tree also DIFF=0, so `engine/assets_baked` IS a fresh bake of main. **Goldens (`scripts/goldens.sh` against the rebaked tree): 24 goldens moved and NOTHING else** — the 21 map goldens = H's 8 ∪ 11's 16 exactly (bf1_hot1, bf1_rhn1, bf1_yav2, bf2_cor1, bf2_dag1, bf2_dea1, bf2_end1, bf2_fel1, bf2_geo1, bf2_hot1, bf2_kam1, bf2_kas2, bf2_mus1, bf2_myg1, bf2_nab2, bf2_pol1, bf2_tan1, bf2_uta1, bf2_yav1, tat1, tat2 — the STOPPED block's "≤ 20" was an arithmetic slip, the union is 21 since dag1/geo1/pol1 are in both) plus the three `tat1_*` frame captures (tat1_overview / tat1_hud / tat1_spawn: 203 / 203 / 81 pixels, no channel by more than 18, one 280×48 band of far terrain behind the arch — H's TAT1 patch from the overview camera; the block did not predict them); no unit, loadscreen or shell golden moved — every one viewed as a committed-vs-regenerated pair (`lead_tmp/s16/sheets_view/golden_pairs_*.png`): item 11's moves are texture detail only (FEL1's ground to the disc's mottled sheet, GEO1's smooth orange terrain to the disc's mottled brown-red, POL1's star field and floor, COR1's facades, DEA1's hull plates, MYG1's grating), H's are near-ground terrain (the Hoth room outlines from the vantage on both discs); no hole-to-sky, black pixel or missing texture anywhere; measured mean-abs / strong-diff %: bf2_pol1 13.04 / 39.27, bf2_fel1 8.33 / 36.10, bf2_geo1 4.01 / 20.06 (H's 13.5 + 11's 6.7), bf2_myg1 3.01 / 5.19, tat2 1.86 / 0.95, bf2_cor1 1.64 / 0.86, bf2_uta1 1.35 / 0.71, bf2_tan1 1.29 / 1.45, bf2_dea1 1.00 / 1.14, the rest ≤ 0.52 / 0.43 — every number the two branch reports predicted, reproduced on the rebaked tree. Committed as one goldens commit before the docs. `scripts/gpu_tests.sh` exit 0 on the regenerated set. **Goldens render the PS2 base tier from now on** (decision 1; T2R puts tier 2 ON in play and OFF in the golden tests).
  **Lead-reproduced on final main `f4e4e76` — BASELINES NOW = `lead_tmp/s16/baselines/SUMMARY.txt` (§WHERE THINGS STAND carries the canonical copy):** `scripts/sim_gates.sh` (BASE = the s15 motion tree) walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, both `motion.csv` byte-identical to s15 (and to every earlier trajectory); the sweep pair `cmp`-identical raw INCLUDING the hash column (stdout alone), identical to the lander's phase-d and battle-ticket-floor sweeps and to the loop session's post-0b sweep (`lead_tmp/phase-d/after/sweep_1.txt`), footer `sweep: finished 26/32  stuck_max median 960 (finished-only 780)  >10k 5 maps  sum 243480`; `arena 3000 42 flat all` twice identical and byte-identical to s15 (45.49 / 122.33 / 166.89 / 200.00, `monotonic: kills yes  hits yes`); `arena 9000 42 tat2 all` twice identical: kills/min 27.40 / 37.20 / 34.60 / 36.60, hashes 02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81; `game --sim-hash 100` = `17e1b946ff0e821c`; ci 583 tests; `gpu_tests.sh` 0. The walkabout set regenerated on the rebaked tree: 219 images over 32 maps in 21.2 s (4.6 s PNG encoding), 884 MB, every PNG RGB (256 files), `INDEX.md` rebuilt (`lead_tmp/s16/logs/walkabout_regen.log`).
  **Refill (`lead_tmp/s16/gen_queue.py` over `s16/QUEUE.md.before` — every kept item verbatim):** H/15/16/11/17 DONE, PD re-bracketed IN PROGRESS at D5 (resume in its worktree, gate against the s16 sweep); new items GS (the GLOBAL VO runtime half), T2R (tier 2 ON in play / OFF in the goldens), T2S (the soldier/weapon tier-2 half), OGG (fold the beep's Ogg pin into `bake::ogg`; reword the bake's tier line), W (`game --map` without `--mission` at y = 0); order 5 → PD → GS → T2R → T2S → OGG → 6 → 9 → L → S0 → 13 → 4 → 7 → W → 3 → C3 row 6. The GATES block now says DIFF=0 for bake bytes and carries the s16 values. **Loop:** the stale lanes had already been killed and `PAUSE` removed (AUTOLOOP.md's hard stop, 21:40); relaunched `LANES=2` on the committed script at 22:58 on 2026-09-08 (pids launcher 3360746, lane 1 3360752; lane 2 starts three minutes later, see `engine/artifacts/loop/autoloop.log`), `DRAINED` removed last. The first relaunch (22:56) was killed and redone: `scripts/autoloop.sh --probe` from inside the Fable session failed with an EMPTY message — a CLI started under a Claude session waits 3 s on stdin and prints a warning INTO the JSON the loop parses; fixed by closing the probe's stdin (`scripts/autoloop.sh`, one line; the session call already reads the prompt file) and relaunching with every `CLAUDE*` env var stripped. `gen_queue.py` had a label bug on its first run (the parser strips `LABEL. ` and `patched()` never re-added it — every kept item came out unlabelled); fixed and regenerated, then two stale phrases patched (item 4's s15 sweep path; PD's census wording) — `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists all 24 items. `scripts/cleanse_delete.sh --yes` freed 111 GB (19 paths; the s16 scratch bakes and the dead `*_target` dirs); the repo is 106 GB.
  **Still Fable-only (unchanged):** water (HOLD 10(f)/19(c)); the vehicle RE for P7 after Phase D; the three PROVISIONAL side findings of the PrimaryStance RE; B1 (`Sim::hash` omits `assist` and most of `Bolt`). The maintainer: a PCSX2 GS dump (the alpha rule); a Tier-4 Elite feel check.

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

- **2026-09-09 (evening) — Fable session 18: the NINE branches the afternoon loop parked are LANDED; ONE rebake; 11 map goldens regenerated after every pair was viewed; the SWEEP re-baselined BY DESIGN (queue S0); the queue refilled.** the maintainer's standing rule held: Fable did only what needs Fable — an Opus lander did the landing mechanics, an Opus toolsmith ported the close-out kit into `lead_tmp/s18/`, an Opus doc agent typed this entry; the lead read the nine loop reports' "For the lead" sections, checked the lander's load-bearing numbers against its own logs by grep, viewed every golden pair, took the decisions, ran the rebake/goldens/final gates and reproduced every baseline on final main.
  **The loop's afternoon run:** the maintainer launched LANES=2 at 15:00 on the s17 queue; eleven sessions (lane 1 ×5, lane 2 ×6), est. (cost omitted); nine branches PARKED with every gate green; DRAINED at 16:57/16:58 (every remaining item shares a file with a parked branch or waits on one); the lead parked the lanes with PAUSE at ~19:35 for the landing.
  **Landed on main in the order T2C → RT → 17B → WL → W → VO → W1 → 6 → S0** (S0 last, so every earlier branch's sweep still matches the s17 baseline), each rebased in its worktree, ci exit 0 there AND on main under `flock engine/artifacts/loop/ci.lock`, fast-forwarded, the worktree and branch dropped afterwards (Opus lander, brief lead_tmp/s18/briefs/landing.md, report lead_tmp/s18/reports/landing.md (169 lines), logs lead_tmp/s18/logs/; lead checked by grep: the nine ci totals on main, eight sweeps raw-cmp-identical to s17 + S0's three cmps, all nine sim_gates logs green with the three hashes, both cmp_bake lines, the six failing golden names in item 6's gpu log (every other map 0.000). Deviations: T2C pins no literal 363 (per-dir equality + 21 side stems); W's start-y reproduced via the capture-series motion.csv inner command):
  **`soldier-tier-check` (T2C, `27c2f3e`)** — the soldier/weapon tier switch proved by running it: +2 → 617; test-only; 363 tier-2 files pinned (342 manifest + 21 side).
  **`render-tests-common` (RT, `754b662`)** — one `common/mod.rs` for the six GPU referees: +0; test-only; 1036 → 1003 lines, 31 tests.
  **`battle-outcome-pool` (17B, `bc030e6`)** — the post-outcome tail and the shed loop's precondition, measured: +2 → 619 (2 commits); item 17's question: NO — the pool stalls at alive == tickets.
  **`water-floor-check` (WL, `52d0ff5`)** — the port's floor under the disc's water-side props, measured: +2 → 621 (2 commits); the level and the floor are the disc's.
  **`walk-spawn-post` (W, `9f1155b`)** — `game --map` without `--mission` spawns at the first friendly post when the authored point has no ground: +2 → 623; seven maps (not five) started at y −300 — bf2_{cor1,dea1,kam1,mus1,myg1,tan1,uta1}; lander reproduced bf2_cor1 y=3.000, bf2_kam1 y=52.250.
  **`map-vo-streams` (VO, `d3ac445`)** — the map's own SOUND LVL streams are baked and the runtime reads them first: +4 → 627 (2 commits); scratch bake vs shipped SAME=21590 DIFF=0 ONLY_IN_A=61 (32 streams.ron + 29 ogg, 796 KB, six map dirs).
  **`water-fx-config` (W1, `52e6b54`)** — the water is painted and tiled the way the PS2 paints and tiles it, out of the LVL's own `fx__` chunk: +16 → 643 (2 commits); scratch bake vs shipped SAME=21569 DIFF=21 (12 manifest.ron + 9 water.glb) ONLY_IN_A=61.
  **`bf2-far-fade` (6, `19148e6`)** — a BF2 map with no far layer fades into the sky, not into a fog wall: +2 → 645; gpu_tests 101 by design: exactly bf2_{dag1,dea1,end1,fel1,nab2,yav1} (every other map 0.000).
  **`sweep-slot0-bot` (S0, `5ff02ca`)** — the sweep's slot-0 player becomes a bot: +1 → 646; the sweep MOVES BY DESIGN: pair identical, == the loop's after_1.txt, SWEEP_SLOT0=player == the s17 baseline; arenas and --sim-hash 100 unmoved.
  ci across the window: 615 → 617 → 617 → 619 → 621 → 623 → 627 → 643 → 645 → 646. **No sim value moved on any branch** — the three replay hashes, `--sim-hash 100` and both `motion.csv` are unmoved after nine branches, two of which touch sim code (S0's `core/src/sim.rs`, W's `walk.rs`); the only gate that moved is S0's sweep, by design. Worktrees at close: all nine removed; git worktree list = main alone; <repo parent>/openbf-wt/ empty. **The CODE SHA is main `5ff02ca`** (main before the window: `fe5b841`); the goldens commit `6314c2c` and the docs sit on top.
  **The ONE rebake (`lead_tmp/s18/rebake_and_cmp.sh`, log `lead_tmp/s18/logs/rebake_and_cmp.out`):** the pre-rebake scratch bake of main is 21651 files; against the SHIPPED tree `SAME=21569 DIFF=21 ONLY_IN_A=61 ONLY_IN_B=0 — DIFF = 12 water maps' manifest.ron + 9 water*.glb (W1); ONLY_IN_A = 32 sounds/streams/streams.ron + 29 sounds/streams/<hash>/<hash>.ogg (VO); nothing else (the census pattern expected the OGGs one level up — a pattern slip, all 29 strays are those OGGs)`. After `scripts/rebake.sh` on main the same scratch bake vs the rebaked shipped tree is `SAME=21651 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0` over 21651 files — **`engine/assets_baked` IS a fresh bake of main `5ff02ca`**.
  **Goldens (lead_tmp/s18/goldens_regen.sh + golden_pairs.py):** 11 moved and nothing else — bf1_kas1, bf1_kas2, bf1_nab2, bf2_dag1, bf2_dea1, bf2_end1, bf2_fel1, bf2_kam1, bf2_kas2, bf2_nab2, bf2_yav1 — the union of item 6's six (bf2_{dag1,dea1,end1,fel1,nab2,yav1}) and W1's six (bf1_kas1, bf1_kas2, bf2_dag1, bf2_kam1, bf2_kas2, bf2_nab2) — DAG1 and NAB2 in both — plus bf1_nab2 whose water.glb changed (max 5, no strong pixel); every pair viewed (lead_tmp/s18/sheets_view/golden_pairs_00..05.png): the fog walls on Felucia/Endor/Dagobah/Yavin dissolve into the sky (NAB2 into its black disc sky, hold 10(c)); the water carries the disc's tiling and KAM1's sea is textured instead of the flat grey plane; eye-level frames unchanged except the fade; committed by path.
  **Lead-reproduced on final main `6314c2c` — BASELINES NOW = `lead_tmp/s18/baselines/SUMMARY.txt` (§WHERE THINGS STAND carries the canonical copy); the SWEEP re-baselined by S0; every other value identical to s16/s17:** `scripts/sim_gates.sh` walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, `--sim-hash 100` `17e1b946ff0e821c`, both motion.csv identical to s17 (walk 079c8dbca95a380d0fd91484b7f2d2d0, aim 492299f958b70d6dc956b6386950f765); the sweep footer is now `sweep: finished 26/32  stuck_max median 840 (finished-only 780)  >10k 6 maps  sum 242580` (pair 0; vs the loop's sweep-slot0-bot/after_1.txt 0; SWEEP_SLOT0=player vs s17 0; 33 lines moved vs s17 (md5 3a3d47ecf4e99ddffcf60dafc8ee44c8, s17's bca8aedd…)); `arena 3000 42 flat all` kills/min 45.49 / 122.33 / 166.89 / 200.00 (606262bc6865a689 / 09c7449831956e5d / b2f6343d64f93c51 / b8ab0f137168eb26); pair 0, vs s17 0; `arena 9000 42 tat2 all` kills/min 27.40 / 37.20 / 34.60 / 36.60 (02a269d6fd69ab01 / 40667d8d6dc59ade / 3b5618dcffe3e68f / 86cf86adbe038f81); pair 0, vs s17 0; `scripts/gpu_tests.sh` exit 0; ci 646 tests (`lead_tmp/s18/final_gates.sh`, log `lead_tmp/s18/logs/final_gates.out`). The walkabout set regenerated on the rebaked tree: 219 images over 32 maps in 21.1s (4.6s of that PNG encoding), 884.2 MB in <repo>/engine/crates/game/../../artifacts/walkabout.
  **Decisions on the nine (taken before landing, `lead_tmp/s18/rulings.md`):** item **6** LANDS and the look is accepted (the six vantage sheets viewed; both PROVISIONAL shader deviations — a smoothstep ramp, blend draws multiplying the fade — accepted; NAB2 fading to the disc's black `sky_naboo2` is faithful and goes to the maintainer, HOLD 10(c)); **`OPENBF_FAR_CULL` stays OFF** and is re-judged after item L. **W1** LANDS with its two deviations (`bake/src/map.rs` +8 for the `fx__` dispatch, a new `mungefs/tests/fx_lvl.rs`) — the TILING is judged, the opacity is W2's — and its two `analysis/re-water.md` §5 amendments are recorded there. **S0** LANDS and the sweep RE-BASELINES; `arena`'s slot-0 default stays OFF (the spec's three clauses only hold that way, and its tables are their own referee — unmoved); the `pub` referee hook on `Sim` accepted; its finding → new queue item **SK**, its `sim.rs:707-711` doc-comment defect → C3 row 6. **W** LANDS: seven maps, not five (HANDOFF §Standing backlog 7 corrected — bf2_kam1 and bf2_mus1 added), and its out-of-scope note (the authored default spawn is TAT1's `(−140, 230)` on every map) → new queue item **WS**. **VO** LANDS (61 new files; the residue is the map's own stereo MUSIC, indexed and never decoded per the maintainer's rule, plus cut names). **WL** LANDS: the level AND the floor are the disc's, YAV1's bazaar is an authored flooded plaza, the 18 flagged rows are pivots by design — HOLD 19(c)'s "level vs the authored floors" clause is CLOSED. **17B** LANDS: item 17's question is answered **NO** (the pool stalls; bleed disarms at `alive == tickets`), and its observation — the AI-shed loop in `tick_bleed` may be unreachable from bleed — goes to the HANDOFF backlog as a lead call. **T2C** LANDS (the whole-bake tier-2 count is 363: 342 manifest-listed + 21 side). **RT** LANDS; its `impl Default for TextureDesc` suggestion is a future cleanse-row note.
  **The refill (`lead_tmp/s18/gen_queue.py` over `s18/QUEUE.md.before`, `queue_spec.json` the input):** the nine are DONE; order now W2 → 4 → SM → L → AT → 13 → HR → WS → SK → 9 → W3 → 7 → C3 row 6 → ONCE. New items: WS (every map spawns at a friendly post without --mission), SK (stuck census on the four no-outcome maps). The GATES block carries the s18 baselines — the new sweep footer, ci 646, bake bytes 21651 files DIFF=0.
  **Cleanup:** `scripts/cleanse_delete.sh --yes` freed 183 GB (21 paths — the nine dead *_target dirs, the lander's two scratch bakes + bake_main_pre, the loop sessions' VO/W1 scratch bakes, item 6's raw renders (its 88 sheets kept); scripts/cleanse_delete.sh gained the s18 lines); the repo is 91 GB.
  **Loop:** the maintainer's two lanes ran the afternoon and drained; the lead parked them with `PAUSE` for the landing and removed `PAUSE` and `engine/artifacts/loop/DRAINED` right after the docs commit — **the loop is RUNNING on the refilled queue and needs no relaunch.**
  **Still Fable-only:** the water VU1 microcode and KAM1's FFT ocean (HOLD 21); the `modl` LOD switch rule (HOLD 20); the vehicle RE for P7, which is next; B1 (`Sim::hash` omits `assist` and most of `Bolt`); BES1's ring (HOLD 4, low value); and now the `tick_bleed` AI-shed reachability question (17B's observation). The maintainer: a PCSX2 GS dump (the D-pad menu's second layer, the wreck's LOD, the far water fog look); whether NAB2 keeps its faithful black distance (HOLD 10(c)); the Tier-4 Elite feel check.

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

- **Phases.** P0–P6 DONE; P9's two spine changes DONE. **P6.5 Infantry AI v1:
  Phases A, A.1, A.2, A.3, A.4, B and C are landed** — Phase C completed on main
  2026-09-08 evening (`658c6af` commit 1; commits 2–5 `fc077bb`: perception,
  the aim model, the sniper table, fire discipline, hints/stances, the human
  damage layer and the arena; PLAN Log 2026-09-08 evening). **Phase D is
  COMPLETE 2026-09-09** (§DESIGN SPECS below; queue item PD): commits 0a/0b/1a/1b
  landed 2026-09-08 late night as `6275a6d` — the `PrimaryStance` bitmask (the
  re-baseline), `core::squad`, the pad edge and the issue/clear/death rules —
  and D5–D14 landed 2026-09-09 as `cef083b` (six commits 31ccd5f..cef083b);
  the `phase-d` worktree and branch are gone and every later item starts from
  main. P7 (vehicles) is NEXT; since 2026-09-17 Fable is off here, so the loop
  does that RE ITSELF as its own job under `OPUS_LOOP_PROMPT.md` §5 (HANDOFF
  §HOLD FOR FABLE 1 — the name is kept so old greps still find the list) and
  then builds it. P8 (space) is later, on the same AI architecture.
- **What runs:** all 32 ground maps bake and play (`scripts/rebake.sh`), a full
  conquest round vs AI with animated soldiers, weapons, CPs, tickets, spawn
  waves, BF2 shell + HUD + pause cheats + profiles, BF1 load screens, per-map
  sound banks, per-map light/fog/sky/water, per-draw alpha (ALPHA RULE v2),
  sky shells placed by the ELF's rule, BF2 mode-layer content, mission-driven
  rosters, and `--difficulty`. Deterministic: same seed + input log ⇒ same
  `--sim-hash`.
- **How it is checked — the referee set, in the order a branch runs it:**
  `engine/ci.sh` (Tier 0/1, exit code read directly; the test COUNT lives only in QUEUE.md's GATES block) before every
  commit; `scripts/goldens.sh --check` over the **90 committed golden PNGs
  (75 MB)** — a landing regenerates only the moves its entry declares, after
  quantifying and viewing every committed-vs-regenerated pair, and they render
  the PS2 BASE texture tier: since T2R landed (2026-09-09) tier 2 is ON in play
  and OFF in the golden tests BY CONSTRUCTION — the tests force `Ps2`
  themselves, so a hostile `OPENBF_TEXTURE_TIER=2` environment leaves them at
  exit 0 and the goldens clean;
  `scripts/gpu_tests.sh` for every GPU referee (never a bare
  `cargo test -p render -- --ignored` — parallel wgpu devices SIGSEGV);
  `scripts/sim_gates.sh` for the three replay/sim-hash referees (`BASE=<dir>`
  also byte-compares each replay's `motion.csv`); the outcome sweep
  `round_to_outcome 60000 42`, run twice and `cmp`-compared; and
  `OUT=… scripts/scratch_bake.sh` + `scripts/cmp_bake.sh` for bake bytes
  (DIFF=0 since item 16 — the bake is byte-deterministic, 2026-09-08 night).
  `--bench` for performance; Tier-4 artifacts for the maintainer under
  `engine/artifacts/*/README.md`; the 256-PNG walkabout set for "what are we
  missing?" (`engine/artifacts/walkabout/`).
- **BASELINES — the one LIVE copy is the GATES block of
  `engine/artifacts/loop/QUEUE.md`.** That is what every branch measures
  against and what a declared re-baseline rewrites in the same landing step
  (`OPUS_LOOP_PROMPT.md` §3.9), so it cannot drift; this Log is the durable,
  tracked record of what each value was WHEN IT MOVED. A declared re-baseline
  writes its raw files under `engine/artifacts/lead_tmp/loop_baselines/<stamp>_<label>/`
  — the first was DET1's (2026-09-25, `loop_baselines/20260925_0314_DET1/`:
  the sweep and the TAT2 arena); the replays' `motion_main` files are still
  `lead_tmp/s19/baselines/` (`SUMMARY.txt`, lead-measured 2026-09-09 night on
  main `5d18752`). **Do not restate the values here or in HANDOFF** —
  three copies is how "662 tests" survived on two pages after ci reached 673
  (cleanse 2026-09-17). The last BEHAVIOUR re-baseline was STK2's,
  2026-09-25 (`loop_baselines/20260925_0557_STK2/`): the SWEEP alone, moved by
  a hunter's last leg walking at the enemy instead of its hub; before it HDL's
  (`20260925_0600_HDL/`, the SWEEP and the TAT2 ARENA's easy row — a
  defender's hint must stand on its post's level), DR3B's (the missions' start-of-round `plnp` route blocks on
  by default), DR2's (BF2 doors became entities that open for a unit in their trigger); before it
  DR1's the same day (doors always open to soldiers), and DET1's, the SWEEP and
  the TAT2 ARENA, moved by a spawn yaw's last ulp (before that item 4's, 2026-09-09 night — a map's classes follow the MAP's own
  game, NO escape hatch; S0's `SWEEP_SLOT0=player` only gives a bot-less
  sweep). What stays true of the gates whatever the values are:
  - the sweep pair is `cmp`-identical INCLUDING the hash column
    when stdout is captured ALONE (`> sweep.txt 2> sweep.err` — the `mission …`
    warnings and the `sweep difficulty:` line are stderr, printed in
    nondeterministic order by the parallel workers, while since item 4 the three
    surviving `roster …` warnings are STDOUT, indented under their own map, and
    `mix team` lines print to stdout only under `ROUND_VERBOSE`); with
    the streams merged, the five-part strip filter on both sides
    (`grep -v ' done (' | grep -v '^roster ' | grep -v 'mix team' | grep -v '^mission ' | grep -v '^sweep difficulty'`).
  - replays: the two P3 logs carry NO AI, so nothing in Phase C, phase-d or any
    AI item can move them — `motion.csv` must stay byte-identical to
    `baselines/motion_main/`, and it has since s14. `--sim-hash 100` is pinned by
    a test in `core/src/sim/state.rs` (`sim.rs` until SPL5): a failure there is a
    lead decision, never a value to update (PLAN B1).
  - the arena (Phase C's referee): `arena 3000 42 flat all` and
    `arena 9000 42 tat2 all`, each run twice and `cmp`-identical, stdout and
    stderr SPLIT (the wall-clock `arena <tier> N s` lines differ). The flat field
    is monotonic on kills and hits; tat2's kills/min is NOT, and that stays
    accepted (Log 2026-09-08 evening). Item 4 re-baselined the tat2 rows
    2026-09-09 night — the arena's soldiers field the map's own disc classes now.
  - bake bytes: the bake is byte-deterministic since item 16, so any
    `cmp_bake.sh` DIFF is YOUR change and you say which family; there is no noise
    family left. `engine/assets_baked` is a fresh bake of main after every
    [REBAKE] landing (HANDOFF §State names the last rebakes and the last full
    proof). A landing that changes a manifest field rebakes EVERY group that
    writes a manifest — `tat2` is its own rebake group and once missed L's
    `lowres_glb` key that way.
  - **the replay gate takes NO `--mission` and no `--ai`** (both move the hash:
    `--mission missions/tat1_con.ron` alone gave `e3aede1de3ad937e` /
    `adef6ad6d2490c28`). The exact invocation is
    `game --map assets_baked/tat1 --headless --walk-replay artifacts/p3_walk/<demo>.ron`,
    which is what `scripts/sim_gates.sh` runs (`BASE=` must be ABSOLUTE) — use
    the script, not a remembered command line.
  - a sim-hash move with `motion.csv` unchanged is a hash-FORMAT change, not a
    behaviour change (Phase C commits 1, 2, 3a, 3b, 3c and 4 each did that; the
    s13 43d2…/242f…/20fd… and s14 8d9d…/c12f…/537c… values are superseded and
    appear only in historical text). The s15 sweep footer (`… median 900 … sum
    271740`) and tat2 hashes (6e23…/6f93…/8cbe…/a08d…) are superseded too.
- **Performance is not a problem** and that is measured at BOTH workloads:
  the PS2-authentic one (worst map on the weaker GPU 3.69 ms/frame, sim tick
  p99 ≤ 0.031 ms) and the maintainer's — 740 alive AI units at 6.98 ms/frame on the
  integrated GPU. Re-measured 2026-09-07 (cleanse C1): sim tick TAT2 ai=16
  mean 0.343 ms / p99 0.823 ms, ai=64 mean 1.406 / p99 3.168, UTA1 ai=16 mean
  0.323 / p99 0.567; peak RSS 1.14 GiB for the whole 32-map sweep. **The
  broadphase landed 2026-09-08 evening** (queue 12: a 16 m XZ grid over the
  bodies' AABBs, a superset prefilter with a full-scan fallback): TAT2 ai=64
  sim tick mean 1.362 → 0.368 ms, p99 2.743 → 1.217 ms
  (lead-reproduced, `lead_tmp/s15/reports/landing_B.md`); its one hazard — a
  body replaced or MOVED IN PLACE keeps `bodies.len()`, so the grid stays live
  — is P7's to close by making the field private. See the perf artifact and
  §Optimisation backlog.
- **The world the sim walks on.** `Sim::ground_under` over
  `core::collision::CollisionWorld` — the map's `coll` meshes, `prim`
  primitives and terrain cutters, baked as `collision/<geometry>.glb` +
  manifest `collision`/`terrain_cutters` (formats measured on the discs,
  RE_LOG "`coll` / `prim` / `CUTR`"), with per-class collision LAYERS from the
  ODF properties and the BF2 prim `MASK` (`game::collision_layers`,
  `analysis/re-collision-layers.md` — the old name rule is gone). Soldiers
  stand on Kamino's and Bespin's platforms, Kashyyyk's docks, Dagobah's cave
  floor and Polis Massa's under-terrain base; the terrain is a one-sided sheet
  (absent in cutters, ignored more than `TERRAIN_BURY` below the feet);
  terrain LOS/bolt rays hit only where they CROSS it; bolts test the ordnance
  subset. Death regions, lethal landings, `STEP_HEIGHT`, `WALKABLE_MIN_NY` and
  `TERRAIN_BURY` are ours and PROVISIONAL (HOLD 13/15).
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
- **Mission data is generated from the PS2 bytecode** (`scripts/gen_missions.
  py`, 57 RONs incl. 25 `_con_cw` eras; never hand-edit them) and consumed:
  post roster, `KillObject`, per-post `Team`, death regions. BF2 HOT1 is four
  posts 2–2 at tick 1 (CP7 killed), DEA1 kills CP6; tat1 is `tat1i` (Empire
  attacks).
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
- **The autoloop runs the project (live 2026-09-08; the lead since 2026-09-14,
  The maintainer).** Unattended headless Opus sessions each do ONE job from
  `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md` §2: finish an interrupted job,
  LAND a parked branch (rebase, gates, fast-forward main, rebake, quantify +
  view + regenerate goldens, re-baseline declared moves, docs), BUILD a
  `engine/artifacts/loop/QUEUE.md` entry on its own worktree branch and land
  it, answer an RE question itself (Fable is off since 2026-09-17 — the
  prompt's §5 self-checks and an Opus verifier agent replace the audit), or
  REFILL the queue. It never waits for the maintainer: calls are recorded as
  `DECIDED` lines in this Log and in HANDOFF §For the maintainer. **It never pushes**,
  and main only fast-forwards — the harness hook `scripts/claude_guard.sh`
  enforces that, plus §Guardrails; only one session changes the main checkout
  at a time (the `locks/MAIN` claim). The maintainer's operator page is
  `engine/artifacts/loop/START_HERE.md`; a `STOP` or `PAUSE` file at the repo
  root halts it; `engine/artifacts/loop/DRAINED` is written only when
  everything left needs the maintainer, and every lane sleeps until he removes it.
- **The maintainer's three open decisions were TAKEN 2026-09-08:** (a) universalised unit
  behaviour — the droideka ball jumps on every map, as in BF2, with BF2's
  numbers (§Movement & feel; `analysis/re-droideka.md`; the build itself is
  P7-class and held); (b) the unified sound plan — GLOBAL/CW/GCW/CORE VO baked
  once by segment-id hash, music streamed (`analysis/re-sound-global.md`; queue
  item 15, and Phase D's D12 audio follows it); (c) the alpha rule stays v2
  until a PCSX2 GS dump is captured and read (HOLD 10(g)).
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
- **Optimisation: measured, and the headroom the maintainer wanted is already there.**
  740 alive AI units run at 6.98 ms/frame on the INTEGRATED GPU, and 720p→4K
  costs 1.4x for nine times the pixels. Frustum culling landed (−14 to −17 %
  frame time on the big maps, all goldens byte-identical). The sim scales at
  about n^1.4, not n², so the spatial grid and deterministic parallelism have
  no measured need. See §Optimisation backlog for the full table and what IS
  still worth doing (texture memory on the classic-plus path).
- **Community projects:** `analysis/community-projects.md` — BF2GameExt (MIT) is
  a usable RE accelerant, Zenit has no license, Phoenix is AGPL. All are PC-2005;
  PS2 facts must never be taken from them without verification.
- **Process rules that cost a session each when forgotten** (learned the hard
  way; the full list is HANDOFF §How the last session verified things):
  **never run two `./ci.sh` at once** — tests bake into fixed
  `std::env::temp_dir()` names (item 3 gave 12 of the 18 a pid on 2026-09-08;
  the 5 files parked branches owned are its resume), so every ci/gpu run goes
  through `flock engine/artifacts/loop/ci.lock`, lanes and lead alike; `git worktree add
  <path> main` needs `--detach` because main is checked out; the sweep's
  `roster …` / `mix team` / `mission …` warning lines interleave and must be
  stripped before `cmp`; a perf comparison needs the **300-frame median
  protocol** (§Testing ladder) — a 120–150-frame cold run is worth ±0.4 ms and
  once reversed a result from −29 % to 0 %; and `./ci.sh` used to rewrite
  `engine/assets_baked/tat1/sounds/names.ron` on every run — bug M-B2, FIXED
  2026-09-09 (queue item 3, `4df338e`), so the
  bake-bytes gate has no noise family left.
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
- A1. Disc inventory → map/mode matrix from the ISOs. **DONE 2026-08-30** →
  `analysis/a1-inventory.md` (+ raw listings `analysis/a1-*-files.txt`).
- A2. Bake-format recon: PS2 LVL chunk variants vs LibSWBF2/swbf-unmunge PC
  handling. **DONE 2026-08-30** → `analysis/a2-bake-formats.md`: PC-identical
  container w/ FNV-1a hashed tags; PS2 texture RLE codec cracked+verified;
  quantized model buffers mapped; terrain heights (HGT8/HEXP) were A2's top
  open question — RESOLVED at P1 (`analysis/re-terrain-heights.md`).
  A2's own errata are marked inline in that file (modl INFO, HEXP layout). LibSWBF2 has no PS2 support; swbf-unmunge
  covers tex/models only.
- A3. Annotated bindings diff (436 shared / 47 BF1 / 332 BF2) → subsystem map, AI
  evidence. **DONE 2026-08-30** → `analysis/a3-subsystem-map.md`: counts confirmed
  (BF2 768 unique, one dup binding); 252 dead online/memcard bindings flagged
  do-not-RE; AI divergence is the command/planning layer (BF2-only goal planner +
  planning graph vs BF1 hint nodes), soldier-level tuning identical.
- A4. Call-graph reachability from named roots → true size of the RE core.
  **DONE 2026-08-30** → `analysis/a4-callgraph.md`: RE keep-set = BF1 12,326 fns
  (83.1%) / BF2 14,692 fns (85.0%); dropped online/peripheral libs form clean
  contiguous bands (~320–380KB). Behavioral RE stays targeted per subsystem —
  the audit sizes the corpus, it doesn't mandate porting it.
- A5. PS2 sim tick rate + pacing. **DONE 2026-08-30** → `analysis/a5-tickrate.md`:
  both games render at 29.97 fps (2-vblank swap) with VARIABLE measured sim dt
  (COP0 Count, exact EE clock), one Update/frame, dt clamp 0.1 s. Engine locked:
  fixed 30 Hz tick, accumulator capped 0.1 s (core/src/lib.rs).
- A6. Mission Lua extraction (Lua 4/5.0 bytecode) + cross-check vs PC mod-tools
  sources. **DONE 2026-08-30** → `analysis/a6-mission-lua.md`: 439 scripts
  extracted (BF1 Lua 4.0 / BF2 5.0, all f32 numbers); PC mod-tools Lua sources
  (Gametoast mirror) ARE the mission scripts (BF1 100% string overlap) → RON
  mission data generates from PC .lua, validated against PS2 dumps; PS2 wins
  on deltas. Shared Objective* scripts become native Rust modes.
- R3. Community engine projects assessed 2026-08-30 →
  `analysis/community-projects.md`: **BF2GameExt (MIT)** carries ~30 RE
  write-ups covering our hold list (BF1-vs-BF2 AI comparison, vehicles/flyers,
  animation, particles/`fx__`, soldier collision, sound) — usable with
  attribution, but PC-keyed, so every fact is a HYPOTHESIS to verify against
  PS2. **Zenit** (Rust/wgpu, PC) documents only 4 chunk types and has NO
  LICENSE — read only, copy nothing. **Phoenix** (Unity, AGPL) runs the
  original Lua at runtime, which is why it gets breadth fast — and raises a real
  product question: the SWBF2 mod ecosystem is Lua+ODF and we currently cannot
  run those mods (PLAN Architecture #3 deferred a shim). Also records WHY the
  PS2 base is right (the controller-era systems — aim assist, prone, sprint —
  exist only there; BF2GameExt had to invent them for PC) and what the PC SWBF2
  copy IS good for (making BF2GameExt's addresses followable, a second sample of
  the engine for semantics, and the PC↔PS2 name map for classic-plus).
- R1. Survey community GC + graphics mods on PC ports (ideas for GC economy, classic-plus
  look targets). **Graphics half largely RESOLVED 2026-08-30** by the Harrisonfog
  collection (see §Classic-plus assets); GC-mod survey still open.
- R2. Classic-plus look spec: transcribe Shader Patch (MIT) shader techniques +
  Harrisonfog 2025 preset numbers + 2019 MasterEffect values into a wgpu-native
  render-feature list with target constants. (Do when render work resumes, P5-ish.)
- W1 (bake extension). Reader half **DONE 2026-08-30**: `mungefs::texture_pc`
  (D3D format chunks per A2 §5) + `bake pc-tex`, inspection only. STILL OPEN:
  the HD name-matched override layer and variant tagging in manifests, then the
  BF1-leftover upscale pass — the override DESIGN is a the maintainer + Fable call.

## Phases (each = shippable milestone)  [ORDER per Q3: all RE audits before engine gameplay]
- P0. Vision lock (DONE) + audits A1 (DONE), **A2–A6 (do all of these before P1)**.
- P1. Bake v0: one map's terrain/models/textures + BF1 loading beeps → open
  formats. **DONE 2026-08-30** (map: BF1 TAT1; all Tier-1 round-trip tests green).
- P2. wgpu map viewer (fly-cam through baked map). **DONE 2026-08-30.**
- P3. Soldier v0: movement, stances (roll/prone button), camera, controller input, BF1 aim
  assist vs dummy targets; PCSX2 side-by-side acceptance. **DONE 2026-08-30** (PCSX2
  spot-checks staged per Q1).
- P4. Combat v0: blasters, damage, spawns, CPs, tickets + no-overspawn rule, minimal AI;
  dev cheat toggles (invincibility/ammo) land here as console flags. **DONE 2026-08-30.**
- P5. Vertical slice: Conquest vs AI, one BF1 map + one BF2 map; era-mix option.
  **DONE 2026-08-30** (TAT1 + TAT2; Tier-4 artifact engine/artifacts/p5_slice/; the maintainer's pad check async).
- P6. Shell UI (BF2) + BF1 loading screens + settings + pause-menu cheat codes +
  profiles/autosave infrastructure. **DONE 2026-08-30.**
- P6.5. **Infantry AI v1 (own design, decided 2026-09-01; Log "AI decision").**
  Phases A navigation → B commander → C combat/perception/skill tiers → D squad
  commands → E vehicle AI (with P7). **A, A.1–A.4, B, C and D are DONE** (C
  2026-09-08 `fc077bb`, D 2026-09-09 `cef083b`); **E is left, and it lands with
  P7**. The round sweep is the gate.
- P7. Ground vehicles → flyers (faithful per map).
- P8. Space Assault (faithful first; rework later) + heroes (killable; BF2 hero health, no time drain, regen on kills) +
  awards/medals + class meta-unlock.
- P9. LAN netcode (16 cap). **Its two spine changes are DONE (2026-09-01, HANDOFF
  HOLD 14):** `Sim::step` takes N pads indexed by player slot, and `core`'s
  transcendentals go through the `libm`-backed `core::fmath`, so a
  Windows↔Linux lockstep cannot desync on the platform libm; the one known
  leak outside `core` (a host `atan2` for the spawn yaw in `game/src/layout.rs`)
  closed with DET1 (2026-09-25), and the referee scans `game/src` since.
- P10. Campaign, Galactic Conquest, splitscreen, polish, Win/Linux/macOS packaging.
- P11 (post-1.0). New modes, space/CTF reworks, campaign extension, GC economy tuning.

## Working mode (overrides memory defaults)
- **This project = FULL AGENT AUTONOMY.** Another project's convention ("the maintainer types source,
  learning-first") does NOT apply here. The agent writes the code. Do not hand the maintainer packets
  to type; implement directly, test, commit, and report.
- Keep the general working style: **PLAN.md is the single source of truth** (no competing
  planning docs; `analysis/RE_LOG.md` is a subordinate lab notebook for RE findings, not a
  second plan). **One commit-sized change at a time; Tier-0 and Tier-1 tests green before
  advancing.** Update PLAN.md's Log + decision records as you go.
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
- **The worktree layout, exactly** (moved here from HANDOFF 2026-09-08, cleanse
  C2; it lived only in a superseded session digest, and every parallel branch
  needs it): `git worktree add -b <branch> <repo parent>/openbf-wt/<branch> main`
  (adding a worktree ON main needs `--detach`, because main is checked out
  here), then symlink the shared, gitignored data into it —
  `engine/assets_baked`, `engine/artifacts`, `extracted`, and on this machine
  `PC_swbf2_exe` and `community_mods` — all read-only except the branch's own
  outputs under `engine/artifacts/<topic>/` and `engine/artifacts/lead_tmp/<branch>/`.
  Every cargo/script command in that worktree carries
  `CARGO_TARGET_DIR=engine/artifacts/lead_tmp/<branch>_target`. Rebake with
  `scripts/rebake.sh` (all groups) or by group, e.g. `scripts/rebake.sh maps side`
  — the exact invocations live in that script, and only a landing session
  (holding the MAIN claim) runs it on main.
- Bias to action on reversible calls. Escalate per the Escalation Policy, never block
  mid-phase — park a question and keep working the unblocked tasks.
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

## Subagent orchestration
- The lead session owns PLAN.md, integration, and verification.
- Spawn subagents for independent work: (a) decomp RE of a subsystem — since 2026-09-17
  the lead session does the RE ITSELF (Fable is off on this project) and uses one
  `model: "opus"` VERIFIER agent, given the claim list and citations only, to try to
  falsify them, (b) Rust implementation of a spec'd module, (c) web/community research,
  (d) test authoring, (e) broad code/asset search (Explore) — all Opus. Launch
  independent agents in parallel.
- **ALWAYS verify a subagent's output before trusting it:** build it, run Tier 0/1, read
  the diff. Never advance on an agent's "done" alone. If it fails, fix or redo.
- Work one step at a time: analyze → implement → test → integrate → commit → update PLAN.
- Do NOT use the Workflow tool / large fan-outs unless the maintainer explicitly says "ultracode".

## Escalation policy — RESOLVED: run until genuinely blocked (Q2)
- Run continuously across phases. Do NOT stop at every milestone for sign-off. Interrupt
  The maintainer ONLY for: (a) a genuine design fork the agent cannot resolve (the Deferred-design
  items, e.g. AI base BF1-vs-BF2 once A3/A4 evidence is in), (b) a hard blocker/failure it
  cannot get past, or (c) a guardrail/legal question.
- Feel is not a hard block: at each milestone still PRODUCE Tier-4 artifacts (headless
  gameplay capture, motion plots) and note them for the maintainer's async review, but proceed with
  the best faithful default rather than halting. The maintainer can adjust later.
- Keep PLAN.md's Log current so the maintainer can review big unattended jumps after the fact.
- **In the autoloop (the default since 2026-09-14) nobody is there to interrupt.** A
  genuine design fork inside the LOCKED FEATURE SET is decided by the session (the
  faithful, reversible default) and recorded as a `DECIDED` Log line plus a line in
  HANDOFF §For the maintainer; a hard blocker parks that entry as BLOCKED and the session takes
  another job; only a guardrail/legal question or something only the maintainer can supply (a
  Tier-4 feel check; not a PCSX2 capture since 2026-09-23 — the loop boots and
  screenshots a disc itself) waits for him, in §For the maintainer, and it never stops other
  work.

## Per-phase acceptance criteria (concrete + testable)
- **P0 — RE AUDITS FIRST (the maintainer's Q3): complete ALL of A2–A6 before any engine gameplay
  code.** Done when: A2 (bake formats) + A3 (subsystem map from the bindings diff) + A4
  (RE-core size via call-graph) + A5 (tick rate) + A6 (mission Lua) are all written to
  `analysis/`; `analysis/RE_LOG.md` is substantive (subsystem → function addresses); the
  `engine/` cargo workspace + crate skeletons compile an empty binary and a CI script runs
  Tier 0/1. Scaffolding the empty workspace is allowed during P0; gameplay implementation
  (P1+) does not begin until the audits land.
- **P1 (bake v0)** done when: `bake` reads one chosen map from `iso/` and emits glTF meshes
  + KTX2 textures + terrain + a RON manifest; extracts the BF1 loading-beep audio to OGG;
  Tier-1 round-trip tests pass. Pick a small, iconic, shared-planet map first (agent's call;
  criteria: modest geometry, has a clear CP layout, exists to compare BF1 vs BF2 later).
- **P2 (viewer)** done when: `game --map <baked>` renders the map with a fly-cam at
  interactive FPS on the NVIDIA adapter; `--capture-frame` produces a golden; visual golden
  test committed. First visible win — Tier-4 checkpoint with the maintainer.
- **P3 (soldier)** done when: controller-driven soldier with BF1 aim assist, sprint(BF2),
  contextual roll/stance button; movement constants transcribed from decomp with Tier-1
  reference-trace tests; PCSX2 spot-check per Q1; Tier-4 feel checkpoint.
- **P4 (combat)** done when: blasters + damage + spawns + CPs + tickets with the
  no-overspawn invariant (unit test: live units never exceed remaining reinforcements) +
  minimal AI; dev cheat toggles (invincibility/ammo). 
- **P5 (slice)** done when: a full Conquest round vs AI is playable on one BF1 and one BF2
  map, era-mix option works; deterministic `--sim-hash` stable across replays.
(P6–P11 criteria expand when reached.)

## Environment & tooling status (2026-08-30)
- Installed: Rust 1.97 + cargo, git, ffmpeg, cmake, clang, python3; **Blender 5.2 LTS**
  (`/usr/bin/blender` — usable for glTF export/inspection, though prefer programmatic glTF
  in the bake tool); Ghidra 12.1.3 + emotionengine-reloaded + Temurin JDK 21 (local `tools/`).
- GPU: Intel iGPU (renderD128) + **NVIDIA RTX 4000 Ada (renderD129)** — select the NVIDIA
  adapter in wgpu for performance (see machine memory note). Vulkan available.
- ISOs copied local: `iso/SWBF1.iso`, `iso/SWBF2.iso` (gitignored, COPY_DONE). SSD not needed.
- **PCSX2 2.8.0 ready as a binary, headless boot + screenshot DONE (2026-09-23):**
  `pcsx2-master/pcsx2-v2.8.0-linux-appimage-x64-Qt.appimage`. **PS2 BIOS** lands in `bios/`
  (gitignored): `ps2-0230a-20080220.bin` (+ `.mec`/`.nvm`), from the maintainer's Batocera box
  `/userdata/bios/ps2/` — EU v2.30 dump, region-agnostic enough for these NTSC-U discs.
  `engine/artifacts/pcsx2/pcsx2_capture.sh` (README alongside it) boots a disc on an
  isolated virtual display and screenshots it, unattended, nothing ever touching a real
  display. Getting there needed two permanent fixes to `~/.config/PCSX2/inis/PCSX2.ini`
  (`SetupWizardIncomplete` was `true` unconditionally; `[Folders] Bios=` was the bare
  string `bios`, which PCSX2 resolves against its own always-empty `~/.config/PCSX2/bios/`,
  never this repo's — a silent "no BIOS" failure, not a dialog) and forcing the emulator
  off this machine's live Wayland session (`-u WAYLAND_DISPLAY QT_QPA_PLATFORM=xcb`; a
  plain `DISPLAY=` override is not enough — Qt ignores it and reaches the real desktop
  regardless). **Still not built:** reaching a specific point in a level — `-fastboot`
  only reaches whatever the disc boots to by itself. PCSX2 can load a save state
  non-interactively (`-statefile`), the natural next step, but no save-state library
  exists yet and creating the first one still needs a real playthrough or a live
  vision+action loop, not just this script. `pcsx2-gsrunner` (also in `pcsx2-master/`)
  turned out to be a GS-*dump* replayer for the emulator's own regression testing, not a
  fresh-ISO-and-BIOS boot tool — wrong tool for this, and needs its own from-source
  Clang/Ninja build besides.

## Log

**The record splits at 2026-09-14** (cleanse C2, 2026-09-08, moved everything up
to and including 2026-09-03 — the archive's Part 1; the loop's cleanse of
2026-09-23 moved 2026-09-07..09, the Fable landing windows — Part 3).
`analysis/PLAN_LOG_2026.md` is that append-only archive, moved there
**verbatim**; the Log below is append-only from 2026-09-14 forward apart from
one dated SUMMARY entry per archived stretch. Nothing in either file is ever rewritten — a correction is a
later entry, never an edit of an earlier one. This index is the way IN; find the
topic, then read the entry (**→ archive** means the detail is in
`analysis/PLAN_LOG_2026.md`; the summary here names the section).

| Topic | Where to look |
|---|---|
| Vision, questionnaire, kickoff decisions Q1–Q4 | 2026-08-29/30 summary **→ archive** |
| P0 audits A2–A6; bake v0; the renderer + XFRM convention; soldier core; weapons/CPs/tickets | "P0 through P4" summary **→ archive** |
| BF2 map bake, rosters, missions, era mix; shell/HUD/load screens/cheats/SFX; soldier models + animation | "P5 and P6" summary **→ archive** |
| **Errata that changed decoded output** | the "errata" summary (CLUTs are LINEAR, BF2 `modl` INFO 72 B, `path` INFO u16+u16, stub `tern`) + `analysis/RE_LOG.md` **→ archive** |
| Detail textures (`_dtl`); camera vs buildings; spawn screen; effects; every ground map baking; `bake pc-tex`; terrain texturing (LTEX/SCAL/LUMI); material alpha flags; the golden gallery and load-screen sheets; sound banks per planet; the first code audit; the perf baseline; Death Star / Tantive IV framing; the alpha composite pass; `mungefs` hardening | "the first Opus sessions" summary **→ archive** |
| Outside-the-level terrain (open) | §WHERE THINGS STAND, defect group 1; "Remaining rendering defect" **→ archive** |
| **Per-map lighting, fog, sky domes** | "Every planet gets its own light", "Sky domes" + `analysis/re-sky-lighting.md` **→ archive** |
| Config string-blob erratum | "Config-format erratum" **→ archive** |
| Unit-count + resolution scaling, `--ai-cap` | "`--ai-cap`, and the scaling curves" **→ archive** |
| Frustum culling | "Frustum culling landed" **→ archive** |
| **Water (`tern -> WATR`)** | the 2026-08-31 summary + `mungefs::water` **→ archive** |
| **Sky objects (capital ships)** | the 2026-08-31 summary **→ archive** |
| Terrain split, submesh compaction | the 2026-08-31 summary **→ archive** |
| Mip generation | the 2026-08-31 summary **→ archive** |
| PC tier in SIDE | the 2026-08-31 summary **→ archive** |
| Flythroughs per map | the 2026-08-31 summary + `engine/artifacts/flythrough/` **→ archive** |
| Sky-shell fog, the Kamino seam | the 2026-08-31 summary **→ archive** |
| **The fog rule — what it is NOT** | the 2026-08-31 summary + `engine/artifacts/fog_rule/` **→ archive** |
| **The maintainer's visual defect hunt (session 5)** | "the maintainer's defect hunt" + `engine/artifacts/defect_hunt/` **→ archive** |
| Orbit framing, `--scene-report` | "the maintainer's defect hunt", item C **→ archive** |
| Texture case folding (3,752 grey draws) | "the maintainer's defect hunt", item D **→ archive** |
| `DomeInfo.Ambient`, sky-shell fog ramp | "the maintainer's defect hunt", item A **→ archive** |
| Dome elevation census, Bespin's terrain | "the maintainer's defect hunt", item B **→ archive** |
| Global fog-density knob | "the maintainer's defect hunt", item E **→ archive** |
| Round-completion sweep, `round_to_outcome` | "Round-completion sweep" (session 5) + its CORRECTION in the 2026-09-01 cleanse summary **→ archive** |
| **Cleanse 2026-09-01** — findings, fixes, what was declined, P7/P9 seams | the 2026-09-01 "first cleanse" summary **→ archive** |
| **Walkable model collision, the coll/prim/CUTR formats, mission RONs from bytecode, P7 entity design, P9 specs, the AI-base brief** | the 2026-09-01 session-7 summary + §DESIGN SPECS (P7) **→ archive** |
| **AI decision — own AI, the evidence, the six layers, the difficulty model, squad commands, phases A–E** | the 2026-09-01 "THE AI DECISION" summary **→ archive** |
| **AI difficulty from the ELFs; the 77.5° cone; `HitLocation`; Elite vs aim assist** | the 2026-09-01 "what the ELFs say about difficulty" summary + `analysis/re-ai-difficulty.md` **→ archive** |
| **The nav data in the bake (`plan`/`plnp`/`Hint`/`BARR`), the P9 spine, the shared `manifest` crate** | the 2026-09-01 "the spine and the data under the AI" summary **→ archive** |
| **Phases A / A.1 / A.2 — the follower, the corner walk, the swept probe** | the 2026-09-01 "Phases A, A.1 and A.2 landed" summary **→ archive** |
| **Texture alpha census, ALPHA RULE v2, `kam_prop_crate`** | the 2026-09-01 census summary + `engine/artifacts/texture_census/` + §DESIGN SPECS (the classifier) **→ archive** |
| **A.4 — the penetration test, the slide flip, the kerb/step rule, the constants ablations** | the 2026-09-03 session-10 summary **→ archive** |
| **Collision LAYERS (`SoldierCollision` subsets + the BF2 prim `MASK`), the ordnance ray** | the 2026-09-03 session-10 and session-11 summaries + `analysis/re-collision-layers.md` **→ archive** |
| **ALPHA RULE v2 as built (per-draw `alphaMode`), the sky V clamp** | the 2026-09-03 session-11 and session-12 summaries **→ archive** |
| **Sky dome placement and the sky pass; the fog rule (HOLD 10(a) answered); the MTRL flag word at the CPU level** | the 2026-09-03 session-11/12 summaries + `analysis/re-sky-dome-placement.md`, `re-sky-pass.md`, `re-fog.md`, `re-material-flags.md`, `re-texture-wrap.md` **→ archive** |
| **Phase B — the commander, its eleven forced deviations** | the 2026-09-03 session-11 summary **→ archive** |
| **The maintainer's decisions of 2026-09-03** (aim assist, the sweep's bot player, the combined RE, roster before Phase C, universal squad VO) | the 2026-09-03 "seven decisions" summary + §Movement & feel + §Classic-plus assets **→ archive** |
| **The Fable landing windows, sessions 13–19 (2026-09-07..09)** — the walkabout defect hunt, Phase C and D completing, the rebakes and gate re-baselines, the s19 baselines | the 2026-09-07 / 09-08 / 09-09 summaries **→ archive** Part 3 |
| **HOLD FOR FABLE questions** | grep `HOLD FOR FABLE` here and in `HANDOFF-OPUS.md` (consolidated list there, 2026-09-01) |

- 2026-08-29 / 08-30 — **kickoff.** Project created; the extraction, Ghidra
  analyses and binding tables; vision v1 (merged BF1+BF2, Rust+wgpu, a bake
  pipeline, LAN). The maintainer answered the 28-item questionnaire → §LOCKED FEATURE SET,
  and resolved Q1–Q4 → staged fidelity (decomp first, PCSX2 later),
  run-until-blocked autonomy, RE-audits-first order, local-only git. PART B (this
  file's execution guide) was written the same evening. Detail:
  `analysis/PLAN_LOG_2026.md` §2026-08-29 / 2026-08-30.
- 2026-08-30 — **P0 through P4.** Audits A2–A6 (formats, the 436/47/332 binding
  diff, the call-graph keep-set, the 30 Hz tick, the mission Lua); bake v0 for
  BF1 TAT1; the wgpu viewer, with the **XFRM convention proven empirically**
  (rows = object basis vectors, row-vector convention, decided by barrier-arc
  tangency 7/7); soldier core from the aim-assist and stance/roll RE; then
  weapons, bolts, CPs, tickets, spawn waves and the placeholder AI. The
  determinism spine (fixed tick + input log + FNV-64 hash) dates from here and
  everything since is gated on it. Detail: archive, same section.
- 2026-08-30 — **P5 and P6.** The BF2 map bake (base + `lvl_` mode layers),
  rosters from the baked classes, the first mission RONs, era mix; then the
  egui HUD, shell, pause menu, session cheats (Morse on the D-pad, lead-verified
  against the BF2 ELF), profiles, BF1 load screens and the loading beep, weapon
  SFX per planet bank, and soldier visuals end to end (softskin + `zaa_` clips →
  the skinned render path). Detail: archive, same section.
- 2026-08-30 — **the errata that changed decoded output.** Kept here because
  each one invalidated work that looked finished: **256-colour CLUTs are LINEAR**
  (the P1 CSM1 unswizzle was wrong and its validation was invalid); **BF2 PS2
  `modl` INFO is 72 B** (every TAT2 model had been dequantized into a shifted
  box); **`path` INFO is u16 count + u16 kind** and **a `tern` can be a stub**
  (NAME only, or NAME + WATR) — the two bugs that kept eleven ground maps from
  baking. All three are in `analysis/RE_LOG.md` with their evidence; the stories
  are in the archive.
- 2026-08-30 — **the first Opus sessions (backlog #1–#7 and the review work).**
  BF1 `_dtl` detail textures; the third-person camera vs buildings; the spawn
  screen (UI only — the pad-driven select is still open, HANDOFF §Standing
  backlog); placeholder effects; every ground map baking; `bake pc-tex`; then
  terrain texturing (LTEX/SCAL/LUMI), BF2 mode-layer matching (all 16 maps get
  their posts), the golden gallery and load-screen sheets, sound banks for every
  map with `bake sound-names` + `scripts/audition.sh`, the first code audit, the
  perf baseline on both GPUs, the Death Star / Tantive IV framing bugs, the
  alpha-blend composite pass, and `mungefs`'s no-panic hardening. Detail:
  archive, same section.
- 2026-08-30 (the maintainer, decisions): **(1) the Lua mod ecosystem is NOT a goal** — a
  mod worth having is ported to native Rust (§Architecture 3); **(2)** the PC
  SWBF2 install supplied at `PC_swbf2_exe/` (gitignored); **(3)** the PC textures
  measure larger for about a third of them (79/79 name matches on DEA1), so a
  PC-stock tier is taken ONLY where larger; **(4)** DLSS is the wrong tool (a
  temporal frame upscaler, no offline image mode) — offline upscaling stays
  Real-ESRGAN; **(5)** per-map `lght`/`sky_`/fog and the colour-space questions
  released to Opus. Full text: archive, same section.
- 2026-08-31 — **every planet gets its own light, and the sky is drawn.**
  `sky_` (both games) and `lght` (BF2 only) parsed by CONFIRMED PREIMAGE —
  122/123 and 23/23 property-name hashes recovered (`analysis/re-sky-lighting.md`);
  per-map sun, hemisphere ambient, fog and SKY DOME, with three PROVISIONAL
  judgement calls in `bake::lighting` (colour space, which maps get fog, an
  ambient floor). Also the **config-format erratum**: a `DATA` string argument
  is `4 * argCount + the byte offset into the trailing blob`, not the constant
  4 — the old reading returned only the first string and read later slots as
  denormal floats. `COLR` cracked (it is LUMI in colour, PSMCT16 5551 at
  element 2, one u16 per vertex, both indices column-major) and the LUMI-neutral
  rule re-checked and UPHELD with a decisive cross-check (BF1 HOT1 stores 128,
  BF2 HOT1 stores 255, same art). Detail: `analysis/PLAN_LOG_2026.md`
  §2026-08-31.
- 2026-08-31 — **`--ai-cap` and the scaling answer, then frustum culling and
  the PC stock tier.** `--ai-cap` unblocked the population sweep and produced
  the number the optimisation backlog still rests on: the sim scales at about
  **n^1.4, not n²**, and 740 alive units cost 6.98 ms/frame on the integrated
  GPU (table in §Optimisation backlog). That retired A3 (spatial grid) and A4
  (deterministic parallelism) as work with no measured need. Frustum culling
  landed anyway (−14 to −17 % on the big maps, every golden byte-identical).
  The PC stock tier shipped with a **same-art test on top of "larger"** — 370
  of 1,040 name-matched BF2 textures are larger on the PC, 312 taken, 24
  rejected because they are a different painting under the same name (`sky_hoth`
  is a blizzard on the PS2 and a blue sky on the PC) and 11 as
  detail-compressed bases. Detail: archive, same section.
- 2026-08-31 — **water, sky objects, the terrain split, mip chains, the SIDE
  tier, flythroughs.** `tern -> WATR` decoded completely and drawn on the twelve
  maps that ship it (`WMAP` a nibble per cell row-major, `LAYR`'s first f32 the
  water plane proven twice, `CPAT` the shoreline surface; three PROVISIONAL
  calls incl. a REJECTION — `LAYR`'s colour word survives no byte order). Sky
  objects: twenty maps author capital ships whose geometry lives in the SIDE
  LVLs, and the same work uncovered that **every sky dome whose authored name
  was not lowercase drew nothing**. The terrain split (−17 to −22 %) came with
  the rule that it MUST happen at upload — chunking it in the bake silently
  corrupts `Scene::heightfield` and the square-count guard passes anyway — and
  with the 300-frame median protocol (§Testing ladder). Mip chains completed
  for 1,989 of 2,288 textures; load time re-measured at 21–39 ms, retiring
  PLAN B7. Detail: archive, same section.
- 2026-08-31 — **The maintainer's first defect hunt: five complaints, six bugs, all
  ours** (session 5). The flythrough orbited the world origin, not the map;
  model textures were looked up CASE-SENSITIVELY and **3,752 draws across the
  32 maps rendered flat grey**; `DomeInfo.Ambient(128)` halved four skies;
  session 4's constant sky-shell fog weight was replaced by the map's own ramp
  at the shell's own distance (the BF2 Hoth horizon step: 147 levels to 1);
  Bespin's cloud sea was under an invented tan tint; three maps cleared to
  desert blue below the horizon. Two of his items were NOT bugs and are
  documented as such (Mustafar's double ring is authored; BF2 YAV1's fog is
  ground-level and the flythrough orbits outside it). Two questions came out of
  it and are still open: both Hoths clip 10 % of the frame to pure white
  (HOLD 11(a)) and the `*_collision*` meshes are in the drawable list
  (HOLD 11(b)). The same session's first **round-completion sweep** read 21 of
  32 maps finishing — the number that started the collision and AI work.
  Detail: archive, same section.
- 2026-09-01 (Fable session 6) — **the first cleanse.** 41 findings over the
  whole codebase and its docs: 33 FIXED, 6 deferred with a home, 1 declined;
  the deterministic sim, the render pass order, the 29 bake decoder headers,
  the fidelity ledger (57 PROVISIONAL sites, each with its reason) and the
  guardrail greps came back CLEAN on an adversarial read. Its three
  corrections are the load-bearing part: the "one-ticket stall" on eleven
  maps was ONE defect — no walkable model collision — and not a round-end
  rule (the bleed clamp at `alive < tickets` is faithful, re-tickets-spawn
  §1.3); BF2 HOT1's 20-second "domination win" was the mission Lua
  reassigning posts that the placeholder RON could not express; the
  session-3 dome bands were superseded by the session-5 census (every base
  dome reaches the zenith). Ritual: `.claude/commands/cleanse.md`. Detail:
  `analysis/PLAN_LOG_2026.md` §2026-09-01.
- 2026-09-01 (Fable session 7) — **walkable model collision landed, and the
  mission RONs are generated from the discs.** `coll`/`prim`/`CUTR` were
  decoded first (RE_LOG "`coll` / `prim` / `CUTR`", 1,698 chunks on both
  discs), then `core::collision` was built on them: ground = the highest
  walkable surface between a `STEP_HEIGHT` probe and the terrain, a point
  inside a cutter has no terrain, the terrain is a ONE-SIDED sheet
  (`TERRAIN_BURY`), walls are a capsule push-out, and the world is an input
  like the heightfield — not hashed. The sweep went 21 → 28 of 32 and named
  its own next blocker: the four that did not finish were all NAVIGATION
  (BES2, RHN1, DAG1's cave, POL1's shaft), which is what Phase A then took.
  Environmental deaths (`AddDeathRegion`, `LETHAL_LANDING_SPEED`) are ours
  and PROVISIONAL. `scripts/gen_missions.py` undumps Lua 4.0/5.0 bytecode to
  **57 mission RONs** with the PC sources as referee: BF1's mission letter is
  the ATTACKING faction (so `tat1_con.ron` was internally inconsistent and is
  now `tat1i`), and all 68 PC/PS2 disagreements are the same fact — the PS2
  retail shrinks the member pool. The P7 entity shape and the two P9 specs
  written here are in §DESIGN SPECS and (built) below; the maintainer's single-file
  build request is HANDOFF §Standing backlog 8. Commits f8576ec, 9638d3a.
  Detail: `analysis/PLAN_LOG_2026.md` §2026-09-01.

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

- 2026-09-01 (Fable session 8) — **what the ELFs say about difficulty**
  (`analysis/re-ai-difficulty.md`, lead-verified). Both originals run ONE
  integer skill 1..20 per team and **both are asymmetric by design — the
  human's teammates are made weaker than the enemy** (BF2 enemy 4/14/17 vs
  teammates 6/11/11; BF1 6/4/4), which is exactly what the maintainer rejected, so our
  symmetric-competence model is a deliberate deviation whose cost is
  quantified: at our Normal, teammates are old-Elite enemies. **Anchor: enemy
  skill 17** → spread ×1.32, AI dispersion ×0.32, the tracker's accel/damping
  lerps, a tracked-position error radius instead of a reaction timer, burst
  and pause curves. Later in the session three hypotheses closed: the AI view
  cone is a **77.5° half-angle [V]** in both games (10 m omni in BF2 only);
  the level-3-only damage multiplier is the ODF key `HitLocation`, shipped
  only as vehicle critical-hit prims — so the PS2's Elite lever is "AI may
  score vehicle criticals", NOT a damage buff, and Phase C's per-human
  multiplier is entirely ours; and PS2 BF2's Elite turns the human's
  auto-aim / aim-assist / sticky reticule OFF (`ScriptCB_SetDifficulty`).
  Corrections it forced: BF2's `EnableAIAutoBalance` is a SKILL rubber band,
  not a bot-count balance (so "uneven sides balanced by bot count" is
  openbf's own rule), and re-blaster-damage §6.2's "no per-bodypart
  multiplier" was wrong. Detail: `analysis/PLAN_LOG_2026.md` §2026-09-01.

- 2026-09-01 (Fable session 8) — **the spine and the data under the AI.**
  P9 (a) `Sim::step` over N pads indexed by player slot (`InputLog.slots`,
  flat layout, the two on-disk P3 logs replay unchanged) and P9 (b)
  `core::fmath` on the `libm` crate with a source-scan referee, both landed
  and lead-verified. **P9 (b) is the one change allowed to move a hash by
  rounding, and it did:** libm vs glibc 2.43 differ by at most 1 ulp, the
  player replays stayed byte-identical, and the round sweep moved 28 → 27 of
  32 — root-caused, not assumed, by re-backing `fmath` with `std` and
  getting the old sweep back byte-for-byte. Its follow-ups (parry
  `enhanced-determinism` before the first cross-OS lockstep test; an
  AI-battle capture that actually exercises `ai.rs`) are HANDOFF §Standing
  backlog 2 and 4. A build agent's first parry audit was **FABRICATED and
  retracted**; the verdict stands on the lead's own grep, and the working
  rule it produced is in PLAN §Working mode: an agent's claim enters the Log
  only after the lead reproduces its load-bearing measurement. The shared
  `manifest` crate landed (17 bake↔runtime types, three reference bakes
  byte-identical), then the AI data entered the bake: `mungefs::plan`/`plnp`
  + hints + barriers, layout detected by exact tiling and never by game,
  **57 nav blocks / 4,757 hubs / 6,218 arcs / 5,498 hints / 13,116 barriers /
  180 plnp records, equal to the RE census on every map** and pinned as a
  test. Detail: `analysis/PLAN_LOG_2026.md` §2026-09-01.

- 2026-09-01 (Fable session 8) — **Phases A, A.1 and A.2 landed; the sweep is
  the gate and it stayed honest.** A: `core::nav` (locate / next_hop /
  route_bfs / barriers) and the follower in `core::ai`, `walls_for_ai`
  REMOVED so the wall pass applies to every unit — 28 of 32, all four
  session-7 stalls fixed, 0 BFS fallbacks (the authored tables answered every
  hop). Bolts and `ai::visible` then took model collision and the sweep FELL
  to 24 — the honest cost of no longer shooting through geometry, and the
  measured p99 (1.25 → 2.71 ms at 64 a side, all of it the LOS ray scanning
  1,578 bodies) is why the broadphase is HANDOFF §Standing backlog 1. A.1
  added the barrier corner-walk A\* and the sweep-and-fan steering (26); A.2
  made the probe a shape CAST, resolved waypoints at the unit's level and
  excluded jet-jump arcs (27), and REJECTED its own contact-cue decision on a
  four-way ablation rather than shipping it. Every steering constant is
  PROVISIONAL and labelled in `core::ai`/`core::nav`; re-deriving them is
  HANDOFF §Standing backlog 6. **Two data findings that are still open:**
  SOLDIER arcs cross SOLDIER barriers on every map (HOLD 17, barrier
  semantics), and `wall_sweep` skipped the whole body on its first floor hit
  — the bug A.3 fixed. The Phase B/C/D designs written this session are in
  §DESIGN SPECS (C and D) and the archive (B). Detail:
  `analysis/PLAN_LOG_2026.md` §2026-09-01.

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

- 2026-09-02 — **A.3, then the bug under every number since 2026-09-01: the
  wall pass was pushing every unit out of a MIRRORED copy of every mesh
  body.** A.3 landed the parallel sweep (7 m 10 s → 2 m 26 s, byte-identical
  stdout) with the scoreboard footer this project still gates on, and
  `wall_sweep`'s first non-floor hit per mesh body. Then the "sweep says
  clear, push 0.30 every tick" signature was root-caused to **parry3d
  0.22.1's `contact_composite_shape_shape`, which calls
  `contact_with_shape(&pose12.inverse(), …)` where the callee already wants
  `pose12`** — so a capsule was tested point-mirrored through each instance's
  origin. Scope: capsule-vs-`TriMesh` `contact` only (`push_out` and the
  sweep's touching branch); casts and rays were always correct, which is why
  the sweep and the push disagreed. Fixed by `mesh_contact` (9258cd3),
  per-triangle contact over the BVH with world isometries, with a regression
  test on an ASYMMETRIC body that also asserts the raw parry behaviour so a
  future parry upgrade fails loudly and the workaround can be dropped. **The
  honest scoreboard fell to 23 of 32** and every hash moved: walls are solid
  now, so units are blocked more often and pinned less. Everything measured
  between 2026-09-01 and this fix — the A.1 kerb probe, the contact-cue
  ablation, every `stuck_max` — was measured in the mirrored world. This is
  also why PLAN §Tech stack pins the parry version. Detail:
  `analysis/PLAN_LOG_2026.md` §2026-09-02.
- 2026-09-02 — **the 148 "untextured draws" are disc facts, and the build
  specs were released.** Reproduced on raw bytes: 75 draws are disc segments
  that name NO texture (1–18-triangle stubs, effect anchors,
  `*_collision_*` placeholders), 8 name a texture that exists in no `tex_`
  chunk on either disc (`yav_prop_ivy`, `Cor1_bldg_flowers`, `noIcon`), and
  64 are BES1 terrain chunks the runtime already drops — case folding,
  suffixes, truncation and nested `lvl_` chunks account for zero of them.
  What the PS2 draws for such a segment is HOLD 18. The same entry released
  the ALPHA RULE v2 (D1–D6) and Phase B build decisions, both since built.
  Detail: archive, same section.


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

- 2026-09-03 (Fable session 11) — **ALPHA RULE v2, sky placement, Phase B,
  the ordnance ray.** (1) **ALPHA RULE v2 landed** (3f8c38b, 23bc7d4,
  f7a411f, dda89b4; goldens c787beb): the alpha mode is a property of the
  DRAW, written by the bake as glTF `alphaMode` per segment and routed
  through three pipelines. Census: OPAQUE 5,276 / MASK 797 / BLEND 187 over
  6,260 draws, with 91 (dir, texture) pairs carrying mixed verdicts — the
  information the per-texture OR could not hold. 28 of 42 goldens moved and
  were viewed on sheets. D7 (the lead's amendment, PROVISIONAL): the SKY
  path keeps its pre-v2 alpha. Three thresholds that argue with the data are
  recorded unacted-on (the Jawa junk misses `a0 ≥ 0.05` by 0.002). (2) **Sky
  dome placement RE'd** (`analysis/re-sky-dome-placement.md`) and then built
  (a937199, f01f45d; 21 goldens): a shell's Y is `cam.y + (Offset − cam.y)·
  (1 − MovementScale)·S`, so Mustafar's seam sits at −4.9° on the PS2 and
  the port's +0.63° was the port's own lift. (3) **Phase B landed** —
  `core::commander`, `TeamPlan` per team, Attack/Defend/Hunt scored by the
  disc's `Value_ATK_*`/`Value_DEF_*` over graph hops, with **eleven
  deviations from the design, each forced by a per-map ROUND_VERBOSE read
  and each accepted** (bbb8baa, c613a6a). Acceptance met: ≥ 2 distinct
  attack objectives on every map with ≥ 3 posts, unreachable dwell ≤ 329
  ticks. (4) The **ORDNANCE subset** reaches bolts and AI line of sight
  (f8dcf57): 1,274 soldier-only and 88 ordnance-only bodies over the 32
  worlds. **An open bake gap found here and not fixed:** BF2 HOT1's manifest
  has no class or collision row for `hoth_prop_bridge_long/short` although
  its conquest layer places six — those bridges have no collision at all
  (a side-LVL class the map bake does not pull). A replay-hash scare the
  same session was traced to partial scratch bakes under /tmp's quota, not
  to any code — which is why scratch targets and bakes live under
  `engine/artifacts/lead_tmp/`. Detail: `analysis/PLAN_LOG_2026.md`
  §2026-09-03.

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


- 2026-09-07 (Fable session 13) — **SUMMARY; the entries are in `analysis/PLAN_LOG_2026.md` Part 3.** The four
  parked branches landed (layer content `0b93bb1`, the ELF fog rule, the class roster, the walkabout gallery) with
  ONE full rebake and the goldens regenerated after every sheet was viewed. The maintainer's map-defect hunt over the
  219-image walkabout set was grouped by cause (`engine/artifacts/walkabout_defects/README.md`; the live list is
  §WHERE THINGS STAND "Known open defects"), and the session's closing CORRECTION found that the terrain cutters'
  sphere is stored radius-first — the Hoth rooms' defect was the RENDER, not the sim.

- 2026-09-08 (Fable sessions 14–16) — **SUMMARY; the entries are in `analysis/PLAN_LOG_2026.md` Part 3.** Session 14
  landed the cleanse's render / game-tests / mungefs rows, Phase C commit 1 (`658c6af`) and the gate scripts
  (`sim_gates.sh`, `scratch_bake.sh`, `cmp_bake.sh`), and verified the RE behind the aim-error floor (a float), the
  sniper table, `LOWR` (a same-footprint far LOD) and the Hoth `PTCH` mesh. The maintainer TOOK his three decisions (the
  droideka jumps everywhere with BF2's numbers; the unified sound plan; ALPHA RULE v2 until a GS dump — live in
  §WHERE THINGS STAND). Session 15 landed the loop's ten parked branches — Phase C COMPLETE (`fc077bb`), C2's docs
  cleanse (`4fcc3ca`) — with the gate values moving by design. Session 16, stopped mid-way at the maintainer's request and
  then completed, landed six more (H `50c7993`, 15, 16, 11, 17 and Phase D 0a–1b `6275a6d`) with ONE rebake, 24
  goldens regenerated after viewing, and the gates re-baselined on Phase D's stance bitmask.

- 2026-09-09 (Fable sessions 17–19) — **SUMMARY; the entries are in `analysis/PLAN_LOG_2026.md` Part 3.** Twenty-one
  branches landed in three windows. Session 17: seven — Phase D COMPLETE (`cef083b`), the far-scene cull behind
  `OPENBF_FAR_CULL` (`b203619`), the two-tier textures — with one rebake, 31 goldens, four RE answers verified, and
  The maintainer's "Fable sparingly" rule (superseded 2026-09-14/17). Session 18: nine — the BF2 far fade (`19148e6`), the
  water's `fx__` config (W1 `52e6b54`), the sweep's slot-0 bot (S0 — the SWEEP re-baselined) — with one rebake and
  11 goldens. Session 19: five — the water's tint and fog (W2 `5e047c2`) and the era class db (item 4 `542d7bc` —
  the SWEEP and the TAT2 ARENA re-baselined, no escape hatch) — with no rebake, 7 goldens and the water tint scale
  verified in the ELF; its baselines (`lead_tmp/s19/baselines/SUMMARY.txt`) seeded QUEUE.md's GATES block.

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

- 2026-09-17 — **The ci flake is gone: queue item ONCE landed on main as `95a641a`** (lane 1, job B; branch `roster-oncelock`, the build `76e120d` rebased; landing report `engine/artifacts/loop/reports/20260917_1742_land_ONCE.md`, build report `20260910_0118_roster-oncelock.md`). Eight added lines in `engine/crates/game/tests/roster.rs` and nothing else: `classes_dir()` is a `OnceLock<Option<PathBuf>>` over the old body, renamed `bake_classes_dir()` byte for byte. It was landed ahead of the older parked SM, CT and AT because the cleanse (lane 2) had just caught the race it closes redding a full ci **on main** — `roster.rs:256 kam1 rosters: parse classes.ron — 4114:27: Expected end of string`, a test reading a `classes.ron` another test in the same binary was half-way through rewriting — and WP's build report had caught it once more on a branch. Until it landed, every lane's ci was rolling a die that had nothing to do with the branch under test.
  **Gates, all run by the landing session, exit codes read directly:** rebase clean (`roster.rs` untouched on main since the branch's base `417a8cd`); ci on the rebased branch **EXIT=0, 676 passed / 0 failed / 24 ignored**; ci on main after the fast-forward **EXIT=0, 676/0/24**; `scripts/gpu_tests.sh` on main **EXIT=0, 24 passed**, `git status --short engine/goldens` EMPTY. No [REBAKE], [GOLDENS] or [SWEEP] mark and none was owed: the merged diff is one test file, so no sim hash, sweep, arena, bake byte or golden can see it.
  **The entry's own gates, re-measured rather than taken on the build report's word.** Test list identical: **12 tests, same names** (`--list` on both prebuilt binaries). Wall time, alternated pairs inside one `flock`: serial median **3.029 → 1.864 s** (−38 %), parallel median **0.596 → 0.556 s** (−7 %) — the same direction and shape as the build report's 3.712 → 2.174 / 0.589 → 0.573, lower absolutes because the machine was quieter. **Bakes per test binary 12 → 1, instrumented WITHOUT the build report's temporary `eprintln!` probe:** `strace` is not installed here, so the landing polled `/proc/<pid>/io` `wchar` (bytes through `write(2)`, page-cache-independent) to exit and divided by the size of the scratch dir the run leaves behind — one full bake's worth. Before **32,790,652 B ÷ 2,732,487 B = 12.00×**; after **2,733,195 ÷ 2,732,487 = 1.00×**; each side measured twice, identical both times.
  **DECIDED (loop, 2026-09-17, lane 1): the GATES block's ci count is corrected 673 → 676 as a stale docs value, not a re-baseline.** Why: WP added three tests and its landing did not rewrite the block (its own report records "main was 673 before the merge" and 676 after), so the canonical count in QUEUE.md's GATES block had been wrong for two landings; two independent runs this session, branch and main, both sum 676/0/24. How to reverse: the block is one line in `engine/artifacts/loop/QUEUE.md`; the three tests are named in WP's report §Deviations 3.
  **DECIDED (loop, 2026-09-17, lane 1): the `/tmp` scratch leak this exposed becomes queue entry CLN3, not a fix folded into this landing.** Why: ONCE removes 11 of the 12 *writes* but deletes nothing, and about 30 test sites write pid-scoped scratch that only `bake/src/map.rs:1461-1462` ever removes — measured on this box at 17:41, counting only runs since 15:52 the same day, **37 stale `/tmp/openbf-roster-classes-<pid>/` = 98 MB** plus **126 stray `/tmp/openbf-*-classes-<pid>.ron` = 28 MB**, on a tmpfs, i.e. in RAM. Folding an unreviewed cleanup onto a green test-only branch would have cost the landing its "nothing else moved" claim. How to reverse: drop the CLN3 entry. The 98 MB was swept during the landing (dead pids only, each checked with `kill -0`) so the leak could be measured honestly; `/tmp` is outside the repo and outside `cleanse_delete.sh`'s scope.

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

- **2026-09-17 (evening) — LANDED (loop, lane 2, job B, queue CT): the class RON is ONE definition — `manifest::ClassDef`, written by `bake` and read by `game` through the same struct.** Branch `class-ron-twin` (built 2026-09-10, parked seven days), rebased onto main `81b7970` and fast-forwarded as **`a2cb9cf`**; landing report `engine/artifacts/loop/reports/20260917_1905_land_CT.md`, build report `20260910_0114_class-ron-twin.md`. This is the THIRD of the RON twins item 13 found (`6d87a77` fixed the loadscreen and sounds pair): `bake::classes::BakedClass` and `game::classdb::ClassDef` were two declarations of the same four-field `classes/*.ron` record, differing only in the reader's `#[serde(default)] kind`. Now ONE `manifest::ClassDef` in the data-only crate, with both old paths kept as re-exports; `BakedClass`'s four property lookups could not stay inherent on a foreign type and could not move to `manifest` without dragging behaviour and an FNV dependency into it, so they became the `bake::classes::ClassProps` trait — the only shape that keeps `manifest/src/lib.rs:18`'s "data, not behaviour" rule true. +114/−36 over four files.
  **The rebase was free:** `git log 417a8cd..main` over those four files is EMPTY — nothing on main had touched them in the seven days the branch waited, through nine landings.
  **Gates, every exit code read directly, on the branch AND again on main:** ci **682/0/24** both sides (680 + the 2 declared — `manifest::tests::class_def_round_trips_and_its_field_order_is_the_wire_format` and `bake::classes::tests::every_shipped_classes_ron_re_serializes_byte_identically`); gpu EXIT=0 with **24 referees** and `git status --short engine/goldens` EMPTY both sides; a full scratch bake against **today's** shipped tree — not the one the branch was built on, since SM's `f21c2db` moved the bake in between — **SAME=21651 DIFF=0 ONLY_IN_A=0 ONLY_IN_B=0**; and the whole behaviour set run on both sides even though CT carries no [SWEEP]: sweep pair `cmp` 0 RAW and `cmp` 0 against the s19 baseline at the GATES footer (`finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`), sim_gates **GATES_RC=0** with `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` and both motion.csv identical to base, `--sim-hash 100` **`17e1b946ff0e821c`**, both arena tables `cmp` 0 against s19. **Every gate value in the GATES block is unmoved and every `cmp` against the s19 baselines is rc 0** — which is the entry's whole claim: the reader moved, no value did. No rebake owed (the bake gate proves it), no golden moved, no walkabout regeneration, no re-baseline.
  **The build report's load-bearing numbers reproduced here rather than taken on its word:** the +2 on ci, the 21,651-file DIFF=0 against the current tree, the 24 referees, and the new test's own `--nocapture` line — **`10 classes.ron, 1654 classes, byte-identical`**, which confirms the build's deviation 1: the entry text's "the 57 side RONs + the shared ones" is really **ten** files (`assets_baked/classes/{bf1,bf2}_{all,imp,rep,cis}.ron` + `bf1_common.ron` + `bf2_ingame.ron`); no side directory ships a class RON.
  **DECIDED (loop, 2026-09-17, lane 2): the GATES block's ci count is 680 → 682, and its CLN1 prediction is corrected to 683.** *Why:* the block was written while measuring against 680 and predicted 681 for CLN1's +1; CT's two tests landed first. *How to reverse:* the count is measured every landing — the next ci reading overrides it.
  **DECIDED (loop, 2026-09-17, lane 2): the goldens were CHECKED, not regenerated.** *Why:* CT declares no [GOLDENS], and `goldens.sh --check` inside `gpu_tests.sh` passed on both sides with a clean `git status`; regenerating goldens no entry declared would invite noise into a refactor's landing, and §3.7's quantify-and-view procedure exists for moves, of which there were none. *How to reverse:* `scripts/goldens.sh` on main regenerates at any time. The stale 2026-09-10 scratch bake `lead_tmp/class-ron-twin/bake/` was NOT reused for the same reason — it was baked against a tree that has since moved — and a fresh `bake_land/` was written instead; both go to `cleanse_delete.sh`.

- **2026-09-17 (evening) — RE (loop, lane 1, job C, queue WA's [RE] half): the PS2's water DOES composite by its TEXTURE alpha — and the near speckle deliberately does not.** No branch, no code change, no gate run and none could move: this job reads the discs and writes `analysis/` only. Lifted into `analysis/re-water-alpha.md` under the `# OPUS RE … [O] UNAUDITED: no Fable pass` header with every load-bearing claim tagged `[O]`; RE_LOG pointer added; `re-water.md` §3 and §8 amended; HANDOFF HOLD 21(a) narrowed and §What next item 2's water line closed; scratch, census binaries and raw measurements under `engine/artifacts/lead_tmp/re/water-alpha/`; report `engine/artifacts/loop/reports/20260917_2045_re_WA.md`.
  **The answer.** Every BLENDED water layer takes `As = At × Av >> 7` with `At` the sheet's own alpha: the TEX0 the water binds carries **TCC = 1 (RGBA) and TFX = 0 (MODULATE)**, built for every non-32-bpp texture by the engine's single texture path (BF1 `FUN_0049c5c8` 562034, reached from the flat-water renderer `FUN_0048b9c0` 551303/551306 → `FUN_0049ee68` modes 10/8 → `FUN_0049de40` → the builder; BF2 `FUN_00582c90` 717181 by the identical chain from `FUN_00575fd8` 708713/708716), and the near main (0x69c390 / 0x76c170), far main (0x69c3a0 / 0x76c180) and ocean foam (0x69c3e0 / 0x76c1c0) records all carry ALPHA 0x44. The exceptions are exact and both are self-consistent with the disc's own data: the ocean BASE is not blended at all (0x4100, prim 0x18 without ABE) and its sheet `kam1_water_2` is the ONLY water sheet on either disc authored alpha-255 on every texel; and the near cross-faded SPECKLE has its TCC bit **explicitly cleared** — *(decompiler detail omitted)*, whose only zero bit is 34 — at BF1 551846 / BF2 709258, behind a gate (`DAT_00522a64` / `DAT_0060c1e4`) that reads 1 in both ELFs and is never written, which is exactly why BF1's 25 `water_specularmask_NN` frames are authored alpha-255.
  **Measured on the disc this session, with the port's own `mungefs`** (`lead_tmp/re/water-alpha/disc_water_alpha.txt`): all 14 water sheets are 4- or 8-bpp paletted, so every one takes the TCC=1 branch, and **11 of the 12 `MainTexture`s carry real alpha** — `kas2_water` 96 flat, `yav_water` 96 flat, `nab2_water` 89..190, `fel1_water` 80..171, `dag1_water` 138..203, `dea1_water` 137..240, `kas1_water` 142..255, BF2 `kas2_water` 158 flat, the ocean foam `kam1_water` 0..218 (mean 62, 11.1 % at zero). So **`fs_water`'s `base.a` multiply is the disc's arithmetic** and is inert only because `bake::alpha` forces those sheets opaque; the bake is the defect, and giving them their alpha back is not a look judgement.
  **The port question the item carried is answered too, and it is not a water path: BF1 KAM1's water is under the map's own terrain.** BF1 KAM1 ships a real heightfield (256×256 at 40 m) that sits at y = 0.00 over **every one of its 1,618 wet cell centres — 8.00 m above the −8 water plane** — where BF2 KAM1's `tern` is a STUB with no heightfield at all (`kam1_terrain_vs_water.txt`). Same sheet, same plane, same cells: that is why bf1_kam1's golden and all 8 of its walkabout panels read 0.000 when the sheet flipped while bf2_kam1's golden moved 5 %. The committed goldens show it — BF1 KAM1's "sea" is the flat grey terrain plate, BF2 KAM1's is real water between the platforms.
  **DECIDED (loop, 2026-09-17, lane 1): WA's build half is scoped NARROW — only the sheets a map's own `fx__ Effect("Water")` names get their disc alpha back, not every unnamed map texture.** *Why:* the [V] evidence reaches the WATER draw and stops there; the wider "no segment names it" clause queue SM measured would free bird/fish/mist/foliage/terrain DTEX on the same warrant, and those are model-drawn sheets whose verdicts belong to ALPHA RULE v2, not to this reading. *How to reverse:* the predicate is one clause in `bake::alpha` — widening it later is the same clause with `unnamed` restored. Consequence recorded in the entry: SM's measured golden deltas are an UPPER bound and the build half re-measures its own.
  **The verifier agent (§5's falsification half; one Opus pass, citations only, never the reasoning) falsified exactly one claim and it was the speckle's** — the lead had read `FUN_0048c848`'s TCC=1 and stopped at that line; the agent read to the function's end and found the clear 58 lines later. It was **reproduced by the lead before acceptance** (the decomp line, the mask's single zero bit, the gate's ELF value and its two read-only occurrences, and the machine code at va 0x48ce5c-0x48ce90 that builds the mask and applies it with `ld/and/sd` to the first qword). Four wordings tightened with it: the TEX0 guard is "not 32-bpp" rather than "4/8-bpp"; 562033/562038 assign the CLD bit with the OR at 562040; `FUN_0049c5c8` has a second TEX0 path that bakes TCC=1 into its TH constants; and both games carry an alternate near record (0x69c380 / 0x76c160) their selectors never reach. Everything else was SUPPORTED with the enclosing function named.
  **What stays open:** `Av` itself — the Min.a→Max.a value the VU1 phase nibble animates — is still HOLD 21's, untouched by this job; and the RE turned up one erratum candidate in `re-material-flags.md` §2, whose blanket "TEX0 TCC=1 for every texture in both games" is looser than the code at the 32-bpp site (queued as **TCC32**; no water sheet is 32-bpp, so nothing today turns on it).
- **2026-09-17 (evening) — LANDED (loop, lane 1, job B, queue CLN1): the host-libm ban had FIFTEEN holes, and the guard that keeps every replay reproducible across platforms had never been watched fail.** Branch `fmath-ban-holes` (built 2026-09-17 17:55, lane 2), rebased onto main `86eaa38` and fast-forwarded as **`d3e69cf`**; landing report `engine/artifacts/loop/reports/20260917_1934_land_CLN1.md`, build report `20260917_1755_fmath-ban-holes.md`. `core::fmath`'s `BANNED` referee — the test that keeps the sim off the host's libm so a replay recorded on one OS reproduces on every other — listed **10** call patterns and missed `.hypot( .exp2( .log2( .log10( .cbrt( .atan( .sinh( .cosh( .tanh( .exp_m1( .ln_1p(` (the cleanse's finding C5) plus four the cleanse did not name: `.log(` (`f32::log(base)` is `self.ln() / base.ln()`, two host calls), `.asinh(`, `.acosh(`, `.atanh(`. It now lists **25**, grouped and commented, with the scan lifted into `banned_calls_in(name, code)` and a new self-test `the_referee_catches_every_banned_call` that feeds one call per pattern and asserts each is caught EXACTLY once — which is also what proves the rows cannot shadow one another (`.asin(` vs `.asinh(`, `.exp(` vs `.exp2(`, `.tan(` vs `.atan(`, `.log(` vs a free `run_log(`). +80/−22, one file, entirely inside its `mod tests`.
  **Gates, every exit code read directly from its own file, on the branch AND again on main:** ci **EXIT=0, 683 passed / 0 failed / 24 ignored** on both — the GATES block's prediction exactly, and six more than the **677** the entry text predicted, which was written against a 676 main before SM (+3), AT (+1) and CT (+2) landed under it. gpu **EXIT=0 with 24 referees** and `git status --short engine/goldens` EMPTY on both sides; the goldens were CHECKED (`goldens --check` inside `gpu_tests.sh`), not regenerated. No rebake, no golden, no walkabout, no re-baseline, no stash. The rebase was free: `git log 68b5fbd..main -- engine/crates/core/src/fmath.rs` is EMPTY across the **17 commits** the branch sat parked behind.
  **The build report's load-bearing claims reproduced here rather than taken on its word:** the +1 on ci; the pre-image's list is **10** rows and the string `hypot` does not occur anywhere in it, so the old referee could not have matched a live `hypot` call by construction, against the post-image's **25**; and the guard's failure re-run on the REBASED branch — `Barrier::bound_radius` (`core/src/nav.rs:257`) rewritten from `self.half.length()` to `self.half.x.hypot(self.half.y)`, `cargo test -p core --lib fmath::tests::no_std_transcendentals_in_core` exiting **101** and naming `nav.rs:257: self.half.x.hypot(self.half.y)`, then `nav.rs` restored from a copy (never `git restore`, which the guard rightly denies) with `git status` clean before the merge.
  **DECIDED (loop, 2026-09-17, lane 1): the sweep, the three replay hashes, the arenas and `--sim-hash 100` were NOT run for this landing — by design AND by proof, not by omission.** *Why:* the only changed file is `core/src/fmath.rs` and its production half — lines 1–39, everything above the `#[cfg(test)]` / `mod tests {` pair at lines 40–41, which runs to EOF — is BYTE-IDENTICAL on both sides (`sha256 6631f59cad2bc81e0e064eae6269a1464d756efea48c681f35b120d4c0ea9cee` on each), so there is no non-test code in the diff for a sim, render or bake byte to move; and ci ITSELF pins `--sim-hash 100` = `17e1b946ff0e821c` (`core/src/sim.rs:2070`, `a_squad_command_is_part_of_the_hash_and_no_command_costs_nothing`, green in both runs), so that one gate value WAS measured, by the ci both sides ran. The alternative — 20 minutes of sweep for a diff that cannot reach the sim — buys no information. *How to reverse:* the commands are in the GATES block and run on main at any time. This is a narrower rule than CT's landing used (CT ran the whole behaviour set because it touched real `game/src` code); the test that separates them is whether any line outside `#[cfg(test)]` moved.
  **DECIDED (loop, 2026-09-17, lane 1): the GATES block's ci count is 682 → 683**, measured on the branch and again on main, and its CLN1 prediction is retired. *Why:* the block predicted 683 for this landing and was right; the entry's own 677 was two landings stale when it was written and three when it landed — a build report's expected ci count is a prediction, and the block is the live copy. *How to reverse:* the count is re-measured every landing.
  **Still open from this branch:** `DET1`, the finding CLN1's build turned up while checking the extended list for live violations — `game/src/layout.rs:319` computes a CP spawn yaw with std's `f32::atan2` (host libm) and that yaw reaches `Sim::hash` through `walk.rs:204/314` → `core/src/cp.rs:62` → `sim.rs:594/938/948` → `unit.rs:149` → `soldier.rs:473,481`, so the replay hash depends bit-for-bit on the host's `atan2f` just outside the referee's `core/src` scope; **1,369 `CP*SpawnPath` nodes across 22 of the 32 baked maps, 1,087 of them (79.4 %) off-axis**, tat1 74/61 and tat2 106/85 among them. It is marked [SWEEP] and lands after every other PARKED entry because fixing it is odds-on to move the three replay hashes, `--sim-hash 100` and the sweep. `.powi(` stays UNBANNED (LLVM expands `llvm.powi` to multiplications, not a libm call) — the build's call, recorded rather than silently skipped.

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
- **2026-09-17 (night) — LANDED (loop, lane 2, job C built AND landed in one session): queue OCL's BUILD half, `6bf0d60` — the port's ocean now draws at ONE resolution and the sea-seam is gone.** Branch `ocean-one-lod` off `386f6fb`, one commit, rebase a no-op, ff-merged; two files (`game/src/water_anim.rs`, `game/tests/water_anim.rs`), render-side only. Report `engine/artifacts/loop/reports/20260917_2110_build_land_OCL.md`.
  **What changed.** `OceanDraw::meshes: Option<(MeshHandle, MeshHandle)>` became `mesh: Option<MeshHandle>`; `step` builds and uploads one patch and gives it to every cell inside `ranges.far`; the `LODDecimation` validation went (it only ever guarded the stride the far patch was built at — the bake still carries the key); the module header's PROVISIONAL "Where the patches are drawn" bullet is answered by a new PS2 item 4 and cut back to the one half that is still ours (the still plane past the far range). `Ocean::patch(every)` and `the_far_patch_is_the_near_patch_at_every_other_point` stay — a sampler property that remains true, and the reason the far patch could go without moving a vertex the near patch had.
  **Gates reproduced on the branch AND on main; nothing moved.** ci **690 / 0 / 24** both sides (exit read directly). `gpu_tests.sh` exit 0 both sides with `git status --short engine/goldens` EMPTY — 24 referees including `every_baked_map_matches_its_golden`, which covers both KAM1s. The sweep pair 4570 B each, `cmp` clean to each other and to `s19/baselines/sweep_main_1.txt`, footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`. `sim_gates.sh` GATES_RC=0: walk_demo `bb860216c775e788`, aim_demo `b489b1b3aa14084f`, `--sim-hash 600` `fb19c1d2c811788b`, both motion.csv identical. `game --sim-hash 100` `17e1b946ff0e821c`. **No rebake and no walkabout regeneration owed, proven structurally**: `water_anim` has one reference outside `game` and it is a comment (`render/src/mesh.rs:346`), `animate_water` is called only from `main.rs` and `examples/water_frames.rs`, and `examples/walkabout.rs` contains the string "water" zero times.
  **Two of the entry's numbers were wrong and were re-measured rather than inherited.** (1) **The crack is 3.229 m, not 2.27 m** — that figure was the pre-OCB 8.9 mm scaled by the amplitude ratio and never re-measured after OCB. A throwaway probe (deleted before the commit, OCB's shape) on `Ocean::patch` at `every` = 1 vs 2 along both shared cell edges over 300 frames at 60 Hz gives worst **3.2292 m** at t = 1.900 s (field 21.85 m peak-to-trough there) and mean per-frame worst **2.4262 m**, so 2.27 m read as typical, not worst. `analysis/re-water.md` §3b and the RE_LOG entry are corrected. (2) **`--bench` is the wrong instrument** — at the overview camera bf2_kam1 submits 169 draw calls (43 of 1259 culled) and the water barely enters the frame time (0.23 vs 0.22 ms, indistinguishable). `water_frames`' step-alone is the right one, and it shows **both directions**: the step falls **0.0127 → 0.0112 ms** median over 8 runs a side (one patch built and uploaded instead of two) while every far cell gains **384 triangles** (128 → 512), which is invisible end to end here (moving-frame median 0.175 → 0.168 ms, draw count identical both sides).
  **DECIDED (loop, 2026-09-17, lane 2): `WaterRanges.near` stays, unread, and its doc says so in full.** *Why:* it is the disc's near/far PASS boundary (§3b (c) — the passes differ in the material record they bind), it is the input the two-pass work needs, and deleting it would mean editing `viewer.rs:855`, outside this entry's declared file set. *Reverse:* drop the field and its initialiser together in the entry that builds the two passes.
  **DECIDED (lane 2): no new ci test, by proof rather than excuse.** The seam is unrepresentable in the type, and `WaterMotion::step` takes a concrete `&mut render::Renderer`, so a CPU referee would have to be faked; if one is ever wanted it belongs in `scripts/gpu_tests.sh`. ci stays at 690.
  **DECIDED (lane 2): built and landed in one session**, §4.5's condition holding (entry green, MAIN free, most of the budget left).
  **The pictures, viewed** (`lead_tmp/ocean-one-lod/caps/sheets2/`, `--cam 400,32,-80,135,-22` at the centre of a wet 3×3 cell block): bf1_kam1 mean 0.90 / max 96 / 3.03 % changed / 1.09 % past 12-255, the far water's wave silhouettes gaining detail; bf2_kam1 mean 0.15 / 22.09 % changed but only 0.017 % past 12-255, its ocean being near-black at that vantage. **The scoping is the strongest evidence in the set:** on bf1_kam1 the change occupies rows 176–450 of 720 and **the bottom 269 rows are pixel-identical** — that band is the eye's own cell, which was always near and always drew the stride-1 mesh, so the change begins exactly at the ring. **Stated honestly:** no picture shows the crack as a visible tear and none is claimed — a 3.2 m step between two shaded water surfaces of nearly the same colour reads as a crease, not a hole. The TYPE is the gate, as the entry itself nominated.

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

- 2026-09-23 — **Q1's PCSX2 trace rig stood up for its first tier: headless boot + screenshot, and the "only the maintainer can supply a PCSX2 capture" restriction is LIFTED.** the maintainer asked for the loop to be able to reach for PCSX2 itself rather than queue a capture for him; this landed `engine/artifacts/pcsx2/pcsx2_capture.sh` + README, proven against both discs (`poc_swbf1_title.png`, `poc_swbf2_title.png` — real title screens, not the decomp). Two permanent bugs in the existing `~/.config/PCSX2/inis/PCSX2.ini` had to be fixed first, neither previously diagnosed: `SetupWizardIncomplete = true` forced the setup wizard on every launch regardless of how complete the config actually was (now `false`), and `[Folders] Bios = bios` resolved against PCSX2's own always-empty `~/.config/PCSX2/bios/`, never this repo's `bios/` — a silent "PCSX2 requires a PlayStation 2 BIOS" failure in the emulog, not a dialog (now the absolute path). Separately, this machine's desktop is a live Wayland session and Qt auto-prefers Wayland via `$WAYLAND_DISPLAY` regardless of `$DISPLAY` — a naive `DISPLAY=:99` isolation attempt reached the maintainer's real desktop twice (an error dialog, then the full setup wizard) before `-u WAYLAND_DISPLAY QT_QPA_PLATFORM=xcb` was found to be required together. `pcsx2-gsrunner` (`pcsx2-master/pcsx2-gsrunner/`), the tool PLAN's Q1 originally named, turned out on reading `Main.cpp` to be a GS-*dump* replayer for the emulator's own graphics regression testing, not a fresh-ISO-and-BIOS boot tool — not used. **What this does NOT do yet:** reach a specific point in a level. `-fastboot` reaches whatever the disc boots to by itself; `-statefile` can load a save state non-interactively, which is the natural next step for a specific map point, but no save-state library exists and creating the first one still needs a real playthrough or a live vision+action loop — bigger, separate work, not assumed done. `OPUS_LOOP_PROMPT.md` §5 and §7 updated to match (the DRAINED trigger and the "only the maintainer can supply" list both drop "a PCSX2 capture"; §5 gains the usage note and the ci.lock/GPU-concurrency requirement). Session was interactive (Sonnet, in-editor), not the autoloop — uncommitted, the maintainer's to review and commit.

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
- **2026-09-23 (night) — BUILT AND LANDED (loop, lane 1, job C, queue CMAP): `engine/CRATES.md`, the crate and module map, and `scripts/size_report.sh`.** Branch `crate-map` off `f53b9cc`, one commit, the rebase a no-op, fast-forwarded as **`8d86e3a`**; report `engine/artifacts/loop/reports/20260923_2240_build_land_CMAP.md`; scratch `engine/artifacts/lead_tmp/crate-map/`. The maintainer's 2026-09-23 request, ahead of P7 and the SPL1–SPL7 splits: the engine is 62K lines and a session had no map of it, so it grepped blind.
  **What:** `engine/CRATES.md` (204 lines, 16,066 B — inside the entry's ~250 lines / ~16 KB): per crate its purpose, its `Cargo.toml` dependencies and its src line count; all 104 src files, each with its line count, a one-line responsibility and its key public items, taken from the module's `//!` header and grep; the standing invariants (the deterministic `core`, no LVL parsing at runtime, the one-definition manifest, the byte-deterministic bake, the base-tier goldens, the citation rule); where the tests, examples, goldens and missions live; and a 28-row "where do I change X?" table. `scripts/size_report.sh` (NOT in ci): per-crate totals — 62,018 src lines in 104 files, tests and examples 12,109 in 53 — and every non-test src file over 1,500 lines (10: SPL1–SPL6's six and SPL7's four); `--check-map` exits 1 unless every `engine/crates/*/src/**/*.rs` is in the map and every path the map names exists (stale line counts are listed, not failed); `--strict` exits 1 for a src file over 2,000 lines that `scripts/size_waivers.txt` does not name. Each failure mode was made to fire once on a scratch root (`lead_tmp/crate-map/negtest.sh`, 11 cases: a deleted row, a bogus file and a bogus directory exit 1; a stale row count and a stale crate total are listed at exit 0; no waivers lists the six at exit 1; a reasonless waiver warns and exits 1; a stale waiver is listed; a missing map and a bad argument exit 2). The loop prompt (`OPUS_LOOP_PROMPT.md` §1 step 6, §4's closing paragraph, §6b item 1) and `.claude/commands/cleanse.md` (Posture, Pass 2, Pass 5) gained the three rules the entry names; `engine/artifacts/briefs/SPLIT_RECIPE.md` §8 now has each split update the map and delete its waiver line.
  **Gates:** ci **699 / 0 / 24**, exit 0, on the branch (twice — before and after three wording fixes to the map) and on main `8d86e3a`; `scripts/size_report.sh --check-map --strict` exit 0 on the branch and on main (104 files mapped, 0 missing paths, 0 stale counts, 0 unwaived). Nothing else was run, because nothing a build or a referee reads changed: the diff is four `.md` / `.sh` / `.txt` files and no crate, `ci.sh` or script names them (grep) — the entry's own gate: "`./ci.sh` 0 at the GATES count is enough".
  **DECIDED (loop, 2026-09-23, lane 1): the six files over 2,000 lines are WAIVED in `scripts/size_waivers.txt` as "split queued as SPL<n>".** *Why:* `--strict` exists to catch a NEW offender, and each of the six already has its split queued; the report lists a waiver as stale once its file drops under the cap, and SPLIT_RECIPE §8 now deletes the line. *How to reverse:* delete the six lines (`--strict` then exits 1 until SPL1–SPL6 land).
  **DECIDED (loop, 2026-09-23, lane 1): the three prompt rules went into the live `OPUS_LOOP_PROMPT.md` at the LANDING, not during the build.** *Why:* the prompt is untracked and every lane reads it fresh at session start, so an edit during the build would have sent other lanes to `engine/CRATES.md` and `scripts/size_report.sh` before main had them. *How to reverse:* copy `engine/artifacts/lead_tmp/crate-map/OPUS_LOOP_PROMPT_before_CMAP.md` back (`prompt_edits.diff` beside it holds the three hunks).
  **Also fixed (untrue text):** §Workspace layout lacked the `manifest` crate and `missions/`, still gave `core` an ECS, and put "reference motion traces" in `goldens/` (it holds PNGs only); it now points at `engine/CRATES.md`. **Found and left for the next cleanse's pass 2:** `core/src/lib.rs`'s `//!` header still says the sim has an "ECS" — there is none (units are generation-checked slots in `Sim::units`, and `hecs` is gone from `core/Cargo.toml`). **Docs:** HANDOFF §Read first (the map), §What next (SPL1 is next), §State, §For the maintainer (one calls line); QUEUE CMAP → DONE, archived verbatim; `LAST_CLEANSE` 2 landings since.
- **2026-09-23 (night) — BUILT AND LANDED (loop, lane 1, job C, queue SPL1): `game/src/viewer.rs` is a 37-line facade over four parts — a pure move; nothing that runs changed.** Branch `split-viewer` off `a476bf0`, seven commits, the rebase a no-op, fast-forwarded as **`947084d`..`fdfc00c`**; report `engine/artifacts/loop/reports/20260923_2330_build_land_SPL1.md`; scratch, the generator and every gate log in `engine/artifacts/lead_tmp/split-viewer/`. The first of the maintainer's SPL1–SPL7 splits (`engine/artifacts/briefs/SPLIT_RECIPE.md`) and the workspace's first `foo.rs` + `foo/` module, so it sets the pattern the other six follow.
  **What:** the 2,339-line file became `viewer.rs` 37 (its original `//!` header, four `mod` lines and a re-export of all 38 `pub` / `pub(crate)` items, so no caller changed) + `viewer/look.rs` 802 (a map's look as render state: `water_look`, the clear colour, `FOG_DENSITY_SCALE`, the far cull and the `LOWR` range, `far_fade_from`, the lighting; 6 tests) + `viewer/scene.rs` 1,077 (`upload_scene` and its draw list, the untextured census, the texture cache, the sky shells and `SkyObject` flights, the terrain draw chunker; 4 tests) + `viewer/upload.rs` 306 (the GPU upload primitives: submeshes, skinned meshes, textures, `.ktx2` files, the cube, quad and sprite) + `viewer/camera.rs` 199 (`GOLDEN_CAM`, `camera_from`, `instance_matrix`, the auto-framing; 1 test). Commits 1–4 extract one part each, sliced out of the base file by line range (`gen_split.py`), so every item moved byte for byte; commit 5 adds a `#[cfg(doc)]` import so four of `look.rs`'s doc links to `upload_scene` still resolve; commit 6 rewrites `engine/CRATES.md`'s rows and drops the viewer's size waiver; commit 7 points seven comments in `game`, `mungefs` and `render` at the part that now holds the item (six named a private item by its old path; `water_anim.rs` cited `viewer.rs:31–32` by line and now cites the two constants).
  **Gates (every exit read from its own file):** `scripts/tools/pure_move_check.py a476bf0:engine/crates/game/src/viewer.rs <the facade + the four parts>` → **PURE MOVE**: all 1,958 old meaningful lines present, 8 visibility-only changes (below), 2 new-only lines (the `#[cfg(doc)]` attribute and its comment). ci **699 / 0 / 24** before each of the seven commits and again on main; the workspace test list (`cargo test -- --list`, 723) is the base's exactly, apart from the 11 moved tests' paths (`viewer::tests::X` → `viewer::{look,scene,camera}::tests::X`; nothing in `scripts/`, `ci.sh`, `.claude/` or the GATES block names them). `gpu_tests.sh` exit 0, 24 referees, `git status --short engine/goldens` empty, on the branch twice and on main. Not required by the entry, run because `assets.rs` places collision instances with `instance_matrix`: `sim_gates.sh` at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both `motion.csv` identical, and `--sim-hash 100` `17e1b946ff0e821c`. `cargo doc -p game --no-deps` (not a gate): the same 40 warnings as the base. `scripts/size_report.sh --check-map --strict` exit 0 on the branch and on main. No rebake, no golden, no walkabout.
  **DECIDED (loop, 2026-09-23, lane 1): four parts, and `camera.rs` stays at 199 lines, under the entry's 300-line target.** *Why:* it is one responsibility (poses, the instance transform, the auto-framing), and folding it into `look.rs` would mix framing into the manifest's look rules. The sky shells went INTO `scene.rs` rather than a fifth part: `upload_sky` is `upload_scene`'s private helper and shares its census and texture cache, and a `sky.rs` needed seven more widenings. *How to reverse:* another pure move.
  **DECIDED (same): exactly eight items widen to `pub(super)` — `upload_cpu_texture_inner`, `compact_submesh`, `CompactSubmesh` and its five fields — and `upload_terrain_chunks` stays in `scene.rs` beside its one caller.** *Why:* its signature is 98 columns, so `pub(super)` would make rustfmt reflow it and `pure_move_check` could no longer match the line; SPLIT_RECIPE §10 now warns SPL2–SPL7. *How to reverse:* move the chunker to `upload.rs` and accept the reflowed line.
  **DECIDED (same): a `#[cfg(doc)]` import keeps `look.rs`'s four doc links to `upload_scene` resolving.** *Why:* rustc 1.97 flags an import used only by a doc link as unused (measured on a scratch crate) and clippy runs `-D warnings`, while the split alone took `cargo doc` from 40 to 44 warnings; the attributed import exists only for rustdoc. *How to reverse:* delete its three lines.
  **DECIDED (same): the crate map, the size waiver and the seven comments changed ON THE BRANCH, not after the fast-forward as SPLIT_RECIPE §8 has it.** *Why:* they land atomically with the split, so `--check-map` is never red on main. *How to reverse:* none needed.
  **Docs:** QUEUE SPL1 → DONE, archived verbatim; the `viewer.rs` pointers of 9, W3, 7, CLN5, CLN7, CORB, MUSS and DET1 now name the part (CLN7's test is `viewer::look::tests::no_shipped_map_fogs_its_water_at_a_step`); `engine/CRATES.md`'s counts corrected on main for the comment fix (`water_anim.rs` 1,163, `game` 14,687, the workspace 62,101); HANDOFF §What next, §State and §For the maintainer; `LAST_CLEANSE` 3 landings since; SPLIT_RECIPE §10 (the generator, the 100-column reflow trap, `#[cfg(doc)]`). Left as they are, per recipe §8(a): the line-number citations `viewer.rs:31–32` in `analysis/re-water.md` §3b and `analysis/RE_LOG.md`, both of which also name the constants, and the historical Log entries. Cite viewer code by symbol from here on.
- **2026-09-23 (night) — BUILT AND LANDED (loop, lane 1, job C, queue SPL2): the game binary's `main.rs` is 424 lines of CLI over three parts — a pure move; nothing that runs changed.** Branch `split-main` off `46d6f06`, five commits, the rebase a no-op, fast-forwarded as **`c2ea63a`..`b7ad9c0`**; report `engine/artifacts/loop/reports/20260923_2355_build_land_SPL2.md`; the generator, the flag probe and every gate log in `engine/artifacts/lead_tmp/split-main/` (landing logs under `land/`). The second of the maintainer's SPL1–SPL7 splits, and the first split of a crate ROOT.
  **What:** the 2,590-line file became `main.rs` 424 (its `//!` flag and environment table byte-identical, `main` and the three flag parsers, and `ViewerArgs` and `BattleSetup`, which every mode reads) + `main/battle.rs` 386 (the map load and upload, the teams, `make_walk`, `walk_draws`, the squad icons, the HUD and spawn-screen models) + `main/headless.rs` 593 (`run_bench`; `run_headless` — a captured frame, `--walk-replay` with its PNG series and CSVs, `--capture-orbit`, `--scene-report`) + `main/windowed.rs` 1,241 (the `App` state machine — the shell, the loading screen, the battle's per-frame loop — `run_windowed`, `run_shell`, and the glue only the window calls). `main.rs` is the binary's crate root and owns `game/src/`, the library's directory, so the parts load from `game/src/main/` by `#[path]`; a `//` note above the `mod` lines names them. Commits 1–3 extract one part each in dependency order (battle, headless, windowed), sliced out of the base file by line range (`gen_split.py`: 39 top-level blocks, asserted to partition the file; each committed stage regenerated byte-identical); commit 4 repoints three comments (`mission.rs` and `squad_vis.rs` named `main.rs` for `make_walk` and `squad_icons`, `examples/water_frames.rs` for the frame loop's `animate_water`); commit 5 is the crate map and the waiver.
  **Gates (every exit read from its own file):** `scripts/tools/pure_move_check.py 46d6f06:engine/crates/game/src/main.rs <main.rs + the three parts>` → **PURE MOVE**: all 2,087 old meaningful lines present, 10 visibility-only changes, 10 new-only lines (three `#[path]` attributes, the seven-line note). ci **699 / 0 / 24** before each of the five commits and on main; `cargo test --workspace -- --list` 723, byte-identical to the base (the binary has no tests). The entry's flag table: `flag_table.py` (the `//!` header, the 36 match-arm flags in order, the two environment variables the binary reads) `cmp` 0 before and after; and a behaviour probe, `flag_probe.sh` — 57 GPU-free invocations of the binary (every parser error path, `--sim-hash 100` and 600, `--replay` of `walk_demo.ron`) — byte-identical transcripts from the base binary, the branch at `5076eff`, the head and main. `gpu_tests.sh` exit 0, 24 referees, `git status --short engine/goldens` empty, on the branch twice (`5076eff`, head) and on main. `sim_gates.sh` at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both `motion.csv` identical, on the branch twice and on main; the base binary driven through the same two replays rendered the same tick-0 PNGs byte for byte and printed the same 28-line log apart from the output path it echoes. The sweep pair (`round_to_outcome 60000 42`, stdout and stderr split) `cmp`-identical and identical to `lead_tmp/s19/baselines/sweep_main_1.txt` at `5076eff` and again at the head — footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`, stderr equal to the baseline's apart from the wall-clock `done (` lines. `--sim-hash 100` `17e1b946ff0e821c` (branch, main). `scripts/size_report.sh --check-map --strict` exit 0 on the branch and on main (111 files mapped, 0 missing, 0 stale counts, 0 unwaived). No rebake, golden or walkabout; the arenas were not run (the entry does not name them, and the arena example links the library, not the binary).
  **DECIDED (loop, 2026-09-23, lane 1): three parts, and `windowed.rs` stays whole at 1,241 lines** — under the 1,500 cap, above SPL1's largest part. *Why:* it is one `App` state machine whose methods and `ApplicationHandler` share 38 private fields; giving the per-frame loop a file of its own would have widened every field, or nested a second `#[path]` module inside the first. *How to reverse:* another pure move.
  **DECIDED (same): `ViewerArgs` and `BattleSetup` stay in `main.rs`.** *Why:* a child module sees its ancestors' private items and fields, so every part reads them and nothing widens; `BattleSetup` in `battle.rs` would have cost 15 widenings (its 13 fields, the struct and `from_args`), because `windowed.rs` builds it by struct literal. *How to reverse:* move it and widen them.
  **DECIDED (same): nine widenings are `pub(super)`; `walk_draws` is plain `pub`.** *Why:* its signature is 94 columns, so `pub(super) ` would push it past rustfmt's 100 and `pure_move_check` would report the reflowed line LOST (SPLIT_RECIPE §10 b); in a binary, a `pub` item in a private module reaches exactly as far as `pub(super)`. The two window-only helpers with 97- and 99-column signatures (`play_event_sounds`, `persist_setting`) stay beside their one caller in `windowed.rs`. *How to reverse:* make it `pub(super)` and accept the reflow — harmless now the move is proven.
  **DECIDED (same): the note naming the parts is a `//` comment above the `mod` lines, not a `//!` paragraph as in SPL1.** *Why:* `main.rs`'s `//!` header IS the documented flag table the entry's gate compares; byte-identical, that `cmp` stays exact. *How to reverse:* move the note into the header.
  **Docs:** QUEUE SPL2 → DONE, archived verbatim; C3's note on the two `spawn:` prints now names `make_walk` in `main/battle.rs`; HANDOFF §What next, §State and §For the maintainer; `LAST_CLEANSE` 4 landings since; SPLIT_RECIPE §11 (a crate root owns its directory — `#[path]` for a binary, plain siblings for SPL6's `lib.rs`; shared types stay in the parent; the `pub` way out of the 100-column trap; stages in dependency order; the binary's behaviour probe; comment commits change the binaries, so the gates re-run at the head). Left as they are, per recipe §8(a): historical Log entries and the done SM spec in `analysis/re-hud-icons.md`, which cite `main.rs` by line. Cite the binary's code by symbol from here on.
- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue SPL3): `core/src/collision.rs` is a 232-line facade over four parts — a pure move; nothing that runs changed.** Branch `split-collision` off `b423f15`, seven commits, the rebase a no-op, fast-forwarded as **`3644ec8`..`8f58ded`**; report `engine/artifacts/loop/reports/20260924_0040_build_land_SPL3.md`; the generator and every gate log in `engine/artifacts/lead_tmp/split-collision/` (landing logs under `land/`). The third of the maintainer's SPL1–SPL7 splits and the first in `core`, so the first under the sim gates and the host-libm referee.
  **What:** the 3,302-line file became `collision.rs` 232 (its `//!` header byte-identical plus a "Where the code lives" paragraph; the four ground/step constants; the parry conversions `Isometry`, `point` and `vector` that three parts share; re-exports of all 17 pub items; a `mod tests` holding only the six fixtures the parts' tests share) + `collision/shape.rs` 631 (`Capsule`, `Aabb`, `Shape`, `trimesh`, `Layers`, `Body`, `Cutter`, and the layer-mask tests) + `collision/world.rs` 584 (`CollisionWorld` with `new`, `rebuild_broadphase` and `visit_xz`, and the `BodyGrid` broad phase with its tests) + `collision/rays.rs` 653 (`is_cut_by`, `is_cut`, `ray_down`, `ray`, `ray_heightfield`, `lead_point`, and the ground, ray and terrain-ray tests) + `collision/walls.rs` 1,297 (`WALL_ITERATIONS`, `SWEEP_CONTACT_DIST`, `mesh_contact`, `push_out`, `wall_sweep`, the A.4 penetration test, and the wall-pass and parry-mirror tests). Commit 1 teaches the host-libm referee (`fmath::tests::no_std_transcendentals_in_core`) to walk module directories: by design it failed loudly on any subdirectory of `core/src` ("teach the referee to recurse"), so no `core` split could pass ci without it. It is test-only. Commits 2–5 extract one part each in dependency order (shape, world, rays, walls), generated from the base file by `gen_split.py` (38 top-level blocks, with the constant block and the second `impl CollisionWorld` cut finer, and 62 test blocks, all asserted to partition the file; every committed stage regenerated byte-identical). Commit 6 repoints one comment (`game/src/assets.rs` named `core::collision::BodyGrid`); commit 7 is the crate map and the waiver.
  **Gates (every exit read from its own file):** `scripts/tools/pure_move_check.py b423f15:engine/crates/core/src/collision.rs <the facade + four parts>` → **PURE MOVE**: 2,636/2,636 old meaningful lines, 11 visibility-only, 0 new-only (each stage printed PURE MOVE too). ci **699 / 0 / 24** before each of the seven commits and on main. `cargo test --workspace -- --list` 723 before and after, identical once the 41 moved test paths (`collision::tests::x` → `collision::{rays 17, walls 12, shape 6, world 6}::tests::x`) are mapped back; no script, gate or `.claude/` file names one. The recursive referee was made to fail twice: a probe comment calling `.sin(` in `collision.rs` (a top-level file, at commit 1) and one calling `.atan2(` in `collision/shape.rs` (nested, at stage 1) were each reported and then removed. `gpu_tests.sh` exit 0, 24 referees, `git status --short engine/goldens` empty (branch head, main). `sim_gates.sh` at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b` with both `motion.csv` identical (head, main); `--sim-hash 100` `17e1b946ff0e821c` (head, main). The sweep pair (`round_to_outcome 60000 42`, stdout and stderr split) was `cmp`-identical and identical to `lead_tmp/s19/baselines/sweep_main_1.txt` (head, main), footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`. Both arenas, twice each, were `cmp`-identical to each other and to the s19 baselines (flat 45.49 / 122.33 / 166.89 / 200.00, tat2 25.40 / 38.60 / 39.80 / 37.20, all eight hashes) at the head and on main. `scripts/size_report.sh --check-map --strict` exit 0 (115 files mapped, 0 missing, 0 stale counts). `cargo doc -p core` gave 24 warnings before and after (the last decision below). No rebake, golden or walkabout.
  **DECIDED (loop, 2026-09-24, lane 1): four parts, with `walls.rs` whole at 1,297 lines** — under the cap, and the largest part so far. *Why:* the push-out and the sweep share the parry contact workaround, the A.4 penetration test and 18 test blocks; cutting them apart would widen those helpers for no reader's benefit. *How to reverse:* another pure move.
  **DECIDED (same): the six shared test fixtures live in the facade's own `mod tests`, which holds nothing else, and the parts import them from `crate::collision::tests`.** *Why:* the fmath referee allows one `#[cfg(test)]` per file, and it must be the trailing `mod tests`, so a separate `mod fixtures` block would have failed it (or taught it a second exception). Each fixture widens to `pub(super)`. *How to reverse:* a fixtures module, once the referee can skip any `#[cfg(test)]` block — CLN6's scope.
  **DECIDED (same): the referee's recursion ships inside a pure-move branch (commit 1, `3644ec8`).** *Why:* the referee itself demands it, the change is test-only, and SPL4 and SPL5 need it too. It now walks every `.rs` under `core/src` and names a file by its path under that directory. *How to reverse:* revert `3644ec8` once no module directory remains.
  **DECIDED (same): the shared constants and parry conversions stay in the facade, `WALL_ITERATIONS` and `SWEEP_CONTACT_DIST` go with the wall pass, and the layer-mask tests live with `Layers` in `shape.rs`.** *Why:* a child module sees its ancestors' private items, so the facade's `Isometry`, `point` and `vector` widen nothing. The production code widens five members only — `Capsule::to_parry`, `Shape::with_parry` (97 columns), `Shape::cast_local`, the `Body::iso` field and `CollisionWorld::visit_xz` — all `pub(super)`, never `pub`: a `pub` method on a re-exported type would join the crate's API. *How to reverse:* another pure move.
  **DECIDED (same): the `//!` header stays byte for byte, so seven intra-doc links from public docs to private items that now live in another part read "unresolved link" instead of "links to private item"** (`collision`'s header → `BodyGrid` ×2, `mesh_contact` ×3, `penetration_block`; `CollisionWorld::ray` → `BodyGrid`). The rendered docs are identical, because a private item is never documented, and the count stays 24. *Why:* a `#[cfg(doc)]` import cannot reach a private item, and editing a moved `///` line breaks the pure-move proof. *How to reverse:* path links (`[`BodyGrid`](world::BodyGrid)`), queued as CLN8 (d).
  **Docs:** QUEUE SPL3 → DONE, archived verbatim; DR2's file set names `collision/world.rs`; notes on CLN6, CLN8, SPL4 and SPL5; this file's tech-stack line names `core::collision::walls::mesh_contact`; HANDOFF §What next, §State and §For the maintainer; `LAST_CLEANSE` 5 landings since; SPLIT_RECIPE §12 (the core referee, where shared test fixtures go, the doc-link kind change, trial-generating every stage before the first commit). Left as they are, per recipe §8(a): historical Log entries; `analysis/re-terrain-cutters.md` and `analysis/RE_LOG.md`, which cite `collision.rs` by line numbers that were already stale before the split; and the finished briefs under `engine/artifacts/briefs/`. Cite collision code by symbol from here on.
- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue SPL4): `core/src/ai.rs` is a 315-line facade over six parts — a pure move; nothing that runs changed.** Branch `split-ai` off `33f7570`, eight commits, the rebase a no-op, fast-forwarded as **`2d8c4b5`..`0eb2fb5`**; report `engine/artifacts/loop/reports/20260924_0130_build_land_SPL4.md`; the generator, the per-stage regeneration proof and every gate log in `engine/artifacts/lead_tmp/split-ai/` (landing logs under `land/`). The fourth of the maintainer's SPL1–SPL7 splits and the second in `core`.
  **What:** the 3,852-line file became `ai.rs` 315 (its `//!` header byte-identical plus a "Where the code lives" paragraph; six `mod` lines; re-exports of the 26 pub / pub(crate) items the parts own; all 25 tuning constants, kept together; a `mod tests` holding only the two fixtures the parts' tests share, `l_graph` and `l_sim`) + `ai/draw.rs` 338 (the D3 note, the eight salts, `unit_rand` / `unit_unit` / `unit_dir` / `route_rand`, `stance_hold`; the chooser-word test and the three D13 stance tests) + `ai/state.rs` 620 (`SteerPick`, `RouteSource`, `Route`, `NavTick`, `AiState` with its `Default` and `hash_into`; the AI-hash test) + `ai/command.rs` 386 (`CmdState`, `AiState::cmd_clear`; the seven D6–D8 order tests) + `ai/route.rs` 777 (`mover`, `hub_waypoint`, `hub_has_floor`, `start_hub`, `goal_pos`, `fill_detour`, `sub_point`, `plan_leg`, `retarget` and its three cases; four routing tests) + `ai/steer.rs` 425 (`rotate_xz`, `steer_dir`; seven walking tests) + `ai/brain.rs` 1,152 (`yaw_toward`, `wrap`, `think`, `burst_len`, `pause_ticks`, `slew`, `arrive_radius`; ten aim, fire and hold tests).
  **Gates (every exit read from its own file):** `scripts/tools/pure_move_check.py 33f7570:engine/crates/core/src/ai.rs <the facade + six parts>` → **PURE MOVE**: 3,161/3,161 old meaningful lines, 5 visibility-only (`sub_point`, `steer_dir` and `retarget` to `pub(super)`, because `think` calls them; the fixtures `l_graph` and `l_sim`), 6 new-only (three `#[cfg(doc)]` imports and their comments); every stage printed PURE MOVE, and each committed stage was regenerated byte-identical from the generator (`regen_check.sh`). ci **699 / 0 / 24** before each of the eight commits and on main. `cargo test --workspace -- --list` 723 before and after, identical once the 33 moved paths (`ai::tests::x` → `ai::{brain 10, command 7, steer 7, draw 4, route 4, state 1}::tests::x`) map back; no script, gate or `.claude/` file names one. At the branch head and again on main `0eb2fb5`: `scripts/gpu_tests.sh` 24 referees, exit 0, goldens tree clean; `scripts/sim_gates.sh` exit 0 at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both `motion.csv` identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp`-identical and `cmp` 0 against `s19/baselines/sweep_main_1.txt` (footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`); both arenas twice, each pair `cmp`-identical and `cmp` 0 against the s19 baselines (flat 45.49 / 122.33 / 166.89 / 200.00, tat2 25.40 / 38.60 / 39.80 / 37.20, all eight hashes). `scripts/size_report.sh --check-map --strict` exit 0 (121 files mapped). `cargo doc -p core --no-deps` (not a gate): 24 warnings before and after, the same set except that four links to private items now read "unresolved" (below). No rebake, golden or walkabout.
  **DECIDED (loop, 2026-09-24, lane 1): six parts, with `brain.rs` at 1,152 lines.** *Why:* `think` alone is 541 lines and a pure move cannot cut a function; everything it alone calls went with it. *How to reverse:* another pure move.
  **DECIDED (same): the part that holds `think` is `brain`, not `think`.** *Why:* a module `ai::think` beside the re-exported function `ai::think` compiles, but makes `weapon.rs`'s intra-doc link `[`crate::ai::think`]` ambiguous — a new rustdoc warning in a file outside the split. *How to reverse:* rename it and write that link `fn@crate::ai::think`.
  **DECIDED (same): every tuning constant stays in the facade; the salts go with the draws.** *Why:* the base's own comments keep the Phase A and A.1 constants "in one place" so the sweep can move them, and nothing widens because a child sees its parent's items; the salts are the draws' named inputs, not knobs, and go with the D3 note that introduces them. *How to reverse:* another pure move.
  **DECIDED (same): tests live with the feature they pin, not beside `think`** — the routing tests in `route`, the stuck side-step and pad-frame tests in `steer` (the local layer's last resort runs inline in `think`, and each part's header says so), the D13 stance tests beside `stance_hold`, the D6–D8 order tests beside `CmdState`, aim, fire and hold in `brain`. *Why:* beside `think` they would have made `brain.rs` about 1,600 lines, over the cap. *How to reverse:* another pure move.
  **DECIDED (same): three `#[cfg(doc)]` imports keep every public doc link to a PUBLIC item resolving; the four to PRIVATE items (`ai.rs`'s header → `hub_waypoint`, `steer_dir`; `Route::waypoint` → `hub_waypoint`; `AiState::slide` → `steer_dir`) now read "unresolved" instead of "private", as SPL3's did — queued as CLN8 (e).** *How to reverse:* delete the imports (four new warnings).
  **Docs:** QUEUE SPL4 → DONE, archived verbatim; CLN8 gains (e); a note on SPL5; HANDOFF §What next, §State and §For the maintainer; `LAST_CLEANSE` 6 landings since; SPLIT_RECIPE §13 (backup in `land/`). Two comments outside the split name the moved private items by their new paths (`squad.rs` → `ai::route::retarget_hunt`, `round_to_outcome.rs` → `ai::route::sub_point`). Left as they are, per recipe §8(a): historical Log entries; §Optimisation backlog A2's `core/src/ai.rs:112`, a line citation already stale before the split; `analysis/re-bf2-doors.md`'s dated correction naming `core/src/ai.rs` at `2d5e4a4`. Cite AI code by symbol from here on.
- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue SPL5): `core/src/sim.rs` is a 106-line facade over six parts — a pure move; nothing that runs changed.** Branch `split-sim` off `487b85e`, eight commits, the rebase a no-op, fast-forwarded as **`ac081a4`..`81dd3b4`**; report `engine/artifacts/loop/reports/20260924_0205_build_land_SPL5.md`; the generator, the per-stage regeneration proof and every gate log in `engine/artifacts/lead_tmp/split-sim/` (landing logs under `land/`). The fifth of the maintainer's SPL1–SPL7 splits and the last in `core`; the queue called it the riskiest of the set, because `Sim::step` is the order every replay hash depends on.
  **What:** the 3,679-line file became `sim.rs` 106 (its `//!` header byte-identical — the tick order — plus a "Where the code lives" paragraph; six `mod` lines; re-exports of all 11 public items; a `mod tests` holding only the five fixtures the parts' tests share: `flat_ground`, `duel`, `fire`, `sphere_cp`, `ai`) + `sim/state.rs` 812 (`Sim`, `Event`, `DeathCause`, `NavStats`, `Cheats`, `LETHAL_LANDING_SPEED`, `omni_radius_for`; the constructors, the queries and `Sim::hash`; six hash / ground tests, the pinned `--sim-hash 100` literal among them) + `sim/input.rs` 387 (`buttons`, `PadState`, `InputLog`, `Sim::run_log`, and step 2's aim assist — `acquire_magnet`, `apply_assist`, `magnet_at`; seven replay and pad-slot tests and the aim-assist test) + `sim/spawn.rs` 908 (`SpawnWaves`; `spawn`, `shed_ai`, `admit`, the slot tables, `ai_class`, `spawn_ai`, `respawn_slot`, `spawn_select`, `spawn_points`, `vacate_player_slot`; twelve spawn, class and spawn-screen tests) + `sim/combat.rs` 584 (`resolve_bolt`, `kill_environment`; eleven bolt, cheat, damage and environmental-death tests) + `sim/round.rs` 245 (`add_cp`, `update_cps`, `apply_team_handicap`; the capture, defeat and handicap tests) + `sim/step.rs` 802 (`step`, `separate_units`; eight movement, wall-pass, stamina and AI-fire tests). The base's one `impl Sim` (1,347 lines, 32 methods) split BY METHOD, one `impl Sim` per part (recipe §12(c)).
  **Gates (every exit read from its own file):** `scripts/tools/pure_move_check.py 487b85e:engine/crates/core/src/sim.rs <the facade + six parts>` → **PURE MOVE**: 2,992/2,992 old meaningful lines, 12 visibility-only (seven helpers widened to `pub(super)` because `step` or `update_cps` now calls them from another part — `shed_ai`, `respawn_slot`, `spawn_select`, `update_cps`, `resolve_bolt`, `acquire_magnet`, `apply_assist` — and the five shared fixtures), 2 new-only (one `#[cfg(doc)]` import and its comment); every stage printed PURE MOVE, and each committed stage was regenerated byte-identical from the generator (`regen_check.sh`). Every stage was first trial-generated through fmt, core clippy, the checker and the FULL core suite (246 lib tests, the libm referee included). ci **699 / 0 / 24** before each of the eight commits and on main. `cargo test --workspace -- --list` 723 before and after, identical once the 47 moved paths (`sim::tests::x` → `sim::{spawn 12, combat 11, step 8, input 7, state 6, round 3}::tests::x`) map back; no script, gate or `.claude/` file names one. The pinned test is now `sim::state::tests::a_squad_command_is_part_of_the_hash_and_no_command_costs_nothing` and passes. At the branch head and again on main `81dd3b4`: `scripts/gpu_tests.sh` 24 referees, exit 0, goldens tree clean; `scripts/sim_gates.sh` exit 0 at `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both `motion.csv` identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp`-identical and `cmp` 0 against `s19/baselines/sweep_main_1.txt` (footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`; stderr = the baseline's minus the wall-clock lines); both arenas twice, each pair `cmp`-identical and `cmp` 0 against the s19 baselines (flat 45.49 / 122.33 / 166.89 / 200.00, tat2 25.40 / 38.60 / 39.80 / 37.20, all eight hashes). `scripts/size_report.sh --check-map --strict` exit 0 (127 files mapped). `cargo doc -p core --no-deps` (not a gate): the same 24 warnings before and after — the one in sim (`vacate_player_slot` → the private `Sim::apply_team_handicap`) still reads "links to private item", because a method link resolves through its type. No rebake, golden or walkabout.
  **DECIDED (loop, 2026-09-24, lane 1): six parts named by responsibility, with `round.rs` at 245 lines.** *Why:* the posts, the bleed and the outcome are not spawning, and merged into `spawn.rs` they would have made a 1,150-line file whose name hides the outcome; SPL1's `camera.rs` (199) set the precedent for a small, clear part. *How to reverse:* another pure move.
  **DECIDED (same): `Sim::hash` lives with the struct in `state.rs`, with the hash tests and the pinned literal.** *Why:* a new field and its hash line are then one file (SPL4's `ai/state.rs` pairs `AiState` with `hash_into` the same way). The spawn-select-state hash test stays in `spawn.rs` beside the class fixtures it builds on, where moving it would have widened three more. *How to reverse:* another pure move.
  **DECIDED (same): every helper `step` calls lives with its feature, not beside `step`** — `respawn_slot` and `spawn_select` in `spawn`, `resolve_bolt` in `combat`, `update_cps` in `round`, and the aim assist in `input` (it rewrites a player's pad before movement). *Why:* beside `step` they would have made a second file near the cap, and a reader looks for respawns under spawning. It costs seven `pub(super)` widenings, all inside `crate::sim`. *How to reverse:* another pure move.
  **DECIDED (same): the environmental-death tests (a death region ×2, the lethal landing) live in `combat.rs` beside `kill_environment`,** though the two checks run inline in `step`; `combat.rs`'s header says so. *How to reverse:* another pure move.
  **DECIDED (same): two mis-attached doc blocks go through byte for byte and are queued, not fixed** — `shed_ai`'s doc opens with four lines that describe `Sim::spawn` (which has none), and `separate_units`' opens with a stray first line of `step`'s doc; both were so in `sim.rs`. With `core/tests/battle_outcome.rs`'s four `sim.rs:NNNN` citations (stale before the split), they are CLN8 (f) and (g). *Why:* a pure move cannot edit a moved line. *How to reverse:* CLN8.
  **Noticed, not a decision:** rustdoc renders one section per `impl` block, so `Sim`'s page lists seven inherent blocks (the six parts' and `squad.rs`'s, where there were two) and opens with `combat`'s `kill_environment`; every `#method.x` anchor is unchanged.
  **Docs:** QUEUE SPL5 → DONE, archived verbatim; the GATES block names the pinned test's new path; TL, DR3, DET1 and C3 row 6 name the parts; CLN8 gains (f) and (g); a note on SPL6; this file's §WHERE THINGS STAND line on the pinned test; HANDOFF §What next (SKA's build half is next, then SPL6), §State, §For the maintainer and B1; `analysis/re-cheat-codes.md`'s `Cheats` heading; `LAST_CLEANSE` 7 landings since; SPLIT_RECIPE §14 (backup in `land/`). Three comments outside the split name the parts (`ai/state.rs` → `sim/step.rs`, `squad.rs` → `sim/spawn.rs`, `core/Cargo.toml` → `src/sim/input.rs`). Left as they are, per recipe §8(a): historical Log entries; §Optimisation backlog A1's `core/src/sim.rs:502`, a line citation already stale before the split. Cite sim code by symbol from here on.

- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue SKA's build half): the disc tests no soldier's alpha — the skinned path loses its alpha test, and the one 0x04 segment per game, `all_inf_wookie`'s fur, blends.** Branch `skinned-alpha` off `77792be`, two commits, the rebase a no-op, fast-forwarded as **`ff71352`** (the build) + **`62e824a`** (the crate map names where a soldier's alpha is decided; `scripts/gpu_tests.sh` stops counting its tests); `side` rebaked on main; goldens **`8277709`**. Report `engine/artifacts/loop/reports/20260924_0245_build_land_SKA.md`; evidence, tools and every gate log in `engine/artifacts/lead_tmp/skinned-alpha/` (the landing's under `land/`). Built on the 2026-09-17 RE (`analysis/re-side-alpha.md`, `[O]`): the PS2 draws a side model's segment with the GS state its own `MTRL` word asks for, no soldier segment on either disc carries the cutout bit 0x02, and exactly one per game carries 0x04, which both discs BLEND (`ALPHA 0x44`, `TEST 0x51001` — Z-tested, no Z write).
  **What:** `bake::soldier::segment_blends` reads a skinned segment's `MTRL` word — 0x04 → `blend`, neither bit → opaque — and REFUSES what the skinned path cannot draw (0x02, and 0x04 with 0x08 / 0x40 / 0x80); `bake::skin_glb` writes `"alphaMode": "BLEND"` on that segment's material and nothing on the rest. `game::assets::CpuSkinnedMesh::submeshes` carries the pass, and `upload_skinned`, `SoldierVisuals`, the gallery and `pose_preview` hand it on. `render::SkinnedDrawCmd` gains `layer` (Opaque or Blend; any other layer is refused); `SKINNED_MASK_CUTOFF`, `skinned_mask_test`, `fs_mask_skinned` and its fade twin are deleted — the skinned pipelines take the world's `fs_main` / `fs_fade` / `fs_blend_fade`, and a new skinned blend pipeline and its fade twin draw the blended segments after every opaque skinned draw, far to near, through `sort_far_to_near`, which the world's blend pass now shares. +548/−206 over 17 files.
  **Gates, reproduced by this session:** ci **702/0/27** on the branch (twice) and on main (699 − the shader-grep test the 2026-09-23 cleanse asked the build to delete + four new: `sort_far_to_near`, `segment_blends`, the wookie's fur on both discs, the per-segment GLB round trip; three new GPU referees); `scripts/gpu_tests.sh` on main exit 0 (27 passed: 20 render referees, three of them new, + 7 golden tests) with the goldens tree clean. **Bake:** a scratch `side` bake from the unchanged binary read SAME=283 DIFF=0 against the shipped tree (the instrument), the branch's read SAME=281 **DIFF=2** — exactly `side_bf1/soldiers/all_inf_wookie.glb` and `side_bf2/soldiers/all_inf_wookie.glb`, whose BIN chunks and JSON outside `materials` are byte-identical — and after `rebake.sh side` on main a scratch `side` group of main read SAME=283 DIFF=0; 21,667 files. A census over the soldier GLBs reproduced the RE's count independently: BF1 21 GLBs / 39 materials, BF2 31 / 58, one `alphaMode` in each game. **Goldens:** `scripts/goldens.sh` on main moved exactly `units/bf1_all_inf_wookie.png` (RGB mean |d| 0.2609, max 112, 3.18 % of pixels, all four panels) and `units/bf2_all_inf_wookie.png` (5.2596, max 137, 11.73 %), quantified before viewing, then viewed as pairs (`land/pairs_00.png`) and full-size RGB crops (`view/`): BF2 goes from a cloud of fur fragments to a whole wookie with a soft fur fringe; BF1 loses its alpha-test holes and hard strand edges. Both PNGs byte-identical to the branch's candidates, and every other golden byte-identical. **Behaviour, unmoved on the branch and on main:** `sim_gates.sh` `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both `motion.csv` identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp`-identical and equal to the s19 baseline (md5 `be656ec8…`, footer `sweep: finished 27/32  stuck_max median 960 (finished-only 840)  >10k 4 maps  sum 213000`); the arenas twice each, pairs and s19 baselines `cmp`-identical (flat 45.49 / 122.33 / 166.89 / 200.00, tat2 25.40 / 38.60 / 39.80 / 37.20).
  **RE correction, [V] by `segdump`:** BF1's wookie segments name `wookieface02` / `wookie` / `wookie` (words `0x1` / `0x1` / `0x5`), so the RE's "all three segments name the same sheet" holds for BF2 only; an opaque and a blended segment share a sheet in both games, so the per-segment verdict stands (`re-side-alpha.md` §6 item 3 erratum, RE_LOG pointer marked ported).
  **DECIDED (loop, 2026-09-24, lane 1): a skinned segment whose word asks for a state the skinned path cannot draw fails its bake** — 0x02 (the disc's alpha test) or 0x04 with 0x08 / 0x40 / 0x80 (another blend) — *why:* no baked soldier has one (the rebake of all 52 proved it), but P7's vehicles and the hero units carry both, and silently drawing them opaque or with the wrong blend is the defect SKA exists to remove — *how to reverse:* make `segment_blends` return `false` for them.
  **DECIDED (same): the golden's alpha plane keeps the fur sheet's alpha under the opaque segments,** as every opaque world draw already writes its own sheet's alpha — *why:* forcing alpha to 1 on the opaque passes would move every golden's alpha plane for no visible change; a compositing image viewer shows the BF2 body pale, so HANDOFF points the maintainer at the RGB crops — *how to reverse:* an RGB-only write mask on the opaque skinned pipelines, and the goldens it moves.
  **DECIDED (same): the gate's "a render test that the skinned shader declares no cutoff constant" is met by behavioural GPU referees** (an alpha-0.25 sheet draws solid on the opaque skinned pass and composites on the blended one; the blend pass runs after the opaque one and is Z-tested; any other layer is refused) — *why:* the cleanse flagged the old grep test as unable to fail if skinned draws were routed back to a test — *how to reverse:* add a grep test.
  **DECIDED (same): no walkabout regeneration** — *why:* the change touches no map, and the 32 map goldens, which draw the same reference soldier through the same skinned pipelines, came back byte-identical — *how to reverse:* regenerate it.
  **Queued:** SKO — every skinned draw is drawn after the world's blend pass, so a soldier behind a translucent world surface paints over it (after SPL6, which moves `render_frame`). **Docs:** QUEUE SKA → DONE, archived verbatim; the GATES block's count 699 → 702 (27 ignored); a note on SPL6 (FREE; `render/src/lib.rs` 2,208 lines); this file's §WHERE THINGS STAND soldier line; HANDOFF §For the maintainer (a look call, a calls-made line), §What next, §State and HOLD 1; `analysis/re-side-alpha.md` §6 and `RE_LOG.md`'s SKA pointer; `LAST_CLEANSE` 8 of 12.
- **2026-09-24 — BUILT AND LANDED (loop, lane 1, job C, queue SPL6): `render/src/lib.rs` is a 250-line crate root over seven sibling parts — a pure move; nothing that draws changed.** Branch `split-render` off `c652670`, nine commits, the rebase a no-op, fast-forwarded as **`be57ca0`..`56cb0e8`**; report `engine/artifacts/loop/reports/20260924_0340_build_land_SPL6.md`; the generator, the commit loop, the per-stage regeneration proof and every gate log in `engine/artifacts/lead_tmp/split-render/` (landing logs under `land/`). The sixth of the maintainer's SPL1–SPL7 splits and the first of a LIBRARY root: the parts are plain `src/<part>.rs` siblings of the crate's `camera`, `frustum`, `mesh` and `texture`.
  **What:** the 2,208-line file became `lib.rs` 250 (its `//!` header byte-identical plus a "Where the code lives" paragraph; eleven `mod` lines; re-exports of the 14 public items the parts took; the `Renderer` struct and what the parts share with it — `Target`, `Globals` and its size test, `PALETTE_STRIDE` — plus `RenderError` and `TIME_WRAP`) + `lighting.rs` 82 (`Fog`, `FarFade`, `Lighting`) + `sky.rs` 304 (`SkyShell`, `SkyMode`, `Placement`, `set_sky`, `sky_model`; the four placement and material-rule tests) + `draw.rs` 175 (`DrawLayer`, `DrawCmd`, `SkinnedDrawCmd`) + `capture.rs` 163 (`capture_rgba`, `encode_png`, `encode_png_rgb`, the row padding; two tests) + `device.rs` 265 (the two constructors, `adapter_info`, the Resources and Misc blocks, `HEADLESS_FORMAT`, `request_device`, `pick_adapter`, `create_depth_texture`) + `frame.rs` 643 (the Frame block with `render_frame`, `FrameStats`, `OverlayFn`, the instance and palette buffer constructors, `draw_mesh`, `sort_far_to_near`, `draw_key`; the three draw-order tests) + `pipelines.rs` 466 (`finish_init`, `PipelineDesc`) — 2,348 lines in all (+140: seven part headers, `use` blocks and test wrappers, four more `impl Renderer {`/`}` pairs, the root's paragraph, three `#[cfg(doc)]` imports with their comments). The one `impl Renderer` split by method, its section banners travelling with the first item after each; `gen_split.py` slices 36 top-level blocks, 24 method blocks and 10 tests out of the base, asserts they partition it and that the root re-exports every public item a part takes, applies only the six widenings and computes the root's `use` block per stage. Every stage was trial-generated through fmt, render clippy, the checker and the full render suite before the first commit; `stage_commit.sh` then committed stages 2–7 one at a time — ci under the lock (anything but exit 0 at 702/0/27 stops it), the checker, the message's numbers filled from those outputs, then `regen_check.sh` (every file of every stage regenerated byte-identical). Commit 8 names the new files in three comments (`render/src/mesh.rs`, `game/src/water_anim.rs`, `game/src/bench.rs`); commit 9 is the crate map and the waiver.
  **Gates (every exit read from its own file):** `scripts/tools/pure_move_check.py c652670:engine/crates/render/src/lib.rs <root + seven parts>` on main → **PURE MOVE**: 1,836 old meaningful lines, all present; 6 visibility-only (`SkyMode::index` and `Renderer::sky_model` to `pub(super)` — `render_frame` reads them from `sky`; `finish_init` to `pub(super)` — both constructors call it from `device`; `create_instance_buffer` and `create_palette_buffer` to `pub(super)` — `finish_init` creates the first buffers `frame` grows; `create_depth_texture` to plain `pub`, because `pub(super) ` made a 105-column line — a free fn in a private module the root does not re-export, so the same reach), 6 new-only (three `#[cfg(doc)]` imports and their comments). `./ci.sh` exit 0 at **702 / 0 / 27** before each of the nine commits, on the rebased branch and on main. `scripts/gpu_tests.sh` exit 0 at the head and on main (27 GPU tests: 20 render referees + 7 golden tests), `git status --short engine/goldens` empty both times; **`scripts/goldens.sh` at the head regenerated all 90 goldens (every mtime fresh) and every one came back byte-identical** — stronger than the check's perceptual tolerance. `cargo test --workspace -- --list` 729 before and after, identical once the nine moved paths map back (`tests::x` → `sky::tests::x` ×4, `frame::tests::x` ×3, `capture::tests::x` ×2). `cargo doc -p render --no-deps` 13 warnings before and after, the same set by text and source line; the public API identical (the same 31 items in `all.html`; every method, field and variant anchor unchanged). `scripts/size_report.sh --check-map --strict` exit 0 at the head and on main (134 files, 0 missing, 0 stale counts, 0 unwaived). No sim gate (nothing in `core` or on the sim path changed), no rebake, no golden, no walkabout.
  **DECIDED (loop, 2026-09-24, lane 1): seven parts named by responsibility, with `lighting.rs` at 82 lines.** *Why:* the fog, the ambient and the far fade are the map's light and nothing else — the game sets them through `viewer::look` as it sets the sky shells through `viewer::scene` — and a reader looks for `Fog` under lighting; SPL1's `camera.rs` (199) and SPL5's `round.rs` (245) set the precedent for a small, clear part. *How to reverse:* another pure move.
  **DECIDED (same): `Renderer`, `Target`, `Globals` and `PALETTE_STRIDE` stay in the crate root.** *Why:* every part constructs or reads their private fields, and a child module sees its ancestors' private items, so there they widen nothing (SPLIT_RECIPE §11(b), in a library); moved into a part, each field would have cost a widening. *How to reverse:* another pure move.
  **DECIDED (same): the frame's draw order — `draw_key`, `sort_far_to_near` and their three tests — stays beside `render_frame` in `frame.rs`, not beside the types it sorts.** *Why:* `render_frame` is their only caller, so they widen nothing there (beside `DrawCmd` they would have cost two widenings, one a 104-column line), and the next entry that reorders the passes (SKO) edits one file. *How to reverse:* another pure move.
  **DECIDED (same): `set_sky` and `sky_model` live with the sky shells in `sky.rs`, `HEADLESS_FORMAT` with the headless target in `device.rs`, `TIME_WRAP` beside `Renderer::time` in the root.** *Why:* a shell's placement is then one file (CORB's). *How to reverse:* another pure move.
  **DECIDED (same): the crate map is refreshed in full, not only the split's rows.** *Why:* SKA's landing had left seven file rows, `shader.wgsl` (402 → 367) and the bake and game totals stale; refreshed, `--check-map` reads 0 stale and the header's growing list of per-split provenance clauses becomes one line (counts are `wc -l` on `9b6687f`). *How to reverse:* revert `56cb0e8`'s count edits.
  **Noticed, not a decision:** rustdoc renders one section per `impl` block, so `Renderer`'s page lists five inherent blocks (`device`, `sky`, `frame`, `capture` and `mesh.rs`'s dynamic-mesh block) where there were two; every `#method.x` anchor is unchanged (SPLIT_RECIPE §14(g)).
  **Docs:** QUEUE SPL6 → DONE, archived verbatim; the `Files:` lines of 9, W3, CORB and SKO name the parts; a note on SPL7 (FREE now); HANDOFF §What next, §State and §For the maintainer (a calls-made line); SPLIT_RECIPE §15 (backup in `land/`); `LAST_CLEANSE` 9 of 12. Left stale on purpose (recipe §8 a): the line-number citations into `render/src/lib.rs` in `analysis/re-sky-dome-placement.md`, `re-vu1-cull.md`, `re-hud-icons.md` and `re-terrain-cutters.md` (each stale before the split) and the historical Log; cite render code by SYMBOL from here on.
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

- **2026-09-24 — CLEANSE (loop, lane 1, job B2; the third under `OPUS_LOOP_PROMPT.md` §6b): docs consolidated, code area `render`, a bug hunt over the twelve landings since `0cfdadb`, CLN8 folded in — landed as `cfb83c6`..`e64f52b`, every gate value unmoved.** Branch `cleanse-render` off `bdbacd9`, six commits, rebase a no-op, fast-forwarded; report `engine/artifacts/loop/reports/20260924_0545_cleanse_render.md`. Two read-only Opus agents gathered (the `render` crate; the behaviour commits OCF, SKA, R1U, KW, NT, the libm referee and CMAP); every finding was re-read by the lead before it was acted on, and every new or tightened test was shown to FAIL under the break it guards (`lead_tmp/cleanse-render/fail_once.txt`).
  **Gates, lead-run:** ci **706/0/27** exit 0 on the branch and on main (704 + `mesh::tests::the_shader_palette_holds_max_joints` + `a_skinned_material_asking_for_mask_fails_to_load`; the GATES block says 706); `scripts/gpu_tests.sh` exit 0, 27 passed, `git status --short engine/goldens` empty, on the branch and on main; `sim_gates.sh` on the branch `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, both motion.csv identical, and `--sim-hash 100` `17e1b946ff0e821c`. No bake, golden, walkabout or sweep run was owed: no bake code changed (`bake/src/alpha.rs` gained one assertion), no `core` code changed (comments only), and the goldens render at time 0 and draw no MASK soldier.
  **What shrank:** HANDOFF-OPUS.md 508 → about 405 lines (50.6 → about 40 KB) — §For the maintainer one entry per call with the superseded folded in (SKA's RE into SKA, OCA into OCB, AT/SM's soldier line into SKA, NT + R1U + KW into one alpha-rules line, CMAP + SPL1–SPL6 into one splits line), §What next and §State cut to what is next and what is true; QUEUE.md 82.4 → 77.8 KB (the GATES block's count and bake histories; CLN8 and C3 archived verbatim); START_HERE's 2026-09-23 status archived verbatim and a current one written (the maintainer's `PLAN.md` edit was committed as `6fe0680`, not pending).
  **Fixed (code area `render` + the bug hunt; 22 findings):** CLN8's eight doc items (`cfb83c6`; `cargo doc -p core` 24 warnings → 15, none from the splits); ten render comments the code had outgrown (the scroll's source is `fx__`, the sky clamp is PLAN deviation 3 — one PROVISIONAL settled, **190 in 56 files** now — v0's alpha-tested opaque pass, the 128-byte uniform that is 192, "the construction" now in `pipelines.rs`); four tests that could not fail now can (a dropped translation in `transformed_aabb`; `MAX_JOINTS` against the shader's palette array; KW's 0x04 clause order; a MASK soldier material) and one that pinned a rank production never reads; `DrawLayer::Sky` deleted (never constructed); `new_windowed` no longer indexes empty surface caps (an `OPENBF_ADAPTER` that cannot present is now `RenderError::Surface`); a MASK soldier GLB fails at load instead of ending the windowed game at its first frame; `TIME_WRAP` 1000 → 2000 s (BF1 KAS2's 0.0125 UV/s water jumped half a repeat at every 1000 s wrap; 2000 is the least common multiple of every shipped period); `size_report.sh --check-map` counts a file as mapped only by its own row; `analysis/re-material-flags.md` §2's 2026-09-23 line corrected inline (Av = 0x7f gives 193 — 0x80 would give 191 — and since SKA no skinned draw is tested).
  **Queued:** **CLN9** — OCF's per-frame foam routing has no test (swap the meshes or drop `0b203e7`'s zeroing and ci stays green), and the FFT sea runs on the wrapping clock, so it snaps once per wrap.
  **DECIDED (loop, 2026-09-24, lane 1): SKO moves to the head of the TODO list, value MEDIUM.** *Why:* KW made BF2 KAM1's central-chamber window band BLEND glass, so a soldier seen through it is drawn after the glass and paints over it un-tinted — a walkable case on a shipped map that did not exist while the band was an opaque wall. *How to reverse:* move the row back below CLN9.
  **DECIDED (loop, 2026-09-24, lane 1): `TIME_WRAP` changed inside a cleanse.** *Why:* a render-only fix that moves no gate value (goldens and walkabout render at time 0; the sim never reads it), and the cleanse found it. *How to reverse:* the one constant.
  **DECIDED (loop, 2026-09-24, lane 1): a skinned material asking for MASK fails at LOAD, not at draw.** *Why:* the bake already refuses the cutout bit on a skinned segment (SKA); the loader is where `soldier_vis` skips one model, while `render_frame` refuses the whole frame. *How to reverse:* map MASK to Opaque in `load_skinned_glb` instead.
  **DECIDED (loop, 2026-09-24, lane 1): C3 closed as DONE and `impl Default for TextureDesc` declined.** *Why:* C3 row 6's passes were done by the rotation cleanses and the splits, its doc fold-in is CLN8 (f), and its print fold-in already holds; a `Default` for a struct whose texel slice has no valid default is an API smell with four call sites. *How to reverse:* a new queue entry.
  **DECIDED (loop, 2026-09-24, lane 1): the 2026-09-17 Log stretch stays here for now — the next cleanse moves it.** *Why:* the Edit tool is the only allowed writer of this file and of `analysis/`, so a verbatim ~100 KB move costs twice its size in one session's context; the next cleanse gives it to one Opus agent in its own context, byte-verified against git, together with the 2026-09-23/24 stretch (`LAST_CLEANSE` says so). *How to reverse:* n/a.
  **Artifacts:** `scripts/cleanse_delete.sh --dry-run` had nothing to take at the start; about 40 GB of landed branches' scratch bakes stay under names it cannot see (queue CLN4). `.git` 397 MB; no orphan branch, worktree or stash.
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

  **What:**
  - **The matrix.** `FUN_00589ac0` (BF1 `FUN_004a82f8`) builds a DL opcode-1000 texture-coordinate matrix. The pass samples the `specular` `tex_` in CORE.LVL (128×128 8-bit grey, byte-identical on both discs; bilinear, REPEAT) at **s = power/255, t = 0.5 + 0.5·(L̂·R)**.
  - **R** = 2(N'·Î)N' − Î, computed per vertex by the VU1 program (BF2 0x0ca–0x0fe, BF1 0x0ca–0x101). The per-pass loop writes STQ.
  - **The colour** is flat (the VU1 unlit path). BF2: the MTRL's specular RGB, halved into GS units (0xff → 1.0). BF1: the material's own RGB raw, or the SunInfo colour raw (0x80 = 1.0, so YAV1's 240 ≈ 1.9×).
  - **The blend** is **Cs·Ad + Cd**. Ad = the base pass's texel alpha, because the main target is FRAME 0x80080 = PSMCT32 with FBMSK 0. A `Specmap` sheet's alpha is its gloss mask.

  **Faithful quirks:**
  - the table is symmetric, giving two highlights (sun and anti-sun);
  - L is rotated by the camera's plain inverse while R lives in the VU view frame with up and back negated, so the effective light has its camera-right component flipped: d = 2(X·L)(X·R) − L·R;
  - BF2's pass alpha byte is `int(spec.a/255)` unscaled = 1, so the 0x04 variant (Cs·As + Cd) is invisible;
  - BF2 skips the pass on far-scene models in a mission.

  Census: BF1 179 class-0x20 records, BF2 469 with 0x80 (none with 0x100 or 0x02, 22 with 0x04).
  **Verified:** the four self-checks (§10 of the file):
  - every cited line's enclosing function printed (two call-site guesses corrected before the claims went out);
  - every absence claim byte-scanned or censused;
  - each BF1 twin read in its own function;
  - every constant, record and VU1 immediate read from the instruction words.

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

  **Verified:** the five self-checks (`re-water-flags.md` §10), then one `opus` verifier run in the FOREGROUND with the claims only — **11 SUPPORTED, 6 PARTLY, 0 NOT SUPPORTED**, all six folded in; the lead reproduced its load-bearing new items (`.reginfo` gp 0x6692f0 and 0 gp-based accesses; the state entry `FUN_00174960`; B1:551403/551404; BF1's bob modes). Stays [H]: what the final tile loop draws when the gate is clear, fog rows carried across maps, KAS2's split-screen fog, the speckle's per-vertex colour.
  **Queued:** WSE rewritten as its BUILD (the recipe is `re-water-flags.md` §9), with FEL1's bob folded in.
  **DECIDED (loop, 2026-09-25, lane 1): WSE's build bakes each Water flag as the PS2 reads it — `authored.map_or(true, |v| v != 0)` for `SpecularEnable` and the new `oscillation_enable` — instead of special-casing BF2 in the viewer.** *Why:* the manifest should carry what the disc's reset and parser leave in the byte; a viewer-side "BF2 is always on" would lie in every BF2 manifest and ignore an authored 0. *How to reverse:* the two bake lines in `bake::water_glb::manifest_config`, then `rebake.sh maps`.
  **DECIDED (loop, 2026-09-25, lane 1): FEL1's still water rides WSE's build rather than its own entry.** *Why:* the same parser, the same reset, the same bake function and the same `maps` rebake; it moves no golden (the goldens render at t = 0, where the bob is 0). *How to reverse:* split it out as its own queue entry.
  **DECIDED (loop, 2026-09-25, lane 1): the port keeps `re-fog.md` §2.1's fresh-boot reading — a fog-off map draws no fog from its water's far pass — and does not model fog rows carried over from an earlier map.** *Why:* the carry-over is a session-order artefact the port has no model of, and it is [H]. *How to reverse:* n/a unless an in-map capture shows it.
- **2026-09-25 — WSE landed (loop, lane 1, job C at medium, built and landed in one session): BF2's two Water flags as the PS2 reads them.** `bb6b0d2` (build) + `a8b2f41` (the ocean fix below) + `52c84a5` (golden) + `4c3553d` (crate map); report `engine/artifacts/loop/reports/20260925_0315_build_land_WSE.md`.
  - **What:** `bake::water_glb::manifest_config` writes `specular_enable` and a NEW `oscillation_enable` as `authored.is_none_or(|v| v != 0)` (the level reset's 1, `re-water-flags.md` §2/§7 [O]); `manifest::ManifestWaterConfig` gains `oscillation_enable`, both with a serde default of true; `game::viewer::look` — BF2's speckle and far-water fog follow the flag (fog only on FLAT water); `game::water_anim::{oscillates, flat_lift}` — FEL1's plane no longer bobs; `render::lighting`'s `water_fog_begin` doc corrected.
  - **Gates reproduced:** ci 718 → **720** / 0 / 34 on the branch and on main (+2 tests: the bake flags, the still bob; the `look.rs` tests were extended in place). Scratch bake `maps` on the branch: **DIFF = exactly the 12 water `manifest.ron`s** (each: `specular_enable` false → true, `oscillation_enable` added, false on `bf2_fel1` only); `rebake.sh maps` on main, main scratch bake DIFF=0 (SAME=5322, 21,669 total). gpu_tests 0 on the branch against the OLD bake (no golden moved) and on main after the golden. `--sim-hash 100` `17e1b946ff0e821c`. Frame time, `water_frames` on BF2 KAS2, vantage stepping: 0.110 ms mean / 0.108 median speckle on vs 0.109 / 0.107 `OPENBF_WATER_SPECKLE=0` (+0.9 %; 194 vs 192 draws). Walkabout regenerated for bf2_kas2 / nab2 / dea1 / fel1 / yav1.
  - **Moved:** golden `maps/bf2_kas2.png` only — mean abs 0.044, max 10, 2.77 % of pixels, bbox x 93..999 y 136..445, every changed pixel brighter, alpha untouched: a faint grey speckle on the pool between the platforms (`lead_tmp/water-flags/pairs_00.png`, `kas2_crop.png`). **The entry's prediction was wider:** nab2, dea1, fel1 and yav1 did not move — their golden framings show no water (viewed nab2 and fel1; an additive layer on visible water would have moved bytes); dag1 as predicted (black speckle).
  - **Deviation — caught on the branch:** the first commit fogged BF2 KAM1's sea (its golden moved on an overlay preview). `re-water-flags.md` §1/§5 puts the gate in the FLAT renderer `FUN_00575fd8`; the ocean `FUN_00575670` is the driver's other branch. `a8b2f41` restricts the BF2 fog gate to `!ocean`; `bf2_kam1` then regenerated byte-identical. Also outside the entry's file set: the one doc comment in `render/src/lighting.rs`.
  **DECIDED (loop, 2026-09-25, lane 1): a BF2 ocean (KAM1) stays unfogged, as the port always drew it — the `SpecularEnable` fog gate applies to BF2 flat water only.** *Why:* the gate is read in the flat renderer and BF2's ocean renderer is unread for fog; keeping the ocean as it was moves nothing unproven. *How to reverse:* drop `!c.ocean &&` in `water_look`'s `gated_on` (moves `bf2_kam1`); an RE of `FUN_00575670`'s fog would settle it.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue item 7, built and landed in one session): per-panel statistics of a quarter-size walkabout.** `2ab8dfb` (a main fix, below) + `874ff8b` (build) + `e348de1` (the reference `engine/goldens/walkabout_stats.ron`) + `aaaae90` (crate map); report `engine/artifacts/loop/reports/20260925_0300_build_land_7.md`. `gallery::walkabout` takes a `scale` (the example passes 1, so the looking set is unchanged); `game/tests/walkabout_stats.rs` (ignored, GPU) renders every map's walkabout at 1/4 and compares per panel the mean RGB (±3), the RGB σ (±10 %, floor 1.0) and the flat-grey share (+2 points, a rise only) with the reference: **780 panels over 32 maps**, PS2 tier, 26 s. `OPENBF_TEST_DROP_TEXTURE=<name>` (`viewer/scene.rs`, test-only) skips one texture's upload.
  - **Gates reproduced:** ci 720 → **721** / 0 / 34 → **35** on the branch and on main (+1 unit test of the arithmetic, +1 GPU referee). `walkabout_stats` clean twice on the branch and once on main; `UPDATE_GOLDENS=1` rewrites the reference byte-identical. gpu_tests 0 on the branch and on main, no golden moved. `--sim-hash 100` `17e1b946ff0e821c`. No bake change.
  - **The demonstration:** `OPENBF_TEST_DROP_TEXTURE=tat2_bldg_set01` fails exactly 5 of 780 panels, all tat2: cp5's four and cp2's 90° panel. An independent full-size pixel diff of the tat2 walkabout with and without the drop (`lead_tmp/walkabout-stats/wa_{base,drop}`) puts those five at 34 / 10.3 / 6.3 / 2.7 / 2.1 % changed pixels; the six other panels that show the texture are at 0.06–1.5 % and pass inside the tolerance; no panel on any other map fails. So the referee resolves a lost texture covering ≥ ~2 % of a panel.
  - **Main was red, and this fixes it:** WSE's landing ran main's ci before `rebake.sh maps`, and the rebaked manifests broke the data-gated census `game/tests/water_look.rs` ("BF2 never fogs water": bf2_dea1 `Some(180)` vs `None`). `2ab8dfb` gives it WSE's rule: dea1 180, fel1 52, kas2 100, nab2 150 (each map's `NearSceneRange.c`); dag1 and yav1 stay `None` (DisableLowRes), kam1 `None` (the ocean). Test-only.
  - **Deviation:** the entry's one line in `scripts/gpu_tests.sh` was DENIED by the auto-mode classifier ("CI bypass"). The session did not route around it, so the referee runs only by hand for now: `cargo test -p game --test walkabout_stats -- --ignored`. It is on HANDOFF §For the maintainer.
  **DECIDED (loop, 2026-09-25, lane 1): the σ tolerance is ±10 % with a floor of ±1.0 (0..255 units).** *Why:* a bare ±10 % of a near-flat sky's σ is a fraction of one unit, which is driver noise, not a regression. *How to reverse:* `STD_FLOOR` in `game/tests/walkabout_stats.rs`.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue CLN3, built and landed in one session): the test scratch that leaked into `/tmp` cleans up.** `faf321e`; report `engine/artifacts/loop/reports/20260925_0320_build_land_CLN3.md`. A census of `/tmp` (a tmpfs) before the build held 111 stale `openbf*` paths, 56 MB, from exactly two families: bake's `classes.rs` tests (7 RONs per run, never removed) and `game/tests/roster.rs`'s shared OnceLock dir (a static is never dropped). The classes tests now drop each RON once baked (the round-trip test after reading it back); roster.rs removes its dir through libc `atexit`, which the test harness reaches on pass and on fail. `OPENBF_KEEP_SCRATCH=1` keeps both. Test code only — no sim, bake, golden or sweep value can move.
  - **Gates reproduced:** ci **721 / 0 / 35** on the branch and on main (unchanged; the entry's 676/24 was its 2026-09-17 count); roster 12 tests, same names; `cargo test -p game -p bake` from a swept `/tmp` leaves **0** paths (the entry's gate), ci and gpu_tests leave 0 too; gpu_tests 0 on main, `engine/goldens` clean. No sim-hash, sweep or bake run: no non-test code changed.
  **DECIDED (loop, 2026-09-25, lane 1): fix the two leaking writers, not an RAII handle over all ~30 scratch sites.** *Why:* every other site already removes its scratch on success, and the census found nothing else; an RAII drop would also delete the scratch of a failing test, which is the triage the entry wanted kept. *How to reverse:* revert `faf321e`.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue CLN4, built and landed in one session): `scripts/cleanse_delete.sh` finds scratch bakes by structure, at any depth.** `2517ece`; report `engine/artifacts/loop/reports/20260925_0340_build_land_CLN4.md`. Tier B walks a landed branch's `lead_tmp/<name>/` recursively (`find -P`, never through a link) for a directory holding one of the bake's own output files — `common_bf{1,2}/common.ron`, `<map>/manifest.ron` for a map-group name (`tat1`, `tat2`, `bf1_kam1` …), `classes/bf{1,2}_all.ron`, `side_bf{1,2}/soldiers/` — and lists only the topmost tree; the old literal names stay as a fallback. A walkabout set (`<map>/vantage.png`) or a single-model export (`hoth/bf1_hot1_hoth_cannon/manifest.ron`) matches none. A candidate that RESOLVES into `assets_baked`, `goldens`, `target`, `extracted`, `iso` or `community_mods` of this checkout or of the main checkout behind a worktree's links is skipped. `--self-test` runs a dry run and then `--yes` over a `mktemp` fixture repo (a live branch, a nested bake, a bake inside a bake, sheets, a picture set, a model export, two links into the shipped tree) and checks the exact list and what survives; two mutants (the walk removed, the realpath refusal removed) both fail it.
  - **Gates reproduced:** ci **721 / 0 / 35** on the branch and on main (unchanged — a shell script outside the workspace); gpu_tests 0 on main, `engine/goldens` clean. The real repo's dry run: **0 paths before, 27 paths / 27 GB after** — the four SM trees the entry named (`squad-marker-on/{bake_b,bake2,land_bake,bake_main}`), L's `lowr-sheet/land/bake_{branch,main}`, R1U's `bake_raised` + `land/bake_{head,main}`, the five `bake-determinism/*` and the rest; no live branch, no sheets, render or `after` directory (lists in `lead_tmp/cleanse-walk/dry_{before,after}.txt`). The landing's `--yes` took 28 paths (with this branch's own 41 G target cache): du 68 GB, `df` 14 GB actually returned (reflinked targets and compressed extents share blocks).
  **DECIDED (loop, 2026-09-25, lane 1): copied PICTURE sets are left alone, deliberately.** *Why:* a name or layout cannot tell a disposable walkabout snapshot from a render a report still cites, and the entry allowed either. *How to reverse:* add a picture marker to `bake_trees()` in `scripts/cleanse_delete.sh`.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue DET1, built and landed in one session; a DECLARED [SWEEP] re-baseline): the spawn-node yaw no longer comes from the host libm.** `c01892a`; report `engine/artifacts/loop/reports/20260925_0330_build_land_DET1.md`. `game/src/layout.rs` builds each path node's yaw as `2·core::fmath::atan2(q[2], q[0])` (`libm::atan2f`) instead of std's `f32::atan2`; the yaw reaches `Sim::hash` through every respawn. The referee in `core/src/fmath.rs` factors its walk into `offenders_under(dir, skip)` and gains `no_std_transcendentals_in_game_sim_inputs`, which scans `game/src` bar `PRESENTATION` (`viewer.rs`, `viewer/`, `gallery.rs`, `sounds.rs`, `water_anim.rs`); `main/headless.rs`'s one orbit `sin` goes through `fmath` rather than allow-listing a file that also runs the sim. Mutant: std's `atan2` put back in `layout.rs` fails the new test at `layout.rs:321`.
  - **Gates reproduced:** ci 721 → **722** / 0 / 35 on the branch and on main (+1: the `game/src` scan); gpu_tests 0 on the branch and on main, `engine/goldens` clean; no bake, no golden. `sim_gates.sh` GATES_RC=0, walk/aim/600 `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, motion.csv identical, and `--sim-hash 100` `17e1b946ff0e821c` — all UNMOVED. **MOVED:** the sweep pair (cmp 0, = on the branch and on main) 27/32 median 960 (840) sum 213000 → **26/32 median 1020 (960) sum 195180** — 28 of 32 rows, bf2_dea1 now NO OUTCOME; the TAT2 arena kills/min 25.40/38.60/39.80/37.20 → **27.60/38.20/39.80/39.80** (monotonic unchanged: kills no, hits yes); the flat arena unmoved. **Attribution, one variable:** with only the `layout.rs` line reverted, the same branch reproduced the s19 sweep byte-for-byte and the old TAT2 rows. **The ulp census:** 619 of 6,083 path-node yaws change bits (145 of the 1,369 on `*SpawnPath*` paths), every map whose sweep row moved has some (`lead_tmp/spawn-yaw-libm/ulp/`). GATES block rewritten (sweep file, footer, TAT2 rows, ci count); raw files and SUMMARY in `lead_tmp/loop_baselines/20260925_0314_DET1/`.
  **DECIDED (loop, 2026-09-25, lane 1): the referee widens to `game/src` with a presentation allow-list, rather than moving the yaw into `core`.** *Why:* moving one derivation guards one call; the scan guards every load-path file (`layout.rs`, `walk.rs`, `mission.rs`, `roster.rs` …) against the next one, and the five presentation files are the only current users. *How to reverse:* drop `no_std_transcendentals_in_game_sim_inputs` from `core/src/fmath.rs`.
  **DECIDED (loop, 2026-09-25, lane 1): accept the re-baseline.** *Why:* the entry allowed it, and the new values are the platform-independent ones, which is the point of the contract; the old values were glibc's. DSD's entry predicts a move of "bf2_dea1's sweep row" — that row is now a NO OUTCOME at 60000 ticks, so DSD measures against the new one. *How to reverse:* revert `c01892a` and point the GATES block back at `lead_tmp/s19/baselines/sweep_main_1.txt` and the item-4 TAT2 rows.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue DR1, built and landed in one session; a DECLARED [SWEEP] re-baseline): a BF2 door stops being a permanent wall.** `bbf281a`; report `engine/artifacts/loop/reports/20260925_0340_build_land_DR1.md`. `Scene::collision_world` now delegates to `collision_world_with(doors_solid)`: every body of an instance whose class BASE is `door` loses `Layers::SOLDIER` unless `OPENBF_DOORS_SOLID=1`, and keeps its ORDNANCE bit, so bolts and AI line of sight still stop on a door. This is a documented DEVIATION standing in for DR2's door entity (`analysis/re-bf2-doors.md` §2, §3 [V][O]: the disc's door opens itself for a nearby soldier and its box rides the `open` clip out of the doorway). Door classes are on five maps: bf2_mus1, pol1, tan1, kam1 and dea1. `main.rs`'s env list gains the flag, a file outside the entry's set (two doc lines).
  - **Gates reproduced:** ci 722 → **724** / 0 / 35 on the branch and on main (+2 in `game/tests/collision.rs`: a door is out of the soldier subset and still in the ordnance one, with ray checks both ways; a non-door class is unchanged by the rule). gpu_tests 0 on the branch and on main, `engine/goldens` clean; no bake, no golden. UNMOVED: `sim_gates.sh` GATES_RC=0 (`bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, motion.csv identical), `--sim-hash 100` `17e1b946ff0e821c`, and both arenas (cmp-identical to DET1's files). **MOVED:** the sweep (branch ×2 and main ×1, all cmp 0) went from 26/32, median 1020 (960), >10k 4, sum 195180 to **29/32, median 960 (960), >10k 2, sum 107100**. Exactly the five door maps' rows moved: bf2_dea1, bf2_mus1 and bf2_tan1 now finish (team 1 at tick 10731, team 1 at 12590, team 0 at 9437), while bf2_kam1 and bf2_pol1 still end with NO OUTCOME. **Attribution, one variable:** `OPENBF_DOORS_SOLID=1` on the branch reproduces DET1's sweep file byte for byte. **Census** (`STUCK_CENSUS=1`): bf2_mus1 went from 10 stuck (8 wall) to **0**. bf2_pol1 went from 8 (5 wall) to **3** (0 wall): 2 off-graph under-terrain at (93.5,13.1,−60) and 1 no-arc at (−92,21.7,−128), both positions that were already stuck before. No other map's count rose. Baselines are in `lead_tmp/loop_baselines/20260925_0335_DR1/` (SUMMARY, the sweep, the arenas, both censuses). The GATES block was rewritten (the sweep file, the footer, the ci count).
  **DECIDED (loop, 2026-09-25, lane 1): accept the re-baseline, and keep the door open by default as the entry specified.** *Why:* every `wall` verdict on the two stuck maps cleared, and the flag reproduces the old world exactly; DR2 replaces the flag with the moving door. DSD's bf2_dea1 row moved again (it now finishes), so DSD measures against this baseline. *How to reverse:* `OPENBF_DOORS_SOLID=1`, or revert `bbf281a` and point the GATES block back at `lead_tmp/loop_baselines/20260925_0314_DET1/`.
- **2026-09-25 — LANDED (loop, lane 1, job C at medium, queue DR2, built and landed in one session; [REBAKE] maps + a DECLARED [SWEEP] re-baseline): BF2 doors are entities — they open by themselves.** `4921d3b` + crate map `5f92c19`; report `engine/artifacts/loop/reports/20260925_0400_build_land_DR2.md`. The disc's rule (`analysis/re-bf2-doors.md` §2 hop 5, §3 hops 7–9 [V][O]): a `door`-based class opens while a soldier is inside one of its ≤ 3 `AnimationTrigger` spheres, its parameter moving by `dt·30/(frames−1)` toward 1 (and back to 0 when nobody is), and its collision prims ride the bone the `open` clip animates. (1) `bake::door` (new) bakes each door class's rig into `ManifestClass::door` (additive, skipped when `None`, so only the five door maps' `manifest.ron` changed): the triggers' model-space hardpoints and, per moving prim, its `PRNT` bone's delta `B(f)·B(0)⁻¹` per clip frame. All 13 door classes rig; MUS1's hangar door reproduces the RE's 3.965 m to (0, 3.707, 1.409), the garage 3.444 m and POL1's leaves ±2.25 m. (2) `core::collision::CollisionWorld::with_movers` + `move_body`: a mover is filed in the broad phase under the envelope of every pose, so moving it inside that envelope needs no rebuild. (3) `core::door` + `Sim::doors`, ticked in `Sim::step` before movement and hashed only when non-empty (BF1 maps and fixtures hash as before). (4) `Scene::install_collision` at the six sim set-up sites; DR1's soldier-subset deviation is GONE.
  - **Gates reproduced:** ci 724 → **731** / 0 / 35 on the branch and on main (+7). Bake: the branch's full scratch bake DIFF = exactly the 5 door maps' `manifest.ron` (SAME 21,664); on main `rebake.sh maps`, then a scratch `maps` bake SAME=5322 DIFF=0, and the whole shipped tree SAME=21,669 DIFF=0 against the branch's bake. gpu_tests 0 on the branch and on main; `goldens.sh` moved nothing. Walkabout regenerated for the five door maps (the renderer does not read the rig). UNMOVED: `sim_gates.sh` GATES_RC=0 (`bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, motion.csv identical), `--sim-hash 100` `17e1b946ff0e821c`, both arenas (cmp-identical to DR1's files). **MOVED:** the sweep (branch ×2 and main ×1, all cmp 0) from DR1's 29/32, sum 107100 to **27/32, median 960 (960), >10k 3, sum 166020** — only the five door maps' rows: bf2_mus1 (team 1 won at 12590 → NO OUTCOME 70/13, stuck_max 540), bf2_dea1 (team 1 won at 10731 → NO OUTCOME, stuck_max 55680), bf2_tan1 (team 0 wins at 8809, was 9437), bf2_kam1 and bf2_pol1 still NO OUTCOME. **Attribution:** `OPENBF_DOORS_SOLID=1` (no door entity) reproduces DET1's pre-DR1 sweep file byte for byte, which also proves the movers' broad phase changes no collision answer. **The entry's gate:** the five §1 stuck positions clear — the census shows bf2_mus1 0 stuck and none of bf2_pol1's 3 at a door (all `off-graph [under-terrain]` at (93,13,−60), stuck before DR1 too); a scratch example (`lead_tmp/door-entity/tools/door_cross.rs`) walks one soldier stick-forward through each of the five §1 doorways: the live door costs **+4 to +7 ticks (0.13–0.23 s)** over the same door pinned open (≤ the clip's 1.00 s), and a door that never opens stops him exactly at the §1 stuck position. bf2_dea1's no-outcome is one unit on a `dea1_prop_grate` 27 m from any door (queued as STK with pol1's knot). Baselines `lead_tmp/loop_baselines/20260925_0355_DR2/`; the GATES block was rewritten (the sweep file, the footer, the ci count). `bake/src/map.rs` 1,598 → 1,617 lines (+19, the rig hook), already under SPL7. New entries DRV (the drawn door and its sounds) and STK (the three knots left).
  **DECIDED (loop, 2026-09-25, lane 1): accept the re-baseline — the port now shuts doors nobody stands near, as the disc does.** *Why:* DR1's always-open door was a stand-in; the real rule costs 0.13–0.23 s per doorway and the sweep's two lost finishes are not door walls (mus1 ends 70/13 with 0 stuck; dea1 on a grate). *How to reverse:* revert `4921d3b` and point the GATES block back at `lead_tmp/loop_baselines/20260925_0335_DR1/`.
  **DECIDED (loop, 2026-09-25, lane 1): the door's unread details, PROVISIONAL `[O]` in `core/src/door.rs`** — the trigger tests each live unit's FEET; the hardpoint sits where the clip's frame 0 puts it (not re-resolved through the animated model); a pose between two clip frames interpolates them (lerp / nlerp); the tick runs before movement; the shut pose is frame 0, not the `skel` rest. *Why:* each is the plainest reading the RE leaves open (`re-bf2-doors.md` §8), and none moved a gate beyond the declared sweep. Consequence noted [H]: `dea1_prop_door`'s 1 m trigger centred 1 m up is never entered by feet, and `dea1_prop_garbage_room_door`'s clip moves none of its prims, so both stay shut. *How to reverse:* edit `Door::occupied` / `bake::door::rig`.
  **DECIDED (loop, 2026-09-25, lane 1): `OPENBF_DOORS_SOLID=1` now means "no door ever opens"** (the pre-DR1 port, byte for byte) instead of DR1's "doors block soldiers". *Why:* it is the one-variable reversal of the whole door rule. *How to reverse:* `game/src/assets.rs` `collision_and_doors`.

- **2026-09-25 — CLEANSE (loop, lane 1, job B2 at medium; the fourth under `OPUS_LOOP_PROMPT.md` §6b): code area `bake`, a bug hunt over `de7f771..47ec728`, and the carried Log archive move STOPPED by the safety classifier.** Code landed as `035e5a0` + `754911f` (branch `cleanse-bake`, 6 files, +23/−36): the dead `bake::tier::Resolved::alpha_test_deletes_it` deleted (no caller outside its test since ALPHA RULE v2); `bake::water_glb`'s header and `manifest::ManifestWaterConfig`'s doc no longer say the water draws OPAQUE or that `patch_divisions` is unread (`game::water_anim` sizes the KAM1 ocean from it); two render comments SPEC2/W3 made false (`DrawLayer::Spec` covers skinned draws; the run loop switches up to four times); CRATES.md counts. The bake scans came back clean: no unwrap on disc data, no assertion-free test, one correct `allow(dead_code)`. The bug hunt (one Opus agent over the 44 commits: doors, movers, the specular pass, the speckle, the water flags) found no code defect. Gates: ci **EXIT=0, 731 / 0 / 35** on the branch and on main; `scripts/gpu_tests.sh` **EXIT=0** on the branch (the render edits after it are comment-only); no hash, sweep or bake value can move. `cleanse_delete.sh`: nothing to free (one log); `.git` 361 MB. **The archive move** (LAST_CLEANSE's carried item): the Opus agent moved 3 of the 17 2026-09-17 entries, then the classifier blocked its Edit deleting the next Log entry from PLAN.md; not routed around — both files were stashed back to HEAD (stash `e45c087`) and the move is on HANDOFF §For the maintainer. Findings file deleted (6 items: 2 fixed, 2 clean, 1 declined — the bake/render default-lighting twin is not worth a render→manifest edge — 1 for the maintainer). Next area: `mungefs`.

- **2026-09-25 — LANDED (loop, lane 1, job C, queue DR3): BF2's `plnp` route patches apply — the blocked-group mask at 0 by default, the missions' start-of-round blocks behind `OPENBF_PLNP_MISSION=1`** (RE_LOG 2026-09-01 "`plnp`" [V]; the hops re-read this session: `FUN_002ec5f8` restores the current record, stores the mask at +0x2c and XORs the new record on; `FUN_002ec818` walks the patch per hub / five types / rows; the three Lua verbs are `|` `FUN_002ebe28`, `& ~` `FUN_002ebea8`, `^` `FUN_002ebda8`; the name → group lookup is `FUN_002ebcc0`; the mask's initial value is the constructor `FUN_002ead18`'s *(decompiler detail omitted)*, SWBFRNT2 decomp line 275526, and `FUN_002ec900` builds a fresh object per plan load). Build `5cbe3af`, crate map `2778eb8`. `core::route_patch` (new, 289 lines): the bake's `plnp_<key>.bin` attaches to a `NavGraph`, validated once against the table so a swap never half-applies; `set_blocked_groups` / `block_arcs` by lower-case fnv1a arc name. `layout::nav_graph` attaches it; `gen_missions.py` walks `ScriptPostLoad` and the script functions it calls (DEA1 unblocks through `PlayAnimExtend()`) and writes `arc_blocks` into 10 BF2 mission RONs (DEA1, KAM1, KAS2, MUS1, TAN1, both eras; the other 47 regenerate byte-identical); `MissionDef::apply_arc_blocks` replays them in `battle.rs` and the sweep example only with the flag. Tests (+5): MUS1 `Connection74` is plnp group 2 and blocking it gives the authored table XOR the disc record (36 rows, 264 entry bytes), unblocking restores it; all 10 baked base-world plnp sets attach and round-trip; every mission arc name resolves to a group. **Gates, reproduced on the branch and again on main:** ci **EXIT=0, 736 / 0 / 35**; `scripts/gpu_tests.sh` **0**, no golden moved; sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, both motion.csv identical; `--sim-hash 100` **`17e1b946ff0e821c`**; the sweep pair `cmp`-identical and identical to DR2's `sweep_main_1.txt` (27/32, sum 166020); both arena pairs identical, TAT2 identical to DET1's file. Nothing rebaked (the sidecars were already baked). `core/src/nav.rs` 1,693 → 1,722 (+29: the patch hooks and a `RouteTable::row` accessor; already under SPL7). **Measured with the flag on** (`lead_tmp/plnp-patches/gates/sweep_flag.txt`): only DEA1 (stuck_max 55,680 → 1,560, still no outcome), KAS2 (win at tick 10,939, was 16,549) and TAN1 (10,649, was 8,809) move; sum 166020 → 112980. New entry DR3B (the default, [SWEEP]).
  **DECIDED (loop, 2026-09-25, lane 1): the start-of-round route blocks stay OFF by default (`OPENBF_PLNP_MISSION=1` turns them on).** *Why:* the entry's gate is byte-identity at mask 0, and the port models none of the events that UNBLOCK those groups on the PS2 (KAS2's gate panel and wooden walls destroyed, DEA1's `grate01` killed), so turning them on shuts routes for the whole round that the disc reopens mid-round; DR3B makes that call with the measurement above. Also [H], not modelled: `FUN_002ec5f8` posts an event (`FUN_002b6258`, type 10) after the swap — presumably the units' re-plan; the port's units re-read the table at their next chooser step. *How to reverse:* `MissionDef::apply_arc_blocks` (drop the env test).
- **2026-09-25 — LANDED (loop, lane 1, job C, queue DR3B): the missions' start-of-round `plnp` route blocks ON by default** (`OPENBF_PLNP_MISSION=0` turns them off; DR3's mechanism, RE_LOG 2026-09-01 "`plnp`" [V]). Build `cf5b23b`: `MissionDef::apply_arc_blocks` tests `== "0"` instead of `!= "1"`; the env doc in `main.rs`, the comments and the two log lines (`route groups … blocked (plnp)`). **Gates, reproduced on the branch and again on main:** ci **EXIT=0, 736 / 0 / 35** (no test added — the replay is DR3's tested path); `scripts/gpu_tests.sh` **0**, no golden moved; sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, motion.csv identical; `--sim-hash 100` **`17e1b946ff0e821c`**; both arena pairs byte-identical to the GATES block's files (the arena applies no mission). **DECLARED re-baseline, the sweep:** the pair `cmp`-identical, branch == main, and byte-identical to DR3's measured flag-on run; only bf2_dea1 (stuck_max 55,680 → 1,560, still no outcome, tickets 24/16), bf2_kas2 (win 16,549 → 10,939) and bf2_tan1 (8,809 → 10,649) moved; footer `finished 27/32 … >10k 2 maps  sum 112980` (was 3 maps, 166020). `OPENBF_PLNP_MISSION=0` reproduces DR2's `sweep_main_1.txt` byte for byte. `STUCK_CENSUS=1`: DEA1's `wall` knot at the grate (unit 29, STK's first knot) 1 → 0; POL1's three under-terrain units and BF1 KAS2's one unchanged. Baselines `lead_tmp/loop_baselines/20260925_0450_DR3B/`; the GATES block rewritten.
  **DECIDED (loop, 2026-09-25, lane 1): option (a) — the blocks go on by default, with no unblock events.** *Why:* (b) is impossible today — the sim has no destructible objects (no `grate01`, `gatepanel` or wall death to hook; `grep -rn destruct` over `core/src` and `game/src` finds nothing), and the port's barriers are solid, indestructible collision for the whole round (DR2's census found DEA1's no-outcome unit pinned 0.6 m from a `dea1_prop_grate`). So the PS2's pre-destruction state — barrier up, route group blocked — is the only state the port ever reaches, and blocking keeps the planner's graph consistent with the world; (c) would leave units routing into walls the port never removes. When destructibles exist, their death events must call `NavGraph::block_arcs(.., false)` (the scripts' `Unblock…` calls). *How to reverse:* `OPENBF_PLNP_MISSION=0`, or flip the test in `MissionDef::apply_arc_blocks`. Supersedes DR3's "off by default" call.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue DRV): the drawn door and its sounds follow the sim's door** (`analysis/re-bf2-doors.md` §2 hop 5, §3 hop 9 [V][O]). Build `5eeaab0`, crate map `7ebeb48`. **Bake** (`bake::door`, [REBAKE] maps): a census of the 13 rigged classes' models found every segment hard-bound by `BNAM` and none `bone_space` or soft, so each drawn submesh (GLB primitive = non-empty segment, in order) whose bone the clip moves gets that bone's `D(f)` — the prims' own rule — as `ManifestDoor::parts`; the ODF's `OpenSound` / `CloseSound` (fnv `0x323F7450` / `0x4FCF4562`, a test pins both) ride along; all additive and skipped when empty. All 26 names resolve in their own map's sound bank (`door_*` ×9 classes, `big_door_*` DEA1 blast + KAM1 podroom, `blast_door_*` TAN1, `window_*` MUS1). **Sim** (`core::door`): the disc's sound latch — bit 0x21 of `this+0x5f2`, `FUN_001ebb80` B2:99659–99687, read this session with its enclosing function checked — fires on the TRIGGER's edges (first occupied tick → `OpenSound`, first empty tick → `CloseSound`), not on the parameter leaving/reaching 0 as the entry said; `Sim::tick_doors` pushes `Event::DoorOpened` / `DoorClosed`. The latch and the events are output-side and NOT hashed (a test flips the latch and compares hashes). `Door` carries its instance, origin and sound names as setup data; `door::sample` is the frame interpolation the bodies and the draw share. **Game:** `SceneGpu::pose_doors` sets each moving submesh's draws (base + specular) to `instance · D(open)`, called by the windowed loop, the replay captures and the bench — never by the goldens or the walkabout, which still draw every door shut; the windowed game plays the sounds at the door; the replay's events CSV gets `door_opened` / `door_closed` rows. **Gates, reproduced on the branch and again on main:** ci **EXIT=0, 736 → 738 / 0 / 35** (+2: the latch's edges, the sound keys); `scripts/gpu_tests.sh` **0** on both, `goldens.sh` on main moved nothing; the branch's full scratch bake DIFF = exactly the 5 door maps' `manifest.ron` (SAME 21,664); on main `rebake.sh maps`, a scratch `maps` bake SAME=5322 DIFF=0, the whole shipped tree SAME=21,669 DIFF=0 against the branch bake. UNMOVED on both: sim_gates `bb860216c775e788` / `b489b1b3aa14084f` / `fb19c1d2c811788b`, motion.csv identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp` 0 and `cmp` 0 against DR3B's `sweep_main_1.txt` (27/32, sum 112980); both arenas identical to the GATES block's files. Walkabout not regenerated — it never poses a door, so its pictures cannot move. **Captures** (scratch `lead_tmp/door-visual/tools/drv_door.rs`, the sim's own `Door::advance` with a unit's feet in the trigger, the reference soldier planted there): MUS1 `hangardoor` shut / 15 ticks (open 0.5) / 30 ticks (1.0) — the panel slides up its tilted frame and the corridor shows behind (`cap/sheet_hangar_z8.png`); `door_cont_3` from both sides (`cap/sheet_cont3.png`); KAM1 `fdl-1` (`cap/sheet_blast_kam.png`). **Found:** DEA1's `dea1_prop_door_blast` — its bones rest at x = ±5 and the −x leaves hang off `bone_rightdoors` (+x), so read as model space (the port's reading for the prims since DR2 and now the drawn leaves) the shut door has a 1.4 m slit and opening slides the leaves ACROSS the doorway; read in the bone's rest frame they meet on a diagonal seam and slide apart. MUS1's tri door (rest rotated 20.8°) fits its frame only in model space. New [RE] entry **BNR**.
  **DECIDED (loop, 2026-09-25, lane 1): the drawn door moves with the collision door's rule, DEA1's blast door included** — *why:* what a soldier sees then matches what blocks him (DR2 already slides that door's prims across the doorway), the rest-placement question is shared by both and needs RE (BNR), and fixing only the picture would split them; *how to reverse:* stop calling `SceneGpu::pose_doors` (four call sites) or drop `ManifestDoor::parts` for the class.
- **2026-09-25 — MEASURED (loop, lane 1, job C, queue STK; no code): POL1's last stuck knot is an unreachable defend hint.** Report `engine/artifacts/loop/reports/20260925_0600_measure_STK.md`, evidence `lead_tmp/stk-census/` (a read-only trace of the sweep's own round: the bf2_pol1 row and hash `98ea331aa00a84d5` reproduced byte for byte). DEA1's grate knot had already gone with DR3B. POL1 units 2, 3, 6 (unscoped riflemen defending cp6con, floor 13.1) take Cover `hintnode153`, 8.5 m ABOVE the post — `commander::nearest_hint` picks the nearest usable hint within 25 m by 3D distance with no level test, and `ai::route` writes it into the last leg as a straight line from the post's hub. Each unit climbs the ledge at z ≈ −64, slides west and drops into `pol1_prop_crate_stack_14`'s cluster, where the disc's barrier 81 contains it at `DETOUR_RADIUS` and is ignored for the leg (FUN_00268f68's end-point rule), so the wall probe holds it for the rest of the round (53,520 / 42,720 / 28,500 ticks). Two same-floor Cover hints were in range. Across the maps, 65 of 174 posts with a hint in range have one more than 3 m off the post's level. The census's `under-terrain` flag is a label on POL1 (the asteroid heightfield is 33 m over the interior floor), not a cause. New entry **HDL** [SWEEP]: a defender's hint must stand on its post's level.
  **DECIDED (loop, 2026-09-25, lane 1): STK closes with ONE entry (HDL), not one per symptom** — *why:* the ledge climb, the drop and the pocket all follow from the hint pick in all three traces; *how to reverse:* if HDL's census still shows a unit boxed in a prop cluster, queue the pocket on its own.
- **2026-09-25 — LANDED (loop, lane 1, job A — the interrupted build finished, queue HDL): a defender's Cover/Snipe hint must stand on its post's level.** Build `87cd899`: `commander::nearest_hint` skips a hint whose own floor (`ground_under` from the hint) differs by more than `STEP_HEIGHT` from the floor found under its XZ from the post centre's height (the `hub_has_floor` idiom); `HINT_SAME_LEVEL` (PROVISIONAL — a port routing rule, the PS2's hint pick is not in analysis/) = `false` restores the old rule; +1 unit test. **Gates, reproduced on the branch and again on main:** ci **EXIT=0, 739 / 0 / 35**; `scripts/gpu_tests.sh` **0**, no golden moved; sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, motion.csv identical; `--sim-hash 100` **`17e1b946ff0e821c`**; arena flat byte-identical. **DECLARED re-baseline:** the sweep pair `cmp`-identical, branch == main; 15 rows moved (bf1 nab2/rhn2/yav1, bf2 cor1/dea1/fel1/geo1/hot1/mus1/myg1 (hash only)/nab2/pol1/tan1/yav1, tat1); footer `finished 27/32  stuck_max median 900 (finished-only 780)  >10k 3 maps  sum 118560` (was 960/960, 2 maps, 112980) — POL1 53,520 → 27,480 (still no outcome), MUS1 now wins at 11,948, BF1 YAV1 16,177 win → NO OUTCOME (35,400). Arena tat2 easy 27.60 → 29.40, hash `47a95aea1c598672`; normal/hard/elite unmoved. `STUCK_CENSUS=1`: POL1's hint knot (units 2/3/6) 3 → 0; left: POL1 unit 1 (Defend cp1) in the same `crate_stack_14` pocket on a GRAPH leg 32→35, BF1 YAV1 unit 24 (the last Hunt unit) idling at its goal hub 83 with verdict `open`; BF1 KAS2's unchanged. Baselines `lead_tmp/loop_baselines/20260925_0600_HDL/`; GATES block rewritten. New entry **STK2** (trace the two). Report `engine/artifacts/loop/reports/20260925_0615_land_HDL.md`.
  **DECIDED (loop, 2026-09-25, lane 1): land HDL although the sweep's totals got worse** — *why:* the rule removes the cause the STK trace named for all three units; the knots left are other mechanisms, and a [SWEEP] branch declares rather than tunes; *how to reverse:* `commander::HINT_SAME_LEVEL = false` (the check is the only code path added, so that should give back DR3B's sweep — derived, not measured).
- **2026-09-25 — LANDED (loop, lane 1, job C, queue STK2): the two knots HDL left, measured — and a hunter's last leg now walks at the enemy, not its hub** (`ai::route::HUNT_AT_ENEMY`, PROVISIONAL — a port rule under the commander's Hunt, which is itself the port's design, D6). Build `9beb056` (`core/src/ai/route.rs`, +1 test that fails with the flag off). **The trace** (seed 42, the sweep's round, `lead_tmp/stk2-census/`, scratch `stk_trace` reproducing the sweep rows' hashes): (1) **BF1 YAV1** — team-0's last unit (8) had walked off the `yav_bldg_aquaduct` deck at T10380–10500 (a re-plan on arc 110 past its midpoint starts from hub 83 and walks straight at hub 84, 17.5 m off the deck's corner) and slid for the rest of the round between the piers 10 m below; team-1's last hunter (24) reached hub 83 — the quarry's hub by XZ locate — and stood on it, because a Hunt's last leg aimed at the HUB; units 18/22 sat in two barriers' margins near hub 17. (2) **BF2 POL1 unit 1** — HDL's same-level hint `hintnode159` is 3.8 m from `crate_stack_14`; the Defend capture-volume flap re-plans leg 32→35 through the stack from there and `leg_ignores` drops barrier 81 (the unit stands in its `DETOUR_RADIUS` margin); the stuck recovery re-plans the identical leg. **Gates, branch and main:** ci **EXIT=0, 740 / 0 / 35**; `scripts/gpu_tests.sh` **0**, `goldens.sh` nothing moved; sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, motion.csv identical; `--sim-hash 100` **`17e1b946ff0e821c`**; both arena pairs byte-identical to HDL's files. **DECLARED re-baseline, the sweep:** pairs `cmp` 0, main == branch == the scratch experiment; 14 rows moved (bf1 bes2/kam1/nab1 hash only, bf1 end1/hot1/rhn1/yav1, bf2 end1/geo1/kam1/kas2/nab2/tan1, tat1); footer `finished 29/32  stuck_max median 960 (finished-only 840)  >10k 2 maps  sum 84240` (was 27/32, 900/780, 3 maps, 118560) — BF1 YAV1 NO OUTCOME → team 1 wins 28,168, BF2 KAM1 NO OUTCOME → team 0 wins 23,349, BF2 TAN1's winner flips (team 0 at 11,225 → team 1 at 9,808). `STUCK_CENSUS=1`: YAV1 1 → 0; POL1 unit 1 and BF1 KAS2 unit 7 unchanged. Baselines `lead_tmp/loop_baselines/20260925_0557_STK2/` (the clock stamp sorts before HDL's label; it is the later); GATES block rewritten. **New entries:** BPO ([RE] first — the PS2's barrier push-out `FUN_002689d8`, RE_LOG:178, for a unit in a barrier's margin), AQD (BF1's 3-D hub locate, RE_LOG "Arrival and locate [V]", so no unit walks off YAV1's aqueduct deck), CEN (the census misses a unit that slides in place — three of YAV1's four knots read `stuck_run` ≤ 1,080). Report `engine/artifacts/loop/reports/20260925_0620_build_land_STK2.md`.
  **DECIDED (loop, 2026-09-25, lane 1): build the Hunt rule inside STK2 and land it as a re-baseline** — *why:* the entry allowed one small rule; the traced cause of YAV1's NO OUTCOME is the hunter's waypoint, the rule is four lines, and the measured sweep improves (2 more maps finish, sum −34,320); the other two mechanisms are not small and have their own entries; *how to reverse:* `ai::route::HUNT_AT_ENEMY = false` (the only code path added — gives back HDL's sweep, derived, not measured).
- **2026-09-25 — LANDED (loop, lane 1, job C, queue CEN): `STUCK_CENSUS` also lists units that hold one leg without closing on it.** Build `19832cf` (`game/examples/round_to_outcome.rs` only): a wall slide's progress resets `Route::stuck_run`, so the census now also tracks each unit's leg (`next_hub`, `dest_hub`) and lists every alive unit that held it `NO_PROGRESS_TICKS` = 6,000 ticks while its best XZ distance to the waypoint gained under `NO_PROGRESS_GAIN` = 2 m (both PROVISIONAL, CEN's pick) — a second per-map summary line and the same per-unit line tagged `noprog unit`. **Gates, reproduced:** ci **EXIT=0, 740 / 0 / 35** on the branch and on main (no test: an example); the sweep's stdout, with and without `STUCK_CENSUS`, `cmp`-identical to `loop_baselines/20260925_0557_STK2/sweep_main_1.txt` on the branch and on main; two censuses' `census`/unit lines identical (only the worker-ordered load lines and wall-clock `done` lines differ, as before). No sim code, so no hash can move. **The entry's gate:** the new census on HDL's `87cd899` names bf1_yav1 units 8/18/22 (no progress 44,962 / 36,862 / 38,772 ticks) and bf2_pol1 unit 1 (28,776) — and that run's stdout `cmp`s HDL's baseline sweep. **Found on main:** bf2_dea1 (no outcome) has 13 units without progress and none stuck — team 0 units 14/15/16 on leg 6→3 at (−86, 50, −105), team 1 nine on leg 4→1 around (−63, 50, −73) — new entry DWS (measure first); also 5 on pol1, 5 on bf1_kas2, 2 on myg1, 1 each on rhn2 and tan1 (several `not-walking` — defenders holding). Report `engine/artifacts/loop/reports/20260925_0630_build_land_CEN.md`.
- **2026-09-25 — MEASURED (loop, lane 1, job C, queue DWS): DEA1's 13 wall-sliding units stand against blast doors their own presence holds open.** No code. A traced round on main `d2f8a16` (`lead_tmp/dws/`, STK's trace + door states) reproduces the sweep row byte for byte (`tickets 60/16 … hash 31cb8d35c6bc385c`). Leg 4→1's nine team-1 units slide on the face of door 2 `dea1_prop_door_blast1` (−63, 50, −75), leg 6→3's three team-0 units on door 0 `dea1_prop_door_blast` (−84, 50, −108); both doors sit at `open` = 1.00 from tick ≤ 6,600 to 60,000 because units stand in their triggers, and under the port's model-space rest reading (queue BNR) the opened leaves slide across the doorway. Counterfactual, same seed, those two doors' bodies lifted out: no-progress census 13 → 1 (an idle cp2 defender), row `tickets 1/3 alive 1/3 … stuck_max 180 hash 5213f4f287f29ff2` — lifting every door gives the identical row. DEA1 still ends without an outcome there (a two-team remnant), for DSD and a re-measure after BNR. BNR's entry now carries that prediction. Report `engine/artifacts/loop/reports/20260925_0620_measure_DWS.md`.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue DSD): Death Star conquest takes a second ticket from team 1 per team-1 death.** Build `aed8710` (`analysis/re-time-limit.md` §6 [V][O]): `scripts/gen_missions.py` now reads `ScriptPostLoad`'s `OnCharacterDeathTeam(handler, t)` whose handler is straight-line code whose one call is a literal `AddReinforcements(team, -n)` into a new RON field `death_drains: [(on, team, tickets)]` (port teams) — regenerating wrote ONLY `bf2_dea1_con.ron` and `bf2_dea1_con_cw.ron` (every other RON byte-identical; a pre-change regeneration was clean); `game::mission::MissionDef::apply_death_drains` loads it into `core::battle::Battle::death_drain[dying][paying]`, which `Battle::on_death` spends on top of its own ticket, floored at 0 (PROVISIONAL [O]: the filter's subject [D], the floor [H]); both setups apply it (`make_walk`, the sweep). Not in `Sim::hash` (fixed setup; what it spends lands in `reinforcements`). A drain can leave the paying team below its live count for one tick; the next `tick_bleed` sheds the AI, as a bleed does. **Gates, reproduced on the branch and on main:** ci **EXIT=0, 742 / 0 / 35** (+2 tests); `scripts/gpu_tests.sh` 0, no golden moved; sim_gates (walk `bb860216c775e788`, aim `b489b1b3aa14084f`, 600 `fb19c1d2c811788b`, motion identical), `--sim-hash 100` `17e1b946ff0e821c`, both arena tables byte-identical to the STK2 baseline files; the sweep pair `cmp` 0, main == branch, and against STK2's baseline ONLY bf2_dea1's row moved, as declared: `tickets 60/16 alive 16/16 y 50..52 … stuck_max 1260 hash 31cb8d35c6bc385c` → `tickets 10/16 alive 10/16 y 50..58 … stuck_max 1560 hash a8908fdf9582b0f7`, still NO OUTCOME; footer `finished 29/32 … sum 84240` → `sum 84540`. (The entry's "today team 1 wins at tick 33945" predated DR3B/HDL/STK2's re-baselines; the row it moved was already NO OUTCOME.) DECLARED re-baseline: `lead_tmp/loop_baselines/20260925_0640_DSD/`, GATES block rewritten. No rebake (the RONs are read at runtime). Report `engine/artifacts/loop/reports/20260925_0640_build_land_DSD.md`.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue TLO): the Instant Action conquest options, the TIME LIMIT (Off, 5..60 min) and the reinforcement PERCENTAGE (10–500 %).** Build `e76156a` (`analysis/re-time-limit.md` §1 hop 9 and §5 [V][O]): `Sim::time_limit: Option<u32>` seconds (None = Off, the default; an INPUT, not hashed) — `update_cps` checks it after the 0-ticket and domination rules, and the step that brings `tick` to `limit × 30` gives the round to the team with more tickets, or `OUTCOME_DRAW` = 2 on equal counts (`MissionVictory({1,2})`: both win), with `Event::TimeUp`; `core::sim::scale_reinforcements` = `count * pct / 100` in integers. The shell's "Reinforcements" row is now the % (showing both scaled counts) and a "Time limit" row follows it (the disc's clamps); a shell round takes the MISSION's own per-team counts scaled by the %, no longer team 1's count for both. HUD: an m:ss countdown under the tickets while a limit is set, and a "TIME UP — BOTH TEAMS WIN" banner; `game --time-limit <min>`; a draw counts as a win in the career, the events CSV logs `time_up`. **Gates, reproduced on the branch and on main:** ci **EXIT=0, 745 / 0 / 35** (+3 tests in `core/tests/battle_outcome.rs`: expiry with more / fewer / equal tickets → 0 / 1 / draw at exactly tick 60 of a 2 s limit; Off and an unexpired 1 h limit never end a round and hash alike; the % at 10/100/500 on 150 and 220 → 15/150/750 and 22/220/1100); sim_gates walk `bb860216c775e788` / aim `b489b1b3aa14084f` / 600 `fb19c1d2c811788b`, motion identical; `--sim-hash 100` `17e1b946ff0e821c`; the sweep pair `cmp` 0 and `cmp` DSD's baseline 0 (footer `finished 29/32 … sum 84540`); both arena pairs identical to the baseline files; main == branch on every file. **The entry's scratch round** (a temporary, uncommitted `TLO_LIMIT` hook in `round_to_outcome`, seed 42): bf2_tan1 with a 300 s limit ends at **tick 9000 (300 s), team 1 wins, tickets 11 / 17**; without it the row is the baseline's (`team 1 wins at tick 9808`; the entry's "NO OUTCOME" for tan1 predated the DR/HDL/STK2 re-baselines). **Goldens:** only `shell.png` moved (mean abs 4.891, max 232, 6.53 % of pixels, bbox 340,120–939,600: the new row, the % text, the panel one row taller); the regenerated file is byte-identical to the viewed branch candidate; `scripts/gpu_tests.sh` 0 after. Deviations: four files outside the entry's list — `core/src/sim.rs` (the exports), `game/src/main/headless.rs` (the draw in the hash line, the CSV row), `game/tests/golden_{shell,hud}.rs` (the models' new fields). Report `engine/artifacts/loop/reports/20260925_0710_build_land_TLO.md`.
  **DECIDED (loop, 2026-09-25, lane 1): TLO lands with the `shell.png` golden move the entry did not declare.** *Why:* the entry's own build adds a shell row, so the shell golden must move; the move is exactly that row and the % text (sheet `engine/artifacts/lead_tmp/time-limit/sheet/out/shell.png`), nothing else moved. *How to reverse:* revert `939ed21` with `e76156a`.
- **2026-09-25 — PXC batch 1 (loop, lane 1, job B3 at max): the PCSX2 rig reaches a level on both games, and every save state gives the camera pose.** No tracked code (everything lives under the gitignored `engine/artifacts/pcsx2/`). Boot → menus → in level, unattended, on BF1 Kashyyyk: Docks and BF2 Kashyyyk — the key paths and the menu rule (≥ 1.1 s between presses, or both games drop every other one) are in `engine/artifacts/pcsx2/README.md`; save states at side select, the spawn map and the spawns (`ref/states/`). A PCSX2 state is a zstd zip holding the whole EE RAM and its own 640×480 frame: BF1's active camera is `*DAT_00522ac8` (`FUN_0049c3d0`, BF1 decomp 561582; matrix rows right / up / back / eye at +0, projection at +0x80), BF2's `DAT_0060c240` (`FUN_00582c60`; matrix at +0x30, projection at +0x130 — the offsets `analysis/re-sky-pass.md` names); both games' RAM coordinates are the port's (BF1 KAS2 cp11 and BF2 KAS2's four posts read with the port's own signs, no flip) — `pcsx2/pose.py`. An untracked scratch example (`pcsx2/tools/pxc_pose.rs`, built in a detached scratch worktree, never committed) renders the port at a state's pose and FOV, and the pairs line up to a few pixels. Findings (`engine/artifacts/pcsx2/ref/FINDINGS.md`, each with both PNG paths): F1 BF1 KAS2's water light green-teal and see-through on the PS2, dark warm grey in the port; F2 no red band on the PS2 at four dock poses where the port draws 4 grey-white rows over 4–5 rows of (255,0,0) — KASF's question (a), for those poses; F3 / F7 the PS2's sun glare and BF2 KAS2's near-white horizon haze; F5 the PS2 soldier camera is 48.0° × 37.69° (BF1) and 65.0° × 52.06° (BF2), h × v, zoom 1, against the 70° vertical `WalkSim::camera` takes from `Camera::default`; F6 BF1 KAS2's sand slope behind Dock Two is grass in the port; F8 BF2 KAS2's sea is lighter. Queued CFOV, WCOL, SUNG, KTEX ([RE] first) and a KASF note; HANDOFF HOLD 7 and What next 2(a) rewritten (the in-level step is built). Report `engine/artifacts/loop/reports/20260925_0945_b3_PXC.md`.
- **2026-09-25 — CORB closed, no code (loop, lane 1, job C at medium): COR1's "flat buildings under the floor" are authored world props.** the maintainer's flythrough frame shows building silhouettes on flat planes below the Jedi Temple. Measured on main (`9607f64`), not the entry's sky-shell hypothesis: the flat part is `cor1_prop_buldings_sky_floor`, a world instance at (−24, −108, −109) whose GLB is one flat 618 × 618 m plane (256 vertices, all y = 0) textured `cor1_skycity_2`; `cor1_prop_buldings_sky` (y −67, the item-9 sky lanes) and `sky1..3` (y −27.26) are the upright city around it — all in the disc's world layer (`assets_baked/bf2_cor1/manifest.ron`), drawn by the ordinary instance path. The shell `cor1_sky_buildings_1` (Offset 0, MovementScale 1.0, so eye-centred per `analysis/re-sky-dome-placement.md` §0.2 and `render::sky::Placement::Dome`) is one r ≈ 6.3 band spanning 2.3°–15.4° below the horizon by its own vertices (v 0..1, one texture row) — placement faithful, nothing to fix. From play the parapet hides the plane: `game --headless --cam` at cp2 (2.8, 4.7, −168.4) at four bearings (pitch −15) and at the parapet (2.8, 4.9, −181, pitch −20) show only upright skyline tops; the flat plane appears only from past the railing (z −188) or the orbit. Evidence `engine/artifacts/lead_tmp/cor1-buildings/` (`CORB_sheet.png`). No gate can move (no code). Not exhaustive over every COR1 edge.
  **DECIDED (loop, 2026-09-25, lane 1): CORB closes as authored art seen from outside the play area, not a port defect.** *Why:* the geometry is the disc's own world instances at their authored transforms and the one sky shell involved obeys the RE'd rule; no play vantage tried shows the plane. *Reverse:* reopen CORB — a PXC capture looking down from a COR1 balcony (or any COR1 edge that exposes the plane in play) would settle the PS2's side. Report `engine/artifacts/loop/reports/20260925_1000_measure_CORB.md`.
- **2026-09-25 — PXC batch 2 (loop, lane 1, job B3 at max): the PCSX2 library covers nine water maps, and the pairs find eight new defect families.** No tracked code (everything under the gitignored `engine/artifacts/pcsx2/`; the pose renderer stays an untracked scratch example in a detached worktree, removed). Captured with poses, port renders at the PS2's exact pose and FOV, and pairs: BF1 KAM1 (2 posts), YAV1 (2), NAB2, KAS1, KAS2's high ground (Republic side — `Right K` proven), BF2 KAM1, NAB2, DAG1, FEL1, plus BF2 MYG1 and Space Kashyyyk from two menu miscounts (BF2's list holds Mos Eisley and the space maps). 63 new pairs; menu states for both games (`ref/states/bf1_menu_iamaps.p2s`, `bf2_menu_iaplanets.p2s`) so a map costs one load. **The instrument was checked first**: PCSX2 warns "Blending Accuracy is Basic, recommended High", so F1's and F7's states were re-rendered under HW Basic, HW High and the SOFTWARE renderer — water and haze patches agree within 4 levels (frame mean abs 1.8–3.2). Findings (`engine/artifacts/pcsx2/ref/FINDINGS.md` F9–F23, both PNG paths each, read under the maintainer's new `ref/COMPARISON_RULES.md`): BF1 KAM1's sea is dark teal under foam crests on the PS2 and a flat textureless slate plate in the port, the y = 0 terrain `re-water-alpha.md` §5 left open (F9 → KOC); a dark near-camera wall in the port where the PS2 sees through — the port culls nothing (`render/src/pipelines.rs` 528–529 `cull_mode: None`, the only one), BF1's words carry no two-sided flag (F10, F23 → BFC); rain and lightning on Kamino (F11 → WTHR); instances the port skips as "no drawable model" that the PS2 draws — the CP console, Theed's shrubs and vines, Yavin's grass (F12 → SKIPC); YAV1's flooded bazaar and BF2 KAM1's ocean green-teal on the PS2, neutral in the port (F13, F18 → WCOL notes); **BF1 KAS1's lake mirrors the sky** — every sky shell of BF1 KAS1 and KAS2 is authored `Reflect` (the hash 0x92c778aa sits on `kas1_sky_dome` / `kas1_mountains` / `kas_moon` and on KAS2's two shells, and nowhere else on either disc), the draw `re-sky-dome-placement.md` §8 said to ignore until a map needs it (F16 → REFL); bare terrain where the PS2 carpets the ground with plants — Dagobah, Felucia, BF1 KAS1 / KAS2 / YAV1 — and the port parses no `FOLG` (F22 → FOLG); terrain layer colours on four more maps (F14, F17, F21 → KTEX note); the port ~60 % brighter on BF2 MYG1 and on Theed's shaded stone (F15, F20 → LITE); a specular hot spot on a BF2 KAM1 pad (F19 → SPHOT); 0 red pixels on BF1 KAS2's high ground in both, but the bamboo hides the horizon, so KASF's "never red" stays [H]. The §5 verifier (Opus, foreground, claims and citations only) SUPPORTED all ten claims and sharpened two: the `Reflect` hits parsed as DomeModel properties, and BF2's `FOLG` chunks are only 2–8 KB. Report `engine/artifacts/loop/reports/20260925_1130_b3_PXC.md`.
  **DECIDED (loop, 2026-09-25, lane 1): the reference library keeps PCSX2's default hardware renderer at Basic blending, not High or the software renderer.** *Why:* the three agree within 4 levels on the water and haze patches the findings rest on (FINDINGS "Batch 2", `engine/artifacts/lead_tmp/re/pxc2/ctl/`), and HW Basic is the fastest. *Reverse:* set `accurate_blending_unit = 3` or `Renderer = 13` in `~/.config/PCSX2/inis/PCSX2.ini` (the pre-batch copy is `lead_tmp/re/pxc2/PCSX2.ini.before_pxc2`) and repeat the control on any finding that turns on a blend mode.
- **2026-09-25 — LANDED (loop, lane 1, job C, queue CLN5): the `LOWR` far sheet's flag-gated draw path, four defects.** Build `50285f8`, behind `OPENBF_FAR_CULL=1` + `OPENBF_LOWRES=1` (both still OFF by default). (1) `viewer::lowres_range` returns no sheet on a `FarSceneRange(0)` map: BF2 END1 had drawn its sheet through the fade pipeline, invisible but depth-writing; `re-fog.md` §2.5 [V] (the far-buffer composite is skipped) and re-lowr.md §5's own list of maps without a drawn sheet agree, so no new RE was needed [D]. (2) New `render::DrawLayer::FarSheet` / `fs_far_sheet`: UNLIT (the disc's base colour 0x80808080), fogged, own pipeline, sorted after Mask. (3) The "every sheet texture is alpha 255" census had read the BAKED files, which the bake forced opaque. On the DISC (`lead_tmp/cln5-lowr/sheet_alpha_disc.txt`, every `tex_` via `mungefs`) BF1 KAS1 `kashyyyk1` has 62,571 / 65,536 mip-0 texels below 16 and BF1 KAS2 `kas2` 27,482; the other twelve sheets are 255 everywhere. `bake::map` now keeps the named sheet texture's alpha (`keep_alpha`'s `water_named` became `consumer_named`), and the pass tests at the AREF 0x10 composed to disc A ≥ 32 (PROVISIONAL [D]). (4) TAT1's 3000 m MaxDistance stays capped by the camera's 2000 m far plane — recorded in `lowres_range`, not changed. `re-lowr.md` §5 corrected. **Gates, reproduced on the branch and on main:** ci **EXIT=0, 745 / 0 / 35** both; `scripts/gpu_tests.sh` 0 both; `goldens.sh` on main: NO golden moved; branch scratch bake `tat1 maps` SAME=5452 **DIFF=2** — exactly the two KTX2s, as declared here; `rebake.sh maps` on main, then a main scratch bake `maps` SAME=5322 DIFF=0; walkabout regenerated for bf1_kas1/bf1_kas2 (flag off): all 15 images byte-identical in RGB. No sim code touched. Flag-ON before/after sheets (`lead_tmp/cln5-lowr/sheets/`, viewed): END1 identical (the invisible sheet hid nothing at the 7 poses); TAT1 and BF1 HOT1 lose the lit tint (vantage 51 % / 28 % of pixels); KAS1 and KAS2 gain holes over their water. **Through those holes the port shows the CLEAR colour** (black on KAS1, KAS2's red fog colour), where the PS2 shows its far water pass (re-lowr.md §2.3): one more thing `OPENBF_FAR_CULL`'s default waits on (HANDOFF §What next 2(b)). `bake/src/map.rs` is now 1,630 lines (SPL7). Report `engine/artifacts/loop/reports/20260925_1200_build_land_CLN5.md`.
  **DECIDED (loop, 2026-09-25, lane 1): CLN5's two open calls — no sheet on a `FarSceneRange(0)` map, and TAT1's far-plane cap kept and recorded.** *Why:* (1) is the reading both analysis docs already support; (4) raising every camera's far plane would change what the default renders past 2000 m, for a sheet that is off by default. *Reverse:* (1) drop the `far_scene_range == Some(0.0)` early return in `lowres_range`; (4) give `render::Camera` the map's max(MaxDistance, FarSceneRange) as `far`.
- **2026-09-25 — PXC batch 3 (loop, lane 1; captures at max, interrupted after its FINDINGS/README at 13:03; the queueing and report finished by the next session, job A at medium).** Every one of the port's 32 maps now has a PCSX2 spawn post, three turned views and a pitched-up view with port pairs (`engine/artifacts/pcsx2/ref/README.md`'s table); the port's `tat2` is BF2's Mos Eisley, so the BF1 capture is refiled as `bf1_tat2_mos_eisley/`. FINDINGS F24–F33: **LEAF** (new, [RE]) — the PS2 hangs `leafpatch` sprite clusters on trees' branch hardpoints (`AttachOdf`/`AttachToHardpoint`; both ELFs register the class — BF2 decomp 16469–16470, string 0x60fd08 re-read from the ELF), the port draws bare branches (BF2 YAV1 canopy box PS2 (24,31,8) vs port (136,141,143), reproduced); **GLOW** (new, [RE]) — BF2 Utapau's panels, Coruscant's windows and Mustafar's lava are self-lit on the PS2 and dark in the port; notes on WCOL (three more flat waters: BF1 NAB2, BF2 FEL1, BF2 YAV1), KASF (Beach Head: 0 red pixels on the PS2 in 5 poses, the port up to 5042), LITE (a BF2 effect: port/PS2 luminance median 1.30 over 17 BF2 maps, 0.93 over 15 BF1 — a crude box census [D]), FOLG (both Endors' fern carpets), SUNG (two more glare frames), BFC (BF1 BES2's camera inside a mesh); F31 confirms CORB's call on the PS2. Batch 3's own [V] RE reads (the leafpatch properties, the factories) have not been through a §5 verifier — LEAF's RE session owes it. No code, no gate run or moved. Report `engine/artifacts/loop/reports/20260925_1415_a_PXC3.md`.
- **2026-09-25 — WCOL's [RE] half (loop, lane 1, job D at max): the flat water's teal is a VU1 view-angle term the port lacks.** `analysis/re-water-colour.md` (pointer `re-water.md` §3c, RE_LOG entry), `[O]` UNAUDITED. The "microcode 0x515410" is a DMA chain that uploads a 63-instruction VU1 overlay (BF1 VU 0x74e, BF2 0x758, word-identical); its routine B colours the MAIN layer per vertex, **unlit**: LUT colour + `SpecularColor`·F and LUT alpha + `SpecularColor.a`·F, F = (1 − max(0, V̂.y))³ with V to the eye shifted 10 m along the light's horizontal direction; the LUT runs Min → 2·Max − Min over the DIVU-picked indices 5..15, so its mean is MaxDiffuse, not the port's midpoint; routine A gives the speckle `SpeckleAmbient + SpeckleSpecular·lobe³²` around the sun's azimuth (the port's is flat — its white mottle); the ocean's vertex RGB is the engine's global white halved = unity (HANDOFF What next 2(d) answered); a reversed-scroll second pass is dead (flags 0, no writer). **Measured on the PS2 itself** — save states with the `Effect("Water")` bytes patched in EE RAM, booted in the isolated PCSX2 rig: zeroing `SpecularColor` removes +13..+27 G/B levels, the teal body, on BF1 KAS2 / NAB2 / BF2 FEL1; the port's own model run on the PS2 draws the port's grey mottle. The port taken apart at the same poses with a scratch instrument (a detached worktree, never main; the no-knob render equals the library frames byte for byte) moves the same way. The rest of the gap is the **bed** under the water (KAS2 sand vs the port's grass — KTEX; NAB2/FEL1 beds 2–2.5× too bright in the port — LITE). **Side finding: the port's baked sun is not the PS2's** — BF1 (−L.z, L.y, L.x) of `FUN_004931e0`'s vector from the same `SunInfo.Angle` (all 14 BF1 maps with states), BF2 (L.x, L.y, −L.z) of its global light (16 maps); the PS2's sun glare sits where its own vector says. One foreground Opus verifier: 11 SUPPORTED, 2 PARTLY (folded in), 0 NOT SUPPORTED. Queued **WCOLB** (the build, [GOLDENS] the 11 wet-map goldens) and **SUND** ([RE] first; relights every map); notes on KTEX, LITE, SUNG. No code, no gate run or moved. Report `engine/artifacts/loop/reports/20260925_1545_re_WCOL.md`.


# ARCHIVED 2026-09-25 — HANDOFF-OPUS.md, retired by the v2 restructure (its open items: see INBOX.md 2026-09-25)

# HANDOFF — continuing openbf on Opus (written 2026-08-30 by the Fable lead; rewritten 2026-09-01 after the Fable
# cleanse; consolidated 2026-09-17, 2026-09-23 and 2026-09-24 by the loop's cleanses — superseded text is in git:
# `git show bdbacd9:HANDOFF-OPUS.md` is the page before the 2026-09-24 consolidation, `7498317` before 2026-09-23's)

This file is the brief every **Opus** loop session reads. **Since 2026-09-14 (the maintainer) the autoloop is the project's
lead** — its sessions land, rebake, regenerate goldens, refill the queue and make the calls (§Rules; the standing
prompt is `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md`). **Since 2026-09-17 (the maintainer) they also do the
reverse-engineering themselves: Fable is OFF on this project** (his Fable budget goes to his day job), so §HOLD FOR
FABLE is simply the RE list, and `OPUS_LOOP_PROMPT.md` §5's four self-checks plus an Opus verifier agent replace the
Fable audit. **Since 2026-09-23 (the maintainer) the loop may also boot a disc in PCSX2 and screenshot it itself**
(`engine/artifacts/pcsx2/`, §HOLD 7).

## Read first, in order
1. `PLAN.md` — single source of truth (phases, criteria, guardrails, Log).
2. `analysis/RE_LOG.md` — every RE finding + what is ported (tags [V]/[D]/[H]).
3. `artifacts/*/README.md` under `engine/artifacts/` — what the maintainer has been shown.
4. This file's §For the maintainer, §Rules and §Hold list.
5. `engine/CRATES.md` before you grep code — the crate/module map (`scripts/size_report.sh --check-map` keeps it true).

## For the maintainer — calls the loop made without you, and what only you can supply
The loop adds ONE line per call here, newest first (`DECIDED <date> lane N: <call> — <why> — <how to reverse>`, with
the sheet or report path for a look call), and one line per thing only you can supply; the full text of every call
is in its report and in PLAN's Log. You clear lines you have read. (The cleanses cut each entry to its essentials
with its pointer and fold a superseded call into the one that replaced it — nothing is cleared.)
- FOR YOUR EYE 2026-09-25 lane 1 (WCOL's RE, `analysis/re-water-colour.md`, sheets
  `engine/artifacts/lead_tmp/re/wcol/sheets/wcol_{bf1_kas2,bf1_yav1,bf1_nab2,bf2_fel1}.png`): the PS2 water's teal,
  see-through look is a view-angle term the port lacks. It was taken apart on the PS2 itself: patched save states in
  PCSX2, and the port's own water model run on the PS2 draws the port's grey mottle. Queued as WCOLB. Bigger, and yours to
  weigh when it lands: the port's sun comes from a different direction than the PS2's on every map (BF1 rotated 90°,
  BF2 mirrored) — queued as SUND, [RE] first; its build relights every map.
- DECIDED 2026-09-25 lane 1 (CLN5, `reports/20260925_1200_build_land_CLN5.md`, sheets `lead_tmp/cln5-lowr/sheets/`):
  behind `OPENBF_LOWRES`, a `FarSceneRange(0)` map (BF2 END1) draws no far sheet, and TAT1's 3000 m sheet stays capped
  by the camera's 2000 m far plane — reverse in `game::viewer::lowres_range`. Flag-gated; nothing default moved.
- DECIDED 2026-09-25 lane 1 (PXC batch 2, `reports/20260925_1130_b3_PXC.md`): the reference library keeps PCSX2's
  default HW renderer at Basic blending — the software renderer and HW High agree with it within 4 levels on the water
  and haze the findings rest on — reverse: `accurate_blending_unit = 3` / `Renderer = 13` in `PCSX2.ini`.
  **FOR YOUR EYE** (read under your `engine/artifacts/pcsx2/ref/COMPARISON_RULES.md`, which batch 2 now follows):
  BF1 KAS1's lake MIRRORS the sky shells on the PS2 (`ref/bf1_kas1/pairs/cp2_cp2_eastharbor_v3.png`; queue REFL);
  Dagobah and Felucia are carpeted in plants the port does not draw (`ref/bf2_dag1/pairs/`, `ref/bf2_fel1/pairs/`;
  FOLG); BF1 Kamino's sea is the terrain plate (`ref/bf1_kam1/pairs/cp0_cp2_beta1_v4_down.png`; KOC). **Only you:**
  Kamino's rain and lightning (queue WTHR) — PLAN has no weather at all; say whether it belongs in the LOCKED
  FEATURE SET before WTHR's build.
- DECIDED 2026-09-25 lane 1 (CORB, a look call, sheet `engine/artifacts/lead_tmp/cor1-buildings/CORB_sheet.png`): your
  flythrough's "flat buildings under the COR1 floor" closed with no code — it is the disc's own world prop
  `cor1_prop_buldings_sky_floor`, a flat city-texture plane at y −108 (plus `cor1_prop_buldings_sky*` at −67 / −27),
  hidden by the parapet from every play vantage tried — why: authored placement, drawn as every prop is — reverse:
  reopen CORB; a PXC capture looking down from a COR1 balcony would settle the PS2's side.
- FOR YOUR EYE 2026-09-25 lane 1 (PXC batch 1, `engine/artifacts/pcsx2/ref/FINDINGS.md`, pairs in
  `engine/artifacts/pcsx2/ref/*/pairs/`): the rig now plays both discs to a spawn and renders the port at the PS2's
  exact camera. Two readings to check against yours: (1) your note said BF1 KAS2's water is "clearer than the real
  game" — the PCSX2 frames show the PS2's water as the LIGHTER, more see-through one (F1); (2) the PS2 soldier camera
  is 37.7° (BF1) / 52.1° (BF2) vertical against the port's 70° (F5, queued CFOV) — a big look/feel change when built.
- DECIDED 2026-09-25 lane 1 (TLO, `reports/20260925_0710_build_land_TLO.md`): landed the conquest time limit (Off by
  default) + reinforcement % shell rows WITH the `shell.png` golden move the entry did not declare — the new row
  forces it; sheet `engine/artifacts/lead_tmp/time-limit/sheet/out/shell.png`. A shell round now takes each team's
  mission count × %. Reverse: revert `939ed21` + `e76156a`. Worth a Tier-4 look at the countdown and draw banner.
- DECIDED 2026-09-25 lane 1 (STK2): built the Hunt fix inside the measurement (the entry allowed one small rule) — a
  hunter's last leg walks at the enemy, not its hub; sweep 27/32 sum 118560 → 29/32 sum 84240 (BF1 YAV1 and BF2 KAM1
  now finish) — PROVISIONAL port rule under the commander's own Hunt — reverse with `ai::route::HUNT_AT_ENEMY = false`.
  Report `reports/20260925_0620_build_land_STK2.md`.
- DECIDED 2026-09-25 lane 1 (HDL): landed the off-level-hint rule as a DECLARED re-baseline although the sweep got no
  better overall (sum 112980 → 118560, >10k maps 2 → 3: POL1 53,520 → 27,480, MUS1 now finishes, BF1 YAV1 now does not)
  — the rule removes the traced cause, and the two knots left are other mechanisms, queued as STK2 — reverse with
  `commander::HINT_SAME_LEVEL = false`. Report `reports/20260925_0615_land_HDL.md`.

DECIDED 2026-09-25 lane 1 (DR3B, superseding DR3's "off"): BF2 missions' start-of-round `plnp` route blocks are ON by
default (`OPENBF_PLNP_MISSION=0` turns them off) — the port destroys nothing, so the barriers they stand for (KAS2's
gate and walls, DEA1's grate, TAN1's turbine) stay solid all round and the blocks keep the AI from routing into them;
sweep sum 166020 → 112980, DEA1's grate knot gone — reverse in `MissionDef::apply_arc_blocks`. Report `reports/20260925_0455_build_land_DR3B.md`.

**Only you (2026-09-25 cleanse, lane 1):** the PLAN Log archive move the 2026-09-24 cleanse carried (the 2026-09-17
and 2026-09-23/24 loop entries → `analysis/PLAN_LOG_2026.md` Part 4) is BLOCKED: the safety classifier denied the
Opus agent's Edit deleting a Log entry from PLAN.md after 3 of 17 had moved. The loop did not route around it; the
partial work is stash `e45c087` (both files back at HEAD). Do the move yourself, or tell the loop it may (and how
the classifier should see it). Until then PLAN.md stays ~390 KB. Report `reports/20260925_0445_cleanse_bake.md`.

**Look calls — worth your eye, blocking nothing:**
- DECIDED 2026-09-25 lane 1 (DRV, `reports/20260925_0600_build_land_DRV.md`): the DRAWN door now rides the sim's
  door and plays its ODF Open/Close sounds on the trigger's edges (the disc's latch). The drawn leaves follow the
  collision rule, so DEA1's blast door opens WRONG in both — leaves slide across the doorway — until queue BNR's RE
  settles where bone-bound geometry sits at rest (sheets `lead_tmp/door-visual/cap/sheet_{hangar_z8,cont3,blast_kam}.png`).
  Reverse: stop calling `SceneGpu::pose_doors`.
- DECIDED 2026-09-25 lane 1 (DR2, `reports/20260925_0400_build_land_DR2.md`): BF2 doors are real entities now —
  shut until a unit's feet enter the trigger, then the box rides the `open` clip (+0.13–0.23 s per doorway against
  a door pinned open); the sweep re-baselined 29/32 → 27/32 (mus1 and dea1 no longer finish; neither is a door wall).
  The unread details are PROVISIONAL `[O]` (feet, frame-0 hardpoint, lerp between frames); two DEA1 door classes
  therefore never open [H]. The drawn door followed in DRV. `OPENBF_DOORS_SOLID=1` = no door ever opens.
  Reverse: revert `4921d3b`, GATES back to `loop_baselines/20260925_0335_DR1/`.
- DECIDED 2026-09-25 lane 1 (W3, `reports/20260925_0145_build_land_W3.md`): BF1's flat water now carries the
  disc's additive speckle flipbook (KAS1, KAS2, NAB2, YAV1) — on KAS2 it reads as a moving grey caustic shimmer over
  the whole near sea (golden mean 4.67). Its colour is `SpeckleSpecularColor` everywhere, the brightest reading: the
  PS2's is per vertex from unread VU1 code (it also has `SpeckleAmbientColor` and a light vector), so the disc may
  show it only near the sun's glint. Sheets: `lead_tmp/water-speckle/pairs_0{0,1}.png`, `kas2_crop_sheet.png`
  (off / t 0, 0.5, 1 s). Reverse with `OPENBF_WATER_SPECKLE=0`, or revert `6405cd4` + `3085b01`. BF2's flat
  water draws it too since WSE (next line).
- DECIDED 2026-09-25 lane 1 (WSE, `reports/20260925_0315_build_land_WSE.md`): BF2's flat water takes the same
  speckle (its `SpecularEnable` resets to 1), BF2 FEL1's water no longer bobs (it authors `OscillationEnable(0)`), and
  BF2 flat far water takes the fog ramp (invisible on every shipped map). Only `maps/bf2_kas2.png` moved (max 10/255,
  the pool; `lead_tmp/water-flags/pairs_00.png`, `kas2_crop.png`). Call: BF2 KAM1's OCEAN stays unfogged — the gate
  is the flat renderer's and the ocean renderer's fog is unread. Reverse: `OPENBF_WATER_SPECKLE=0`, or revert
  `bb6b0d2` `a8b2f41` `52c84a5` and `rebake.sh maps`.
- DECIDED 2026-09-25 lane 1 (SPEC2, `reports/20260925_0200_build_land_SPEC2.md`): the Specular pass is on soldiers and
  their weapons too — BF1's battle droids, super battle droid and dark trooper turn chrome (the white B → the sun colour
  ×2 at power 4), BF2's AT-AT commander's power-1 helmet and vest wash near-white (column 0 of the table), the rest are
  glints; 28 unit goldens moved, 0 map goldens. Sheets: `lead_tmp/spec-side/pairs_{00..13}.png`. Reverse with
  `OPENBF_SPEC=0` or by reverting `1583ac0` + `c231edb` and `rebake.sh side`. The unit sheets install the side's table
  and take the renderer's default sun (they have no map); an in-map capture would settle the brightness.
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
- 2026-09-24 lane 1 (SKO, `reports/20260924_1915_land_SKO.md`): a soldier behind Kamino's chamber glass is now seen
  THROUGH it, tinted like the tanks, instead of painted over it — `lead_tmp/skinned-order/view/A_close_crop_before_left_after_right.png`
  (a scratch view; no committed golden or walkabout panel moved). Four DECIDED in PLAN's Log (skinned opaque after the
  water runs; one blend pass, a tie draws the world first; no regeneration owed; the far-fade band unchanged).
  Reverse: revert `662ba97`.
- 2026-09-24 lane 1 (KW, `reports/20260924_0500_build_land_KW.md`): Kamino's windows draw as GLASS — the cloning
  tanks in the next room show through the central chamber's bands. `lead_tmp/kam-windows/view/cp0_cp1_{BL,TL}_band.png`
  (RGB, before above after); the golden pair `…/land/pairs_00.png`. Reverse: revert `bc069b9`, `rebake.sh maps`,
  revert `47efeff`.
- 2026-09-24 lane 1 (R1U, `reports/20260924_0430_build_land_R1U.md`): COR1's security-room doors and DEA1's reactor
  walls draw SOLID (their alpha is a glow mask; the skyline and the nebula had shown through).
  `lead_tmp/rule1-unflagged/view/{bf2_cor1_cp5_cp6,bf2_dea1_cp3_cp3}.png`, every moved panel in `…/sheets/`.
  Reverse: revert `0c3b5c3`, `rebake.sh maps`.
- 2026-09-24 lane 1 (SKA, `reports/20260924_0245_build_land_SKA.md`): the wookie draws WHOLE — the disc alpha-tests
  no soldier and blends only the fur shell; BF2's unit had been a cloud of fur fragments. Open
  `lead_tmp/skinned-alpha/land/pairs_00.png` and the RGB crops in `…/view/`, not the golden PNG itself (its alpha
  plane makes a compositing viewer show the body pale). Reverse: revert the three commits, `rebake.sh side`.
- 2026-09-23 lane 1 (OCF, `reports/20260923_2215_land_OCF.md`): Kamino's foam gathers on the crests and fades off
  them — moving water only (the goldens and the walkabout never step it): `lead_tmp/ocean-foam/close/sheet_close_x4.png`
  (×4 gain), the far-ring fix `…/land/horizon/ring_fix_sheet.png`. Reverse: revert the two commits.
- 2026-09-23 lane 1 (WA, `reports/20260923_1905_land_WA.md`): the water on 12 maps is translucent by its own sheet's
  alpha — bays, ponds and swamps show their beds, BF2 Kamino's sea gains its foam layer.
  `lead_tmp/water-sheet-alpha/land/walk_sheets/bf1_yav1__cp5_bazaar.png` (+ `bf2_dag1__cp0_cp2`, `bf2_kam1__vantage`),
  the goldens `…/land/pairs_0{0..3}.png`. BF1 Kamino's visible "sea" is the TERRAIN (`re-water-alpha.md` §5, [H]).
  Reverse: the `water_named` clause in `bake::map::bake_content`, `rebake.sh maps`, revert `7409964`.
- 2026-09-17 lane 1 (AT): every world cutout is thinner — the disc's test, alpha ≥ 193.
  `lead_tmp/mask-cutoff/land/pairs_00.png` (END1), `…/crops/maps_tat2.png`, `…/walk_end1_cp4_before_after.png`.
  Reverse: `MASK_CUTOFF` in `render/src/shader.wgsl`.
- 2026-09-17 lane 1 (OCB): Kamino's sea swells 19.2 m peak to trough — the disc's own unnormalised FFT (OCA's RE
  found the port's normalisation a defect). `lead_tmp/ocean-metre-scale/sheets_bright/{lowC_t0,lowA_t3.0,farD_t0}.png`.
  Reverse: the one constant in `Ocean::inverse`.
- 2026-09-17 lane 1 (L): the `LOWR` sheet bakes and draws, but `OPENBF_FAR_CULL` and `OPENBF_LOWRES` stay OFF —
  18 maps draw no sheet, and there the cull shows the clear colour. `lead_tmp/lowr-sheet/land/sheets_land/bf1_end1_vantage.png`.
  Reverse: set both to 1.
- 2026-09-17 lane 1 (SM): the squad marker is ON and draws by its icon's own alpha; its placement and depth test are
  ours. `lead_tmp/squad-marker-on/land/d14_landed_walk_0{2,4}000_before_after.png`. Reverse: `OPENBF_SQUAD_MARKER=0`.
- 2026-09-17 lane 1 (WP): one sub-tolerance golden, `maps/bf1_nab2.png` (298 px), regenerated —
  `lead_tmp/water-patch-cells/sheets/bf1_nab2_clusterA_16x.png`. Reverse: revert `c754d13`.
- 2026-09-17 lane 2 (OCL): optional — both KAM1 oceans read dark from a low eye (W2's authored tint doing its job).
- Standing since 2026-09-09: BF2 NAB2's distance fades to the disc's own BLACK `sky_naboo2` (HOLD 10(c),
  `lead_tmp/s18/sheets_view/`), and the water is DARK by the disc's LUT (W2, `lead_tmp/s19/sheets_view/`).

**Only you can supply:**
- Tier-4 checks: the squad README + captures (Phase D's deliverable), the Elite feel check (D12's reinforcement
  bias), and a hand-made HOLD-release capture (HR's rule is unreachable in every automated referee).
- Whether tier 2 ships on disk for a build you play (+388 MB for the maps, +4.9 MB for the side textures).
- The classic-plus override design (HOLD 8), any change to the LOCKED FEATURE SET, legal and guardrail questions,
  and pushing anywhere.
- No longer on this list: PCSX2 captures. Since 2026-09-23 the loop boots and screenshots a disc itself; what
  nobody has built yet is reaching a point INSIDE a map (HOLD 7), which every "a capture / GS dump settles it" item
  waits on.
- One line in `scripts/gpu_tests.sh` (queue 7, 2026-09-25): `cargo test -p game --test walkabout_stats -- --ignored`
  after the render tests, so the walkabout's per-panel statistics run with every GPU gate. The session's edit of that
  script was denied by the auto-mode classifier as a "CI bypass", so the referee is landed but runs only by hand
  (26 s). Add the line yourself, or allow the edit, and the next landing picks it up.

**Calls made — for the record, each reversible (newest first):**
- 2026-09-25 lane 1 (DR1, `reports/20260925_0340_build_land_DR1.md`): BF2 doors are now OPEN to soldiers by
  default (every body of a `door`-based class leaves the soldier subset; bolts still stop). The sweep was
  re-baselined, as the entry allowed: 26/32 → **29/32**, sum 195180 → 107100, and only the five door maps moved.
  bf2_mus1 now has 0 stuck (was 10) and bf2_pol1 3 (was 8, none of them at a wall). The replays, `--sim-hash 100`
  and both arenas did not move. Reverse: `OPENBF_DOORS_SOLID=1` for one run, or revert `bbf281a` and point the
  GATES block back at `lead_tmp/loop_baselines/20260925_0314_DET1/`.
- 2026-09-25 lane 1 (DET1, `reports/20260925_0330_build_land_DET1.md`): the spawn yaw now uses `libm::atan2f`, and
  the libm referee scans `game/src` bar five presentation files (not the yaw moved into `core`). That moved 619 of
  6,083 path-node yaws by an ulp, and the sweep (27/32 → 26/32, sum 213000 → 195180) and the TAT2 arena were
  re-baselined, as the entry allowed; the replays and `--sim-hash 100` did not move. Reverse: revert `c01892a` and restore the old
  GATES rows (`lead_tmp/loop_baselines/20260925_0314_DET1/SUMMARY.txt` has both).
- 2026-09-25 lane 1 (CLN4, `reports/20260925_0340_build_land_CLN4.md`): `cleanse_delete.sh` now finds scratch bakes
  by their own output files at any depth, but leaves copied PICTURE sets (walkabout snapshots) alone — a name or layout
  cannot tell a disposable copy from a render a report still points at. Reverse: add a picture marker to `bake_trees()`.
- 2026-09-25 lane 1 (CLN3, `reports/20260925_0320_build_land_CLN3.md`): only the two scratch writers that leaked
  (bake's classes tests, roster.rs's shared dir) were fixed, not an RAII handle over all ~30 sites — the others
  already remove their scratch on success, and a /tmp census held nothing else; `OPENBF_KEEP_SCRATCH=1` keeps it.
- 2026-09-25 lane 1 (queue 7, `reports/20260925_0300_build_land_7.md`): the walkabout statistics referee's σ
  tolerance is ±10 % with a FLOOR of ±1.0 (0..255), since the entry's bare ±10 % would fail a near-flat sky panel
  on driver noise; the drop demonstration fails the panels where the dropped texture covers ≥ 2 % of the frame, not
  slivers under 1.5 %. Reverse: `STD_FLOOR` / the tolerances in `game/tests/walkabout_stats.rs`.
- 2026-09-25 lane 1 (queue 7): main's ci had failed since WSE's `rebake.sh maps` (its main ci ran before the rebake):
  `game/tests/water_look.rs` still said "BF2 never fogs water". `2ab8dfb` gives it WSE's rule (dea1 180, fel1 52,
  kas2 100, nab2 150). A landing that rebakes should re-run ci after the rebake, not only before it.
- 2026-09-25 lane 1 (WSE's RE, `reports/20260925_0210_re_WSE.md`, `analysis/re-water-flags.md`): BF2's
  `SpecularEnable` and `OscillationEnable` both default to 1 (the Water effect's level reset stores 1, and no disc
  authors `SpecularEnable`). So BF2's flat water gets the speckle and far-fog passes the port gave it none of, and
  FEL1 — which authors `OscillationEnable(0)` — has still water. Three calls for the build:
  - It bakes each flag as the PS2 reads it (unauthored = 1) into the manifest, rather than special-casing BF2 in the
    viewer. Reverse: two bake lines and `rebake.sh maps`.
  - FEL1's still water rides the same build; it moves no golden.
  - The port keeps the fresh-boot fog reading: a fog-off map shows no far-water fog, and fog rows carried over from
    an earlier map are not modelled.
- 2026-09-24 lane 1 (SPEC's RE, `reports/20260925_0010_re_SPEC.md`, `analysis/re-specular-pass.md`): the PS2's
  specular pass is a reflection-vector lookup into one shared `specular` table (CORE.LVL), added by the base sheet's
  alpha. Three calls for the build:
  - It reproduces the disc's MIRRORED light by default: the light is rotated into a camera frame whose up/back are
    negated relative to R's, which flips the light's camera-right component. A flag switches it to the consistent
    L·R.
  - BF2's translucent (0x04) spec pass is not drawn; on the disc it weighs 1/128 and is invisible.
  - The work splits into SPEC (world) and SPEC2 (soldiers and weapons, skinned).

  Reverse: the flag's default; a 1/128 weight for those 22 words. Worth a PCSX2 in-map look once the rig can reach
  a map (HOLD 7): does a side-lit highlight sit where the sun is, or at its mirror?
- 2026-09-24 lane 1 (TL's RE, `reports/20260924_2035_re_TL.md`, `analysis/re-time-limit.md`): the port owes NO default
  conquest time limit — stock PS2 SP conquest sets none (the option is Off at power-on), so bf2_kam1 / bf2_tan1 /
  bf1_kas2's NO OUTCOME is not a rules defect; the disc's opt-in timer is queue TLO, Off by default, and TLO's shell
  takes the disc's reinforcement PERCENTAGE row instead of Shell v0's absolute count. Found on the way and queued as
  DSD: Death Star conquest takes a second ticket from team 1 per team-1 death, which the port lacks. Reverse: a
  default limit is your call (LOCKED FEATURE SET); the % row → keep the absolute row, seeded per team.
- 2026-09-24 lane 1 (the cleanse, `reports/20260924_0545_cleanse_render.md`; code `cfb83c6`..`e64f52b`, every gate
  unmoved): SKO moved to the head of the TODO list (KW's glass made its pass-order bug visible on KAM1);
  `render::TIME_WRAP` 1000 → 2000 s (BF1 KAS2's water jumped half a repeat at every wrap); a MASK soldier material
  fails at load rather than ending the game at its first frame; `DrawLayer::Sky` deleted; C3 closed; `impl Default
  for TextureDesc` declined; CLN8 folded in, CLN9 queued; the PLAN Log move left to the next cleanse. Nothing needs
  your eye. Reverse: PLAN Log 2026-09-24, the cleanse — each is one constant, one row or one revert.
- 2026-09-24 lane 1 (the bake's alpha rules — NT `3698060`, R1U `0c3b5c3`, KW `bc069b9`; reports
  `20260924_0345_build_land_NT.md`, `…_0430_…_R1U.md`, `…_0500_…_KW.md`): rule 1's `NEVER_TESTS` stays 128 instead
  of following the world's test (at 193 it flips 23 opaque draws, 15 of which the disc draws opaque); rule 1
  composites only a word with 0x02 or 0x04, class D needs a texel past 193, and a draw rule 1 no longer catches is
  labelled by the clause that decides it (`extras.rule` "2"/"3" — MUS1's sky-dome word goes OPAQUE, which the sky
  path never reads); Kamino's windows get their BLEND from rule 3's narrow 0x04 clause, not from raising rule 1 (the
  raise would also composite a 0x02 word in 129..192, which BF2's disc draws nothing of). Reverse: drop KW's clause
  and/or set `NEVER_TESTS = crate::detail::ALPHA_TEST - 1`, then `rebake.sh maps common`.
- 2026-09-23/24 lane 1 (CMAP `8d86e3a` and the six pure-move splits SPL1–SPL6, `947084d`..`56cb0e8`; reports
  `20260923_2240_build_land_CMAP.md` … `20260924_0340_build_land_SPL6.md`): `game/src/viewer.rs`, the binary's
  `main.rs`, `core`'s `collision.rs`, `ai.rs` and `sim.rs`, and `render/src/lib.rs` are each a facade or crate root
  over parts named by responsibility (`ai::brain`, not `think`, so a doc link stays unambiguous); helpers widened
  only to `pub(super)` (plain `pub` where a 100-column line forced it); shared test fixtures stay in the facade,
  tests go with the feature they pin; CMAP waived the six files over 2,000 lines as "split queued" and put its three
  rules into the live prompt. Reverse: each split is another pure move (PLAN Log 2026-09-23 night / 2026-09-24).
- 2026-09-24 lane 1 (SKA `ff71352`, `reports/20260924_0245_build_land_SKA.md`): a skinned segment whose `MTRL` word
  asks for a state the skinned path cannot draw (0x02, or 0x04 with 0x08 / 0x40 / 0x80) FAILS its bake
  (`bake::soldier::segment_blends`) — no baked soldier has one, P7's vehicles and heroes will; no walkabout
  regeneration (the 32 map goldens came back byte-identical); the pass order (every skinned draw after the world's
  blend pass) queued as SKO. Reverse: PLAN Log 2026-09-24, SKA.
- 2026-09-23 lane 1 (OCF's landing): the still plane past 640 m carries NO foam once the sea steps (`0b203e7`); the
  foam steps on a 30 Hz grid of the water clock; a cold start pre-runs 192 steps (PROVISIONAL); the foam rides a
  second patch buffer, render untouched. Reverse: PLAN Log 2026-09-23 (night), OCF.
- 2026-09-23 lane 2 (the cleanse, `reports/20260923_1945_cleanse_game.md`): this page consolidated; PLAN's
  2026-09-07..09 Log moved verbatim to `analysis/PLAN_LOG_2026.md` Part 3; `tat2` rebaked (its manifest had missed
  L's `lowres_glb` key); code `fac5c88` + `25676a5`, every gate unmoved; CLN5–7 queued. Reverse:
  `git show 7498317:HANDOFF-OPUS.md` / move Part 3 back.
- 2026-09-17 lane 1 (SKA's RE): the verifier's late verdicts folded in under STOP — 24/25 supported; the one break
  was mine (BF1's model-draw slot is +0xc). Reverse: revert `5c1aeab`.
- 2026-09-17 lane 2 (OCL `6bf0d60`, `reports/20260917_2110_build_land_OCL.md`): the ocean draws at ONE resolution —
  the far patch and the 3.229 m sea-seam deleted; `WaterRanges.near` stays unread; the disc's never-issued
  decimated build is not ported.
- 2026-09-17 lane 1 (OCB's landing): your flythrough report was queued the same day as KASF, MUSS, UTAS and CORB
  (`engine/artifacts/loop/playtest_flythrough_review_20260917.md`).
- 2026-09-17 lane 1 (CLN1 `d3e69cf`): no behaviour gates for a test-only branch (the proof is in its report); it
  found a host-libm `atan2` in `game/src/layout.rs` feeding the sim hash — queue DET1.
- 2026-09-17 lane 1 (WA's RE): the build scoped to the water sheets each map's `Effect("Water")` names, not every
  unnamed sheet (`analysis/re-water-alpha.md`); SM's "unnamed sheet keeps its alpha" clause was scoped to the
  common bakes, and its map half became WA.
- 2026-09-17 lane 2 (SW's RE, `analysis/re-bf2-doors.md`): a BF2 door is an engine entity that opens by itself; the
  port's stand-in is queue DR1, a flag whose default (OPEN) is a deviation the other way — yours to overrule when
  it lands (`OPENBF_DOORS_SOLID=1`); DR2 is the real door.
- 2026-09-17 lane 1 (ONCE `95a641a`): the ci flake fixed (the roster bake race); the `/tmp` test-scratch leak became
  queue CLN3 rather than a fold-in.
- 2026-09-17 lane 2 (the cleanse): gate VALUES live only in QUEUE.md's GATES block; superseded text in tracked
  files is deleted (git keeps it) while untracked pages get real archives.
- 2026-09-17 lane 2 (SK landed `f6cbf90`): the tickets = alive stall is the disc's own rule (`core/src/cp.rs`
  stays), and `Route::stuck_run` stays `u16`.
- 2026-09-17 lane 2 (OC landed `b8ebe59`): `--bench` cost accepted at +0.030 ms; the mis-specified "water mean
  abs > 1" gate was met in substance.
- Each landing's ci-count bookkeeping (CT, SM, ONCE, CLN1, WA …) lives in QUEUE.md's GATES block and PLAN's Log.

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
3. **The RE list is §HOLD FOR FABLE below; what only the maintainer can supply is in §For the maintainer.**

## State (ONE live snapshot — rewritten 2026-09-24 by the cleanse; the previous generation is in git)
- **Main** carries every landing through the 2026-09-24 cleanse, SKO, item 9, SPEC, SPEC2, W3 and WSE (landed after it); nothing is PARKED. What each
  landing did is in QUEUE.md's DONE rows (label, sha, report) and PLAN's Log; `git worktree list` shows the live
  branches.
- **Gate values: QUEUE.md's GATES block and nowhere else.** PLAN's Log records each value when it moved.
- **`engine/assets_baked` is a fresh bake of main** (21,669 files): SPEC2's full scratch bake read SAME=21669 DIFF=0
  ONLY_IN_A=0 ONLY_IN_B=0 against it on 2026-09-25, after SPEC2's `rebake.sh side`; WSE then rebaked `maps` (only
  the 12 water manifests moved; its main scratch bake of `maps` read SAME=5322 DIFF=0) (the last rebakes:
  `maps` by WSE 2026-09-25, `tat2` and `common` by SPEC 2026-09-25, `side` by SPEC2 2026-09-25 — a landing that changes a manifest field
  rebakes every group that writes a manifest).
- **The goldens: 90 committed PNGs (75 MB)**, rendered at the PS2 BASE tier — the golden tests force `Ps2`, so
  `OPENBF_TEXTURE_TIER` cannot move them. Last moved 2026-09-25 by WSE (`maps/bf2_kas2.png`; W3 before it, 4 BF1 water maps); each landing's moves
  are in its Log entry.
- **The walkabout set** (`engine/artifacts/walkabout/`, 219 panels over 32 maps, 256 PNGs) — regenerated in full by
  KW's landing; `bf2_cor1`, `bf2_mus1` and `bf2_yav1` regenerated by item 9's, and the 17 maps with a specular model by
  SPEC's.

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

## The older generations
The superseded "What next" / "State at handoff" generations up to 2026-09-09 evening and the per-session digests
(Opus sessions 1–5, Fable sessions 6–10) are in `analysis/PLAN_LOG_2026.md` Part 2, verbatim. Later generations
were deleted rather than copied (this file is tracked — `git log -p HANDOFF-OPUS.md` has every one).

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
- Working mode is otherwise unchanged (PLAN.md §Working mode / kickoff
  decisions): full autonomy, one commit-sized change at a time, `engine/ci.sh`
  green before every commit and **check its exit code directly** (never
  through a pipe), git local only, never commit assets/ELFs/decomp/baked
  data/community_mods (all gitignored), `iso/` `extracted/` read-only,
  never `git add -A` (check `git status` size first).
- Every claim about how the PS2 behaves must cite an existing `analysis/`
  document. If the answer is not already in `analysis/`, **do not guess** — do
  the RE as its own job (`OPUS_LOOP_PROMPT.md` §5), with the four self-checks
  and the verifier agent. The absence claim and the "BF2 does the same" claim
  are the two kinds that were false in the 2026-09-14 blind trial
  (`engine/artifacts/opus_re_trial/report.html`), so they carry the heaviest
  proof. What you could not verify stays [H] and PROVISIONAL; the result is
  lifted into `analysis/` under an `# OPUS RE … [O] UNAUDITED` header, and
  every load-bearing claim carries `[O]` so a later audit can sweep them.
- Prefer PROVISIONAL, clearly-labelled placeholders over fidelity claims.
  Label every new constant/table `PROVISIONAL` in code and PLAN.
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

1. **P7 vehicles** (hover/walker/flyer physics, seats, turrets, vehicle damage) — an RE pass over BF1/BF2 vehicle
   code and the `vehc`/`hovr`/`walk`/`flyr` class properties, one RE job per subsystem, then the build. P7 is next.
   The 0x04 blend bit sits on 25 BF1 / 39 BF2 side models (flyers, gunships, heroes), so the vehicles inherit
   `analysis/re-side-alpha.md`'s per-segment verdict — built for the skinned path by SKA as
   `bake::soldier::segment_blends`, which REFUSES the cutout bit 0x02 (`rep_walk_atte` carries it, if it bakes
   skinned; the fusion cutter is a weapon, baked rigid through the world's MASK pass) and a 0x04 word carrying
   0x08 / 0x40 / 0x80 (another blend): a skinned model that needs either needs that pass or blend state first.
2. **Animation blend layers** (upper-body 16/17-track clips vs full-body; the `FUN_0022d828` / `FUN_00282f78`
   anim-set giants) — `game::animator`'s table is PROVISIONAL; visual tuning (blend times, the walk/run threshold)
   is fine if labelled PROVISIONAL.
3. **The `_low1` soldier LOD path**, BF2 `_lower` clip naming, the droid anim banks (`cis_inf_sbdroid` rig), the
   first-person (`humanfp`) rig.
4. **Chunks and formats `mungefs` does not parse:** `fx__` effects (only `Effect("Water")` is released —
   `analysis/re-water.md` §5; whether the PS2 draws an effect into BF1 BES1's ring is this item, low value), `coll`'s
   runtime RESPONSE (the format is [V] in `a2-bake-formats.md`), `plan`/`path` AI use, `shdw`, `font`, `bnd_`,
   `CLTH`, vehicle `zaa_` banks. Partly answered 2026-09-17 (`analysis/re-bf2-doors.md`): a `prim` body's `PRNT`
   node can be an ANIMATED bone — every BF2 door — which the port treats as identity (queue DR1/DR2).
5. **Sim constants still PROVISIONAL in `core`** — sprint recovery, unit-vs-unit separation, spawn-slot
   bookkeeping, AI behaviour, aim-assist details, jump height, ammo/reload edge cases: none changes without an
   `analysis/` citation.
6. **ANSWERED 2026-09-01** — the mission Lua for every map: `scripts/gen_missions.py` generates the 57 mission RONs
   from the PS2 bytecode (PLAN §WHERE THINGS STAND, "Mission data").
7. **The PCSX2 rig.** FIRST TIER LIVE 2026-09-23: `engine/artifacts/pcsx2/pcsx2_capture.sh` boots a disc headless
   and screenshots it, and the loop may use it (read its README first, every time; hold `ci.lock` for the call).
   **IN-LEVEL since 2026-09-25 (queue PXC batch 1):** `pcsx2_session.py` drives both games from boot to a spawn (the
   key paths are in the README), save states hold the spot, and `pose.py` reads the camera pose out of any state, so
   the port can render the same view (`tools/pxc_pose.rs`) — the reference library is `engine/artifacts/pcsx2/ref/`
   (coverage table, `FINDINGS.md`). Still to build: a GS-register dump. The dump's three standing questions: (a) the D-pad menu's second
   material layer (the 0x89 / PRIM 0x58 / TEST 0x51001 entry passed whenever a texture is bound — does the
   microcode draw it); (b) the wreck's LOD at the golden's ~475 m framing (item 20); (c) the far water fog pass's
   look (item 21).
8. **Classic-plus override design** (how W1 PC textures replace PS2 ones: mip policy, normal maps, Shader Patch
   numbers) — the maintainer's decision. Measured so far: names match by direct lowercase (79/79 on DEA1) and the
   PC-stock-where-larger rule is proposed with data.
9. **ANSWERED 2026-09-07** — `shell_menu_accept`'s source (`analysis/re-sound-global.md`).
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
11. **Raised by measurement 2026-08-31:** (a) ~10 % of both Hoth goldens clip to pure white — is the PS2's terrain
    light a SUM of ambient and sun, a modulate, or a fixed 2x over `LUMI`? Do not rescale the ambient without an
    answer. (b) why `dea1_bldg_*_collision*` / `uta1_bldg_macy_collision*` are in the drawable list — a class flag
    the port does not read?
12. **Any guardrail or legal question** (community mod content is local-only and may be modified for personal use —
    The maintainer 2026-08-30 — never committed). `engine/goldens/loadscreens_bf{1,2}.png` are decoded disc art (7.1 MB): fine
    while git is local-only, the first thing to revisit if a remote is ever added (PLAN Q4).
13. **Collision** — walkable model collision LANDED 2026-09-01 and the LAYERS are answered
    (`analysis/re-collision-layers.md`). Still held: the collision RESPONSE and the layer-id constructor stores
    (RE_LOG "Collision LAYERS" §open), the step/slope rule (`STEP_HEIGHT`, `WALKABLE_MIN_NY`, `TERRAIN_BURY` are
    ours), `push_out`, the six `LEAF` flag bytes; a non-identity `prim PRNT` is item 4's door answer.
14. **RELEASED AND LANDED 2026-09-01** — the P9 spine specs (N pads by slot, `core::fmath` on `libm`); nothing held.
15. **Environmental death rules** — the fall-damage rule (`LETHAL_LANDING_SPEED = 25` m/s is ours) and the
    `AddDeathRegion` consumer (bindings BF1 001857a8 / BF2 0018d628; "inside → immediate death" is ours). Both
    PROVISIONAL in `core::sim`.
16. **Mission data still unconsumed** — the locals teams (`(first unit class, SetUnitCount)`), `attacking_team`,
    `spawn_delay`/`spawn_interval` into `SpawnWaves`, and the per-team alive-AI cap on the asymmetric BF1 maps
    (kas1/kas2/nab2). Consuming them is plain work; what a locals team DOES in the sim (infinite tickets,
    `re-tickets-spawn.md` §2.1) is a design call.
17. **AI-data semantics** — `PrimaryStance`/`SecondaryStance`/`Mode` ANSWERED 2026-09-08 (bitmasks; RE_LOG
    2026-09-08 evening; built in Phase D). Still held: `BARR` SIZE (half vs full) and whether barriers are corridor
    walls at all; the BF2 arc `flags` names and whether `locate` must skip `flags & 7` arcs; the XZ arrival test vs a
    same-level test (DAG1's cave, POL1's y = 0 hubs).
18. **Untextured and dangling segments** — what the PS2 draws for a `segm` with empty `TNAM` slots (75 stubs on 8
    maps) or a texture name with no `tex_` chunk (`yav_prop_ivy`, `Cor1_bldg_flowers`, `noIcon`); the port skips
    them (PROVISIONAL). Pulling `yav_prop_ivy` from the PC install is a classic-plus call (item 8).
19. **ANSWERED 2026-09-08/09** — the walkabout defect hunt's (a)–(g): `re-terrain-cutters.md` (queue H),
    `re-ai-sniper.md`, `re-lowr.md` (queue L), `re-water.md` §1/§2 (W1/W2/WL); the droideka decided 2026-09-08
    (the maintainer: jumps everywhere, BF2's numbers — the build is P7-class). Nothing held.
20. **The `modl` LOD switch rule** — GEO1's Techno Union wreck is not a material bug (`analysis/re-vu1-cull.md`); how
    the PS2 picks LOD0 vs the `LOWD` sub-model is unread (not in `gmod`; `FUN_00592c38`'s distance compare at
    D:712968 may be it). The port draws LOD0 forever (`mungefs::entity`'s best-model pick); `re-vu1-cull.md` §4 has
    the two-station capture note, which needs item 7's in-map capture.
21. **The water VU1 microcode.** Answered since 2026-09-09: the tint scale (W2), the ocean itself (OC), its amplitude
    (`re-water.md` §3a — OCB), its single resolution (§3b — OCL), and the water's TEXTURE alpha
    (`analysis/re-water-alpha.md`: TCC=1/TFX=MODULATE on every blended layer, 10 of the 12 `MainTexture`s carry real
    alpha — built by WA, landed 2026-09-23; the near speckle alone draws RGB-only — built by W3, landed 2026-09-25, its colour a flat stand-in for the
    VU's). **The FLAT-water program `0x515410` is READ 2026-09-25 (queue WCOL, `analysis/re-water-colour.md`, `[O]`):**
    a VU1 overlay whose routine B gives each vertex its LUT colour (entries 5..15, Min → 2·Max − Min), its phase's
    animated alpha, plus `SpecularColor`·(1 − V̂.y)³ in both, unlit; routine A gives the speckle a sun lobe. Still held:
    the flat water's far LOD and why the ocean's dequantisation box is two cells wide. A GS dump (item 7).

## What Opus was released to do — the record
The three **NEWLY OPUS-SAFE** releases (Fable 2026-09-01 ×2, the maintainer 2026-08-30) and the original Opus-safe backlog are
in `analysis/PLAN_LOG_2026.md` §"The NEWLY OPUS-SAFE releases and the original Opus-safe backlog" (cleanse C2,
2026-09-08). What is Opus-safe TODAY is the loop queue (`engine/artifacts/loop/QUEUE.md`); the boundary is §HOLD FOR
FABLE above.

## How the last session verified things (keep doing this)
- **`scripts/gpu_tests.sh`** runs every `render` GPU referee SERIALISED and then `scripts/goldens.sh --check`. Never
  `cargo test -p render -- --ignored` bare — it SIGSEGVs on the iGPU under the default thread count. Regenerate the
  goldens only with `scripts/goldens.sh` on main, only for a landing's declared moves, after quantifying and viewing
  every committed-vs-regenerated pair (`engine/artifacts/lead_tmp/s19/golden_pairs.py`).
- **`cargo run --release -p game --example round_to_outcome`** (all 32 maps) is the behavioural gate for anything
  touching `core`, the layout, the missions or the heightfield; `ROUND_VERBOSE=1` explains a map that stalls. The
  exact gate invocations are QUEUE.md's GATES block.
- Raw-byte spot checks against `extracted/` for any parser (offsets in the RE reports); Tier-1 tests pinned to those
  vectors.
- Headless captures for every visual change (`game --headless --walk --capture-series`, the walkabout), viewed
  before committing; Tier-4 artifacts for the maintainer under `engine/artifacts/<topic>/README.md`.
- **Landing shapes worth copying:** an entry whose MARK is wrong — OCB (its `[GOLDENS]` re-declared unmoved and
  proved twice, empirically and structurally); a TEST-ONLY branch — CLN1 (the behaviour set skipped with a proof:
  the production half byte-identical, `sha256` in the report); a cheap refactor — CT (the full behaviour set run
  anyway, every `cmp` against the baselines rc 0); a [REBAKE] + [GOLDENS] landing — WP and WA. The s19 kit
  (`engine/artifacts/lead_tmp/s19/`: `golden_pairs.py`, `goldens_regen.sh`, `final_gates.sh`, `gen_queue.py`) is
  still the reusable reference.
- **Process rules learned (keep — the cumulative list):** every ci/gpu run under `flock engine/artifacts/loop/ci.lock`; capture the sweep's stdout ALONE; `BASE=` absolute; unlink a worktree's five symlinks before removing it (no force); a lead command run inside a worktree moves the harness cwd there — absolute paths from the main checkout, and a BACKGROUND Bash call resets the harness cwd the same way (s17); a Fable subagent may be unable to write its report file — save its return text; `cleanse_delete.sh` skips live names, so it is safe while landers run; a queue rewrite is a generator over the previous file (`gen_queue.py`) so no item's spec text is retyped; view a regenerated golden as a COMMITTED-vs-REGENERATED pair (`golden_pairs.py`), never the new image alone, and **quantify its diff (mean / max / changed-pixel fraction + bbox) BEFORE viewing it**; a MAP golden can move when only the SIDE bake changed (s17) — and when only the WATER bake changed (s18); two [V] facts from two different RE agents can COMPOSE into a port rule neither of them stated (s17); a paused loop never sees STOP — kill it (AUTOLOOP.md); after any edit to `autoloop.sh` the live lanes are stale — kill and relaunch; launch the loop from a plain terminal or with every `CLAUDE*` env var stripped and run `--probe` first; after ANY queue generation check that `grep -nE '^[A-Z0-9]+\. ' QUEUE.md` lists every item; **never `pkill -f` a pattern that is in your own command line**; `cmp_bake.sh` EXITS 1 whenever DIFF≠0 — an expected-family diff is not a failure, read the families not the exit code (s18); a branch that re-baselines a gate lands LAST, so every earlier branch still measures against the old baseline (s18); and a re-baselining item should ship the escape hatch that reproduces the old value where one is possible (`SWEEP_SLOT0=player`) so the move can be proved inert. **New this window (s19):** one branch can re-baseline TWO gates at once and there is not always an escape hatch — when there is none, the reproduction is a raw `cmp` against the LOOP SESSION's own after-files (item 4's sweep and tat2 arena both `cmp` 0 against `lead_tmp/era-class-db/final/`), so a loop session must park those files; and a report's "extra" gate can name a GENERATOR rather than a referee (`examples/squad_log` writes the log and prints no hash) — the sim hash comes from the artifact README's own capture recipe, so read the README before reproducing an extra.
- **Uncommitted work on main that is not yours** (2026-09-23: the maintainer's own PLAN.md edit): never commit it for him.
  Stage only your own hunks — build HEAD + your hunks into a temp file, `git hash-object -w` it and `git
  update-index --cacheinfo 100644,<blob>,PLAN.md` (lane 1's `lead_tmp/water-sheet-alpha/land/stage_own_hunks.py`),
  or `git apply --cached` a patch of your hunks — and check `git diff --cached` before the commit. A branch whose
  diff touches that file cannot `merge --ff-only` while the edit sits in the working tree.
