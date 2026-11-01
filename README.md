# openbf

An open-source Rust engine that plays the PlayStation 2 versions of
Star Wars: Battlefront and Star Wars: Battlefront II.

**Unofficial fan project.** Not affiliated with or endorsed by Lucasfilm, Disney,
Electronic Arts, LucasArts or Pandemic Studios. This repository contains no game
assets: you need your own PS2 discs of both games. Everything the game needs is
extracted from your ISOs on your own machine.

Work in progress: playable, but far from finished.

**Want to help develop it? Start with the [open issues](https://github.com/sprucey149/openbf/issues).**
Comment on an unassigned task and wait for confirmed scope and assignment before coding,
so contributors and automated agents do not duplicate work. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Building your copy

One command turns your two disc images into one game file:

```bash
git clone https://github.com/sprucey149/openbf.git
cd openbf
mkdir iso   # put your own disc images here: iso/SWBF1.iso and iso/SWBF2.iso
python3 scripts/pack.py
./dist/openbf
```

`pack.py` extracts the discs, bakes the assets, builds the game, and packs everything into
`dist/openbf` (about 3 GB). Run that file from anywhere; it keeps your profile and its bug-report snapshots in an
`openbf_data/` folder beside itself. Run the script again after pulling new code: it skips what is still current
and bakes again only when the code that bakes has changed (that can take a while).

**`dist/openbf` contains the games' own content, so never upload or share it.**
Everyone builds their own from their own discs.

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

Disc images: `--iso1 <path>` and `--iso2 <path>` point the script at images stored somewhere else.

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
