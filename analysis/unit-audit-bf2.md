# OPUS RE (2026-09-29) - [O] UNAUDITED — Unit fidelity audit, BF2 (card UAUD2)

The maintainer's DECISION 3 (INBOX 2026-09-28): audit every unit class on every map of both games. This document covers BF2;
`analysis/unit-audit-bf1.md` covers BF1. It lists every class the 33 BF2 conquest scripts (`*_con`) load and checks
each one column by column against three sources:
- **the disc:** the mission bytecode, plus the side-LVL sublevels and INGAME.LVL each script reads, resolved the way
  the PS2 resolves a BF2 class (§1.1);
- **the PS2 itself:** the class objects and live soldiers in the RAM of the BF2 save states, and the class-select
  screens of all four factions;
- **the port as it runs:** main at 144b3df, through its own `FactionRoster::pair` and `SoldierVisuals` cache. Code
  citations are re-checked at 781acbd; UGAL and PADM landed during the session and change none of the measured columns.

Tags: [V] read and cited; [D] derived; [H] hypothesis. [O] marks the load-bearing claims, lifted from this session and
not yet audited. Tools and outputs are in `engine/artifacts/lead_tmp/unit-audit/` (README.md there; the BF2 tools end
in `2`). One verifier checked 13 claims (§5); its corrections are folded in.

## 0. Verdict per column

The 33 scripts load 76 class definitions: 49 fielded team-1/2 soldier classes, the droideka, 18 heroes and 8 local
classes. Tables: §2 (rosters), §3 (per class).

| column | verdict | gap → card |
|---|---|---|
| **Which map / era loads it** | **MATCH** for the soldier classes the port fields. The port's 31 BF2 conquest RONs equal the disc's teams 1–2: classes in order, min, max, units, reinforcements (§2) [O]. | Not fielded: the droideka (16 CW scripts; the PS2 fields 1–2 in every CW state) → **DEKA**; heroes → HERO; locals (the RON keeps one class) → MSND / TEAMS. BF2 Jabba's Palace (`tat3c_con`, `tat3g_con`) has no port map → **B2JP**. |
| **Weapon slots** | **GAP.** Every fielded class has 4–6 slots and the PS2 instantiates every slot on every soldier (§3.1) [O]. The port models the primary only; its primary is right for all 76 definitions. | GREN (secondary cycle), UORD: 26 of the 49 fielded primaries are not bolts. Award slots → M4 (GREN note). |
| **What each draws** | Body model: MATCH. Gun: **wrong for 8 classes** (the shared-body cache, UWPN) (§3.2) [O]. Skins: **GAP for 9 classes**, among them the CIS assault, engineer and assassin droids on every CW map and the plain rebel trooper on 7 GCW maps (6 of them port maps) (§3.3) [O]. Root scale: **GAP** (MagnaGuard 1.24, Wookiee 1.15, snipers 0.95). | **UWPN**, **USKN** |
| **Items and grenades** | **GAP.** No grenade, mine, detpack, dispenser, auto turret, recon droid, buff, stealth or time bomb; no drops (§3.4). | GREN, PWUP |
| **Sounds** | **MATCH:** fire, reload, ordnance impact. **GAP:** hurt, death and choke sounds; up to 211 `VOSound` lines per class (squad commands, requests, AI chatter); the port's squad chatter is silent on BF2 maps by its own design (§3.5) [O]. | **USND** |
| **Movement constants** | **MATCH** for MaxSpeed, strafe, turn, acceleration, MaxHealth, clips on all 49 fielded classes. **GAP:** the per-stance `ControlSpeed` factors. The port uses the BF2 constructor's defaults. Every BF2 team class inherits `com_inf_default`'s sprint / jump / roll turn factor 0.35 (port 0.2 / 0.5 / 0.6), confirmed in RAM (§3.6) [O]. **GAP:** jet / jump packs (jet troopers, dark trooper). Chase camera: MATCH for 44 of 49; the Wookiee, MagnaGuard and jet troopers author their own. | **CSPD**; JETP; CAMS |
| **Animations** | Rifle-bank primaries on human skeletons: MATCH for the clips baked. **GAP:** the droids' own clip sets (`sbdroid_rifle_*`, `bdroid_rifle_*`); the bazooka-bank primaries (9 rocketeers, the clone commander's chaingun); the pistol-bank officer primaries. Also 10 of the 34 requested clips (prone, walk back, die) are missing from every BF2 soldier bake (§3.7) [O]. | ANIM |
| **Class unlocks** | **GAP (new column).** BF2 locks each side's officer (8 points) and special class (12) on the player's class select until the player has scored that many points; the PS2 shows "Get N more points to unlock!" and a black silhouette. The port reads nothing (§3.8) [O]. | **UNLK** |

## 1. Method and instruments

1. **Disc census** — `census2.py` → `census2_con.{md,json}` (33 `*_con` scripts), `census2_all.*` (87 ground scripts).
   - The bytecode comes from `scripts/gen_missions.py`'s Lua 5.0 abstract interpreter (`calls50`): `ReadDataFile` in
     order, the `SetupTeams` slot tables (soldier, assault, engineer, sniper, officer, special: class, min, max),
     `SetHeroClass`, the locals' `AddUnitClass`, `SetUnitCount` / `SetReinforcementCount`.
   - `lvl.py` (UAUD1) splits a side LVL into its `lvl_` sublevels; every name any BF2 script passes to `ReadDataFile`
     names its wrapper. INGAME.LVL's top-level classes (`com_inf_default` among them) are in scope for every mission.
   - Each class is resolved as §1.1 describes. `census2.py` also computes the port's view (props_all / first value).
2. **PS2 RAM, class objects** — `ramclass2.py`: every object with the word 0x61e8d8 at +0x1c in the 23 BF2 in-battle
   states (`bf2_*spawn.p2s` + `bf2_*_v*.p2s`): parent, slots, ammo, channels, count, ControlSpeed table, jump velocity,
   override textures. Output `ramclass2.json`.
3. **PS2 RAM, live soldiers** — `ramcensus2.py` over the 20 `bf2_*spawn.p2s` states (19 ground + space Kashyyyk):
   class, health, instantiated weapon objects. Output `ramcensus_bf2.txt`, `ramcensus2.json`. Layout, found by voting on
   the KAS2 state and reproduced by the verifier: the class pointer at E+0x8 and E+0x4a4, health / max at E+0x170 /
   E+0x174 (`re-blaster-damage.md` §6.2's health component), weapon objects at E+0x7c0+4i (E+0x7a8 = the current
   channel-0 weapon), the weapon's class at weapon+0x60. B2's override draw reads the class as *(decompiler detail omitted)*, i.e.
   entity+0x4a4 (§3.3) [D].
