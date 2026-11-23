# OpenBF VR (`openbf.vr`)

**OpenBF VR** is OpenBF's first bundled **engine extension**. Supported Windows and Linux builds include it out of the box, disabled by default, so players can toggle VR on or off from **`openbf-launcher`** without recompiling, editing configuration files, or passing CLI flags manually.

## Quick Start (Packaged & Launcher Workflow)

1. Connect your VR headset and start an OpenXR runtime that supports `XR_KHR_vulkan_enable2` and stereo views (for example, SteamVR, Virtual Desktop / Oculus OpenXR, Monado, or WiVRn).
2. Open **`openbf-launcher`**.
3. Open the **Mods** tab.
4. Toggle **OpenBF VR** on (this atomically updates `openbf_data/extensions.ron`).
5. Click **Play**.

To return to normal desktop play later, disable **OpenBF VR** in **Mods** and launch again.

> **Graphics API note:** OpenXR graphics interop requires Vulkan. If **Options -> Graphics API** in the launcher is set to Direct3D 12 while OpenBF VR is enabled, `openbf-launcher` automatically overrides it to Vulkan for VR sessions while keeping your saved desktop preference intact.

## CLI & Developer Launch Options

The underlying game executable (`openbf` / `game`) uses `openbf_data/extensions.ron` and accepts these overrides:

| Flag / Env | Effect |
|---|---|
| `--list-extensions` | Print discovered bundled extensions as RON (inspects platform/build support) |
| `--vr` | Force-enable `openbf.vr` for this launch without mutating `extensions.ron` |
| `--desktop` / `--no-vr` | Force the desktop display backend even if `openbf.vr` is enabled in `extensions.ron` |
| `--enable-ext <id>` / `--disable-ext <id>` | Override a specific extension for this run only |
| `--headless` / `--sim-hash` | Headless simulation/bench/replay runs always ignore graphical extensions |
| `OPENBF_XR_LOADER` | Absolute path to a custom OpenXR loader DLL / `.so` |
| `OPENBF_DATA_DIR` | Override the portable `openbf_data/` directory location |

Example local run from `engine/` (requires local `assets_baked/` and `missions/`):

```sh
cargo run --release --locked -p game -- --map assets_baked/tat2 --mode eli --vr
```

## Platform Support & Build Requirements

- **Windows:** Supported. Builds link the OpenXR loader statically (`openxr` crate), requiring CMake and a C++ toolchain at compile time. Pass `--no-vr` to `scripts/pack.py` (or `--no-default-features` to Cargo) only if building a desktop-only binary without a CMake toolchain.
- **Linux:** Supported. Uses the system OpenXR loader (`libopenxr_loader.so`).
- **macOS & other targets:** `openbf.vr` registers as an unsupported extension entry with an explanation in the **Mods** tab; desktop gameplay works normally.

## Startup Error Handling & Recovery

OpenBF VR negotiates the Vulkan graphics device with OpenXR before opening the main renderer window.

- **If OpenXR initialization fails** (for example, no headset/runtime is running, missing `XR_KHR_vulkan_enable2`, or bad `OPENBF_XR_LOADER`):
  - The game writes `openbf_data/last_startup_error.ron` and `openbf_data/logs/startup_error.log` cleanly without blocking headless CLI invocations with modal popups.
  - When launched via `openbf-launcher` (`OPENBF_LAUNCHED_BY_LAUNCHER=1`), the game automatically reopens `openbf-launcher`, which displays a **Startup Error Recovery Banner** offering **Play in Desktop Mode** (`--desktop` one-shot override), **Disable OpenBF VR**, or **Dismiss**.
  - When invoked directly from a terminal, `game` prints the error summary and suggests `--desktop` (`--no-vr`).
- Successful windowed startup clears `openbf_data/last_startup_error.ron`.

## VR Controls & Gameplay Features

All functionality from the experimental `codex/openxr-vr` branch is preserved:

| Input | Action |
|---|---|
| Headset | 6-DoF look and physical lean (with world geometry eye clipping) |
| Right controller | Menu cursor laser in UI panels; physical primary weapon aiming in battle |
| Right trigger | Click in menus / fire primary weapon in battle |
| Either thumbstick up/down in menus | Scroll lists and class/spawn pickers under the cursor |
| Left trigger | Hold to spawn held grenade in palm, swing and release to throw; fires secondary weapons otherwise |
| Left grip near a rifle/launcher fore-end | Two-handed weapon grip and stabilized aiming |
| Left thumbstick | Look-relative movement |
| Click left thumbstick (L3) | Sprint latch in movement direction |
| Right thumbstick left / right | 30-degree snap turn |
| Right thumbstick down | Reload |
| Right thumbstick click (or left menu) | Pause menu |
| Right A / X / Trigger confirm buttons | Jump in battle; confirm / back navigation in menus |
| Right B / Left Y | Switch primary / secondary weapon |
| Left X / Left A | Enter or leave vehicles / interact |
| **F10** in desktop window | Recenter headset origin and seated height |

