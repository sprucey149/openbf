# Advanced Legacy Addon Maps

OpenBF supports multi-variant legacy Battlefront II (2005) addon maps that select among multiple weather or time-of-day world archives, author custom sound banks and lighting, or wire simple environmental world events in compiled Lua.

Copy an addon folder into `openbf_data/mods/` and enable it in the launcher's **Mods** tab. When a map defines multiple weather or time-of-day variants, Instant Action lists the map once and selects a variant according to the mission's single-player weights; LAN hosts use the fixed multiplayer variant scripted for the chosen mode and era.

---

## How Multi-Variant Addons Are Handled

- **Static Lua 5.0 branch analysis (`mungefs::lua50`, `mungefs::addon`):** `mungefs::addon::parse_addme_script` evaluates Lua 5.0 conditional branches in a standalone environment (without querying other installed addons, so registrations guarded by third-party mod checks stay out of standalone Instant Action). `analyze_standalone_variants` enumerates bounded integer choices during setup, reads embedded helper definitions, and records ordered `ReadDataFile` selections along with single-player weights and the multiplayer branch.
- **Primary + companion world assembly (`bake::mods::world`):** Addons that split per-variant terrain, sky, and lighting into a companion archive alongside a shared primary world archive are assembled per variant into isolated map directories (`assets_baked/mod_<slug>__<variant>/`). Each variant keeps its own manifest, minimap, and load screen, while byte-identical files across variants share storage via hard links where supported by the cache filesystem.
- **PC normal-mapped water (`mungefs::texture_pc`):** The PC texture decoder accepts signed D3D9 `V8U8` bump data, storing signed U/V in linear RG channels and reconstructing a positive up component for the normal-water shader.
- **Static world events (`mungefs::addon::world_events::compile`):** Extracts a closed set of deterministic environment actions from reachable mission callbacks (named station destruction, one-shot delays, world animations moving collision and rendered instances together, route/barrier toggles, object kills, and named effects at world objects or path points). No legacy Lua executes at runtime.
- **Addon sound banks & CORE template inheritance (`bake::sounds::bake_addon_sounds`):** Bakes one addon-owned sound archive per map group (including PC IMA stereo streams), shared across variants. Runtime audio resolves local sound properties first, inherits missing spatial/bus/rolloff defaults from the packaged stock CORE sound bank when an addon references a stock template, and supplements with every stock planet bank the mission's sound reads name, so music taken from a second planet's bank plays even when the addon has its own sound archive. Regional stereo ambience fades between authored inner and outer radii.
- **PC first-person weapon presentation:** When built with a local PC Battlefront II source, PC first-person weapon meshes and textures are packaged under `side_bf2/pc_first_person` and used in PC addon rounds under Classic Plus, while stock maps and strict tier keep their regular side assets.

---

## Verification Summary

Verified against a multi-variant PC BF2 addon map (5 weather/time-of-day variants, 6 era/mode combinations per variant, custom sound archive, authored `EntityLight` beams/flares, normal-mapped water, and a destructible-station world event) in an isolated `OPENBF_DATA_DIR`, keeping the original addon folder read-only (SHA-256 inventory verified unchanged before and after):

| Area | Verification Status |
|---|---|
| Launcher discovery & coexistence | **Verified (automated + headless):** Discovered simultaneously alongside a single-world addon map in one `OPENBF_DATA_DIR`; exposed as a single Instant Action row covering all 6 era/mode combinations while filtering out cross-mod-guarded registrations. |
| Packaged launch & all variants | **Verified (headless against `openbf.pak`):** All 30 variant/era/mode combinations built and loaded with stock classes, expected command posts, and authored death regions; enabling/disabling individual mods works cleanly. |
| Outcomes & two-flag CTF | **Verified (headless):** Seeded Conquest, Hunt, and two-flag CTF runs completed to victory or stayed active through full runs with authored pickup/capture volumes. |
| World events | **Verified (automated):** Destroying the authored trigger object disabled its command post and ran the delayed world animation. |
| Cache reuse & hardlinks | **Verified (packaged):** Warm launches reused cache metadata; editing one source file rebuilt only that mod's cache; hardlinks confirmed shared identical collision meshes across variants. |
| Audio conversion & template inheritance | **Verified (automated + muted Windows device test):** Decoded stereo IMA streams and sample banks; inherited stock CORE spatial/rolloff/bus templates so distant loops attenuate to silence beyond their cutoff radius. Relative PC loudness against a retail recording remains pending. |
| PC water shader | **Verified (serial GPU test):** `scripts/gpu_tests.sh --render --test suite water_normal_refracts_floor_and_reflects_above_plane` passed. |
| Stock determinism | **Verified:** Baseline and candidate packaged binaries matched `--no-mods --desktop --sim-hash 100` (`71dabcfadf689bff`) and 15-second stock Geonosis Conquest traces. |
| Interactive play, live LAN, and VR hardware | **Pending manual verification:** Interactive keyboard/mouse, controller, live LAN audio/weather sync, and headset play remain pending human playtests. |

### Known Limitations

- Ambient fish/bird flocks, movie previews, and general-purpose Lua timer or trigger APIs outside the closed world-event action set are not translated.
- Only one custom sound archive per map group is converted; additional custom sound archives emit a warning.
- Original PC audio output gain still needs a matched retail recording for final relative loudness calibration.

---

## Integration Notes

- Addon conversion shares `classes.ron` and `ModCacheMeta` (`MOD_CONVERTER_ID`) with generic stock resource replacement conversion.
- Global side replacements that supply alternate PC first-person weapon artwork override `side_bf2/pc_first_person` paths cleanly; private addon side namespaces remain isolated.
