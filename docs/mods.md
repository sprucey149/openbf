# Legacy Battlefront II Mod Folder Support & Load Order

OpenBF supports filesystem-based legacy **Star Wars Battlefront II (2005)** addons and resource replacement mods out of the box without any import button, import wizard, or manual mod registration.

For full technical details on crate boundaries, VFS resolution, conversion pipelines, and cache invalidation, see [docs/modding-architecture.md](modding-architecture.md).

---

## 1. Quick Start (`openbf_data/mods/`)

The filesystem is the single source of truth:

1. Open **`openbf-launcher`**, switch to the **Mods** tab, and click **Open Mods Folder** (or browse directly to `<data_dir>/mods/`).
2. Copy or extract each mod folder directly into `openbf_data/mods/`. OpenBF automatically discovers classic BF2 addon maps (`addme.script` + `mission.lvl`), conventional `side/` and `fpm/` replacement packs, `_LVL_PC/` trees, and single loose `.lvl` replacement files without requiring manual folder reorganization or manifests:

   ```text
   openbf_data/
     mods/
       MyAddonMap/                     # classic addon map (addme.script + mission.lvl)
         addme.script
         data/_LVL_PC/MAP/MAP.lvl
       MySideReplacementPack/          # side + FPM replacement pack (no addme.script)
         side/all.lvl
         fpm/...
       MyWorldRetex/                   # single loose replacement LVL
         end1.lvl
     mod_cache/          # auto-generated native assets (safe to delete anytime)
     mod_targets.ron     # optional hand-written targets for ambiguous LVLs
     stock_sources.ron   # stock copy paths synced from launcher.ron
     extensions.ron      # persistent enabled states + load order (schema v2)
   ```

3. Return to the launcher (or click **Refresh**): the launcher automatically re-scans `openbf_data/mods/` on startup and window focus regain.
4. Enable the mod, arrange its priority with **Up** / **Down**, wait for automatic background preparation into `openbf_data/mod_cache/`, and click **Play**.

Direct launches of `openbf` without opening the launcher first also read `openbf_data/extensions.ron` and `openbf_data/mods/`, preparing missing or stale caches automatically via `bake mod-convert` when `bake` is available. Pass `--no-mods` to suppress all content mods for a launch.

---

## 2. Discovery, Identities & Safety

- **No untrusted code execution:** Discovery and mission inspection use static `ucfb` LVL scanning and a bounded static Lua 5.0 bytecode constant/opcode evaluator (`mungefs::lua50`)—arbitrary mod scripts or third-party native DLLs are never executed.
- **Stable non-colliding identities:** Each discovered folder gets an internal mod ID (`content.<slug>`) and isolated asset/mission namespace (`mod_<slug>`) derived from the installed folder identity rather than the legacy three-character addon tag alone. Multiple mods sharing the same legacy tag (for example, `MAP_A` and `MAP_B`) remain distinct in the list, in `mod_cache/`, and in Instant Action with folder-name disambiguation.
- **Map folders:** With **Map folders** on in the launcher's options, Instant Action lists its maps in folders: Battlefront 1, Battlefront 2, then one per mod that adds a map, named as the mod. Map names drop the `(bf1)`, `(bf2)` and `(mod)` suffixes. **All Maps** inside a folder means that folder's maps. The setting reaches the game through `OPENBF_GROUP_MAPS`, and the multiplayer lobby keeps the flat list.
- **Game-folder downloads:** A mod laid out as the game folder it is copied over (`GameData/addon/<ID>/...`, with or without the `GameData` folder) is one entry in the **Mods** tab. Each addon inside it is read on its own, and the folder's `data/_LVL_PC` files are replacement candidates.
- **Map names:** A map is named as the game names it: `mapname.name.` plus the longest start of the mission script's name that the addon's `core.lvl` has a string for (`rxc1_con`: `rxc1_`, `rxc1`, `rxc`, ...).
- **Compatibility classification:** Each mod is marked **Supported**, **Supported (warnings)**, **Unsupported**, or **Invalid** (for example rejected symlink escapes or corrupt/empty folders) with expandable diagnostics inside the **Mods** tab.
- **Stock protection:** Unreferenced top-level LVLs placed inside an addon folder (for instance stray copies of stock map LVLs bundled inside an addon) are detected, left unmounted with a warning, and never allowed to shadow stock maps accidentally.

---

## 3. Manual Load Order, Namespaces & Overrides

In the **Mods** tab, every content mod shows a **#1, #2, ...** priority badge with **Up** and **Down** controls:

