---
id: CHEAT2
title: Five session cheats on the PS2's pause-menu Morse code - invincibility (sos), infinite ammo (ammo), no reloading (clip, split from ammo), infinite stamina (run), unlock all classes (all), plus `cheat` for all five; on until the main menu
milestone: M3
value: 3
phase: build
status: done
deps: UNLK3
files: engine/crates/core/src/sim/state.rs,engine/crates/core/src/weapon.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/sim/guns.rs,engine/crates/core/src/sim/ordnance.rs,engine/crates/core/src/sim/pickups.rs,engine/crates/core/src/sim/remotes.rs,engine/crates/ui/src/menu.rs,engine/crates/game/src/cheats.rs,engine/crates/game/src/pause.rs,engine/crates/game/src/unlocks.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/src/main.rs,engine/crates/game/tests/suite/cheat_ammo.rs,engine/crates/game/tests/suite/unlocks_bf1.rs,analysis/re-cheat-codes.md
declares:
needs:
effort:
branch: cheats
resume: 2026-10-28 built on playtest-20261028; waiting for the maintainer's playtest (the windowed wiring has no automated test).
created: 2026-10-28
---
## Why
The maintainer 2026-10-28: "some cheats I want to add in addition to what we have: invincibility, infinite ammo, infinite
stamina, unlock locked characters, no reloading. The no reloading is a part of the infinite ammo currently, so those
need to be separated. [...] each of these needs a morse code like the bf2 ps2 version had, and when a cheat code is
toggled, it remains toggled until the session ends, ie the game is exited or the user quits back to the main menu,
then everything toggles back to off." Later the same day: one more word, `cheat`, toggles all five together.
