# PAD-XPAD-RUMBLE result (2026-11-27)

Branch `claude/pad-xpad-rumble`, from private main 86dbbfec8.

## What changed

1. **Pads in xpad mode map fully.**
   - `engine/crates/input/pads.txt`: an SDL line for GUID
     `03000000373500007505000046010000`. That is the GameSir G7 Pro dongle after it
     re-enumerates as "Xbox 360 Controller for Windows" (3537:0575, which xpad names
     "Generic X-Box pad"). It uses the xpad standard layout, the same as the
     dongle's other xpad-mode line (3537:109c).
   - `engine/crates/input/src/lib.rs`: a general fallback for any pad on gilrs's
     driver layout, meaning a pad with no SDL line:
     - `LeftTrigger2`/`RightTrigger2` read from the `LeftZ`/`RightZ` axes
       (ABS_Z/ABS_RZ, -1..1 mapped to 0..1, pressed past `TRIGGER_PRESS`, strictly).
       A trigger that has sent no report yet reads as released.
     - Only on a pad that also has ABS_RX/ABS_RY (`RightStickX`/`RightStickY`). A
       generic HID pad with its right stick on Z/Rz (centred at 0, which would read
       as half a trigger and hold FIRE) gets no trigger fallback and keeps the
       unresolved-FIRE warning.
     - The d-pad counts as resolved when the pad has hat 0. gilrs already turns the
       hat into d-pad button state, and `is_pressed` reads it, so the d-pad worked
       before; only `unresolved` reported it wrongly.
     - `is_down`, and through it every gameplay, menu, split-seat and remap reader,
       uses the fallback. The free-fly camera's analog up/down reads
       `input::trigger_value`, which applies the same fallback. `unresolved` and the on-screen warning stop listing those
       controls.
     - SDL-mapped pads and the Windows unified pads are untouched. The fallback
       runs only when `mapping_source() == Driver` and the button has no code.