- **Higher in the list = higher override priority (`#1` wins).**
- **Ordering persists atomically** in `openbf_data/extensions.ron` across launcher/game restarts, temporary disables, and folder removal/re-insertion (`ExtensionProfile::reconcile_order`).
- **Explicit namespacing:** Each addon's maps (`assets_baked/mod_<slug>__<map>`) and missions (`missions/mod_<slug>__<mission>.ron`) live inside its own namespace, so unrelated mods never overwrite each other just because internal filenames happen to match.
- **Scoped overrides only:** Stock resource replacements (`OverlayRule::Overrides`) are scoped to their target game (`bf1` or `bf2`) via `pak::set_overlay_scope`, so BF2-only replacements never affect BF1 maps or units. Overlapping overrides report diagnostic conflict notices in both the launcher **Mods** tab and runtime logs, showing which higher-priority mod won.
- **Dependency validation:** Declared dependencies and ordering conflicts (via optional `openbf_mod.ron`) are validated before activation (`extensions::mods::resolve_content_mods`), blocking invalid configurations with clear explanations.
- **Engine extensions stay separate:** Content-mod priority does not select exclusive display backends; engine extensions such as `openbf.vr` continue to resolve independently via capability/exclusivity rules in `ExtensionRegistry`.

---

## 4. Automatic Conversion Cache & Runtime Architecture

- **Versioned cache (`openbf_data/mod_cache/<namespace>/`):** Background conversion (`bake mod-convert`) emits native GLB geometry/collision, KTX2 textures, `manifest.ron`, `classes.ron`, `effects.ron`, `foliage.bin`, translated `missions/*.ron`, optional `locl.ron`, replacement overrides under `overrides/assets/`, and `cache_meta.ron` (recording `schema_version`, the `converter` id, SHA-256 source fingerprint, replacement key, catalog digest, and user target choices).
- **Zero source mutation & incremental rebuilds:** Source mod files in `openbf_data/mods/` are strictly read-only. Re-launches reuse cached native assets when metadata matches and automatically rebuild only the affected mod when its source files, target selections, stock catalog, or converter version change—never rebuilding stock `assets_baked/` or repacking `openbf.pak`.
- **Clean dependency boundary:** `game` never links `mungefs` in normal dependencies. `game::mods_runtime` loads enabled caches directly via `extensions` + `manifest` + `pak::overlay` (spawning external `bake mod-convert` only when a cache needs creation or refresh).
- **Checkout and packed compatibility:** `pak::overlay` layers namespaced mod directories (`NamespaceDir`), mission prefixes (`NamespacePrefix`), and game-scoped priority-ordered overrides (`Overrides`) uniformly over both development checkouts (`assets_baked/`, `missions/`) and packed retail builds (`openbf.pak`).

---

## 5. Compatibility Overview

- See [generic legacy LVL resource replacements](mods-resource-replacements.md) for stock resource catalogs, evidence-based target resolution, game-scoped replacement semantics, and verification of side/FPM packs and loose world LVL replacements.
- See [advanced legacy maps](mods-advanced-maps.md) for multi-variant weather/time-of-day addon maps, companion world archives, PC normal-mapped water, static world-event compilation, and addon sound banks.
- See [modding architecture](modding-architecture.md) for the full technical design and known limitations.

### Supported Out of the Box

- Conventional PC `_LVL_PC` addon maps referencing stock BF2 sides, classes, weapons, vehicles, and planet sound banks.
- Standard `addme.script` + `mission.lvl` mission scripts (`con`, `1flag`, `ctf`, `ass`, `hunt`, `eli`), including multi-variant single-player world selection with fixed multiplayer branches.
- PC `tern` terrain (with PC directional lighting), PC DXT1/DXT3/DXT5/A8R8G8B8 textures, PC `V8U8` normal-mapped water, PC bump and glossmap specular materials (`MTRL` 0x20 / 0x08, specular cube from a PC BF2 `core.lvl`), static collision meshes, world instances/regions/layers, `plan`/`plnp` nav graphs, `core.lvl` localized text tables, addon load screens, and addon sound banks with stock CORE template inheritance.
- Soldiers and weapons from an addon map's own side LVLs, with their classes and textures, including a hero a mission script picks while it runs for a third team.
- Generic stock LVL replacements for playable `side/*.lvl` files (class patches, soldiers, weapons, vehicles, side effects, textures), `fpm/...` first-person arm LVLs, stock world LVLs (both texture-only fast path and full world re-bake), and `load/<map>.lvl` load screens.

### Not Supported / Skipped with Diagnostics

- Missions on the game's own maps. A mission that reads a stock world (`cor\cor1.lvl`) and adds the mod's layers on top is reported and not listed as a map.
- Custom eras and custom game modes. A new map's missions in a mode openbf does not know are reported as unsupported.
- Arbitrary runtime Lua mission logic outside supported declarative tables and closed world-event actions.
- Custom third-party native DLL plugins.
- Vehicles, side effects, unit sounds and voice lines, first-person arms and custom animation banks from an addon map's own side LVLs.
- Dynamic BF2 `CLTH` cloth simulation (deferred engine-wide under `CLOTH2`; rigid flagpoles and worn cloth render without cloth physics).
- Menu shell side LVLs (`side/*shell.lvl`), PC 1.1 `GeometryAddon` attachments, 3D starfighter cockpit FPM models, normal maps on materials without the PC bump flag, custom `zaf_`/`zaa_` animation banks, and unsupported skinned MTRL blend modes.
