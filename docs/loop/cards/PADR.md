---
id: PADR
title: Configurable controller action bindings
status: archived
issue: 14
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #14](https://github.com/sprucey149/openbf/issues/14).

## Why

The controller mapping table supports alternate mappings, but the running game
constructs the default mapping. Players cannot edit or persist controller
bindings through settings yet.

## Proposed acceptance

- Edit controller action bindings in controls settings, save the layout in the
  profile, and retain it after restart. Existing profiles keep current defaults.
- Restore defaults and handle conflicts, cancelled capture, held-button release
  and disconnect/reconnect without stuck or duplicate actions.
- Keep menu navigation, accept/back and a recovery path usable while editing.
  Preserve contextual action semantics and unchanged default gameplay/replays.
- Retain discriminating profile/input regression tests and demonstrate the
  complete interaction with a real controller. Record unrun manual checks.
- Coordinate the shared screen and profile boundary with KBMR; do not run two
  overlapping input/UI implementations independently.

This is a scoped remapping proposal, not authorization to change movement,
aiming, controller deadzones or the gameplay action set.
