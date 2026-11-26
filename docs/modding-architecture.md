# OpenBF Modding & Extension Technical Architecture

This document is the consolidated technical reference for OpenBF's content-modding pipeline, virtual filesystem (VFS) overlay stack, conversion caches, and bundled engine extension system.

- User guide: [docs/mods.md](mods.md)
- Advanced multi-variant addon maps: [docs/mods-advanced-maps.md](mods-advanced-maps.md)
- Generic legacy LVL resource replacements: [docs/mods-resource-replacements.md](mods-resource-replacements.md)
- Engine extensions & VR: [docs/extensions.md](extensions.md), [docs/vr.md](vr.md)

---

## 1. Crate Boundaries & Dependency Flow

A core invariant of OpenBF is that **the runtime binary (`game`) never links `mungefs` in normal dependencies** (`mungefs` is dev-only in `game`). All binary LVL and Lua 5.0 bytecode parsing stays in `mungefs`, `bake`, and `launcher`:

```text
openbf_data/mods/<folder>/          (read-only user mod files)
        │
        ▼
  mungefs::modscan                  (static ucfb LVL inventory + bounded Lua 5.0 evaluator)
  extensions::mods::replace         (evidence-based matching against stock catalogs)
        │
        ▼
  bake mod-convert                  (external process: LVL/Lua -> native GLB/KTX2/OGG/RON)
        │
        ▼
openbf_data/mod_cache/<namespace>/  (versioned native assets + cache_meta.ron)
        │
        ▼
  game::mods_runtime                (reads cache_meta.ron, validates fingerprint & versions)
  pak::overlay                      (mounts NamespaceDir, NamespacePrefix, scoped Overrides)
        │
        ▼
  game / core / render / ui / audio (consumes native assets transparently via pak::*)
```

| Crate | Modding & Extension Responsibility |
|---|---|
| `mungefs` | `lua50` (bounded static Lua 5.0 bytecode parser/evaluator), `addon` (legacy addon folder inspector, mission folder, world-event compiler), `inventory` (chunk/property digests), `modscan` (unified folder scanner + `FactsCache`). |
| `extensions` | `ExtensionManifest`, `ExtensionProfile` (`extensions.ron` v2), `ExtensionRegistry`, `StartupErrorReport`, `mods` (`ModCacheMeta`, identity/slugging, `FileHashCache`), `mods::catalog` (`StockCatalogs`, `StockSourceList`), `mods::replace` (`ResourceOp`, target resolution, `TargetSelections`), `mods::resolve` (load-order validation, conflict diagnostics, multiplayer digest). Optional `runtime` feature defines `EngineExtension`, `EarlyDisplayContext`, and `ActiveDisplayExtension`. |
| `bake` | `stock` (reads stock assets from disk or `openbf.pak`, generates `assets_baked/catalog/`), `mods::convert` (atomic staging + swap into `mod_cache/`), `mods::world` (multi-sublevel world assembly + hardlink deduplication), `mods::mission` (native mission RON writer), `mods::replace` (side, FPM, world, and loadscreen replacement conversion). |
| `pak` | Read-only stock archive (`openbf.pak` / appended exe) or checkout directory mounts plus `pak::overlay` (`NamespaceDir`, `NamespacePrefix`, game-scoped `Overrides`, `Resolved::Removed` hiding). |
| `launcher` | `openbf-launcher` GUI: native setup pipeline (including stock catalog generation), background `ModsWorker` (lazy metadata listing, deep inspection, `bake mod-convert` queue), **Mods** tab UI (priority `#1, #2, ...`, target disambiguation ComboBoxes, engine extension toggles), and startup error recovery banner. |
| `game` | `mods_runtime` (discovers enabled caches, spawns `bake mod-convert` when missing/stale, mounts overlays & localization, injects Instant Action maps), `classdb` (applies `<classes>.patch.ron` sidecars), `main::extensions_host` (resolves bundled extensions, runs `early_init_display`, writes `last_startup_error.ron`), `main::windowed::xr` (routes frame lifecycle to `ActiveDisplayExtension`). |
| `openbf_vr` | Bundled `openbf.vr` OpenXR display/input/audio/haptics extension implementing `EngineExtension` + `ActiveDisplayExtension`. |

