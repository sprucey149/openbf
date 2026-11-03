---
id: PROFSEAL
title: Release builds keep the profile in a sealed (non-editable) format, so unlocks, awards and career numbers have to be earned; dev builds keep profile.ron
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer 2026-10-28: "during this dev period, we can keep the profile as a ron file. But when we make this a full release
game [...] we need to make the profile file unreadable and can't be edited [...] it removes the spirit of the game of
having to do a bit of grinding to unlock characters and the awards". Not urgent: post-alpha, before a release.

## Notes
- A local file can never be made impossible to edit; the aim is that a text editor can't change it and a casual
  edit is detected. Options: a binary encoding plus a keyed checksum (HMAC) over the contents, refusing (or resetting
  the career part of) a profile whose checksum fails; obfuscation on top is optional.
- Dev builds keep `openbf_data/profile.ron`; a cargo feature or build flag picks the sealed format for release.
- Settings (controls, audio) could stay readable; only the career / unlock / award parts need sealing.
- Migrate an existing `profile.ron` into the sealed file once, on the first release-build run.
