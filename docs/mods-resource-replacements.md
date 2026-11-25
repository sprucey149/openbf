# Legacy LVL Resource Replacement Mods

OpenBF supports generic legacy Star Wars Battlefront II (2005) LVL replacement mods without mandatory directory names, registration scripts (`addme.script`), or user-authored manifests.

Players copy extracted mod folders directly into `openbf_data/mods/`—whether the mod uses an original `_LVL_PC/` tree, conventional `side/` and `fpm/` folders, or a single loose `.lvl` file—and enable or reorder them in the launcher's **Mods** tab.

---

## 1. Architecture Overview

### 1.1 Discovery & Capabilities (`mungefs::modscan`, `extensions::mods::replace`)

- **Layout-agnostic scanning:** `mungefs::modscan::inspect_mod_folder` scans every `.lvl` file inside a mod folder alongside any addon registrations (`addme.script` / `mission.lvl`). Folder names act as resolution hints rather than prerequisites.
- **Content capabilities:** Mods are described by composable capabilities (`Capability`: `AddonMissions`, `WorldReplacement`, `SideReplacement`, `ShellSideReplacement`, `FirstPerson`, `Textures`, `Models`, `Animation`, `Audio`, `ShellHud`, `Localization`, `ClassPresentation`, `GameplayClasses`, `Collision`, `Effects`, `Missions`, `Other`) instead of mutually exclusive categories.
- **Read-only source safety:** Original mod files and stock game archives (`openbf.pak`, discs, PC installs) are never modified. All scan caches (`cache/file_hashes.ron`, `cache/lvl_facts.ron`), user target choices (`mod_targets.ron`), stock source references (`stock_sources.ron`), and converted native assets (`mod_cache/<namespace>/`) live in `openbf_data/`.

### 1.2 Stock Resource Catalog (`extensions::mods::catalog`, `bake::stock`)

- Generated during base asset preparation (`scripts/rebake.sh catalog` and `launcher::pipeline::rebake::bake_stock_catalog`) via `bake stock-catalog` and `bake stock-record`, and packaged inside `assets_baked/catalog/` (and therefore inside `openbf.pak`).
- Stores metadata only (zero copyrighted pixels, meshes, audio, or text):
  - `<game>_<platform>.ron`: schema/generator versions, source game & platform, logical path below `_LVL_<platform>`, file size, SHA-256, detected game/platform, `lvl_` sublevel hashes, per-chunk `(tag, name)` FNV-1a-64 payload digests, per-class `BASE` and per-property-key digests, and associated stock baked outputs.
  - `stock_bake.ron`: which platform supplied each game's gameplay data, the class property hashes treated as presentation-only, and SHA-256-verified whole-group dependencies (`bf1_anims`, `bf2_anims`).
- `StockCatalogs::digest_with` hashes the raw catalog bytes into a SHA-256 digest used for cache invalidation whenever the stock bake changes.

### 1.3 Evidence-Based Target Resolution (`extensions::mods::replace`)

Each candidate LVL is matched against the packaged stock catalogs using:

1. Explicit player selection (`openbf_data/mod_targets.ron`) or optional mod manifest (`openbf_mod.ron`), when present.
2. Backup/alternative folder filtering (`backup`, `old`, `orig`, `copy`, `copia`, `unused`, `disabled`, `alt`, `optional`, `variant`, `vanilla`, `stock`, etc. stay inactive unless explicitly selected).
3. Case-insensitive trailing relative-path suffix matching (preserving full relative paths such as `fpm/imp/allxwing.lvl` vs `fpm/rep/allxwing.lvl`).
4. Detected game (`BF1` / `BF2`) and platform (`PC` / `PS2` / `Xbox`) from internal chunk formats and `_LVL_<platform>` anchors.
5. Exact SHA-256 match against stock (`OpStatus::Identical`, skipped automatically).
6. Internal `(tag, name)` resource overlap with a minimum overlap and lead threshold for filename-only matches (allowing a loose `end1.lvl` to resolve unambiguously to `bf2 pc:end/end1.lvl` while refusing to guess between ambiguous `allxwing.lvl` copies).

