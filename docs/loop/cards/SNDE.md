---
id: SNDE
title: BF1's commander speaks with the other era's voice: on a two-era planet the port's sound bank keeps ONE definition per name for the whole LVL (the last sub-level's, <planet>2cw), where the PS2 holds the sub-level its mission names - so in the default battle on 11 of 15 BF1 maps the Empire and the Rebels hear the Clone Wars stream's takes at every post event (the maintainer: 'empire voiceovers and droid voiceover lines')
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/bake/src/sounds.rs,engine/crates/game/src/sounds.rs,engine/crates/game/src/mission.rs,engine/crates/manifest/src/lib.rs,scripts/gen_missions.py
declares: rebake=sounds
needs:
effort:
branch: snde
resume: landed e45d75a3 2026-10-06
created: 2026-10-05
alpha: yes
domain: audio
---
## Why
Filed by the audit of ANNC2 (2026-10-05, PARTLY; `reports/20261005_1150_audit_ANNC2.md`, evidence
`engine/artifacts/lead_tmp/audit-annc2/` = A/). The maintainer, 2026-10-04, BF1 Kashyyyk Docks as the Empire: "it has empire
voiceovers and droid voiceover lines". ANNC2 filed that under its era-mix rule; it is not one. His battle was the
map's default pairing (screenshot `playtest/playtest_20261004/1_kas2_empire_hud_blotch.png`: a stormtrooper, an AT-ST,
the Rebels' bowcaster in the console), and ANNC2's landing leaves it as it was.

The cause is the sound bank. A BF1 planet's SOUND LVL holds one sub-level per map and era (`kas1gcw`, `kas1cw`,
`kas2gcw`, `kas2cw`), and each defines the commander's post lines under the SAME names with its own era's stream and
segment. The PS2 loads the one sub-level its mission names (`ReadDataFile("sound\\kas.lvl;kas2gcw")`). The port bakes
every sub-level into one `sounds.ron` per planet and `game::sounds::merge_props` keeps one definition per name: the
last in file order that can make a noise - the LVL's last sub-level, `<planet>2cw`. So in a GCW battle on a two-era
BF1 planet the Empire's and the Rebels' "captured / lost a command post" lines play the Clone Wars stream's takes
(the CIS commander's). `merge_props`'s own comment marks the rule PROVISIONAL ("which sublevel's definition the PS2
itself uses is not established"): it is established now.

## Evidence
- Disc [V] (`python3 A/sublevel_defs.py ...`, output `A/sublevel_defs_kas.txt`, my own byte read of KAS.LVL):
  `imp_off_com_report_lost_commandpost` is gcw_vo segment 1c18b3b0 in kas1gcw / kas2gcw and cw_vo segment bdf175ac in
  kas1cw / kas2cw; `imp ..captured` 1d18b543 / c0f17a65; `all ..captured` 4ff2552b / bdf175ac; `all ..lost`
  4ef25398 / bef1773f; `imp ..lost_commandpost3` 2218bd22 / bef1773f. The mission scripts: `kas2i.luac` names
  `sound\kas.lvl;kas2gcw`, `kas2c.luac` `kas2cw`.
- PS2 RAM [L] (`A/ram_defs.py <state.p2s>`; `A/ram_defs_all_bf1.txt`): all 161 BF1 save states of
  `pcsx2/ref/states/` - 96 GCW, 64 Clone Wars, 1 menu. Each battle state holds one era's names only, and all six
  post-line names sit beside that era's stream: 160 of 160, none beside the other era's.
- Port (`A/take_files.tsv`, probe `A/examples/audit_takes.rs files`): on `bf1_kas2` those names resolve to
  `audio/global_bf1/99d69d17/...` (cw_vo); on the GCW-only `bf1_hot1` to `4ddc71b2/...` (gcw_vo).
- A whole default battle (`A/runs/new/bf1_kas2_con.log`, Empire against Rebels, seed 42): 18 commander lines
  start, 11 of them from the Clone Wars stream (`_captured_commandpost` x6, `_lost_commandpost` x3,
  `_lost_commandpost3` x2), 7 from the mission's own (the bleeding and low-reinforcement lines, which only the gcw
  sub-levels define).
- Scale (`A/sublevel_census_bf1.txt`, `python3 A/sublevel_census.py bf1`, input from
  `A/examples/audit_sublevels.rs`): of the 28 sub-levels BF1's missions load, 16 hold names the port resolves to
  another sub-level's audio - 256 names, 201 of them streamed voice. All 12 GCW sub-levels on two-era planets: 14+
  voice names each (the 12 post lines of the four armies x captured / lost / lost3, `shell_auditionSFX / VO`). Not only
  voice: Bespin Platforms CW takes Cloud City's `cis_bes_amb_*` / `rep_bes_amb_*` Music stream (11 names), Yavin
  Temple 20-36 sample sounds from Yavin Arena, Dune Sea 4-5. BF2 (`A/sublevel_census_bf2.txt`): 0-4 names per
  mission (hero loops, a few others); its commander lines sit in per-army streams and are clean.
- Why no check saw it: ANNC2's census and `every_mission_line_has_a_take` test line NAMES; the name is right and the
  take exists.

## Notes
- The maintainer will hear this at once: every post event of a default battle on 11 of the 15 BF1 maps.
- The `--mission` flag and the tests that build a `SoundLib` from the map directory alone need the sub-level too.
## Landed 2026-10-06 (e45d75a, report `reports/20261006_1350_card_SNDE.md`)
- Acceptance 2-5 run on main's rebaked sounds: KAS2 GCW = gcw_vo 1d18b543 / 1c18b3b0 / 4ff2552b / 4ef25398, KAS2 CW =
  cw_vo; 57 missions, 53,907 names, 0 of other audio (old rule 274); 588 commander lines, 0 off (old rule 198).
- Added beyond the card: an army of the other era (era mix) resolves in the map's sub-level of its own era, because
  a sub-level aliases the other era's post lines to its own takes (26 BF1 missions, 77 guest lines, 0 off).
- Still PROVISIONAL: which of two definitions of one name inside ONE sub-level the PS2 keeps.
