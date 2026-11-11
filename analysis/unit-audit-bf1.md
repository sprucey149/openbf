# OPUS RE (2026-09-29) - [O] UNAUDITED — Unit fidelity audit, BF1 (card UAUD1)

> This is the September 29, 2026 audit. Its port verdicts describe the revisions
> listed below. For current scope, see the [feature set](plan/FEATURE_SET.md)
> and [source gates](source-gates.md).

The maintainer's DECISION 3 (INBOX 2026-09-28): audit every unit class on every map of both games. This document covers BF1.
It lists every soldier class the 28 BF1 Instant Action mission scripts load, and checks each one column by column
against three sources:
- **the disc:** the mission bytecode plus the side-LVL sublevels the script reads;
- **the PS2 itself:** the RAM of 22 in-battle save states over 16 maps, and the class-select screens of all four
  factions;
- **the port as it runs:** main at e69e73a, through its own `FactionRoster::pair`. Every code citation was re-checked
  at 6ea549d.

Tags: [V] read and cited; [D] derived; [H] hypothesis. [O] marks the load-bearing claims, lifted from these sessions
and not yet audited. Tools and outputs are in `engine/artifacts/lead_tmp/unit-audit/`; its `README.md` explains how to
reuse them for UAUD2 (BF2).

Two sessions wrote this. The first built the census, the tables and the gap cards, then ended on an API error. The
second re-derived the load-bearing numbers and ran a second verifier; it corrected six statements and measured one new
gap, the chase camera's distance (→ CAMS). §5.2 lists what it reproduced and what it changed.

## 0. Verdict per column