---

## 2. How Users Install and Manage Mods

1. **Filesystem as source of truth:** Players copy extracted mod folders into `openbf_data/mods/` (accessible via **Open Mods Folder** in `openbf-launcher` -> **Mods**). Original files in `openbf_data/mods/` are strictly read-only.
2. **Automatic discovery:** `openbf-launcher` scans `openbf_data/mods/` on startup, on window focus regain, and on **Refresh**. Small uncached mods (up to 8 MiB of candidate replacement LVLs) are deep-inspected inline; larger uncached mods appear immediately via metadata-only discovery and deep-inspect on demand when enabled or expanded.
3. **Stable identity:** Each folder receives a deterministic ID (`content.<slug>`) and isolated asset/mission namespace (`mod_<slug>`) derived from the folder name (or optional `openbf_mod.ron` `id`), with a short SHA-256 suffix on slug collisions. Two folders shipping the same legacy 3-character addon tag remain distinct everywhere.
4. **Enable & order:** Players toggle mods on/off and set override priority (`#1` = highest priority) with **Up** / **Down**. Enabled state and order persist atomically in `openbf_data/extensions.ron` (schema v2) across restarts, temporary disables, and folder removal/re-insertion.
5. **Ambiguous replacement targets:** Targets come only from the Battlefront II stock catalog. A replacement LVL that fits no single stock file clearly (for example, a filename shared across faction directories with equal content) is listed as an unclear target and stays stock. A target can be named in `openbf_data/mod_targets.ron` or the mod's `openbf_mod.ron`.
6. **Direct launches & suppression:** Launching `openbf` directly without opening the launcher first still reads `openbf_data/extensions.ron` and prepares missing/stale caches via `bake mod-convert` (if `bake` is reachable). Passing `--no-mods` suppresses all content mods for that run.

---

## 3. Legacy Addon Maps vs. Resource Replacement Mods

OpenBF describes every mod by composable `Capability` flags rather than rigid folder rules, supporting both classic addons and stock replacements (as well as hybrid packs containing both):

| Aspect | Legacy Addon Map | Legacy LVL Resource Replacement |
|---|---|---|
| Typical layout | `addme.script` + `mission.lvl` + `data/_LVL_PC/<tag>/...` | `_LVL_PC/...`, `side/*.lvl`, `fpm/...`, or loose `<map>.lvl` |
| Registration | Static Lua 5.0 evaluation of `addme.script` + `mission.lvl` | Evidence-based matching against `assets_baked/catalog/` |
| Asset namespace | Isolated `assets_baked/mod_<slug>__<map>/` and `missions/mod_<slug>__*.ron` | Game-scoped overrides under `overrides/assets/` |
| Stock interaction | Adds new Instant Action maps/missions; unreferenced stock-named LVLs are isolated | Replaces stock side/FPM/world/loadscreen outputs for the target game (`bf1` or `bf2`) |
| Coexistence | Multiple addon maps never collide on internal filenames | Higher load-order rank (`#1`) wins per overridden file; conflicts are reported |

---

## 4. Resource Resolution, Namespaces & Override Rules

Every runtime asset read in `game` goes through `pak`. `pak::overlay::resolve_file(mount_name, rel_key)` resolves paths in strict order:

1. **Private namespaces first (`NamespaceDir` and `NamespacePrefix`):**
   - Addon map directories (`assets_baked/mod_<slug>__<map>/...`), private custom side directories (`assets_baked/mod_<slug>_side_<side>/...`), and mission files (`missions/mod_<slug>__*.ron`) live inside their own prefixes.
   - Because namespaces are checked before overrides and carry unique per-mod prefixes, unrelated addon maps cannot shadow each other or leak private assets into stock maps.