2. **Rumble sends less.**
   - Reading gilrs 0.11.2 `src/ff/server.rs`: a pulse adds no device traffic.
     `EffectBuilder::finish` and `play` are only channel messages to gilrs's
     force-feedback thread. That thread, however, calls `set_ff_state` on every
     force-feedback device every 50 ms tick, playing or not. Each call is an
     `EVIOCSFF` upload plus an `EV_FF` play write, and xpad and hid-playstation
     turn each one into an output report. So every rumble-capable pad got about 20
     reports a second from the moment it connected, whether anything was playing or
     not. On that night's setup that meant the 8BitDo dongles (xpad), the DS4 over
     Bluetooth (hid-playstation) and the GameSir in xpad mode. It cannot be changed
     from outside gilrs.
   - The fix vendors gilrs 0.11.2 at `engine/vendor/gilrs`, applied with
     `[patch.crates-io]` in `engine/Cargo.toml`, with `vendor` excluded from the
     workspace. It changes one thing, in `combine_and_play`:
     - A device is written when its state changes, on that tick.
     - An unchanged playing state is refreshed every other tick, written with a
       150 ms length instead of 100 ms so it never lapses.
     - A stop is written on three ticks in a row, then the device is left alone.
       `set_ff_state` reports no failure, and the Windows backend ignores the
       length, so a single lost stop could otherwise leave the motors running.
   - `vendor/gilrs/OPENBF.md` records the change, what was stripped (examples,
     Cargo.lock, dev-deps, the DB's image and script) and the licence. LICENSE-MIT
     was added. A `std::f32` import was dropped to clear a deprecation warning.
     CHANGELOG.md, README.md and the DB's README were dropped too. The game's
     rumble callers are unchanged.
   - Public export: `scripts/export_public.py` now ships `engine/vendor/`, with a
     reviewed 640 KB budget for the SDL DB (zlib, its LICENSE ships with it).
     Without that, the exported `[patch.crates-io]` line would point at a folder
     missing from the public tree. A new refusal, `manifest_path_problems`, rejects
     any exported Cargo.toml `path =` that names something not exported
     (`ManifestPathTests` in scripts/tests/test_export_public.py). The two licence
     files are allowed as bare-named text, keep their upstream authors' addresses
     (`VENDORED_NOTICES`, by exact path; zlib forbids altering the notice) and skip
     the corpus excerpt check, which matched them only because the corpora carry
     the same standard MIT and zlib texts. The scrub changes no vendored file.

## Measurements

These come from four virtual xpad pads made through uinput
(`engine/crates/input/tests/uinput_pads.rs`, `rumble_reports_per_second_four_pads`).
The counts are force-feedback writes per second per pad, uploads plus plays. The
stock figures were taken by commenting out the patch line.

Vendored figures are from the final tree (stop repeated on three ticks),
`cargo test -p input --test uinput_pads -- --ignored --test-threads=1 --nocapture`:

```
idle:               0.0 / 0.0 / 0.0 / 0.0
all firing (8 Hz):  20.0 / 20.0 / 20.0 / 20.0
idle after firing:  0.0 / 0.0 / 0.0 / 0.0
motor while firing: (3108, 1, 621, 1305) x4
fire cue (199, 1, 39, 83); hit (350, 1, 122, 350); flyer impact (701, 1, 140, 701);
preview (400, 1, 400, 400)
```

The stock column was measured earlier with the patch line commented out (not
committed, output not saved); it is 2 writes per 20 Hz tick per pad, which the
code gives directly.

| scenario | stock gilrs | vendored |
|---|---|---|
| idle | 39.3-40.0 | 0.0 |
| all four firing a repeater (cue 0.42/55 ms + 0.20/45 ms at 8 Hz) | 39.3-40.0 | 20.0 |
| idle after firing | 39.3-40.0 | 0.0 |

The motor timeline is rebuilt from the uploads and plays the way the kernel plays
them. Each tuple is (motor-on ms, separate runs, strong full-scale ms, weak
full-scale ms). Stock and vendored match to within about 2 ms in every case:

- 3 s of firing: (3105, 1, 621, 1304) on both.
- Fire cue: (200, 1, 40, 83) on both.
- Hit: (350, 1, 122, 350) vendored, (350 to 351, 1, 122, 350 to 351) stock.
- Flyer impact: (701 to 702, 1, 140, 701 to 702) on both.
- Preview: (401, 1, 401, 401) vendored, (400 to 402, ...) stock.

## Tests

- `cargo test -p input`: 40 lib tests passed. The new ones are
  `xpad_style_driver_pads_resolve_triggers_and_dpad` (a synthetic xpad-style axis
  set) and `gamesir_xbox_360_mode_guid_has_the_xpad_layout`.
- `cargo test -p input --test uinput_pads -- --ignored --test-threads=1`: 3 of 3
  passed.
  - `hid_pad_with_its_right_stick_on_z_never_fires`: a virtual 1209:f00e pad with
    centred -32768..32767 ABS_Z/ABS_RZ and no ABS_RX/RY. `RightTrigger2` stays
    unresolved, and neither trigger nor FIRE reads held with Z/Rz centred, full or
    at minimum.
  - `xpad_pads_without_a_db_line_map_triggers_and_dpad`: a virtual 3537:0575 pad
    resolves through the new line (SdlMappings, nothing unresolved). A virtual
    1209:f00d pad with no line stays on the Driver layout with nothing unresolved.
    On both, the triggers press and release (a light touch is not a press), each
    hat direction gives exactly its d-pad arm, and the sampled `PadState` carries
    FIRE.
  - Disabling the trigger fallback makes this test fail on the unknown pad's
    trigger.
  - On stock gilrs, before the change, this test failed at the mapping and the
    rumble test failed at its idle check.
- `cargo clippy -p input --all-targets`: no warnings in input or the vendored
  gilrs.
- The lib test also covers a pad with {LeftStick, LeftZ, RightZ, DPad} and no
  ABS_RX/RY: the d-pad resolves, the triggers do not.
- `python3 -m pytest scripts/tests/test_export_public.py`: 83 passed.
  `python3 scripts/export_public.py --check --ref HEAD` on the fix commit: clean.
- `scripts/one_player_gate.py` on the release `game` built from the fix tree: every
  row matches (bf1_hot1 f2d22884a7116994, bf2_uta1 7152c06c6607554e, bf2_spa1
  064e247849c498f3, lan_soak ok 1559 ticks, 4312 hash checks).

## Deviations

- The fix vendors a third-party crate, about 600 KB, nearly all of it the SDL DB
  (which the game's gamepad mapping still relies on; `pads.txt` only fills gaps).
  The patch pins gilrs at 0.11.2 until the copy is dropped. That is
  the only place the per-tick writes can be cut. The maintainer may prefer
  upstreaming the change and dropping the copy, as OPENBF.md says.
- The uinput tests are `#[ignore]`. They need `/dev/uinput` and a seat ACL on new
  event nodes.

## Left

- Not run on littlemallard's real pads. Worth confirming there:
  - The GameSir in its 0575 mode now logs `SdlMappings (GameSir G7 Pro (Xbox 360
    Mode))`.
  - Whether the dongle drops get rarer with 4 pads and no idle rumble traffic.
    That is a hypothesis, not measured: the 20 Hz writes reached each pad's radio
    the whole session.
- Report upstream to gilrs: idle writes every tick.