| column | verdict | gap → card |
|---|---|---|
| **Which map / era loads it** | **MATCH** for the soldier classes the port fields. All 26 BF1 mission RONs equal the disc's teams 1–2: classes, counts, pools, tickets (§2) [O]. | Not fielded: the droideka (13 CW missions) → **DEKA**; heroes → HERO; locals → MSND / TEAMS. BF1 Mos Eisley (`tat2i`/`tat2r`) has no port map → **B1ME**. |
| **Weapon slots** | **GAP.** The port models `WeaponName1` only. The disc gives each team-1/2 soldier 3–4 slots (the droideka 2, heroes and most locals 1), and the PS2 instantiates every slot (§3.1) [O]. | GREN (secondary cycle). **UORD** for the 13 fielded classes whose primary is a missile, shell, emitter or bullet but flies as a bolt. |
| **What each draws** | Body model: MATCH. Primary weapon model: right in the roster, but **wrong in battle for 6 classes**: a class sharing a body model draws the first such class's gun (§3.2) [O]. Per-class skins (`OverrideTexture`) and `SkeletonRootScale`: **GAP** (§3.3) [O]. | **UWPN**; **USKN** |
| **Items and grenades** | **GAP.** No grenade, mine, recon droid, fusion cutter, dispenser or time bomb; no drops (§3.4). | GREN, PWUP |
| **Sounds** | **MATCH:** the primary's fire and reload sounds, its ordnance's impact sound, and squad-order chatter. **GAP:** hurt/death voice, AI combat chatter, low-health loops, weapon movement foley, jet engines (§3.5) [O]. | **USND** |
| **Movement constants** | **MATCH** for all 34 fielded classes: MaxSpeed, MaxStrafeSpeed, MaxTurnSpeed, `Acceleraton`, MaxHealth, clips (§3.6) [O]. **GAP:** jet and jump packs (dark trooper, jet trooper; Geonosian wings). **GAP:** the chase camera. On the PS2 it sits 3.50 m behind the track centre (the classes' CAMERASECTION); the port uses 3.2 m, BF2's value, for every class (§3.6) [O]. | **JETP**; **CAMS** |
| **Animations** | Rifle-bank primaries: MATCH. **GAP** for the primaries on the `bazooka` (7 classes), `sbdroid` (the SBD) and `bdroid` (pilot and assassin droids) banks (§3.7) [O]. | ANIM (value raise proposed) |

## 1. Method and instruments

1. **Disc census** — `census.py`. `missions.py` runs scripts/gen_missions.py's Lua 4.0 undumper over every BF1
   mission and collects `ReadDataFile`, `AddUnitClass`, `SetHeroClass`, `SetTeamName`, `SetUnitCount` and
   `SetReinforcementCount`.
   - `lvl.py` splits each SIDE LVL into its top-level `lvl_` sublevels. Each wrapper's hash is fnv1a over
     `(byte | 0x20)` of the name `ReadDataFile` passes. All 69 names the 44 scripts pass to `ReadDataFile('SIDE\\…')`
     hit a wrapper (verifier). 8 wrappers are never requested: `all_droid_R2D2`, the K-wing, the hovernaut, the
     landspeeder, …
   - A class resolves in the sublevels its mission read, then in COMMON.LVL.
   - Property names come from `vocab.txt` plus the VEH0 dictionaries. 43 soldier keys stay unnamed; they are the
     squad-chatter and wading keys, with the sound names as values.
   - Output: `census_ia.md` / `.json`, and `census_all.*` which adds the 16 `_h` scripts.
2. **PS2 RAM** — `ramcensus.py`. For each soldier health component C (the units.py/ps2live.py scan) [V]:
   - `*(C−4)` is the class object; its +0x14 word is the fnv hash of the class name.
   - `K+0x910+4i` (K = C+0x298) are the weapon objects; `weapon+0x60` is the weapon class, with its name hash at +0x14.
   - Droidekas are objects with vtable 0x530138: class at +4, current health at +8, max at +0xc.
   - Run over all 22 BF1 `*_spawn.p2s` states in `engine/artifacts/pcsx2/ref/states/`. Output: `ramcensus_bf1.txt`.
3. **PCSX2 look.** A live session, isolated and under ci.lock, captured the class-select screen of every class:
   - BF1 KAS2 CW, CIS and Republic: `pcsx2/cis_classes_sheet.png`, `pcsx2/rep_classes_sheet.png`;
   - BF1 END1 GCW, Alliance and Empire: `pcsx2/end1_left_sheet.png`, `pcsx2/end1_imp_sheet.png`;
   - the super battle droid (SBD) in play (`pcsx2/sbd_ingame.png`, `10_sbd_idle_crop.png`) and the droideka as the
     player (`pcsx2/deka_sheet.png`).
4. **The port** — scratch example `uaud_roster.rs`, never committed; a copy is in this dir.
   - It runs `FactionRoster::pair` over the 26 BF1 mission RONs on main e69e73a.
   - It repeats `SoldierVisuals::load`'s per-geometry cache to report the gun each class is drawn with.
   - Output: `port_roster_bf1.txt`.
5. **Absence searches** for every "the port never" claim, over `engine/crates` including tests and examples (§5).
6. **The PS2 chase camera** — `camdist.py`. In each of the 22 states it reads:
   - the camera matrix at `*(0x522AC8)` (rows right, up, back, eye);
   - the player: the alive soldier whose track centre (feet at C+0x228, + 1.8 m) lies closest to the view axis.

   It reports the eye's distance behind that centre. Output: `camdist_bf1.txt`.

## 2. Which map and era loads which class [V: census_ia.md; PS2: ramcensus_bf1.txt]

Counts are the `AddUnitClass` minimums. Heroes are `SetHeroClass`. Locals are team 3; on hot1i it is team 4, the probe
droid.

| map (port key) | era (script) | team 1 | team 2 | hero T1 / T2 | locals (team 3+) |
|---|---|---|---|---|---|
| bes1 (bf1_bes1) | GCW (bes1a) | soldierurban 11, vanguard 5, pilot 5, marksman 4, smuggler 3 | storm_trooper 11, shock_trooper 5, pilottie 5, scout_trooper 4, dark_trooper 3 | lukeskywalker / darthvader | — |
| bes1 (bf1_bes1) | CW (bes1r) | clone_trooper 11, arc_trooper 5, clone_pilot 5, clone_sharpshooter 4, jet_trooper 3 | battledroid 11, assault 5, pilotdroid 5, assassindroid 4, droideka 3 | macewindu / countdooku | — |
| bes2 (bf1_bes2) | GCW (bes2a) | soldierurban 11, vanguard 3, pilot 4, marksman 4, smuggler 3 | storm_trooper 11, shock_trooper 3, pilottie 4, scout_trooper 4, dark_trooper 3 | lukeskywalker / darthvader | — |
| bes2 (bf1_bes2) | CW (bes2r) | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | battledroid 11, assault 3, pilotdroid 4, assassindroid 4, droideka 3 | macewindu / countdooku | — |
| end1 (bf1_end1) | GCW (end1a) | soldierjungle 11, vanguardjungle 3, pilot 4, marksmanjungle 4, smuggler 3 | storm_trooper 11, shock_trooper 3, pilotatst 4, scout_trooper 4, dark_trooper 3 | lukeskywalker / darthvader | trooper 4, repair 6 |
| geo1 (bf1_geo1) | CW (geo1r) | clone_trooper 14, arc_trooper 4, clone_pilot 5, clone_sharpshooter 5, jet_trooper 4 | battledroid 11, assault 3, pilotdroid 4, assassindroid 4, droideka 3 | macewindu / countdooku | geonosian 7 |
| hot1 (bf1_hot1) | GCW (hot1i) | storm_troopersnow 11, shock_troopersnow 3, pilotatat 4, scout_troopersnow 4, dark_troopersnow 3 | soldiersnow 11, vanguardsnow 3, pilot 4, marksmansnow 4, smugglersnow 3 | darthvader / lukeskywalkersnow | imp_droid_probe 1 |
| kam1 (bf1_kam1) | CW (kam1c) | battledroid 12, assault 3, pilotdroid 4, assassindroid 4, droideka 2 | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | countdooku / macewindu | — |
| kas1 (bf1_kas1) | CW (kas1c) | battledroid 14, assault 4, pilotdroid 5, assassindroid 5, droideka 4 | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | countdooku / macewindu | mechanic 2, rocketeer 2, warrior 3 |
| kas1 (bf1_kas1) | GCW (kas1i) | storm_trooper 14, shock_trooper 4, pilotatst 5, scout_trooper 5, dark_trooper 4 | soldier 11, vanguard 3, pilot 4, marksman 4, smuggler 3 | darthvader / lukeskywalker | mechanic 2, rocketeer 2, warrior 3 |
| kas2 (bf1_kas2) | CW (kas2c) | battledroid 14, assault 4, pilotdroid 5, assassindroid 5, droideka 4 | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | countdooku / macewindu | warrior 3, rocketeer 2, mechanic 2 |
| kas2 (bf1_kas2) | GCW (kas2i) | storm_trooper 14, shock_trooper 4, pilotatst 5, scout_trooper 5, dark_trooper 4 | soldier 11, vanguard 3, pilot 4, marksman 4, smuggler 3 | darthvader / lukeskywalker | warrior 3, rocketeer 2, mechanic 2 |
| nab1 (bf1_nab1) | CW (nab1c) | battledroid 12, assault 3, pilotdroid 4, assassindroid 4, droideka 2 | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | countdooku / macewindu | soldier 5, rider 2 |
| nab1 (bf1_nab1) | GCW (nab1i) | storm_trooper 11, shock_trooper 3, pilotatst 4, scout_trooper 4, dark_trooper 3 | soldier 11, vanguard 3, pilot 4, marksman 4, smuggler 3 | darthvader / lukeskywalker | soldier 5, rider 2 |
| nab2 (bf1_nab2) | GCW (nab2a) | soldierurban 11, vanguard 3, pilot 3, marksman 3, smuggler 2 | storm_trooper 11, shock_trooper 3, pilotatst 3, scout_trooper 3, dark_trooper 2 | lukeskywalker / darthvader | — |
| nab2 (bf1_nab2) | CW (nab2c) | battledroid 11, assault 3, pilotdroid 3, assassindroid 3, droideka 2 | clone_trooper 11, arc_trooper 3, clone_pilot 3, clone_sharpshooter 3, jet_trooper 2 | countdooku / macewindu | — |
| rhn1 (bf1_rhn1) | GCW (rhn1i) | storm_troopersnow 11, shock_troopersnow 3, pilotatat 4, scout_troopersnow 4, dark_troopersnow 3 | soldiersnow 11, vanguardsnow 3, pilot 4, marksmansnow 4, smugglersnow 3 | darthvader / lukeskywalkersnow | — |
| rhn1 (bf1_rhn1) | CW (rhn1r) | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | battledroid 11, assault 3, pilotdroid 4, assassindroid 4, droideka 3 | macewindu / countdooku | — |
| rhn2 (bf1_rhn2) | GCW (rhn2a) | soldiersnow 11, vanguardsnow 3, pilot 3, marksmansnow 3, smugglersnow 3 | storm_troopersnow 11, shock_troopersnow 3, pilotatat 3, scout_troopersnow 3, dark_troopersnow 3 | lukeskywalker / darthvader | — |
| rhn2 (bf1_rhn2) | CW (rhn2c) | battledroid 11, assault 3, pilotdroid 3, assassindroid 3, droideka 3 | clone_trooper 11, arc_trooper 3, clone_pilot 3, clone_sharpshooter 3, jet_trooper 3 | countdooku / macewindu | — |
| tat1 (tat1) | GCW (tat1i) | storm_trooper 5, shock_trooper 2, pilottie 3, scout_trooper 1, dark_trooper 1 | soldierdesert 11, vanguard 3, pilot 4, marksman 4, smuggler 1 | darthvader / lukeskywalker | tuskenraider 5, tuskenhunter 2 |
| tat1 (tat1) | CW (tat1r) | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | battledroid 11, assault 3, pilotdroid 4, assassindroid 4, droideka 3 | macewindu / countdooku | tuskenraider 5, tuskenhunter 2 |
| tat2 (NOT BAKED) | GCW (tat2i) | storm_trooper 11, shock_trooper 3, pilotatst 4, scout_trooper 4, dark_trooper 3 | soldierdesert 11, vanguard 3, pilot 4, marksman 4, smuggler 3 | darthvader / lukeskywalker | jawa 14 |
| tat2 (NOT BAKED) | CW (tat2r) | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | battledroid 11, assault 3, pilotdroid 4, assassindroid 4, droideka 3 | macewindu / countdooku | jawa 14 |
| yav1 (bf1_yav1) | CW (yav1c) | assault 12, battledroid 3, pilotdroid 4, assassindroid 4, droideka 2 | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | countdooku / macewindu | — |
| yav1 (bf1_yav1) | GCW (yav1i) | storm_trooper 11, shock_trooper 3, pilotatst 4, scout_trooper 4, dark_trooper 3 | soldierjungle 11, vanguardjungle 3, pilot 4, marksmanjungle 4, smuggler 3 | darthvader / lukeskywalker | — |
| yav2 (bf1_yav2) | GCW (yav2i) | storm_trooper 11, shock_trooper 3, pilotatst 4, scout_trooper 4, dark_trooper 3 | soldier 11, vanguard 3, pilot 4, marksman 4, smuggler 3 | darthvader / lukeskywalker | — |
| yav2 (bf1_yav2) | CW (yav2r) | clone_trooper 11, arc_trooper 3, clone_pilot 4, clone_sharpshooter 4, jet_trooper 3 | battledroid 11, assault 3, pilotdroid 4, assassindroid 4, droideka 3 | macewindu / countdooku | — |

- **The port's 26 BF1 RONs match the disc** for teams 1–2 on every mission: same classes in the same order, counts,
  `SetUnitCount`, `SetReinforcementCount` [V, a diff of each RON against `missions.py`]. YAV1 CW's fill class is
  `cis_inf_assault` (12), and the PS2 fields it that way: 12 and 10 assault droids in the two YAV1 states.
- **The PS2 fields exactly these classes** [V, ramcensus_bf1.txt; second verifier F6].
  - Each state's live classes are a subset of exactly one of its own map's IA missions, the era's. Across all 28
    missions the CW rosters repeat, so 17 of the 22 states would fit several.
  - Team-1/2 class records are at or below their `AddUnitClass` number. The one exception is KAS2 city hall: 12
    clone-trooper records against 11, of which 11 are alive.
  - The locals' first class is topped up to the team's `SetUnitCount`: END1 has 8 Ewok troopers against 4, TAT1 has
    10 Tusken raiders against 5.
  - Locals are present on END1 (14 Ewoks), GEO1 (7 Geonosians), KAS1/KAS2 (7 Wookiees), TAT1 (12 Tusken records) and
    BF1 TAT2 (13–14 Jawas).
  - `imp_droid_probe` (BASE droid) is outside the census's scan, which covers the soldier pool and the droideka vtable.
- **Coverage of the 22 states:** the 21 `bf1_*_spawn` states plus `tat1_cp0_cp10_postA_spawn`.
  - 20 are CW (the library took each map's first era); 2 are GCW (END1, HOT1).
  - Every REP and CIS team class, the droideka included, is present in all 20 CW states (14 maps).
  - The GCW urban, desert and standard variants are checked on the disc only.
- **No hero was live in any of the 22 states.** Heroes are loaded on every mission but none had spawned [V, RAM].
  Each state holds exactly one class object for each of its era's two heroes (vtable 0x5842B0, +0x14 = fnv(name)):
  Mace Windu and Dooku on CW, Luke and Vader on END1, snow Luke and Vader on HOT1. When BF1 spawns them is HERO's
  question.
- **Not fielded by the port:**
  - `cis_inf_droideka`: rejected by `ClassRoster::build`'s soldier-chain test (roster.rs 354) on all 13 CW RONs.
  - The five hero classes (four characters; Luke has a snow variant): no RON names a hero.
  - Every local: `MissionDef::locals` is carried, not consumed (mission.rs 14–16, 90–94). It holds only the first
    local class. END1, KAS1/2, NAB1 and TAT1 each author 2–3 local classes.
  - `imp_droid_probe` (hot1i team 4).
- **BF1 Mos Eisley is not a port map.** The port's `tat2` is BF2's (`scripts/rebake.sh` bakes `$B2/TAT/TAT2.LVL`), yet
  BF1's `TAT/TAT2.LVL`, `tat2i` and `tat2r` are on the disc and PCSX2 plays them (`ref/bf1_tat2_mos_eisley/`) → B1ME.

## 3. Per-class verdicts

The table is generated by `table.py` from `census_ia.json`, `port_roster_bf1.txt` and `ramcensus_bf1.txt`.
- **ch0 / ch1:** the disc's weapon slots on the primary and secondary cycles (`WeaponChannelN` = 1). Every soldier's
  slots 3–4 carry it; slots 1–2 never do.
- **drawn:** port vs disc.
- **anim banks:** each slot's weapon `AnimationBank`; the verdict is for the primary, since the port holds nothing else.
- **sounds:** the class sound properties the port never plays.
- **PS2 RAM:** in how many of the 22 states and 16 maps the class has a soldier record. The first draft counted
  state-name prefixes, so it showed 16 for the CW classes instead of 20 / 14. `imp_droid_probe` is not scanned.

| class | team | missions | port fields it | ch0 weapons (disc) | ch1 items (disc) | drawn (port vs disc) | movement | anim banks | sounds (port plays none of) | drops | PS2 RAM: states / maps (of 22 / 16) |
|---|---|--:|---|---|---|---|---|---|---|---|---|
| `all_inf_soldierurban` | Alliance | 3 | yes | inf_rifle + inf_pistol | inf_thermaldetonator + inf_crackgrenade | primary matches; skin `all_inf_trooperurban` missing | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 0 / 0 |
| `all_inf_vanguard` | Alliance | 9 | yes | inf_torpedo_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | primary matches | match | bazooka/pistol/grenade: primary needs `bazooka` | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 0 / 0 |
| `all_inf_pilot` | Alliance | 14 | yes | inf_shotgun + inf_pistol | inf_fusioncutter + inf_powerup_dispenser | primary matches | match | rifle/pistol/tool/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 2 / 2 |
| `all_inf_marksman` | Alliance | 9 | yes | inf_sniperrifle + inf_pistol | inf_thermaldetonator + inf_remotedroid | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death | ammo 0.05, health25 0.10, health100 0.10 | 0 / 0 |
| `all_inf_smuggler` | Alliance | 11 | yes | inf_bowcaster + inf_mortar_launcher | inf_remote_charge_dispenser | primary matches; skin override = the model's own textures (no-op); root scale 1.15 missing | match | rifle/grenade: primary matches | gap: Hurt,Death | ammo 0.40, health100 0.30, dual 0.10 | 1 / 1 |
| `all_inf_lukeskywalker` | Alliance | 12 | NO: hero | lightsaber | — | — | — | lightsabre: primary needs `lightsabre` | gap: LowHealth | ammo 0.20, health25 0.05, health100 0.0 | 0 / 0 |
| `imp_inf_storm_trooper` | Empire | 11 | yes | inf_rifle + inf_pistol | inf_thermaldetonator + inf_crackgrenade | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_shock_trooper` | Empire | 11 | yes | inf_torpedo_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | primary matches | match | bazooka/pistol/grenade: primary needs `bazooka` | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_pilottie` | Empire | 3 | yes | inf_mortar_launcher + inf_pistol | inf_fusioncutter + inf_powerup_dispenser | primary matches | match | rifle/pistol/tool/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 0 / 0 |
| `imp_inf_scout_trooper` | Empire | 11 | yes | inf_sniperrifle + inf_pistol | inf_thermaldetonator + inf_remotedroid | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_dark_trooper` | Empire | 11 | yes | inf_shotgun + inf_pistol | inf_thermaldetonator | primary matches | match; JetJump 21.0 not modelled | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget,Engine | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_darthvader` | Empire | 14 | NO: hero | lightsaber | — | — | — | lightsabre: primary needs `lightsabre` | gap: LowHealth | ammo 0.20, health25 0.05, health100 0.0 | 0 / 0 |
| `rep_inf_clone_trooper` | Republic | 14 | yes | inf_rifle + inf_pistol | inf_haywiredetonator + inf_crackgrenade | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `rep_inf_arc_trooper` | Republic | 14 | yes | inf_torpedo_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | primary matches; skin override = the model's own textures (no-op) | match | bazooka/pistol/grenade: primary needs `bazooka` | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `rep_inf_clone_pilot` | Republic | 14 | yes | inf_arccaster + inf_pistol | inf_fusioncutter + inf_powerup_dispenser | primary matches | match | rifle/pistol/tool/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `rep_inf_clone_sharpshooter` | Republic | 14 | yes | inf_sniperrifle + inf_pistol | inf_thermaldetonator + inf_remotedroid | draws `rep_weap_inf_rifle` not `rep_weap_inf_sniperrifle`; skin `rep_inf_sharpshooter` missing | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `rep_inf_jet_trooper` | Republic | 14 | yes | inf_rocket_launcher + inf_commando_pistol | inf_thermaldetonator | primary matches | match; JetJump 5.0 not modelled | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget,Engine | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `rep_inf_macewindu` | Republic | 14 | NO: hero | lightsaber | — | — | — | lightsabre: primary needs `lightsabre` | gap: LowHealth | ammo 0.20, health25 0.05, health100 0.0 | 0 / 0 |
| `cis_inf_battledroid` | CIS | 14 | yes | inf_wrist_blaster + inf_wrist_shrapnel | inf_wrist_rocket | primary matches | match | sbdroid: primary needs `sbdroid` | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `cis_inf_assault` | CIS | 14 | yes | inf_torpedo_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | primary matches; skin `cis_inf_battledroid_blue` missing | match | bazooka/pistol/grenade: primary needs `bazooka` | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `cis_inf_pilotdroid` | CIS | 14 | yes | inf_mortar_launcher + inf_pistol | inf_fusioncutter + inf_powerup_dispenser | draws `cis_weap_inf_launcher` not `com_weap_inf_torpedo` | match | bdroid/pistol/tool/grenade: primary needs `bdroid` | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `cis_inf_assassindroid` | CIS | 14 | yes | inf_sniperrifle + inf_pistol | inf_thermaldetonator + inf_remotedroid | draws `cis_weap_inf_launcher` not `cis_weap_inf_sniperrifle`; skin `cis_inf_battledroid_red` missing | match | bdroid/pistol/grenade: primary needs `bdroid` | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `cis_inf_droideka` | CIS | 14 | NO: walkerdroid | inf_repeater + walk_droideka_shield | — | — | — | bdroid: primary needs `bdroid` | gap: Death,Engine | ammo 0.40, health100 0.30, dual 0.10 | 20 / 14 |
| `cis_inf_countdooku` | CIS | 14 | NO: hero | lightsaber | — | — | — | lightsabre: primary needs `lightsabre` | gap: LowHealth | ammo 0.20, health25 0.05, health100 0.0 | 0 / 0 |
| `all_inf_soldierjungle` | Alliance | 2 | yes | inf_rifle + inf_pistol | inf_thermaldetonator + inf_crackgrenade | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 1 / 1 |
| `all_inf_vanguardjungle` | Alliance | 2 | yes | inf_torpedo_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | primary matches | match | bazooka/pistol/grenade: primary needs `bazooka` | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 1 / 1 |
| `all_inf_marksmanjungle` | Alliance | 2 | yes | inf_sniperrifle + inf_pistol | inf_thermaldetonator + inf_remotedroid | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death | ammo 0.40, health100 0.30, dual 0.10 | 1 / 1 |
| `imp_inf_pilotatst` | Empire | 8 | yes | inf_mortar_launcher + inf_pistol | inf_fusioncutter + inf_powerup_dispenser | primary matches | match | rifle/pistol/tool/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `ewk_inf_trooper` | locals | 1 | NO: local/other team | inf_spear + inf_tool | inf_rock | root scale 0.5 missing | — | grenade/tool: primary needs `grenade` | gap: Hurt,Death,LowHealth | — | 1 / 1 |
| `ewk_inf_repair` | locals | 1 | NO: local/other team | inf_tool + inf_rock | — | skin `Ewok2` missing; root scale 0.5 missing | — | tool/grenade: primary needs `tool` | gap: Hurt,Death,LowHealth | — | 1 / 1 |
| `geo_inf_geonosian` | locals | 1 | NO: local/other team | inf_blunderbuss | — | — | —; JetJump 8.0 not modelled | rifle: primary matches | gap: Hurt,Death,Engine | ammo 0.05, health25 0.0, health100 0.20 | 1 / 1 |
| `imp_inf_storm_troopersnow` | Empire | 3 | yes | inf_rifle + inf_pistol | inf_thermaldetonator + inf_crackgrenade | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_shock_troopersnow` | Empire | 3 | yes | inf_torpedo_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | draws `imp_weap_inf_rifle` not `imp_weap_inf_launcher` | match | bazooka/pistol/grenade: primary needs `bazooka` | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_pilotatat` | Empire | 3 | yes | inf_mortar_launcher + inf_pistol | inf_fusioncutter + inf_powerup_dispenser | primary matches | match | rifle/pistol/tool/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_scout_troopersnow` | Empire | 3 | yes | inf_sniperrifle + inf_pistol | inf_thermaldetonator + inf_remotedroid | draws `imp_weap_inf_rifle` not `imp_weap_inf_sniperrifle` | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `imp_inf_dark_troopersnow` | Empire | 3 | yes | inf_shotgun + inf_pistol | inf_thermaldetonator | primary matches | match; JetJump 21.0 not modelled | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget,Engine | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `all_inf_soldiersnow` | Alliance | 3 | yes | inf_rifle + inf_pistol | inf_thermaldetonator + inf_crackgrenade | primary matches; skin `Rebel_face01` missing | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 1 / 1 |
| `all_inf_vanguardsnow` | Alliance | 3 | yes | inf_torpedo_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | draws `all_weap_inf_rifle` not `all_weap_inf_launcher`; skin `all_inf_vanguard_hoth`,`Rebel_face02` missing | match | bazooka/pistol/grenade: primary needs `bazooka` | gap: Hurt,Death,AcquiredTarget | ammo 0.05, health25 0.10, health100 0.10 | 1 / 1 |
| `all_inf_marksmansnow` | Alliance | 3 | yes | inf_sniperrifle + inf_pistol | inf_thermaldetonator + inf_remotedroid | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death | ammo 0.40, health100 0.30, dual 0.10 | 1 / 1 |
| `all_inf_smugglersnow` | Alliance | 3 | yes | inf_bowcaster + inf_mortar_launcher | inf_remote_charge_dispenser | primary matches; skin `wookie_snow`,`wookieface02_snow` missing; root scale 1.15 missing | match | rifle/grenade: primary matches | gap: Hurt,Death | ammo 0.40, health100 0.30, dual 0.10 | 1 / 1 |
| `all_inf_lukeskywalkersnow` | Alliance | 2 | NO: hero | lightsaber_blue | — | — | — | lightsabre: primary needs `lightsabre` | gap: LowHealth | ammo 0.20, health25 0.05, health100 0.0 | 0 / 0 |
| `imp_droid_probe` | None | 1 | NO: local/other team | droid_probe_blaster | — | — | — | rifle: primary matches | — | ammo 0.15, health25 0.0, health100 0.0 | 0 / 0 |
| `wok_inf_mechanic` | locals | 4 | NO: local/other team | inf_bowcaster + inf_fusioncutter | — | skin `wookie`,`wookieface` missing; root scale 1.15 missing | — | rifle/tool: primary matches | gap: Hurt,Death | ammo 0.20, health25 0.05, health100 0.0 | 5 / 2 |
| `wok_inf_rocketeer` | locals | 4 | NO: local/other team | inf_torpedo_launcher + inf_bowcaster | — | skin `wookie03`,`wookieface03` missing; root scale 1.15 missing | — | bazooka/rifle: primary needs `bazooka` | gap: Hurt,Death | ammo 0.20, health25 0.05, health100 0.0 | 5 / 2 |
| `wok_inf_warrior` | locals | 4 | NO: local/other team | inf_bowcaster | — | skin `wookie02`,`wookieface02` missing; root scale 1.15 missing | — | rifle: primary matches | gap: Hurt,Death | ammo 0.20, health25 0.05, health100 0.0 | 5 / 2 |
| `all_inf_soldier` | Alliance | 4 | yes | inf_rifle + inf_pistol | inf_thermaldetonator + inf_crackgrenade | primary matches | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 0 / 0 |
| `gun_inf_soldier` | Locals | 2 | NO: local/other team | inf_haywiredetonator | — | skin `gungan` missing; root scale 1.1 missing | — | grenade: primary needs `grenade` | gap: Hurt,Death,AcquiredTarget,LowHealth | — | 0 / 0 |
| `gun_inf_rider` | Locals | 2 | NO: local/other team | inf_haywiredetonator | — | skin `gungan02` missing; root scale 1.1 missing | — | grenade: primary needs `grenade` | gap: Hurt,Death,AcquiredTarget,LowHealth | — | 0 / 0 |
| `all_inf_soldierdesert` | Alliance | 2 | yes | inf_rifle + inf_pistol | inf_thermaldetonator + inf_crackgrenade | primary matches; skin `all_inf_trooperdesert` missing | match | rifle/pistol/grenade: primary matches | gap: Hurt,Death,AcquiredTarget | ammo 0.40, health100 0.30, dual 0.10 | 0 / 0 |
| `tat_inf_tuskenraider` | locals | 2 | NO: local/other team | inf_tuskenrifle | — | — | — | —: primary matches | gap: Hurt,Death,AcquiredTarget | — | 1 / 1 |
| `tat_inf_tuskenhunter` | locals | 2 | NO: local/other team | inf_tuskensniperrifle | — | — | — | rifle: primary matches | gap: Hurt,Death,AcquiredTarget | — | 1 / 1 |
| `tat_inf_jawa` | locals | 2 | NO: local/other team | inf_jawa_fusioncutter | — | root scale 0.6 missing | — | tool: primary needs `tool` | gap: Hurt,Death,AcquiredTarget,LowHealth | ammo 0.5, health25 0.4, health100 0.1 | 2 / 1 |

### 3.1 Weapon slots [V disc; V RAM; V PCSX2]

- Every BF1 soldier class names its weapons as `WeaponName1..4`, `WeaponAmmoN` and `WeaponChannelN` (1 on slots 3–4)
  [V, verifier c3.py over all 28 IA missions]. The walkerdroid and the probe droid use unnumbered `WeaponName`.
- The PS2 instantiates every slot, in slot order, on every soldier. For example, all 11 clone troopers in the KAS2
  state hold rifle, pistol, haywire detonator, crack grenade [V, RAM; verifier C11].
- The class-select screens name the same equipment, class by class. For example, SUPER BATTLE DROID lists Wrist
  Blaster, Tri Shot, Wrist Rocket; DROIDEKA lists Repeating Blasters, Shield Emitter.
- The on-screen names are the localised ones:
  - Blaster Cannon / Blast Cannon: the rebel pilot's / dark trooper's `*_shotgun`;
  - Radiation / Mortar / Grenade Launcher: the mortars;
  - Ion Bolt Caster: `rep_weap_inf_arccaster` (it reads "ON BOLT CASTER");
  - EMP Launcher: `rep_weap_inf_rocket_launcher`;
  - Concussion / EMP Grenade: crack / haywire;
  - Recon Droid, Time Bomb, Dispenser Health & Ammo.
- The port takes `.first()` of `WeaponName1` (else `WeaponName`) and its `WeaponAmmo1` (roster.rs 308–323). No
  runtime code reads `WeaponName2+` or `WeaponChannelN`; the only hits are a `#[cfg(test)]` block in
  `bake/src/classes.rs` [V, search; verifier C13]. → GREN.
- **The primary's ordnance kind.** The port flies every primary as a straight bolt:
  - `OrdnanceClass` (`core/src/weapon.rs` 99–115) has no field for a kind, gravity, turn rate or guidance;
  - `from_props` (178–181) says "Gravity is deliberately NOT read … bolts fly straight".

  The disc's ordnance BASEs [V; second verifier F2]:
  - `missile`: the torpedo launchers of all four vanguard/shock/ARC/assault classes (7 classes with the jungle and
    snow variants; their ordnance authors TurnRate 2.0, Gravity 0), plus the Wookiee rocketeer's and the Geonosian's
    (locals);
  - `shell` (Gravity 1.75): the IMP pilots' and the CIS pilot droid's mortar launcher;
  - `emitterordnance`: the clone pilot's arccaster;
  - `bullet`: the jet trooper's EMP launcher;
  - `bolt`: every rifle, pistol, sniper rifle, blaster cannon, bowcaster and wrist blaster.

  So the port has 13 of 34 fielded classes firing a primary whose flight it does not model [V, census_ia.md
  "Weapons"] → UORD.

### 3.2 What each draws: the weapon model [V code; measured]

- `ClassRoster` resolves the right model for every class: the primary's `GeometryName`, which may be borrowed. The SBD's
  wrist blaster has none, and the port correctly draws no gun [V, port_roster_bf1.txt].
- **The battle draws the wrong one for 6 classes.** `SoldierVisuals::load` caches assets by `<game>/<GeometryName>`
  (soldier_vis.rs 128–131). The asset carries the first class's hand weapon (`load_one` 216–250), and `weapon_draws`
  draws `sa.weapon` for every class that shares it (386–395) [V; verifier C14].
- The table below is measured over the 26 RONs by the port's own roster. The second verifier re-derived it from the
  disc and the RON order (F1). The four shared bodies take their gun from these first loaders:
  - `rep_inf_trooper`: `rep_inf_clone_trooper`;
  - `cis_inf_bdroid`: `cis_inf_assault`, on every CW RON;
  - `imp_inf_snowtrooper`: `imp_inf_storm_troopersnow`;
  - `all_inf_snowtrooper`: `all_inf_soldiersnow`.

| class | draws | should draw | missions |
|---|---|---|---|
| `rep_inf_clone_sharpshooter` | `rep_weap_inf_rifle` | `rep_weap_inf_sniperrifle` | 13 CW |
| `cis_inf_pilotdroid` | `cis_weap_inf_launcher` | `com_weap_inf_torpedo` | 13 CW |
| `cis_inf_assassindroid` | `cis_weap_inf_launcher` | `cis_weap_inf_sniperrifle` | 13 CW |
| `imp_inf_shock_troopersnow` | `imp_weap_inf_rifle` | `imp_weap_inf_launcher` | hot1, rhn1, rhn2 |
| `imp_inf_scout_troopersnow` | `imp_weap_inf_rifle` | `imp_weap_inf_sniperrifle` | hot1, rhn1, rhn2 |
| `all_inf_vanguardsnow` | `all_weap_inf_rifle` | `all_weap_inf_launcher` | hot1, rhn1, rhn2 |

  → UWPN. The unit gallery's own guess (UGAL) is a separate path: BF1's `cis_inf_sbdroid` golden holds
  `cis_weap_inf_sniperrifle`.

### 3.3 What each draws: skins and scale [V disc, V ELF, V model data]

> 2026-10-16 (card USKN, `re-soldier-skeleton-skins.md` [O]): the live soldier's override draw is `FUN_002013a0` (two
> model draws); `FUN_001be1e8` below is another object's. SkeletonRootScale is not a mesh scale: the Wookiee (and the
> droids, Gungans, Jawa, Ewoks) look wrong in the port because it poses them at the human clips' bone lengths, where the
> PS2 keeps the class rig's.

