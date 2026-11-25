# OpenBF Engine Extension Architecture

OpenBF supports modular **engine extensions** cleanly separated from the core simulation and desktop presentation path. The first bundled extension is **OpenBF VR** (`openbf.vr`), which lives in [`engine/crates/openbf_vr/`](../engine/crates/openbf_vr/) and activates at runtime without rebuilding the executable or altering vanilla desktop behavior.

## Goals and Boundaries

- **Runtime activation without recompiling:** Bundled extensions are compiled into supported release builds and enabled or disabled at runtime in `openbf_data/extensions.ron` (typically toggled in `openbf-launcher` under **Mods**).
- **Zero intrusion on desktop & headless paths:** When an extension is disabled—or suppressed by `--desktop`, `--no-vr`, `--headless`, `--sim-hash`, or LAN multiplayer—desktop rendering, simulation determinism, and multiplayer lockstep streams behave identically to vanilla OpenBF.
- **Deterministic simulation authority:** Only `core` owns weapon rules, projectile simulation, cooldowns, ammo, collision, and damage. Extensions observe read-only `SimView` snapshots and feed quantized deterministic inputs (`PadState::tracked_shots`, `PadState::tracked_combat`, `Steer::Tracked`) through engine interfaces.
- **Filesystem content mods live alongside engine extensions in `extensions.ron` (schema v2):** Content mods discovered from `openbf_data/mods/` share persistent enabled toggles in `openbf_data/extensions.ron`, while maintaining strict separation of concerns:
  - Content mods (`content.<slug>`) carry an explicit user-managed `order` (`#1` first = higher override priority), isolated map/mission namespaces, game-scoped stock resource replacements (`pak::set_overlay_scope`), and versioned native asset caches in `openbf_data/mod_cache/` (see [docs/mods.md](mods.md) and [docs/modding-architecture.md](modding-architecture.md)).
  - Engine extensions (`openbf.vr`) remain bundled, capability-resolved modules whose display backend selection never depends on arbitrary content-mod load order. Arbitrary third-party native DLL plugins remain unsupported.

## Crate Architecture

### `extensions` ([`engine/crates/extensions/`](../engine/crates/extensions/))

Shared metadata, persistence, dependency resolution, and runtime contracts:

- **Default build** (used by both `openbf-launcher` and `game`; no GPU or simulation dependencies):
  - `ExtensionManifest` (`id`, `name`, `version`, `description`, `kind`, `origin`, `capabilities`, `exclusive_capabilities`, `dependencies`, `conflicts`, `supported`, `unsupported_reason`)
  - `ExtensionKind` (`EngineExtension`, `GameplayMod`, `ContentPack`)
  - `ExtensionOrigin` (`Bundled`, `ThirdParty`)
  - `Capability` (`DisplayBackend`, `SpatialInput`, `TrackedWeapons`, `StereoPresentation`, `CustomHud`, `AudioListener`, `Haptics`) and `ExclusiveCapability::DisplayBackend`
  - `ExtensionProfile` (`openbf_data/extensions.ron`, atomic temporary-file replacement; defaults disabled unless explicitly enabled)
  - `StartupOverrides` (`--vr`, `--desktop`/`--no-vr`, `--enable-ext`, `--disable-ext`)
  - `StartupErrorReport` (`openbf_data/last_startup_error.ron`)
  - `ExtensionRegistry::resolve` returning a deterministic topologically sorted `ActivationPlan` (`DisplayBackendSelection::Desktop` or `DisplayBackendSelection::Extension(id)`) or `Vec<ResolutionError>` on unknown IDs, unsupported extensions, missing dependencies, explicit conflicts, exclusive-capability collisions, or dependency cycles
- **`runtime` feature** (used by `game` and compiled extensions such as `openbf_vr`):
  - `EngineExtension`: provides `manifest()` and `early_init_display()`
  - `EarlyDisplayContext`: holds pre-window/runtime state before `Renderer` creation and implements `open_renderer(window)`
  - `ActiveDisplayExtension`: per-frame lifecycle (`begin_frame`, `transform_pad`, `apply_sprint_latch`, `on_sim_ticks_consumed`, `on_sim_events`, `stereo_view`, `draw_stereo_eyes`, `finish_stereo`, `finish_flat`, `menu_buttons`, `recenter`, `shutdown`)
  - Shared value types (`SimView`, `FrameContext`, `FrameDirectives`, `StereoView`, `TrackedGun`, `EyeClipQuery`, `ListenerPose`, `MenuCursorFrame`, `MenuButtons`)

### Reusable Engine Capabilities

General-purpose capabilities extracted into base crates (inactive unless an extension requests them):

