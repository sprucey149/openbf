# RE: Soldier hit zones, headshots and bolt prims (BF1 + BF2) [V][D]

Issue #41, 2026-10-27. Two read-only RE passes over the listings; claims are tagged as found.

## Part A: the soldier prims, the bolt sweep, the vehicle crit prims, the sniper


B1:n = analysis/SWBFRONT.ELF-decomp.c line n, B2:n = analysis/SWBFRNT2.ELF-decomp.c. s = class `CollisionRootScale`
(BF2 cls+0x1224, key 0xc64a53ab, setter B2:170980; BF1 cls+0x1e60, B1:123573; default 1.0). m = max(s, 1).
Frame: entity-local, y up from the feet, +z = facing [D: prone head sits at z +0.75 s], x sign = side [H].

## 0. Shape constructors [V]
- type 0 sphere: B2 FUN_00169df8 / B1 FUN_001695a0 (r).
- type 3 vertical cylinder (flat caps, axis = local Y, centred at the translation): B2 FUN_00169e68 / B1 FUN_00169610 (r, full h);
  ray test FUN_0016f950 (B2:13224): |xz| < r, |y-cy| <= h/2, cap normal (0,±1,0). Uses translation only (B2 FUN_0016a5e8 case 3, B2:8878).
- type 2 box: B2 FUN_00169f00 / B1 FUN_001696a8 (half extents a,b,c; bound = sqrt(a²+b²+c²)) [half-extents D from the bound].
- Prim ctor B2 FUN_00160380 -> FUN_00160040: name hash = FNV-1a over (c|0x20) (FUN_00539308; note '_' -> 0x7f),
  matrix = identity + translation, last arg (bone) = 0 -> NOT bone-attached (FUN_00160160 uses the bone only if non-zero).

## 1. Prims per posture
### BF2 class lists FUN_002517e8 (B2:170187-170456): stand +0xbec, crouch +0xd38, prone +0xe84 (stride 0x14c)
Posture select FUN_0024f0b0 (B2:168302-168416): arg 0 -> +0xbec, 1 -> +0xd38, 2 -> +0xe84 [V].
| prim (idx) | stand | crouch | prone |
|---|---|---|---|
| coll_head (0) | sphere r 0.17·m at (0, 1.65s, 0.16) | sphere 0.17·m at (0, 1.2s, 0.25) | sphere 0.17·m at (0, 0.28s, 0.75s) (class list x −0.07) |
| coll_torso (1) | sphere r 0.4s at (0, 1.15s, 0) | sphere 0.4s at (0, 0.7s, 0) | sphere r 0.01 at origin (dead) |
| coll_body (2) | v-cyl r 0.35, h 1.1s, cy 0.55s | v-cyl 0.35, h 0.7s, cy 0.35s | box ½(0.25, 0.2, 0.87s) at (0, 0.2, −0.25s) |
| main_body (3) | v-cyl r 0.57, h 1.65s, cy 0.9s | v-cyl 0.45, h 1.45s, cy 0.65s | v-cyl 0.8, h 0.45s, cy 0.25s |
| coll_body_lower (4) | sphere 0.57 at (0,0.57,0.15) | same | (prone order: medium 0.8 at (0,0.72,0.333k), upper 0.385 at (0,0.3465,k), lower (0,0.3465,−k), medium2 (0,0.3465,−0.333k); k = 0.8s−0.1925) |
| coll_body_upper (5) | sphere 0.57 at (0, 1.9s−0.42, 0.15) | (0, 1.6s−0.42, 0.15) | |
Class head/torso radii read CollisionHeadScale (+0x1140, key 0x4f90c3a1) / CollisionTorsoScale (+0x1144, 0xb199e320),
but the LIVE soldier builds its own head/torso (FUN_00234a68 B2:149713-149757: r 0.17·max(s,1), torso 0.4s, no scale keys)
and copies class prims from index 2 on (B2:149752; FUN_0024f0b0 copies 2.. too, B2:168408) [V]; no class sets either key (bake grep) -> dead.
**Per frame the BF2 head moves** (FUN_0024f350 B2:168821-169060): head centre = s·(table point + CollisionHeadOffset cls+0x1120, key 0x33ab42ba),
torso = s·(table + CollisionTorsoOffset +0x1130, key 0x463a5c7d) [V]. The table (DAT_005fdd48 + bank·0x4b0 + 0x8dc0, filled by FUN_00282f78 B2:216195-216340)
holds bone_head (0x8afdf5e4) · local (0.035, 0.03, 0) and bone_ribcage (0xe5b51072) · (−0.12, −0.05, 0), sampled at t = 0.1333 s per anim [V];
which row per state (0x7e4 states 0..0x12, move dir a local 0/1/2) [H]. Writes only x,y (0x870/0x874), z kept from the posture default [V decomp; H decomp loss].
Fielded offsets: cis default (0, −0.11, 0.05), cis_inf_rifleman (super droid) and wookiee (0, −0.2, 0), snipers (0, 0.15, 0); s: wookiee 1.15, BF2 snipers 0.95.