- **16 IA classes author `OverrideTexture` / `OverrideTexture2`** [V, census]. BF1's soldier-class setter
  `FUN_0020f2e0` stores them:
  - OverrideTexture at +0x18d0 (line 122890);
  - OverrideTexture2 at +0x18d4 (line 122865).
- **Which texture an override replaces** [V; found by the second verifier (F5), re-read here]:
  - The draw is BF1's `FUN_001be1e8`, a vtable method: its one reference is the data word at 0x52e0b4, in the
    vtable 0x52e088 that constructor `FUN_001bdfd0` installs (B1 65496). Its object's +0x134 is the soldier class
    [D: +0x18d0 / +0x18d4 are the soldier setter's two override slots].
  - It tests class+0x18d0 (B1 65890). When that is set, it builds an override list on the model (`FUN_004985f0`,
    65898).
  - It sets the model group fnv("override_texture") = 0xdc5803e9 to OverrideTexture (65899–65902). When +0x18d4 is
    set, it sets group fnv("override_texture2") = 0xb98e11c1 to OverrideTexture2 (65904–65907).
  - Each disc model segment names its group in an `MNAM` chunk (`verify2/modl.py`):
    - `rep_inf_trooper` and `cis_inf_bdroid`: their one segment is `override_texture`;
    - `all_inf_soldier`: only the `All_Inf_trooper` body is, not the face or the packs;
    - `all_inf_snowtrooper`: the body is `override_texture`, the face `override_texture2`;
    - `rep_inf_arctrooper`: the body is `override_texture`, the pack `override_texture2`.
  - The port's baked soldier GLBs keep the texture names as material names, not the MNAM slot.
- Two classes' overrides therefore change nothing: the ARC trooper's and the smuggler's (`wookie` / `wookieface02`)
  name their model's own slot textures. Neither does the snow soldier's body override; its face override does.
- The changes the PS2 shows and the port does not:

| class | override (slot) | model's texture in that slot |
|---|---|---|
| sharpshooter | `rep_inf_sharpshooter` (red), body | `rep_inf_trooper` |
| assault droid | `cis_inf_battledroid_blue`, body | `cis_inf_battledroid` |
| assassin droid | `cis_inf_battledroid_red`, body | `cis_inf_battledroid` |
| urban / desert rebel | `all_inf_trooperurban` / `all_inf_trooperdesert`, body | `All_Inf_trooper` |
| snow vanguard | `all_inf_vanguard_hoth` body + `Rebel_face02` face | `ALL_inf_soldier_HOTH` / `ALL_face01` |
| snow soldier | face `Rebel_face01` | `ALL_face01` |
| snow smuggler | `wookie_snow` + `wookieface02_snow` | `wookie` / `wookieface02` |

  The locals carry the Wookiee, Gungan and Ewok variants. The PS2 class-select frames show the sharpshooter's red and
  the assault/assassin droids' blue/red markings (cis_classes_sheet, rep_classes_sheet). Each override texture lives in
  the same sublevel as its class [V].
- **`SkeletonRootScale`** is `%f` into +0x1e5c (line 123186). When it is ≠ 1.0, and the class's `SkeletonName` (+0x1d80)
  is set and is not the default human skeleton, `FUN_00441ca8(scale, skeleton, FUN_00444bb8("bone_root"))` scales
  `bone_root` [V, B1 123186–123195; string at 0x532a40 = `bone_root`; `FUN_00444bb8` is a name hash, which one is
  not checked].
- `CollisionRootScale` goes to +0x1e60 (123572). Values:

| scale | classes |
|---|---|
| 1.15 | Wookiee smuggler; the three Wookiee locals |
| 1.1 | Gungans |
| 0.6 | Jawa |
| 0.5 | Ewoks |

  The port reads neither key: the Wookiee smuggler is drawn at 1.0 on 14 GCW missions (13 of them port maps)
  [V, search] → USKN.

### 3.4 Items, grenades, drops [V disc, V RAM]

- **Secondary cycle (ch1) per class** (the RAM confirms it for every class in §2):

| items | classes |
|---|---|
| thermal detonator + crack (concussion) grenade | troopers |
| haywire (EMP) + crack | clone trooper |
| thermal + mine dispenser | vanguard, shock, ARC, assault |
| thermal + remote droid (recon) | marksman, scout, sharpshooter, assassin |
| fusion cutter + powerup dispenser (health & ammo) | pilots |
| wrist rocket | SBD |
| remote charge dispenser (time bomb) | Wookiee smuggler; its mortar "Grenade Launcher" is ch0 slot 2 |
| thermal only | dark trooper, jet trooper |

- The mine is `com_weap_inf_landmine` (BASE mine, COMMON.LVL), the recon droid `*_remotedroid_ord` (BASE droid), the
  thrown grenades BASE sticky. The port models none of them → GREN.
- **Drops** [V; second verifier F3]:
  - Every team-1/2 class and every hero authors a `DropItemClass` / `DropItemProbability` / `NextDropItem` list,
    read by the setter (0x406d2b75, line 123060).
  - Three profiles cover them; the dual pack is `com_item_powerup_dual`:

| ammo | health25 | health100 | dual | classes |
|---|---|---|---|---|
| 0.40 | — | 0.30 | 0.10 | 22: every ALL, REP and CIS team class except the three below, the droideka included (PWUP 2026-10-02: the droideka only AUTHORS the keys — it is not a soldier class, the engine never reads them and it drops nothing; `re-powerups.md` §3.1) |
| 0.05 | 0.10 | 0.10 | — | 13: the 11 IMP classes; the ALL marksman of the basic/urban/desert sets; the ALL snow vanguard |
| 0.20 | 0.05 | 0.0 | — | the 5 hero classes; the 3 Wookiee locals |

  - Other locals: the Geonosian authors 0.05 / 0.0 / 0.20, the probe droid 0.15 / 0 / 0, the Jawas 0.5 / 0.4 / 0.1.
    The Ewoks, Gungans and Tuskens author none.
  - The port drops nothing → PWUP.

### 3.5 Sounds [V disc, V ELF, V code; H when the PS2 plays each]

| class property | what | port |
|---|---|---|
| `HurtSound`, `DeathSound` (setter `FUN_00221e48` 135984 / 136071) | `*_inf_com_chatter_wound` / `_death`, `_female` for the marksmen, `_wookie`, `droid_death`, `tusken_*`, `gungan_*`, `end_inf_ewok_*`, `geo_inf_*` | not played |
| `AcquiredTargetSound`, `HidingSound`, `ApproachingTargetSound`, `FleeSound`, `PreparingForDamageSound`, `HeardEnemySound` (`FUN_0020f2e0`, e.g. 123048) | AI combat chatter | not played |
| ~26 unnamed keys | `<side>_inf_com_chatter_tac_*` and `ai_<side>_…_tac_*`; the Wookiee smuggler uses `wok_inf_…` | the port plays the FACTION's `_tac_follow/hold/spreadout/allclear` on a squad order (windowed.rs 82–108) — per-class voice missing |
| `LowHealthSound` (123549) / `LowHealthThreshold` | saber hum, Vader's breath, Ewok breathing, Gungan heartbeat | not played |
| `EngineSound` | `rep_inf_Jetpack_engine_parameterized` (jet / dark trooper), Geonosian wings, droideka engine | not played |
| weapon `JumpSound`, `LandSound`, `RollSound`, `ProneSound`, `SquatSound`, `StandSound`, `ChangeModeSound`, `WeaponChangeSound`, `FireEmptySound` | the `*_mvt_*` foley per weapon | not played |
| weapon `FireSound`, `ReloadSound`; ordnance collision sound | — | **played** (windowed.rs 30–70) |

- The names are baked: BF1 KAS2's `sounds/names.ron` holds `all_inf_com_chatter_wound`, `droid_death`,
  `all_weap_inf_rifle_mvt_jump`, `rep_weap_jetpack_turnon` and the rest [V, grep]. Only the triggers are missing.
- **When** the PS2 plays each one (on damage, on death, on the AI's perception events, the throttles) is [H] → USND
  (RE first).

### 3.6 Movement constants [V disc vs V port]

- The port reads MaxSpeed, MaxStrafeSpeed, MaxTurnSpeed, MaxPitchSpeed and `Acceleraton` (BF1's spelling) through
  `SoldierConstants::from_props` (`core/src/soldier.rs` 112–142). All 34 fielded BF1 classes equal the disc, 0
  differences over MaxSpeed, strafe, turn, acceleration, MaxHealth and clips (`port_roster_bf1.txt` vs
  `census_ia.json`, re-run by the second session):
  - MaxSpeed: 6.5 for the riflemen, the SBD and the jet trooper; 6.0 for the launcher classes, the dark trooper and the Wookiee; 7.0 for pilots and snipers;
  - strafe 4.5, turn 4.0, acceleration 70;
  - MaxHealth: 300, 240 for the snipers and the jet trooper, 450 for the Wookiee;
  - WeaponAmmo1 clips.
- BF1 classes author no `JumpHeight`. The port's BF2 jump (LOCKED feature set) stands.
- `AimValue` and `AimFactor*` are consumed nowhere in either ELF (weapon.rs 45–47, the prior RE).
- **Jet and jump packs: GAP** [V; second verifier F4].
  - The setter compares JetJump (0xc4b2cb20) at line 123562 and stores it at +0x18d8 on 123563. It compares JetPush at
    123503 and stores it at +0x18dc.
  - The port turns `JetJump > 0` into a route-planning flag only: roster.rs 396 → `ai/route.rs` 30 → `nav.rs` 144,
    where a JET_JUMP arc is usable only by a jet mover.
  - No code reads JetPush or any JetFuel* key. The only jet-fuel hit is the `infinite_jet_fuel` cheat toggle.
  - Values (the fuel columns are JetFuelCost / JetFuelInitialCost / JetFuelRechargeRate):

| class | JetJump | JetPush | fuel | notes |
|---|---|---|---|---|
| dark trooper (+ snow) | 21.0 | 0.0 | 0.0 / 0.5 (snow 0.4) / 0.025 | a jump pack, effect `darktrooper_jetpack` |
| jet trooper | 5.0 | 8.0 | 0.2 / 0.25 / 0.1 | `JetType hover`, a true jet pack |
| Geonosian (local) | 8.0 | 8.0 | 0.1 / 0.2 / 0.03 | wings: `JetType hover`, `JetDuration 5.0`, effect `geo_wingsfly` |

  PLAN locks "Jet/jump-pack: BF2 feel" → JETP.
- **The chase camera: GAP** [V disc, V PS2 RAM, V code; second verifier F7].
  - 33 of the 52 IA classes author one CAMERASECTION set:
    - STAND: eye and TrackCenter y 1.8, TrackOffset (0, 0, 3.5), TiltValue 5;
    - CROUCH: 1.3, (0, 0.15, 3.5);
    - PRONE: 0.5, (0, 0, 3.0);
    - the zoom sections: (0.4, 0.05 or 0.2, 2.8), tilt 3.5.
  - 8 other sets: the Wookiee smuggler (STAND centre 2.25, offset 4.0), the droideka (0.5 up, 4.5 back), the jet
    trooper (tilt 7.5), the locals.
  - On the PS2, in 19 of the 22 states the eye sits exactly 3.50 m behind the player's track centre, along the view
    axis, at pitch −9.01° (`camdist_bf1.txt`). The other 3 (2.99, 3.22, 3.35 m) are still on the axis: [H] pulled in by
    geometry behind the player.
  - The port's chase camera is one stance table, `camera_section` (`game/src/walk.rs` 41–49): stand (1.8, 0, 3.2),
    crouch (1.3, 0.15, 3.0), prone (0.5, 0, 3.0), sprint (1.8, 0, 4.0). Its doc names BF2's `com_inf_default` as
    the source. It serves every class of both games; TrackCenter/TrackOffset are read only for vehicle seats.
  - So every BF1 soldier is framed 0.3 m too close standing and 0.5 m too close crouched → **CAMS**. CFOVM keeps the
    FOV per mode.

### 3.7 Animations [V disc; V PCSX2]

- Every soldier gets the same 34 `human_rifle_*` clips (`bake/src/soldier.rs` 155–190, banks filtered on `human`
  at 281) [V]. Each weapon names an `AnimationBank`, read by `FUN_002864f0` 210216 [V].
- BF1 COMMON.LVL has 8 `human_animation_*` banks with 262 entries, 261 distinct clips; one unnamed clip sits in two
  banks. Matched against candidate names, they hold at least the counts below. 142 clips match no candidate:

| clip set | clips |
|---|---|
| `human_rifle_*` | 43 (≥ 64 by the verifier's wider list) |
| `human_bazooka_*` | 23 |
| `human_tool_*` | 20 |
| `human_sbdroid_*` | 20 |
| `human_pistol_*` | 6 |
| `human_bdroid_*` | 5: stand idle_emote, walkforward, runforward, shoot, reload |
| `human_grenade_*` | 2 |

  [V, anims.py, re-run by the second session; verifier C7 and F9]. No candidate lightsabre clip is in COMMON.
- On the PS2 the SBD holds both forearms out at waist height (`pcsx2/10_sbd_idle_crop.png`), not a rifle pose.
- The port's rifle-pose SBD, launcher troopers and bdroid snipers/pilots are therefore wrong for 10 of 34 fielded
  classes before any secondary weapon exists → ANIM.

## 4. Gaps → cards

- **New cards:**

| card | milestone / value | what |
|---|---|---|
| **DEKA** | M1 / 4 | the droideka as a fielded CIS class |
| **UWPN** | M1 / 4, build | each class draws its own gun |
| **USKN** | M1 / 4 | OverrideTexture/2 and SkeletonRootScale |
| **USND** | M3 / 3 | unit voice and foley |
| **JETP** | M3 / 3 | jet and jump packs |
| **UORD** | M3 / 3 | missile, shell, emitter and bullet primaries |
| **B1ME** | M1 / 3 | BF1 Mos Eisley |
| **CAMS** | M1 / 3, build | the chase camera per class and game (second session) |

- **Notes on existing cards:**
  - GREN: the per-class slot table and the value;
  - PWUP: the three drop profiles;
  - ANIM: the bank census and a raise to 4;
  - HERO: the per-mission hero classes, AddHealth 1e6, NoEnterVehicles 1, none live in the states;
  - MSND and TEAMS: the full locals class lists, their scales and skins, and that the RON keeps one class;
  - SPSEL: the PS2's class and equipment names;
  - CFOVM: per-class camera sections; the distance moved to CAMS;
  - KNOCK: the four droid classes' `ChunkFrequency 0.6` / `com_inf_droid_exp`;
  - UGAL: the battle-side twin UWPN;
  - USKN (second session): the override-slot question answered (§3.3), so its RE keeps only the root scale and
    BF2's twin.

## 5. Checks run

### 5.1 The RE checks

- **Check 1** (`re_check.py fn b1 …`, re-run by the second session):
  - lines 122890, 122865, 123186, 123572, 123048, 123562, 123503, 123060, 122860, 122997, 123077 and 123549 are in
    `FUN_0020f2e0`;
  - 135984 and 136071 are in `FUN_00221e48`;
  - 210216 is in `FUN_002864f0`;
  - 156007 (NoEnterVehicles) is in `FUN_0023da58`;
  - 65896, 65899 and 65904 (the override draw) are in `FUN_001be1e8`; 65496 (its vtable 0x52e088) is in
    `FUN_001bdfd0`.
  - Every compared hash was recomputed from its name: OverrideTexture 0x09db74ac, OverrideTexture2 0x227894ba,
    SkeletonRootScale 0x4cacc3d0, CollisionRootScale 0xc64a53ab, AcquiredTargetSound 0x399ee477, JetJump 0xc4b2cb20,
    JetPush 0xf0b75e90, DropItemClass 0x406d2b75, WeaponChannel3 0x2cfc4381, LowHealthSound 0xf8d3e5d2, HurtSound
    0x72981af7, DeathSound 0x6a9d502c, AnimationBank 0xa7678f8f, NoEnterVehicles 0x939b9985, ChunkFrequency
    0x43b50014, override_texture 0xdc5803e9, override_texture2 0xb98e11c1.
- **Check 2** (absence): these searches over `engine/crates` (tests and examples included) found no runtime hit:
  - `OverrideTexture`, `SkeletonRootScale`, `CollisionRootScale`, `DropItem`, `powerup`, `AnimationBank`,
    `HurtSound`, `DeathSound`, `AcquiredTargetSound`, `LowHealth`, `EngineSound`, `JumpSound`;
  - `WeaponName[2-9]` and `WeaponChannel` (test code only);
  - also each hash in signed, unsigned and case variants (verifier C15).
- **Check 3** (the twin): BF2 is UAUD2's. The same port paths serve both games, so UWPN, USKN, USND, JETP and UORD apply
  to BF2 too — UAUD2 checks its classes.
- **Check 4:** `re_check.py word b1 0x532a40 4 --str` = `bone_root`; 0x532ae8 = `%f`; 0x532b08 = `%d`. The ELF word at
  0x52e0b4 is 0x001be1e8, the only data reference to the override draw.
- **Check 5:** no flag-gated branch is claimed. SkeletonRootScale's apply path needs `SkeletonName` set first; the
  smuggler authors SkeletonName before the scale [V, verifier].
- **First verifier** (first session): one foreground Opus agent, given claims and citations only
  (`verify_claims.md`, C1–C16). Result: 11 SUPPORTED, 5 PARTLY, 0 NOT SUPPORTED. The five corrections are folded in:
  - C3: the droids use unnumbered `WeaponName`, and the channel appears only on slots 3–4;
  - C4: the hash is `|0x20`, not lowercase;
  - C9: the SkeletonName guard;
  - C13: the line number;
  - C15: windowed.rs also plays door sounds.

  Its per-claim verdict text was lost with that session (API error); only this summary survives.
- **Second verifier** (second session): one foreground Opus agent, claims and citations only (`verify_claims_v2.md`,
  F1–F10), verdicts in `verify2/VERDICTS.md`. It wrote its own readers for the disc, the baked class files, the GLBs,
  the bytecode dumps and EE RAM, and recounted everything, including all 22 states.
  - Result: 8 SUPPORTED (F1–F5, F8–F10; three with notes), 2 PARTLY (F6, F7), 0 NOT SUPPORTED.
  - Every correction is folded in above and was re-derived by the second session:
    - F6: "one mission" means one of the state's own map's missions; the locals' top-up and KAS2's dead 12th record;
    - F7: the camera script took a bystander in YAV1 viaduct; picking by the view axis gives 19 of 22 at 3.50 m, not 18;
    - F4: JetJump's store line;
    - F5: the GLBs hold material names only. The verifier also found the override draw (§3.3).

### 5.2 What the second session re-derived and changed

- **Reproduced:**
  - all 26 RONs equal the bytecode's teams 1–2 (classes, counts, units, tickets);
  - movement constants: 34 classes, 0 differences;
  - the 6 wrong-gun classes (`port_roster_bf1.txt` MISMATCH lines);
  - the ordnance bases: 7 missile, 4 shell, 1 emitter, 1 bullet;
  - the drop profiles;
  - the jet props;
  - the override-vs-material table;
  - the camera sets;
  - the hero props;
  - anims.py's clip counts;
  - every code citation, at main 6ea549d (VTURB touched none of the cited files);
  - the absence searches, by name and by hash;
  - the PCSX2 class-select sheets, viewed: every equipment list, the sharpshooter's red, the assassin droid's red,
    the assault droid's blue.
- **Corrected from the first draft:**
  1. 6 wrong-gun classes, not 5;
  2. the RAM column now counts states / maps. It counted state-name prefixes, 16 instead of 20 / 14;
  3. the Geonosian's jet row (JetPush 8.0 and its fuel, not "—");
  4. drops are authored by team-1/2 classes, heroes and some locals, not "every class";
  5. the no-op overrides are two classes plus one slot, not "3 of the 16", and the model textures of the snow rows
     are filled in;
  6. the RAM counts sentence (see F6).
- **Added:** the override-slot mechanism (§3.3, [V]) and the chase-camera gap (§3.6 → CAMS).

## 6. What stayed [H]

- When the PS2 plays each class sound.
- Why 3 of the 22 camera states sit closer than 3.50 m (geometry behind the player is the guess), and what the soldier
  camera's `TiltValue` does.
- The property 0x891891e9 that the override draw sets on each group (a texture slot, from the code's shape).
- When a BF1 hero spawns.
- How each ordnance kind flies.
- Whether the Wookiee's 1.15 `bone_root` scale also scales its speed or camera.
- The unnamed tac-chatter keys' exact names (the values are the sound names).