4. **The port** — scratch example `uaud2_roster.rs` (UAUD1's `uaud_roster.rs` with `bf2` / `side_bf2`), built in a
   detached worktree, never committed: `FactionRoster::pair` over the 31 BF2 RONs and `SoldierVisuals::load`'s
   per-geometry cache → `port_roster_bf2.txt` (357 class rows).
5. **PCSX2 look** — `pcsx2_bf2/look.sh` under ci.lock: the class-select screen of every class (Left/Right cycles; Down
   does nothing), from the KAS2 Republic, KAM1 CIS, END1 Alliance and HOT1 Empire spawn-map states, and from a new state
   `ref/states/bf2_cor1_gcw_spawnmap.p2s` (Coruscant, Conquest, GCW, Rebels; menu frames `pcsx2_bf2/cor1g_menu.png`).
   Sheets: `pcsx2_bf2/{rep,cis,end,hot,cor1g}_sheet.png`; the CIS droids brightened: `cis_droid_skins.png`.

### 1.1 How BF2 builds a class [V; verifier V1, V2]

- The entc loader `FUN_001e28a8` hashes the TYPE name and walks the class list DAT_00688e84; **a name that already
  exists is skipped** (first definition wins). Otherwise it calls vtable slot 3 of the BASE class (B2 93071), then
  vtable slot 8 once per PROP in file order.
- For a soldier class, slot 3 is `FUN_00255510` (ELF 0x61e8e4 = 0x00255510; table 0x61e8d8). It allocates 0x12b0 and
  calls `FUN_00250dc8(new, parent)` (172171). That copy-constructor writes the table at class+0x1c (169752) and
  **copies the parent's** ControlSpeed columns +0x77c / +0x79c / +0x7bc (169847–169849), weapon slots +0x7f8 (169853),
  ammo +0x818 (169854), channels +0x838 (169855), slot count +0x840 (169856) and override textures +0x844 / +0x848
  (169857–169858). It resets the current weapon section: `DAT_005fd0c8 = 0` (169801). That global has two writers
  in the corpus and the ELF: 169801 and 170988.
- Slot 8 is the setter `FUN_00252bb0` (ELF 0x61e8f8). Its weapon keys:

| key (hash) | effect | lines |
|---|---|---|
| `WEAPONSECTION` 0xd0329e80 | current section = value − 1 | 170986–170988 |
| `WeaponName` 0xfbf47dba, `Weapon` 0x6f332041 | slot[section] = the class with that name; count = max(count, section + 1) | 170920, 171836–171851 |
| `WeaponName1` 0x2ce1a1d1, `PrimaryWeapon` 0xdc697015 | slot 0 | 171649, 171048 → 171065 |
| `WeaponName2` / `3` / `4` | slot 1 / 2 / 3; count raised | 172104, 171746, 171639; 172105–172110 |
| `WeaponAmmo` 0xdc19bd13; `WeaponAmmo1..4` | ammo[section]; slots 0..3 | 171079–171104 |
| `WeaponChannel` 0x16472328; `WeaponChannel1..4` | channel byte; any value outside 0..1 is stored as 0 | 171512–171518; 171726, 171596, 171656, 171606 |

- So a BF2 class is its parent with the child's own sections overwritten, slot by slot [V]. The port's
  `ClassDb::props_all` instead lets the child-most definer of a key replace the whole list (classdb.rs 149–175), and
  `primary_weapon` reads `WeaponName1` before `WeaponName` (roster.rs 306–323).
- On today's data the two agree where it matters [V, census2.py; verifier V3]:
  - the port's primary equals the PS2's slot 0 in all 76 conquest class definitions (101 of 101 over the 87 ground
    scripts);
  - no class those scripts load re-authors only part of its parent's `WeaponName` sections;
  - the port sees one weapon only on the 18 classes written with the numbered keys `WeaponName1..4`: 16 heroes,
    `wok_inf_rocketeer`, `tat_inf_jawa`.
- The doc comment at classdb.rs 152–155 is wrong. It says `rep_inf_ep2_jettrooper_rifleman` "names one weapon where
  its base names four"; that class authors `WeaponName1..WeaponName4` under `WEAPONSECTION 1..4` [V, REP.LVL] → GREN
  note.

## 2. Which map and era loads which class [V: census2_con.md; port: engine/missions/*.ron]

Counts are the SetupTeams minimums; every script gives the soldier slot max 25 and the other five max 4 (66 of 66
teams). "units/tickets" = the member pool and reinforcements. Heroes are `SetHeroClass`.

| map (port key) | script | team 1 (side units/tickets: class min) | team 2 | heroes T1 / T2 | locals (team 3) |
|---|---|---|---|---|---|
| cor1 (bf2_cor1) | cor1c_con | cis 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | rep 16/150: ep3_rifleman 7, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | darthmaul / macewindu | — |
| cor1 (bf2_cor1) | cor1g_con | all 20/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | imp 20/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | luke_jedi / emperor | — |
| dag1 (bf2_dag1) | dag1c_con | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | yoda / grievous | — |
| dag1 (bf2_dag1) | dag1g_con | all 20/150: rifleman_jungle 9, rocketeer_jungle 1, engineer 1, sniper_jungle 1, officer 1, wookiee 1 | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | yoda / darthvader | — |
| dea1 (bf2_dea1) | dea1c_con | rep 16/150: ep3_rifleman 7, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | cis 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | obiwan / emperor | — |
| dea1 (bf2_dea1) | dea1g_con | all 16/150: rifleman_fleet 7, rocketeer_fleet 1, engineer_fleet 1, sniper_fleet 1, officer 1, wookiee 1 | imp 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | luke_jedi / emperor | — |
| end1 (bf2_end1) | end1g_con | all 25/150: rifleman_jungle 10, rocketeer_jungle 1, engineer_jungle 1, sniper_jungle 1, officer_jungle 1, wookiee 1 | imp 25/150: rifleman 10, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | hansolo_tat / darthvader | ewk_inf_trooper 3, ewk_inf_repair 3 (units 6) |
| fel1 (bf2_fel1) | fel1c_con | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper_felucia 1, ep3_officer 1, ep3_jettrooper 1 | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | aalya / jangofett | — |
| fel1 (bf2_fel1) | fel1g_con | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | all 20/150: rifleman_jungle 9, rocketeer_jungle 1, engineer_jungle 1, sniper_jungle 1, officer_jungle 1, wookiee 1 | bobafett / chewbacca | — |
| geo1 (bf2_geo1) | geo1c_con | rep 25/150: ep2_rifleman 10, ep2_rocketeer 1, ep2_engineer 1, ep2_sniper 1, ep3_officer 1, ep2_jettrooper 1 | cis 25/150: rifleman 10, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | macewindu / countdooku | geo_inf_geonosian 7 (units 7) |
| hot1 (bf2_hot1) | hot1g_con | imp 19/150: rifleman_snow 9, rocketeer_snow 1, engineer_snow 1, sniper_snow 1, officer 1, dark_trooper 1 | all 19/150: rifleman_snow 9, rocketeer_snow 1, engineer_snow 1, sniper_snow 1, officer_snow 1, wookiee_snow 1 | darthvader / luke_pilot | — |
| kam1 (bf2_kam1) | kam1c_con | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | jangofett / obiwan | — |
| kam1 (bf2_kam1) | kam1g_con | all 20/150: rifleman_urban 9, rocketeer_fleet 1, engineer_fleet 1, sniper_fleet 1, officer 1, wookiee 1 | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | hansolo_tat / bobafett | — |
| kas2 (bf2_kas2) | kas2c_con | rep 25/150: ep3_rifleman 10, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper_felucia 1, ep3_officer 1, ep3_jettrooper 1 | cis 25/150: rifleman 10, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | yoda / jangofett | wok_inf_warrior 2, wok_inf_rocketeer 2, wok_inf_mechanic 1 (units 5) |
| kas2 (bf2_kas2) | kas2g_con | all 22/150: rifleman_jungle 10, rocketeer_jungle 1, engineer 1, sniper_jungle 1, officer 1, wookiee 1 | imp 22/150: rifleman 10, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | chewbacca / bobafett | wok_inf_warrior 2, wok_inf_rocketeer 2, wok_inf_mechanic 1 (units 5) |
| mus1 (bf2_mus1) | mus1c_con | rep 16/150: ep3_rifleman 7, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | cis 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | obiwan / darthmaul | — |
| mus1 (bf2_mus1) | mus1g_con | all 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | imp 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | obiwan / anakin | — |
| myg1 (bf2_myg1) | myg1c_con | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | kiyadimundi / grievous | — |
| myg1 (bf2_myg1) | myg1g_con | all 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | luke_jedi / bobafett | — |
| nab2 (bf2_nab2) | nab2c_con | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | darthmaul / obiwan | — |
| nab2 (bf2_nab2) | nab2g_con | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | all 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | emperor / leia | — |
| pol1 (bf2_pol1) | pol1c_con | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | darthmaul / yoda | — |
| pol1 (bf2_pol1) | pol1g_con | all 20/150: rifleman_urban 9, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | leia / emperor | — |
| tan1 (bf2_tan1) | tan1c_con | rep 16/150: ep3_rifleman 7, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | cis 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | yoda / grievous | — |
| tan1 (bf2_tan1) | tan1g_con | all 16/150: rifleman_fleet 7, rocketeer_fleet 1, engineer_fleet 1, sniper_fleet 1, officer 1, wookiee 1 | imp 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | leia / darthvader | — |
| tat2 (tat2) | tat2c_con | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | obiwan / darthmaul | tat_inf_jawa 7 (units 7) |
| tat2 (tat2) | tat2g_con | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | all 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | bobafett / hansolo_tat | tat_inf_jawa 7 (units 7) |
| tat3 (NOT BAKED) | tat3c_con | rep 16/150: ep3_rifleman 7, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | cis 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | aalya / darthmaul | gam_inf_gamorreanguard 3 (units 2) |
| tat3 (NOT BAKED) | tat3g_con | imp 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | all 16/150: rifleman 7, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | bobafett / luke_jedi | gam_inf_gamorreanguard 3 (units 2) |
| uta1 (bf2_uta1) | uta1c_con | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | grievous / obiwan | — |
| uta1 (bf2_uta1) | uta1g_con | all 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, wookiee 1 | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | hansolo_tat / bobafett | — |
| yav1 (bf2_yav1) | yav1c_con | cis 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, droideka 1 | rep 20/150: ep3_rifleman 9, ep3_rocketeer 1, ep3_engineer 1, ep3_sniper 1, ep3_officer 1, ep3_jettrooper 1 | darthmaul / macewindu | — |
| yav1 (bf2_yav1) | yav1g_con | imp 20/150: rifleman 9, rocketeer 1, engineer 1, sniper 1, officer 1, dark_trooper 1 | all 20/150: rifleman_jungle 9, rocketeer_jungle 1, engineer_jungle 1, sniper_jungle 1, officer_jungle 1, wookiee 1 | bobafett / chewbacca | — |

- **The port's 31 RONs equal the disc** for teams 1–2 on every mission: the classes in order, min, max, units,
  reinforcements [V; verifier V10 re-derived it from the bytecode]. The bytecode spells some names in mixed case
  (`CIS_inf_*` on fel1c, kam1c, mus1c); the PS2 hashes ignore case, and so does the RON (lowercased).
- **The PS2 fields these classes** [V, `ramcensus_bf2.txt`]: 683 soldier records over the 19 ground states and space
  Kashyyyk's pilots. §3's last column counts them: the CIS classes and the REP ep3 classes are live in 12–17 of the
  17 CW states each; the map-specific variants only where loaded (the Felucia sniper on 2, the ep2 set on GEO1). The GCW classes appear
  only on END1 (jungle rebels, the plain Empire, 6 Ewoks) and HOT1 (snow sets): the library took each map's first
  era. The plain Alliance classes were seen on the class-select screen only (§3.3).
- **Not fielded by the port:**
  - `cis_inf_droideka`: rejected by `ClassRoster::build` (roster.rs 364: BASE chain ends at `walkerdroid`) on all 15
    CW RONs. The PS2 fields it: 30 droideka records (20 alive, max health 550) in all 17 CW states, 14 maps [V, RAM;
    verifier V4]. Its class-select entry: "Repeating Blasters, Shield Emitter", locked for 12 points.
  - Heroes: no RON names one. 18 hero classes are loaded by the 33 scripts; none was alive in any state.
  - Locals: `MissionDef::locals` keeps the first team-3 class and is carried, not consumed (mission.rs 57, 90–94).
    END1 authors 2 Ewok classes (and loads `ewk_inf_scout` without adding it), KAS2 3 Wookiee classes, GEO1 the
    Geonosian, TAT2 the Jawa, TAT3 the Gamorrean guard.
- **BF2 Jabba's Palace is not a port map.** `TAT/TAT3.LVL`, `tat3c_con`, `tat3g_con` (standard rosters + Gamorrean
  guards: `gam_inf_gamorreanguard` AddUnitClass 3, SetUnitCount 2, a melee axe) are on the disc; there is no RON, bake or golden, and no recorded
  decision to drop it → B2JP.

