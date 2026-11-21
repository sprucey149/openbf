# Branding: the application icon

The application icon is project artwork, not game data: it is not from a disc, it is
small, and every file in `branding/` is regenerable, so it is tracked in git. One
master, `branding/openbf.png` (750x750 RGBA), and the containers each platform reads.

```bash
python3 scripts/make_icons.py          # write every asset from branding/openbf.png
python3 scripts/make_icons.py --check  # fail when a committed asset does not draw what the master draws
```

`scripts/make_icons.py` resamples with Lanczos and writes:

| path | what it is |
|---|---|
| `branding/windows/openbf.ico` | 16, 24, 32, 48, 64, 128, 256 px, PNG-compressed entries |
| `branding/macos/openbf.iconset/` | `icon_16x16.png` … `icon_512x512@2x.png`, `iconutil`'s names |
| `branding/macos/openbf.icns` | the same container, written as bytes (`icp4`…`ic14`) |
| `branding/linux/hicolor/<size>/apps/openbf.png` | the icon-theme sizes, 16 to 512 px |
| `branding/linux/openbf.desktop.template` | the application entry, with `@EXE@` to fill in |

## How the icon reaches each platform

- **Windows.** Windows shows the icon stored in the `.exe`. `engine/crates/game/build.rs`
  runs `build_icon.rs`, which finds `rc.exe` (through `WindowsSdkDir`, then
  `C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64`, then `PATH`), compiles
  `branding/windows/openbf.ico` into `openbf.res` in `OUT_DIR`, and passes it to
  `link.exe` with `cargo:rustc-link-arg-bin=game=…`. `scripts/pack.py` copies the
  linked binary, so `dist/openbf.exe` carries the icon with no extra step, and a box
  without the SDK gets a `cargo:warning=` and the generic icon.
- **Linux.** An executable carries no icon, so it comes from the icon theme and the
  application entry. `scripts/pack.py` writes `dist/openbf.desktop` beside the game,
  and `scripts/install_linux.sh` installs it to `~/.local/share/applications/openbf.desktop`
  and the PNGs to `~/.local/share/icons/hicolor/*/apps/openbf.png`, refreshing the
  desktop and icon caches when the tools exist. The entry file is named
  `openbf.desktop` and `StartupWMClass=openbf`, which is what a Wayland session
  matches an app id against.
- **macOS.** `scripts/pack.py --app-bundle` (default on macOS) writes
  `dist/openbf.app` with `Info.plist` naming `CFBundleExecutable=openbf`,
  `CFBundleIconFile=openbf.icns` and `CFBundlePackageType=APPL`, the game in
  `Contents/MacOS/`, `openbf.pak` and `openbf_data/` beside it in `Contents/MacOS/`
  (where `pak::find_archive` and `game::profile::data_dir` look), and
  `openbf.icns` in `Contents/Resources/`. Nobody has run a macOS build yet (see
  `scripts/pack.py`'s header): the layout is pinned by
  `scripts/tests/test_pack_limits.py`, and `branding/macos/openbf.icns` is generated
  as bytes here because macOS 10.7+ reads PNG in every entry type. On macOS,
  `iconutil -c icns branding/macos/openbf.iconset -o branding/macos/openbf.icns` is
  the Apple-native path from the committed `.iconset`.

## Notes

- The master is 750 px, so the 1024 px entry (`icon_512x512@2x.png`, `ic10`) is a
  Lanczos upscale. Replace the master with a 1024 px drawing when the art is redrawn.
- `--check` compares what each asset draws (pixels, and the set of entries in each
  container), not bytes, so a different Pillow version is not reported as stale.
  `scripts/tests/test_make_icons.py` runs this in
  `python3 -m unittest discover -s scripts/tests`.
- The window itself is given no icon in code: `window_attributes` in
  `engine/crates/game/src/main/windowed.rs` sets only the title, and the OS shows the
  bundle, `.desktop` and `.exe` icons.
- `branding/` holds no game content, so it does not fall under the "no game data in
  git" rule in `AGENTS.md`.
