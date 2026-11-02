---
id: PADM
title: Controllers work out of the box: gilrs has no mapping for the maintainer's GameSir G7 Pro (USB 3537:1022), so the right stick (camera) is dead and X/Y are swapped — bundle SDL_GameControllerDB + a project mapping file via GilrsBuilder::add_mappings, log each pad's resolution, warn on an unresolved camera/fire
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/input/src/lib.rs,engine/crates/input/pads.txt,engine/crates/input/examples/pads.rs,engine/crates/game/src/main/windowed.rs,engine/crates/ui/src/hud.rs,engine/CRATES.md
declares:
needs:
branch: padm
resume:
created: 2026-09-29
---

## Why
The maintainer's INBOX 2026-09-28: on his GameSir G7 Pro the camera cannot turn, so nothing is testable on a pad. Value 5 by his
word ("M3 or earlier"). Engine plumbing, not PS2 behaviour: write Rust directly.

## Evidence (Claude's probe for the maintainer, gilrs 0.11.2 / gilrs-core 0.6.8 = engine/Cargo.lock, on the real pad)
- USB `3537:1022`, generic HID (usbhid, js0), SDL GUID `03000000373500002210000010010000`; 15 buttons (evdev 304–318),
  axes X/Y/Z/Rz + GAS/BRAKE + HAT0. The right stick is ABS_Z / ABS_RZ, not RX/RY.
- gilrs's bundled DB has no entry: `map_name=None`, `mapping_source=Driver` → `RightStickX/Y -> None` (camera dead),
  North=307 / West=308 (the reverse of SDL's: RELOAD and ACTION swapped), triggers only digital TL2/TR2.
- Upstream SDL_GameControllerDB (mdqinc, 2026-09-28) lacks it too; the same-layout GameSir pads' mapping, applied to this
  GUID, resolves everything. **Verified by the maintainer on the real pad (2026-09-28 evening) — ship it as is:**
  `03000000373500002210000010010000,GameSir G7 Pro,a:b0,b:b1,back:b10,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b12,leftshoulder:b6,leftstick:b13,lefttrigger:a5,leftx:a0,lefty:a1,rightshoulder:b7,rightstick:b14,righttrigger:a4,rightx:a2,righty:a3,start:b11,x:b3,y:b4,platform:Linux,`
- The two `gilrs::Gilrs::new()` sites: `crates/game/src/main/windowed.rs:331` and `:790`; the pad → PadState map is
  `crates/input/src/lib.rs` (FIRE = `Button::RightTrigger2` at :44, aim `Axis::RightStickX` at :109).

## Notes
- The maintainer's workaround until this lands, from `engine/`: `SDL_GAMECONTROLLERCONFIG='<the line above>' ./target/release/game`.