## 3. Per-class verdicts

Generated by `table2.py` from `census2_con.json`, `port_roster_bf2.txt` and `ramcensus2.json`.
- **ch0 weapons:** the primary cycle, award slots excluded; **ch1 items:** `WeaponChannel` 1. The class-select screen
  lists exactly ch0 then ch1 for each of the 30 entries seen (six classes on each of five screens, §3.1).
- **award slots:** weapons with `MedalsTypeToUnlock` ≥ 0 (§3.1).
- **drawn:** port vs disc: the gun the port draws, the skin override, `SkeletonRootScale`.
- **movement:** the constants the port reads; ControlSpeed and the camera are §3.6.
- **anim bank:** the primary's `AnimationBank`, plus the class's own clip set (`SkeletonName` / `AnimationName`).
- **unlock pts:** `PointsToUnlock` (§3.8).
- **PS2 RAM:** live soldier records / states / maps, over the 19 ground states.

| class | team | missions | port fields it | ch0 weapons (disc) | ch1 items (disc) | award slots | drawn (port vs disc) | movement (port vs disc) | anim bank | unlock pts | PS2 RAM: records / states / maps (of 19 / 16) |
|---|---|--:|---|---|---|---|---|---|---|--:|---|
| `cis_inf_rifleman` | cis | 16 | yes | inf_wrist_blaster + inf_wrist_trishot | inf_wrist_rocket | award_wrist_blaster | matches | match | rifle + `sbdroid` set |  | 141 / 17 / 14 |
| `cis_inf_rocketeer` | cis | 16 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | skin `cis_inf_battledroid_red` missing | match | bazooka + `bdroid` set |  | 33 / 15 / 12 |
| `cis_inf_engineer` | cis | 16 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | gun `cis_weap_inf_launcher` not `cis_weap_inf_rifle`; skin `cis_inf_battledroid_orange` missing | match | rifle + `bdroid` set |  | 27 / 16 / 13 |
| `cis_inf_sniper` | cis | 16 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | gun `cis_weap_inf_launcher` not `cis_weap_inf_sniperrifle`; skin `cis_inf_battledroid_camo` missing | match | rifle + `bdroid` set |  | 32 / 17 / 14 |
| `cis_inf_officer` | cis | 16 | yes | inf_bulldog + inf_mortar_launcher | inf_debuff_poison + inf_remotedroid | — | root scale 1.24 missing | match | pistol + `magnaguard` set | 8 | 35 / 17 / 14 |
| `cis_inf_droideka` | cis | 16 | NO: walkerdroid | inf_repeater + walk_droideka_shield | — | — | — | — | rifle + `cis_walk_droideka` set | 12 | 30 / 17 / 14 |
| `cis_hero_darthmaul` | cis | 7 | NO: hero | doublesaber | inf_sabre_throw + inf_force_push | — | — | — | - + `maul` set |  | 0 |
| `rep_inf_ep3_rifleman` | rep | 15 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_rifle + award_pistol | matches | match | rifle |  | 126 / 16 / 13 |
| `rep_inf_ep3_rocketeer` | rep | 15 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | matches | match | bazooka |  | 37 / 16 / 13 |
| `rep_inf_ep3_engineer` | rep | 15 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | matches | match | rifle |  | 26 / 15 / 12 |
| `rep_inf_ep3_sniper` | rep | 13 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | matches | match | rifle |  | 29 / 14 / 11 |
| `rep_inf_ep3_officer` | rep | 16 | yes | inf_chaingun + inf_pistol | inf_buff_defense + inf_remotedroid | award_pistol | matches | match | bazooka + `clonecommander` set | 8 | 33 / 15 / 13 |
| `rep_inf_ep3_jettrooper` | rep | 15 | yes | inf_emp_launcher + inf_commando_pistol | inf_thermaldetonator | award_pistol | matches | match; JetJump 5 not modelled | rifle | 12 | 20 / 12 / 10 |
| `rep_hero_macewindu` | rep | 3 | NO: hero | lightsaber | inf_sabre_throw + inf_force_push | — | — | — | - + `mace` set |  | 0 |
| `all_inf_rifleman` | all | 7 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_pistol + award_rifle | skin `all_inf_soldierdesert` missing | match | rifle |  | 0 |
| `all_inf_rocketeer` | all | 8 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | matches | match | bazooka |  | 0 |
| `all_inf_engineer` | all | 10 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | matches | match | rifle |  | 0 |
| `all_inf_sniper` | all | 8 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | root scale 0.95 missing | match | rifle |  | 0 |
| `all_inf_officer` | all | 13 | yes | inf_incinerator + inf_invisibility | inf_buff_health + inf_timebomb | — | matches | match | rifle + `bothanspy` set | 8 | 0 |
| `all_inf_wookiee` | all | 16 | yes | inf_bowcaster + inf_mortar_launcher | inf_thermaldetonator + inf_remotedroid | — | root scale 1.15 missing | match | rifle | 12 | 3 / 1 / 1 |
| `all_hero_luke_jedi` | all | 4 | NO: hero | lightsaber | inf_sabre_throw + inf_force_push | — | — | — | - |  | 0 |
| `imp_inf_rifleman` | imp | 16 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_rifle + award_pistol | matches | match | rifle |  | 9 / 1 / 1 |
| `imp_inf_rocketeer` | imp | 16 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | matches | match | bazooka |  | 1 / 1 / 1 |
| `imp_inf_engineer` | imp | 16 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | matches | match | rifle |  | 2 / 1 / 1 |
| `imp_inf_sniper` | imp | 16 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | matches | match | rifle |  | 2 / 1 / 1 |
| `imp_inf_officer` | imp | 17 | yes | inf_sonic_blaster + inf_mortar_launcher | inf_buff_damage + inf_remotedroid | — | matches | match | pistol + `impofficer` set | 8 | 4 / 2 / 2 |
| `imp_inf_dark_trooper` | imp | 17 | yes | inf_arccaster + inf_commando_pistol | inf_thermaldetonator | award_pistol | matches | match; JetJump 21 not modelled | rifle | 12 | 4 / 2 / 2 |
| `imp_hero_emperor` | cis/imp | 5 | NO: hero | sidioussaber | inf_force_lightning + inf_force_choke | — | root scale 1.15 missing | — | - |  | 0 |
| `rep_hero_yoda` | all/rep | 5 | NO: hero | lightsaber_yoda | inf_force_pull + inf_force_push | — | root scale 0.342974 missing | — | - |  | 0 |
| `cis_hero_grievous` | cis | 4 | NO: hero | grievoussabers | inf_buff_damage | — | — | — | - |  | 0 |
| `all_inf_rifleman_jungle` | all | 5 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_pistol + award_rifle | matches | match | rifle |  | 9 / 1 / 1 |
| `all_inf_rocketeer_jungle` | all | 5 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | matches | match | bazooka |  | 2 / 1 / 1 |
| `all_inf_sniper_jungle` | all | 5 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | root scale 0.95 missing | match | rifle |  | 1 / 1 / 1 |
| `imp_hero_darthvader` | imp | 4 | NO: hero | lightsaber | inf_sabre_throw + inf_force_choke | — | root scale 1.25 missing | — | - |  | 0 |
| `rep_hero_obiwan` | all/rep | 7 | NO: hero | lightsaber_obiwan | inf_sabre_throw + inf_force_push | — | — | — | - + `obiwan` set |  | 0 |
| `all_inf_rifleman_fleet` | all | 2 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_pistol + award_rifle | matches | match | rifle |  | 0 |
| `all_inf_rocketeer_fleet` | all | 3 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | gun `all_weap_inf_rifle` not `all_weap_inf_launcher` | match | bazooka |  | 0 |
| `all_inf_engineer_fleet` | all | 3 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | matches | match | rifle |  | 0 |
| `all_inf_sniper_fleet` | all | 3 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | gun `all_weap_inf_launcher` not `all_weap_inf_sniperrifle`; gun `all_weap_inf_rifle` not `all_weap_inf_sniperrifle` | match | rifle |  | 0 |
| `all_inf_engineer_jungle` | all | 3 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | matches | match | rifle |  | 1 / 1 / 1 |
| `all_inf_officer_jungle` | all | 3 | yes | inf_incinerator + inf_invisibility | inf_buff_health + inf_timebomb | — | matches | match | rifle + `bothanspy` set | 8 | 1 / 1 / 1 |
| `all_hero_hansolo_tat` | all | 4 | NO: hero | hero_hanpistol + inf_fusioncutter | inf_detpack + inf_buff_defense | — | — | — | pistol + `hansolo` set |  | 0 |
| `ewk_inf_trooper` | locals | 1 | NO: local (RON keeps one) | inf_spear | inf_rock | — | root scale 0.5 missing | — | - |  | 3 / 1 / 1 |
| `ewk_inf_repair` | locals | 1 | NO: local | inf_tool | inf_rock | — | override Ewok2/None (the model has no override slot: no effect); root scale 0.5 missing | — | tool |  | 3 / 1 / 1 |
| `rep_inf_ep3_sniper_felucia` | rep | 2 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | matches | match | rifle |  | 6 / 2 / 2 |
| `rep_hero_aalya` | rep | 2 | NO: hero | lightsaber_aalya | inf_sabre_throw + inf_force_pull | — | — | — | - + `aalya` set |  | 0 |
| `cis_hero_jangofett` | cis | 3 | NO: hero | hero_jangopistol + hero_flamethrower | inf_wrist_rocket_fett + inf_timebomb | — | — | — | pistol + `fett` set |  | 0 |
| `imp_hero_bobafett` | imp | 8 | NO: hero | hero_bobarifle + hero_flamethrower | inf_wrist_rocket + inf_detpack | — | — | — | rifle + `fett` set |  | 0 |
| `all_hero_chewbacca` | all | 3 | NO: hero | inf_bowcaster + hero_rocket_launcher | inf_timebomb + inf_buff_damage | — | root scale 1.15 missing | — | rifle |  | 0 |
| `rep_inf_ep2_rifleman` | rep | 1 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_rifle + award_pistol | matches | match | rifle |  | 11 / 1 / 1 |
| `rep_inf_ep2_rocketeer` | rep | 1 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | matches | match | bazooka |  | 3 / 1 / 1 |
| `rep_inf_ep2_engineer` | rep | 1 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | matches | match | rifle |  | 1 / 1 / 1 |
| `rep_inf_ep2_sniper` | rep | 1 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | gun `rep_weap_inf_rifle` not `rep_weap_inf_sniperrifle`; skin `rep_inf_sharpshooter` missing | match | rifle |  | 2 / 1 / 1 |
| `rep_inf_ep2_jettrooper` | rep | 1 | yes | inf_emp_launcher + inf_commando_pistol | inf_thermaldetonator | award_pistol | matches | match; JetJump 5 not modelled | rifle | 12 | 2 / 1 / 1 |
| `cis_hero_countdooku` | cis | 1 | NO: hero | lightsaber | inf_force_lightning + inf_force_choke | — | — | — | - + `dooku` set |  | 0 |
| `geo_inf_geonosian` | locals | 1 | NO: local (RON keeps one) | inf_blunderbuss | — | — | — | — | rifle |  | 7 / 1 / 1 |
| `imp_inf_rifleman_snow` | imp | 1 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_rifle + award_pistol | matches | match | rifle |  | 8 / 1 / 1 |
| `imp_inf_rocketeer_snow` | imp | 1 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | gun `imp_weap_inf_rifle` not `imp_weap_inf_launcher` | match | bazooka |  | 2 / 1 / 1 |
| `imp_inf_engineer_snow` | imp | 1 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | matches | match | rifle |  | 1 / 1 / 1 |
| `imp_inf_sniper_snow` | imp | 1 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | gun `imp_weap_inf_rifle` not `imp_weap_inf_sniperrifle` | match | rifle |  | 3 / 1 / 1 |
| `all_inf_rifleman_snow` | all | 1 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_pistol + award_rifle | skin `Rebel_face01` missing | match | rifle |  | 8 / 1 / 1 |
| `all_inf_rocketeer_snow` | all | 1 | yes | inf_rocket_launcher + inf_pistol | inf_thermaldetonator + inf_mine_dispenser | award_pistol + award_rocket_launcher | gun `all_weap_inf_rifle` not `all_weap_inf_launcher`; skin `all_inf_vanguard_hoth` missing; skin `Rebel_face02` missing | match | bazooka |  | 2 / 1 / 1 |
| `all_inf_engineer_snow` | all | 1 | yes | inf_shotgun + inf_fusioncutter | inf_detpack + inf_powerup_dispenser | award_shotgun | override all_inf_vanguard_hoth/Rebel_face02 (the model has no override slot: no effect) | match | rifle |  | 1 / 1 / 1 |
| `all_inf_sniper_snow` | all | 1 | yes | inf_sniper_rifle + inf_pistol | inf_thermaldetonator + inf_autoturret_dispenser | award_sniper_rifle + award_pistol | root scale 0.95 missing | match | rifle |  | 1 / 1 / 1 |
| `all_inf_officer_snow` | all | 1 | yes | inf_incinerator + inf_invisibility | inf_buff_health + inf_timebomb | — | matches | match | rifle + `bothanspy` set | 8 | 3 / 1 / 1 |
| `all_inf_wookiee_snow` | all | 1 | yes | inf_bowcaster + inf_mortar_launcher | inf_thermaldetonator + inf_remotedroid | — | skin `all_inf_wookie_snow` missing; root scale 1.15 missing | match | rifle | 12 | 1 / 1 / 1 |
| `all_hero_luke_pilot` | all | 1 | NO: hero | lightsaber_blue | inf_sabre_throw + inf_force_push | — | — | — | - |  | 0 |
| `all_inf_rifleman_urban` | all | 2 | yes | inf_rifle + inf_pistol | inf_thermaldetonator | award_pistol + award_rifle | skin `all_inf_soldierurban` missing | match | rifle |  | 0 |
| `wok_inf_warrior` | locals | 2 | NO: local (RON keeps one) | inf_bowcaster | inf_thermaldetonator | — | — | — | rifle |  | 3 / 1 / 1 |
| `wok_inf_rocketeer` | locals | 2 | NO: local | inf_torpedo_launcher + inf_bowcaster | — | — | override wookie03/wookieface03 (the model has no override slot: no effect) | — | bazooka |  | 1 / 1 / 1 |
| `wok_inf_mechanic` | locals | 2 | NO: local | inf_bowcaster + inf_fusioncutter | — | — | override wookie/wookieface (the model has no override slot: no effect) | — | rifle |  | 1 / 1 / 1 |
| `rep_hero_anakin` | imp | 1 | NO: hero | lightsaber_anakin | inf_sabre_throw + inf_force_choke | — | — | — | - + `obiwan` set |  | 0 |
| `rep_hero_kiyadimundi` | rep | 1 | NO: hero | lightsaber_kiyadi | inf_sabre_throw + inf_force_pull | — | — | — | - + `kiadimundi` set |  | 0 |
| `all_hero_leia` | all | 3 | NO: hero | hero_targetpistol | inf_thermaldetonator + inf_buff_invuln | — | root scale 0.95 missing | — | pistol |  | 0 |
| `tat_inf_jawa` | locals | 2 | NO: local (RON keeps one) | inf_jawa_arccaster + inf_jawa_fusioncutter | — | — | root scale 0.6 missing | — | rifle |  | 0 |
| `gam_inf_gamorreanguard` | locals | 2 | NO: local | inf_axe | — | — | — | — | - |  | 0 |