Ambiguous LVLs surface a target selector (`ComboBox` of ranked candidates plus **Ignore**) directly in the launcher's **Mods** tab; selections persist in `openbf_data/mod_targets.ron` outside the mod folder and trigger automatic rescan and cache invalidation.

### 1.4 Scoped Overlay & Replacement Semantics (`pak::overlay`, `game::mods_runtime`, `manifest::class_patch`)

- **Priority order:** Higher in the launcher list (`#1`) wins. `extensions::mods::resolve_content_mods` detects override conflicts across both `declared_overrides` and active stock replacement targets.
- **Game scoping:** Replacement outputs are mounted in per-game `OverlayRule::Overrides` layers (`scope: Some("bf2")` or `Some("bf1")`). At battle load (`game::assets::scope_overlays_to_map`), `pak::set_overlay_scope` activates only the current map's game scope so BF2-only replacements never affect BF1 maps or units.
- **Full vs Patch mode & removed files:** In `ReplacementMode::Full` (default, matching original LVL replacement semantics), stock companion files no longer produced by a re-baked soldier/vehicle/world (e.g., stock cloth on a model that removed cloth) and removed stock classes are hidden via `ReplacementOutput { removed: true }` (`pak::overlay::Resolved::Removed`) and `ClassPatch::Remove`. In `ReplacementMode::Patch`, only changed/added resources are overlaid without removing stock companions or classes.
- **Class patching:** Side class changes are written as `<classes>.patch.ron` sidecars (`manifest::class_patch`) containing only properties whose digest differs from the stock PC catalog. `game::classdb` applies these patches over the stock PS2-canonical `classes/<g>_<side>.ron`, preserving untouched retail gameplay values.
- **Multiplayer compatibility:** `game::mods_runtime` includes `(mod_id, fp:replacement_key)` in `content_mod_multiplayer_digest` whenever a mod has active gameplay-changing replacements (conservatively treating any property outside `stock_bake.ron`'s presentation set as gameplay). Purely cosmetic replacements (e.g., texture-only Endor or cosmetic-only side skins) stay out of the gameplay digest.

### 1.5 PC LVL Conversion (`bake::mods::replace`)

- **SIDE (`side/<side>.lvl`):** Class patches (`classes/<g>_<side>.patch.ron`), re-baked soldiers (`bake_soldier_anims` with sibling mod/stock sides and verified animation banks), weapons (`bake_weapon_with`, hiding stock `pc_first_person/weapons/<m>.glb` overrides when replaced), vehicles (`bake_vehicles`), side effects (`bake_side`), standalone changed textures (`bake_textures_flagged`), and tier-2 mirroring (`both_tiers`).
- **First-person (`fpm/...`):** Re-baked first-person arm GLBs (`bake_fpm_with_bank`) with the stock first-person animation bank, plus standalone changed FPM textures.
- **World (`<planet>/<map>.lvl`, e.g. `end/end1.lvl`):**
  - *Texture-only fast path:* When only `tex_` chunks differ, re-encodes only the changed world textures (and `_dtl` companions) into `<map_dir>/textures/` at both texture tiers, matching each stock sheet's alpha state while preserving stock geometry, terrain heights, collision, navigation, and missions verbatim.
  - *Full world path:* When geometry, terrain, world layout, lighting, sky, water, collision, or classes change, stages the LVL alongside `SIDE/TUR.LVL` (mod or stock) and runs `bake::map::bake_map`, mirroring tier-2 textures while keeping stock `sounds/` and `load/` intact. For PC BF2 terrains (`mungefs::terrain::Terrain::pc_patches`), `bake::terrain_glb` and `bake::foliage` apply PC BF2's `core.lvl -> SHDR -> normal_terrain_shader.xml` non-`Static` directional lights with hue-preserving normalization on top of the patch `VBUF` static vertex colours so dynamic-sun PC maps (such as `end1`, `tat2`, `kas2`, `fel1`, `nab2`) match original PC brightness while leaving PS2 and `Static`-sun terrains untouched.
- **Load screens (`load/<map>.lvl`):** Re-baked via `bake_loadscreen`.
- **Honest unsupported reporting:** Unsupported resources (`GeometryAddon` PC 1.1 attachments, 3D starfighter cockpit models, normal/bump maps, shell-only side LVLs that OpenBF does not load, custom animation banks, unsupported MTRL blend modes, or malformed cloth literals) are explicitly recorded in `ModCacheMeta::unsupported` and `warnings` and displayed in the launcher rather than silently dropped.

---

## 2. Shared Interfaces & Integration Notes

- `extensions::mods::ModCacheMeta` (schema v1, converter v9): extended with `resource_ops`, `replacement_key`, `replacement_outputs`, `unsupported`, `catalog_digest`, and `target_choices` (`#[serde(default)]` for backwards-compatible deserialization).
- `pak::OverlayRule::Overrides`: carries `scope: Option<String>` and treats allowed paths absent on disk as `Resolved::Removed` (hiding stock files without mutating `openbf.pak`).
- `game::assets::scope_overlays_to_map`: invoked before battle prefetch in both `windowed/loading.rs` and `windowed/events.rs`, and at the start of `load_scene`.
- `bake::soldier::set_lender`: allows in-process configuration of borrowed animation LVL paths without mutating process environment variables.
- Compatible with existing addon map conversion (`ABR`, `KTH`), `side_bf2/pc_first_person` weapon handling, and the bundled `openbf.vr` extension.

---

## 3. Compatibility & Validation Report

> Validation notes distinguish **conversion/inspection checks** from **runtime gameplay & visual verification**, and list remaining manual/real-file checks honestly.

### Target A: Improved Sides Mod 1.0 (`SWBF2 Improved Sides mod 1.0`)

Source archive: `E:\Projects\SWBF2_Improved_Sides_mod_1.0.zip` (read-only; SHA-256 verified unchanged). Contains `Readme.txt`, 8 `side/*.lvl` files (`all`, `allshell`, `cis`, `cisshell`, `imp`, `impshell`, `rep`, `repshell`), and 86 FPM LVLs across faction directories, with no `addme.script` or manifest.

- **Discovery & target resolution:** Automatically discovered without `addme.script`. Resolved all 94 LVLs cleanly with **0 ambiguous** entries: **14 active** replacements (4 playable sides, 4 shell sides, 6 modified FPM LVLs preserving faction-specific paths such as `fpm/imp/imptiftr.lvl` and `fpm/rep/repjedfi.lvl`) and **80 byte-identical** FPM LVLs skipped automatically.
- **Conversion:** Converted into `mod_cache/` producing **1,200+ native asset overrides** (class patches for `all`/`cis`/`imp`/`rep`, re-baked infantry/hero GLBs and cloth, 64 weapons, vehicles for all 4 sides, FPM arm assets, and textures at both tiers) plus removed companion markers.
- **Honest unsupported / skipped items:**
  - `side/*shell.lvl` (4 files): reported honestly as having no effect because OpenBF does not load menu shell side LVLs.
  - `all_inf_pilot`, `all_inf_hothpilot`, `all_inf_grenadier`: kept stock with explicit warning due to MTRL blend bits `0xc8` unsupported by the skinned BLEND pass.
  - `cis_inf_darthmaul`, `rep_inf_kiadimundi`: kept stock with explicit warning due to a malformed `"0.0."` float literal in the mod's cloth property `0xc1133964`.
  - `GeometryAddon` attachments (12 models), 3D starfighter cockpit FPM models (`imp_1st_cockpit_tiefighter`, `rep_1st_cockpit_jedistarfighter`), normal/bump maps (`*_bump`, `*_normal`), and custom `zaf_`/`zaa_` animation banks (`swbfiii`, `bfiii`, `magnaguard`, `gunner`) are reported explicitly under `unsupported`.
- **Runtime & gameplay validation:** Verified in headless battle runs and visual captures (see verification log in PR/session summary): replaced infantry/weapons/vehicles render, skinning and animations function, BF1 maps remain unaffected, and `--no-mods` / disabling restores stock assets cleanly.

### Target B: Brighter Endor Mini-Mod (`Brighter_Endor_Mini-Mod.zip` → loose `end1.lvl`)

Source archive: `E:\Projects\Brighter_Endor_Mini-Mod.zip` (`SHA256: eed63286cba226cc4d72e109280b41c4d9fc57a0179044ef580285926e3b28b0`, read-only). Contains only `Readme.txt` and a single loose `end1.lvl` (39,051,880 bytes) with no subfolders, manifests, or registration scripts.

- **Binary inspection:** Despite being described as a retexture mod, binary inspection revealed a **full replacement PC BF2 world LVL** with **100% resource key overlap** against stock `bf2 pc:end/end1.lvl`:
  - **33 changed `tex_` textures** (SWBF1 demo Endor textures: `end_main_1..6`, `end_canopy`, `end_prop_tree*`, `end_prop_fern`, `end_prop_bush_1`, `end_bldg_hut*`, `sky_endor`, `yav_prop_boulder`, etc.).
  - **Lighting, sky, terrain, models, classes & world layer changes:** modified `lght` (1), `sky_` (1), `tern` (`end1`), `modl` (3: `editor_grasspatch`, `end_bldg_ewok_walkway_16lowd`, `end_weap_treesmash`), `gmod` (`end_weap_treesmash`), `entc` (`end_weap_treesmash`, 1 property `0x0e8c3fe5`), and `wrld` (`end1_turrets`).
  - Unchanged: `coll` (33), `prim` (11), `plan` (1), `path` (2), `prp_` (1), `fx__` (3), `skel` (40).
- **Discovery & target resolution:** Automatically discovered from the loose `end1.lvl` with zero manual configuration; resolved unambiguously to `bf2 pc:end/end1.lvl` (100% overlap vs. 0% for `load/end1.lvl`, and game-filtered away from `bf1 pc:end/end1.lvl`), with capabilities `[WorldReplacement, Textures, Models, GameplayClasses]`.
- **Conversion:** Automatically routed through the full world replacement path (`bake::map::bake_map` with staged `SIDE/TUR.LVL`) in **0.97 s** against `dist/openbf.pak`, emitting **165 replacement entries** (156 written, 9 removed; 14,532 foliage instances, 85 path props, 5 particle effects; preserving stock `sounds/` and `load/`) with **0 errors and 0 unsupported items**.
- **Runtime & visual verification (`dist/openbf.exe`):**
  - Mounted simultaneously alongside **Improved Sides Mod 1.0**, **Abregado-Rae: Venezia**, and **Kothlis: Sea Haven** (`#1 Brighter Endor Mini-Mod`, `#2 SWBF2 Improved Sides mod 1.0`, `#3 ABR`, `#4 KTH`).
  - Headless Endor overview and third-person walk captures confirmed brighter daylight lighting/fog, lush green ferns/foliage, sunlit redwood trunks, mossy demo boulders, and Improved Sides Rebel Jungle Troopers rendering together cleanly (`316D9D55...` / `EA3899B3...` vs. stock `E839B360...` / `518891B4...`).
  - 15 s Endor Conquest (`bf2_end1_con.ron`) walk run confirmed 6 command posts, 12 vehicle pads, 6 Ewoks, 1 death region, and full bot combat functioning cleanly.
  - `--no-mods` restored stock Endor byte-for-byte; BF1 Endor (`bf1_end1`) rendered byte-for-byte identical to stock (`921F9DB3...`) with all four mods enabled.
  - Additionally validated the fast **texture-only world replacement** path (~0.4 s) against a texture-only modified loose `end1.lvl` (`gameplay: []`).

### Generic Infrastructure Checks

- Automated unit & integration tests cover: multiple folder layouts, loose LVL discovery, duplicate filenames across FPM directories, ambiguous target detection and persistent Mods UI ComboBox selection (`mod_targets.ron`), backup/alternative folder suppression, `Full` vs `Patch` replacement semantics, load-order conflict resolution (`resolve_content_mods`), BF1/BF2 overlay scoping and removed-file hiding, cache reuse/invalidation, ABR conversion, and pinned stock determinism (`--no-mods --desktop --sim-hash 100` = `71dabcfadf689bff`).
