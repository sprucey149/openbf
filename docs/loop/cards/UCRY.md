---
id: UCRY
title: Hurt and death cries, both games (the maintainer's INBOX 2026-10-12 18:55, ask 3; slice of USND): a unit cries out on a health hit of more than 2 % of the health it has left and survives, and dies with its death cry (cutting the hurt), on ONE voice per unit, in its own class's HurtSound / DeathSound (the droids' droid_death, the Wookiee's, the Gungans') - sample-bank 3-D one-shots outside the voice queue, chance 0.85 / 1, 7-200 m; RE done 2026-10-13 (analysis/re-unit-chatter.md 2.7, 3.6); refereed by cries per hit and by the PS2's per-minute rows; must not overload the mixer
milestone: M3
value: 5
phase: build
status: done
deps: USND
files: engine/crates/game/src/event_sounds.rs,engine/crates/game/src/sounds.rs,engine/crates/core/src/sim/state.rs,engine/crates/game/tests/unit_voice.rs
declares:
needs:
effort:
branch: ucry
resume: landed 53a9d7e0 2026-10-16; audit incomplete 2026-10-26 — restore all held primary inputs and verified landing/parent history; reports/20261026_0343_audit_UCRY.md
domain: audio
created: 2026-10-13
alpha: yes
---
## Why
The maintainer's INBOX 2026-10-12 18:55, ask 3 (umbrella card USND, slice 2 of his order): units cry out when hit and when
they die, in their own faction's voice. The port plays neither. On the PS2 these are the most frequent unit voices
by far and need no voice queue (`analysis/re-unit-chatter.md` 2.7, 3.6, 5 `[O]`).

## Evidence (the PS2's rule, both games)
- **Hurt**: inside the damage function, after armour and shield: a HEALTH hit of more than 2 % of the health LEFT
  after it, on a unit that survives, asks the class's `HurtSound`. A hit the shield swallows, a zero-damage hit and
  a hit of 2 % or less ask nothing. BF2 also needs damage-flag bit 2 and has `HurtFallSound` for a hit with no
  attacker (authored by no class: silent).
- **Death**: the hit that kills, or a forced kill of a living unit, asks `DeathSound`.
- **One voice per unit**: both go to one handle on the entity; a sound still on it is stopped first (a new hurt cuts
  the last one, death cuts hurt).
- **Data**: sample-bank 3-D one-shots, 7-200 m, gain 0.9; `..._wound` PlayProbability 0.85, `..._death` 1, Wookiee
  and natives 0.8; no interval, three unlocked takes (droid death six). The port's bank already applies
  PlayProbability (`game/src/sounds.rs` 1153-1162).
- **Voices** (`lead_tmp/usnd/bf1_classes.tsv`, `bf2_classes.tsv`): the class's own key - BF1 54 of 59 soldier
  classes (the CIS die with `droid_death`; `..._female`, `..._wookie`, Gungan, Ewok, Tusken, Jawa, Geonosian
  variants), BF2 132 of 162.
- **PS2 rates** (`lead_tmp/usnd/live_bf1.txt`, `live_bf2.txt`; per minute, in range / within 50 m of the eye):
  hurt NAB1 post 42.8 / 0.3, fight 58.0 / 17.3; TAT1 41.3 / 7.5, 70.5 / 19.3; UTA1 72.5 / 6.0, 139.3 / 50.3;
  MYG1 20.8 / 0.5, 83.8 / 31.8. Death NAB1 27.0 / 0.8, 33.0 / 10.8; TAT1 16.5 / 5.0, 34.8 / 8.8; UTA1 18.8 / 0,
  35.8 / 11.8; MYG1 9.3 / 0.8, 26.5 / 10.5.

## Notes
- The maintainer's order puts the combat calls (card UCALL) first; this slice is the cheapest and shares no bake change with
  it - whichever the picker reaches first.
- The generic object's second one-shot hurt (traps, props) and `DamageRegionSound` (BF2's choke in a damage region)
  are out of scope.
- MIXH RE (2026-10-16, `analysis/re-mix-headroom.md`): the fight-log clipping is the port's per-ear level law (a
  centred 3-D sound 1.0 per ear in kira where the PS2 gives 0.5; BF2's effects bus 0.63 not applied), not the cries:
  the PS2's own fight starts through the port's law clip like the port (peaks 2.6-6.0), through the PS2's law <= 0.02 %
  over. MIXH is now a build card; its acceptance re-runs this card's `census/run.sh` fight logs per ear.
## Note (2026-10-16 03:36, lane 2, card MIXH LANDED a91d252)
The mixer clause's cause is fixed on main: the PS2's per-ear law + bus gains + a master limiter. On MIXH's branch, UCRY's
5 fight logs without cries (main has none): per-ear frames over full scale 0.0004-0.0011 % (BF2) and 0.035 / 0.072 %
(NAB1 / TAT1) by the law alone, 0 through the limiter (`lead_tmp/mixh/census/summary*.txt`). To re-measure WITH the cries:
rebase ucry on main, rebuild ucry-census-scratch with its follow patch, run `census/run.sh fight ...`, then
`scripts/tools/mix.py <log> engine/assets_baked --limit` (take rows now carry `|left|right`; `--law old` = pre-MIXH).
## Note (2026-10-16 04:40, lane 2, land session)
Rebased on main 1763d73 (MIXH + AMBS): conflicts in game/src/sounds.rs (struct fields: ambient + cries both kept) and CRATES.md;
`play_cry` / `play_cries` now go through MIXH's `Start::ears` (gain x the property's bus, per-ear law) and `mixer_pair`, and
`note` gets the ears. Census on the rebased tree (ucry-census-scratch = ucry 53a9d7e + follow_only.patch), fight NAB1 s1/s2,
UTA1 s3/s4 (`lead_tmp/ucry/census/rebased/summary*.txt`): hurt cries per qualifying hit 1457 / 1723 = 0.846, death 542 / 542;
frames over full scale before the limiter with / without cries 4877/3662 (1.33x), 4258/2850 (1.49x), 324/193 (1.68x),
185/102 (1.81x); through the limiter peak 1.00 and 0 frames over in all 8 renders; under -3 dB at most 0.34 % (NAB1 s1, 0.28 % without cries).
