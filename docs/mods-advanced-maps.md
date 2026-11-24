# Advanced legacy maps

Copy the original `KTH` folder into `openbf_data/mods/KTH` and enable **Kothlis: Sea Haven** on the launcher’s Mods tab. The launcher prepares the cache automatically. Instant Action lists **Cloudy**, **Dawn**, **Dusk**, **Night**, and **Rain** separately so players can choose the environment. The original Battlefront II presents one map entry and chooses the weather randomly before a single-player round. [FelipeGabe’s release and original-game screenshots](https://www.moddb.com/games/star-wars-battlefront-ii/addons/kothlis-sea-haven-20)

Each variant offers Conquest, two-flag Capture the Flag, and the authored sniper Hunt in both the Clone Wars and Galactic Civil War. Registrations guarded by checks for Dark Times, Battlefront Extreme, Beach Troopers, or Republic Commando stay out of the standalone menu. ABR can be enabled at the same time. The bundled VR extension keeps its own enabled state.

## How the addon is handled

- `mungefs::addon::parse_addme_script` follows Lua 5.0 conditional branches in the standalone environment. Every file check is evaluated inside the standalone model without querying other installed addons. `analyze_standalone_variants` enumerates a bounded integer choice during setup, reads embedded helper definitions from the addon, and records the ordered `ReadDataFile` selections. [Lua 5.0 instruction semantics](https://www.lua.org/source/5.0/lopcodes.h.html)
- The installed addon has one primary world archive and a companion archive with five named selectors. Each companion sublevel carries its own terrain, sky, and lighting. The primary archive contributes the selected mode, weather, and shared day/evening layers. The selected world uses the existing PC model, material, collision, terrain, navigation, and effect readers. The minimap sheet and five variant load screens come from the addon. Variant-exclusive assets keep their own manifests. Byte-identical files share storage where the cache filesystem supports hard links.
- The PC texture decoder accepts signed D3D9 `V8U8` bump data. The converter keeps signed U and V in the linear RG channels and computes a positive up component for the existing normal-water shader. [Microsoft’s D3D9 bump-map format reference](https://learn.microsoft.com/en-us/windows/win32/direct3d9/bump-map-pixel-formats)
- `mungefs::addon::world_events::compile` extracts fixed actions from reachable environment callbacks. Instant Action uses the existing world-animation player, which moves collision bodies and rendered instances together. The environment action list supports named station destruction, one-shot delays, route and barrier changes, object kills, and named effects at world objects or path points. No legacy script executes during play.
- The converter can bake one addon-owned sound archive per map group, including PC IMA stereo streams. Variants share that audio bake. The runtime resolves its local properties first and supplements them with the stock planet bank the mission names.

## Compatibility evidence

The original Battlefront II installation was verified by the user on November 24, 2026. OpenBF work and comparisons started from `915df64e9f93c171c1827e40f95dca6e3c045011`. The installed KTH folder stayed read-only. Its 22-file SHA-256 inventory matched after inspection and testing. All generated assets and captures stayed outside Git.

| Check | Result |
| --- | --- |
| Launcher discovery | The launcher’s scanner found KTH and ABR from one isolated `OPENBF_DATA_DIR`. KTH exposed five entries with six missions each. ABR retained its four missions. |
| Packaged launch | The game and converter were run from an isolated installation using an existing stock `openbf.pak`. All 30 KTH variant, era, and mode combinations built a battle with their stock classes. The headless sweep found six Conquest posts, four CTF posts, and two Hunt posts per world, plus two authored death regions. Separate launches with one mod disabled mounted and played the other. |
| Outcomes and navigation | A seeded Rain Conquest round ended on tickets at 98 s with a 20-ticket override. A seeded Dawn Hunt reached its 50-point target at 148 s. The Night CTF bot run stayed active through 180 s. The Dawn CTF referee carried the enemy flag into the authored capture volume five times and reached victory. |
| Environmental event | The Dawn Conquest referee destroys the installed map's named trigger. The command post is disabled and the delayed world pose changes. The normal battle loader supplies the map, station, post, and animation data. |
| Cache | The packaged launch prepared both mods. A warm launch reused both metadata files. Changing an isolated KTH source file rebuilt KTH and reused ABR. After restoring KTH, the packaged game rebuilt its cache and launched ABR. `fsutil hardlink list` confirmed one shared collision mesh across all five KTH variant directories. |
| Audio conversion | The packaged cache contains two KTH sample banks, four WAV files, and a shared PC IMA stream body with eleven indexed stereo segments. Playback and variant-specific music still need an interactive check. |
| Stock determinism | Baseline and candidate packaged binaries both returned `71dabcfadf689bff` for `--no-mods --desktop --sim-hash 100`. |
| VR | The game built with the bundled OpenXR extension. The saved-VR-profile test confirmed that headless and explicit desktop launches still work. Hardware play and stereo captures are pending. |
| PC water | The serial GPU referee verified normal-mapped refraction, reflection, and responsiveness to changing normal samples. |
| Visual comparison | Five headless captures show the separate sky, geometry, terrain, foliage, and weather choices. The daytime boardwalk, roofs, and palms correspond to the author's screenshots. The night overview remains darker than the published night boardwalk. Matched outdoor, interior, and water views still need a comparison in the original game. [Original-game screenshots](https://www.moddb.com/games/star-wars-battlefront-ii/addons/kothlis-sea-haven-20) |

### Reproduce the checks

Place the player's KTH and ABR copies in the isolated `OPENBF_DATA_DIR/mods` and prepare them through the launcher or a packaged launch. The two data-dependent test variables point to that data directory and a directory of baked stock class RONs.

```text
cargo test --manifest-path engine/Cargo.toml -p launcher --lib discovers_kothlis_and_abr_with_only_standalone_missions -- --ignored
cargo test --manifest-path engine/Cargo.toml -p game --bin game --features vr addon_destroyed_station_runs_authored_animation_and_timer_actions -- --ignored
cargo test --manifest-path engine/Cargo.toml -p game --bin game --features vr addon_two_flag_uses_authored_pickup_and_capture_volumes -- --ignored
openbf --desktop --map assets_baked/mod_kth__kth_rain --mission missions/mod_kth__kth_rain_con.ron --mode con --headless --seconds 180 --walk --ai 12 --tickets 20
openbf --no-mods --desktop --sim-hash 100
```

Set `OPENBF_KTH_TEST_DATA` to the isolated data directory and `OPENBF_KTH_TEST_CLASSES` to the stock class directory. The tests use locally sourced assets. Interactive mouse/keyboard, controller, and headset play still need a human check. Fish and bird flocks, movie previews, and general-purpose Lua timer or trigger APIs remain outside the translated environment action set.

The `mungefs` library suite ran 205 tests successfully, with three ignored data-dependent tests. The serial GPU check was `scripts/gpu_tests.sh --render --test suite water_normal_refracts_floor_and_reflects_above_plane`. The bake version lock check passed. The full cross-source bake and alignment gates still need their independent stock reference bakes.

## Integration with side conversion

The addon converter continues to use `classes.ron` and the shared `ModCacheMeta` schema. The converter version is four so caches built by the single-world importer are rebuilt. Side conversion can use the same classes writer and cache schema. A combined branch should coordinate the converter version if it changes emitted side assets as well.