2. **Game-scoped explicit overrides (`OverlayRule::Overrides`), in load-order priority (`#1` first):**
   - Only relative paths explicitly listed in `allowed_paths` are eligible.
   - Stock replacement layers carry `scope: Some("bf2")` or `scope: Some("bf1")`. Before battle prefetch and scene load, `game::assets::scope_overlays_to_map` calls `pak::set_overlay_scope(Some(<source_game>))`, ensuring BF2 replacements never affect BF1 maps (even where BF1 borrows BF2 weapon kits).
   - **Full vs. Patch mode & removed files:** In `ReplacementMode::Full` (default), stock companion files no longer produced by a re-baked replacement (such as stock cloth on a clothless soldier model) are recorded with `ReplacementOutput { removed: true }`. When `allowed_paths` contains a path absent on disk under `overrides/assets/`, `pak::overlay` returns `Resolved::Removed`, hiding the stock file from `pak::read`, `pak::is_file`, `pak::open`, and `pak::read_dir`. In `ReplacementMode::Patch`, only changed/added outputs are overlaid.
   - **Class patches:** Side replacement class edits are emitted as `<classes>.patch.ron` sidecars (`manifest::class_patch`) containing only properties whose digest differs from the stock PC catalog. `game::classdb` reads the in-scope override stack (`pak::read_override_stack`) and applies `manifest::class_patch::apply_stack` over the stock PS2-canonical `classes/<g>_<side>.ron`: a `Full` replacement forms a coherent base that shadows lower-priority patches to the same side, while partial `Patch` layers above it compose from lowest to highest priority so `#1` wins on conflicts and independent class changes compose cleanly.
3. **Stock fallback:**
   - Any path not claimed by an overlay falls through transparently to the immutable stock mount (`assets_baked/` and `missions/` on disk in a checkout, or `openbf.pak` / appended executable in a packed build).
4. **Localization (`locl`):**
   - Addon `core.lvl` string tables are converted to `locl/english.ron` and stacked in load-order priority via `game::event_feed::set_mod_locls`.

---

## 5. How LVL Files & Compiled Lua Become Native Resources

**No legacy script or third-party native DLL is ever executed.**

1. **Static Lua 5.0 translation (`mungefs::lua50`, `mungefs::addon`, `bake::mods::mission`):**
   - `mungefs::lua50` parses compiled Lua 5.0 chunks and runs a bounded static opcode evaluator (`evaluate_standalone_registration`, `analyze_standalone_variants`) in a standalone environment (external mod checks evaluate false so cross-mod registrations stay out of standalone Instant Action).
   - `mungefs::addon::mission_fold` folds team rosters, eras, modes (`con`, `1flag`, `ctf`, `ass`, `hunt`, `eli`), heroes, spawns, audio planet references, and multi-variant world selections (retaining single-player weights and the fixed multiplayer branch).
   - `mungefs::addon::world_events::compile` extracts a closed set of deterministic world actions (named station destruction, one-shot timers, world animation triggers, route/barrier toggles, object kills, named effects) from reachable callbacks.
   - `bake::mods::mission` emits native `missions/mod_<slug>__*.ron` files.
2. **Addon world & audio baking (`bake::mods::convert`, `bake::mods::world`):**
   - Primary and companion world LVLs are assembled per variant and baked through `bake::map` into GLB meshes/terrain/water/lowres, KTX2 textures (including signed D3D9 `V8U8` normal maps for PC water), `manifest.ron`, `classes.ron`, `effects.ron`, `foliage.bin`, and `world_anims.ron`. Byte-identical files across variants share storage via hard links where supported.
   - Optional addon load screens (`bake::loadscreen`), `core.lvl` localization (`bake::hud::bake_locl`), and one addon sound archive per map group (`bake::sounds::bake_addon_sounds`, inheriting stock CORE templates for spatial/bus defaults) are baked into the same cache.
