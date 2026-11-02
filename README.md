# openbf

An open-source Rust engine that plays Star Wars: Battlefront and Star Wars: Battlefront II,
built from your own copies of both games, with the PlayStation 2 versions as its gameplay reference.

**Unofficial fan project.** Not affiliated with or endorsed by Lucasfilm, Disney,
Electronic Arts, LucasArts or Pandemic Studios. This repository contains no game
assets: you need your own copies of both games (see [What you need](#what-you-need)).
Everything the game needs is extracted from your copies on your own machine.

Work in progress: playable, but far from finished.

**Want to help develop it? Start with the [open issues](https://github.com/sprucey149/openbf/issues).**
Comment on an unassigned task and wait for confirmed scope and assignment before coding,
so contributors and automated agents do not duplicate work. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Building your copy

One command turns your copies of the two games into one game file:

```bash
git clone https://github.com/sprucey149/openbf.git
cd openbf
mkdir iso   # put your own copies of both games here; see "What you need" below
python3 scripts/pack.py
./dist/openbf
```

### What you need

- **Both games.** You need a copy of Battlefront (BF1) and a copy of Battlefront II (BF2).
  It will not build with only one of them.
- **A console copy of BF2.** PC BF2 does not contain everything the game needs, such as the menus
  and HUD it uses, so it can only be an add-on to a console BF2, never your only BF2.
- **Mix and match.** The aim is that consoles can be mixed freely, for example PS2 BF1 with Xbox BF2,
  and that you get the same game whichever you use: only texture and asset sizes change.
  The Status note below says how much of this works today.
- **For best results, add PC and Xbox copies.** The build keeps the largest copy of each texture, as long as
  it is the same painting. The PC copy is the largest much more often: about 680 textures across
  the two games, against 12 where the Xbox copy is larger. Those are mostly on BF2 Geonosis, such as a sky at
  1024x256 against the PC's 512x128.

> **Status: Xbox discs are not used yet.** A build today needs PS2 discs of both games. Xbox discs are
> recognized but not read. Next to a PS2 disc of the same game, an Xbox disc is ignored and the build
> carries on. Without that PS2 disc, the build stops and says so. PC BF1 can't be your only BF1 yet either.

| You supply | What you get today |
|---|---|
| PS2 BF1 + PS2 BF2 | Builds, with the PS2 textures |
| PS2 BF1 + PS2 BF2 + PC BF1 and/or PC BF2 | Builds, the same game with larger PC textures where they exist |
| Any working set above + the DLC folder | Builds, with Kit Fisto and Asajj Ventress added (see below) |
| Xbox BF1 and/or Xbox BF2, without a PS2 disc of that game | Refused for now (planned) |
| PC BF1 without a console BF1 | Refused for now |
| PC BF2 without a console BF2 | Refused |
| Only one of the two games | Refused |

### Where to put your files

Everything goes in `iso/`. File and folder names don't matter, because each input is recognized by what's
inside it. One tree that works, with a PC install copied or symlinked in as a whole folder:

```text
iso/
  SWBF1.iso                      a disc image, any name
  SWBF2.iso                      a disc image, any name
  My PC Battlefront/             any name
    GameData/Data/_LVL_PC/       a PC BF1 install
  My PC Battlefront II/          any name
    GameData/data/_lvl_pc/       a PC BF2 install
  DLC_XBOX/                      optional, see below
```

- **Disc images** must be plain dumps: a PS2 `.iso`, or an Xbox `.iso` including trimmed `.xiso` images and
  full dumps. CHD, CSO, ZSO, zip, 7z, rar and gzip files are refused, so unpack them first. Raw
  BIN dumps are refused too, so convert them to a 2048-byte `.iso`.
- **Disc images must be the original discs.** A rebuilt or modified PS2 disc is refused, and so is a folder of
  files extracted from a disc: add the disc image instead.
- **PC copies** are the classic 2004 and 2005 releases, such as those from Steam or GOG. Aspyr's Classic
  Collection is refused.
- Something unusable doesn't stop a build as long as both games still have a source. The summary names what
  was refused and why.

To see what your folder adds up to without building anything, run `python3 scripts/input_scan.py`
(`python scripts\input_scan.py` on Windows). It prints what it found, what it will use and what it refused, and
what to add if something is missing.

To use copies stored somewhere else, name them instead: `--iso1 <file>` and `--iso2 <file>` for BF1 and BF2 disc
images, `--pc1 <folder>` and `--pc2 <folder>` for PC installs, and `--iso-dir <folder>` to scan another folder in
place of `iso/`.

### DLC heroes (optional)

Kit Fisto and Asajj Ventress were Xbox downloadable content for BF2. The practical way to get them is the
community pack [Xbox DLC and Title Update Content for PC](https://www.moddb.com/mods/xbox-dlc-content-for-pc/downloads/xbox-dlc-and-title-update-content-for-pc)
on ModDB. Download it, unpack it, and put its `DLC_XBOX` folder anywhere in `iso/`:

```text
iso/
  DLC_XBOX/
    addme.script
    data/_LVL_PC/SIDE/dlc.lvl
    ...
```

Leaving the rest of the pack beside it is fine, for example `iso/ADDON_XBOX/DLC_XBOX/`: only these two heroes
are used, and none of the pack's maps or other content. They take the Republic and CIS hero slots on BF2 battles
that have those heroes. These are the community's adapted versions of the heroes rather than the Xbox originals,
and they have no voice lines of their own. Thanks to the pack's authors: its heroes are by Gogie and Ryan Hank,
assembled by iamashaymin, with Xbox map conversions by SleepKiller. Without the folder, the game builds unchanged.
Adding or removing it bakes everything again on the next run.

### What the build does

`pack.py` extracts the discs, bakes the assets, builds the game, and packs everything into
`dist/openbf` (about 3 GB). A build of about 3.9 GiB or more becomes a small `dist/openbf` plus `dist/openbf.pak`,
on every OS: keep the two together, and on a filesystem other than FAT32, which cannot hold a file of 4 GiB or more.
Run the game from anywhere; it keeps your profile and its bug-report snapshots in an `openbf_data/` folder beside
itself. Run the script again after pulling new code: it skips what is still current
and bakes again only when the code that bakes has changed (that can take a while).

**`dist/openbf` and `dist/openbf.pak` contain the games' own content, so never upload or share them.**
Everyone builds their own from their own copies of the games.

Requirements on Linux:
- Rust (https://rustup.rs), Python 3, bash, ffmpeg
- ALSA and udev libraries (on Fedora: `alsa-lib-devel systemd-devel`; on Debian/Ubuntu: `libasound2-dev libudev-dev`)
- A Vulkan-capable GPU, and a game controller or a keyboard and mouse

### Windows

The same command builds `dist\openbf.exe`. You need:
- Rust (https://rustup.rs), with the Visual Studio C++ Build Tools that rustup offers to install
- Python 3 (https://python.org, or `winget install Python.Python.3.12`)
- Git for Windows (https://git-scm.com): the bake runs in its bash; `pack.py` finds it by itself
- ffmpeg on your `PATH` (`winget install Gyan.FFmpeg`, then open a new terminal)
- A DirectX 12 or Vulkan GPU, and a game controller or a keyboard and mouse

In a new PowerShell or Command Prompt:
```
git clone https://github.com/sprucey149/openbf.git
cd openbf
mkdir iso
python scripts\pack.py
dist\openbf.exe
```
Your profile and snapshots are saved in `openbf_data\` beside `openbf.exe`.

## Controls

The game is built around a controller and plays like the PS2 versions, with their aim assist.
Keyboard and mouse also work, as a deliberate addition for PC: click into the battle to capture
the pointer; Esc pauses and frees it again. Mouse aim gets no aim assist. Sensitivity and invert-Y
are in Options and in the pause menu.

| Action | Keyboard and mouse |
|---|---|
| Move | W A S D |
| Look and aim | mouse |
| Fire / secondary fire | left / right button |
| Zoom | middle button or Z |
| Jump | Space |
| Crouch / prone | C or Left Ctrl |
| Sprint | Left Shift |
| Reload | R |
| Use (enter or leave a vehicle) | E |
| Next / previous weapon | Q or the wheel |
| Switch secondary weapon | G |
| First / third person (also "Viewpoint" in Options and the pause menu) | V |
| Squad orders | F1 follow, F2 hold, F3 at ease, F4 move out (or the arrow keys, as the D-pad) |
| Spawn screen | W A S D or arrows to pick a post, Q / Tab or the wheel for the class, Space or Enter to spawn, Backspace back |
| Menus | arrows, Enter, Esc or Backspace |

In a hover tank, speeder or flyer the mouse turns the vehicle, as the steering stick does;
in a walker it aims the walker. In a gunner's seat or a turret it turns the gun.
Tested and working on Linux and Windows, with the NTSC-U PS2 releases (SLUS-20898 and SLUS-21240).

## How it was made

The engine is written in Rust, with substantial help from AI agents and community contributors.
Development uses automated tests and comparisons with the
original games in PCSX2, including memory readings and rendered images. Work is at different
stages of implementation and verification; an implemented feature is not necessarily fully validated.
That scaffolding is published too:
- `PLAN.md`: the goals and the locked feature set
- `engine/CRATES.md`: a map of the code
- `analysis/`: reverse-engineering notes, supporting references, and remaining uncertainties
- `docs/loop/`: how the AI loop is run (its rules, its session prompt, the backlog and every work card)
- `scripts/`: the build, bake, test-gate and loop tools

No code from the original games is included; the decompilation is used only as a reference for
behaviour.

This repository is updated as changes are integrated. [GitHub Issues](https://github.com/sprucey149/openbf/issues)
coordinate feature scope and ownership; `docs/loop/cards/` contains detailed implementation notes
and smaller work items. A card without an issue is not automatically available: propose an issue first.

## Join in

I started this project because I wanted a better Star Wars: Battlefront, and I still want that
game for myself. I'd much rather build it together with everyone who loves these games too.
**Pull requests, bug reports, feature requests, ideas, playtesting and feedback of any kind are
welcome and hoped for.**

- **Found a bug?** Open an issue: say what you did, what you saw, which map, and attach a
  screenshot if you can. Press **F9** in the game right after it happens: that saves a snapshot of every unit's state
  to `openbf_data/playtest/last.json`. Attach that file too; it usually shows the cause.
- **Want a feature?** Search the [issues](https://github.com/sprucey149/openbf/issues) first, then open
  an issue describing the player experience and intended scope. The cards in `docs/loop/cards/`
  provide more roadmap detail.
- **Want to write code?** Choose an unassigned issue, comment that you would like to take it,
  and wait for confirmed scope and assignment. Assigned issues are reserved. If your idea has no
  issue, open one before coding. Use one feature per branch/PR and an early draft for larger work.
  [CONTRIBUTING.md](CONTRIBUTING.md) explains the workflow and evidence expected.

It's MIT licensed, so you're also free to fork it and take it your own way. If you do, I'd love
to hear about it, and even more to see your improvements come back here.
