# Legacy Battlefront II Mod Folder Support & Load Order

OpenBF supports filesystem-based legacy **Star Wars Battlefront II (2005)** addons out of the box without any import button, import wizard, or manual mod registration.

---

## 1. Quick Start (`openbf_data/mods/`)

The filesystem is the single source of truth:

1. Open **`openbf-launcher`**, switch to the **Mods** tab, and click **Open Mods Folder** (or browse directly to `<data_dir>/mods/`).
2. Copy or extract each addon folder directly into `openbf_data/mods/` using classic BF2 layout:

   ```text
   openbf_data/
     mods/
       ABR/
         addme.script
         Readme.txt
         data/
           _LVL_PC/
             ABR/
               ABR.lvl
             Load/
               common.lvl
             core.lvl
             mission.lvl
     mod_cache/          # auto-generated native assets (safe to delete anytime)
     extensions.ron      # persistent enabled states + load order (schema v2)
   ```

3. Return to the launcher (or click **Refresh**): the launcher automatically re-scans `openbf_data/mods/` on startup and window focus regain.
4. Enable the mod, arrange its priority with **Up** / **Down**, wait for automatic background preparation into `openbf_data/mod_cache/`, and click **Play**.

Direct launches of `game` without opening the launcher first also read `openbf_data/extensions.ron` and `openbf_data/mods/`, preparing missing or stale caches automatically via `bake mod-convert`. Pass `--no-mods` to suppress all content mods for a launch.

---

## 2. Discovery, Identities & Safety

- **No untrusted code execution:** Discovery and mission inspection use static `ucfb` LVL scanning and a bounded static Lua 5.0 bytecode constant/opcode evaluator (`mungefs::lua50`)—arbitrary mod scripts or third-party native DLLs are never executed.
- **Stable non-colliding identities:** Each discovered folder gets an internal mod ID (`content.<slug>`) and isolated asset/mission namespace (`mod_<slug>`) derived from the installed folder identity rather than the legacy three-character addon tag alone. Multiple mods sharing the same legacy tag (e.g. `ABR_A` and `ABR_B`) remain distinct in the list, in `mod_cache/`, and in Instant Action with folder-name disambiguation.
- **Compatibility classification:** Each mod is marked **Supported**, **Partial (with warnings)**, **Unsupported**, **Disabled**, or **Invalid** (for example rejected symlink escapes or corrupt/empty folders) with expandable diagnostics inside the **Mods** tab.
- **Stock protection:** Unreferenced top-level LVLs placed inside an addon folder (for instance extra `kas2.lvl` / `tat2.lvl` copies in an addon pack) are detected, left unmounted with a warning, and never allowed to shadow stock maps accidentally.

---

## 3. Manual Load Order, Namespaces & Overrides

In the **Mods** tab, every content mod shows a **#1, #2, ...** priority badge with **Up** and **Down** controls:

- **Higher in the list = higher override priority (`#1` wins).**
- **Ordering persists atomically** in `openbf_data/extensions.ron` across launcher/game restarts, temporary disables, and folder removal/re-insertion (`ExtensionProfile::reconcile_order`).
- **Explicit namespacing:** Each mod's maps (`assets_baked/mod_<slug>__<map>`) and missions (`missions/mod_<slug>__<mission>.ron`) live inside its own namespace, so unrelated mods never overwrite each other just because internal filenames happen to match.
- **Intentional overrides only:** Shared stock resource overrides (`OverlayRule::Overrides` and prioritized localization `locl` tables) participate in load-order precedence. Overlapping declared overrides report diagnostic conflict notices in both the launcher **Mods** tab and runtime logs, showing which higher-priority mod won.
- **Dependency validation:** Declared dependencies and ordering conflicts are validated before activation (`extensions::mods::resolve_content_mods`), blocking invalid configurations with clear explanations.
- **Engine extensions stay separate:** Content-mod priority does not select exclusive display backends; engine extensions such as `openbf.vr` continue to resolve independently via capability/exclusivity rules in `ExtensionRegistry`.

---

## 4. Automatic Conversion Cache & Runtime Architecture

- **Versioned cache (`openbf_data/mod_cache/<namespace>/`):** Background conversion (`bake mod-convert`) emits native GLB geometry/collision, KTX2 textures, `manifest.ron`, `classes.ron`, `effects.ron`, `foliage.bin`, translated `missions/*.ron`, optional `locl.ron`, and `cache_meta.ron` (recording `schema_version`, `converter_version`, and SHA-256 source fingerprint).
- **Zero source mutation & incremental rebuilds:** Source addon files in `openbf_data/mods/` are strictly read-only. Re-launches reuse cached native assets when the fingerprint matches and automatically rebuild only the affected mod when its source files change or converter metadata is stale—never rebuilding stock `assets_baked/` or repacking `openbf.pak`.
- **Clean dependency boundary:** `game` never links `mungefs` in normal dependencies. `game::mods_runtime` loads enabled caches directly via `extensions` + `manifest` + `pak::overlay` (spawning external `bake mod-convert` only when a cache needs creation or refresh).
- **Checkout and packed compatibility:** `pak::overlay` layers namespaced mod directories (`NamespaceDir`), mission prefixes (`NamespacePrefix`), and priority-ordered overrides (`Overrides`) uniformly over both development checkouts (`assets_baked/`, `missions/`) and packed retail builds (`openbf.pak`).