### 3.1 Weapon slots [V disc; V RAM; V PCSX2]

- Every fielded class resolves to 4–6 slots (§1.1). The RAM agrees class by class and soldier by soldier
  [verifier V4]:
  - all 633 non-engine class objects in the 22 ground states carry exactly the modelled count, slots, ammo and
    channels (this session and the verifier, each with its own reader). Across the 1,119 class names in INGAME.LVL
    and the 18 side LVLs no name is defined twice with different content, so first-definition-wins never changes a
    class on this disc;
  - of the 683 soldier records, 656 hold their class's full slot list as live weapon objects, in slot order; 27 hold
    none (26 at health ≤ 0, one above). No soldier holds a partial list.
- The class-select screens (`pcsx2_bf2/*_sheet.png`) list ch0 then ch1 in slot order and never the award slots.
  - Clone Trooper: Blaster Rifle, Blaster Pistol, Thermal Detonator.
  - Super Battle Droid: Wrist Blaster, Tri-Shot, Wrist Rocket.
  - MagnaGuard: Bulldog RLR, Radiation Launcher, Neuro Poison, Recon Droid.
  - Bothan Spy: Incinerator, Stealth, Regeneration, Time Bomb.
  - Imperial Officer: Sonic Blaster, Mortar Launcher, Rage (Damage Increase), Recon Droid.
  - Clone Commander: Chaingun, Blaster Pistol, Rally (Defense Increase), Recon Droid.
  - Engineers: Shotgun / Blast Cannon, Fusion Cutter, Detpack, Dispenser Health & Ammo.
  - Snipers: Sniper Rifle, Blaster Pistol, Thermal Detonator, Auto Turret.
  - Heavies: Rocket Launcher, Blaster Pistol, Thermal Detonator, Mines.
  - Jet Trooper: EMP Launcher, Commando Pistol, Thermal Detonator.