- **`core` & `input`**: Quantized spatial inputs (`TrackedShot`, `TrackedHand`, `TrackedThrow`, `TrackedCombat`), `Steer::Tracked`, per-hand saber sweeps and cooldowns, velocity-based grenade throwing (`Thrown::throw_tracked`), custom muzzle rays, and `Event::SaberHandContact`.
- **`net`**: `strip_tracked_inputs` strips local spatial VR fields on `read_pad`, `host::HostCore::submit_input`, `client::ClientCore::submit_input`, and netplay frame submission so LAN lockstep wire traffic and simulations remain unaffected.
- **`render`**: `Renderer::new_windowed_with_device` / `finish_windowed` for external graphics device/instance creation (e.g., OpenXR Vulkan interop), multi-view `begin_frame()` / `Frame::render_to(..., ViewProjection::Asymmetric(fov), ...)` / `Frame::finish()`, curved stereo `Visor` pass (`visor.rs`), `capture_flat_frame` / `flat_frame` / `mirror_frame`, and `WorldScale::blit`.
- **`ui`**: Virtual controller pointer forwarding in `EguiOverlay` and reticle-free HUD feed helpers (`hud::feed_with_reticle`, `HudView::update_without_reticle`) for stereo rendering where world aiming uses physical weapon sights instead of a screen-center crosshair.
- **`game`**: First-person tracked hand/forearm/weapon transforms and grenade preview rendering (`soldier_vis/tracked.rs`), `EyeClipQuery` on `WalkSim`, audio listener override support, and the extension host in [`engine/crates/game/src/main/extensions_host.rs`](../engine/crates/game/src/main/extensions_host.rs) and [`engine/crates/game/src/main/windowed/xr.rs`](../engine/crates/game/src/main/windowed/xr.rs).

## Startup Lifecycle & Failure Recovery

Windowed startup follows this order before any game graphics device is created:

1. Parse CLI overrides (`--vr`, `--desktop`, `--no-vr`, `--enable-ext <id>`, `--disable-ext <id>`).
2. Resolve the portable data directory (`OPENBF_DATA_DIR`, `<checkout>/dist/openbf_data`, or `<exe_dir>/openbf_data`) and load `openbf_data/extensions.ron`.
3. Register compiled-in bundled extensions (`bundled_extensions()`), plus platform/build availability manifests, and call `ExtensionRegistry::resolve(...)` to validate dependencies, conflicts, and exclusive capabilities.
4. Determine the selected `DisplayBackendSelection`.
5. If an extension owns `ExclusiveCapability::DisplayBackend`, run `early_init_display()` **before** creating the desktop window or wgpu `Renderer`.
6. Open the window and initialize `Renderer` via `EarlyDisplayContext::open_renderer(window)` (or desktop `Renderer::new_windowed(window)` when desktop backend is selected).
7. On successful window/renderer startup, delete any stale `openbf_data/last_startup_error.ron`. Run game shell, loading, and battle frames through `ActiveDisplayExtension` hooks, and call `ext.shutdown()` during `windowed/close.rs` before dropping GPU resources.
8. **If early initialization or stereo runtime creation fails:**
   - Write a human-readable `StartupErrorReport` to `openbf_data/last_startup_error.ron` and append details to `openbf_data/logs/startup_error.log`.
   - Print diagnostics to `stderr` suggesting `--desktop` (never block headless CLI runs with modal OS dialogs).
   - When `OPENBF_LAUNCHED_BY_LAUNCHER=1` is set (launched from `openbf-launcher`), respawn `openbf-launcher` detached so the user sees a **Startup Error Recovery Banner** offering **Play in Desktop Mode**, **Disable OpenBF VR**, or **Dismiss**.

## Exclusive Capabilities & Ordering

Exclusive features such as display ownership never rely on fragile mod-list ordering. Instead:

- Extensions declare `exclusive_capabilities` (for example, `ExclusiveCapability::DisplayBackend`).
- If two enabled extensions claim the same exclusive capability or declare mutual `conflicts`, `ExtensionRegistry::resolve` rejects startup cleanly with a structured error.
- Activation order among compatible extensions is computed deterministically via dependency-ordered topological sort rather than arbitrary load order edits.

## Adding Another Bundled Extension

1. Create a crate under `engine/crates/<name>/` depending on `extensions = { path = "../extensions", features = ["runtime"] }` (and only the specific workspace crates needed).
2. Define a canonical `ExtensionManifest` in `extensions/src/lib.rs` (marked `ExtensionOrigin::Bundled`) and register your `EngineExtension` implementation in `game/src/main/extensions_host.rs::bundled_extensions()`.
3. Implement `early_init_display()` / `ActiveDisplayExtension` (or extend `extensions::runtime` with narrow trait hooks if a new extension category—such as a custom camera or input driver—needs additional points).
4. Expose only deterministic quantized inputs to `core::sim`; keep extension state outside simulation hashes and network bundles.
