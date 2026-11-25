# Legacy LVL Resource Replacement Mods

OpenBF supports generic legacy Star Wars Battlefront II (2005) LVL replacement mods without mandatory directory names, registration scripts (`addme.script`), or user-authored manifests.

Players copy extracted mod folders directly into `openbf_data/mods/`—whether the mod uses an original `_LVL_PC/` tree, conventional `side/` and `fpm/` folders, or a single loose `.lvl` file—and enable or reorder them in the launcher's **Mods** tab.

---

## 1. Architecture Overview

### 1.1 Discovery & Capabilities (`mungefs::modscan`, `extensions::mods::replace`)

- **Layout-agnostic scanning:** `mungefs::modscan::inspect_mod_folder` scans every `.lvl` file inside a mod folder alongside any addon registrations (`addme.script` / `mission.lvl`). Folder names act as resolution hints rather than prerequisites.
- **Content capabilities:** Mods are described by composable capabilities (`Capability`: `AddonMissions`, `NewWorld`, `WorldReplacement`, `SideReplacement`, `ShellSideReplacement`, `FirstPerson`, `Textures`, `Models`, `Animation`, `Audio`, `ShellHud`, `Localization`, `ClassPresentation`, `GameplayClasses`, `Collision`, `Effects`, `Missions`, `Other`) instead of mutually exclusive categories.
- **Read-only source safety:** Original mod files and stock game archives (`openbf.pak`, discs, PC installs) are never modified. All scan caches (`cache/file_hashes.ron`, `cache/lvl_facts.ron`, `cache/mod_index.ron`), user target choices (`mod_targets.ron`), stock source references (`stock_sources.ron`), and converted native assets (`mod_cache/<namespace>/`) live in `openbf_data/`.

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
6. Internal `(tag, name)` resource overlap with a minimum overlap and lead threshold for filename-only matches (allowing a loose world LVL such as `end1.lvl` to resolve unambiguously to `bf2 pc:end/end1.lvl` while refusing to guess between ambiguous shared filenames).

Ambiguous LVLs surface a target selector (`ComboBox` of ranked candidates plus **Ignore**) directly in the launcher's **Mods** tab; selections persist in `openbf_data/mod_targets.ron` outside the mod folder and trigger automatic rescan and cache invalidation.

### 1.4 Scoped Overlay & Replacement Semantics (`pak::overlay`, `game::mods_runtime`, `manifest::class_patch`)

