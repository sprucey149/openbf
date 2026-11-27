# gilrs 0.11.2, vendored for openbf

Upstream: https://gitlab.com/gilrs-project/gilrs, crate version 0.11.2, licensed
Apache-2.0 or MIT (LICENSE-MIT here; the SDL_GameControllerDB keeps its own
LICENSE). Applied through `[patch.crates-io]` in `engine/Cargo.toml`.

Removed from the published crate: examples, its Cargo.lock, the dev-dependencies,
CHANGELOG.md, README.md (and the manifest's `readme` key), and the DB's README,
mapping guide image and helper script. The DB itself stays: the game's
`pads.txt` only fills its gaps. The public export ships this folder
(`engine/vendor/` in `scripts/export_public.py`, with a reviewed budget for the
DB), so the `[patch.crates-io]` path resolves in the public tree too.

Changes, marked `openbf:`:

- `src/ff/server.rs` (`combine_and_play`): upstream's force-feedback thread
  uploads the combined rumble state and sends a play request to every
  force-feedback pad on every 50 ms tick, playing or not; the kernel's
  memoryless drivers (xpad, hid-playstation) turn each into an output report
  over the radio the pad's input shares. Now a device is written when its state
  changes (a new effect, the stop) on the tick it changes, an unchanged playing
  state is refreshed every other tick instead of every tick (written with a
  three-tick length instead of two, so it never lapses between refreshes), and a
  stop is written on three ticks in a row (a lost stop write is retried; the
  Windows backend ignores the length) and the device is then left alone. Measured with four virtual xpad pads
  (`engine/crates/input/tests/uinput_pads.rs`): 40 writes a second per pad idle
  before, 0 after; 39 a second while firing before, 20 after, with the same
  motor timeline.
- `src/ff/mod.rs`: the `std::f32` module import dropped (a deprecation warning
  once built as a path dependency); `f32::MAX` is the same constant.

Drop this copy when upstream skips idle writes itself.
