# engine/ — the crate and module map

Read this before grepping code. One row per source file: `path` lines — what it owns · key public items. Paths
are relative to `engine/crates/`; row counts are `wc -l` snapshots. Re-run the live report after source edits.
`scripts/size_report.sh` prints the live counts; its `--check-map` exits 1 when a source file is missing here or a
path here is gone. A split (queue SPL1–SPL7, `engine/artifacts/briefs/SPLIT_RECIPE.md`) updates its rows. Each
module's `//!` header is its spec.

## The crates

| crate | owns | depends on (workspace; external) |
|---|---|---|
| `mungefs` | the LVL (`ucfb`), ISO9660 and XDVDFS readers; the runtime never links it | flate2, thiserror |
| `bake` | the offline tool: LVL → GLB / KTX2 / OGG / RON in `engine/assets_baked/` | mungefs, manifest; glam, png, serde(_json), ron, anyhow |
| `manifest` | the bake↔runtime schema, data only | serde |
| `pak` | the game's read-only data: a directory, or the archive appended to the packed game (card PACK) | — |
| `core` | the deterministic sim: soldiers, weapons, AI, posts, collision | glam, serde, parry3d, libm |
| `render` | the wgpu renderer, windowed or headless | wgpu, winit, glam, bytemuck, pollster, image, thiserror |
| `ui` | BF2's HUD (interpreter + layout), egui menus, shell, spawn and loading screens | manifest; egui(-wgpu, -winit), wgpu, winit, glam |
| `input` | gilrs pad → `PadState` | core; gilrs |
| `audio` | kira playback behind the `backend` feature; the streamed music decoder | pak; kira (optional), anyhow |
| `net` | empty home for the locked LAN feature | — |
| `game` | the binary + library: assets, viewer, play loop, goldens | core, manifest, pak, input, render, ui, audio; glam, winit, gilrs, ron, serde(_json), gltf, ktx2, wgpu, anyhow, image; dev: bake, mungefs |

Flow: disc → `mungefs` → `bake` → `engine/assets_baked/` (schema `manifest`) → `game` → `core`, `render`, `ui`,
`audio`. Use `scripts/size_report.sh` for current per-crate and total source, test and example counts.

## Standing invariants — breaking one is not a refactor
- **`core` is deterministic:** no wall clock (bar two diagnostic timers never hashed or read by the sim,
  `core/src/sim/step.rs`'s `ai_wall_ms` and `core/src/ai/tactic.rs`'s `TableStats.secs`; `core::fmath`'s `no_wall_clock_in_core`
  referee allows only those). Gameplay random choices use deterministic draws from `SimRng`, `GameLcg` and
  fixed-input hashes such as the AI draws in `core/src/ai/draw.rs`.
  Simulation decisions never depend on hash collection iteration order. Use `core::fmath` on the `libm` crate.
  Its `BANNED` referee also scans the simulation inputs in `game/src`. Parry uses `enhanced-determinism`.
  `--sim-hash 100` is pinned by a test in `core/src/sim/state/tests.rs` — a failure is a lead decision, never a value to
  update.
- **The runtime never parses LVL:** only `bake` depends on `mungefs` (`game` as a dev-dependency only).
- **`manifest` is one definition for both sides**, round-tripped by `manifest/src/tests.rs`; a new field means
  rebaking every group that writes a manifest (`land.py` turns a declared `rebake=maps` into `maps tat1 tat2`).
- **The bake is byte-deterministic:** any `scripts/cmp_bake.sh` DIFF is your change.
- **The goldens render the PS2 base texture tier** (the tests force `Ps2`).
- **Gameplay follows the openbf rulebook.** Cite original-game evidence in `analysis/`. A `PROVISIONAL` or
  deliberately different value names its replacement measurement with `replace:`, `measure:` or `trace:`.
  Run GPU referees serially via `scripts/gpu_tests.sh`. `scripts/gates.py` runs and compares the gates in
  `engine/artifacts/loop/gates.toml`.