- **Priority order:** Higher in the launcher list (`#1`) wins. `extensions::mods::resolve_content_mods` detects override conflicts across both `declared_overrides` and active stock replacement targets.
- **Game scoping:** Replacement outputs are mounted in per-game `OverlayRule::Overrides` layers (`scope: Some("bf2")` or `Some("bf1")`). At battle load (`game::assets::scope_overlays_to_map`), `pak::set_overlay_scope` activates only the current map's game scope so BF2-only replacements never affect BF1 maps or units.
- **Full vs Patch mode & removed files:** In `ReplacementMode::Full` (default, matching original LVL replacement semantics), stock companion files no longer produced by a re-baked soldier/vehicle/world (e.g., stock cloth on a model that removed cloth) and removed stock classes are hidden via `ReplacementOutput { removed: true }` (`pak::overlay::Resolved::Removed`) and `ClassPatch::Remove`. In `ReplacementMode::Patch`, only changed/added resources are overlaid without removing stock companions or classes.
- **Class patching:** Side class changes are written as `<classes>.patch.ron` sidecars (`manifest::class_patch`) containing only properties whose digest differs from the stock PC catalog. `game::classdb` applies these patches over the stock PS2-canonical `classes/<g>_<side>.ron`, preserving untouched retail gameplay values.
- **Multiplayer compatibility:** `game::mods_runtime` includes `(mod_id, fp:replacement_key)` in `content_mod_multiplayer_digest` whenever a mod has active gameplay-changing replacements (conservatively treating any property outside `stock_bake.ron`'s presentation set as gameplay). Purely cosmetic replacements (e.g., texture-only world overrides or cosmetic-only side skins) stay out of the gameplay digest.

### 1.5 PC LVL Conversion (`bake::mods::replace`)

- **SIDE (`side/<side>.lvl`):** Class patches (`classes/<g>_<side>.patch.ron`), re-baked soldiers (`bake_soldier_anims` with sibling mod/stock sides and verified animation banks), weapons (`bake_weapon_with`, hiding stock `pc_first_person/weapons/<m>.glb` overrides when replaced), vehicles (`bake_vehicles`), side effects (`bake_side`), standalone changed textures (`bake_textures_flagged`), and tier-2 mirroring (`both_tiers`).
- **First-person (`fpm/...`):** Re-baked first-person arm GLBs (`bake_fpm_with_bank`) with the stock first-person animation bank, plus standalone changed FPM textures.
- **World (`<planet>/<map>.lvl`):**
  - *Texture-only fast path:* When only `tex_` chunks differ, re-encodes only the changed world textures (and `_dtl` companions) into `<map_dir>/textures/` at both texture tiers, matching each stock sheet's alpha state while preserving stock geometry, terrain heights, collision, navigation, and missions verbatim.
  - *Full world path:* When geometry, terrain, world layout, lighting, sky, water, collision, or classes change, stages the LVL alongside `SIDE/TUR.LVL` (mod or stock) and runs `bake::map::bake_map`, mirroring tier-2 textures while keeping stock `sounds/` and `load/` intact. For PC BF2 terrains (`mungefs::terrain::Terrain::pc_patches`), `bake::terrain_glb` and `bake::foliage` apply PC BF2's `core.lvl -> SHDR -> normal_terrain_shader.xml` non-`Static` directional lights with hue-preserving normalization on top of the patch `VBUF` static vertex colours so dynamic-sun PC maps match original PC brightness while leaving PS2 and `Static`-sun terrains untouched.
- **Load screens (`load/<map>.lvl`):** Re-baked via `bake_loadscreen`.
- **Honest unsupported reporting:** Unsupported resources (`GeometryAddon` PC 1.1 attachments, 3D starfighter cockpit models, normal/bump maps, shell-only side LVLs that OpenBF does not load, custom animation banks, unsupported MTRL blend modes, or malformed cloth literals) are explicitly recorded in `ModCacheMeta::unsupported` and `warnings` and displayed in the launcher rather than silently dropped.

---

## 2. Shared Interfaces & Integration Notes

- `extensions::mods::ModCacheMeta` (schema v1, converter v9): extended with `resource_ops`, `replacement_key`, `replacement_outputs`, `unsupported`, `catalog_digest`, and `target_choices` (`#[serde(default)]` for backwards-compatible deserialization).
- `pak::OverlayRule::Overrides`: carries `scope: Option<String>` and treats allowed paths absent on disk as `Resolved::Removed` (hiding stock files without mutating `openbf.pak`).
- `game::assets::scope_overlays_to_map`: invoked before battle prefetch in both `windowed/loading.rs` and `windowed/events.rs`, and at the start of `load_scene`.
- `bake::soldier::set_lender`: allows in-process configuration of borrowed animation LVL paths without mutating process environment variables.
- Compatible with legacy addon map conversion, `side_bf2/pc_first_person` weapon handling, and the bundled `openbf.vr` extension.

---

## 3. Validation Summary by Replacement Archetype

### Archetype A: Multi-Side + FPM Replacement Pack

Validated against a large legacy BF2 side and first-person replacement pack (8 `side/*.lvl` files + 86 faction `fpm/` LVLs, no `addme.script` or manifest; source archive verified read-only):

- **Discovery & target resolution:** Automatically resolved all 94 LVLs with 0 ambiguous entries (14 active replacements including 4 playable sides, 4 shell sides, and 6 modified FPM LVLs preserving faction-specific paths; 80 byte-identical FPM LVLs skipped automatically).
- **Conversion:** Produced 1,200+ native asset overrides (class patches for all four sides, re-baked infantry/hero GLBs and cloth, 64 weapons, vehicles, FPM arm assets, and textures at both tiers) plus removed companion markers.
- **Honest unsupported / skipped diagnostics:** Shell side LVLs (`side/*shell.lvl`), models with unsupported skinned MTRL blend bits or malformed cloth float literals, `GeometryAddon` attachments, 3D cockpit FPM models, normal/bump maps, and custom `zaf_`/`zaa_` animation banks were cleanly kept stock or reported under `unsupported` and `warnings`.
- **Runtime & visual verification:** Headless battle runs and visual captures confirmed replaced infantry, weapons, and vehicles render and animate cleanly, BF1 maps remain unaffected, and `--no-mods` / disabling restores stock assets.

### Archetype B: Loose Single-File World Replacement LVL

Validated against a loose single-file PC BF2 world replacement (`end1.lvl` with no subfolders, manifests, or registration scripts; source verified read-only):

- **Discovery & target resolution:** Automatically resolved unambiguously to `bf2 pc:end/end1.lvl` via 100% resource overlap and game filtering (disambiguated from `load/end1.lvl` and `bf1 pc:end/end1.lvl`).
- **Full world & texture-only paths:** Full world replacement (`bake::map::bake_map` with staged `SIDE/TUR.LVL`) converted in ~1 s against `openbf.pak` with 0 errors and 0 unsupported items while preserving stock `sounds/` and `load/`. A synthetic texture-only loose world LVL also validated the ~0.4 s texture-only fast path with empty gameplay delta.
- **Coexistence & scoping:** Mounted simultaneously alongside a multi-side replacement pack and two legacy addon maps in packaged `openbf.exe`; headless overview and walk captures confirmed combined world + side replacements on BF2, byte-identical stock rendering on BF1, and clean restoration under `--no-mods`.

### Generic Infrastructure Checks

- Automated unit & integration tests cover: multiple folder layouts, loose LVL discovery, duplicate filenames across FPM directories, ambiguous target detection and persistent Mods UI ComboBox selection (`mod_targets.ron`), backup/alternative folder suppression, `Full` vs `Patch` replacement semantics, load-order conflict resolution (`resolve_content_mods`), BF1/BF2 overlay scoping and removed-file hiding, cache reuse/invalidation, and pinned stock determinism (`--no-mods --desktop --sim-hash 100` = `71dabcfadf689bff`).