---

## 5. Legacy BF2 Addon Compatibility Report

See the [advanced legacy map compatibility report](mods-advanced-maps.md) for Kothlis: Sea Haven 2.0, including the five Instant Action variants and the verified behavior of each mode.

### Verified Acceptance Target: Abregado-Rae: Venezia (`ABR`)

Tested headlessly end-to-end against the retail-modded `ABR` addon copied into an isolated `OPENBF_DATA_DIR` (original source folder verified unchanged via SHA-256 before and after every test):

- **Discovered metadata & safety warnings:** Identified title *Abregado-Rae: Venezia*, author *Bobfinkl*, description, localized strings (`core.lvl`), loading background (`Load/common.lvl`), stock planet sound references `["tat", "yav"]`, 4 playable Instant Action missions (`abrg_con`, `abrc_con`, `abrg_ctf`, `abrc_ctf`), plus warnings skipping unregistered scripts (`multiobjectivecontainer`, `abrg_eli`) and isolating 2 unreferenced LVLs (`kas2.lvl`, `tat2.lvl`).
- **Auto-conversion & caching:** Cold `game` launch auto-converted `ABR` via `bake mod-convert` into `mod_cache/mod_abr/` in ~3.7 s; unchanged re-launches reused `cache_meta.ron` (`META_UNCHANGED=True`); source file edits automatically triggered cache invalidation and rebuild (`META_REBUILT=True`).
- **Scene & rendering verification:** Headless overview, `--scene-report`, and `--walk-replay --hud` captures verified twilight sky lighting, canals/water, terrain (`mungefs::terrain::pc`), Venetian architecture, collision bodies, Stormtrooper units, minimap icons, HUD counters, and `0 untextured on the disc` / `0 terrain chunks dropped` (note: 448 far-LOD `*LOWD` low-res material draws fall back cleanly when `*_lowres`/`*_lowrez` companion sheets are absent).
- **Conquest (`abrg_con` / `abrc_con`):** 6 command posts and full team rosters spawn, navigate, fight, neutralize/capture posts, and complete Conquest to victory (`outcome Some(1)` reached at 73 s with 25 tickets and 16 AI).
- **Two-Flag CTF (`abrg_ctf` / `abrc_ctf`):** 4 spawn-only CTF posts (`hide_cps: true`), both flag capture zones, and both rigid `com_icon_republic_flag` / `com_icon_cis_flag` flagpoles standing inside team capture rings (`abr_ctf_flag1_cam.png`, `abr_ctf_flag2_cam.png`) verified; bots actively navigate (`CtfOffense`/`CtfDefense`) and pick up enemy flags (`flag_picked_up` recorded at tick 865). Note: dynamic BF2 `CLTH` flag banner cloth simulation remains deferred engine-wide under `CLOTH2`, so ground flags display the rigid flagpole, glowing capture ring, minimap markers, and HUD/world indicators.
- **Packed runtime, `--no-mods`, and duplicate legacy IDs:** Verified running directly against `dist/openbf.pak` (with zero on-disk `assets_baked/` or `missions/` folders), verified `--no-mods` cleanly hides mod content, and verified copying `ABR` into two separate folders (`ABR_A` and `ABR_B`) produces distinct namespaces (`mod_abr_a`, `mod_abr_b`) mounted in configured priority order (`#1 ABR_B`, `#2 ABR_A`).

### General Compatibility Bounds

- **Supported out of the box:** Conventional PC `_LVL_PC` addons referencing stock BF2 sides/classes/weapons/vehicles/planet sound banks, standard `addme.script` + `mission.lvl` mission scripts (`con`, `1flag`, `ctf`, `ass`, `hunt`, `eli`), PC `tern` terrain, PC DXT1/DXT3/DXT5/A8R8G8B8 textures, static collision meshes, world instances/regions/layers, `plan`/`plnp` nav graphs, and `core.lvl` localized text tables.
- **Not supported / skipped with diagnostics:** Arbitrary runtime Lua mission logic outside supported declarative tables, custom third-party native DLL plugins, custom non-stock side LVL unit/hero authoring lacking stock assets, and BF2 `CLTH` cloth simulation.

### Recommended Manual Interactive Checks

Because the verification above ran headlessly on Windows with simulated inputs, a human playtest is still recommended for:

1. Launching `openbf-launcher` interactively and clicking **Open Mods Folder**, verifying OS window-focus rescan latency with a folder drop/removal.
2. Playing a full interactive mouse-and-keyboard Conquest and 2-Flag CTF round on **Abregado-Rae: Venezia** from the in-game Instant Action menu.
