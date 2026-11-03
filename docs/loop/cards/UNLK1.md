---
id: UNLK1
title: Class unlocks on BF1 too (OURS - the BF1 PS2 has none): the same two tiers as BF2's (officer 8 points, special 12) on every BF1 class that is really fielded as a special (jet trooper, droideka, dark trooper, wookiee) or an officer
status: archived
---

Historical technical record. Recorded state before archival was `in-progress`.

## Why
The maintainer (INBOX 2026-10-21 17:50 item 4a). A named deviation: write it into FEEL_NOTES / the deviation list. Check each class
against the BF1 class lists: only a class BF1 really fields is locked.

## Acceptance
- A census line: BF1 classes locked per side and tier; a test that a BF1 special class is locked at 11 points and free
  at 12.

## Build (2026-10-28, with UNLK3)
Census: BF1 locks one class per side, all at the special tier (12): Alliance `all_inf_smuggler` /
`all_inf_smugglersnow` (Wookiee), Empire `imp_inf_dark_trooper` / `imp_inf_dark_troopersnow`, Republic
`rep_inf_jet_trooper`, CIS `cis_inf_droideka`. BF1 fields no officer class, so no 8-point tier (the maintainer's
2026-10-28 design names only these four). Deviation in FEEL_NOTES. Test `game/tests/suite/unlocks_bf1.rs`
(Kamino jet trooper, Yavin dark trooper and smuggler: refused at 11, spawned at 12). The droideka's lock has
no real-map test until DEKF fields it. Details on card UNLK3.