## mungefs — disc formats (bake-only; each header holds the byte layout)
- `mungefs/src/lib.rs` 320 — the `ucfb` chunk walker, FNV-1a tags · `Chunk`, `open`, `fnv1a_lowercase`
- `mungefs/src/iso.rs` 213 — ISO9660 disc reader for `bake extract-iso` · `Iso`; extraction policy lives in `bake/src/main.rs`.
- `mungefs/src/xdvdfs.rs` 354 — Xbox XDVDFS disc reader for `bake extract-xiso` and `bake xbox` · `Xdvdfs`
- `mungefs/src/texture_xbox.rs` 424 — Xbox `tex_`, swizzle included, for maps, donors and the button icons · `Texture`, `is_xbox`
- `mungefs/src/rle.rs` 147 — the PS2 RLE codec (`tex_`, `pal_`, `HGT8`, the terrain's `ALPH` walk) · `decode`, `decode_prefix`
- `mungefs/src/config.rs` 252 — munged config trees (`path`, `lght`, `sky_`, `fx__` incl. particles, `hud_`, `ffx_`, `sanm`, `snd_` / `mus_`, foliage) · `parse_tree`
- `mungefs/src/texture.rs` 402 — PS2 `tex_` + palette → RGBA8 · `Texture`
- `mungefs/src/texture_pc.rs` 1074 — PC `tex_` (D3D9 surfaces) → RGBA8, the tier-2 source · `find_textures`
- `mungefs/src/model.rs` 946 — `modl` → segments + material flags, the MTRL's specular fields and BF2 UV-scroll bytes · `Model`, `Segment`, `Segment::uv_scroll`
- `mungefs/src/model/skin_tests.rs` 381 — existing skin_tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `mungefs/src/model/tests.rs` 363 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `mungefs/src/skel.rs` 408 — `skel` bind poses · `Skeleton`
- `mungefs/src/anim.rs` 662 — `zaa_`/`zaf_` animation banks and rigs · `AnimBank`, `find_banks`
- `mungefs/src/sanm.rs` 197 — BF2 INGAME.LVL `sanm` soldier-animation metadata · `find_sanm`, `SoldierAnimations::get`, `stored_hundredths`; format and timing rules in the module header (`analysis/re-soldier-anim-layers.md` §6.4).
- `mungefs/src/entity.rs` 267 — `entc` classes, `gmod` LOD tables (model + triangle count) and BF2's INFO cloth list · `EntityClass`, `GeomModel`
- `mungefs/src/cloth.rs` 351 — BF2 `CLTH` cloth meshes and collision shapes · `Cloth`, `ClothShape`, `find_cloths`; format in the module header; disc referee: `mungefs/tests/suite/cloth_disc.rs`.
- `mungefs/src/foley.rs` 273 — per-sublevel `ffx_` soldier foley tables · `find_all`, `FoleyLevel`, `FoleyGroup`, `FoleySoldier`; schema in the module header (`analysis/re-foley.md` §1).
- `mungefs/src/world.rs` 280 — `wrld` instances, regions, hints, barriers · `World`
- `mungefs/src/terrain.rs` 1092 — `tern` heightfield, layers and their `ALPH` alphas, cutters, `LOWR` · `Terrain`, `LowResSheet`
- `mungefs/src/terrain/xbox_tests.rs` 192 — existing xbox_tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `mungefs/src/terrain/tests.rs` 568 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `mungefs/src/water.rs` 420 — `tern → WATR` water planes · `Water`
- `mungefs/src/foliage.rs` 493 — terrain `FOLG` masks and `prp_` foliage/path-prop records · `FolgMask`, `FoliageFile`, `FoliageMesh`, `PathLine`; format in the module header and `analysis/re-foliage.md`.
- `mungefs/src/hud.rs` 315 — BF2 INGAME.LVL's `hud_` layouts (a `config` tree + the recovered record names, card BHUD) · `parse`, `HudLayout`, `name`, `NAMES`
- `mungefs/src/font.rs` 163 — CORE.LVL bitmap font sheets and glyph records · `parse`, `Font`, `Glyph`; format in the module header.
- `mungefs/src/fx.rs` 1278 — `fx__` water, HDR and weather configuration (particle effects live in `particle.rs`) · `WaterConfig`, `HdrConfig`, `LightningConfig`, `LightningBoltConfig`, `PrecipitationConfig`, `WindConfig`; decoded effects and rules in the module header.
- `mungefs/src/particle.rs` 1038 — `fx__` particle-effect parsing and PS2 arithmetic · `Effect`, `Emitter`, `Range`, `ps2_mul`, `ps2_sub`; keys, defaults and rounding rules in the module header (`analysis/re-fx.md` §1–§4, [O]).
- `mungefs/src/sky.rs` 534 — `sky_` dome (its `Reflect` shells and `ReflectionColor`), sun, ambient, fog, far range · `Sky`
- `mungefs/src/light.rs` 308 — `lght` lights (BF2 only) · `Lights`
- `mungefs/src/locl.rs` 157 — CORE.LVL localised text tables · `parse`, `find`, `Locl`, `key`; binary format in the module header.
- `mungefs/src/coll.rs` 910 — `coll` meshes and `prim` primitives · `CollisionMesh`, `Primitives`; leaf triangulation and signed-fan repair route through `coll_fan.rs`.
- `mungefs/src/coll_fan.rs` 491 — signed-fan collision repair · `repairs`; region reconstruction and POL1 doorway rationale in the module header (STARC).
- `mungefs/src/path.rs` 442 — `PATH` spawn and AI paths · `Path`
- `mungefs/src/plan.rs` 839 — `plan` graph + BF2 `plnp` patches · `Plan`, `Plnp`
- `mungefs/src/soundbank.rs` 344 — `SampleBank` samples · `SampleBank`
- `mungefs/src/sound_props.rs` 626 — `snd_` properties, layered/parameterized sounds and `mus_` music records · `SoundProp`, `find_named_sublevels`, `MusicRecord`, `find_music`; graph, bus and take-lock fields in the module header and record docs.
- `mungefs/src/stream.rs` 330 — music/VO streams · `Stream`

## bake — the `bake` binary (groups: `scripts/rebake.sh`)
- `bake/src/dlc.rs` 437 — selective optional Kit Fisto/Ventress bake: namespaced dependency classes, their effect layer and sound bank · `bake`, `bake_sounds`.
- `bake/src/dlc_attachment.rs` 106 — Fisto's separate head tentacles folded into his body with their own joints.
- `bake/src/dlc_output.rs` 383 — temporary DLC staging, texture aliases and installation without retail asset replacement.
- `bake/src/main.rs` 727 — the CLI, one subcommand per bake (`map`, `common`, `soldier`, `vehicle`, `effects`, `sounds`, …)
- `bake/src/lib.rs` 179 — module roots · `write_ron`
- `bake/src/gameplay.rs` 462 — apply a reviewed PS2 gameplay correction set before decoding a donor LVL; PS2 content is returned verbatim
- `bake/src/gameplay/program.rs` 518 — the correction set's collision, skeleton and animation-bank programs · `Programs`
- `bake/src/gameplay/rebuild.rs` 613 — rebuild corrected class chunks from the table · `chunk`
- `bake/src/gameplay/terrain.rs` 250 — simulation heights inside the play/query bounds, separate full drawing heights · `apply_terrain`, `apply_render_heights`
- `bake/src/gameplay/tests.rs` 506 — the gameplay table's tests.
- `bake/src/map.rs` 919 — map/common public entry points, input-reduction helpers, manifest re-exports and regression tests · `bake_map`, `bake_common`; calls the private content baker.
- `bake/src/map/content.rs` 1676 — shared map/common LVL orchestration: models, terrain, collision, navigation, effects, classes and pulled SIDE turrets · `bake_content`; output contract in the parent header; `side_classes.rs` owns turret pulls.
- `bake/src/map/render_alignment.rs` 319 — fingerprinted donor-art transforms, end stretches and donor-vertex column caps; preserves PS2 outputs, gameplay headers and collision · `Alignment`
- `bake/src/map/render_groups.rs` 327 — fingerprinted rigid return of displaced donor vertex groups (BF1 Hoth chamber, PC Rhen Var cathedral window) and of the donor shadow vertices outlining them; PS2 never selects a recipe · `Groups`
- `bake/src/map/render_offsets.rs` 394 — fingerprinted donor vertex displacements and face removals on final recovered drawing models; guards unknown inputs and leaves PS2 untouched · `Offsets`
- `bake/src/map/render_offsets_table.rs` 431 — donor fingerprints, final primitive counts and fitted instruction references for Citadel and Felucia · `RECIPES`
- `bake/src/map/render_offsets_data0.rs` 946 — Citadel donor-relative vertex instructions, first group.
- `bake/src/map/render_offsets_data1.rs` 548 — Citadel donor-relative vertex instructions, second group.
- `bake/src/map/render_offsets_data2.rs` 814 — Xbox Felucia bladder donor-relative vertex instructions (also used by PC1 inputs).
- `bake/src/map/render_offsets_data3.rs` 814 — PC2 Felucia bladder donor-relative vertex instructions under its distinct fingerprint.
- `bake/src/map/render_reassembly.rs` 613 — rejoin verified donor-only Jabba room/pipe, Polis Massa platform and capital-ship LOWD panels into canonical render models without changing simulation placements or collision · `models`
- `bake/src/map/texture_alias.rs` 238 — bind native donor far artwork under the public corrected sky name, preserving both tiers and optional-file lifecycle · `far_alias`, `install`
- `bake/src/map/render_recovery.rs` 646 — exact shared-LVL presentation lookup, Jabba axe sheet and source-art recovery for differently partitioned map structures · `shared`, `pieces`; collision and world records remain unchanged.
- `bake/src/map/render_symmetry.rs` 450 — restore missing DEA1 door halves, fit the lower frame from donor opening profiles and share canonical leaf motion · `enabled`, `model`, `door_parts`.
- `bake/src/map/render_chamber.rs` 145 — fingerprinted Xbox HIGH/LOWD chamber annex drawing clipped from canonical contact surfaces, retaining the donor core and donor UV projection · `Chamber`.
- `bake/src/map/render_cloudcity.rs` 90 — copies fingerprinted donor bars into the Cloud City ramp drawing; retains donor topology/materials and canonical simulation inputs · `models`
- `bake/src/map/render_column.rs` 373 — fingerprinted Utapau donor-group deformation, donor-vertex stem bands and full corrected donor HIGH reuse inside the original LOWD header, preserving canonical collision · `enabled`, `model`, `lowd`.
- `bake/src/map/render_lookout.rs` 426 — fingerprinted Citadel donor roof/corner weld profiles, existing-rim floor reassembly and copied upper-lip face; retains donor UVs/materials and gameplay headers · `enabled`, `model`.
- `bake/src/map/render_entrance.rs` 462 — fingerprinted donor corner deformation, front-rim profile and existing-vertex floor/edge reassembly for the Rhen Var tunnel entrance · `Entrance`.
- `bake/src/map/render_faces.rs` 438 — fingerprinted donor edge flips for Rhen Var, Geonosis, Tatooine and Felucia, with one donor-vertex rock face closure; preserves drawing attributes and collision · `Faces`.
- `bake/src/map/render_conform.rs` 814 — fingerprinted plant/tunnel/cliff contact drawing from active collision with donor materials/UVs, planar hangar UV continuation and accepted HIGH-stream reuse for hangar LOWD, preserving gameplay headers · `Conformation`.
- `bake/src/map/collision_standin.rs` 719 — lower-detail missing-model stand-ins from corrected collision shapes and own-copy material/UV projection · `fit`; called only for measured absent model types.
- `bake/src/side_classes.rs` 66 — pull BF2 SIDE/TUR.LVL classes placed by a map but absent from its LVL · `side_classes_lvl`, `pull_side_classes`; joined by `bake/src/map/content.rs`.
- `bake/src/shell.rs` 537 — BF2 shell logo, menu movie packs and music bake · `bake_shell`, `mvs_segments`, `movie_segments`, `read_movie`; ISO fallback and transcode/output rules in the module header (`analysis/re-bf2-shell.md` §1, §4).
- `bake/src/shell_pc.rs` 66 — fingerprinted PC galaxy loops and original Instant Action backdrop; preserves console screen structure.
- `bake/src/alpha.rs` 1427 — PROVISIONAL per-draw ALPHA RULE v2 and BF1/BF2 ELF overrides · `verdict`, `census_verdict`, `bf1_elf_verdict`, `bf2_elf_verdict`, `Bf1Class`, `disc_alpha_tests`, `keep_alpha`; rule, sky exception and census switch in the module header (`analysis/re-material-flags.md`).
- `bake/src/spec.rs` 237 — PROVISIONAL specular-pass bake and shared CORE textures/table · `spec_pass`, `SpecPass`, `pass_textures`, `bake_table`, `bake_core_texture`; alpha retention and pass rules in the module/API docs (`analysis/re-specular-pass.md`, [O]).
- `bake/src/tier.rs` 1110 — texture tiers: PS2, or a larger same-art donor · `Tier`, `tier2`, `same_art`
- `bake/src/detail.rs` 183 — `<name>_dtl` detail maps · `write_detail_for`, `ALPHA_TEST`
- `bake/src/glb.rs` 928 — model GLB writer and per-primitive material/lighting metadata · `model_to_glb`, `world_model_to_glb`, `world_contact_surface_to_glb`, `LightPath`, `light_path`, `glow_path`; alpha and COL0/VU1 rules in the module/API docs.
- `bake/src/far_lowd.rs` 146 — far-scene LOWD geometry extension (UTA1 cliffs) · `extension`; selection rules and runtime routing in the module header (`analysis/re-far-lowd.md`).
- `bake/src/gltf_bin.rs` 129 — the shared GLB BIN builder · `Bin`
- `bake/src/skin_glb.rs` 273 — skinned GLB: skeleton, skin, clips (or a skeleton and clips alone) · `skinned_to_glb`
- `bake/src/terrain_glb.rs` 872 — terrain base/layer GLBs, vertex lighting and UVs · `terrain_to_glb`, `terrain_layers_to_glb`, `gs_vertex_byte`, `layer_uv`; layer order, quad splits and cutter handling in the module header.
- `bake/src/water_glb.rs` 658 — `WATR` → water GLB + config · `water_to_glb`
- `bake/src/terrain_glb/alignment.rs` 549 — donor drawing on corrected heights/cutters, query-grid prefix, donor painted layers · `Surface::build`, `base_glb`, `layers_glb`
- `bake/src/lowres_glb.rs` 230 — `LOWR` → `lowres_<name>.glb` · `lowres_to_glb`
- `bake/src/ktx2w.rs` 227 — KTX2 RGBA8 writer, full mip chain · `write_rgba8`
- `bake/src/lighting.rs` 1056 — map lighting, fog and sky reduction from `sky_`/`lght` · `lighting_from`, `sky_from`; per-game rules and PROVISIONAL choices in the module header.
- `bake/src/soldier.rs` 691 — soldier model/rig/clip conversion, shared types and regression tests; preserves output API exports · `SOLDIER_CLIPS`, `segment_blends`, `to_skinned_model`, `pose_rig`; cloth routes to `cloth.rs`; rules in the module/API docs.
- `bake/src/soldier/tests.rs` 843 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `bake/src/soldier/output.rs` 703 — soldier/weapon GLBs (a weapon's model and textures from several LVLs, a bone-space one at its bind pose), HUD weapon meshes with vertex colors, animation metadata and texture output · `bake_soldier`, `bake_soldier_anims`, `bake_weapon`, `bake_textures_flagged`, `texture_flags`, `weapon_nodes`, `BakedWeapon`; alpha, gloss and fire-point rules in the API docs.
- `bake/src/soldier/render_clips.rs` 449 — Repairs visual clips from owned animation channels and writes meshless companions.
- `bake/src/fpm.rs` 187 — `bake fpm`: a first-person arm model (FPM LVL) as a skinned GLB with its clip-table clips, the `hp_camera` bone added where the rig lacks it · `bake_fpms`, `arm_model`, `BakedFpm`; `analysis/re-first-person-arms.md` §1.
- `bake/src/soldier/lod.rs` 257 — authored `GeometryLowRes` companions, high-rig bind/joint remap at bake time, PS2 strip-count and sphere metadata; issue #69.
- `bake/src/shadow.rs` 224 — BF1 vehicle shadow sidecars beside their models, never a reason to fail a bake · `write_model`, `bake`; `analysis/re-vehicle-shadow.md` §9.
- `bake/src/vehicle_render.rs` 1190 — fingerprinted donor nose/brace, Hailfire quad/crest/pack repairs; subdivided donor rack casings and ear-clipped fitted caps. Collision, joints and simulation headers stay unchanged.
- `bake/src/vehicle_offsets.rs` 65 — vehicle selection of the shared fingerprinted vertex-offset mechanism and numeric-table guards.
- `bake/src/vehicle_offsets_table.rs` 118 — Hailfire donor fingerprints, final primitive guards and displacement references; Xbox and PC, both games.
- `bake/src/vehicle_offsets_data0.rs` 954 — Xbox BF1 donor-relative rack/cap vertex displacements.
- `bake/src/vehicle_offsets_data1.rs` 951 — PC BF1 donor-relative rack/cap vertex displacements.
- `bake/src/vehicle_offsets_data2.rs` 948 — Xbox BF2 donor-relative rack/cap vertex displacements.
- `bake/src/vehicle_offsets_data3.rs` 951 — PC BF2 donor-relative rack/cap vertex displacements.
- `bake/src/vehicle.rs` 1205 — SIDE vehicle assets and `vehicles.ron`, including destruction geometry, collision, bind poses and walker skins · `bake_vehicles`, `VEHICLE_BASES`; output and pose rules in the module header, donor artwork instructions in `vehicle_render.rs` and `vehicle_offsets.rs`; walker metadata in `walker.rs`.
- `bake/src/anim_sets.rs` 354 — resolve soldier animation inventories, sets, masks and root speeds · `Inventory`, `SoldierSets`, `plan_bf1`, `plan_bf2`, `masks`, `root_speed`; algorithms in `bake/src/anim_sets/bf1.rs` and `bf2.rs`; PS2-memory referee: `bake/tests/suite/anim_sets.rs`.
- `bake/src/anim_sets/bf1.rs` 211 — BF1 soldier animation tables and fallbacks · `build`, `Tables`, `FALLBACK`, `weapon_set`; rules in the module and parent `anim_sets.rs` headers.
- `bake/src/anim_sets/bf2.rs` 730 — BF2 soldier animation registration and inherited set resolution · `Registry`, `Registry::register`, `build`, `base_name`; loader rules in the module and parent `anim_sets.rs` headers.
- `bake/src/walker.rs` 313 — walker animation metadata, collision bind points and skin/layer bake · `bake`, `clips`, `prim_points`, `skin_glb`, `skin_glb_of` (the droideka's too), `layer_bones`, `ENTRIES`; entry and pose rules in the module header (`analysis/re-walker.md`).
- `bake/src/droideka.rs` 55 — the droideka's walkerdroid bake: the `cis_walk_droideka` bank's 14 state clips (fold, roll loop, unfold), its skin GLB and collision bind points into the side vehicle manifest · `ENTRIES`, `is_droideka_base`, `bake`, `skin_glb`; the LVL walk is `vehicle.rs`'s `bake_droidekas`; `analysis/re-droideka-unit.md` §4, §7.2 and `analysis/droideka-65.md`.
- `bake/src/bind_pose.rs` 588 — placed-model bind poses, BNAM segments and door/vehicle collision prims (also used by the vehicle bake) · `BindPose`, `pose_bnam`, `BindPose::rest_prims`; transformations and evidence in the module header.
- `bake/src/hardpoints.rs` 201 — model-space bind hardpoints for vehicles and armed buildings across map/common/side bakes · `attach`, `of`, `wants`, `Skeletons`; skeleton lookup and rider axes in the module header.
- `bake/src/turret_rig.rs` 559 — per-node rigid rigs for armed buildings and health/ammo droids · `TurretRigs::of`, `split`, `attach`; splitting, unsupported cross-node triangles and bind metadata in the module header (`analysis/re-turret-draw.md`, `analysis/re-powerup-station.md` §6).
- `bake/src/leafpatch.rs` 381 — leafpatch class properties and attachment replay · `leaf_class`, `attach`, `is_leafpatch`, `is_attach_prop`; replay/caps in the module header (`analysis/re-leafpatch.md` §2–§3, [O]).
- `bake/src/entity_light.rs` 322 — EntityLight classes and prop attachments · `light_class`, `attach`, `Miss`; missing-hardpoint policy in `Miss` (BF2 carrier origin), replay and light fields in the module header (`analysis/re-point-lights.md` §2, `analysis/re-kamino-decks.md` §5).
- `bake/src/cloth.rs` 349 — BF2 worn cloth as rigid-skinned soldier GLB segments; simulation remains deferred (CLOTH2) · `worn_cloths`, `cloth_segment`; eligibility, transforms and pinning in the module header (`analysis/re-cloth.md`, [O]).
- `bake/src/classes.rs` 595 — `entc`/`wpnc`/`ordc`/`expc` → `classes/*.ron` · `bake_classes`
- `bake/src/effects.rs` 470 — layered LVL particle effects and resources → `effects.ron` · `Collected`, `bake_side`, `names`; common/map/side outputs and name dictionary in the module/API docs (`analysis/re-fx.md`).
- `bake/src/door.rs` 463 — BF2 door rigs: triggers, collision/draw motion and sounds · `rigs`, `rig`, `DoorSpec`; clip/transform rules and caveats in the module header (`analysis/re-bf2-doors.md`, [O]).
- `bake/src/foliage.rs` 770 — terrain foliage placement/tint, path props and grasspatch bake → `foliage.bin` · `bake`, `place`, `tint`, `path_props`, `rng_table`, `grass_class`, `grass_atlas`; ELF-table dependency and arithmetic in the module header (`analysis/re-foliage.md`).
- `bake/src/foliage/tests.rs` 509 — the placement and tint against instances read out of the PS2's RAM (the height is `game::viewer::foliage`'s); the ELF table and grass atlas, the grass class, the path props
- `bake/src/sounds.rs` 861 — SOUND LVL bank/property/stream bake → WAVs and `sounds.ron` · `bake_sounds`; per-sublevel inheritance and engine-graph resolution in `read_sublevels` / `engines`; stream outputs in the module header.
- `bake/src/foley.rs` 87 — the LVL's foley tables → `<map>/sounds/foley.ron` (card FOLY; written by `bake sounds`) · `table`, `write`
- `bake/src/sound_names.rs` 367 — names for hashed sound properties · `build_dictionary`
- `bake/src/global_sound.rs` 584 — GLOBAL VO, per-map streams · `bake_global_sound`
- `bake/src/adpcm.rs` 239 — SPU2 ADPCM decode · `decode`
- `bake/src/ogg.rs` 281 — Vorbis with a reproducible serial · `encode_mono`
- `bake/src/beep.rs` 147 — the BF1 loading beep · `bake_beep`
- `bake/src/hud.rs` 463 — common HUD layouts, fonts, sheets and localised text bake · `bake_hud`, `bake_fonts`, `bake_locl`; `bake_common` integration and spawn-screen resources in the module/API docs (`analysis/re-hud.md`).
- `bake/src/hud/art.rs` 232 — validate larger donor sheets and bake sharper UI-only frame, button and HUD art (#114).
- `bake/src/hud/art/filter.rs` 126 — bounded cubic reconstruction of selected small UI sheets with alpha-aware filtering (#114).
- `bake/src/hud/distance.rs` 136 — generate sub-texel glyph contours and padded signed-distance samples from the player's alpha coverage (#114).
- `bake/src/hud/fonts.rs` 292 — choose larger/cleaner matching glyphs from selected input cores, retain source layout metrics and write SDF atlases plus fingerprints (#114).
- `bake/src/map_icons.rs` 146 — write SIDE minimap textures named by class MapTexture properties into the HUD's shared side directories · `bake`; issue #114.
- `bake/src/hud/menu.rs` 404 — local BF2 Light menu glyphs, native advances, SDF alpha erosion, optional PS2 source and mirrored parentheses (#114).
- `bake/src/hud_layout.rs` 168 — shared stock single-player HUD geometry normalization · `normalize`, native source guards and essential PC hierarchy.
- `bake/src/hud_layout/geometry.rs` 969 — measured PS2 numeric geometry corrections, disambiguated by semantic owner · `Geometry`, `CORRECTIONS`.
- `bake/src/hud_layout/text.rs` 218 — native PC font-role cap origins and selected-atlas metrics · `normalize`, `shift_y`.
- `bake/src/hud_layout/tests.rs` 254 — stock source, hierarchy, duplicate mesh, controls and no-op regressions.
- `bake/src/xbox_icons.rs` 136 — the original Xbox pad's `icon_duke_*` sheets from the player's Xbox `common.lvl` · `bake_xbox_icons`, `ICONS`
- `bake/src/loadscreen.rs` 188 — BF1 loading-screen LVLs (the zoom sequence) and BF2's still pools → KTX2 + RON · `bake_loadscreen`
- `bake/src/xbox.rs` 863 — `bake xbox` and `bake xbox-prints`: an Xbox disc's maps, and the PS2 gameplay hashes every bake from one is held to · `bake_xbox`, `print_map`, `write_prints`
- `bake/src/donor.rs` 304 — Card COVR: a BF1 map's Cover hints from the BF2 version of the same map (lead_tmp botre xbox_pc note finding 1). BF2's Yavin Temple authors 973

## manifest — the schema
- `manifest/src/lib.rs` 1404 — shared bake/runtime records and environment root re-exports · `Manifest`, `CommonManifest`, `VehicleManifest`, `ManifestWalker`, `ClassDef`, `FxSet`, `SoundsRon`, `FoleyRon`, `LoadScreen`, `SoldierSkins`, `ManifestTurretRig`, `music_body`, `foliage_bin`; schema compatibility rules in the crate header.
- `manifest/src/environment.rs` 845 — environment schema: weather, local lights/HDR, foliage and `foliage.bin` codec, water, lighting, fog and sky · `ManifestPrecipitation`, `ManifestWind`, `ManifestLightning`, `ManifestPointLight`, `ManifestHdr`, `ManifestFoliage`, `foliage_bin`, `ManifestWater`, `ManifestLighting`, `ManifestSky`; all names re-exported at crate root.
- `manifest/src/anim_sets.rs` 297 — `soldiers/<model>.sets.ron`: a soldier's animation sets over its GLB's clips, and the two games' slot name tables · `SoldierSets`, `Bf1Sets`, `Bf2Sets`, `bf1::weapon_set`, `bf2::base_name`
- `manifest/src/fpm.rs` 152 — the first-person arm clip tables (BF1 5 rows, BF2 4, 11 columns) and the arm GLB path · `COLUMNS`, `BF1_TABLE`, `BF2_TABLE`, `clip_names`, `glb_path`; `analysis/re-first-person-arms.md` §2.
- `manifest/src/hud.rs` 166 — `hud/*.ron`, `fonts/*.ron`, the `locl` text table · `HudIndex`, `HudLayout`, `HudNode`, `HudArg`, `HudFont`, `HudGlyph`, `HudLocl`, `locl_key`
- `manifest/src/shell.rs` 139 — shell asset index and JPEG movie frame-pack schema · `ShellIndex`, `ShellMovie`, `FramePack`; format in the module header (`analysis/re-bf2-shell.md` §1, §4).
- `manifest/src/tests.rs` 1499 — the round trip: what the bake writes, the runtime reads

## core — the sim (tick order: the header of `core/src/sim.rs`)

- `core/src/vehicle/flyer.rs` 917 — PS2 BF1/BF2 flight states, speed, attitude, boost and landing restrictions, and BF2's AI curve follower; scoped fidelity in analysis/re-flyer.md · FlyerParams, FollowerParams, FlyerState, FlyerAi, FlightPhase, step, spline_step, curve_at, untilted.
- `core/src/vehicle/flyer/tests.rs` 401 — flight equation, transition, boost, landing and parked-hash regressions.
- `core/src/sim/flyers.rs` 665 — flight/world down rays, mission bands, provisional contact sweep, slide, staying over the terrain sheet and an empty craft's rest, the BF2 follower's dispatch and normal-pad integration tests · FlightLimits, Sim::tick_flyers.
- `core/src/ai/flyer_drive.rs` 792 — a bot at a flyer's pilot seat: BF1's fly agent, altitude and stick law; BF2's path planner and pursuit curve; invented policy named in its header · can_fly, throttle_for, speed_share, stick_to, drive.
- `core/src/ai/flyer_drive/tests.rs` 1016 — boarding through takeoff and flight, the switch, replay, stick/speed/altitude laws and the follower's curves.
- `core/src/ai/flyer_drive/admission.rs` 134 — BF2 discovery/join class, enable, capacity and direction gates; live-pilot occupancy, atomic plan admission and refused-join loiter; named runtime enable toggles (test callers only).
- `core/src/lib.rs` 72 — module roots and fixed tick rate · `TICK_HZ`, `TICK_DT`
- `core/src/sim.rs` 196 — simulation facade, public re-exports and shared test fixtures · `Sim`, `PadState`, `InputLog`, `Event`, `buttons`; tick order and part ownership are specified in its module header.
- `core/src/sim/rider_health_tests.rs` 245 — a vehicle's health and its riders' are two pools: what reaches each.
- `core/src/sim/state.rs` 1420 — simulation state, counters, cheats, construction and queries; events and the replay hash have rows below · `Sim`, `NavStats`, `Cheats`, `Hazard`, `LETHAL_LANDING_SPEED`, `omni_radius_for`; door methods and hash referees have separate rows below.
- `core/src/sim/state/events.rs` 483 — what the sim tells the game · `Event`, `Call`, `DeathCause`
- `core/src/sim/state/hash.rs` 411 — the whole-state hash the replay gates read · `Sim::hash`

- `core/src/sim/state/tests.rs` 644 — sim state/query regressions and hash invariants, including the pinned `--sim-hash 100` referee.
- `core/src/sim/state/tests/hash_bolts.rs` 177 — HASH field-by-field bolt serialization, predictive-hit continuation and 600-tick repeatability controls.
- `core/src/sim/state/tests/hash_history.rs` 416 — reachable history and continuation probes for state omitted from the sim hash; see its header for the limits of the collision claims.
- `core/src/sim/doors.rs` 82 — door ticking and opening-leaf queries · `tick_doors`, `door_opening`, `opening_leaf_over` (DR2 / MOUTH2 / DOORS).
- `core/src/sim/trace.rs` 200 — SHASH read-only canonical per-tick hash stream, quantization and bounded full tuples · `TraceWriter`, `POSITION_QUANTUM`, `HEALTH_QUANTUM`, `CAPTURE_QUANTUM`, `TIMER_QUANTUM`.
- `core/src/sim/trace/tests.rs` 177 — SHASH replay equality/first divergence, independent wire vectors, field sensitivity, read-only controls and legacy hash pins.
- `core/src/sim/input.rs` 839 — pad/button definitions, input logs and replay, player aim-assist acquisition and reticle picks; includes replay, pad-slot and assist tests · `PadState` (`fire_lag`: a press fired part way into its tick's turn; `pick`: a spawn-screen mouse pick, `ScreenPick`), `buttons`, `InputLog`, `AimTarget`, `Sim::run_log`, `Sim::magnet_at`, `cone_score`; see the module header and `analysis/re-aim-assist.md`.
- `core/src/sim/input/tests.rs` 926 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `core/src/sim/population.rs` 91 — optional per-team combatant targets; reserved human slots, cap changes and replenishment limits.
- `core/src/sim/native_teams.rs` 427 — directed team relations, native member pools, owned/ally command-post placement and separate respawn waves · `AllySpawn`, `Sim::add_native_team`.
- `core/src/sim/spawn.rs` 268 — shared spawn stages, class weights, wave clock and placement helpers; facade for runtime and regression modules.
- `core/src/sim/spawn/runtime.rs` 1348 — spawning, no-overspawn shedding, AI class weights, spawn screens/post selection, the class gate with the slot's career costs (UNLK3) and spawn-floor reachability · `Sim::spawn`, `Sim::spawn_ai`, `Sim::spawn_posts`, `Sim::class_refusal`, `Sim::spawn_cost`, `Sim::floor_flood`, `Sim::floor_path`. Both floor searches track accepted cells by grid position and storey. Their queues determine traversal order.
- `core/src/sim/spawn/runtime/floor.rs` 210 — bounded floor floods, capsule-clear steps and floor paths · `Sim::floor_reach`, `floor_flood`, `soldier_clear`, `floor_path`, `floor_path_near`
- `core/src/sim/spawn/class_change.rs` 277 — issue #28: change class at a friendly command post while alive (use-button reach and facing, in-place swap, health carried, per-class per-life ammo) · `Sim::class_change_post`, `class_change_open`, `class_change_accept`.
- `core/src/sim/spawn/tests.rs` 1266 — spawn waves, class shares, command walkers, respawn seating and post selection regressions; shared screen fixtures.
- `core/src/sim/spawn/tests/class_change.rs` 354 — issue #28 class-change regressions: reach, facing, team posts, health carry, ammo memory and death reset.
- `core/src/sim/spawn/tests/screens.rs` 499 — spawn-screen flow, carousel timing, class refusal and replay regressions.
- `core/src/sim/abilities.rs` 86 — shared officer radius query and activation · `ability_targets`, `fire_ability`.
- `core/src/sim/blast.rs` 1001 — shared explosion area-damage path · `Sim::explode`, `soldier_blast_sphere`, `BLAST_GATHER_MAX`; gather, LOS, falloff and team rules are specified in the module header and `analysis/re-vehicle-death.md` §2–§5.
- `core/src/sim/bounds.rs` 1048 — playable-level floor resolution, arc/hub walkability, rim/kill-floor tests and lost-bot floor return paths · `Sim::resolve_nav_floors`, `Sim::level_at`, `Sim::rim_drop`, `Sim::return_point`, `Sim::return_path`, `Sim::floor_walk_to`, `Sim::floor_line`, `ReturnPath`, `Sim::arc_unwalkable`, `Sim::lands_under_the_map`, `Level`; floor/return rules are specified here and in `analysis/re-kill-floor.md`. Trapped hubs are pinned by `game/tests/suite/nav_walls.rs`; kill-floor/under-map deaths bypass invincibility.
- `core/src/sim/bounds/tests.rs` 646 — floor resolution, arc walled/walkable, per-hub start check, rim and kill-floor regressions.
- `core/src/sim/buffs.rs` 185 — shared timed buff state, refresh, hashing, healing and attributed poison · `Buffs`, `tick_buffs`.
- `core/src/sim/combat.rs` 785 — bolt impacts, damage, kills/tickets and impact explosions; environmental death and cheat exemptions; includes damage/death regressions · `Sim::kill_environment`; see module header, with blasts routed through `Sim::explode`.
- `core/src/sim/combat/tests.rs` 1176 — the bolt, damage and kill tests.
- `core/src/sim/round.rs` 622 — command-post registration/capture, conquest bleed, difficulty handicap and round outcome; includes capture/defeat/handicap tests · `Sim::add_cp`; rules are in the module header.
- `core/src/sim/seats.rs` 949 — human/bot vehicle entry, explicit AI-call/player-order claim-budget reconciliation, follower boarding, pickup clock/output and claim-maintenance write-back, all-AI bailout dispatch/census, takeover and exit placement, driver input and rider transport · `Sim::use_pass`, `Sim::ai_use_pass`, `Sim::enter_target`, `Sim::enter_seat`, `Sim::enter_vehicle`, `Sim::exit_vehicle`, `Sim::shut_in`, `Sim::shut_in_solid`, `driver_input`; see module header, `analysis/re-vehicles.md` §5 and `analysis/re-hover.md` §5.2.
- `core/src/sim/seats/tests.rs` 589 — vehicle entry/refusal/takeover, follower boarding/seat contention, driving, exit placement and input-shaping regressions on flat ground with the TAT1 skiff fixture.
- `core/src/sim/damage_regions.rs` 253 — authored `damage` regions (Mustafar's lava and steam): the name's keys, and per-update damage to soldiers and vehicles whose sphere touches one · `DamageRegion`, `Sim::tick_damage_regions`.
- `core/src/sim/self_turrets.rs` 426 — placed BF2 self-piloted building controllers, targeting and shared seat-gun dispatch (issue #91).
- `core/src/sim/guns.rs` 531 — mounted weapon ticking, seat switching and live look/eye/barrel queries for human and bot riders · `Sim::next_seat`, `Sim::switch_pass`, `Sim::seat_gun`, `Sim::seat_look`, `SeatLook`; shared weapon firing and bot fairness checks are specified in the header and `analysis/re-vehicles.md` §5/§9.
- `core/src/sim/guns/tests.rs` 1072 — seat-switch/charge-tenure, human charge release, firing/aimer, occupancy-hash and bot-gunner regressions, including Mygeeto turret camera/building, barrel and muzzle alignment (TURR).
- `core/src/sim/step.rs` 1421 — fixed-tick orchestration and after-writeback wait-start and routed-departure calls, unit separation, soldier wall/zoom handling and human aim rays · `Sim::step`, `Sim::aim_hit`, `Sim::tick_zoom`; tick order is in `sim.rs`, aim details in `analysis/re-aim-assist.md` §10 and zoom details in `analysis/re-zoom.md` §5; regressions live in `core/src/sim/step/tests.rs`.
- `core/src/sim/step/tests.rs` 1581 — sim tick, salvo blast-clearance, separation and determinism regressions, extracted from the step implementation.
- `core/src/sim/step/tests/scratch.rs` 45 — retained AI scratch versus fresh allocation: hashes, event order and buffer lifetime.
- `core/src/sim/vehicles.rs` 918 — vehicle slots, pickup/charge-tenure, pending-call and boarding-maintenance lifecycle, occupancy/team synchronization, hull damage/death, pad spawning/decay, vehicle ticking and collision hull synchronization; includes lifecycle/hash/pad/hull tests · `Sim::spawn_vehicle`, `Sim::seat_unit`, `Sim::unseat_unit`, `Sim::vehicle_team`, `Sim::damage_vehicle`, `Sim::impact_vehicle`, `Sim::destroy_vehicle`, `Sim::add_vehicle_pad`, `Sim::tick_walkers`, `Sim::sync_hulls`. Only the seat/unseat writers update both `Seat.occupant` and `Unit.mount`; lifecycle rules are specified in the header.
- `core/src/sim/traffic.rs` 188 — walker/hover priority, fixed-tick hold and bounded bypass direction
- `core/src/sim/traffic/tests.rs` 251 — strict priority, contact hold/hash, separation, nearest contact and mesh-mover controls
- `core/src/sim/vehicles/tests.rs` 1160 — the vehicle lifecycle tests.
- `core/src/sim/droidekas.rs` 358 — the droideka's sim step: pad edges into fold/deploy/shield/jump and the player's held ball sprint, the bot form order, collision capsules by form, bot barrel convergence within authored aimer limits, fire gating and barrels, events (`Event::Droideka`), shield and ordnance spheres in the world · `Sim::step_droideka`, `Unit::ai_eye`, `Unit::droideka_point`, `Unit::droideka_shield`, `Unit::droideka_spheres`, `Unit::droideka_gun`, `Unit::droideka_muzzle`, `Unit::lend_droideka_shield` (blasts through `Health`), `Unit::separation_radius`.
- `core/src/sim/droidekas/tests.rs` 881 — Droideka sim inputs, look preview and no-zoom contracts.
- `core/src/sim/droidekas/tests/aim.rs` 316 — bot droideka ramp hits, barrel convergence and aimer limits, world-level pitch, slope deployment, class-eye perception and human-shot controls.
- `core/src/sim/step/tests/jet.rs` 162 — JETP in the sim: the spawn tank and the second tap, the infinite-fuel cheat, a jet replay's hash, a hover against a wall, the mission ceiling.
- `core/src/sim/step/tests/ramps.rs` 81 — running jump clearance across uphill, level and downhill mesh floors, both games' rules, walk/sprint speeds and press timings; BF1 uphill takeoff preserves its climbing momentum.
- `core/src/sim/look.rs` 208 — keyboard + mouse look: seat-gun look by angle, hover/walker mouse driving (walker free yaw with authored hull following), the between-tick view preview (the mouse's, and a controller's held stick with the aim assist) · `LookPreview`, `MOUSE_STEER`, `Sim::look_preview`, `seat_look`, `walker_aim`, `mouse_drive`; rules in `analysis/plan/FEATURE_SET.md` (Platform & meta).
- `core/src/sim/look/tests.rs` 360 — mouse aim/respawn independence, mouse look past the turn rate, pitch/prone limits, zoom scaling, preview == tick, old-log compatibility and mouse-log replay.
- `core/src/rng.rs` 49 — SCE `rand()`, the seeded RNG · `SimRng`
- `core/src/lcg.rs` 140 — the gameplay LCG (spread, delays) · `GameLcg`
- `core/src/fmath.rs` 649 — deterministic `libm` transcendental entry points · `sin_cos`, `atan2`, `asin`; the method/path/function-value/glam referees check recursive `core/src` and non-presentation `game/src` usage, with counted drawing-expression exceptions in mixed files; `no_wall_clock_in_core` allows wall-clock reads only in two diagnostic timers. Scope and allowed operations are specified in the header.
- `core/src/soldier.rs` 1227 — soldier stance, ground/air movement, floor contact, roll, the prone double tap, sprint/energy, jump and jet rules (card JETP: fuel, the jump-jet, the hover, the Jedi double jump, the fly-height ceiling) · `Soldier`, `SoldierConstants`, `GroundRules`, `AirEnv`; detailed rules and provenance are in the module header, `analysis/re-stance-roll.md` and `analysis/re-soldier-ground.md` (including SKIRT).
- `core/src/soldier/step.rs` 393 — one soldier tick on foot · `Soldier::step_looking`
- `core/src/soldier/air.rs` 423 — knockback, take-off, the jet pack and steering in the air · `Soldier::knock`, `take_off`, `step_jet`, `air_steer`
- `core/src/soldier/tests.rs` 1440 — soldier stance/ground/jump reference vectors and uphill/contact regressions, including Kamino pod-skirt behavior (SKIRT); see `analysis/re-stance-roll.md` §5.
- `core/src/soldier/tests/fc_timers.rs` 56 — FC-FALL measured BF2 stand/sprint one-second boundary, overrun and landing-reset controls.
- `core/src/soldier/tests/fc_sprint.rs` 58 — Issue #103 repeated refused-roll edges preserve sprint drain and empty-bar exit.
- `core/src/soldier/tests/juni.rs` 436 — JUNI merged BF2 jump laws: matched-class take-off and complete-flight rows, real Sim caller, eligibility/lock/energy boundaries and original Thrown controls.
- `core/src/soldier/tests/jet.rs` 678 — JETP jets against `analysis/re-jetpack.md` §6: the kick, the border and energy gates, the dark trooper's leap and fuel, the jet trooper's hover speeds, flight time and refill, BF1 on BF2's rules, the tap-off, landing, the ceiling, the Jedi double jump, Vader's hover, the hash.
- `core/src/soldier/tests/prone_tap.rs` 159 — the `[ours]` prone double tap (2026-10-28): every stationary tap and JUMP transition, the 1 s window edges, the roll unchanged, and the bots' press mirror (`stance_tap_after`, `jump_stands_after`) against the step.
- `core/src/unit.rs` 1372 — combat-unit state/handles, active weapon selection, camera sections/chase camera, aim/fire points, zoom and target aim-error blocks · `Unit`, `UnitSpec`, `UnitHandle`, `Unit::active_weapon`, `Zoom`, `CameraSection`, `camera_section`, `IdealCamera`, `AimBlock`, `AIM_BOX_MIN`/`MAX`, `SPAWN_PITCH`; camera/error rules and tests live with these types (AIMA, ZOOM, CLSE2).
- `core/src/unit/tests.rs` 216 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `core/src/droideka.rs` 429 — the droideka entity (card DEKU/DEKS): the 14-state form machine (fold, unfold, the step dispatch), shield toggle, ordnance spheres, water wear and deterministic hashing; class, shield, ball and walk forms in `core/src/droideka/` · `Droideka`, `Form`, `Order`, `Cue`, `Sphere`, `local_to_world`, `ray_sphere`; module rules in `analysis/re-droideka-unit.md` and `analysis/droideka-65.md`.
- `core/src/droideka/tests.rs` 888 — Native droideka form, motion and shield controls.
- `core/src/droideka/class.rs` 363 — the droideka's class keys from both discs (named keys and the uncracked `HASH_*` ones), clip frames, camera sections and the rates derived from them · `DroidekaClass`, `ClipInfo`, `default_clips`, `CAMERA_*`.
- `core/src/droideka/shield.rs` 116 — the shield's class keys and points: the damage it takes first and what it passes on (`analysis/re-droideka-unit.md` §2) · `ShieldClass`, `Shield::absorb`.
- `core/src/droideka/ball.rs` 160 — the ball form: the sprint and the jump on the shared bar (drain, jump cost, lockout) and the rolling integrator.
- `core/src/droideka/walk.rs` 138 — the walk form: leg steps, turns and the walked move.
- `core/src/vehicle.rs` 562 — vehicle entity/spec, seats/mounts, team/hull state and collision damage · `Vehicle`, `VehicleSpec`, `SeatSetup`, `VehicleHandle`, `Seat`, `SeatKind`, `Mount`, `is_ring`, `impact_damage`, `collision_desc`; see module header and `analysis/re-vehicles.md` §5–§8. Occupancy writers are in `core/src/sim/vehicles.rs`.
- `core/src/vehicle/hover.rs` 820 — BF2-ground-handling hover integrator: drive, contacts, springs, boost/energy and bike jump · `HoverParams`, `HoverInput`, `HoverState`, `JumpParams`, `HullBody`, `HullShape`, `ContactSource`, `step`, `drive`, `springs`, `contacts`, `terrain_sphere`; called by `Sim::tick_hovers`. Order, frames, formulas and provisional cases are specified in the header, `analysis/re-hover.md` and `analysis/re-vehicle-rest.md` §5; input timer shaping belongs to the seat pass.
- `core/src/vehicle/tow.rs` 46 — Posed walker tow primitives and world-space ray query · `TowPrimitive`, `TowFrames`.
- `core/src/vehicle/walker.rs` 1006 — BF2-ground-handling walker integrator, clip-driven locomotion, terrain/leg/building contacts and drawn leg heading · `WalkerParams`, `WalkerState`, `Clip`, `LegPair`, `WalkerWorld`, `step`, `legs_rot`, `TRANSITIONS`; see module header and `analysis/re-walker.md` §2–§9.
- `core/src/vehicle/walker/tests.rs` 409 — walker reference-vector and locomotion/boost/turn/leg/clip regressions using `analysis/re-walker.md` §14.6's AT-ST fixture on flat ground.
- `core/src/vehicle/gun.rs` 841 — mounted-gun setup/state, aimer and model-frame angle helpers · `Aimer`, `GunSetup`, `GunWeapon`, `GunState`, `AIM_RAY`, `dir`, `angles`, `turned`; see module header and `analysis/re-vehicles.md` §5/§9 for limits, rates, fire points and pilot behavior.
- `core/src/vehicle/hover/tests.rs` 989 — hover equation/reference-vector, determinism, boost/jump and terrain/building/hull-contact regressions; owns the shared TAT1 skiff fixture · `skiff`; see `analysis/re-hover.md` §4/§6/§7/§9/§13.5 and HSTEP/HHUL.
- `core/src/vehicle_pad.rs` 189 — vehicle spawn-pad definitions and counters · `VehiclePad`, `PadDef`, `PadClass`; `analysis/re-vehicles.md` §7 and the module header specify countdown, spawn-count and idle behavior; faction class resolution belongs to `game::vehicle_pads`.
- `core/src/weapon.rs` 1249 — weapon class data, fire cycle (with the `ammo` and `clip` cheats, CHEAT2), spread, cyclic shot tilts and replay state, clips, grenade/zoom keys and AI range; re-exports ordnance/damage types · `WeaponClass`, `Health`, `WeaponState::with_clips`, `Thrown`, `floats` (an ODF value's N floats), `zoom_press`, `zoom_at`, `ai_range`; `Thrown::throw_as_bf2` applies BF2 throw behaviour to BF1 classes fielded on BF2 maps. BF1 maps use BF1 grenade rules (GRNP). Rules and provenance live in source docs and `analysis/re-blaster-damage.md`.
- `core/src/weapon/class.rs` 654 — weapon class parsing, native rifle presets, range, zoom and capability queries · `WeaponClass` methods; re-exported type and method paths unchanged
- `core/src/weapon/ability.rs` 65 — parsed BF2 radius ability configuration · `Ability`.
- `core/src/weapon/dispenser.rs` 62 — dispenser properties for pickups and placed explosives, launch speed and failed-shot ammo restoration · `Dispenser<I>::new`, `Dispenser::speed`, `WeaponState::refund_dispense`; reuses grenade cycle defaults; `analysis/re-grenades.md` §5.1.
- `core/src/weapon/repair.rs` 61 — authored repair-tool channels and amounts · Repair::from_props; analysis/re-build-zones.md.
- `core/src/weapon/ordnance.rs` 573 — ordnance/explosion class data and parsing, blast falloff, damage descriptors and health/shield application · `OrdnanceClass`, `ExplosionClass`, `Health`, `HealthType`, `DamageDesc`, `blast_falloff`; re-exported by `weapon`; provenance in the parent module.
- `core/src/weapon/tests.rs` 1123 — re-blaster-damage §9's reference vectors and the state-machine behaviours
- `core/src/weapon/tests/dispenser_bounds.rs` 85 — FC-BOUNDS held re-trigger, inactive weapon and unusual charge-duration regressions.
- `core/src/weapon/tests/ammo.rs` 136 — loaded and reserve clips, finite exhaustion, reload timing and conservation
- `core/src/sticky.rs` 599 — thrown-ordnance class and instance physics: flight, bounce, stick/rest and fuse · `StickyClass`, `Sticky`, `bounce`, `throw_dir`; see module header and `analysis/re-grenades.md` §4 `[O]`. World sweeps/blasts belong to `core/src/sim/ordnance.rs`.
- `core/src/laser.rs` 324 — render-only bolt/laser appearance and light properties · `LaserLook`; setter, fade, width and per-game light rules are in the header and `analysis/re-bolt.md` §1–§5 `[O]`.
- `core/src/flash.rs` 309 — render-only muzzle-flash class properties, fade and aim matrix · `MuzzleFlashLook`, `matrix`; see module header and `analysis/re-muzzle-flash.md` §1–§4 `[O]`.
- `core/src/impact.rs` 217 — render-only impact/trail/explosion effect selection · `Surface`, `ImpactLook`, `ExplosionLook`; see module header and `analysis/re-fx.md` §7 `[O]` for surface slots, defaults and per-game effect lists.
- `core/src/projectile.rs` 530 — bolts, one predictive ray at spawn and complete replay serialization · `Bolt`
- `core/src/sim/pickups.rs` 518 — dropped-item tables/state, drop and pickup ticking · `ItemClass`, `DropTable`, `Pickup`, `Sim::add_item_class`, `drop_item`, `dispense_shots`, shared `spawn_pickup`, `tick_pickups`; the buff state is `Buffs` in `buffs.rs`; rules are in the header and `analysis/re-powerups.md` §3–§5. Damage applies buffs through `buff_scale` in `combat.rs`.
- `core/src/sim/abilities/tests.rs` 352 — officer effects, duration, radius, self targeting, bot policy, ammo and replay regressions.
- `core/src/sim/pickups/tests.rs` 499 — pickup drop-table, lifetime/pool, per-game touch/grant/velocity, energy/buff and hash referees; see `analysis/re-powerups.md` §3–§5.
- `core/src/sim/pickups/tests/dispensers.rs` 244 — dispenser supply, both firing channels, failed-spawn refunds, pool/owner limits, flight, lifetime, hashing and ammo-refill exclusion.
- `core/src/sim/stations.rs` 418 — health/ammo/vehicle station class/state and serving ticks · `StationClass`, `StationAnim`, `Station`, `Sim::add_station_class`, `tick_stations`; reach, clock, grants and destruction rules in `analysis/re-powerup-station.md` §3/§4/§14.
- `core/src/sim/repair.rs` 394 — fusion-cutter target acquisition, energy-gated pulses, station rebuilding and fallen building turrets (`Target::Building`) · Sim::repair_station; analysis/re-build-zones.md.
- `core/src/sim/building_turrets.rs` 332 — a map's building turrets: spawn on their building, the fall to wreck / pad / hidden (model and collision), no regeneration (ours), the cutter's rebuild at MaxHealth · `BuildingTurret`, `TurretFall`, `Sim::add_building_turret`, `Sim::repair_building`, `Sim::building_down`; analysis/re-build-zones.md 1 (card TBLD).
- `core/src/sim/building_turrets/tests.rs` 321 — a fallen turret's bodies and rise at MaxHealth only, no rise by itself in 300 s, the cutter's heal / rebuild / build rates (mutation-checked).
- `core/src/sim/repair/tests.rs` 522 — repair/rebuild progress, relation/range/occlusion/energy, collision restoration and hash regressions.
- `core/src/station_motion.rs` 93 — deterministic station idle/serving clock shared by collision and drawing; its state and private generator enter the replay hash.
- `core/src/sim/stations/tests.rs` 251 — station rate/remainder, reach, team/liveness, ammo-cap and serving-state referees; see `analysis/re-powerup-station.md` §3/§4.
- `core/src/sim/stations/tests/destruction.rs` 258 — station bullet and splash damage, building/team multipliers, wreck collision, service shutdown, chained explosions and deterministic fixed-step coverage.
- `core/src/sim/ordnance.rs` 748 — soldier weapon channels/cycles/switching, held projectile shots, grenade launches and flight/contact/fuse/blast ticking · `Sim::held_weapons`, shared `Sim::soldier_shots` dispatch, `Sim::launch`, `Sim::tick_stickies`; see module header and `analysis/re-grenades.md` `[O]`. Includes PS2 throw-vector referee `core/tests/data/grenade_ps2_bf2.txt`, BF1-as-BF2 and mesh-floor regressions (GRNP/GRFL).
- `core/src/sim/ordnance/tests.rs` 1400 — the soldier weapon and grenade tests.
- `core/src/aim_assist.rs` 579 — BF1 aim-assist magnetism · `apply`
- `core/src/water.rs` 136 — the water plane as the sim sees it: one level plus the WMAP wet-cell mask (a nibble per 32 m cell, the PS2's clamped lookup), and the downward crossing tested at the crossing's cell · `WetMask`, `Plane`; `Sim::water_plane` / `Sim::water_under`; `analysis/re-fx.md` §15. The mask and oscillation flag are load-time inputs like `water_level`; `Sim::water_plane` adds the deterministic flat-water bob from the sim tick.
- `core/src/sim/water.rs` 55 — PS2 soldier drowning and fire-point submersion; fixed-tick breath state lives on `Soldier`, water height/mask/bob are shared map inputs. `core/tests/drowning.rs` pins measured timelines, both weapon channels, queued salvos, surface resets and replay hashes.
- `core/src/boundary.rs` 96 — the map boundary's point test and countdown time; only the remote droid reads it so far · `Boundary`, `leaving_time`.
- `core/src/heightfield.rs` 374 — terrain triangle height/normal queries and bounds-assisted ray-march probes · `Heightfield`, `HeightProbe`; interpolation and probe rules are specified in the module header.
- `core/src/collision.rs` 334 — collision facade, public re-exports, shared conversions/fixtures and step/ground constants · `STEP_HEIGHT`, `WALKABLE_MIN_NY`, `TERRAIN_BURY`, `NO_GROUND`; layer semantics, determinism and the parry 0.22.1 contact workaround are specified in its header.
- `core/src/collision/shape.rs` 1372 — collision geometry, body/layer/hull/cutter types and mesh construction · `Capsule`, `Aabb`, `Shape`, `trimesh`, `Layers`, `Body`, `HullPart`; includes layer-mask and analytic-cylinder oracle tests. `Body::ground_extent` supplies the vehicle footprints in `core/src/ai/avoid.rs`; detailed shape rules live here.
- `core/src/collision/shape/mesh_ray.rs` 81 — a mesh ray with parry's traversal and triangle hits, sharing the ray's reciprocals between the BVH's boxes · `cast`.
- `core/src/collision/shape/mesh_ray/tests.rs` 131 — the box cost and whole mesh rays against parry, bit for bit, ties and backfaces included.
- `core/src/collision/world.rs` 1393 — collision-world storage, deterministic broad-phase candidates, moving map bodies and live vehicle hulls · `CollisionWorld`, `BodyGrid`, `CollisionWorld::move_body`, `CollisionWorld::set_hull`, `HullFilter`; ordering, mover envelopes and hull filtering are specified in the header/type docs.
- `core/src/collision/traffic.rs` 79 — swept walker/hover hull translations; local-to-world contact normals
- `core/src/collision/ceilings.rs` 153 — the capsule sweep a rising soldier stops under · `CollisionWorld::ceiling_sweep`
- `core/src/collision/rays.rs` 1285 — terrain-cut, ground/world/terrain ray, enclosing-mesh and lead queries · `is_cut_by`, `CollisionWorld::ray_down`, `CollisionWorld::ray`, `CollisionWorld::ray_with`, `CollisionWorld::ray_past`, `CollisionWorld::enclosing`, `ray_heightfield`, `ray_terrain_ps2`; `ray_past` skips a turret's own building (TURR), `enclosing` serves shut-in tests (VMESH). BF2 grenade terrain-segment rules are specified here and in `analysis/re-grenades.md` §13.2 `[O]`.
- `core/src/collision/cost.rs` 151 — what a tick's rays cost, counted for the slow-frame log and read by nothing in the sim · `RayCost`, `WorstCast`, `take`.
- `core/src/collision/walls.rs` 10 — facade for soldier wall queries and their tests; preserves wall-query constant exports.
- `core/src/collision/walls/queries.rs` 1331 — soldier wall push-out/sweeps, ledges/substeps, hull-shape contacts and parry contact workaround · `CollisionWorld::push_out`, `CollisionWorld::push_out_soldier`, `CollisionWorld::push_out_soldier_hulls`, `CollisionWorld::push_out_soldier_from`, `CollisionWorld::wall_sweep`, `CollisionWorld::wall_sweep_body`, `CollisionWorld::wall_sweep_small`, `shape_contacts`, `mesh_contact`, `pushes_past`; detailed rules are in source docs, `collision.rs` and `analysis/re-soldier-walls.md`.
- `core/src/collision/walls/tests.rs` 1339 — wall push/sweep, substep, ledge, hull contact and parry-mirror regression tests; paths remain `collision::walls::tests`.
- `core/src/region.rs` 225 — box / sphere / cylinder regions · `Region`
- `core/src/cp.rs` 598 — posts: capture machine, bleed, domination · `CommandPost`
- `core/src/door.rs` 291 — BF2 self-opening door state, triggers, sound edges and clip pose sampling · `Door`, `DoorBody`, `Trigger`, `Edge`, `sample`; see module header and `analysis/re-bf2-doors.md` `[O]`.
- `core/src/battle.rs` 367 — reinforcements, no-overspawn · `Battle`
- `core/src/announcer.rs` 798 — pure announcer request rules, serialized voice queue, speaker handles and script bleed state · `Announcer`, `VoiceQueue`, `PostLines`, `ScriptBleed`, `Line::speaker`; `game::announce` owns audio playback. Gates and timing are specified in the header and `analysis/re-announcer.md`.
- `core/src/music.rs` 740 — pure battle-music state, scheduling and playback/fade rules · `Ambient`, `Player`, `Music`, `Rule`, `state_bf1` (PROVISIONAL), `state_bf2`; `game::music` owns audio playback. See module header and `analysis/re-ambience-music.md` §3 `[O]`; includes PS2-state and Naboo timeline referees.
- `core/src/nav.rs` 1435 — authored planning graph, PS2 locate/router, XZ unit-locate deviation D17, barrier detours and switches · `NavGraph`, `locate`, `locate_unit_on`, `detour`, `set_barriers`; geometry/search rules and provenance are in the module header.
- `core/src/nav/barriers.rs` 147 — authored barrier geometry: containment, segment crossings and silhouette corners · `Barrier` methods
- `core/src/nav/tests.rs` 1139 — planning-graph locate, route-choice/BFS, jet-arc, route-byte, barrier and corner-detour regressions (LOC3/OBST2).
- `core/src/nav/flank.rs` 321 — derived alternative flank routes and match-share draws (OURS, D5) · `FlankTable`, `FlankRoute`, `NavGraph::flank_table`, `NavGraph::authored_route`, `share_draw`, `origin`, `join`; built by `Sim::build_flank`. Construction, acceptance and joining rules are in the header and `analysis/bot-design.md` 6.2.
- `core/src/nav/flank/tests.rs` 138 — flank-table alternate-route acceptance, traversal and share-draw regressions on small graphs; owns shared diamond-graph fixture · `graph`.
- `core/src/route_patch.rs` 317 — BF2 `plnp` route patches: Block/UnblockPlanningGraphArcs on the route table · `RoutePatches`
- `core/src/ai.rs` 865 — AI facade, public re-exports, shared fixtures and shared tuning constants (PROVISIONAL, including vehicle boarding/driving) · `HOLD_RANGE`, `THINK_PERIOD`, `PROBE_DIST`; layer ownership is in its header and `analysis/bot-design.md`, with source evidence in `analysis/re-soldier-ai.md`.
- `core/src/ai/items.rs` 149 — BF2 pickup detour scan, need checks, waypoint return and deterministic state · ItemSeek, ItemDetour; analysis/re-powerups.md.
- `core/src/ai/droideka.rs` 58 — bot droideka rules (`analysis/re-droideka-unit.md` §5): roll to travel, deploy to fight or recover, fights only inside 10 m or when hit, never targets flyers, stands to shoot and raises the shield over half · `form_order`, `takes_fight`, `steer`, `SIGHT_FIGHT`, `SHIELD_RAISE`.
- `core/src/ai/droideka/tests.rs` 177 — Droideka bot form and shield-choice contracts.
- `core/src/ai/items/tests.rs` 258 — pickup-seeking need, range, cooldown, route-return and replay regressions.
- `core/src/ai/state.rs` 1438 — persistent per-unit AI state and hash, including generation-safe pilot pickup clocks and BF1 departure attempt, BF1 release time, independent boarding maintenance and next-think call identities, human boarding issuer provenance and autonomous-search deadline, grouped by owning layer, plus un-hashed think diagnostics · `AiState`, `FightState`, `Route`, `NavTick`, `SteerPick`, `RouteSource`, `RideState`, `LostState`, `HoldState`, `StallState`, `MoverState`, `Walk`, `WalkAsk`, `Behaviour`. Field rules and hash regression live here.
- `core/src/ai/state/bookkeeping.rs` 365 — navigation tick diagnostics, neutral AI initialization and retained-state serialization · `NavTick`, `AiState::hash_into`; public paths retained by the facade
- `core/src/ai/state/tests.rs` 393 — AI state lifecycle and optional replay-hash regression tests; extracted unchanged to keep the runtime module below the strict size limit.
- `core/src/ai/stall.rs` 802 — last-resort stalled-bot recovery/recycling (OURS, PROVISIONAL) and its regressions · `watch`, `send_off`; net-progress/failed-window rules and the once-per-place floor-walk escape are specified in its header/source docs; off-level recovery belongs to `LostState`/bounds.
- `core/src/ai/command.rs` 358 — squad-order AI scratch state and cleanup, with follow/hold/release regressions · `CmdState`, `AiState::cmd_clear`; execution belongs to `squad::steer`.
- `core/src/ai/draw.rs` 427 — deterministic per-unit hash randomness, named salts and stance draws; includes stance regressions · `unit_rand`, `unit_unit`, `unit_dir`, `route_rand`, `stance_hold`; header specifies its separation from the sim LCG.
- `core/src/ai/route.rs` 958 — bot objectives, graph legs, same-storey hub/waypoint/post selection and route advancement · `plan_leg`, `mover`, `advance_route`, `post_point`, `aim_last_leg`, `unit_goal_hub`. Rules are in the module/method docs; regressions are in `core/src/ai/route/tests.rs`.
- `core/src/ai/route/hubs.rs` 341 — the hub a unit routes from: waypoints, drawn points, floors and levels · `hub_waypoint`, `hub_point`, `start_hub`, `level_hub`, `post_hub`; `first_leg` / `floor_walked` serve SPNC's census
- `core/src/ai/route/retarget.rs` 380 — choose a post, approach point or hunt goal and resolve its hub on the correct floor · `retarget`, `retarget_to_post`, `retarget_board`, `retarget_hunt`
- `core/src/ai/route/tests.rs` 1191 — AI route, same-storey/arrival, post/hub waypoint and barrier/corner-detour regressions, including deck spawn, per-unit hub points and progress-window continuation (DAGS/HUBP/MOUTH).
- `core/src/ai/avoid.rs` 1247 — foot-bot obstacle avoidance and parked-vehicle footprints (OURS, PROVISIONAL) · `pick`, `clear`, `footprint`, `foot_ahead`, `at_foot_of`; side commitment, leave/corner/rim rules and traversal regressions live here; see `analysis/bot-design.md` 4.7. Probe ownership is `steer.rs`.
- `core/src/ai/steer.rs` 1366 — AI wall/hull, barrier, floor/steep-band/ramp and opening-door probes; includes walking regressions · `rotate_xz`, `way_ahead`, `Stop`, `StopKind`; `way_ahead` is diagnostic only. Probe rules are specified in the header/source docs; heading selection belongs to `avoid.rs`.
- `core/src/ai/tactic.rs` 1435 — card TPOS: the derived position/cover table and the Tactician’s fighter position selection, hold/cycle and reservations · `PositionTable`, `Position`, `Source`, `build`, `fight`, `grant`, `keep`; see the module header and `analysis/bot-design.md` 4.1 / 4.6 for generation, filtering and timing rules. Diagnostic ownership stays here: `covered_from_target` (B12), `NavTick::tpos_ev` / `tpos_funnel` and the sweep’s `fightsplit` / `tactician` / `tfunnel` lines.
- `core/src/ai/tactic/cover.rs` 96 — directional cover masks and indexed position queries · `Position` and `PositionTable` methods, `sector_bit`, `sector_pair`
- `core/src/ai/tactic/tests.rs` 1501 — position-table/Tactician regressions: generation/reachability, candidate scoring, reservations/spacing, cover cycle, arrivals/deadlines and friend/grenade safety (TPOS/TPOSR); cases and constants live in the tests.
- `core/src/ai/team.rs` 612 — derived think-window unit grid, follow spacing and team Cover/Flank roles · `UnitGrid`, `Sim::units_within`, `follow_ease`, `board`, `flank_cap`; shared perception stays in `perception::tell`. Grid lifetime/order, role rules and constants are specified in the header and `analysis/bot-design.md` 4.1/4.3/6.2; derived grid is not hashed.
- `core/src/ai/team/tests.rs` 471 — unit-grid equivalence/lifetime, follow spacing and Cover/Flank role regressions, including diamond-route behavior.
- `core/src/ai/behave.rs` 77 — foot-bot behavior facade, shared `Tick`, and the `think` / `lost_jump` exports; see `analysis/bot-design.md` 3.
- `core/src/ai/behave/brain.rs` 1012 — foot-bot tick priority chain, next-think call delivery and explicit human boarding over retained commands, valid boarding maintenance, guard goal, failed-walk recovery, hub arrival and the health droid’s final approach; hands one aim request to the gunner and one move request to the mover, writing no sticks.
- `core/src/ai/behave/position.rs` 614 — fight and hold positions, hit reactions, grenade escape and lost-bot jump probes.
- `core/src/ai/behave/position/tests.rs` 1266 — the 25 behavior regressions spanning tick dispatch, fight positions, reactions, grenades and return to the level.
- `core/src/ai/abilities.rs` 183 — pure officer support, remote-droid and auto-turret (the BF2 cluster rule) selection through secondary fire and switch inputs · `input` (re-exported as `ai::ability_input` in `ai.rs`).
- `core/src/ai/arsenal.rs` 960 — foot-bot weapon/grenade choice, vehicle/crowd preferences, throw claims/ballistics and sniper sidearm selection · `think`, `think_droid`, `choose`, `score`, `against_vehicle`, `against_droid`, `droid_suited`, `is_crowd`, `vehicle_bonus`, `grenade_forced`, `armed`, `Armed`, `lob`, `lob_aim`, `sidearm`; scoring has PROVISIONAL branches. Rules and provenance are in the header, `analysis/re-grenades.md` 7 and `analysis/re-soldier-ai.md` 22.
- `core/src/ai/arsenal/tests.rs` 1720 — AI weapon-choice/grenade solver, window/group/claim/safety and vehicle/crowd combat regressions, including full sim scenes (GRAI/GRAIA/HVYW).
- `core/src/ai/snipe.rs` 363 — purposeful sniper Overwatch selection, reservation, travel and hold upkeep (OURS, PROVISIONAL) · `is_sniper`, `pick`, `lane`, `upkeep`, `holds`, `arrive`, `walk_failed`; see module header and `analysis/bot-design.md` 4.3. Close-range sidearm choice belongs to `arsenal::sidearm`.
- `core/src/ai/think_memo.rs` 136 — what a tick's thinks already asked a frozen world: lane rays and floor walks, open only during the thinks · `during_thinks`, `lane`, `walk`
- `core/src/ai/fix.rs` 434 — a cutter carrier's broken-turret objective (OURS, card TBLD): the 50 m pick ahead of post / spot / ring, one claim a turret, give-up and release, the walk and the serve pad (stand, turn, cutter up, trigger) · `FixState`, `cutter`, `upkeep`, `walk`, `serve`.
- `core/src/ai/snipe/tests.rs` 416 — sniper lane/spot reservation, quiet-lane expiry, perch reachability and close-range sidearm regressions (SNPR).
- `core/src/ai/gunner_foot.rs` 1336 — foot-bot gunner, sole writer of look sticks and FIRE: aim/error tracking, bursts/charge/tap, weapon requests and friend line/blast safety · `run`, `AimRequest` (`Droid`: an enemy remote droid's lead point), `GunReport`, `yaw_toward`, `seen_aim`, `error_dir` (the aim error turns every tick, ours), `track_step` (the PS2's D9 tracker law), `fire_discipline`, `charge`, `track_error`, `radius`, `stance_factor`, `range_table`; BF1 shooter-error tracking is PROVISIONAL; BF2 uses the target error block. Detailed rules and regressions live here; see `analysis/bot-design.md` 3/4.8. BF2 target error state belongs to `unit::AimBlock`.
- `core/src/ai/gunner_foot/tests.rs` 1627 — foot-gunner aiming, burst, tap, hold, blast-clearance and dispersion regressions; reference assertions remain with their fixtures · `ai::gunner_foot::tests`
- `core/src/ai/gunner_foot/tests/droideka.rs` 53 — class-rate pitch control for droidekas with ordinary-soldier controls.
- `core/src/ai/gunner_foot/tests/radius_rows.rs` 89 — 80 rows of PS2 RAM (BF2 Kamino, units at rest: size term, stance word, error, visibility, cap, radius) that `gunner_foot::radius` must reproduce (card CLSE2)
- `core/src/ai/mover.rs` 1442 — foot-bot mover, sole writer of move sticks, ROLL_STANCE and JUMP: destination/arrival, stance, movement execution and progress/outcome books · `run`, `MoveRequest`, `Moved`, `stuck_window`; see module header and `analysis/bot-design.md` 3/4.7. Heading comes from `avoid::pick`; last-resort recovery is `stall`'s. Arrival/floor and blocked-walk regressions live here.
- `core/src/ai/ride.rs` 883 — bot vehicle boarding claims/travel, delayed BF1 call delivery over retained Hold/Follow commands (WGORD), explicit unarmed-seat provenance (WGPAS/PILEIN), source-budget concurrent BF1 claims after think write-back (WGMUL), seated dispatch, first routed departure, subsequent reached-post command-walker departures (WGNXT), initial pickup, BF1 release expiry, BF2 human precedence and maintained-claim waits, low-health all-AI bailout and dismount decisions · `claim`, `deliver_call`, `claim_seat`, `board_goal`, `board_point`, `command`, `drive`, `set_out`, `at_drop`; `Sim::ai_use_pass` executes requests. Driver laws belong to `hover_drive`/`walker_drive`, building-turret decisions to `man`; includes boarding/driving/gunner regressions.
- `core/src/ai/ride/tests/mod.rs` 894 — existing mod regressions extracted from the facade; logical test module paths and selectors unchanged
- `core/src/ai/ride/tests/order_scene.rs` 79 — the order scene `get_out_tests` and `pile_in_tests` share.
- `core/src/ai/ride/tests/pile_in_tests.rs` 568 — actual PILE IN pad edges, capacity/delayed-think/eligibility/lifecycle/order/hash regressions and field/idle/follower controls.
- `core/src/ai/ride/tests/get_out_tests.rs` 648 — actual EVERYONE OUT pad edges, scheduled exits, own-search deadline and explicit reentry, membership, retained orders, lifecycle and hash regressions.
- `core/src/ai/ride/tests/call_board_tests.rs` 1249 — GBOARD natural BF1 call-to-next-think claim/entry/departure scenes, ten call/autonomous controls, identity/admission/order/lifecycle/arbitration/clock/hash referees; WGORD issuer-death cleanup and explicit player-slot generation reuse.
- `core/src/ai/ride/tests/passenger_call_tests.rs` 893 — WGPAS 48 and WGMUL 32 actual-step called-seat scenes; exact source budget/priority/freshness/generation, simultaneous think/write-back, AI/player occupancy, capacity shrink, departure in both issuer orders, no-call policy, provenance/hash/cooldown/lifecycle and long pickup controls.
- `core/src/ai/ride/tests/seat_call_tests.rs` 341 — WGSEAT actual-step 64-scene capacity/armament/source matrix, independent start/departure principals and next-think unavailable-seat admission in both unit orders.
- `core/src/ai/ride/tests/post_setout_tests.rs` 655 — WGNXT natural 48-scene post-stop/capture/next-route matrix, repeated success/failure, delayed recipient write-back/orders, seat/range controls and lifecycle/hash referees.
- `core/src/ai/ride/tests/setout_call_tests.rs` 484 — WGOUT actual-step natural/public phases, independent departure rolls, late recipients in either index order, route/turn/tenure controls and unaffected traces.
- `core/src/ai/ride/tests/wait_call_tests.rs` 462 — WGCAL actual wait-start calls: natural/public entry phases, delayed/failed rolls, both index orders, 3-D issue membership, next-think boarding/departure, lifecycle and unchanged trace controls.
- `core/src/ai/ride/tests/claim_wait_tests.rs` 619 — CWAIT natural long-approach/departure scenes, freshness/give-up/identity and slot-reuse continuations, progress/aim/bailout controls and permanent-completion referee.
- `core/src/ai/ride/tests/order_wait_tests.rs` 681 — WPOKE actual button-edge repairs and controls, mounted reach, scheduled consumption, maintained/expired claims, generation/tenure resets and hash continuations.
- `core/src/ai/ride/tests/human_wait_tests.rs` 488 — HWAIT natural boarding, human eligibility/geometry, deadline, slot-generation, completion and tenure regressions; direct predicate boundaries and route/RNG/event/bailout controls.
- `core/src/ai/man.rs` 570 — purposeful bot building-turret offers/claims with autonomous-search cooldown and obsolete ground-maintenance cleanup, threat/defense tests and exit rules (OURS, D11) · `offer_pass`, `purpose`, `hittable`, `defends`, `front_line`, `known`, `arc_holds`, `arc_of`, `exit_rule`, `endgame`, `seated`, `is_turret`, `seat_gun`; see module header and `analysis/bot-design.md` 6.1.
- `core/src/ai/man/tests.rs` 1512 — turret-purpose/offer, capacity, exit/cooldown and unreached-claim regressions; includes recoilless gunner aim, arc, own-building LOS, downward fire and charge/lifecycle controls (TURR/TAIM/STCHG).
- `core/src/ai/hover_drive.rs` 527 — bot hover stick law and path-driving wrapper · `sticks`, `Class`, `Body`, `Order`, `look_ahead`, `stuck`, `probe_ray`, `drive`; `RideState::hover` carries raw sticks. Equations, obstacle/unstick rules and provenance are in the module header and `analysis/re-hover-ai.md` `[O]`.
- `core/src/ai/hover_drive/tests.rs` 310 — hover-AI stick law against 347 PS2 RAM rows, with bike, obstacle probe, unstick, path look-ahead/stuck and segment-retention regressions; boarding/dismount scenes belong to `ride`.
- `core/src/ai/hover_drive/tests/ps2_rows.rs` 381 — the 347 rows (numbers read from PS2 RAM, card HAIS's ps2_rows.csv) · `ROWS`, `BRANCHES`
- `core/src/ai/walker_drive.rs` 456 — bot walker stick law and path-driving wrapper · `sticks`, `Class`, `Body`, `drive`, `ARRIVE`, `COMMAND_ENDGAME`; `RideState::sticks` carries raw sticks. Turn/strafe/arrival and command-walker/tauntaun rules are in source docs and `analysis/re-walker-ai.md` `[O]`.
- `core/src/ai/walker_drive/tests.rs` 1270 — walker-AI law against 145 PS2 RAM rows and flat-ground AT-ST/BF1/spider convergence, post travel, command-walker stay-aboard and natural empty/full pickup regressions (WAI/WALKB1/WAIT4).
- `core/src/ai/walker_drive/tests/ps2_rows.rs` 155 — the 145 rows (numbers read from PS2 RAM, the RE session's ps2_rows.csv) · `ROWS`
- `core/src/ai/gunner.rs` 297 — seated-bot aim/fire requests, shared foot-gunner charge release/fire gates and barrel-cone checks · `aim`, `Look`; `Sim::seat_gun` executes them. Rules and PROVISIONAL AI-gunner limitations are in the module header; building-turret aim state belongs to `ManState::aim`.
- `core/src/ai/arcm.rs` 221 — failed-link memory, team arc exclusions and back-and-forth detection · `ArcmState`, `Osc`, `ArcFails`; OURS, `analysis/bot-design.md` §9 D20.
- `core/src/ai/dfocus.rs` 270 — Card DFOCUS (D25, OURS): focus fire on HUMAN players at Hard and Elite. The holders of a human's hit tokens (`super::tokn`) are his focusers: at
- `core/src/ai/proxy.rs` 455 — fixed human proxy profiles and shared tier targeting, health-droid and post choices · `Profile`, `Strength`, `front`, `mend`, `pick`, `TARGETS`.
- `core/src/ai/tokn.rs` 265 — Card TOKN (D24, OURS): damage pacing against HUMAN players - hit tokens. Each human has a budget of tokens per difficulty ([`budget`]); a bot's
- `core/src/ai/unseen.rs` 170 — Card UNSN (D23, OURS): a bot the stall watch gives up is moved on, not killed, when no human can see it happen. The recycle of card RCYC
- `core/src/ai/wake.rs` 135 — early bot think on hits, near misses and newly seen enemies, with wake cooldown · `HIT`, `NEAR`, `SEEN`; OURS, `analysis/bot-design.md` §9 D21.
- `core/src/ai/why.rs` 430 — Card WHY: a reason code on every behaviour change and on every failed walk of a bot (`analysis/bot-design.md` 4.4, 4.5: "every walk has an
- `core/src/ai/wake/tests.rs` 156 — Card HYST: the early think's tests.
- `core/src/ai/unseen/tests.rs` 179 — tests for `core/src/ai/unseen`
- `core/src/ai/tokn/tests.rs` 254 — tests for `core/src/ai/tokn`
- `core/src/ai/proxy/hand.rs` 136 — proxy-only player aim and held-trigger model · `Hand`, `aim`, `tolerance`, `holds`, `tap`.
- `core/src/ai/proxy/tests.rs` 657 — profile and goal tests, including health droid service and disconnected or stale route controls
- `core/src/ai/dfocus/tests.rs` 371 — tests for `core/src/ai/dfocus`
- `core/src/ai/arcm/tests.rs` 290 — Card ARCM: the back-and-forth detector, the bot's closed arcs, the team's count and the route draw round them.
- `core/src/commander.rs` 1026 — persistent per-bot Conquest post goals and defend-hint allocation · `TeamPlan`, `plan`, `draw_post`, `defend_cap`, `post_threatened`; per-game redraw rules and port defender limits are specified in the header and `analysis/re-conquest-goal.md`.
- `core/src/commander/tests.rs` 1311 — the commander's tests.
- `core/src/perception.rs` 852 — contact memory, view/LOS, target acquisition and teammate contact sharing · `scan` (enemy remote droids are candidates too: `Foe`, `alive`, `foe_motion`, `droid_view_distance`), `tell`, `choose_target`, `choose_target_with`, `los_seat`, `past_buildings`; turret muzzle/own-building/arc behavior is specified with the helpers (TURR). Fairness requires a held perception contact; see module header.
- `core/src/perception/tests.rs` 883 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `core/src/skill.rs` 1031 — the four difficulty tiers (Easy, Normal, Hard, Elite; the maintainer, 2026-10-28), skill vector, tier-dependent grenade/reaction choices and the per-team human-surplus rule (card DTEAM) · `Difficulty`, `Skill`, `Tier`, `Tier::react`, `TRACKER_SHARE`, `Skill::tracker_skill`, `team_skill`, `TeamSkill`; provisional tier values and provenance are specified in the module header.
- `core/src/squad.rs` 1304 — BF1 squad orders, pending-call cancellation on actual player orders, tenure-bound mounted release delivery, pending acknowledgments and follow/hold/release execution, generation-bound ground-pilot PILE IN/EVERYONE OUT, own-search suppression and follower vehicle goals · `Order`, `Pending`, `BoardIssuer`, `Command`, `steer`; rules and replay regressions live here and in `analysis/re-squad-orders.md` (SQACK).

## game — the binary and its library

- `game/src/dlc.rs` 120 — optional Republic and CIS heroes the Hero Options row can put on every BF2 map, labels and animation descriptors.
- `game/src/dlc/tests.rs` 660 — content-gated hero lifecycle, every-map, effect layer, tentacle and absent-addon hash checks.
- `game/src/vehicle_class/flyer.rs` 198 — flyer class defaults and property routing, including turret first refusal, destructible-chunk fallback and the path follower's numbers · FlyerClass.
- `game/src/main/sim_trace.rs` 190 — SHASH headless CLI, input validation and fixture or baked mission setup · `run`.
- `game/src/main.rs` 649 — binary CLI and mode dispatch; flags and environment variables are specified in its module header · `main`, `ViewerArgs`, `BattleSetup`. The binary loads its parts with `#[path]` because the library owns `game/src/`.
- `game/src/main/battle.rs` 1132 — battle construction and shared draw/HUD setup, including the installed-turret bounds/readout fixture · `make_walk`, `walk_draws`, `hud_model`.
- `game/src/main/battle/native_spawn_tests.rs` 315 — owned-content Dune Sea launch, native post lifecycle and deterministic friendly/two-army controls through `make_walk`.
- `game/src/main/headless.rs` 1276 — offscreen bench, frame/orbit capture, walk/ride replay, scene reports and the asset-free scripted aim referee · `run_bench`, `run_headless`, `run_aim_replay`, `tick_event_feed` (drains unused HUD output each replay tick).
- `game/src/main/headless/detailed_bench.rs` 507 — opt-in raw CPU tick and full replay-render frame measurements (thread CPU, instructions, allocations, memory), cold GPU objects, periodic sim hashes and captures · `run`.
- `game/src/main/windowed.rs` 1443 — windowed shell/loading/battle state machine, the Instant Action playlist played row by row, per-tick event/audio and black-box hooks, pause/cheats and profile autosave, including newly earned class kills · `App`, `run_windowed`, `run_shell`, `play_event_sounds`, `start_take`.
- `game/src/main/windowed/tests.rs` 270 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `game/src/main/windowed/events.rs` 1034 — native window lifecycle and application event dispatch · `ApplicationHandler for App`; headless automation never starts this path
- `game/src/main/windowed/career.rs` 14 — finished/abandoned battle profile save and career summary logging, using the event feed's battle totals · `record_round`.
- `game/src/main/windowed/front.rs` 1061 — shell front screens, movie/music (takes in turn), menu clicks, the BF2 Instant Action screen, Instant Action, settings and binding capture · `FrontShell`, `App::redraw_shell`; `ui::shell_ps2` owns the screen flow.
- `game/src/lib.rs` 80 — module roots
- `game/src/world_fingerprint.rs` 101 — baked world compatibility identity for a future multiplayer handshake.
- `game/src/assets.rs` 1370 — baked-map loading into a CPU scene, texture tiers, collision/hull body extraction and mesh-reader re-exports · `load_scene`, `Scene`, `TextureTier`, `mode_layer_index`, `body_layers`, `hull_parts`.
- `game/src/assets/tests.rs` 391 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `game/src/assets/visual_clips.rs` 168 — Loads optional meshless clip companions for rendering while preserving the simulation body.
- `game/src/assets/mesh.rs` 527 — static/skinned GLB mesh readers, material and contact-surface extras, and streaming fixtures · `load_glb`, `load_glb_joints`, `load_skinned_glb`, `CpuSkinnedMesh`, `SkinnedAsset`.
- `game/src/assets/render_ground.rs` 125 — validate an explicit corrected presentation grid and anchor foliage without changing the simulation heightfield · `Scene::render_heightfield`.
- `game/src/main/windowed/menu_actions.rs` 69 — settings autosave for pause-menu actions · `persist_setting`.
- `game/src/main/windowed/mouse.rs` 296 — native menu PNG cursor, focus/screen capture policy and mouse glue for `App` · `sync_grab`, `release_mouse`, `mouse_window_event`.
- `game/src/kbm.rs` 529 — keyboard + mouse → `PadState`: the one bindings table, held keys and latched taps, mouse look in `LOOK_UNIT`s, merging with the controller, pointer lock (Locked, else Confined + hidden) and menu keys · `Kbm`, `Bindings`, `Trigger`, `Bind`, `menu_key`, `menu_bits`, `RAD_PER_COUNT`.
- `game/src/kbm/tests.rs` 479 — every pad bit bound, controller pass-through, WASD, taps, sensitivity/invert, stick takeover and a 144 Hz frame-to-tick replay.
- `game/src/viewer.rs` 62 — viewer facade and stable re-exports; its module header owns the XFRM transform convention and routes to the parts below.
- `game/src/viewer/walker_aim.rs` 172 — Walker drawn aimer transforms.
- `game/src/viewer/look.rs` 931 — manifest-derived water, fog, cull/LOWR, far-fade, lighting and scoped sky-layer tuning · `water_look`, `speckle_look`, `water_shading`, `far_cull`, `lowres_range`, `apply_lighting`.
- `game/src/viewer/look/tests.rs` 661 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `game/src/viewer/scene.rs` 1123 — scene GPU upload and draw ranges for terrain, water, foliage, models/LOWD and sky; moving door/turret/station records, texture cache and untextured census · `upload_scene`, `SceneGpu`, `SpeckleAnim`, `sky_wrap_v`, `pose_doors`; posing lives in `turret_pose` / `stations`.
- `game/src/viewer/scene/world.rs` 803 — world scene upload and its draw/source census · `upload_scene`, re-exported at its existing path
- `game/src/viewer/scene/terrain.rs` 169 — terrain draw chunks, layer uploads and chunk bounds; the scene’s terrain upload helpers.
- `game/src/viewer/scene/tests.rs` 457 — scene draw-range regressions for water replacement and the ranges that follow it.
- `game/src/viewer/foliage.rs` 223 — terrain foliage model placement, tint and fade · `records`, `models`, `matrix`; module header and `analysis/re-foliage.md` §6.
- `game/src/viewer/grass.rs` 452 — grass sprite templates and patch placement · `template`, `patch_matrix`, `build`; module header and `analysis/re-foliage.md` §6–§7; per-view quads belong to `render::grass`.
- `game/src/viewer/leaf.rs` 538 — leaf sprite templates and attached/world/FOLG object placement · `template`, `objects`, `object_matrix`, `build`; module header and `analysis/re-leafpatch.md` §4–§5; per-view quads belong to `render::leaf`.
- `game/src/viewer/upload.rs` 350 — GPU mesh/texture uploads and procedural primitives · `upload_submeshes`, `upload_skinned`, `upload_texture_file`, `upload_texture_file_scrolling`.
- `game/src/viewer/pickups.rs` 154 — field pickup models and visibility · `PickupVisuals`, `item_drawn`; submitted through `VehicleVisuals::draws`; module header and `analysis/re-powerups.md` §4.
- `game/src/viewer/vehicles.rs` 992 — vehicle model/texture lookup and draws, including CPU-skinned walkers through `walker_pose` · `VehicleVisuals`, `VehicleSides`, `find_model`, `find_texture`; `ClassVis` (`draws`), `upload_parts` and `pose_class` are shared with `droidekas.rs`.
- `game/src/viewer/stations.rs` 485 — service-droid scene node poses driven by simulation motion · `StationAnimState`, `StationNodes`, `pose`, `StationDraw`, `SceneGpu::pose_stations`; `analysis/re-powerup-station.md` §6/§14.
- `game/src/viewer/turret_fall.rs` 61 — a fallen building turret's view: its intact draws left out and its wreck / pad drawn while the sim has it down · `FallenDraw`, `SceneGpu::fallen`; card TBLD.
- `game/src/viewer/turret_pose.rs` 606 — turret/recoil and rider node poses, scene turret records and zoomed-gunner visibility · `pose`, `part_matrices`, `PoseSpec`, `TurretDraw`, `rigged_turrets`, `GunnerHang`, `nine_pose`, `SceneGpu::hidden_for`, `view_draws`, `pose_turrets`, `pose_turrets_at`; module header and `analysis/re-turret-draw.md` 2–4.
- `game/src/viewer/walker_pose.rs` 547 — walker skeletal posing, CPU skinning, posed rider-node queries and stabilized gunner camera mount frames · `WalkerRig`, `node_now`, `skin`; module header and `analysis/re-walker.md` §5, §6, §10.
- `game/src/viewer/droidekas.rs` 510 — the droideka's draw: CPU-skinned model posed by state (the roll clip driven by the spin, the lean about the ball centre) and the shield bubble's additive shell with its grow/shrink · `DroidekaVisuals`, `walker_state`, `model_matrix`, `roll_time`, `bubble_scale`, `bubble_tris`, `bubble_color`.
- `game/src/viewer/droidekas/tests.rs` 258 — Droideka visual state, bubble and palette controls.
- `game/src/viewer/camera.rs` 199 — golden/overview camera poses and XFRM placement · `GOLDEN_CAM`, `instance_matrix`, `overview_bounds`, `look_at`; transform convention in `game/src/viewer.rs`.
- `game/src/walk.rs` 1422 — player integration with `core::Sim`, respawn tracking, per-tick class kill attribution and shared battle accounting into the profile, chase/seat cameras with eased stance framing, reticle and zoom projection · `WalkSim`, `flush_class_kills`, `record_battle`, `vehicle_camera`, `turret_scope_camera`, `ps2_reticle_y`; seat camera remains PROVISIONAL (VSEAT); module header and `analysis/re-zoom.md` §5.
- `game/src/walk/camera.rs` 556 — the chase, eye, seat and turret cameras, view pull, field of view and see-through rules · `WalkSim::camera_track`, `seat_camera`, `settle_camera`, `camera_see_through`
- `game/src/walker_shots.rs` 135 — Walker shot ordinals, authored barrel frames and tracer styles.
- `game/src/walk/walker_view.rs` 148 — Walker camera and seat view state.
- `game/src/walk/tests.rs` 1091 — walk integration, camera easing, zoom, aiming, spawning and backdrop regressions extracted from the walk implementation.
- `game/src/map_boundary.rs` 92 — which baked paths form each map's boundary, a table read off both discs' `bnd_` chunks · `shape_names`, `boundary`.
- `game/src/layout.rs` 963 — manifest command posts, spawn paths and navigation graph · `MapLayout`, `nav_graph`.
- `game/src/mission.rs` 1168 — mission RON loading and army/team selection, planning/barrier calls, sound/music/voice and camera-shot data · `MissionDef`, `MusicCalls`, `Pick`, `pick`; generated by `scripts/gen_missions.py`; module header describes field-consumption limits.
- `game/src/native_teams.rs` 184 — native conquest class setup and authored ally spawn paths · `install`.
- `game/src/round_end.rs` 284 — the end of a round (issue #33): the camera shot, the victory / defeat banner's time, then BF2's stats pages (issue #50) fed this frame's presses, Square's Done and the idle quit · `RoundEnd`, `Phase`, `RoundEnd::wants_stats`; `analysis/re-round-end.md` §2-§3.
- `game/src/round_stats.rs` 573 — every soldier's round for the stats pages (points, kills, posts, sprees, team kills, vehicles, class / weapon time, who killed whom) read from the sim's events by the feed, and the pages built from it (awards, Personal Stats lines, the profile's Career Stats, the medals' table) · `RoundStats`, `UnitRound`, `Names`, `screens`, `MEDALS`, `medal_level`; module header and statsref README.
- `game/src/roster.rs` 1439 — class data into faction/unit/weapon rosters, soldier constants with the jet data (`soldier_constants`, BF1 borrowing BF2 jet keys), held-slot registration, BF2 SBD gameplay with native map audio on both map sets and weapon visual specifications · `FactionRoster`, `held_slots`, `ClassRoster::register`.
- `game/src/roster/weapon_db.rs` 539 — weapons read from the class db: HUD key, charge stages, ordnance, thrown and sticky weapons, donors · `weapon_from_db`, `thrown_weapon_from_db`, `weapon_hud`
- `game/src/roster/looks.rs` 177 — muzzle flash, laser, impact and explosion looks read from the class db · `flash_look`, `laser_look`, `impact_look`, `explosion_look`, `explosion_class`
- `game/src/roster/items.rs` 285 — pickups, stations, drop tables and item pools read from the class db · `item_class_from_db`, `station_class_from_db`, `add_stations`, `item_pool`
- `game/src/roster/tests.rs` 230 — the roster's tests.
- `game/src/droideka.rs` 237 — the droideka class from its ODF (play-db gameplay keys, native shield sounds, camera sections, ExplosionName) and its baked record (clip frames and root travel, ordnance prims, gun hardpoints) for both teams of a battle, and the shield hum's loops · `droideka_class`, `camera_sections`, `install`, `bakes`, `apply`, `as_walker`, `model_point`, `shield_gain`, `shield_loops`.
- `game/src/droideka/tests.rs` 219 — Baked droideka class and rig loading checks.
- `game/src/stations.rs` 251 — service-droid model spheres, rotating collision setup, wreck collision and render models from map/common bakes · `install_collision`, `wreck_models`.
- `game/src/turret_fall.rs` 127 — a building turret class's fallen look (DestroyedGeometryName, the unbuilt pad, hidden; the common gun turret's pad deviation), its collision parts and the fallen models per placed instance · `fall_look`, `collision_parts`, `fallen_models`; card TBLD.
- `game/src/shell_ia.rs` 127 — the Instant Action screen's content from the bake: both games' names, baked maps, mission pairs; the profile tier ↔ Session Options' AI level · `instant_action`, `ia_level`, `port_difficulty`.
- `game/src/shell_movie.rs` 428 — shell movie frame-pack playback and background JPEG decoding · `ShellMovie`, `load_index`, `shell_dir`, `decode`; module header and `analysis/re-bf2-shell.md` §4.
- `game/src/classdb.rs` 677 — entity-class RON loading, BASE inheritance, case-insensitive resolution and ordered weapon-slot inheritance and an optional canonical SBD weapon database · `ClassDb`, `resolve`, `weapon_slots`, `WeaponSlot`.
- `game/src/class_picker.rs` 282 — the dev class picker's classes per side · `ClassPicker`, `Choice`
- `game/src/class_picker/playable.rs` 116 — the class names the picker offers, per game · `BF1`, `BF2`
- `game/src/class_picker/tests.rs` 222 — the picker's lists and availability.
- `game/src/vehicle_class/walker.rs` 907 — walker class parsing, section/default conversions and BF1-body adaptation · `WalkerClass`, `WALKER_BASES`, `turret_claims`, `bf1_body`; census tests here; module header and `analysis/re-walker.md` §11, §13.9.
- `game/src/vehicle_class.rs` 1491 — vehicle/building-turret class properties, seat/gun/rider rules, hull-body selection and flyer rest height · `HoverClass`, `seats`, `SeatSpec`, `GunnerDraw`, `PilotPose`, `health_type`, `ai_size`, `neutral_when_empty`, `damage_effects`, `explosion_name`, `HoverClass::hull`, `hull_names`, `TURRET_BASES`, `is_turret`, `FLYER_BASES`, `landed_height`; module header, `analysis/re-hover.md` §13.2 and `analysis/re-vehicle-rest.md` §2.
- `game/src/vehicle_class/aim.rs` 44 — Authored vehicle aimer metadata and limits.
- `game/src/vehicle_pads.rs` 1500 — manifest vehicle pads/building turrets into sim vehicle, gun, hull and seat setup; extra mission side-file routing · `build`, `add_turrets`, `install`, `mission_side_files`, `seat_setup`, `seat_gun`, `hardpoints`, `hull_parts`, `hover_hull`, `walker_bakes`, `building_sphere`, `PadBuild`, `TurretDef`, `class_key_team`; module header and `analysis/re-vehicles.md` §7–§8.
- `game/src/vehicle_pads/flight.rs` 85 — authored BF2 path Properties into route setup, including retail type_EntityPath names, PS2 numeric readers, class limits and enabling-object bindings · `flight_paths`.

- `game/src/conquest_defaults.rs` 205 — map/era combatants and calibrated ticket defaults; percentage resolution and pause population changes.
- `game/src/collision_layers.rs` 486 — which bodies are in a collision layer · `layer_bodies`
- `game/src/capture_feed.rs` 398 — command-post capture icon selection and meter presentation, including optional own-post/direction rules · `icon`, `for_unit`, `meter_sign`, `PostState`, `Rules`; module header and `analysis/re-capture-display.md` §3.1.
- `game/src/cheats.rs` 227 — the session's cheats (card CHEAT2): flags copied into each battle, the pause menu's Morse words (PS2 `sos` `ammo` `hud` `slow` `80s` `at` `alias`, ours `clip` `run` `all` `cheat`), all off back at the main menu (`[ours]`) · `Session`; `analysis/re-cheat-codes.md` Port.
- `game/src/camera_collide.rs` 901 — chase-camera pull-in, prop fading and visible baked contact surfaces (PROVISIONAL) · `CameraBlockers`
- `game/src/camera_collide/tests.rs` 626 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `game/src/alloc_count.rs` 64 — heap allocation counter behind the `alloc-count` feature, for bench builds · `so_far`.
- `game/src/ambience.rs` 327 — fixed world sound-emitter discovery and distances · `Ambience`, `Emitter`, `Kind`, `emitters`, `with_distances`; module header and `analysis/re-ambience-music.md` 1–2; referee `game/tests/suite/ambience_ps2.rs`, RAM data `game/tests/data/ambience_ps2.txt`.
- `game/src/animator.rs` 1153 — visual soldier clip selection/layering and blending · `Animator`, `AnimInput`, `ClassAnim`, `RollClock`, `body`, `travel`, `find_clip`; module header and `analysis/re-soldier-anim-layers.md` §3, §7 / `analysis/re-roll-picture.md` §7.
- `game/src/animator/rig.rs` 634 — the animation rig: clip sets, body and action slots, blend times, and the clip player · `Rig`, `Player`, `Ctx`
- `game/src/animator/hit.rs` 183 — cosmetic directional hit pose, event consumption, clock and bone masks · `Direction`, `Animator::note_hit`.
- `game/src/animator/aim.rs` 518 — soldier alert state and head/torso aim posing · `Aim`, `AimStep`, `AimKind`, `global`; module header and `analysis/re-soldier-anim-layers.md` §3.4, §7.
- `game/src/animator/census.rs` 327 — headless animator diagnostics for clip layers, alert/aim and roll-root travel · `Census`; `ANIM_CENSUS` on `round_to_outcome`; module header describes the referee measurements.
- `game/src/animator/tests.rs` 1610 — the animator on a synthetic rig whose clips say which one a bone was sampled from
- `game/src/animator/tests/hit.rs` 282 — hit direction probabilities, fade, stance masks, action continuity and unchanged sim hash.
- `game/src/animator/tests/death.rs` 60 — directional death clip selection and missing-clip fallback.
- `game/src/animator/jump.rs` 111 — BF1's timed jump poses, landing selection and interruption, and fade back to locomotion · `Jump`; `analysis/re-soldier-anim-layers.md` §10.
- `game/src/animator/turn.rs` 131 — root twist, roll travel and stationary foot turns, with bounded waist rotation for mouse aiming; `analysis/re-soldier-anim-layers.md` §11.
- `game/src/animator/tests/legs.rs` 123 — firing and rapid mouse-turn regressions, including controller behavior and animation clocks.
- `game/src/animator/tests/jet.rs` 112 — Card JETP2: the jet stances play `jetpack_hover` on the baked jet classes of both games, a shot plays over it, and the soldier lands back
- `game/src/skin.rs` 445 — skeleton, pose and looping frame sampling → joint palette · `Skeleton`, `Pose`, `Clip`
- `game/src/soldier_vis.rs` 780 — shared soldier/weapon asset types, draw plans, rider/seat helpers and muzzle geometry; preserves the `soldier_vis` API · `SoldierVisuals`, `DrawPlan`, `WeaponAsset`, `FlashModel`, `weapon_in_hand`, `seat_now`, `rider_now`, `fire_point`.
- `game/src/soldier_vis/runtime.rs` 1394 — `SoldierVisuals` asset loading, animation updates, spawn-screen stills and body/weapon/flash draw submission · `for_walk`, `load`, `load_cpu`, `update_with`, `still_draws`, `step_hover_nines`.
- `game/src/soldier_vis/runtime/animation.rs` 311 — advance soldier animation from the current sim · `SoldierVisuals::update`, `update_with` and foot-step animation
- `game/src/soldier_vis/water.rs` 31 — posed left/right foot positions in model and world space for water contact: `WaterFeet`, `SoldierVisuals::water_feet`.
- `game/src/soldier_vis/first_person.rs` 459 — the player's first-person arms and weapon: loading per class, the clip choice and playback each frame, the draw and the muzzle on the 1st-person gun · `FirstPersonVisuals`, `first_person_draws`; `analysis/re-first-person-arms.md` §5-§6.
- `game/src/soldier_vis/lod.rs` 584 — per-camera PS2 soldier budgets/density, ten-body far queue, reusable selection scratch and low-companion validation; presentation only.
- `game/src/soldier_vis/grenade.rs` 168 — grenade strength variants, a class picked from the other disc fitted to the map's throw (`fit_throw_to_map`), and pack drop-action selection, including instant-release dispensers.
- `game/src/soldier_vis/grenade/tests.rs` 742 — real grenade and eight-faction pack drop-animation regressions, standing and moving, and the throw each map fields, the pause menu's picked class included.
- `game/src/soldier_vis/tests.rs` 1238 — soldier and weapon poses, movement, stance transitions and animation regressions; retains `soldier_vis::tests` paths.
- `game/src/squad_vis.rs` 237 — the squad-order marker, a dev-mode overlay (off in normal play) · `squad_draws`, `markers_on`
- `game/src/event_feed.rs` 1291 — per-viewer HUD event/statistic messages, battle-long shot/hit/capture totals, every soldier's round (`round_stats`), localized names and spawn texts, the AI's PS2 names per team (`bot_names`), field/driver squad-order state, reticle target readout and hit-direction edges · `EventFeed`, `battle_stats`, `round`, `unit_name`, `Texts`, `points`, `order_arm`, `ambush_edge`, `POST_LINES`; module header and `analysis/re-event-messages.md`; post lines are the port's rule.
- `game/src/event_feed/tests.rs` 1092 — the event feed's tests.
- `game/src/bot_names.rs` 162 — BF2's AI names: the side's text-table lists and the PS2's per-team LCG (B2 `FUN_0036d880` / `FUN_0036f5f8`), seeded by the side · `BotNames`, `side_number`; module header.
- `game/src/event_sounds.rs` 1342 — once-per-tick event sounds, weapon-derived diagnostic names (contact property/position from Impact), unit cries and station ambience asks; headless `SOUND_LOG=<csv>` records starts and world emitters · `each`, `cries`, `hurts`, `station_asks`, `fire_sound_starts`, `listener`, `SoundLog`, `sound_names`; module/API docs and `analysis/re-weapon-sounds.md` §7.7 own trigger/cry rules; EVTK regression here covers sounds/effects/remnants at multiple frame rates.
- `game/src/event_sounds/tests.rs` 635 — device-free actual-sim impact audio caller regressions: fired weapon and exact contact position, seat gun, rocket splash, grenade contact/fuse/blast, terrain/world/hull, coincident and silent-contact controls (HITSND); caller reads preserve sim hash/events.
- `game/src/first_person.rs` 349 — first-person arm rows, clip choice, playback and placement from an eye pose · `Chooser`, `Playback`, `bf1_row`, `bf2_row`, `clip_name`, `run_rate`, `model_matrix`, `frame_scale`, `FpWeapon`; `analysis/re-first-person-arms.md` §2-§5, §8.
- `game/src/first_person/tests.rs` 335 — rows, clip choice (idle/run/shoot/reload/change/jump/fall/roll/prone/spawn), run rate, blend, placement and PS2 framing regressions.
- `game/src/viewpoint.rs` 190 — viewpoint preference and toggle state machine · `Viewpoint`, `ViewToggle`; `analysis/re-first-person.md` §3.
- `game/src/foley/droideka.rs` 242 — three native leg-bone contact sounds, cached animation-height curves and per-unit contact history; BF2 0.48 m downward crossing, no skeleton work in the tick.
- `game/src/foley.rs` 758 — soldier footsteps/stance sounds, surface selection and shared battle sound-sublevel ordering · `Foley`, `Table`, `Surface`, `clip_bf1`, `clip_bf2`, `View`, `Who`, `WHO`, `sublevels`, `battle_levels`, `foreign_armies`; prop-floor surface lookup remains PROVISIONAL-missing; module header and `analysis/re-foley.md`.
- `game/src/engine_sound.rs` 646 — presentation-only vehicle engine-loop graph evaluation and per-vehicle tick state · `graph`, `factors`, `Instance`, `EngineSounds`, `EngineClass`, `Voice`; module header and `analysis/re-vehicle-sounds.md`.
- `game/src/repair_feedback.rs` 40 — active cutter presentation and sound-loop requests shared by audio and effects; respects the active weapon channel and available energy.
- `game/src/repair_feedback/tests.rs` 352 — both discs' cutter channels, authored firing clips, muzzle particles, sound requests and feedback termination.
- `game/src/engine_sound/tests.rs` 722 — vehicle engine-loop regressions using inline AAT/AT-ST records and data-gated PS2 live-multiplier replay against the NAB1 bake.
- `game/src/dev.rs` 40 — dev mode, default-on while the game is unfinished, including packed builds (`OPENBF_DEV=0` disables it) · `on`
- `game/src/unlocks.rs` 397 — the eight unlockable classes (BF2's officer + special per side, BF1's four specials mapped to them; ours, cards UNLK1-UNLK3): one career counter each from `Career::class_kills`, BF1's lock at the BF2 cost, the cost earned down by career kills (25 half, 100 none), the slot's costs into the sim (all 0 under the `all` cheat, CHEAT2), the Career Stats rows · `UNLOCKABLES`, `Unlockable`, `of_class`, `bf1_cost`, `effective_cost`, `class_cost`, `set_player_gate`, `apply_costs`, `rows`; module header.
- `game/src/unit_probe.rs` 529 — shared read-only unit diagnostic row for sweep `UNIT_SAMPLES` and black-box snapshots · `Probe`, `locus_of`, `Doing`, `LEGEND`; field/API docs own steering-stop, hold/boarding, obstacle-commitment and droid-target semantics.
- `game/src/blackbox.rs` 2429 — dev-mode playtest history and rotating snapshots on F9, quit, panic and stuck alarm; read-only on the sim and excluded from its hash · `BlackBox`, `Snapshot`, `Meta`, `on`, `write_rotating`, `read`, `SoundRow`, `sound_digest`. `blackbox::SCHEMA` owns schema age and supported older reads; module header owns history/build diagnostics; operation in `analysis/dev-tools.md`.
- `game/src/blackbox/rotating.rs` 173 — serialized snapshot file rotation shared by the hotkey writer and panic hook, with same-thread recursion protection and filesystem regression tests · `write`, `PREVIOUS`.
- `game/src/blackbox/session.rs` 323 — Card DPRX: the black box's MATCH-LONG tally of one subject - the human, or in a headless round the sweep's human proxy (`core::ai::proxy`). The rings
- `game/src/fx.rs` 1281 — particle-effect lookup with disc-specific rules for imported SBD trails, lifecycle and draw submission, event-driven impact/explosion hooks, gain effects that follow their unit (one live per unit and kind) and PROVISIONAL fallback quads · `Fx`, `Library`, `LiveEffect`; module header and `analysis/re-fx.md` §5–§7; muzzle flashes come from `soldier_vis`.
- `game/src/fx/sky.rs` 128 — dome-attached decorative effects, authored hardpoints and rotation.
- `game/src/fx/storm.rs` 139 — BF2 Kamino lightning groups, skybolt placement and temporary dome, fog and sun lighting, including the ocean. See `analysis/re-weather.md` §§7–8 and `analysis/re-kamino.md` §2.
- `game/src/fx/water_contact.rs` 385 — soldier water-contact clocks, class-authored entry/fall splashes and posed-foot wakes, using the existing ripple and floating-effect pools; issue #115, `analysis/re-water.md` section 9.6.
- `game/src/fx/ingest.rs` 334 — a tick's sim events into live effects · `Fx::ingest_sim`
- `game/src/fx/abilities.rs` 66 — officer activation effects and timed recipient auras · `follow_buffs`.
- `game/src/fx/repair.rs` 72 — repair-pulse muzzle effects, owner following and initial-burst placement at the drawn weapon tip.
- `game/src/fx/children.rs` 128 — EMITTER particles replay their child templates along native path nodes (0.3 s samples, 0.1 s merge, secant velocities), with the parent's colour replacing the child's and native child offsets · `path`, `frame_at`, `draw`; `analysis/re-fx.md` §13; referees `game/src/fx/tests/resupply.rs` (live PCSX2 node captures) and `game/tests/suite/resupply_feedback.rs`.
- `game/src/fx/emitter.rs` 1003 — particle emitter spawning, burst/lifetime state and Transformer evaluation · `Instance`, `Particle`, `Frame`, `Lcg`; module header and `analysis/re-fx.md` §5.
- `game/src/fx/tests/resupply.rs` 195 — resupply gain effects: both games' red/blue child emitters drawn from the bake, BF1 child bursts spaced along the parent path, path-node replay against live BF1 captures, and the gain that follows its unit once per kind.
- `game/src/fx/draw.rs` 451 — effect particle draw shapes, view fades and density thinning · `view_terms`, `emitter_draws`; module header and `analysis/re-fx.md` §6.
- `game/src/bolt_vis.rs` 874 — laser bolt ribbons, fire-point placement, fired-class impact and air-expiry remnants, surface/event correlation, fades and BF2 light output · `BoltVis`, `bolt_draw`; module header and `analysis/re-bolt.md` §3–§4, §5.2.
- `game/src/bolt_vis/tests.rs` 1060 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `game/src/bolt_light.rs` 227 — BF2 terrain omni-light gathering and patch selection · `gather`, `OmniLight`; module header and `analysis/re-bolt.md` §5.3.
- `game/src/point_light.rs` 364 — local-light source preparation, colour packing, model bounds and frame submission · `still_lights`, `submit`, `command_post`, `model_sphere`, `Relation`; module header and `analysis/re-point-lights.md`; gather/shading belongs to `render::point_light`.
- `game/src/holo.rs` 1129 — command-post hologram state, beam/icon/axial-flare draws and authored scan-line scroll in viewer relation colours · `Holos`, `Holo`, `HoloArt`, `holo_draws`, `still_draws`, `switched_on`; module header and `analysis/re-point-lights.md` §3, §10, §15–16.
- `game/src/water_anim.rs` 1204 — moving water: FFT ocean, foam and flat-water bob · `Ocean`, `Foam`, `WaterMotion`, `cells_in_range`, `cell_draws`; unwrapped f64 water time and CPU cell routing; module header owns whole-cell far-pass limits and `analysis/re-water.md` references.
- `game/src/weather.rs` 931 — precipitation, wind, rain-shadow cell selection and splash draws · `Weather`, `Precip`, `Wind`, `Shadow`, `View`, `cell_walk`; module header and `analysis/re-weather.md`.
- `game/src/sounds.rs` 1486 — runtime sound banks/stream lookup (the shell's with the CORE clicks: `load_shell`), map-specific SBD fire/equip/reload fallbacks, shared state, attenuation/bus/pan laws, engine/ambient loop voices, mixer handoff and instrumentation · `SoundLib`, `Refused`, `attenuation`, `Handed`, `AmbientHeard`, `AmbientTake`, `MusicTrack`, `load_for`, `for_battle`, `music`, `loop_voices`, `ambient_voices`, `ambient_heard`, `ambient_take`; playback methods live in the private child; black-box starts via `handed`; rules in module/API docs and `analysis/re-sound-distance.md` / `analysis/re-unit-chatter.md`; sublevel referees `game/tests/suite/sound_sublevels.rs`.
- `game/src/sounds/mixing.rs` 103 — distance attenuation, per-ear levels, mixer conversion and authored bus gains · `attenuation`, `side`, `ears_3d`, `mixer_pair`, `bus_table`
- `game/src/sounds/walker.rs` 25 — Walker weapon and movement sound routing.
- `game/src/sounds/playback.rs` 763 — private SoundLib prepared voices, shared loop playback for engine and repair sources, one-shots, cries, station playback and layer scheduling · `play_loop_voice`, `play_loop_set`, `play_repair_loops`, `play_shield_loops`, `voice_take`, `play_voice`, `ask_line`, `play`, `play_2d`, `cry`, `cries`, `play_cries`, `station_ambient`, `play_station_ambient`, `start`, `tick`.
- `game/src/sounds/tests.rs` 1339 — sound lookup/mixing/gate regressions, generated VOICEP banks and device-free playback preparation; explicit-output public callers and input-driven bank census are opt-in tests.
- `game/src/spawn_page.rs` 326 — spawn-screen text/class-panel content for the player's slot · `page`; module header and `analysis/re-spawn-flow.md` §2–§4.
- `game/src/spawn_preview.rs` 932 — spawn-screen side soldier and class line-up placement/animation · `shown`, `stands`, `place`, `place_at`, `lineup_at`, `ring_place`, `ring_at`, `Preview`; module header and `analysis/re-spawn-flow.md` §2, §4.1 own fitted perspective, tint, scale and ring constants.
- `game/src/spawn_backdrop.rs` 244 — mission camera-shot selection and level-start spawn-screen backdrop · `CameraShot`, `ShotPicker`, `Backdrop`; module header and `analysis/re-spawn-flow.md` §1.1.
- `game/src/music.rs` 544 — mission/bank music into `core::music`, playback changes and headless `MUSIC_LOG` · `BattleMusic`, `HeadlessLog`, `volume_option`; profile / `OPENBF_MUSIC_VOLUME` control; module header and `analysis/re-ambience-music.md` §3.
- `game/src/announce.rs` 891 — mission/post commander lines and unit combat/squad/low-health voice requests into `core::announcer`; BF2 unit voice through `crate::vo` · `Commander`, `CallAsk`, `battle_lines`, `calls`, `gate_calls`, `vo`, `vo_body`; playback in `game/src/main/windowed.rs`; module/API docs and `analysis/re-unit-chatter.md`.
- `game/src/vo.rs` 988 — BF2 `VOSound` entry parsing, context/ranking, speaker selection and event requests · `Vo`, `VoLines`, `Entry`, `Said`, `Who`, `Body`, `context`, `best`, `class_lines`, `EVENTS`, `ASKED`; module header and `analysis/re-unit-chatter.md` 3.
- `game/src/pause.rs` 290 — pause-menu action dispatch, Respawn confirmation state and the rows' share of the D-pad (Left/Right kept for the Morse cheats, CHEAT2) · `PauseMenu`, `apply_menu_action`, `rows_input`, `pause_sound`; `analysis/re-spawn-flow.md` §7, BUGX scene tests.
- `game/src/profile.rs` 555 — autosaved settings, validated custom controls and career stats, including backward-compatible per-class kills and recorded capture totals · `Profile`, `Career`, `BattleStats`
- `game/src/render_blend.rs` 1064 — presentation interpolation of modelled projectiles and the eased camera position and AI-driven walker gunner heading stabilization, with snap handling and bit-exact restoration before sim readers; seat looks and aimers blended; a controller's aim drawn the frame's share into the next tick's turn and a FIRE press stamped with it · `Blend`, `Blend::stamp`, `Pose`, `blend`, `blend_in_looking`, `fraction`, `lerp_angle`; `OPENBF_RENDER_BLEND=0` disables it; module header owns fields, thresholds and restore contract.
- `game/src/render_blend/walker.rs` 126 — Between-tick walker pose interpolation.
- `game/src/hud_feed.rs` 1106 — BF2 HUD art loading and per-frame `ui::hud_tree` / `ui::hud_draw` → `render::hud` wiring, including weapon model/icon fallbacks · `HudView`; module/API docs own BF1 row keys and material-alpha rules.
- `game/src/hud_feed/tests.rs` 131 — HUD font loading regressions and the single test root for nested Cutter and weapon-layer tests.
- `game/src/hud_feed/item_layer_tests.rs` 283 — weapon-row compositing in nested and flat HUD trees, mesh/bitmap order and headless real-content overlap · `weapon_pills_precede_both_rows_and_bitmap_fallback_in_nested_and_flat_layouts`, `weapon_pills_real_content_overlap`.
- `game/src/hud_feed/cutter_tests.rs` 132 — secondary Cutter pose, missing-model/pose and existing-placement regressions.
- `game/src/hit_flash.rs` 59 — BF2 local damage flash threshold, alpha, duration and fade · `Flash`.
- `game/src/input_icons.rs` 607 — the Button prompts setting, the device Auto follows and the shipped SVG icons (`game/assets/input_icons/README.md`: sources, licences) · `IconChoice`, `DeviceTracker`, `icons`, `rasterize`
- `game/src/minimap.rs` 1039 — map/class minimap configuration and per-frame battle/spawn-map feeds, including command walkers; mission-resolved still feeds and asset-free caller regressions for captures/goldens · `MapStatic`, `ClassIcon`, `bounds_of`, `still`, `spawn_still`; module/API docs and `analysis/re-hud-map.md` §8.
- `game/src/gallery.rs` 1414 — headless map/unit/walkabout sheets with floor placement and class-specific held weapons, including installed DLC classes · `map_sheet`, `vantage_camera`, `model_class`, `discover_units`; `soldier_spot` owns grounding and `step_water` pins water per eye for goldens.
- `game/src/gallery/unit_discovery_tests.rs` 127 — unit-gallery class/weapon/animation discovery regressions; broken installed DLC class data fails the gallery instead of selecting generic rifle poses.
- `game/src/bench.rs` 583 — `--bench` per adapter · `Report`; `--bench-toggle`: one feature off and on in alternating blocks · `Toggle`; `OPENBF_FRAMEAB`'s paired prices · `price`.
- `game/src/frame_profile.rs` 902 — windowed frame timing, GPU-object census, dev hitch record/black-box integration and `OPENBF_FRAME_PROFILE=<csv>` output · `FrameProfile`, `Record`, `Hitches`, `FrameRow`; module header owns switches, thresholds, warmup and CSV fields.
- `game/src/dyn_res.rs` 240 — the Low Power Resolution setting's controller: steps the world's size down when frames run long and probes back up · `DynRes`, `STEPS`.
- `game/src/matchlog.rs` 826 — The MATCH LOG (card DPRX) — DEV SCAFFOLDING beside the black box: one compact file per battle with everything the human did, for the WHOLE
- `render/src/lib.rs` 548 — renderer facade, shared state and public re-exports · `Renderer`, `RenderError`, `TIME_WRAP`, `Renderer::clear_scene` (drops a map's look on leaving a battle); public submodules: `hud`, `affine`; crate header owns sky rules and unbuilt features.
- `render/src/lighting.rs` 400 — renderer lighting/fog/fade/water data and CPU light-sum reference · `Lighting`, `DirLight`, `Fog`, `FarFade`, `WaterShading`, `light_sum`; shader-unit rules in the module/API docs.
- `render/src/stars.rs` 403 — the star fields of the maps that author them (issue #16): the PS2's own star generator and twinkle, drawn in the sky pass after the first dome shell · `analysis/re-stars.md`.
- `render/src/sky.rs` 530 — sky-shell placement and material modes, with their tests · `Renderer::set_sky`, `SkyShell`, `SkyMode`, `Placement`, `mirror_squash`; rules in the module docs and crate header.
- `render/src/fog_cloud.rs` 192 — Authored FogCloud tiled dust, source annular fade and heightfield placement; one-time load warning for a potentially oversized cloud, retaining the per-frame guard.
- `render/src/rand.rs` 10 — seeded random sampling for decorative sky and atmosphere.
- `render/src/sky/flight.rs` 177 — decorative sky-object flight state, source movement and spawn RNG; no simulation state.
- `render/src/draw.rs` 736 — static/skinned draw-list types, lighting paths, fades and LOD switching · `DrawCmd`, `DrawLayer`, `LightPath`, `Fade`, `LodSwitch`, `LodSide`, `SkinnedDrawCmd`; cull/dither rules in type docs; GPU referee: `render/tests/suite/lod_switch.rs`.
- `render/src/device.rs` 443 — GPU/device setup, upload and resize · `Renderer::new_windowed`, `Renderer::new_headless`, `HEADLESS_FORMAT`; adapter selection (`OPENBF_ADAPTER`) in the module header. The offscreen constructor checks nonzero extents and the device’s texture limit before target allocation. Constructors finish in `pipelines.rs`.
- `render/src/bindings.rs` 62 — layout entries and borrowed bind-group resource constructors · `uniform_layout`, `texture_layout`, `sampler_layout`, `texture_view`, `sampler`, `entire_buffer`
- `render/src/resources.rs` 45 — unmapped buffers and single-level D2 textures through GPU creation instrumentation · `buffer`, `texture_2d`
- `render/src/pipelines.rs` 970 — renderer pipeline/layout construction and initial GPU resources · `finish_init`, `PipelineDesc`; pass inventory in the module header.
- `render/src/frame.rs` 606 — frame submission, culling, instancing, pass/draw ordering and overlay · `Renderer::render_frame`, `FrameStats`, `OverlayFn`; full pass sequence in the module header.
- `render/src/frame/submit.rs` 1280 — frame validation, instance/palette uploads and ordered render-pass submission · `Renderer::render_frame`; frame helpers and public paths remain in the facade
- `render/src/frame/tests.rs` 362 — frame sort keys, pass ordering and interface light regressions.
- `render/src/frame_ab.rs` 136 — per-frame interleaved pricing states for the bench (`OPENBF_FRAMEAB`) · `FrameAb`.
- `render/src/world_scale.rs` 180 — the world drawn smaller than the window and upscaled, the HUD at full size · `Renderer::set_world_scale`; `world_scale.wgsl`.
- `render/src/hud.rs` 521 — window-pixel HUD rendering (clamped and repeating sheets) from `ui::hud_draw` · `HudPass`, `HudVertex`, `HudBatch`, `HudTexture`; display-value blending and mesh depth rules in the module header (`Renderer::display_view_format`).
- `render/src/grass.rs` 553 — grasspatch sprite buffers and draws (also shared by leaves) · `Renderer::set_grass`, `GrassClass`, `GrassPatch`, `GrassSprite`, `sprite_quad`; view geometry/fade in the module header; GS blend remains PROVISIONAL ([O]).
- `render/src/leaf.rs` 378 — leafpatch per-view sprite buffers and object draws over the grass pass · `Renderer::set_leaves`, `LeafClass`, `LeafObject`, `LeafSprite`; twist, wind and fades in the module header (`analysis/re-leafpatch.md` §6–§7); BF2 lighting remains PROVISIONAL.
- `render/src/light_beam.rs` 748 — command-post hologram lightbeam geometry and visibility · `Beam`, `vertices`, `view_factor`, `fan`, `crosses_near`; fan/alpha rules and near-plane caveat in the module header (CPHOL).
- `render/src/grass.wgsl` 111 — `vs_grass` / `fs_grass` (grass and leaves; BF2 leaves' static `Lit` on the vertex
  normal), appended to `shader.wgsl` as one module
- `render/src/bolt.rs` 345 — laser-bolt ribbon geometry and draws · `Renderer::set_bolts`, `BoltDraw`, `ribbon`; geometry, blend/depth rules and shared particle ordering in the module header (`analysis/re-bolt.md` §3, [O]).
- `render/src/particles.rs` 810 — per-view particle geometry and sorted runs shared with bolts (`ParticleShape::Shaded`: world triangles with their own vertex bytes, the water ripple) · `Renderer::set_particles`, `ParticleDraw`, `ParticleShape`; shape/blend rules in the module header (`analysis/re-fx.md` §6, [O]).
- `render/src/particle_blur.rs` 259 — scene reduction and BLUR particle composite with visible-character coverage.
- `render/src/particle_blur.wgsl` 30 — alpha-masked sampling of the reduced scene in display space.
- `render/src/shadow.rs` 365 — BF1 vehicle volumes: capped z-fail, one display-space multiply · `ShadowMesh`, `Renderer::set_shadow_triangles`; `analysis/re-vehicle-shadow.md`.
- `render/src/bolt.wgsl` 97 — `fs_bolt_core` / `fs_bolt_glow` (GS MODULATE, unfogged), appended after `grass.wgsl`
- `render/src/terrain_light.rs` 478 — BF2 terrain light projection pass · `Renderer::set_omniproj`, `Renderer::set_terrain_lights`, `TerrainLights`, `LitPatch`; gather contract, gamma addition and frame-copy sequencing in the module header (`analysis/re-bolt.md` §5.3, [O]).
- `render/src/terrain_light.wgsl` 94 — `vs_terrain_light` / `fs_terrain_light`, appended after `shader.wgsl`
- `render/src/affine.rs` 154 — screen-space vertex-colour blend constants and CPU shader twin, with UTA1 pen-floor referee · `guard`, `guard_w`, `layer_bit`, `AFFINE_BIT`; shader partner: `shader.wgsl::affine_w`; rules in the module header (`analysis/re-vertex-colour-interp.md` §3–§5).
- `render/src/point_light.rs` 291 — BF2 per-view local-light filing and per-model gather · `Renderer::set_point_lights`, `PointLight`, `LitSphere`, `SpotCone`; selection/upload rules in the module header; shader term: `shader.wgsl::point_sum`.
- `render/src/filter.rs` 349 — the DomeInfo frame postprocess · `FrameFilter`, `box_side`, `Renderer::set_frame_filter`; see the module header and `analysis/re-glow.md` §3 for its display-byte formula and separable GPU passes (BFLT / ISPF).
- `render/src/filter.wgsl` 85 — DomeInfo filter shader (separate module) · `vs_filter`, `fs_filter_rows`, `fs_filter`; paired-sample box implementation: `box_sum`; CPU owner: `filter.rs`.
- `render/src/specular.rs` 179 — specular lookup-table upload and tex-gen CPU reference · `Renderer::set_specular_table`, `SpecParams`, `spec_texcoord`; pass/blend rules and PROVISIONAL status in the module header (`analysis/re-specular-pass.md`, [O]).
- `render/src/capture.rs` 164 — the headless readback and the PNG encoders (with and without the alpha plane); the
  padding and encoder tests · `Renderer::capture_rgba`, `encode_png`, `encode_png_rgb`
- `render/src/shader.wgsl` 1272 — world/skinned shader: vertex lighting, alpha, fog, water/speckle, sky, specular and terrain · `shade_unfogged`, `gs_lit`, `point_sum`, `MASK_CUTOFF`; grass/bolt/terrain-light/filter have separate `.wgsl` files; HUD shader is in `hud.rs`.
- `render/src/made.rs` 192 — GPU-object creation wrappers and frame-recorder inventory · `Made`, `note`, `take`, `texture_bytes`; enable/cap and wrapper contract in the module header; consumer: `game::frame_profile` (HITCH).
- `render/src/mesh.rs` 459 — vertex formats, GPU meshes · `Vertex`, `MeshData`
- `render/src/texture.rs` 293 — RGBA8 → bind group; samplers · `TextureDesc`
- `render/src/camera.rs` 125 — the fly camera (Y-up, −Z forward) · `Camera`
- `render/src/frustum.rs` 175 — frustum culling, a box's distance range from the eye (card LODP) · `Frustum`, `distance_range`
- `render/src/gpu_timer.rs` 207 — GPU ms per section of the frame (card ISPD): `OPENBF_GPU_TIMING=1`, timestamp queries and an async readback ring · `GpuTimer`

## pak — the game's data, on disk or packed (card PACK)
Every runtime read of baked data goes through `pak` (`read`, `read_to_string`, `exists` / `is_file` / `is_dir`, a
sorted `read_dir`, a seekable `open` for the music); a direct `std::fs` read of `assets_baked/` or `missions/` in
`game` or `audio` is a bug the packed game shows as a missing asset. Files the player owns (profile, black box, logs)
stay on `std::fs`. `game`'s `main` mounts `assets_baked` and `missions`: the directory when it exists, else the
archive. The archive is the one inside the executable, else `openbf.pak` in its folder (`find_archive`). `game
--write-pack <out> [--pak <file>] [--split-at <bytes>] <name>=<dir>...` writes the packed copy (`scripts/pack.py` runs it
on a stripped copy): one file below about 3.9 GiB, an executable and `openbf.pak` from there on, on every OS; unfinished packed builds keep dev mode on by default (`OPENBF_DEV=0` disables it; `dev::on`).
- `pak/src/lib.rs` 867 — packed/disk runtime data facade · `Archive`, `write_pack`, `write_pack_at`, `find_archive`, `collect`, `mount`, `packed`, `read`, `read_dir`, `open`, `Reader`; archive layout, deduplication and mount policy in the module/API docs.
- `pak/examples/pack_load.rs` — writes and reads a pack as the game does (`scripts/tests/test_pack_limits.py`) and measures load cost: `bench <dir> <scratch> [rounds]` times reading every file raw, legacy Deflate and zstd.
- `pak/src/tests.rs` 486 — round trip, re-pack, a damaged footer, ranged reads, dedupe, the disk fallback

## ui, input, audio, net
- `ui/src/lib.rs` 47 — module roots; re-exports `egui`
- `ui/src/hud.rs` 1373 — HUD models/events, round-end banner geometry and egui compatibility drawing (the banner and stats use the HUD pass) · `HudModel`, `RoundEndModel`, `HudWeapon`, `HudTarget`, `HudMessage`, `HudStats`, `message_color`, `target_color`, `feed`, `draw_hud`, `draw_pad_warning`; event coverage in the module header.
- `ui/src/hud_tree.rs` 1406 — BF2 HUD tree state, events, fades/colours and message queue · `Hud`, `Elem`, `Fader`, `BarFlash`, `EventValue`, `MultiLine`, `Line`; interpretation scope in the module header (`analysis/re-hud.md`, `analysis/re-target-hud.md`, `analysis/re-event-messages.md`).
- `ui/src/hud_tree/fixture.rs` 194 — existing shared HUD test fixtures; test-only module and visibility unchanged
- `ui/src/input_icons.rs` 568 — the one lookup every drawn control prompt goes through · `Icons`, `IconSet`, `Prompt`, `PadButton`, `pad_sheet`, `pad_name`
- `ui/src/hud_tree/tests.rs` 432 — HUD-tree regressions for faders, messages, number colour, bar flashes and group tint · same private `hud_tree::tests` identities.
- `ui/src/hud_draw.rs` 1255 — HUD layout → window-pixel triangles, mesh parts scrolled by the HUD clock · `draw`, `HudFrame`, `HudArt`, `anchor_shift`, `MESH_UNIT`; coordinate/Hor+ rules in the module header; spawn pages/veil route through `spawn_draw.rs`.
- `ui/src/hud_capture.rs` 292 — command-post capture icon and direction mark · `CaptureIcon`, `Side`, `Mark`, `Part`, `picture`, `BOX`; split-colour layout and port additions in the module header (`analysis/re-capture-display.md` §3.3–4).
- `ui/src/hud_ambush.rs` 316 — hit-direction chevrons and per-edge timers · `Ambush`, `Edge`, `edge`, `visible_box`, `SHEET`; direction, fade and measured/Hor+ layout rules in the module header (`analysis/re-hud-attacker.md`).
- `ui/src/hud_map.rs` 773 — minimap and spawn-map model/layout · `MapModel`, `MapView`, `MapIcon`, `heading`, `view_radius`, `post_color`, `window_point`; view rules and unbuilt portions in the module header (`analysis/re-hud-map.md`); spawn page uses `spawn_draw.rs`.
- `ui/src/hud_squad.rs` 428 — squad-order cross model/layout · `SquadCross`, `Part`, `centre`, `label_anchor`, `CENTRE`; sheet/label/fade/Hor+ rules and taker-count addition in the module header (`analysis/re-squad-orders.md` §2b).
- `ui/src/population.rs` 211 — shared percentage rounding, asymmetric pair ceiling and resolved menu text.
- `ui/src/menu.rs` 894 — pause/settings and editable controls menus, session cheats, the PS2 Morse recogniser and its words (PS2 seven, ours `OUR_WORDS`) · `MenuModel`, `draw_controls`, `MorseCheats`, `CHEAT_WORDS`, `OUR_WORDS`
- `ui/src/menu/pointer_navigation_tests.rs` 110 — existing pointer_navigation_tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `ui/src/menu/morse_tests.rs` 97 — existing morse_tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `ui/src/menu/tests.rs` 450 — existing tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `ui/src/menu/class_picker_tests.rs` 67 — existing class_picker_tests regressions extracted from the facade; logical test module paths and selectors unchanged
- `ui/src/match_length.rs` 39 — generated mean Instant Action match length by team size and reinforcements (`scripts/calibrate_match_length.py`).
- `ui/src/shell.rs` 920 — BF2 PS2 Instant Action screen and Session Options model: the merged both-games map list, the five states and every press, playlist add/remove/All/128 cap, option ranges/steps/defaults, dimmed rows, the launch list · `InstantAction`, `Launch`, `IaCommand`; rules and `[ours]` calls in the module header (`analysis/re-bf2-menus.md` §1, §2, §6, §7).
- `ui/src/shell/catalog.rs` 332 — eras, both games' maps and the catalog built from them · `MapEntry`, `catalog`, `Era`, `BF1_MAPS`, `BF2_MAPS`
- `ui/src/shell/options.rs` 399 — the Session Options page: groups, rows, values · `SessionOptions`, `Opt`, `Group`, `pieces`
- `ui/src/shell/texts.rs` 399 — the screen's localized text · `IaTexts`, `OPTION_TEXTS`
- `ui/src/shell/keys.rs` 238 — one frame of the screen's input · `InstantAction::step`
- `ui/src/shell_ps2.rs` 1054 — BF2 shell title/front-menu flow, focus, its clicks, pulse and flashy entrance, and stack layout · `ShellMenus`, `Stack`, `StackRow`, `RowId`, `Page`, `ShellCommand`, `stack_layout`; disabled screens and measured geometry in the module header (`analysis/re-bf2-shell.md` §2–§4, §9).
- `ui/src/shell_feel.rs` 526 — how the shell's menus sound and move · `MenuSound`, `Cues` (select click 0.1 s limit, list move rule), `Pulse` (`BUTTON`, the only pulse; a list's cursor box stays solid), `flashy`, `ShellPlaylist`; numbers in the module docs (`analysis/re-bf2-menus.md` §4, §5).
- `ui/src/shell/tests.rs` 1062 — Instant Action model regressions: merged list order/keys, dimmed rows skipped, state presses, All Maps/Modes/Eras and the cap, option ranges, Session Options pages, the launch list · private `shell::tests`.
- `ui/src/shell_ia.rs` 1095 — the Instant Action screen and Session Options pages drawn in the HUD pass (backdrop, list windows, command stack, info box, option rows, help lines) · `Ctx::instant_page`, `LEFT_WINDOW`, `RIGHT_WINDOW`, `INFO_BOX`, `STACK_X`; script vs `[M]` geometry in the module header (`analysis/re-bf2-menus.md` §1.2, §2.2).
- `ui/src/spawn.rs` 524 — the spawn screen's egui panel (the battle without spawn screens, and the fallback with no HUD pass) · `SpawnModel` (post and class selections are the sim's, card SPSEL)
- `ui/src/stats.rs` 658 — the end of the round's stats pages as BF2's PS2 has them (issue #50): lists, sorting, the cursor, paging, the pad's buttons per page, the career box's unlocks page (UNLK3, ours) · `StatsScreens`, `StatRow`, `TeamList`, `Award`, `Career`, `CareerView`, `Medal`, `Pad`, `Command`, `sort_rows`, `clock`; module header.
- `ui/src/stats_draw.rs` 829 — Team / Personal / Career Stats drawn in the HUD pass over black, every place measured on the statsref PS2 frames · `Ctx::stats_page`; `[M]` geometry, colours and stretches in the constants' docs.
- `ui/src/spawn_draw.rs` 1070 — BF2 side/class/spawn-map screens in the HUD pass · `SpawnPage`, `window_point`; veil, button/help geometry and Hor+ layout in the module header (`analysis/re-spawn-flow.md` §2–§4, `analysis/re-bf2-shell.md` §3).
- `ui/src/loading.rs` 72 — the BF1 loading screen · `draw_loading`
- `ui/src/overlay.rs` 241 — egui ↔ winit ↔ wgpu glue; reapplies the game-owned native cursor after platform output · `EguiOverlay`
- `ui/src/cursor.rs` 149 — the shell's pointer activity gate: hover moves menu focus only on fresh motion or a click; native cursor image and policy live in `game/src/main/windowed/mouse.rs` · `Pointer`
- `ui/src/hud_scope.rs` 125 — The sniper scope's overlay — the PS2's `ScopeDisplay` HUD element (`analysis/re-zoom.md` §8 [V][O]; BF1 vtable 0x540880, update
- `input/src/lib.rs` 1238 — gilrs → deterministic PS2-layout `PadState` (keyboard + mouse merge in `game/src/kbm.rs`) · `Mapping`, `pad_from_gamepad_with_arms`, `stick_arms`, `TRIGGER_PRESS`; always initialize via `open_gilrs` (project SDL mappings/DB/env); diagnostics: `log_pads`, `pad_warning`, `input/examples/pads.rs`; mapping rules in the module/API docs.
- `audio/src/lib.rs` 324 — kira playback through the master limiter; a looping voice with settable gain, rate and pan; a one-shot that can be cut (card UCRY) · `Audio`, `Voice`, `Shot`
- `audio/src/limiter.rs` 170 — master lookahead limiter · `Limiter`; stereo linking, full-scale behavior and PROVISIONAL timing constants in the module header/API docs (MIXH).
- `audio/src/stream.rs` 519 — streamed SPU2 ADPCM music over `pak::open` and kira · `StreamTake`, `TakeReader`, `MusicVoice`, `Audio::play_stream`; stereo block layout in the module header; baked body paths: `manifest::music_body`.
- `net/src/lib.rs` 10 — empty until the LAN phase

## Tests, examples, goldens, data

- `game/tests/ui_sharp.rs` 536 — serial headless owned-source menu/HUD captures, option-value visibility and treated font labels and loaded-font pointer checks (#114).
- `game/tests/ui_sharp/cutter.rs` 195 — real-content Fusion Cutter model selection, missing-model fallback, and headless both-era captures (#114).

- `game/examples/flyer_drive.rs` 143 — scripted pad flight on a real map with PS2 class numbers; deterministic headless replay.
- `game/tests/flyer.rs` 399 — original PS2 class extraction/census, flight property/limit decoding, owned bake identity and seeded class-restricted Space Yavin flight rounds.
- `engine/ci.sh` runs workspace fmt, all-target clippy `-D warnings`, build, tests and doctests.
  It uses nextest's `ci` profile when installed, otherwise `cargo test`; both select
  `--lib --bins --tests --example arena`. Other examples are compile-checked by clippy
  without linking each as a test binary (WEARC). Run the script for the full CI gate.
  Unit tests sit in each module's
  `mod tests`; `<crate>/tests/` holds integration tests, which skip without `extracted/` or `engine/assets_baked/`.
  Each file directly under a crate's tests dir is its own binary linking the whole engine (~130 MB written per link), so
  `<crate>/tests/suite/main.rs` gathers game's, render's, bake's and mungefs' integration tests as modules of ONE binary each (card WEARY, SSD wear; a
  test id is `suite <file>::<name>`); a new test file goes there with a `mod` line. Beside game's stay the goldens and
  `walkabout_stats` (found by name by gates.py / goldens.sh) and `texture_tier` / `capture_display_ps2` (they set the
  process-wide texture tier).
- `game/tests/suite/bug_checks.rs` 578 — BUGX synthetic corpse targeting, station-service control, hit-chevron fade through respawn and death-region-under-water spawn flow; Suicide menu paths remain uncovered.
- `game/tests/suite/officer_abilities.rs` 237 — disc-based officer class census, secondary activation and deferred hero ability regressions.
- `game/tests/suite/pack_dispenser.rs` 176 — all twenty class variants and eight faction dispensers; real class data, self/friend/enemy touch collection, health/ammo and refill exclusion.
- `game/tests/suite/career_class_kills.rs` 482 — UNLK2 real Utapau officer/Wookiee kills and capture, respawn/class switching, whole-battle shot/hit totals, catch-up ticks, autosave/reload and victory accounting; synthetic same-tick dead-shooter credit and damage eligibility/tick deduplication.
- `game/tests/suite/cheat_ammo.rs` 156 — CHEAT2 on BF1's stormtrooper rifle off the disc: infinite ammo (reserve fixed, free reloads), no reloading (clip full, the reserve pays, 275 rounds in one burst), both (nothing drawn) and neither, per tick.
- `game/tests/suite/spawn_lineup_clear.rs` 250 — the class screen's real soldiers stay clear of its panel and the window's edge at each aspect (GPU), its outlines are not measured on opening, and a capture of the real page.
- `game/tests/suite/unlocks_bf1.rs` 179 — UNLK1/UNLK3 on real BF1 Kamino and Yavin: the jet trooper, dark trooper and Wookiee smuggler refuse the class accept at 11 points and spawn at 12; career kills (BF2's and BF1's jet trooper together) halve the cost at 25 and free it at 100.
- `game/tests/suite/turret_damage.rs` 720 — building turrets take the ordnance's damage (playtest 2026-10-28): BF2 Kamino's Minigun Turret with a bot gunner under the EMP launcher, rocket launcher and rifle (the drop by the building scales, direct + blast; destroyed at 0 with the gunner killed), BF2 Hoth's dish and BF1 Kamino / Naboo / Geonosis turrets, and a gunner's own rounds sparing his turret.
- GPU referees: `render/tests/suite/` (one binary, card WEARY), run only by `scripts/gpu_tests.sh` (then `scripts/goldens.sh --check`).
- Goldens: *engine/goldens/maps/*, *engine/goldens/units/* and the HUD / shell / spawn PNGs beside them; referees
  `game/tests/golden_gallery.rs` and the other `golden_*.rs`; drawn by `game/src/gallery.rs`.
  *engine/goldens/walkabout_stats.ron* is per-panel numbers, not a picture: `game/tests/walkabout_stats.rs` renders
  every walkabout at 1/4 and fails the named panel whose mean / σ / flat-grey share moved (`OPENBF_TEST_DROP_TEXTURE`
  in `game/src/viewer/scene.rs` is its test hook); run it by hand, `cargo test -p game --test walkabout_stats -- --ignored`.
- `game/examples/play_census.rs` tries every usable thing on every map with a player's input (spawns, posts, vehicles, pads, stations, doors, round ends, roaming traps, and a bot round's unused objects and non-shot deaths); its header lists the probes.
- Gate tools are examples: `game/examples/round_to_outcome.rs` (the sweep; `scripts/sweep_seeds.py` runs it over N seeds
  and tables finish rate, stuck_max spread and deaths by cause per map, card SWPN;
  no-progress follows foot travel and active ground pilots, excluding flyer/passenger seats and arrived parked walkers (#113); its `recycle_lv` lines, `FLOOR_GRID`
  and `NAV_FLOORS` show a recycle against the routing's own floors, the floors round a spot and every hub's resolved
  floor, card STLVL; its `endgame` line times a round from the tick a side stands clamped at 3 alive or fewer, and
  `TRACE_UNIT` names a rider's vehicle, card STEND; `BOT_SAMPLES` writes every unit at 4 Hz and the shots / hits /
  kills that `scripts/bot_quality.py` turns into bot-quality numbers beside the PS2's `scripts/bot_quality_ps2.json`,
  `sweep_seeds.py --botq`, card BOTM), `game/examples/arena.rs` (one tier per run, or `vs a:b` / `vs all`: a tier per team through `Sim::team_difficulty`, the higher tier's kill share and paired t over seeds, card ARVS),
  `game/examples/walkabout.rs`, `game/examples/water_frames.rs`, `game/examples/leak_census.rs` (the collision
  leak census, card LEAK: a soldier driven at every wall of every map, runs that cross a face or end enclosed /
  buried / off the map, counted by mechanism; its accepted list `leak_census_accepted.txt`), `game/examples/ordnance_census.rs`
  (the ordnance floor census, card GRFL: 4 grenades on every floor a soldier stands on, the ones that pass it keyed
  by the body crossed; its accepted list `ordnance_census_accepted.txt`), `game/examples/pose_render.rs`
  (the port at a PCSX2 pose, water stepped, no soldier or HUD — `scripts/parity.py`'s PS2 parity score and gate, card PARITY), `game/examples/fx_capture.rs`
  (one particle effect spawned on the ground and rendered over its life from a pose, card FXP2), `game/examples/blackbox_read.rs`
  (the digest of a black box snapshot: the header, the human's last 10 s, the focus unit, blocked bots, events; card BLKB —
  `round_to_outcome` writes one headless with `BLACKBOX_AT` / `BLACKBOX_ON`); census tools beside them (`hover_drive.rs`: one hover driven on a real map; `walker_drive.rs`: a human-driven walker's phases, the speed each settles at, the planted foot's slip and a replay, `--frames` its chase view;
  `turret_ride.rs`: one building turret entered, fired and left; `spawn_node_census.rs`: every post's spawn node on every map against the first leg a bot spawned there plans, before and after card DAGS's rule, against its accepted list `spawn_node_census_accepted.txt`, card SPNC) and in
  `mungefs/examples/`. Replays and hashes: `scripts/sim_gates.sh`.
- `engine/missions/` — 61 ground Conquest RONs from `scripts/gen_missions.py`, including 27 alternate-era missions.
  `engine/assets_baked/` (gitignored) comes from
  `scripts/rebake.sh`; `scripts/scratch_bake.sh` + `scripts/cmp_bake.sh` check it.

- `mungefs/tests/suite/support.rs` 34 — shared LVL/terrain fixture reads and LVL traversal; callers retain their gates, sorting and reference assertions.
- `game/examples/support/ordnance_grid.rs` 70 — bounded finite diagnostic grid parsing and probe limits for the ordnance census · `parse_cell`, `bounds`.
- `game/examples/support/pose_ride_spec.rs` 75 — asset-independent positional `POSE_RIDE` parsing · `RideSpec`, `parse`.
- `game/examples/support/replay_check.rs` 54 — vehicle/turret example replay hash comparison and process-status regression · `check`.

## Where do I change X?
| to change … | start in | also |
|---|---|---|
| weapon fire rate, spread, damage | `core/src/weapon.rs` (`WeaponClass::from_props`) | `bake/src/classes.rs`, `game/src/roster.rs` |
| the muzzle flash (model, size, colour, fade, where on the gun) | `core/src/flash.rs` (`MuzzleFlashLook`), `game/src/roster/looks.rs` (`flash_look`), `game/src/soldier_vis/runtime.rs` (`flash_draws`, `fire_points`) | `bake/src/soldier/output.rs` (`weapon_nodes`), `bake/src/alpha.rs` (`flash_textures`), `manifest/src/lib.rs` (`WeaponNodes`), `render/src/particles.rs` (`ParticleShape::Mesh`), `game/src/fx.rs` (`submit_with`) |
| how a laser bolt looks (ribbons, lengths, colours, the impact remnant) | `core/src/laser.rs` (`LaserLook`), `game/src/roster/looks.rs` (`laser_look`) | `game/src/bolt_vis.rs`, `render/src/bolt.rs`, `render/src/bolt.wgsl`, `render/src/pipelines.rs` |
| BF2's bolt lights on the terrain (the pink wash) | `game/src/bolt_vis.rs` (`omni_lights`), `game/src/bolt_light.rs` (`gather`) | `core/src/laser.rs` (`LaserLook::light`), `render/src/terrain_light.rs`, `render/src/terrain_light.wgsl`, `render/src/frame.rs`, `game/src/viewer/scene.rs` (`install_omniproj`), `bake/src/spec.rs` (`bake omniproj`) |
| soldier speed, stances, energy | `core/src/soldier.rs` (`SoldierConstants::from_props`) | `game/src/classdb.rs` |
| jet and jump packs (fuel, the leap, the hover, the Jedi double jump, the fly-height ceiling; card JETP) | `core/src/soldier/air.rs` (`Soldier::jet_on`, `step_jet`, `fly_ceiling`) | `game/src/roster.rs` (`soldier_constants`, `BF1_JET_BORROW`), `core/src/sim/step.rs` (`AirEnv`, the cheat), `game/src/mission.rs` (`max_player_fly_height`), `scripts/gen_missions.py`, `analysis/re-jetpack.md` §13 |
| a faction's classes | `game/src/roster.rs` | `game/src/mission.rs` |
| which class a bot or a respawning player takes | `core/src/sim/spawn/runtime.rs` (`ai_class`, `respawn_slot`) | `game/src/roster.rs` |
| a mission's teams, tickets, timing | `scripts/gen_missions.py` → `engine/missions/` | `game/src/mission.rs` |
| post capture, bleed, domination | `core/src/cp.rs` | `core/src/region.rs`, `core/src/sim/round.rs` (`update_cps`) |
| reinforcements, respawn | `core/src/battle.rs` | `core/src/sim/spawn.rs` (`SpawnWaves`), `core/src/sim/spawn/runtime.rs` (`respawn_slot`) |
| one tick's order, a new tick stage | `core/src/sim/step.rs` (`Sim::step`) | the header of `core/src/sim.rs` |
| a bolt's damage, a kill, an environmental death | `core/src/sim/combat.rs` (`resolve_bolt`, `kill_environment`) | `core/src/weapon/ordnance.rs` (`Health`) |
| a class's weapon slots, the two channels, the switch inputs, a thrown grenade (cycle, flight, bounce, stick, fuse, blast) | `game/src/classdb.rs` (`weapon_slots`), `core/src/sim/ordnance.rs`, `core/src/sticky.rs` | `game/src/roster.rs` (`held_slots`), `game/src/roster/weapon_db.rs` (`thrown_weapon_from_db`), `core/src/weapon.rs` (`Thrown`), `core/src/unit.rs` (`HeldWeapon`), `game/src/main/battle.rs` (the HUD's grenade row) |
| pickups: what a class drops, the item's numbers and box, the pool size; the drop, the item, the touch | `game/src/classdb.rs` (`drop_table`), `game/src/roster/items.rs` (`drop_entries`, `item_class_from_db`, `item_pool`), `core/src/sim/pickups.rs` | `core/src/sim/combat.rs` (`unit_killed`, `kill_environment`: the drop; `buff_scale`), `core/src/unit.rs` (`UnitSpec::drops`, `Unit::buffs`), `game/tests/suite/roster.rs` (`drop_tables_are_the_discs`), `game/src/viewer/pickups.rs` (the draw), `game/src/event_sounds.rs` (`PowerupSound`, the landing), `game/src/fx.rs` (the taker's gain effect) |
| health / ammo / vehicle droids (`powerupstation`): the class numbers, the placed stations, the pass | `game/src/roster/items.rs` (`station_class_from_db`, `add_stations`), `core/src/sim/stations.rs` | `game/src/vehicle_pads.rs` (`install` places them), `core/src/sim/pickups.rs` (`give_ammo`), `game/tests/suite/vehicle_pads.rs` (`the_showcase_maps_place_their_droids`), `game/src/event_sounds.rs` (`PowerupSound`, `station_asks`: the chatter), `game/src/sounds.rs` (`station_ambient`), `game/src/fx.rs` (the gain effect) |
| a droid's idle turn and serving moves (the FX-7's arms and socket, the gonk's rock and bob) | `core/src/station_motion.rs`, `game/src/viewer/stations.rs` | `bake/src/turret_rig.rs` (the per-node parts), `core/src/sim/stations.rs` (`StationAnim`, `Station::motion`, rotating collision), `game/src/roster/items.rs` (`station_anim`), `game/src/viewer/scene.rs` (the draws), `game/tests/suite/station_draw.rs` (BF1 Naboo's 15 droids end to end) |
| an explosion's area damage, a vehicle's death blast, a rocket's impact blast | `core/src/sim/blast.rs` (`Sim::explode`) | `core/src/sim/combat.rs` (`bolt_explodes`), `game/src/roster/looks.rs` (`impact_explosion`, `explosion_class`), `core/src/weapon/ordnance.rs` (`ExplosionClass`, `OrdnanceClass::explosion`, `blast_falloff`), `game/src/vehicle_pads.rs` (`blast_setup`), `game/src/vehicle_class.rs` (`blast_sphere`) |
| how a bot aims, moves, fires | `core/src/ai/behave/brain.rs` (`think`: what it is doing), `core/src/ai/gunner_foot.rs` (the look sticks and FIRE), `core/src/ai/arsenal.rs` (which weapon, and the grenade), `core/src/ai/mover.rs` (the move sticks, stance, roll, jump) | `core/src/perception.rs`, `core/src/skill.rs`, the constants in `core/src/ai.rs` |
| where a bot walks: its objective, its route leg | `core/src/ai/route/retarget.rs` (`retarget`), `core/src/ai/route.rs` (`plan_leg`; `leg_point`: the PS2's aim along a leg, from the last corner passed) | `core/src/commander.rs`, `core/src/nav.rs` |
| how a bot gets round a wall, a crate, a parked vehicle | `core/src/ai/avoid.rs` (`pick`: the side, the corner, the leave rule, the early turn; `foot_ahead`: a standing vehicle's footprint) | `core/src/ai/steer.rs` (the probes: `wall_ahead`, `floor_ahead`), `core/src/ai/mover.rs` (`stuck_window`: a bot going round is not stuck), `core/src/collision/walls/queries.rs` (`wall_sweep`) |
| which posts the AI attacks | `core/src/commander.rs` | `core/src/nav.rs` |
| AI routes, barriers, where a post or a unit is on the graph | `core/src/nav.rs` | `mungefs/src/plan.rs`, `game/src/layout.rs`, `game/tests/suite/locate_ps2.rs` (locate against 1,004 live PS2 path goals, `game/tests/data/locate_ps2.txt`) |
| blocked route groups (`plnp`) | `core/src/route_patch.rs` | `game/src/mission.rs` (`arc_blocks`), `scripts/gen_missions.py` |
| squad orders | `core/src/squad.rs` | `core/src/ai/command.rs`, `game/src/event_sounds.rs` (the player's line), `game/src/announce.rs` (the units' answers through the voice queue, `Commander::orders`), `game/src/squad_vis.rs` (the dev-mode marker), `ui/src/hud_squad.rs` (the D-pad cross in the HUD), `game/src/event_feed.rs` (the cross's state and its count of takers, `EventFeed::squad`), `game/tests/suite/hud_layout.rs` (the cross against three PS2 frames), `game/tests/suite/squad_orders_ps2.rs` (the PS2's scenes on the port) |
| squad vehicle map regressions | `game/tests/suite/squad_vehicle_maps.rs` | Loaded BF1/BF2 ground vehicle calls, retained Hold/Follow, full-seat refusal, pilot release orders and lifecycle cancellation; registered by `game/tests/suite/main.rs`. |
| aim assist | `core/src/aim_assist.rs` | `core/src/sim/input.rs` (`acquire_magnet`, `apply_assist`) |
| the zoom (R3): a weapon's steps, what cancels it, the zoomed turn rate, the shoulder and scope cameras, the zoomed FOV | `core/src/weapon.rs` (`zoom_press`), `core/src/sim/step.rs` (`tick_zoom`), `core/src/unit.rs` (`Zoom`, `camera_section`, `scope_point`) | `game/src/walk.rs` (`step_tick`), `game/src/walk/camera.rs` (`fov_y`) |
| first-person view, camera and arms (cards FPV, FPA): eye camera, FOV ease, toggle, FPM arms and 1st-person weapon | `core/src/unit.rs` (`first_person_view`, `eye_view`, `scope_path`), `core/src/sim/step.rs` (`first_person`) | `game/src/viewpoint.rs` (`Viewpoint`, `ViewToggle`), `game/src/walk/camera.rs` (`eye_camera`, `in_eye_view`, `shows_first_person_arms`), `game/src/first_person.rs` (`Chooser`, `Playback`), `game/src/soldier_vis/first_person.rs` (`FirstPersonVisuals`), `bake/src/fpm.rs`, `manifest/src/fpm.rs`, `analysis/re-first-person.md`, `analysis/re-first-person-arms.md` |
| where a human's shot goes, the chase camera's tilt, the reticle's height | `core/src/unit.rs` (`camera_section`, `ideal_camera`, `aim_ray`, `fire_point`), `core/src/sim/step.rs` (`aim_hit`, `human_shot`) | `game/src/walk/camera.rs` (`WalkSim::camera`), `game/src/walk/camera.rs` (`reticle_y`), `ui/src/hud.rs` (`HudModel::reticle_y`, `reticle_y`), `game/tests/suite/hud_layout.rs`, `analysis/re-aim-assist.md` §10 |
| how a soldier follows the ground (ramps, folds, fall by airtime; per game) | `core/src/soldier.rs` (`Soldier::step`, `resolve_ground`, `GroundRules`) | `core/src/sim/step.rs` (3a': contact after the wall pass), `core/src/sim/state.rs` (`ground_under_n`, `Sim::ground_rules`), `core/src/ai/steer.rs` (`steep_ahead`: too steep to climb = a wall for the AI) |
| walking on models, walls | `core/src/collision/walls/queries.rs` (the wall pass), `core/src/collision/rays.rs` (`ray_down`) | `core/src/sim/state.rs` (`ground_under`), `game/src/collision_layers.rs`, `game/examples/leak_census.rs` (the check) |
| BF2 doors (triggers, the open clip, the moving box, the drawn door, the sounds; a bot at a door that is opening) | `core/src/door.rs`, `game/src/assets.rs` (`Scene::collision_and_doors`), `game/src/viewer/scene.rs` (`SceneGpu::pose_doors`), `game/src/main/windowed.rs` (`play_event_sounds`) | `bake/src/door.rs`, `core/src/collision/world.rs` (`move_body`), `core/src/sim/doors.rs` (`door_opening`, `opening_leaf_over`), `core/src/ai/steer.rs` (`StopKind::Door`; no steep floor in an opening leaf's box) |
| what a bolt or a line of sight hits | `core/src/collision/rays.rs` (`CollisionWorld::ray`) | `core/src/projectile.rs`, `core/src/perception.rs` |
| the pad mapping | `input/src/lib.rs` (`Mapping`) | `core/src/sim/input.rs` (`PadState`, `buttons`) |
| keyboard + mouse (bindings, pointer lock, mouse look, the view between ticks) | `game/src/kbm.rs`, `core/src/sim/look.rs` | `core/src/sim/input.rs` (`PadState::mx`/`my`, `MOUSE_AIM`), `core/src/soldier.rs` (`mouse_turned`), `game/src/render_blend.rs` (`blend_in_looking`), `game/src/main/windowed/mouse.rs`, `analysis/plan/FEATURE_SET.md` (Platform & meta) |
| where the shot came from (the hit-direction chevrons) | `ui/src/hud_ambush.rs` (the rule, fade, draw) | `game/src/event_feed.rs` (`ambush_edge`: which hits, whose frame), `core/src/sim/combat.rs` / `blast.rs` / `vehicles.rs` (`Event::Hit` / `VehicleHit`'s `dir`), `ui/src/hud.rs` (`HudModel::ambush`), `game/tests/golden_hud.rs` (`hud_ambush.png`), `analysis/re-hud-attacker.md` |
| the event messages (who killed you, posts, orders) and the points + kills pop-up | `game/src/event_feed.rs` (the gate, names, colours, points), `ui/src/hud_tree.rs` (`MultiLine`) | `ui/src/hud.rs` (`HudMessage`, `HudStats`, `feed`), `ui/src/hud_draw.rs` (`multiline`), `bake/src/hud.rs` (`bake_locl`), `mungefs/src/locl.rs`, `manifest/src/hud.rs` (`HudLocl`), `game/src/walk.rs` (`post_keys`), `game/tests/suite/hud_layout.rs` (the PS2-frame checks), `analysis/re-event-messages.md` |
| the end of the round's stats pages (Team / Personal / Career Stats) and the AI's names | `ui/src/stats.rs` (model, buttons), `ui/src/stats_draw.rs` (layout), `game/src/round_stats.rs` (the numbers, awards, pages), `game/src/bot_names.rs` (names), `game/src/unlocks.rs` (the career unlocks' rows) | `game/src/round_end.rs` (`RoundEnd::stats`), `game/src/main/battle.rs` (`stats_pages`), `game/src/main/windowed.rs` (presses, Square), `game/src/event_feed.rs` (`round`, `unit_name`), `game/src/profile.rs` (`Career::points`), `analysis/re-round-end.md` §3.2 |
| the HUD (BF2's tree, drawn) | `ui/src/hud_tree.rs` (elements, events, fades), `ui/src/hud_draw.rs` (layout, anchors) | `ui/src/hud.rs` (`HudModel`, `feed`, the egui lines), `game/src/hud_feed.rs` (`HudView`: art + per-frame run), `render/src/hud.rs` (the pass), `game/src/main/battle.rs` (`hud_model`), `game/src/roster/weapon_db.rs` (`weapon_hud`), `bake/src/glb.rs` (`hud_model_to_glb`: HUD meshes keep COL0), `game/tests/suite/hud_layout.rs` (the PS2-frame check), `game/examples/hud_capture.rs`, `analysis/re-hud.md` |
| the minimap (where a post or unit sits, the backdrop, the wedge) | `ui/src/hud_map.rs` | `game/src/minimap.rs` (the feed: map square, icons, detection), `game/src/main/battle.rs` (`hud_model`, `make_walk`), `game/src/hud_feed.rs` (`map_point`), `game/tests/suite/hud_layout.rs` (the PS2-frame check), `analysis/re-hud-map.md` |
| control prompts' icons (the Button prompts setting, which button a prompt shows, Auto) | `ui/src/input_icons.rs`, `game/src/input_icons.rs` | `input/src/lib.rs` (`family_of`, `ActivePad::deliberate`), `game/src/hud_feed.rs` (`set_icons`, the paint fallback), `bake/src/xbox_icons.rs`, `mungefs/src/texture_xbox.rs`, `game/src/profile.rs` (`input_icons`), `game/tests/golden_shell.rs` (`help_lines_draw_every_icon_set`), `game/tests/prompt_capture.rs` (each set's screens as PNGs, to look at) |
| menus, shell, spawn screen | `ui/src/menu.rs`, `ui/src/shell.rs` + `ui/src/shell_ia.rs` (Instant Action), `ui/src/shell_feel.rs` (clicks, pulse, entrance), `ui/src/spawn.rs`, `ui/src/spawn_draw.rs` (the PS2 screens) | `game/src/main/windowed.rs`, `game/src/pause.rs` (`apply_menu_action`), `game/src/main/battle.rs` (`spawn_model`), `game/src/spawn_page.rs`, `game/src/spawn_backdrop.rs`, `game/src/spawn_preview.rs` (the screens' soldiers), `game/tests/golden_spawn.rs` |
| the sky dome darkened by the Lightning effect (Kamino, BF1 Yavin 2: shell 0 at 153/255) | `game/src/viewer/scene.rs` (`sky_shell_tint`), `manifest/src/environment.rs` (`ManifestLightning::dome_tint`) | `mungefs/src/fx.rs` (`LightningConfig`), `bake/src/map/content.rs` |
| the DomeInfo frame filter (the whole-frame lift of every BF1 map and BF2 TAT2) | `bake/src/lighting.rs` (`filter_from`), `render/src/filter.rs` | `mungefs/src/sky.rs` (`DomeInfo`), `manifest/src/environment.rs` (`ManifestFilter`), `game/src/viewer/look.rs` (`frame_filter_from`), `render/src/frame.rs` |
| fog, sun, ambient, sky | `bake/src/lighting.rs` | `game/src/viewer/look.rs`, `game/src/viewer/scene.rs` (sky shells), `render/src/shader.wgsl` |
| how vertex colour and the local lights blend across a triangle (the PS2's screen-space blend, its guard band, the off switch) | `render/src/affine.rs`, `render/src/shader.wgsl` (`affine_w`, `AFFINE_COLOUR`) | `render/src/frame.rs` (the instance's bit, the frame's guard) |
| how a world model is lit (COL0, prelit, unlit, Light2, `Static()`) | `bake/src/glb.rs` (`light_path`) | `render/src/draw.rs` (`LightPath`), `render/src/shader.wgsl` (`shade_unfogged`), `game/src/assets/mesh.rs` (`light_of`), `bake/src/lighting.rs` |
| BF2's local lights on models (lamps, EntityLights, the command post's light) | `game/src/point_light.rs`, `render/src/point_light.rs` (`gather`), `render/src/shader.wgsl` (`point_sum`, `gs_lit`) | `bake/src/lighting.rs` (`point_lights_from`), `bake/src/entity_light.rs`, `bake/src/glb.rs` (`model_sphere`), `manifest/src/environment.rs` (`ManifestPointLight`), `manifest/src/lib.rs` (`ManifestClassLight`), `mungefs/src/light.rs`, `game/src/viewer/scene.rs` (`DrawCmd::lit`), `game/src/assets.rs` (`CpuMesh::sphere`), `render/src/draw.rs` (`DrawCmd::lit`), `render/src/frame.rs` |
| BF1 vehicle shadow volumes (BF2 draws none) | `mungefs/src/shadow.rs`, `bake/src/shadow.rs`, `manifest/src/shadow.rs` | `game/src/viewer/vehicles.rs`, `render/src/shadow.rs`, `render/src/shadow.wgsl`, `render/src/device.rs` (`Depth24PlusStencil8`), `render/src/frame.rs`; authored sidecars, posed nodes, capped z-fail and one display-space ×0.625 pass |
| the command post's hologram (the beam, the icon, the relation colours, the fade across a capture, the flicker) | `game/src/holo.rs`, `render/src/light_beam.rs` | `render/src/particles.rs` (`ParticleShape::Beam`), `game/src/point_light.rs` (`submit`: the light × I), `game/src/viewer/scene.rs` (`SceneGpu::holo`), `core/src/cp.rs` (the meters) |
| the terrain's texture layers (which, their alphas, UVs, draw order) | `mungefs/src/terrain.rs` (`patch_alpha`), `bake/src/terrain_glb.rs` (`terrain_layers_to_glb`, `layer_uv`) | `game/src/viewer/scene.rs` (`upload_terrain_layer_chunks`), `render/src/draw.rs` (`DrawLayer::TerrainLayer`), `render/src/pipelines.rs`, `render/src/shader.wgsl` (`fs_terrain_layer`), `render/src/frame.rs` (`draw_key`) |
| how the terrain is lit (unlit, COLR × the per-game GS scale) | `bake/src/terrain_glb.rs` (`gs_vertex_byte`), `bake/src/map/content.rs` (`source_game`) | `render/src/draw.rs` (`LightPath::Terrain`), `render/src/shader.wgsl`, `game/src/viewer/scene.rs` |
| the terrain foliage (FOLG layers, `prp_`, path props, fade, grass patches) | `bake/src/foliage.rs`, `mungefs/src/foliage.rs` | `manifest/src/environment.rs` (`ManifestFoliage`, `foliage_bin`, `ManifestGrassClass`), `game/src/viewer/foliage.rs`, `game/src/viewer/grass.rs`, `render/src/grass.rs`, `game/src/viewer/scene.rs`, the leaf patches `bake/src/leafpatch.rs`, `game/src/viewer/leaf.rs`, `render/src/leaf.rs`, `render/src/draw.rs` (`Fade`), `render/src/shader.wgsl` (`fade_alpha`, `far_cull`), `core/src/heightfield.rs` (`normal_at`) |
| a particle effect's data (the `fx__` emitters, per layer) | `mungefs/src/particle.rs`, `bake/src/effects.rs` | `manifest/src/lib.rs` (`FxSet`), `bake/src/map/content.rs` (the effect sheets keep alpha), `scripts/rebake.sh` (side: `bake effects`), `mungefs/tests/suite/fx_lvl.rs` (census + PS2 RAM vector) |
| how a particle effect runs and draws (bursts, the Transformer, the shapes, blends, the sort) | `game/src/fx/emitter.rs`, `game/src/fx/draw.rs` | `game/src/fx.rs` (`Library`, `Fx::submit`), `render/src/particles.rs`, `render/src/bolt.wgsl` (the shared fragment shaders) |
| which effect a hit or a blast spawns (the struck surface, the ordnance's and explosion's names) | `core/src/impact.rs` (`ImpactLook`, `ExplosionLook`), `game/src/fx/ingest.rs` (`Fx::ingest_sim`), `game/src/fx.rs` (`Fx::explode`), `game/src/fx/trail.rs` (the trails) | `core/src/sim/combat.rs` (`push_impact`), `core/src/sim/state/events.rs` (`Event::Impact`), `game/src/roster/looks.rs` (`impact_look`, `explosion_look`), `game/src/vehicle_pads.rs` (`blast_setup`) |
| the water's look | `bake/src/water_glb.rs`, `mungefs/src/fx.rs` | `game/src/viewer/look.rs` (`water_look`, `speckle_look`), `game/src/viewer/scene.rs` (the draws, `SpeckleAnim`), `render/src/draw.rs` (`DrawLayer::Speckle`), `render/src/shader.wgsl` |
| the water's motion | `game/src/water_anim.rs` | `game/tests/suite/water_anim.rs` |
| weather: rain, snow, ash, embers, pollen (the particles, the cells drawn, rain shadows, wind, splashes) | `game/src/weather.rs` | `mungefs/src/fx.rs` (`PrecipitationConfig`, `WindConfig`), `bake/src/map/content.rs`, `manifest/src/environment.rs` (`ManifestPrecipitation`, `ManifestWind`; the `rainshadow…` regions), `render/src/particles.rs` (`ParticleShape::Streaks`), `game/src/main/windowed.rs` (`rain_splashes`, `tracked_velocity`), `game/src/gallery.rs` and `game/examples/pose_render.rs` (a still's weather), `game/tests/suite/weather_ps2.rs` (PS2 RAM vectors) |
| a side vehicle's model, LODs, chunks, collision | `bake/src/vehicle.rs` | `manifest/src/lib.rs` (`VehicleManifest`), `scripts/rebake.sh` (side), `game/src/viewer/vehicles.rs` (drawn in a battle) |
| a vehicle in the sim, its seats and riders | `core/src/vehicle.rs` | `core/src/sim/vehicles.rs`, `core/src/sim/seats.rs` (enter / drive / exit), `game/src/walk.rs` (`vehicle_camera`) |
| how a walker moves (clips, turn, legs) | `core/src/vehicle/walker.rs` | `game/src/vehicle_class/walker.rs`, `bake/src/walker.rs`, `core/src/sim/vehicles.rs` (`tick_walkers`), `game/examples/walker_drive.rs` |
| a building turret drawn turned to its aim (which node turns, the barrels' recoil) | `game/src/viewer/turret_pose.rs` | `bake/src/turret_rig.rs` (the per-node parts, the rig), `manifest/src/lib.rs` (`ManifestTurretRig`), `game/src/vehicle_class.rs` (`SeatSpec`: the node names, `PitchTurnFactor`, `BarrelRecoil`), `game/src/viewer/scene.rs` (the parts' draws), `game/tests/suite/turret_pose.rs` (the PS2's own rows, `game/tests/data/turret_pose_ps2.txt`; the drawn pose against the seat's gun), `game/examples/pose_render.rs` (`POSE_TURRET`) |
| a building turret's gunner drawn or hidden, where he hangs, his seat pose (card VTURG) | `game/src/soldier_vis.rs` (`seat_now`) | `game/src/vehicle_class.rs` (`SeatSpec::gunner_draw`, `PilotPose`), `game/src/viewer/turret_pose.rs` (`GunnerHang`, `nine_pose`), `game/src/animator.rs` (`Animator::seat`), `bake/src/soldier.rs` (`SEAT_CLIPS`), `game/tests/suite/turret_gunner.rs` (the census, the PS2's gunner rows `game/tests/data/turret_gunner_ps2.txt`, the draw end to end), `game/examples/pose_render.rs` (`POSE_GUNNER`) |
| a building turret's two views: third person on the seat's look, R3's zoom (the toggle, BF2's stick zoom, the eye on the aim frame, the class's `FirstPersonFOV`, his building left out of his own view; card VTURZ) | `core/src/sim/guns.rs` (`tick_seat_zoom`, `seat_zoom`, `SeatLook::scope_eye`) | `core/src/vehicle/gun.rs` (`GunState::zoom`, `GunSetup::scope_eye`, `ray_eye`), `core/src/sim/seats.rs` (`ride`: the press), `core/src/sim/vehicles.rs` (`unseat_unit`: the pilot change), `game/src/walk/camera.rs` (`seat_camera`, `zoom_fovs`), `game/src/walk.rs` (`turret_scope_camera`), `game/src/vehicle_class.rs` (`SeatSpec::first_person_fov`), `game/src/viewer/turret_pose.rs` (`hidden_for`, `view_draws`), `game/tests/suite/turret_views.rs` (the camera on the look, the own-view hide against a second view, the PS2's zoomed camera on 8 RAM rows), `game/examples/pose_render.rs` (`POSE_ZOOM`), `analysis/re-turret-draw.md` 4.1 |
| a vehicle's rider drawn or hidden, where he hangs, his seat pose (card VRIDE) | `game/src/soldier_vis.rs` (`rider_now`), `game/src/soldier_vis/runtime.rs` (`step_hover_nines`) | `game/src/vehicle_class.rs` (`RiderHang`, `SeatSpec::animated_pilot_position`), `game/src/vehicle_pads.rs` (`PadBuild::riders`), `game/src/viewer/turret_pose.rs` (`HoverNine`), `game/src/viewer/walker_pose.rs` (`WalkerRig::node_now`), `game/src/viewer/vehicles.rs` (`walker_rigs`), `bake/src/hardpoints.rs` (the bind axes), `bake/src/soldier.rs` (`SEAT_CLIPS`), `game/tests/suite/vehicle_rider.rs` (the census `game/tests/data/vehicle_rider_census.txt`, the PS2's placements, the bike rides `game/tests/data/vehicle_rider_bike_<game>.txt`, the draw end to end) |
| walker close-camera visibility and smooth between-tick animation | `game/src/camera_collide.rs`, `game/src/render_blend/walker.rs` | `game/src/viewer/vehicles.rs` (`visibility_opacity`), `game/src/soldier_vis/runtime.rs` (rider attachment at draw time), `game/tests/walker_close.rs`, `game/tests/walker_nodes.rs` |
| walker riding controls and AI-held armed seats (issue #67 follow-up) | `core/src/sim/look.rs`, `core/src/sim/guns/seat_switch.rs` | `core/src/vehicle/gun.rs` (positive-domain rear yaw and aimer bind frame), `core/src/ai/gunner.rs`, `game/src/vehicle_pads.rs`, `game/src/walk.rs` (main gun camera mount), `game/tests/walker_controls.rs`, `analysis/walker-aim-67.md` |
| walker head/guns aim over the walk pose (issue #67) | `game/src/viewer/walker_aim.rs` | `game/src/vehicle_class/aim.rs`, `game/src/viewer/walker_pose.rs` (`aimed_palette`, `node_now`), `game/src/viewer/vehicles.rs`, `game/src/soldier_vis/runtime.rs` (`walker_fire_frame`), `bake/src/walker.rs` (`restore_atte_cannon_offset`), `game/tests/walker_nodes.rs`, `game/examples/walker_aim.rs`, `analysis/walker-aim-67.md` |
| how a walker is drawn (skin, pose, leg layers) | `game/src/viewer/walker_pose.rs` | `bake/src/walker.rs` (`skin_glb`), `bake/src/vehicle.rs`, `bake/src/glb.rs` (`model_to_glb_joints`), `game/src/viewer/vehicles.rs`, `game/src/assets.rs` (`load_glb_joints`) |
| a seat's gun (weapons, aimer, fire point, the seat switch) | `core/src/vehicle/gun.rs` | `core/src/sim/guns.rs`, `game/src/vehicle_pads.rs` (`seat_gun`), `bake/src/hardpoints.rs` (the fire points) |
| how a bot boards, drives and leaves a vehicle, and fires a seat gun | `core/src/ai/ride.rs`, `core/src/ai/walker_drive.rs` (a walker's sticks), `core/src/ai/gunner.rs`; a building turret: `core/src/ai/man.rs` (card TURR) | `core/src/ai/behave/brain.rs` (the dispatch), `core/src/sim/seats.rs` (`ai_use_pass`, `ride`), `core/src/ai/route.rs` (`mover`) |
| where a team spawns (posts' nodes, a command walker's points, the opening wave) | `core/src/sim/spawn/runtime.rs` (`spawn_points`, `walker_spawn_points`, `pad_walker_spawn_points`, `player_spawn_posts` / `pick_spawn_post`: the human's spawn-map pick) | `game/src/vehicle_class/walker.rs` (`SpawnPointLocation`), `game/src/walk.rs` (`add_posts`, `opening_wave`), `game/src/main/battle.rs` (`spawn_model`), `game/examples/round_to_outcome.rs` (`SPAWN_CENSUS`) |
| vehicle spawn pads (which class, when, the decay) | `core/src/vehicle_pad.rs` | `core/src/sim/vehicles.rs` (`tick_vehicle_pads`), `game/src/vehicle_pads.rs` |
| where a pad's vehicle stands (the pad's ray; a parked flyer's LandedHeight) | `core/src/sim/vehicles.rs` (`tick_vehicle_pads`, `PAD_RAY`) | `game/src/vehicle_class.rs` (`landed_height`), `game/src/vehicle_pads.rs` (`vis_floors`, `install`), `bake/src/vehicle.rs` + `manifest/src/lib.rs` (`ManifestVehicle::vis_min_y`), `game/tests/suite/vehicle_pads.rs` (the class table) |
| a vehicle's class numbers and seats | `game/src/vehicle_class.rs` | `game/src/classdb.rs` (`load_map`), `bake/src/map.rs` (the map's `classes.ron`) |
| texture alpha, cutouts | `bake/src/alpha.rs` | `render/src/shader.wgsl` (`MASK_CUTOFF`) |
| a soldier's alpha (opaque or blended, never tested) | `bake/src/soldier.rs` (`segment_blends`) | `render/src/draw.rs` (`SkinnedDrawCmd::layer`), `render/src/frame.rs` (the skinned opaque draws, `blend_order`) |
| the specular second pass (which segments, its colour, the table, the tex-gen) | `bake/src/spec.rs` | `game/src/viewer/scene.rs` (the world draws, the table, the BF2 near-scene cull), `game/src/soldier_vis.rs` (a side model's: `part_spec`, `with_spec`), `bake/src/skin_glb.rs` (a soldier's `extras.spec_pass`), `render/src/specular.rs`, `render/src/shader.wgsl` (`vs_spec`, `vs_spec_skinned`, `fs_spec`), `render/src/frame.rs` (`draw_key`, the skinned pass after the skinned opaque run, both before the water) |
| a pass's blend or depth state, a new pipeline | `render/src/pipelines.rs` (`finish_init`, `PipelineDesc`) | its field on `Renderer` in `render/src/lib.rs`, its selection in `render/src/frame.rs`, `render/src/shader.wgsl` |
| the order the frame draws in | `render/src/frame.rs` (`render_frame`, `draw_key`, `sort_runs`, `blend_order`, `sort_far_to_near`) | `render/src/draw.rs` (`DrawLayer`) |
| where a sky shell sits, how it composites | `render/src/sky.rs` (`Placement`, `SkyMode`) | `game/src/viewer/scene.rs` (the sky shells), `bake/src/lighting.rs` |
| a geometry drawn as its LOWD past the near scene (card LOD: the model to `NearSceneRange[2]`, the LOWD from `[1]`, one dithered threshold; `OPENBF_LOD=0` = off) | `game/src/viewer/scene.rs` (`upload_scene`), `game/src/viewer/look.rs` (`lod_bands`, `lod_mode`) | `game/src/assets.rs` (`Scene::resolve_lowd`), `render/src/draw.rs` (`LodSwitch`, `instance_cull`, `all_discarded`), `render/src/frame.rs` (the distance drop, `draw_key`'s LOWD rank: card LODP), `render/src/shader.wgsl` (`far_cull`), `render/tests/suite/lod_switch.rs` |
| what the PS2's far scene adds when the switch is off (a LOWD's extra geometry, BF2 UTA1's far wall) | `bake/src/far_lowd.rs` | `manifest/src/lib.rs` (`ManifestGeometry::far`), `game/src/assets.rs` (`Scene::resolve_far`), `game/src/viewer/scene.rs`, `render/src/draw.rs` (`cull_near`) |
| PC / HD textures | `bake/src/tier.rs` | `game/src/assets.rs` |
| soldier animation | `game/src/animator.rs` | `bake/src/soldier.rs`, `bake/src/anim_sets.rs` (the PS2's sets per weapon, baked), `manifest/src/anim_sets.rs` (the file), `game/src/roster.rs` (`class_anim`: a class's family and its weapons' banks), `game/src/animator/aim.rs` (alert, the aim pose), `game/src/soldier_vis.rs` (the weapon in hand picks the set; what makes a soldier alert), `game/src/gallery.rs` (a unit sheet through its set), `mungefs/src/sanm.rs` |
| sounds | `bake/src/sounds.rs` | `game/src/sounds.rs`, `game/src/event_sounds.rs` (which event starts which sound), `game/src/ambience.rs` (the world's ambience streams and loops), `audio/src/lib.rs`, `scripts/sound_bake_check.py` (every baked take against ffmpeg's decode of the disc) |
| a unit's hurt and death cries (card UCRY: a surviving hit over 2 % of the health left, the kill; the class's `HurtSound` / `DeathSound`; one voice per unit) | `game/src/event_sounds.rs` (`hurts`, `cries`) | `core/src/sim/state/events.rs` (`Event::Hit::health_left`), `game/src/sounds/playback.rs` (`cry`, `cries`, `play_cries`), `game/src/roster.rs` (`sound_key`), `game/src/walk.rs` (`Cries`), `audio/src/lib.rs` (`Shot`), `game/src/main/windowed.rs` (`play_event_sounds`), `game/tests/suite/unit_voice.rs` (the rule, the victim's class, every showcase class's voice and take), `analysis/re-unit-chatter.md` 2.7, 3.6 |
| BF1's combat calls (card UCALL: "there he is" at the tracker's take, "grenade!" from the first AI soldier within 7.5 m of a live grenade once a second, "heard something" when hearing turns a bot; the class's `AcquiredTargetSound` / `FleeSound` / `HeardEnemySound` on the voice queue; never hide / approach / predamage) | `core/src/sim/step.rs` (the write-back's `Event::Call`, `grenade_calls`), `game/src/announce.rs` (`Commander::calls`, `gate_calls`) | `core/src/perception.rs` (`choose_target` returns the take), `core/src/ai/behave/brain.rs` (`NavTick::acquired`, `heard_turn`), `core/src/sim/state/events.rs` (`Call`), `core/src/sim/state.rs` (`GRENADE_WARN_DELAY`), `game/src/sounds/playback.rs` (`ask_line`), `game/src/walk.rs` (`Cries::call`), `game/src/roster.rs`, the take locks (`mungefs/src/sound_props.rs` `take_lock`, `bake/src/sounds.rs` `take_locks`, `manifest` `SoundProp::take_locks`), `game/src/event_sounds.rs` (`SoundLog::voice`: SOUND_LOG's `call` / `voice` rows), `game/tests/suite/unit_calls.rs` (every fielded BF1 class's own voice, the Gungans'), `analysis/re-unit-chatter.md` 1, 2 |
| BF1's AI squad talk and the low-health voice (card UTALK: a trooper's "follow me" (0.3) as it sets out for its post and "move out" as it arrives; a ground pilot whose pickup wait starts or whose first route or subsequent reached-post command-walker route sets out calls for a gunner (0.3, independently of ground gun-seat capacity/armament; WGCAL/WGOUT/WGSEAT), while other vehicles retain entry timing and every friendly AI on foot within 30 m answers "yes"; GBOARD/WGORD give ground receivers the called vehicle at their next think with existing claim arbitration and retain prior Hold/Follow commands; `LowHealthSound` under `LowHealthThreshold`, the Gungans' 1.1) | `core/src/sim/step.rs` (the write-back's follow / move out), `core/src/sim/seats.rs` (`need_gunner_call`), `game/src/announce.rs` (`Commander::calls`, `gate_calls`) | `core/src/ai/behave/brain.rs` (`NavTick::trooper`), `core/src/sim/state/events.rs` (`Call`), `core/src/sim/state.rs` (`FOLLOW_CHANCE`, `NEED_GUNNER_CHANCE`), `core/src/announcer.rs` (`Line::speaker`, `VoiceQueue::holds` / `release`), `game/src/roster.rs` (`cries`), `game/src/walk.rs` (`Cries`, `LOW_HEALTH_THRESHOLD`), `game/src/sounds.rs` (`streamed`), `game/tests/suite/unit_calls.rs`, `analysis/re-squad-orders.md` 5, `analysis/re-unit-chatter.md` 2.8 |
| BF2's unit voice (card UVO2: the class's `VOSound` lines picked by event and context, said only by a unit within 50 m of the event's subject - the player when none is named; AcquiredTarget at the tracker's take, FriendlyFire, Grenade, KillingSpree4 / 8 for a human's kills, Nearby Friendly / Enemy Slaughter; BF2 steps past a locked take) | `game/src/vo.rs` | `game/src/announce.rs` (`Commander::calls`, `vo_body`, `gate_calls`), `core/src/sim/step.rs` (the tracker's take and the grenade warning on BF2 too), `core/src/sim/state/events.rs` (`Call::Vo`), `game/src/roster.rs` (`cries`: `vo`), `game/src/walk.rs` (`Cries::vo`), `game/src/sounds/playback.rs` (`ask_line_with`), `game/tests/suite/unit_vo.rs` (every fielded BF2 showcase class's lines and unit type against the disc census, the pick by unit type, a take for each), `analysis/re-unit-chatter.md` 3, 5 |
| which sub-level's definition of a sound a battle plays (the era of a commander's line) | `game/src/sounds.rs` (`map_prop`, `find_named`), `game/src/foley.rs` (`sublevels`, `foreign_armies`) | `scripts/gen_missions.py` + `game/src/mission.rs` (`sound`), `bake/src/sounds.rs` (`level`), `game/tests/suite/sound_sublevels.rs` |
| vehicle engine loops (which record, on / off, what feeds the graphs) | `mungefs/src/sound_props.rs`, `bake/src/sounds.rs` (`engines`) | `game/src/engine_sound.rs`, `game/src/sounds.rs` (`loop_voices`), `game/src/main/windowed.rs` (`play_event_sounds`) |
| footsteps and stance sounds (which row, which surface, when a step starts, who steps) | `mungefs/src/foley.rs`, `bake/src/foley.rs` | `game/src/foley.rs`, `game/src/main/windowed.rs` (`play_event_sounds`) |
| the battle's music (which Music, when, fades and gaps, the streamed take) | `core/src/music.rs` | `game/src/music.rs`, `game/src/sounds.rs` (`music`), `audio/src/stream.rs`, `game/src/mission.rs` (`MusicCalls`), `scripts/gen_missions.py`, `mungefs/src/sound_props.rs` (`find_music`), `game/src/main/windowed.rs` |
| the commander's voice-overs (which line, when, the voice queue) | `core/src/announcer.rs` | `game/src/announce.rs`, `game/src/layout.rs` (`CommandPostDef::vo`), `game/src/mission.rs` (`VoiceDef`), `scripts/gen_missions.py`, `game/tests/suite/announcer_ps2.rs` |
| the BF2 HUD's data (layouts, fonts, the extra sheets) | `bake/src/hud.rs` | `mungefs/src/hud.rs` (record names), `mungefs/src/font.rs`, `manifest/src/hud.rs`, `analysis/re-hud.md` |
| a manifest field | `manifest/src/lib.rs` | its writer and reader; rebake every manifest group |
| a new disc chunk | a new `mungefs` module | a `bake` module |
| a new bake group | `bake/src/main.rs` | an `if want <group>` block in `scripts/rebake.sh` and its name in `REBAKE_GROUPS` there |
| a CLI or `OPENBF_*` flag | `game/src/main.rs` | — |
| the capture icon (the post he stands in, on the HUD) | `game/src/capture_feed.rs` (the rule) | `ui/src/hud_capture.rs` (the draw), `core/src/sim/round.rs` (`Sim::cp_inside`, `counts_in_cp`: one rule with the capture tick), `game/src/main/battle.rs` (`hud_model`), `game/tests/capture_display_ps2.rs` (the PS2's rows; the frame box on screen), `analysis/re-capture-display.md` |
| what a playtest snapshot holds, when it is written | `game/src/blackbox.rs` (the rings, the file, the sound rows' digest), `game/src/unit_probe.rs` (a unit's row), `game/src/sounds.rs` (`handed`: the sound rows) | `game/src/main/windowed.rs` (F9, quit, the alarm), `game/examples/blackbox_read.rs` (the digest), `game/examples/round_to_outcome.rs` (`HeadlessBox`), root `CLAUDE.md` |
| a golden's framing | `game/src/gallery.rs` | `game/src/viewer/camera.rs` (`GOLDEN_CAM`), `scripts/goldens.sh` |
| what the sim hash covers | `core/src/sim/state/hash.rs` (`Sim::hash`) | a declared [SWEEP] re-baseline |

- `game/src/fx/ripple.rs` 470 — flat-water ripples: the PS2's 100-record pool (kept while a camera is within 120 m and the ripple lives), spawned by velocity-shaped soldier wakes (2 s, 0.25 m), bolt water hits (1.5 s, 0.01 m) and water blasts (3 + s, s = min(0.4, R/2)), drawn as the 21-column three-strip `FXrippl1` ring lit from the manifest's water colour and sun; the splash bob lift · `Water`, `Look`, `ring`, `Fx::set_water`, `Fx::ripples`; `analysis/re-fx.md` §15; referees the module tests and `game/tests/suite/water_fx.rs` (KAS1 mask and GPU frame).
- `game/src/fx/trail.rs` 624 — per-projectile trail identities, survivor matching and independent retirement · Rounds, Fx::follow_rounds; repeated collinear shots stay distinct.
- `game/src/fx/jet.rs` 157 — A jet soldier's `JetEffect` (card JETP2, `analysis/re-jetpack.md` §8): spawned once when he is in a jet stance and has none running, riding him
- `game/src/fx/tests/jet.rs` 225 — Card JETP2: a soldier in a jet stance carries his class's `JetEffect`; it stops when the jet ends or he dies, and a new jet spawns a new one.
- `core/src/mine.rs` 195 — placed-explosive class data, detonation causes, flight/attachment state and replay serialization; the half-step fall and friction rebound thrown items share · MineClass, Detonation, Mine, fall, rebound.
- `core/src/sim/mines.rs` 482 — placed-explosive launch, limits, damage, flight, arming, proximity/contact/timer triggers and remote input; the throw origin, oldest-beyond-the-limit, item sweep, item damage and `damage_item` shared with droids and auto-turrets.
- `core/src/sim/mines/tests.rs` 390 — placed-explosive gameplay, damage, attachment and input regression tests.
- `game/src/roster/mines.rs` 18 — mine entity and trigger/expiry/destroyed explosion resolution through baked class chains.
- `core/src/remote.rs` 506 — the remote droid's class data, launcher, destruct (salvo / burst), flight step, drain and replay state · DroidClass, RemoteWeapon, Destruct, Salvo, Droid, StrikeRun; `analysis/re-remote-droid.md`.
- `core/src/remote/tests.rs` 183 — droid class parsing, hover, steering, drain and salvo geometry tests.
- `core/src/sim/remotes.rs` 682 — droid launch, control routing and hand-back (exit or a weapon switch forward), flight over the world, blaster, destruct, damage, salvo runs and the bot's droid pad; `free_shots`, the ownerless-gun bolts the auto-turret fires too, and `is_item_gun`, which keeps their draw off the owner's hand.
- `core/src/sim/remotes/tests.rs` 1377 — remote droid control, expiry, combat, destruct, bot, weapon-switch hand-back, bots shooting droids (and not friendly, hidden, lost or destroyed ones; seated gunners), yaw wrap, shared item weapons and replay regression tests.
- `game/src/roster/remotes.rs` 162 — the remote weapon's droid, blaster and destruct (salvo shell or burst) resolution through baked class chains.
- `core/src/autoturret.rs` 322 — the BF2 sniper's portable auto-turret: class data with the portableturret constructor defaults, fall / land / bounce, lifetime, the hover-stick aim turn and replay state · TurretBody, TurretClass, Turret, TurretEnd, Aim; `analysis/re-portable-turret.md`.
- `core/src/autoturret/tests.rs` 99 — turret class parsing, lifetime, landing / bounce and turn-law tests.
- `core/src/sim/tow.rs` 587 — Mounted harpoon flight, posed latch, world-space wrap tangents, post-wrap damage and fall pose · `TowCable`, `TowFall`.
- `core/src/sim/tow/tests.rs` 480 — Cable geometry, wrap tangents and order, release, and post-wrap damage fixtures.
- `core/src/sim/turrets.rs` 334 — turret throw and MaxItems replacement, fall over the world, the stationary controller's target choice, aim and fire, and its end and explosion.
- `core/src/sim/turrets/tests.rs` 582 — auto-turret deploy, range, kill credit, BuildingScale damage, expiry, replacement, the bot cluster rule, a long bot fight and replay regression tests.
- `game/src/roster/turrets.rs` 40 — the auto-turret dispenser's turret entity, gun and explosion resolution through baked class chains.
- `core/src/cloak.rs` 111 — Bothan cloak properties, concealment curve, byte visibility and deterministic state.
- `core/src/sim/cloaks.rs` 113 — cloak activation, shared energy, drain, fade, flicker and lifecycle cleanup; behavioral tests in `core/src/sim/cloaks/tests.rs` cover perception memory and portable-turret visibility integration.
- `core/src/sim/cloaks/tests.rs` 244 — cloak activation, drain, reveal, flicker, perception and turret-visibility tests.
- `game/tests/suite/cloaking.rs` 87 — inherited ordinary-class cloak census and activation against the local disc bake.
- `core/src/guided.rs` 262 — award rocket class, quaternion flight, energy/boost, distinct end reasons and replay state; separate from recon-droid hover movement.
- `core/src/sim/guided.rs` 308 — guided launch, player/bot control routing, swept collision, detonation and control return; behavioral tests in `core/src/sim/guided/tests.rs`.
- `core/src/sim/droid_death.rs` 104 — The droid's chunk death (card DROIDDEATH, `analysis/re-droid-death.md` [V]): a CIS soldier droid that dies bursts 6 times in 10 - its class's
- `core/src/sim/droid_death/tests.rs` 334 — Card DROIDDEATH's referees (`analysis/re-droid-death.md`): the death roll's 6 in 10 with no damage threshold, the push roll's mark, the burst
- `core/src/sim/guided/tests.rs` 373 — guided launch, control, collision, detonation and hand-back tests.
- `game/tests/suite/guided_rockets.rs` 377 — all 14 inherited guided launchers, award gating, camera/interpolation restoration, smooth mouse/controller turning at 60/75/144 Hz and all-side/map-local equipment census.

- `game/src/fx/chunks.rs` 1019 — cosmetic droid and vehicle chunk launch, bounded pool and floor contacts.
- `bake/src/attached_effects.rs` 149 — ordered AttachEffect replay and baked hardpoint frames/repeat bounds.
- `game/src/fx/props.rs` 415 — static prop effect frames and optional repeat timers, presentation only.
- `render/src/bloom.rs` 319 — selective BF2 pre-clamp emissive gather and authored blur · `Bloom`
- `render/src/bloom.wgsl` 65 — threshold, five-tap blur and display-space composite · bloom fragment entry points
- `render/src/falling.rs` 484 — airborne leaf pool, flutter/tumble, visibility retirement and world quads · `LeafEmitter`


## PR #77 effect modules and referees

- `mungefs/src/lightbeam.rs` 267 — LightBeam config parsing and defensive numeric defaults · `LightBeamConfig`.
- `mungefs/src/shadow.rs` 292 — `shdw` closed half-edge meshes, PS2 and Xbox/PC layouts · `ShadowPiece`, `is_skinned`; `analysis/re-vehicle-shadow.md` §9.
- `game/src/forest.rs` 105 — scene bird configuration and read-only simulation actor snapshots · `configure`, `submit_actors`.
- `game/src/fx/chunks/tests.rs` 618 — chunk launch, inertia, contacts, lifetime, pool and real-data regressions.
- `render/src/falling/birds.rs` 313 — bounded anchor bird pool, motion, atlas wings and lifecycle · `ForestBirdType`.
- `render/src/falling/ground.rs` 294 — terrain gusts and actor contact impulses · `LeafActor`.
- `render/tests/suite/particle_blend.rs` 443 — GPU referees for display-byte blending, masked scene blur and leaf/bird cutouts.

- `bake/src/droid_chunks.rs` 587 — disc-derived SBD sections, five Magnaguard mesh cuts and collision spheres; every droid chunk model (the battle droid's too) with its sheets renamed `openbf_droid_*` so no living soldier sheet is replaced · `bake`, `manifest::DroidChunkManifest`; guard: `bake/tests/suite/side_steps.rs` 197 (no `rebake.sh side` step rewrites or shadows another's file).
- `game/examples/droid_chunks.rs` 104 — offscreen paired droid deaths through the normal class, event, soldier and effect render paths.
## BF2 hero modules

- `bake/src/combo.rs` 582 — strict raw PS2 combo config decoding, documented gameplay output corrections and all-disc keyword/clip closure.
- `bake/src/combo/compatibility.rs` 91 — one-field Vader terminal target correction; raw-reader evidence, valid recovery states and other programs remain unchanged.
- `bake/src/heroes.rs` 234 — complete BF2 hero dependency and model bake entry point.
- `core/src/ai/fix/tests.rs` 250 — repair-target selection referees and focused hero regression fixtures.
- `core/src/ai/melee.rs` 207 — native melee close-mode inputs and fair navigation probes.
- `core/src/combo/conditions.rs` 331 — packed combo conditions and transition selection.
- `core/src/combo/output.rs` 351 — authored animation, damage-window, block and movement outputs.
- `core/src/combo/tests.rs` 506 — focused hero regression fixtures.
- `core/src/combo.rs` 407 — deterministic authored combo states, transitions, input events and timing.
- `core/src/hero_weapon.rs` 759 — BF2 Force, blade, combo and throw configuration and future state hashes.
- `core/src/sim/force/choke_review/tests.rs` 60 — focused hero regression fixtures.
- `core/src/sim/force/tests.rs` 469 — focused hero regression fixtures.
- `core/src/sim/force.rs` 482 — Force contacts, held choke, ordinary damage effects and input locks.
- `core/src/sim/heroes/tests.rs` 429 — focused hero regression fixtures.
- `core/src/sim/heroes.rs` 649 — BF2 hero eligibility, offers, replacement, cooldown and kill healing.
- `core/src/sim/saber/deflect.rs` 221 — native block payment, incoming arc and projectile redirection.
- `core/src/sim/saber/hits.rs` 254 — posed blade contacts and per-attack damage delivery.
- `core/src/sim/saber/tests.rs` 477 — focused hero regression fixtures.
- `core/src/sim/saber.rs` 266 — authored saber input and movement consumer.
- `core/src/sim/saber_throw/tests.rs` 418 — focused hero regression fixtures.
- `core/src/sim/saber_throw.rs` 422 — primary-attachment saber flight, rotating contacts and return.
- `core/src/unit/melee.rs` 146 — expanded native hurt-primitive blade query.
- `game/src/animator/combo_pose.rs` 181 — authored saber and offhand poses over real hero rigs.
- `game/src/cloth/tests.rs` 231 — focused hero regression fixtures.
- `game/src/cloth.rs` 348 — hero cloth pins, constraints and collision simulation.
- `game/src/fx/force/lightning.rs` 142 — authored LightningBolt strip construction.
- `game/src/fx/force/tests.rs` 186 — focused hero regression fixtures.
- `game/src/fx/force.rs` 428 — Force visual lifecycle and authored ordnance-effect lookup.
- `game/src/hero_blades/tests.rs` 279 — focused hero regression fixtures.
- `game/src/hero_blades.rs` 544 — real posed blade tracks and deterministic integer-frame input bake.
- `game/src/heroes.rs` 326 — mission hero roster installation and authoritative tick score feed.
- `game/src/soldier_vis/cast.rs` 314 — actual offhand cast animation selection.
- `game/src/soldier_vis/cloth.rs` 179 — live hero cape meshes and scene lighting.
- `game/src/soldier_vis/saber.rs` 322 — authored saber hilts, edges, trails and thrown attachment rendering.
- `game/src/sounds/hero.rs` 199 — saber visibility and flight-loop audio properties.
- `manifest/src/cloth.rs` 40 — optional raw cloth simulation companion schema.
- `manifest/src/combo.rs` 159 — typed combo keywords and recursive authored config schema.
- `manifest/src/shadow.rs` 9 — the render-only shadow sidecar's row · `ShadowPiece`.

## BF2 hero follow-up modules

- `bake/src/combo/hostile.rs` 94 — Focused regression fixtures.
- `bake/src/combo/validate.rs` 188 — Hero data and runtime helpers.
- `core/src/combo/input/tests.rs` 57 — Hero data and runtime helpers.
- `core/src/combo/input.rs` 145 — Native eight-event input history, B2 0033f908/0033fb80/0033fe38.
- `core/src/combo/tests/packed.rs` 148 — Focused regression fixtures.
- `core/src/combo/tests/reference.rs` 159 — Focused regression fixtures.
- `core/src/combo/thrust.rs` 168 — Retained-input rotation in FUN_0033f600, with its literal 45-degree caps.
- `core/src/combo/timing.rs` 38 — EE single-precision arithmetic rounds finite timing operations toward zero.
- `core/src/hero_weapon/cache/tests.rs` 95 — Hero data and runtime helpers.
- `core/src/hero_weapon/checked.rs` 13 — Hero data and runtime helpers.
- `core/src/hero_weapon/deflect.rs` 140 — Native block pose lifecycle and the cached 0.1-second attachment pose.
- `core/src/hero_weapon/tests/hostile.rs` 220 — Focused regression fixtures.
- `core/src/sim/force/choke.rs` 253 — The PS2 Ray area is a bounded angular slab, with retained Hold targets.
- `core/src/sim/force/choke_tests.rs` 225 — Native angular slab and no late acquisition controls.
- `core/src/sim/force/rate.rs` 156 — The area weapon overrides the soldier's ordinary energy recovery rate.
- `core/src/sim/hero_bodies.rs` 176 — Physical hero queries include the native exposed-rider collision type.
- `core/src/sim/heroes/tests/spawn.rs` 356 — Focused regression fixtures.
- `core/src/sim/saber/deflect/selection.rs` 96 — Direction buckets and fallback masks in FUN_003074a8.
- `core/src/sim/saber/movement.rs` 218 — Native gravity/target latch before the soldier's ordinary integration.
- `core/src/sim/saber/tests/alignment.rs` 105 — Focused regression fixtures.
- `core/src/sim/saber/tests/deflect.rs` 256 — Focused regression fixtures.
- `core/src/sim/saber/tests/interruption.rs` 175 — Focused regression fixtures.
- `core/src/sim/saber/tests/riders.rs` 37 — Physical contacts distinguish exposed riders from enclosed crews.
- `core/src/sim/saber/tests/targets.rs` 298 — Physical contacts use the same blade queries for either controller.
- `core/src/sim/saber_throw/tests/reference.rs` 96 — Optional original-content referee. The private fixture is not distributed.
- `core/src/sim/saber_throw/tests/targets.rs` 121 — Static station bodies and dynamic vehicle hulls share the flight query.
- `game/tests/suite/coruscant_turret_contact.rs` — Optional retained-scene witness: stock Maul at the reported pose damages the bound Coruscant minigun through its authored map bodies; requires CORUSCANT_TURRET_ASSETS.
- `core/src/soldier/tests/motion.rs` 101 — The weapon callback supplies desired movement before ordinary integration.
- `game/src/animator/offhand_pose.rs` 48 — A melee set's named offhand action over the ordinary movement pose.
- `game/src/hero_blades/throw_bind.rs` 99 — Melee-throw construction samples ThrowAnimation at InitialSalvoDelay.
- `game/src/hero_riders.rs` 317 — Deterministic collision frames for exposed seats and authored walker tow primitives from the baked rig.
- `game/src/sim_pose.rs` 49 — Portable pose sampling for collision/setup inputs; drawing keeps its own sampler.
- `game/src/soldier_vis/saber/tests.rs` 78 — Hero data and runtime helpers.
- `game/src/soldier_vis/saber_trail/history/tests.rs` 96 — Hero data and runtime helpers.
- `game/src/soldier_vis/saber_trail/history.rs` 190 — Native eight-sample origin/direction history (B2 45c6f0/45c940),.
- `game/src/soldier_vis/saber_trail.rs` 104 — Saber trails keep actual posed world frames rather than resampling a.
- `render/src/particles/trail.rs` 127 — The native saber trail has a colored sweep between origin and tip,.

### Additional BF2 hero modules

- `bake/src/combo/loads.rs` 98 — Native ReadDataFile requests identify hashed children of lvl_, not classes.
- `bake/src/combo/scope.rs` 97 — Retain SCOP presence after the shared value decoder. This is combo-local;
- `game/src/animator/custom/tests.rs` 240 — Focused BF2 hero regression coverage.
- `game/src/animator/custom.rs` 260 — BF2's shared custom-animation descriptors. The first actual SCOP locks a
- `game/src/hero_blades/physical.rs` 99 — Validation at the boundary where baked figure data becomes gameplay input.
- `game/src/hero_blades/root_speed.rs` 62 — BF2 282700 measures the first half, then caches signed15-bit cm/s.
- `game/src/hero_blades/tests/boundary.rs` 454 — Focused BF2 hero regression coverage.
- `game/src/soldier_vis/saber/tests/flight.rs` 293 — Focused BF2 hero regression coverage.
- `game/src/soldier_vis/saber/tests/visibility.rs` 142 — CPU trail/audio visibility counts at the eighth blade and malformed ninth-edge guard.
- `game/src/soldier_vis/tests/combo_aim.rs` 142 — Focused BF2 hero regression coverage.
- `game/src/soldier_vis/tests/hero_behavior.rs` 436 — hero contact and pose regressions on real maps.
- `game/src/spawn_page/tests/heroes.rs` 126 — Focused BF2 hero regression coverage.

### Additional BF2 hero modules

- `bake/src/sounds/repairs.rs` 70 — Explicit PS2 data repair: TAN omits three female-voice aliases that TAT
- `core/src/sim/heroes/tests/drain.rs` 325 — Focused BF2 hero regression coverage.

- `game/src/viewer/droidekas/preview.rs` 63 Deployed droideka idle skin for the spawn carousel.
- `game/src/viewer/droidekas/aim.rs` 213 Authored gun-subtree pitch, muzzle palette and native-mesh controls.
- `game/src/bolt_vis/shots.rs` 251 Native shot provenance and bounded legacy shot association.
- `core/src/unit/tests/audit.rs` 117 Aim-history hashing and human/proxy continuation regressions.
- `bake/tests/suite/sound_repairs.rs` 114 — Original PS2 Tantive alias and byte/PCM controls.
- `core/src/sim/guns/seat_switch.rs` 106 — seat cycling in either direction, friendly AI takeover and reseating without a new bail/RNG draw.
- `game/tests/walker_controls.rs` 260 — actual Geonosis pilot yaw/body following, both AI-held gun seats, their weapons, rear bind direction/arc and main gunner camera origin.
- `game/examples/support/walker_battle.rs` 85 — shared local Geonosis battle fixture for walker evidence examples and integration tests.

### Non-hero ordnance modules (missiles, emitters, beams, fat rays, tracers)

- `core/src/emitter.rs` 481 — the emitter ordnance query law: one instant cone-and-chain hit per shot.
- `core/src/missile.rs` 531 — missile class data, flight law and lock target.
- `core/src/sim/beam.rs` 128 — beam ordnance: a held ray cast as a per-tick round; `fly_bolts`.
- `core/src/sim/beam/tests.rs` 130 — beam scene tests.
- `core/src/sim/burn.rs` 47 — an emitter's ignite and burn effects, over the one `hero_weapon::Burn`.
- `core/src/sim/burn/tests.rs` 229 — emitter burn scene tests.
- `core/src/sim/emitter.rs` 754 — emitter shots: gather, query, damage, push and arc events.
- `core/src/sim/fatray.rs` 83 — fat ray ordnance damage along a wide sweep.
- `core/src/sim/fatray/tests.rs` 168 — fat ray scene tests.
- `core/src/sim/missiles.rs` 145 — missile steering step and the launcher dwell lock.
- `core/src/sim/missiles/tests.rs` 575 — missile homing, expiry and dwell lock scene tests.
- `core/src/sim/ordnance/tracers.rs` 311 — BF2 secondary ordnance: the periodic round beside the primary.
- `core/src/sim/saber/tests/contact.rs` 107 — hand blade contact effects and sounds.
- `core/src/sim/saber/tests/turning.rs` 183 — a human hero turns during an attack; bots keep the lock.
- `core/src/sim/saber_throw/tests/collision.rs` 365 — thrown saber stops and returns on collision.
- `core/src/weapon/beam.rs` 71 — beam ordnance class data.
- `core/src/weapon/emitter.rs` 254 — emitter ordnance class data.
- `core/src/weapon/fatray.rs` 72 — fat ray ordnance class data.
- `core/src/weapon/test_props.rs` 17 — property list fixture for the weapon class tests.
- `game/src/event_sounds/ordnance.rs` 295 — the flight hum of ordnance in the air, doppler pitched.
- `game/src/fx/arc.rs` 528 — emitter lightning arcs between a shot's links.
- `game/src/fx/flame.rs` 110 — burn effects on a burning unit's bones and flame shots.
- `game/src/fx/flame/tests.rs` 127 — burn effect and flame shot tests.
- `game/src/fx/tests.rs` 712 — particle effect behaviour tests.
- `game/src/roster/secondary_tests.rs` 79 — secondary ordnance roster tests against the BF2 class bake.

- `game/src/fx/turret.rs` 53 — portable turret damage-band effects and lifecycle · `Fx::follow_turrets`.
- `game/src/fx/tests/turret.rs` 106 — PS2 portable turret bands and headless GPU emitter lifecycle regression.

- `game/src/holo/build.rs` 176 — build-pad hologram state, cutter-carrier visibility and regression tests.

### Bot jet control and BF2 fuel HUD

- `core/src/ai/jet.rs` 301 — rechargeable foot-bot jet scheduler, tactical sky/optional barrier-line clearance, committed flight and hash.
- `core/src/ai/jet/tests.rs` 623 — launch order, fuel/range gates, height steering and deterministic hops.
- `game/tests/suite/jet_bots.rs` 276 — original class route eligibility, BF2 fuel-bar lifecycle, isolated Mustafar links with mission hazards and PS2-measured damage-region controls.
- `game/tests/suite/jet_round.rs` 286 — seeded headless Mustafar traversal round: four jet troopers plan both links out and back, land alive without hazard contact or stalled progress, and replay every tick.

- `game/examples/attachment_load_report.rs` 525 — CPU-only actual vehicle load routes, geometry/alpha hashes and whole-input catalog stability; explicitly excludes GPU and collision acceptance.
- `mungefs/src/sun_flare.rs` 70 — BF2 root SunFlare config parser, including angle, halo rings and optical settings · `SunFlare::parse`
- `bake/src/map/atmosphere.rs` 47 — authored SunFlare schema reduction and DustEffect alpha consumers · `sun_flare`, `dust_textures`
- `game/src/viewer/dust.rs` 167 — active BF1/BF2 placed dust/mist classes to renderer pools; BF1 Geonosis and BF2 Hoth conquest placement regressions · `load`
- `game/tests/suite/atmosphere.rs` — BF1 Geonosis headless sky motion, paused flight, dust and FogCloud attribution; run through `scripts/gpu_tests.sh --game atmosphere`.
- `render/src/dust.rs` 222 — shared BF2 DustEffect particle pools, placed volumes and life/radius alpha · `Dust`, `DustConfig`
- `render/src/sun_flare.rs` 228 — authored sun direction, native display pass and source depth visibility · `SunFlare`, `Renderer::set_sun_flare`
- `render/src/sun_flare.wgsl` 55 — depth-filtered additive sun triangles and HDR gather; native source-mask filtering remains provisional
- `render/src/sun_flare/geometry.rs` 316 — original sun/halo polygons, expanded source masks, two spike fans, thirteen reflections and alignment glare · `build`

- `game/src/bolt_vis/expiry_tests.rs` 86 — air-expiry remnant duration, fade and premature-removal controls.

- `game/src/render_blend/projectile_tests.rs` 82 — modelled bullet/missile interpolation, slot compaction, newborn isolation and bit-exact restoration.

- `game/tests/suite/rocket_expiry.rs` 116 — real-disc ExplosionExpire loading and wrist-rocket natural air expiry through authored effects.

- `game/tests/suite/rocket_visible.rs` 396 — all-side rocket/torpedo/recoilless/guided mesh census, faithful trail-only pins, headless side/head pixels and display-loop captures across source bakes.

- `bake/src/map/render_textures.rs` 492 — missing donor sheets, fingerprinted PC snow and PC/Xbox console replacements, turret alias and same-art full-size coarse atlases · `recover`, `recover_xbox_mustafar`, `larger_own`.

- `game/tests/suite/donor_materials.rs` 287 — owned map material census, unchanged gameplay manifests and serial headless normal-water captures.

- `game/tests/suite/collision_immunity.rs` 82 — partial/inherited CollisionScale immunity and explicitly enabled owned-input class checks for issue #113.

- `core/src/sim/deathmatch.rs` 126 — deathmatch kill scoring, score limits, team points, team size maintenance and Hero Assault class distribution · `Deathmatch`, `Sim::deathmatch_kill`, `Sim::step_deathmatch`, `Sim::assault_class`, `Sim::team_points`.

- `core/src/sim/deathmatch/tests.rs` 182 — deathmatch kills, suicides/team kills, score limit win condition, class pickers and hash invariance tests.

- `game/src/main/timed_battle.rs` 115 — fixed-duration headless battle evaluation, logging points, alive counts and hero class concurrency · `run`.

- `game/src/main/windowed/tests/assault.rs` 188 — owned-asset headless shell launch rejection, authored seats/options, partial hero rosters and spawn/death/respawn regressions.
- `core/src/sim/ordnance/tests/charged_sticky.rs` 199 — charged sticky launch, unchanged unknotted instance bits and blended fuse/warning controls at rest, in water and on an attached actor.

- `game/tests/suite/charged_sticky.rs` 140 — explicit retained-profile class loads and stepped FIRE observations for charged spear, rock, thermal controls and mortar stages.