- **Award slots.** Every trooper class also carries 1–2 award weapons, e.g. `rep_weap_award_rifle`
  (MedalsTypeToUnlock 1). The values are 0 pistol, 1 rifle, 2 rocket launcher, 4 sniper rifle, 5 shotgun; the hero
  guns derived from them reset it to −1. The PS2 instantiates them on every soldier, AI included (RAM), and hides
  them on the class-select screen. When they become usable is [H] → PLAN M4 "awards" (note on GREN).
- **The port** models the primary only (roster.rs 306–323) → GREN. The primary's ordnance kind matters too: the port
  flies every primary as a straight bolt (UAUD1 §3.1). The BF2 fielded primaries by ordnance BASE [V, census]:

| ordnance | classes |
|---|---|
| `bolt` | 23: the rifles, the sniper rifles, the SBD's wrist blaster, the Wookiee's bowcaster, the Imperial officer's sonic blaster |
| `missile` | 12: the 9 rocketeers' rocket launchers, the jet troopers' EMP launcher (2), the MagnaGuard's Bulldog |
| `bullet` | 10: the 9 engineers' shotguns, the clone commander's chaingun |
| `emitterordnance` | 4: the 3 Bothan spies' incinerator, the dark trooper's arc caster |

  → UORD (26 of 49).