Additional behaviors preserved:

- **Curved 16:9 Visor HUD:** Renders the native HUD across the helmet visor FOV while hiding the screen-center crosshair (`HudView::update_without_reticle`) and retaining enemy target health readouts.
- **Physical melee:** Tracked single- and dual-handed lightsaber poses drive spatial blade sweeps, block pulses, and per-hand OpenXR haptic feedback.
- **Tracked arms & weapons:** First-person forearms, hands, held weapons, and Super Battle Droid wrist blasters follow controller tracking; muzzle flashes and bolts spawn at the physical barrel/wrist.
- **Stereo multi-view & mirror:** Left/right eyes render via `render`'s asymmetric multi-view pipeline (`Renderer::begin_frame()` + `ViewProjection::Asymmetric(fov)`), with sun flare rendered accurately per eye and left eye mirrored to the desktop window during battle (flat menu panel mirrored when in menus).
- **Audio listener:** 3D sound positioning tracks the headset pose in battle.
- **Vehicles & multiplayer isolation:** Vehicles continue using desktop/gamepad controls cleanly; LAN multiplayer strips local tracked VR weapon/combat inputs before packet encoding and simulation submission so deterministic lockstep gameplay cannot desync.

## Comparison Against `codex/openxr-vr`

| Area | Experimental `codex/openxr-vr` | Bundled Extension (`openbf.vr`) |
|---|---|---|
| Crate ownership | Embedded in `game/src/vr/` behind `#[cfg(feature = "vr")]` and `--vr` | Isolated in `engine/crates/openbf_vr/`, registered via `extensions` |
| Renderer integration | VR-specific stereo path replacing multi-view helpers | Adapts `Renderer::begin_frame()` and `ViewProjection::Asymmetric(fov)` directly |
| Activation UX | Recompile / pass `--vr` via CLI | Disabled by default; runtime toggle in `openbf-launcher` **Mods** tab (`extensions.ron`) |
| Failure handling | Exits or blocks with message box | Writes `last_startup_error.ron` + logs; relaunches launcher with one-click desktop recovery |
| LAN isolation | VR disabled only via CLI habit | Tracked inputs stripped cleanly at `net::bundle::read_pad`, `host`/`client::submit_input`, and `netplay` frame calls |
| Gameplay parity | Initial VR locomotion, weapons, melee, grenades, visor HUD, audio listener | 100% preserved; no working VR feature removed |

## Manual Headset Verification Checklist

*(Automated tests verify compilation, extension resolution, atomic config persistence, deterministic `--sim-hash 100` parity, LAN tracked-input stripping, multi-view asymmetric rendering, flat-panel capture, visor GPU composition, and OpenXR failure recovery without hardware; physical headset validation requires an attached OpenXR runtime.)*

With an OpenXR headset runtime active:

1. Toggle **OpenBF VR** on in `openbf-launcher` **Mods** tab and click **Play** without any CLI flags.
2. Verify shell menus appear on the curved flat panel in headset and point/click cleanly with the right controller trigger.
3. Launch Ground Conquest and verify stereo eye rendering, curved visor HUD edges (no center crosshair, enemy health visible on aim), 6-DoF head tracking, F10 recenter, desktop mirror, and 3D audio orientation.
4. Test physical blaster aiming, two-handed rifle grip/release, L3 sprint latch, 30-degree snap turn, and Super Battle Droid wrist blasters.
5. Switch to a lightsaber hero (for example, Mos Eisley Hero Assault) and verify tracked blade swings, dual-saber left hand where applicable, block detection, and haptic feedback.
6. Test grenade spawn in left palm on left trigger hold, velocity throw on swing release, ammo spend on release, and closing cleanly back to desktop.
7. Toggle **OpenBF VR** off in **Mods**, launch again, and confirm standard desktop rendering resumes.
8. Close the OpenXR runtime before launch with **OpenBF VR** enabled to verify the launcher reopens with the Startup Error Recovery Banner and **Play in Desktop Mode** recovers immediately.
