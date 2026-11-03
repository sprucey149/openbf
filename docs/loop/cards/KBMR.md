---
id: KBMR
title: Configurable keyboard and mouse action bindings
status: archived
issue: 13
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #13](https://github.com/sprucey149/openbf/issues/13).

## Why

The roadmap promises remapping. Keyboard/mouse action bindings currently use
`Bindings::pc_default`; players need to customize them through the controls UI.
Existing sensitivity and invert settings remain available.

## Proposed acceptance

- Change a keyboard/mouse action binding in the controls UI, save it in the
  profile, and retain it after restarting. Older profiles keep current defaults.
- Restore defaults and handle conflicts, cancelled capture, focus loss and
  press/release edges without stuck or duplicate actions.
- Keep default-input behavior, controller input and existing deterministic
  replays unchanged. Preserve the current simulation/input boundary.
- Retain discriminating persistence/input tests, normal integration checks and
  a before/after UI demonstration. Record any required manual checks honestly.

One feature branch/PR. Agree on the final scope and assignment before coding;
an early draft PR should make overlap visible. This proposal does not authorize
unrelated input, camera, simulation or vehicle changes.