### 3.2 What each draws: the gun [V code; measured; verifier V9]

`SoldierVisuals::load` keys assets by `<game>/<GeometryName>` (soldier_vis.rs 128); a later class with the same body
reuses the first class's asset, its gun included, and `weapon_draws` draws that gun (386–395). Over the 31 BF2 RONs
this draws the wrong gun for **8 classes** (9 class-gun pairs; `port_roster_bf2.txt` MISMATCH lines):

| class | draws | should draw | RONs |
|---|---|---|---|
| `cis_inf_engineer` | `cis_weap_inf_launcher` | `cis_weap_inf_rifle` | all 15 CW |
| `cis_inf_sniper` | `cis_weap_inf_launcher` | `cis_weap_inf_sniperrifle` | all 15 CW |
| `all_inf_rocketeer_snow` | `all_weap_inf_rifle` | `all_weap_inf_launcher` | hot1 |
| `imp_inf_rocketeer_snow` | `imp_weap_inf_rifle` | `imp_weap_inf_launcher` | hot1 |
| `imp_inf_sniper_snow` | `imp_weap_inf_rifle` | `imp_weap_inf_sniperrifle` | hot1 |
| `all_inf_rocketeer_fleet` | `all_weap_inf_rifle` | `all_weap_inf_launcher` | dea1, tan1 |
| `all_inf_sniper_fleet` | `all_weap_inf_rifle` (dea1, tan1), `all_weap_inf_launcher` (kam1) | `all_weap_inf_sniperrifle` | dea1, tan1, kam1 |
| `rep_inf_ep2_sniper` | `rep_weap_inf_rifle` | `rep_weap_inf_sniperrifle` | geo1 |