### BF1 FUN_0020e2e0 (B1:122400-122720): stand +0x1b60, crouch +0x1c14, prone +0x1cc8 (stride 0xb4); class lists used directly
(the collision object's list pointer = cls + posture·0xb4 + 0x1b60, B1:117776 / 120733) [V]; no per-frame head move found (no bone_head CRC in BF1) [V absence by grep].
| prim (idx) | stand | crouch | prone |
|---|---|---|---|
| main_body (0) | v-cyl r 0.57, h 1.65, cy 0.9 (unscaled) | v-cyl 0.45, h 1.45, cy 0.65 | v-cyl 0.8, h 0.45, cy 0.25 |
| coll_body (1) | v-cyl r 0.35, h 1.55s, cy 0.8s + 0.17(m−1) | v-cyl 0.35, h 0.9s, cy 0.6s + 0.17(m−1) | box ½(0.25, 0.2, 0.87s) at (0, 0.2, −0.25s) |
| coll_head (2) | sphere 0.17·m at (−0.09, 1.65s, 0.16) | sphere at (−0.12, 1.2s, 0.25) | sphere at (−0.07, 0.28s, 0.75s) |
| lower/upper | 0.57 at (0,0.57,0.15) / (0, 1.9s−0.42, 0.15) | upper (0, 1.6s−0.42, 0.15) | medium/upper/lower/medium2 as BF2 |
Head index 2 in every posture -> BF1 default HitLocation {2, ×3} (B1:122725) [V].

## 2. Which prim a bolt hits [V]
- Bolts vs units = the per-frame prev->current sweep (B2 FUN_0030fad0, B1 FUN_00273128), not the spawn ray (re-ordnance-kinds §2).
- Sweep -> FUN_00161c60 (disasm 0x161cac: passes &manager+0xe0 as the prim-index out, and filter byte 1) -> FUN_00163078 -> per object FUN_00162f60
  -> soldier collision vt+0x24 (vtable 0x61eb28 at entity+0xcc, slot 0x1691a0) -> FUN_00168318 (B2:~8960 area; loop B2 at "argument 3+0x104").
- FUN_00168318: tests prims in list order; on each hit shrinks the ray (argument 1 *= frac) and records the index -> the result = **nearest prim along the
  segment**; across objects the query length also shrinks -> nearest overall. With the filter set, only prims in the list's 0x400-category mask (+0x128).
- BF2 masks (FUN_002517e8 B2:170459-170556, by prim-name hash via FUN_001681c8): **0x400 = coll_head | coll_torso | coll_body** (bolt-hittable);
  2 = main_body; 0x80/0x10/8 = coll_body_lower/upper/medium/medium2. BF1 (B1:122757-122806, FUN_00167b90 loop B1:6541 uses +0xa4):
  **0x200 = coll_head | coll_body**; 2 = main_body; 0x40/8/4 = lower/upper/medium/medium2. main_body never takes a bolt.
- Impact passes manager+0xe0 as `hit` to vt+0x3c -> FUN_00313c40 -> iface+0x1c (B2:304604). BF1 passes manager+200 (B1:195532).

## 3. Vehicle crit prims in the bake
- Present with names: engine/assets_baked/side_bf1/vehicles/{all,cis,imp,rep}/vehicles.ron, side_bf2/vehicles/{all,cis,imp,rep}/vehicles.ron,
  tat1/manifest.ron (prims[] with name, parent, kind, dims, rotation, translation, mask; walker form name/bone/pos/radius e.g. p_crithit on bone_head).
- Port: game/src/assets.rs `hull_parts` matches HitLocation by prim name into `HullPart::hit_mult` (issue #41); BF1 rows per §3.1.

### 3.1 OURS: BF1 vehicles take BF2's weak spots (2026-11-10)

A named deviation, for one game on every map and so that a launcher's lock has a part to go to
(`re-ordnance-kinds.md` §5.1). Tables: `game::assets::BF1_WEAK_SPOTS`, `BF1_ADDED_WEAK_SPOTS`.

- **The originals.** BF2 authors a x4 `HitLocation` on 15 vehicle classes, each on a prim in the ordnance subset.
  BF1 authors none over 1: its two lines are x0 (`cis_tread_hailfire`, `rep_fly_vtrans`). BF1's models do carry the
  critical prims, mostly outside the ordnance lists, so no round met them.
- **The rule.** A BF1 armoured ground vehicle takes BF2's x4 row on its own model's critical prim (dealt x2, or x4 on the AT-TE, AT-AT and spider walker, on a
  ground map, below), which joins the ordnance
  subset: AAT `p_crithit`, hailfire `p_crithit` (replacing its x0), spider walker `p_crithit1` / `p_crithit2`, the
  three AT-STs `p_crit_hit`, the AT-AT `p_neck_soldier`, the combat speeder `p_crithit`. The hovernaut and the MTT
  have no BF2 twin and take x4 on the `p_crithit` their models carry.
- **Added parts.** Two hulls have no prim a round can reach, and take BF2's part for the same hull at BF2's place,
  in the ordnance subset only. The two fighter tanks: their prims are BF2's one for one, and BF2 added a disc
  (radius 0.5 m, 0.2 m thick) on the rear face at (0, 0.90, -3.85). The AT-TE: its own `p_crithit` (a 1 m sphere at
  (0, 3.43, -1.30)) lies inside the hull boxes; BF2 moved it to a box (half 0.96 x 0.71 x 2.08) under the front of
  the belly at (0, 3.11, 3.14).
- **None:** speeder bikes, the STAP, the tauntaun, the kaadu, the landspeeder and every flyer, as in BF2.
- **Effect.** As on BF2 maps, the factor is a human's only (`Sim::hit_scale`): a bot's hit there is plain. The
  rows carry BF2's authored x4 as data; on a ground map every strike on a weak part, BF1's or BF2's, deals the
  port's factor for the vehicle's class (`Sim::weak_spot_factor`, OURS, the maintainer 2026-11-10: "ATTE, atat,
  and spider walker: make weak spot hits x4. Everything else x2 and that seems good."): **x4** on the AT-TE, the
  AT-AT and the spider walker (`weak_lock::HEAVY_WALKER_WEAK_SPOT_FACTOR`; every class of them in both games, BF2's
  Kashyyyk spider too, and any class whose chain reaches one: `game::vehicle_class::HEAVY_WALKERS`), **x2** on
  every other vehicle (`weak_lock::GROUND_WEAK_SPOT_FACTOR`), locked or not, whatever the round: a torpedo (600)
  does 1200 on a BF1 tank's part and 2400 on a BF1 AT-TE's. Space rounds keep the data's factors. Blast damage is
  never located on a prim in either original, so a splash takes x1 (`re-ordnance-kinds.md` §5.1). Torpedoes to
  destroy on the part (`weak_spot_kill_table`): AAT and fighter tanks (6600) 6, hailfire (5200) 5, combat speeder
  (5400) 5, AT-STs (6000) 5 at x2; spider (6000) 3, AT-TE (15000) 7, AT-AT (30000) 13 at x4; on the hull 11, 9, 9,
  10, 10, 25, 50. (Before 2026-11-10's third pass: x4 for an unlocked strike, and a part lock's factor from the
  centre's line; the third pass first built x2 on every vehicle.)
- **Side effect.** The BF1 prims that join the ordnance subset now stop bolts and sight rays that used to pass
  through them to the hull behind; each sits on or inside its hull.
- measure: maintainer playtest of rockets against BF1 vehicles (B7). Exposure per vehicle: the survey table in
  `re-ordnance-kinds.md` §5.1.

## 4. Sniper damage [V data]
| | BF2 com_weap_inf_sniper_rifle_ord | BF1 {rep,cis,imp,all}_weap_inf_sniperrifle_ord |
|---|---|---|
| MaxDamage | 300 | 160 |
| falloff start/dur/end (0xa74561b2/0xee08d1ba/0x955f71e9) | 0 / 0 / −1 -> none | BF1 has no falloff code |
| Velocity, LifeSpan, range | 2000, 0.15 s, 300 m | 2000, 0.15 s, 300 m |
| Person/Droid scale | 1.0 / 1.0 (Vehicle 0.1, Building 0.1) | 1.0 / 1.0 (Vehicle 0.0) |
| ShotDelay | 1.0 | 0.5 |
Troopers: BF2 rep_inf_ep3_rifleman, imp_inf_rifleman, all_inf_rifleman 400 (person), cis_inf_rifleman 400 (droid, super battle droid);
BF2 snipers 300. BF1 rep_inf_clone_trooper, imp_inf_storm_trooper, all_inf_soldier, cis_inf_battledroid 300; marksman/scout/sharpshooter 240.
| range | BF2 body vs 400 | BF2 head | BF1 body vs 300 | BF1 head |
|---|---|---|---|---|
| 20/50/100/150 m | 300 (no kill, 100 left) | 900 kill* | 160 (no kill, 2 shots) | 480 kill |
*BF2 AI shooters get ×3 only at difficulty level 3 (FUN_001e0580). Body shot kills only BF2/BF1 snipers? BF2 300 HP sniper: yes (300 >= 300) [D: kill test >=/>, untraced]; BF1 240 classes: no.
openbf (2026-11-10, playtest B1): bots never get the ×3 head or the weak-spot factor (×2, ×4 on the AT-TE, AT-AT and spider walker; `re-ordnance-kinds.md` §5.1) on ground rounds, at any tier (BF2's below-level-3 rule for every tier, so difficulty comes from behaviour; BF1, which has no gate, is a named deviation). Space rounds keep the old Hard-and-up gate. `Sim::hit_scale`.


## Part B: the headshot multiplier and the award


B2:n = line n of analysis/SWBFRNT2.ELF-decomp.c, B1:n = analysis/SWBFRONT.ELF-decomp.c.
Helpers in this scratchpad: elf.py (read words), mdis.py (capstone, skipdata), lvlcls.py (LVL class-chunk PROP dump).

## 1. Mechanism (BF2) [V]
- Soldier entity ctor FUN_00234a68 installs health-iface vtable **DAT_0061ea80** at entity+0x22c (B2:149406).
  This vtable was MISSING from re-ai-difficulty §9.2's list. Words (ELF): +0x1c = 0x255d38 -> FUN_00238e20
  (soldier TakeDamage, B2:172715/152382); +0x24 = 0x255d58 -> FUN_0024ec88 (B2:172725/168073, disasm checked).
- FUN_0024ec88: mult = FUN_002699a0(soldier, hit) [class HitLocation table cls+0x2b8/+0x2bc]
  × soldier+0x102c when soldier+0x1028 > 0 (that pair = damage-taken buff: Guardian medal bit 7 -> 0.5, B2:168143; FUN_0024eb70 setter).
- **The head default**: soldier-class setup FUN_002517e8 builds the soldier collision prims in order
  coll_head (first), coll_torso, coll_body, main_body, coll_body_lower/upper/medium/medium2 (B2:170200-170446), then
  `if (cls+700 == 0) { tbl = alloc(8); tbl[0] = {node 0, 3.0f (0x40400000)}; count = 1 }` (B2:170444-170450).
  So **every soldier class with no ODF HitLocation (= all of them; byte search of all 165 PS2 LVLs confirms none) gets
  coll_head (node 0) ×3.0**. Inside `if ((cls+0x778 & 0x20) == 0)` (B2:170187); bit 0x20 = key 0xc372b928 (B2:171291),
  set only on geo_inf_acklay (GEO.LVL), whose else-branch uses geo_inf_acklay%d prims and no default -> acklay immune.
  Droidekas are class `walkerdroid` (not EntitySoldier) -> no default -> immune. Matches the CORE.LVL tip text:
  "Score a critical hit on enemy units by aiming for their heads... Only Droidekas are immune to head shots."
- Per-instance mirror: FUN_00234a68 adds coll_head, coll_torso first to the entity's list, then class prims from index 2 (B2:149737-149757) [V]; head = index 0 [D].
- FUN_00238e20: `if (hit == 0)` fire Lua event **ObjectHeadshot** (DAT_00720f48, B2:152541; host/offline only), regardless of mult/attacker;
  then FUN_001e0580(soldier+0x170, desc, dir, pos, hit, …) (B2:152580).
- FUN_001e0580 (B2:91541): mult>1 is reset to 1.0 if FUN_002a29c8() < 3 AND attacker is AI (desc[0]==0 or desc[0]+0x130<0) (B2:91567-91580).
  So humans always get ×3 headshots; AI only at difficulty level 3. dmg = desc.damage × mult × desc.scale2 in FUN_001dfb38, before PersonScale etc.
- "headshot" feedback object (fnv 0xd7a77563) at the hit point when mult>1, enemy, desc[8+HealthType] >= 1.0 (B2:91597-91630).
- Explosion/no-prim path FUN_001e0900 passes hit = -1 -> 1.0 (B2:91687) -> splash never headshots.

## 2. Which weapons [V/D]
- Any ordnance whose impact reaches iface+0x1c with a prim index (direct bolts/beams/etc) gets ×3 on coll_head: no weapon filter in the chain [V].
- No sniper-specific one-shot rule in code. Sniper bolt com_weap_inf_sniper_rifle_ord MaxDamage 300, award beam 600; soldier MaxHealth 300 [V data],
  so a ×3 head = 900 [D]. Real one-shot-anywhere depends on falloff/health, not traced.

## 3. Award [V]
- FUN_0035fd50(stats=attacker+0x1a0, medal=weapon.ScoreForMedalsType (wpn+0x1e0), killed, critical=(mult>1), zone=HealthType&7, 0,0, +1) (B2:91673; args confirmed in disasm 0x1e0890-0x1e08c4).
- Medal table DAT_0062c560, 12 B/entry {s16 threshold, u8 needKill, u8 needCritical, s16 zoneMask, u8, u8, char* name} (decoding confirmed by disasm 0x35fde0-0x35fe4c):
  0 gunslinger 6 kills | 1 frenzy 12 kills | 2 demolition 4 crit, mask 8 (vehicle) | 3 technician 1 | **4 marksman 6 crit, mask 7 (HealthType 0-2), no kill needed** | 5 regulator 8 kills | 6 endurance 12 | 7 guardian 24 | 8 warhero 36.
- ScoreForMedalsType 4 = com_weap_inf_sniper_rifle + com_weap_award_sniper_rifle only. Marksman = 6 head HITS (not kills) on live enemy soldiers in one life;
  threshold ×3/4 (=4) when FUN_0035fc28 >= 2 (B2:357125). Unlocks award sniper rifle (MedalsTypeToUnlock 4). Loc text CORE.LVL: "Score %s headshots with your sniper rifle within one life and you'll unlock the Beam Rifle." Demolition text "%s rocket launcher critical hits on a vehicle" confirms the needCritical field.
- `headShots` string: only the online leaderboard display (ScriptCB_GetLeaderBoardPlayerData -> FUN_00186140, B2:28434; shell ifs_mp_leaderboarddetails "ifs.stats.headshots"). Upload source not traced [D].
- HeadshotKill chatter (event 9): FUN_00238e20 B2:152585-152609: hit==0, victim died, killer human, enemy (resolves re-unit-chatter's [H]).

## 4. BF1 [V]
- FUN_0020e2e0 builds main_body(0), coll_body(1), coll_head(2), ... per posture list (B1:122439-122507), then `if (cls+0x14a0 == 0)` table = {node 2, 3.0} (B1:122725-122732).
  Lookup FUN_001e3080 reads cls+0x149c/+0x14a0 (B1:92189). FUN_001bc748 (B1:64451) applies mult for every attacker (no difficulty gate), plays "headshot" feedback; no medal system.
  BF1 ODF HitLocation only cis_tread_hailfire / rep_fly_vtrans ×0 (openbf gives BF1's armoured vehicles x4 parts: Part A §3.1).

## 5. What the existing notes got wrong
- re-ai-difficulty §9.2: vtable list missed soldier DAT_0061ea80; FUN_0020a940 is not the soldier TakeDamage (soldier's is FUN_00238e20);
  "default: no entries, every node ×1.0, no data constant" is false for soldiers (hardcoded coll_head ×3.0, B2:170444; B1:122725);
  "no soldier carries one... >1 is vehicle crit" wrong: headshots on soldiers ARE the main >1 case; the level-3 gate therefore DOES matter vs humans (AI headshots).
- re-blaster-damage §6.2 "no per-bodypart or headshot multipliers anywhere" wrong (both games ×3 head).