3. **Stock LVL replacement baking (`bake::mods::replace`, `bake::stock`):**
   - During stock asset preparation (`scripts/rebake.sh catalog` or `launcher::pipeline::rebake::bake_stock_catalog`), `bake stock-catalog` writes metadata-only catalogs (`assets_baked/catalog/<game>_<platform>.ron` and `stock_bake.ron`) containing chunk/property digests and output mappings (no copyrighted assets).
   - `bake::mods::replace` diffs each active replacement LVL against its stock catalog entry and re-bakes only what changed:
     - **SIDE (`side/*.lvl`):** Class patch sidecar, changed soldiers/weapons/vehicles (borrowing stock animation banks verified by SHA-256), side effects, and changed textures at both texture tiers.
     - **First-person (`fpm/...`):** Re-baked first-person arm GLBs with the stock FPM animation bank and changed textures.
     - **World (`<planet>/<map>.lvl`):** Texture-only fast path when only `tex_` chunks differ; full `bake::map::bake_map` path (with staged `SIDE/TUR.LVL` and PC `normal_terrain_shader.xml` directional lighting) when geometry, terrain, lighting, sky, water, collision, or classes change, while keeping stock `sounds/` and `load/` intact.
     - **Load screens (`load/<map>.lvl`):** Re-baked via `bake::loadscreen`.
   - Anything unsupported is recorded explicitly in `ModCacheMeta::unsupported` and `warnings`.

---

## 6. Conversion Caches & Invalidation

### Converted Mod Cache (`openbf_data/mod_cache/<namespace>/`)

