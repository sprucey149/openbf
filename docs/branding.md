# Branding: the application icon

The application icon is the classic Battlefront II clone trooper's helmet, drawn from
your own copy of the games when you pack. No icon artwork is kept in the repository:
like every other picture the game shows, it exists only after `scripts/pack.py` has
baked your copy (PC, Xbox or PS2). The only committed branding file is the Linux
application entry's text template, `branding/linux/openbf.desktop.template`.

## How it is made

`scripts/pack.py`, after the bake and before it packs the game:

1. runs `game --render-icon engine/branding_baked/openbf.png --assets engine/assets_baked`.
   `engine/crates/game/src/icon.rs` loads the first trooper body the bake wrote:
   BF2's `side_bf2/soldiers/rep_inf_ep3trooper.glb` (the Republic rifleman), then
   BF1's `side_bf1/soldiers/rep_inf_trooper.glb`, then BF2's `rep_inf_trooper.glb`.
   It draws the helmet alone (every bone but the head folds away) in its bind pose,
   from three-quarters in front, with a narrow lens, a key light, a cool rim light and
   soft ambient light. It draws it headless at 2048 px, twice, over black and over
   white, and takes the transparency from the difference (no render pass writes alpha).
   It then filters the result down to 1024 px and centres the helmet in the square.
2. runs `python3 scripts/make_icons.py --out engine/branding_baked`, which writes each
   platform's container beside the master:

| path under `engine/branding_baked/` | what it is |
|---|---|
| `openbf.png` | the 1024 px master, RGBA |
| `windows/openbf.ico` | 16, 24, 32, 48, 64, 128, 256 px, PNG-compressed entries |
| `macos/openbf.iconset/` | `icon_16x16.png` … `icon_512x512@2x.png`, `iconutil`'s names |
| `macos/openbf.icns` | the same container, written as bytes (`icp4`…`ic14`) |
| `linux/hicolor/<size>/apps/openbf.png` | the icon-theme sizes, 16 to 512 px |

`engine/branding_baked/` is git-ignored, like `engine/assets_baked/`, because it is a
picture of the games' own content.

If the render fails (no trooper in the bake, no GPU adapter, an old `--game-bin`),
pack.py prints one `note:` line and packs the game without a new icon. It never fails
the pack over the icon. A previous render's containers are kept.

## How the icon reaches each platform

- **Windows.** Windows shows the icon stored in the `.exe`. pack.py runs cargo with
  `OPENBF_ICON` set to `engine/branding_baked/windows/openbf.ico`.
  `engine/crates/game/build.rs` and `engine/crates/launcher/build.rs` run the same
  `build_icon.rs`, which finds `rc.exe` (through
  `WindowsSdkDir`, then `C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64`,
  then `PATH`; `windres` on a MinGW target), compiles the `.ico` into `openbf.res` in
  `OUT_DIR`, and passes it to `link.exe` with `cargo:rustc-link-arg-bin=<bin>=…`
  (`game` or `openbf-launcher`). On the first pack the `.ico` does not exist yet when
  they are first built, so pack.py builds both again after rendering it. The build
  scripts watch the file, so that second build relinks `game.exe` and
  `openbf-launcher.exe` with the icon, and later packs do nothing extra unless the icon
  changes. `dist/openbf.exe` and `dist/openbf-launcher.exe` are copies of those
  binaries.
- **Linux.** An executable carries no icon, so the icon comes from the icon theme and
  the application entry. pack.py writes `dist/openbf.desktop` beside the game from the
  template. `scripts/install_linux.sh` installs it to
  `~/.local/share/applications/openbf.desktop` and copies the rendered PNGs to
  `~/.local/share/icons/hicolor/*/apps/openbf.png`, then refreshes the desktop and icon
  caches when those tools exist. With no render it installs the entry alone. The entry
  file is named `openbf.desktop` and `StartupWMClass=openbf`, which is what a Wayland
  session matches an app id against.
- **macOS.** `scripts/pack.py --app-bundle` (the default on macOS) writes
  `dist/openbf.app`. Its `Info.plist` names `CFBundleExecutable=openbf`,
  `CFBundleIconFile=openbf.icns` and `CFBundlePackageType=APPL`. The game,
  `openbf.pak` and `openbf_data/` go in `Contents/MacOS/`, because that is where
  `pak::find_archive` and `game::profile::data_dir` look: beside the running
  executable. The rendered `openbf.icns` goes in `Contents/Resources/`. Packing again
  keeps the bundle's `openbf_data/` (profiles, saves, logs). A flat `dist/openbf_data/`
  from a pack before the bundle is merged in: files the bundle lacks are moved in, and
  a file both have keeps the bundle's copy, with the other left in place and named in
  the summary. A bundle `openbf.pak` left from an earlier split build is removed when
  the new build is one file. Nobody has run a macOS bundle yet. The layout is pinned by
  `scripts/tests/test_pack_limits.py` (`MacOsBundle`), and the `.icns` is written as
  bytes here because macOS 10.7+ reads PNG in every entry type.

## A development build

A plain `cargo build` or `cargo test` with no `OPENBF_ICON` embeds nothing and prints
nothing, on every OS. `build_icon.rs` only acts for a Windows target, and only when
`OPENBF_ICON` names a file that exists. To render the icon by hand from a checkout's own
bake:

```bash
cargo run -p game --release -- --render-icon engine/branding_baked/openbf.png --assets engine/assets_baked
python3 scripts/make_icons.py          # the containers, beside the master
python3 scripts/make_icons.py --check  # fail when a container does not draw what the master draws
```

## Notes

- `--check` compares what each container draws (pixels, and the set of entries in
  each container), not bytes, so a different Pillow version is not reported as stale.
  `scripts/tests/test_make_icons.py` tests every container from a synthetic image drawn
  by the test itself. It runs in `python3 -m unittest discover -s scripts/tests`.
- The window itself is given no icon in code: `window_attributes` in
  `engine/crates/game/src/main/windowed.rs` sets only the title, and the OS shows the
  bundle, `.desktop` and `.exe` icons.