The PS2 frames show each droid with its own gun (`cis_droid_skins.png`: the engineer's shotgun, the assassin's long
rifle, the assault droid's launcher) → UWPN.

### 3.3 What each draws: skins and scale [V disc, V ELF, V RAM, V PCSX2]

> 2026-10-16 (card USKN, `re-soldier-skeleton-skins.md` [O]): SkeletonRootScale scales bone_root's clip translation, not
> the mesh; the MagnaGuard, Wookiee, snipers and droids look wrong in the port because it poses them at the human clips'
> bone lengths, where the PS2 keeps the class rig's (MagnaGuard head 1.643 vs 1.865 m).

- **The draw** is BF1's mechanism [V; verifier V6]. `FUN_00239bb0` (one caller, FUN_00255db8) reads the class at
  *(decompiler detail omitted)* (entity+0x4a4). When +0x844 is set it gives the model group fnv("override_texture") = 0xdc5803e9
  that texture (155278–155290). **Only inside that branch**, when +0x848 is set, it gives the group 0xb98e11c1 the
  second one (155292–155296): OverrideTexture2 needs OverrideTexture. The setter stores OverrideTexture 0x9db74ac at
  +0x844 (171503–171505) and OverrideTexture2 0x227894ba at +0x848 (171701–171703); the copy-constructor inherits both.
- **The overrides that change the look** (the texture differs from the model's `override_texture` MNAM segment,
  `verify2/modl.py`; RAM +0x844 / +0x848 hold the same hashes):

| class | override | model slot texture | scripts |
|---|---|---|---|
| `cis_inf_rocketeer` (assault droid) | `cis_inf_battledroid_red` | `cis_inf_battledroid` | 16 CW |
| `cis_inf_engineer` (engineer droid) | `cis_inf_battledroid_orange` | same | 16 CW |
| `cis_inf_sniper` (assassin droid) | `cis_inf_battledroid_camo` | same | 16 CW |
| `all_inf_rifleman` | `all_inf_soldierdesert` | `all_inf_soldierjungle` | 7 GCW |
| `all_inf_rifleman_urban` | `all_inf_soldierurban` | same | 2 |
| `all_inf_rifleman_snow` | face `Rebel_face01` | `ALL_face01` | hot1 |
| `all_inf_rocketeer_snow` | `all_inf_vanguard_hoth` + face `Rebel_face02` | `all_inf_soldier_hoth` / `ALL_face01` | hot1 |
| `all_inf_wookiee_snow` | `all_inf_wookie_snow` | `all_inf_wookie` | hot1 |
| `rep_inf_ep2_sniper` | `rep_inf_sharpshooter` | `rep_inf_trooper` | geo1 |

  - No effect: the overrides whose texture is the model's own (the jungle rebels, the rocketeers' `vanguarddesert`,
    the snipers' `markspersonjungle`, the Wookiee's `all_inf_wookie`, the ARC trooper's blue), and those on models with no override segment
    (`all_inf_engineer_snow`, `wok_inf_rocketeer`, `wok_inf_mechanic`, `ewk_inf_repair`).
  - The PS2 frames: `cis_droid_skins.png` (orange engineer, tan-camo assassin, red assault droid);
    `cor1g_sheet.png` (Coruscant GCW rebel soldier in tan) beside `end_sheet.png` (Endor's jungle green).
- **`SkeletonRootScale` ≠ 1** [V disc; the setter stores it at +0x1220 (172070–172071) and CollisionRootScale at
  +0x1224 (170980–170981); RAM agrees; verifier V8]:

| scale | fielded classes | others |
|---|---|---|
| 1.24 | `cis_inf_officer` (MagnaGuard), 16 CW | — |
| 1.15 (+ CollisionRootScale 1.15) | `all_inf_wookiee` / `_snow` | Chewbacca; Emperor (no collision scale) |
| 0.95 (+ CollisionRootScale 0.95) | `all_inf_sniper` / `_jungle` / `_snow` | Leia |
| 1.25 / 0.343 / 0.5 / 0.6 | — | Vader; Yoda (collision 0.42); Ewoks; Jawa (collision 0.6) |
| collision only | — | Grievous 1.10; the three Wookiee locals 1.15 |

- The port reads none of these keys [V, search] → USKN.

### 3.4 Items, grenades, drops [V disc, V RAM]

- ch1 per class (the RAM instantiates all of them):

| items | classes |
|---|---|
| thermal detonator | riflemen, jet troopers, dark trooper |
| thermal + mine dispenser | rocketeers |
| detpack + powerup dispenser (health & ammo) | engineers |
| thermal + auto-turret dispenser | snipers |
| buff (defense / damage / health) or poison + recon droid | clone commander, Imperial officer, MagnaGuard |
| health buff + time bomb (ch0 slot 2: stealth) | Bothan spy |
| thermal + recon droid (ch0 slot 2: grenade launcher) | Wookiee |
| wrist rocket (ch0 slot 2: Tri-Shot) | SBD |

- The ordnance BASEs: `sticky` (thrown grenades), `mine` (mines, detpacks, time bombs), `portableturret`,
  `powerupitem`, `droid` (recon droid), `emitterordnance` (buffs). The port models none → GREN.
- **Drops** [V]: all 49 fielded classes inherit `com_inf_default`'s list: ammo 0.25, health25 0.15, offense 0.02,
  defense 0.02, health100 0.08, dual 0.08, energy 0.10 (`com_item_powerup_*`, INGAME.LVL). Heroes
  (`com_jedi_default`) author their own. The port drops nothing → PWUP.

### 3.5 Sounds [V disc; H when the PS2 plays each]

| class property | BF2 | port |
|---|---|---|
| `HurtSound`, `DeathSound` | `<side>_inf_com_chatter_wound` / `_death`, `droid_death` for the CIS | not played |
| unnamed 0x7241c3bb | `repmalechoke`, `impmalechoke`, `allmalechoke` (a choke reaction [H]); the CIS repeat their wound sound. **Named 2026-10-13 (card USND, `re-unit-chatter.md` 3.1): `DamageRegionSound`**, a sound + a name per line | not played |
| `VOSound` `<sound> <event>` lines | 0–211 per class. 41 of 49 fielded classes carry 162–211 lines over 32 events: `SC_*` squad commands and responses, the `*VO` requests (medic, ammo, repair, pickup, backup, spotted), AcquiredTarget, Grenade, MissileIncoming, HeadshotKill, KillingSpree4/8, FriendlyFire, … The three Bothan spies, the SBD and the MagnaGuard carry none; the Wookiees 20; the Imperial officer 44 | not played |
| weapon `FireSound`, `ReloadSound`; the ordnance's collision sound | — | **played** (game/src/main/windowed.rs 30–47) |

- The port's squad-order chatter builds `<side>_inf_com_chatter_tac_*`, BF1's names, and its own comment says it is
  silent on BF2 maps (windowed.rs 72–110). The BF2 command voice is the class's `SC_*` `VOSound` lines
  [V, verifier V13] → USND.

### 3.6 Movement constants [V disc vs V port; V RAM]

- **What the port reads matches.** `port_roster_bf2.txt` vs the disc: 357 class rows, 0 differences over geometry,
  primary, clips, MaxHealth, MaxSpeed, MaxStrafeSpeed, MaxTurnSpeed, Acceleration and the jet flag. The groups:

| classes | MaxHealth | MaxSpeed / strafe |
|---|---|---|
| riflemen, engineers, CIS / IMP officers | 400 | 7.0 / 5.25 |
| rocketeers | 550 | 6.0 / 4.5 |
| snipers | 300 | 8.0 / 6.0 |
| Bothan spies, jet troopers, dark trooper | 300 | 7.0 / 5.25 |
| clone commander | 400 | 6.0 / 4.5 |
| Wookiees | 800 | 6.0 / 4.5 |

  All have turn 5.0, Acceleration 70, JumpHeight 1.78. The setter stores JumpHeight h as a jump velocity, `…
  RAM holds 8.005 = sqrt(36 × 1.78), so the call is a square root [D]. The port's jump velocity is 8.0.
- **ControlSpeed: GAP** [V; verifier V5].
  - The setter parses `ControlSpeed = <stance> f f f` (172005–172053, format '%s %f %f %f' at ELF 0x61e8b8).
  - The stance index comes from the name's hash: stand 0, crouch 1, prone 2, sprint 3, jump 4, roll 5, jet 6,
    tumble 7. The three columns go to +0x77c / +0x79c / +0x7bc.
  - The constructor `FUN_00250488` sets column 3 to 1.0, 1.0, 0.5, 0.2, 0.5, 0.6, 1.0, 0.1 (169403–169418). That is
    exactly the port's `CONTROL_SPEED` (core/src/soldier.rs 28–37). The port multiplies column 3 into the yaw and
    pitch rates (soldier.rs 279–286).
  - INGAME.LVL's `com_inf_default` authors sprint (1.50, 0.50, **0.35**), jump (0.03, 0.03, **0.35**) and roll
    (0.02, 0.02, **0.35**). All 49 fielded classes inherit it.
  - In RAM, 570 class objects carry those rows; the constructor's rows remain only on the engine base `soldier`
    (fnv 0x9ccd1fb5) and the Ewoks (BASE soldier).
  - So a BF2 soldier on the PS2 turns at 0.35 × MaxTurnSpeed while sprinting, jumping and rolling. The port uses 0.2,
    0.5 and 0.6. Heroes (`com_jedi_default`: sprint 2.5 / 0.5 / 0.5, jet 1.5 / 1.25 / 1.25, jump 0.1 / 0.1 / 0.6) and
    the jet troopers' jet row differ again.
  - No port code reads ControlSpeed [V, search] → **CSPD**. BF1 authors no ControlSpeed (UAUD1 census).
- **Jet and jump packs: GAP** [V]:

| class | JetJump | JetPush | JetAcceleration | fuel cost / initial / recharge / min border | other |
|---|---|---|---|---|---|
| `rep_inf_ep3_jettrooper`, `_ep2_` | 5.0 | 8.0 | 30 | 0.16 / 0.25 / 0.12 / 0.24 | JetType hover, `rep_sfx_jetpack` |
| `imp_inf_dark_trooper` | 21.0 | 0.0 | 10 | 0.0 / 0.5 / 0.075 / 0.49 | a jump pack, `imp_sfx_jetpack_darktrooper` |

  The port turns JetJump > 0 into a route flag only (UAUD1 §3.6) → JETP.
- **Chase camera: MATCH for 44 of 49** [V]. The port's `camera_section` (game/src/walk.rs 41–49) is `com_inf_default`'s
  table: stand (1.8, 0, 3.2), crouch (1.3, 0.15, 3.0), prone (0.5, 0, 3.0), sprint (1.8, 0, 4.0). The exceptions:

| class | stand | crouch | sprint | TiltValue |
|---|---|---|---|---|
| `all_inf_wookiee` / `_snow` | centre 2.25, offset 4.0 | 3.5 | centre 2.25, offset 4.5 | 5 |
| `cis_inf_officer` (MagnaGuard) | offset 4.0 | 3.5 | offset 5.0 | 10 (default) |
| the jet troopers | offset 3.5 | 3.5 | 4.0 (default) | 7.5 (stand, crouch) |

  → CAMS note. BF2's TiltValue is 10 in `com_inf_default` (BF1: 5).

### 3.7 Animations [V disc; V bake]

- BF2 INGAME.LVL banks: `human_0` (41 clips, the rifle set), `human_1`, `human_2` (bazooka, 20 named), `human_3`
  (pistol 7, tool 20), `human_4`, `humanfp`, `humanlz` [V, anims.py]. CIS.LVL adds `sbdroid` (27 clips; 17 match
  `sbdroid_rifle_*`: idle, walk, run, shoot, reload, crouch, jump, land, dive, sprint) and `bdroid` (11; 7 match
  `bdroid_rifle_*`, `bdroid_pistol_sprint_full`) [V; verifier V12].
  - The SBD's `SkeletonName` is `sbdroid` and the battle droids' is `bdroid`. That the PS2 picks the clip set by that
    prefix is [D] from the matching names.
  - The officers have 2-clip banks: `clonecommander`, `bothanspy`, `impofficer` (the class's `AnimationName`),
    `magnaguard` (its skeleton). Their clip names are unresolved.
- The port bakes one clip list for every soldier from banks named `human*` (bake/src/soldier.rs 155–190, 281). On
  BF2 each of the 31 baked soldiers gets 24 of the 34 names. Missing on every one: stand_walkbackward, the six prone
  clips, the two getdown_prone clips and die [V, `side_bf2/soldiers/*.anims.ron`; verifier V12].
- Gaps: the droid sets (the SBD on every CW map; the three battle droids); the bazooka bank for the 9 rocketeers and
  the clone commander's chaingun; the pistol bank for the Imperial officer and the MagnaGuard; the missing prone / die
  clips → ANIM.

### 3.8 Class unlocks [V disc, V PCSX2; H the reader]

- The base class setter `FUN_0026a340` stores `PointsToUnlock` 0x714b2463 with '%d' (ELF 0x61fc20) at class+0x374
  (186907–186909).
- The disc gives **8** to every officer (clone commander, MagnaGuard, Bothan spy ×3, Imperial officer) and **12** to
  the special class (jet troopers, droideka, Wookiees, dark trooper). No other fielded class authors it. RAM agrees,
  the droideka's class object included [verifier V11].
- At battle start the PS2's class-select screen shows "Get 8 more points to unlock!" / "Get 12 more points to
  unlock!" on exactly those classes and draws them as black silhouettes; AI soldiers of those classes are already in
  the field ("Unit Count: 3 Max: 4"). See `rep_sheet.png`, `cis_sheet.png`, `end_sheet.png`, `hot_sheet.png`,
  `cor1g_sheet.png`.
- Which score it compares with, and whether the gate re-locks, is [H] (the reader of +0x374 is not traced).
- The port reads nothing [V, search] → UNLK.

## 4. Gaps → cards

- **New cards:**

| card | milestone / value | what |
|---|---|---|
| **CSPD** | M3 / 3, build | the per-class ControlSpeed table (BF2 sprint / jump / roll turn 0.35) |
| **UNLK** | M3 / 3, re | the officer / special unlock points on the player's class select |
| **B2JP** | M1 / 3, build | BF2 Jabba's Palace (TAT3) as a port map |

- **Notes on existing cards (BF2 rows):**
  - UWPN: the 8 classes of §3.2.
  - USKN: the 9 skins of §3.3, the nesting of OverrideTexture2, the root scales, FUN_00239bb0.
  - DEKA: the BF2 droideka: 16 CW scripts, 1–2 fielded per state, 550 health, unlock 12, `cis_walk_droideka` bank.
  - GREN: the BF2 slot model (§1.1), the class-select lists, the award slots, the classdb comment.
  - PWUP: the 7-item `com_inf_default` drop list.
  - ANIM: §3.7.
  - USND: the VOSound table.
  - JETP: the jet values.
  - UORD: 26 of 49 BF2 primaries.
  - CAMS: the 5 BF2 exceptions.
  - HERO: the 18 hero classes and the heroes per script.
  - MSND / TEAMS: the BF2 locals.
  - SPSEL: the class-select names, equipment lists, Left / Right, the locked silhouettes.

## 5. Checks run

### 5.1 The RE checks

- **Check 1** (`re_check.py fn b2 …`):
  - 169752, 169801, 169847–169856 are in `FUN_00250dc8`;
  - 170920, 170986, 170988, 171065, 171079, 171512, 171518, 171596, 171606, 171628, 171639, 171649, 171701, 171716,
    171725, 171726, 171746, 171836, 171846, 171851, 172005, 172052 and 172104 are in `FUN_00252bb0`;
  - 172171 is in `FUN_00255510`;
  - 169403, 169405, 169408, 169412, 169413, 169416–169418 are in `FUN_00250488`;
  - also in `FUN_00252bb0`: 170972–170981 (JumpHeight, CollisionRootScale), 171048, 171503–171505, 171703, 172070–172071
    (SkeletonRootScale), 172105–172110;
  - 186907–186909 are in `FUN_0026a340`;
  - 155278–155296 are in `FUN_00239bb0`;
  - 93071 is in `FUN_001e28a8`.
- **Check 2** (absence):
  - `ControlSpeed` has 3 hits in engine/crates, all comments; its hash 0x47bf39c1 has none.
  - `PointsToUnlock` / 0x714b2463, `OverrideTexture`, `SkeletonRootScale`, `CollisionRootScale`, `DropItem`,
    `VOSound`, `HurtSound`, `DeathSound`, `WeaponName2/3/4`, `JetPush`, `JetFuel`, `MedalsTypeToUnlock`,
    `sbdroid_rifle`, `bdroid_rifle`, `SkeletonName`: 0 hits.
  - `AnimationName` is read only for doors (bake/src/door.rs).
  - `DAT_005fd0c8` writers: 2 (`re_check writers`; the verifier's ELF store scan agrees).
- **Check 3** (the twin): BF1 classes are flat (BASE = an engine base, UAUD1), so the copy-then-override model is
  BF2's alone. The override draw `FUN_00239bb0` is BF1 `FUN_001be1e8`'s twin; both use the same two group hashes.
- **Check 4:** ELF 0x61e8e4 = 0x00255510, 0x61e8f8 = 0x00252bb0; the format strings 0x61e898 '%d', 0x61e8a8 '%f',
  0x61e8b8 '%s %f %f %f', 0x61fc20 '%d'.
- **Check 5:** the current section `DAT_005fd0c8`: default 0 per class (169801), one other writer (170988). The
  channel byte's default is the parent's (copied), else 0.
- **Verifier:** one foreground Opus agent, given only the claims (V1–V13) and their citations. Claims:
  `lead_tmp/unit-audit/verify_claims_bf2.md`; verdicts and independent readers: `verify3/`.
  - Result: 9 SUPPORTED (V1–V7, V10, V11; V6 with a caveat), 4 PARTLY (V8, V9, V12, V13), 0 NOT SUPPORTED.
  - Every correction was reproduced by this session and is folded in:
    - V4: 294 of 294 against the conquest census; 633 of 633 class objects over the 22 ground states;
    - V6: OverrideTexture2 applies only inside the OverrideTexture branch;
    - V8: the heroes' and locals' scales;
    - V9: 8 classes, not 9 (the fleet rows on dea1 and kam1);
    - V12: 24 clips baked, not 34;
    - V13: the CIS assault, engineer and assassin droids carry 171 VOSound lines each; the Bothan spies, the SBD and
      the MagnaGuard carry none.
  - The verifier also found the entc loader's first-definition-wins check and its slot-3 derive (93071).

## 6. What stayed [H]

- When the PS2 plays each class sound and VOSound event.
- Which score `PointsToUnlock` is compared with, and whether a class re-locks.
- When an award weapon becomes usable (medals).
- That the SBD and battle droids take their clip set from `SkeletonName`, and the officers' 2-clip overlays.
- The unnamed key 0x7241c3bb (a choke sound by its values).
- The BF2 soldier camera's TiltValue.
- How each ordnance kind flies.