- Built into a sibling `.staging.<pid>` directory and committed via directory rename swap (`swap_dir`) so interrupted conversions never corrupt an existing cache.
- Governed by `cache_meta.ron` (`ModCacheMeta`), validated on every launcher scan and game startup (`ModCacheMeta::validate`):
  - `schema_version == MOD_CACHE_SCHEMA_VERSION` (currently `1`)
  - `converter == MOD_CONVERTER_ID` (a hash of the converter's own sources, made by `engine/crates/extensions/build.rs`)
  - `source_fingerprint` (SHA-256 over relative paths, sizes, and file SHA-256s of the mod folder)
  - `replacement_key` (derived from active `ResourceOp`s; validated via `catalog_digest` and `target_choices` when replacements are present)
  - Presence and path-safety checks on every expected `manifest.ron`, mission RON, `locl/english.ron`, and non-removed `replacement_outputs` file on disk.
- Safe to delete `openbf_data/mod_cache/` at any time; enabled mods regenerate automatically.

### Launcher & Scanner Acceleration Caches (`openbf_data/cache/`)

- `file_hashes.ron` (`extensions::mods::FileHashCache`): caches per-file SHA-256 keyed by `(path, len, mtime_ns)`.
- `lvl_facts.ron` (`mungefs::modscan::FactsCache`): caches `LvlFacts` per LVL file keyed by `(path, len, mtime_ns)`.
- `mod_index.ron` (`launcher::mods_worker::ModIndexCache`): caches `DiscoveredMod` keyed by `(tree_stamp, catalog_digest, target_choices)`.

---

## 7. Interaction Between Engine Extensions & Content Mods

- **Shared persistence, independent resolution:** Both bundled engine extensions (`openbf.vr`) and content mods (`content.<slug>`) store enabled booleans in `openbf_data/extensions.ron` (schema v2). `ExtensionRegistry::resolve` ignores `content.*` IDs and resolves bundled extensions via capability/exclusivity rules and topological dependency sort; `extensions::mods::resolve_content_mods` resolves content mods by user load order (`profile.order`).
- **Display backend isolation:** Content mod ordering never affects display backend selection. `openbf.vr` claims `ExclusiveCapability::DisplayBackend` and initializes OpenXR before window/renderer creation.
- **VR + content mods coexistence:** `openbf_vr` renders through `render`'s asymmetric multi-view pipeline and `game`'s normal asset loader (`pak::*`), so addon maps, replaced sides/weapons/worlds, and custom localization are visible in VR automatically. When OpenXR startup fails, `game` writes `openbf_data/last_startup_error.ron` and reopens `openbf-launcher` for one-click Desktop recovery without touching content mod settings.
- **Determinism & LAN multiplayer:**
  - `core` remains deterministic (`core::fmath`, seeded `SimRng`, fixed 30 Hz ticks).
  - `net` strips local VR tracked inputs (`strip_tracked_inputs`) before packet encoding and simulation submission.
  - `ContentManifest` carries `maps` (intersected across peers), `active_mods`, and `override_digest` (which excludes purely cosmetic replacements). At round load, `Sim::static_digest` enforces simulation equality across peers and benches mismatched guests cleanly back to the lobby.
  - Multi-variant addon maps select their scripted `multiplayer_variant` in LAN sessions instead of rolling a single-player random weather variant.

---

## 8. Extending the System (Developers & Coding Agents)

- **Preserve crate boundaries:** Never add `mungefs` as a normal dependency of `game` or `extensions`. Keep runtime loading strictly over `extensions` + `manifest` + `pak`.
- **No converter version to bump:** `MOD_CONVERTER_ID` is a hash of every non-test source file in the crates `bake` links (`bake`, `audio`, `extensions`, `mungefs`, `manifest` with its `data/`, `pak`), computed at build time by `engine/crates/extensions/build.rs`. Any change to the converter makes existing caches read as stale and rebuild; the launcher and game pick the `bake` binary whose `bake mod-converter-id` matches their own.
- **Adding a new stock replacement target kind:**
  1. Ensure stock catalog generation (`bake::stock`, `scripts/rebake.sh catalog`, `launcher::pipeline::rebake::bake_stock_catalog`) indexes the target LVL and its outputs.
  2. Add classification and conversion in `bake/src/mods/replace.rs`, emitting `ReplacementOutput` entries (including `removed: true` for removed companions in `Full` mode) and honest `unsupported` diagnostics for unhandled chunks.
  3. Update `Capability` classification in `extensions/src/mods/replace.rs` if needed.
- **Adding a new bundled engine extension:** Follow the four-step checklist in [docs/extensions.md](extensions.md#adding-another-bundled-extension) and keep all extension state outside `core::sim` hashes and LAN packets.
- **Keep third-party mod names out of repository docs:** Document supported capabilities, formats, and test procedures generically without naming third-party community mod projects.

---

## 9. Known Compatibility Limitations

- **Unsupported legacy mod features (reported honestly in diagnostics):**
  - Arbitrary runtime Lua mission logic outside supported declarative tables and closed world-event actions (e.g., ambient fish/bird flocks, custom Lua timers/triggers, movie previews).
  - Third-party native DLL plugins.
  - Full custom non-stock side LVL model/texture/animation authoring inside addon maps (only custom side `classes.ron` is currently baked for addon-private sides).
  - Dynamic BF2 `CLTH` cloth simulation (deferred engine-wide under `CLOTH2`; rigid flagpoles/worn cloth render without physics).
  - Menu shell side LVLs (`side/*shell.lvl`), PC 1.1 `GeometryAddon` attachments, 3D starfighter cockpit FPM models, normal/bump maps on non-water surfaces, custom `zaf_`/`zaa_` animation banks, and unsupported skinned MTRL blend modes.
- **Multiple full side replacements targeting the same stock side:** Two `Full` side LVL replacements targeting the same stock side are treated as coherent replacement sets where `#1` shadows `#2` rather than mixing incompatible models/skeletons/classes; partial `Patch` layers above a full replacement or each other compose per-class with `#1` winning on conflicts.
- **Standalone converter availability:** `scripts/pack.py` builds and installs `openbf-launcher` and `bake` alongside `openbf` (+ `openbf.pak`) so standalone distributions can discover and auto-convert mods without development tools on `PATH`. Direct launches of `openbf` also locate `bake` via `OPENBF_BAKE_BIN` or `openbf_data/tools/bake`.
