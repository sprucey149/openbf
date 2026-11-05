# OPUS RE (2026-09-30) - [O] UNAUDITED
# BF2's minimap (the Map element) and world markers (the Target element) — card BMAP

Corpora: D: = `analysis/SWBFRNT2.ELF-decomp.c` (BF2), B: = `analysis/SWBFRONT.ELF-decomp.c` (BF1). Layout data: the
`1playerhud` tree of `analysis/re-hud.md` (dump `engine/artifacts/lead_tmp/re/bhud/dump_named.txt` lines 1123–1469).
RAM = PCSX2 save states in `engine/artifacts/pcsx2/ref/states/` (`eeMemory.bin`, VA = offset; each state's own
`Screenshot.png` is the 640×480 frame it was saved on). Tools (all in `engine/artifacts/lead_tmp/re/bmap/`): `ram.py`
(state reader), `predict.py` (the model below → icon pixel positions from a state's own RAM), `measure.py` / `batch.py`
(icon centroids in the frame vs the prediction), `backdrop.py` / `bdfit2.py` (the map image through the model vs the
frame), `checks.txt` (the re_check outputs), `cap/` (three new PCSX2 captures: HOT1 and NAB2 Large view, NAB2 turned).
Tags: [V] read (lines cited), [D] derived, [H] hypothesis, [M] measured on RAM or a PS2 frame.
Hops: (1) the Map element's vtable 0x62fc90 → ctor `FUN_003a2d60`, parser `FUN_003a8548`, update `FUN_003a44e8` and its
helpers; (2) the live element in RAM (vtable word scan) to read every field the code computes; (3) the class map-info
setter `FUN_001e2610`; (4) the Target element (vtable 0x6302d8) and the marker tracker `FUN_00179698` /
`FUN_00179f60` / `FUN_00179c10`; (5) the BF1 twin `FUN_002cc128`; (6) 70 walkabout states + 3 captures: predicted vs
measured icon centres; the backdrop image by NCC; (7) re_check fn/absent/word/writers/callers (§9); (8) one Opus
verifier (§10). Load-bearing claims carry [O].

## Headline
- **The minimap is heading-up and eye-centred** [V][O]: centre = the player camera's eye (x, z); radius R = the
  controlled entity class's MapViewMin..MapViewMax, interpolated by its horizontal speed over MapSpeedMin..MapSpeedMax
  (soldiers: com_inf_default 50 / 50 / 0 / 100 → always ±50 m; flyers 300 or 750; ground vehicles the defaults 125→250 m
  over 10→100 m/s); a world point maps to (u, v) = ((X − cx), (Z − cz))·0.5/R of a 0.22·448-px square backdrop (+X
  right, +Z down), the whole map group turned by −heading (so the camera's forward is always up), Scale 0.9, at
  PositionSmall.
- **Checked on the PS2** [M][O]: the model predicts every post icon from a state's own RAM; on all 16 BF2 maps (70
  walkabout states) 263 of 264 unoccluded own-team / enemy post icons sit within ±3 px of the frame's (inside the disc:
  76, mean 1.10 px, max 2.98; clamped to the rim: 188, mean 0.82, p90 1.44); the acceptance poses HOT1 postA spawn
  (the post inside the disc, 15.9 m away: 0.20 px) and HOT1 walk cp1 (rim posts 0.41–0.50 px).
- **The map image** [V][O][M]: `<world>_map` covers the whole-map square of the Large view: a world region named
  `mapbounds` (all 16 BF2 maps have one; TAN1's is turned 180°, giving a negative radius that mirrors the image) or else
  the command posts' bounding box + 100 m (BF1's only rule). The Small view samples the square's sub-rect around the
  eye; re-rendered through the model the image matches the frame best at the model's own scale and offset on 8 of 10
  maps (NCC 0.70–0.94; two featureless maps undecided).
- **Posts** = the CP entity class's MapTexture × MapScale (6 px base): friend (1,86,213) / enemy (200,100,100) / neutral
  (192,192,192), a 1 Hz blink toward the capturing team (checked on YAV1), clamped to the rim when out of range, the
  post number above it fading out toward the rim. **Units** = their class MapTexture × MapScale, turned by heading;
  enemies only when detected. **World markers** = Target styles picked by on/off screen × line of sight × kind, scaled
  MinScale→MaxScale by distance, off-screen ones clamped to the ±0.9 NDC square and turned toward the target.

## 1. The Map element (`Map("player1map")`, type 0xdfa2efb1) [V]
- Factory `FUN_003a2d20` D:399332; vtable 0x62fc90 (word check §9): +0x14 parser `FUN_003a8548`, +0x1c per-frame update
  `FUN_003a44e8` (replaces the base's `FUN_0039da30`, which it calls last).
- Ctor `FUN_003a2d60`: three views named from the table 0x5ff8e8 {"Small", "Large", "Spawn"} (current view +0xc33c,
  ctor 1, D:399546). Per view: `Backdrop%s` (masked bitmap, +0xdb0 + 0x1c0·v, z 0xff), `PlayerFOV%s` (Model3D, z 0x7b,
  D:399621), `PlayerDirection%s` (z 0x7c, D:399637), `Target%s` (z 0x7e, D:399667), `Post%s` (z 0x80, D:399690),
  `PostText%s`, `PostSelect%s` (`hud_icon_flash`), `Marker%s`, `North%s` (`hud_map_northpointer`); pools shared by the
  views: 128 target, 16 post, 16 post-text, 16 post-select, 32 marker sprites (D:399677, 399700, 399755, 399771;
  the post-select sprites are first named `PostSelected%s` (0x62fb58, D:399554), then `PostSelect%s`). Config
  children address these sub-elements by name (`BackdropSmall`, `PostSmall`, …; re-hud.md §1.1).
- Sizes [V]: backdrop side s·H px square, s = 0.22 / 0.75 / 0.65 (Small / Large / Spawn: ELF 0x5ff8c0 + 12·v; the call
  D:399607 passes w = s·H/W screen widths, h = s screen heights; RAM rects ±49.28 / ±168 / ±145.6 [M]); every
  target / post / marker / north sprite starts 6 × 6 px (4 × 4 with ≥ 2 viewports: 0x62fb20 {6.0, 4.0}, D:399651,
  399663; RAM rect ±3 [M]) and is scaled per item (§3).
- Defaults: per-view positions (0.15, 0.82) / (0.5, 0.5) / (0.3, 0.5) (0x5ff8b8; D:399569 is a 12-byte copy from rodata
  0x62fd70 = 0.15, 0.82, 0.22 over the ELF's 0.8 — verifier; copied D:399576) — overridden by the data's PositionSmall
  (0.856385, 0.186248) / PositionLarge (0.308639, 0.557075, z −38.1331) / PositionSpawn (0.303257, 0.477156) (parser
  D:402466, 402407, 402420; RAM 438.47, 83.44 px etc. [M]); +0xc3c4 = 10 (the post-select decay rate), +0xc3c8 = 3 (its
  growth), +0xc3cc = 0xffd55601 = friend blue (D:399702–399704); flag bits +0xc3d4 ← 0x18 (D:399549): bit 3 = allow the
  minimap (property `allowminimap`, D:402519), bit 4 = post numbers on (property 0x5f18ecef, not in the data), bit 2 =
  the minimap disallowed (set with `allowminimap` 0 and by `DisableSmallMapMiniMap`), bit 1 = CPs hidden (`hideCPs`),
  bit 0 = texture bound.
- Unnamed parser properties (none in the data) [V]: 0x55951ec1 / 0xce9d2fdd / 0xb5faa499 = the per-view shape
  {Circle, Rectangle} (table 0x5ff8f8; default 0 = Circle for all three), 0x4f20a696 → +0xc3cc, 0x8b5b4c23 → +0xc3c0,
  0xa34f782d → +0xc3c4, 0xc1419eb5 → +0xc3c8.

## 2. The world → map transform
### 2.1 Centre, radius and the unit square [V][O]
- Each frame `FUN_003a44e8` copies the player camera's rows (camera = `*(DAT_005fc4ec + 4·player + 0x2c)`, rows at
  +0x30 right, +0x40 up, +0x50 back, +0x60 eye — the pose.py layout): +0xc3a0 = back, +0xc390 = eye (D:400060,
  400064). The Small view record (+0xc300: cx, cz, R, k, shape) = (eye.x, eye.z, R, 0.5/R) (D:400167–400169). [M] RAM
  HOT1 cp1: (16.307, −15.772, 50, 0.01) = the walkabout eye.
- R [V][O]: the controlled entity (`+0x150`, else `+0x14c`, else `+0x148` of the player) gives the map info: the first
  entry of its `FUN_002690c8` list whose object has +0xc0 ≥ 0, else its own class's ([H] what that list holds); the
  block runs only while a controlled entity exists. speed = the HORIZONTAL speed √(vx² + vz²) of the entity's vfunc
  +0x4c vector (ELF 0x3a488c `VMUL.xz`, `VADDz.x`, `VSQRT` — verifier): t = clamp((speed − MapSpeedMin)/(MapSpeedMax −
  MapSpeedMin), 0, 1), R = MapViewMin + (MapViewMax − MapViewMin)·t (D:400131, 400139 / 400154); a fixed 125 m when the
  mode byte DAT_00600548 (DAT_00600549 when DAT_0060054f) is set and the entity's vfunc +0xb4 has bit 0x10 (D:400165;
  [H] a space-map rule). The keys (setter `FUN_001e2610`): MapSpeedMin +0x34 (D:92892), MapSpeedMax +0x38, MapViewMin
  +0x3c (D:92937), MapViewMax +0x40, MapScale +0x30, MapTexture +0x44 (hash); defaults 10 / 100 / 125 / 250 / 1 (ctor
  `FUN_001e22a8` D:92775–92780); MapViewMin also raises +0x54 to at least its value (D:92939–92942). Disc values (the
  port's class RONs): com_inf_default / com_hero_default / com_jedi_default 50 / 50 / 0 / 100; flyers 750 / 750 or 300 /
  300; no ground vehicle sets them (→ 125..250 by speed) [M].
- `FUN_003a5228` D:400461–400492 [V][O]: u = (X − cx)·k, v = (Z − cz)·k; Circle: u² + v² > 0.25 → hidden (targets) or
  scaled onto the rim (posts, markers, the eye); Rectangle: max(|u|, |v|) > 0.5 likewise; then × the backdrop's width
  and height → local px about the group origin (+x right, +y down).

### 2.2 Rotation and placement [V][O]
- Small view: θ = atan2(−back.x, back.z) (D:400174); the group matrix = the base (Scale diag(0.9) from the config) ×
  rotation(−θ), translation PositionSmall (D:400181, 400234). [M] RAM HOT1 cp1: rows (0.9, −0.0076), (0.0076, 0.9) for
  back = (−0.0084, 0.0112, 0.9999). In screen terms (y down): local (x, y) → (x cos α − y sin α, x sin α + y cos α), α =
  −θ: the camera's horizontal forward points up (checked: facing −X puts the north pointer on the right, HOT1 walk
  b090).
- Large / Spawn: no heading rotation; the group turns by the mission's north angle (DAT_005ff8e0, only when ≠ 0:
  D:400178); the view switch sets the unrotated matrix at the view's position (`FUN_003a80e8` D:402237, 402244).
- North angle [V][O]: Lua `SetMapNorthAngle(deg[, flag])` → `FUN_003a8b98` (D:37181, 402583) is the only writer besides
  the ctor's 0 (writers §9). [M] per mission from RAM (every BF2 state): 180° + flag 1 on cor1g_con and nab2c/g_con,
  180° flag 0 on tan1c/g_con, 0 elsewhere (cor1c_con 0 in both its states though its PC source calls it — the PS2's
  value wins). The flag also turns the Small view's north pointer by the angle (D:400582); without it the Small
  pointer stays on the image's −Z edge. [M] frames: NAB2 / COR1-GCW pointer at the bottom facing −Z; HOT1, TAN1, POL1,
  UTA1 at the top.
- HUD projection [D] (re-hud.md §2.4 mapping, no new parameter): a group-space point (px, py) (512×448 px) lands at
  display fraction (−0.0059 + 1.0139·px/512, −0.0242 + 1.0484·py/448). [M] The Large view's PositionLarge z = −38.13
  draws it smaller: 10 posts in two captures (HOT1, NAB2) fit that mapping scaled by **0.948 about the display centre**
  (max residual 1.66 px); the root-depth model (1.24·D = 549.8 → 549.8/587.9 = 0.935) misses by up to 4.75 px — the
  measured 0.948 is the value, the cause stays [H]. Small (z 0.004) and Spawn (z 0) need no factor.

### 2.3 The whole-map square and the map image [V][O][M]
- `FUN_003a7af0` (called once, from the update, while +0xc3d4 bit 0 is clear: D:399976): the region named `mapbounds`
  (hash 0x866e09f4, lookup `FUN_00538108`, D:401977) of type 2 or 5 gives centre = its translation (x, z) and half
  sizes h_x = row0.x·size.x, h_z = row2.z·size.z — signed (D:401980 ff.); otherwise every command post (list
  DAT_005fed34; a post without a live entity counts as (0, 0)) gives min/max x, z and h = (max − min)/2 + 100
  (D:402070). R_L = max(h_x, h_z) (D:402074), centre (D:402075), k_L = 0.5/R_L (D:402077); Spawn copies Large, shape
  word included (D:402078).
  [M] all 16 BF2 maps have a type-2/5 `mapbounds` and the rule reproduces RAM exactly (e.g. HOT1 (−239.55, −109.15)
  R 400; TAN1 (−299.02, 134.93) R −89.43: its box is yawed 180° so both halves are negative and the max picks −89.43).
  The port's baked manifest regions show the rotation as diag(−1, 1, −1)·RAM (HOT1: identity in RAM) — convert before
  use.
- Texture [V]: `sprintf("%s_map", world)` (0x62fc20, world name at 0x825da0, D:402114) on all three backdrops, mask
  `map_mask` (0x62fc28, D:402122). The config's `mask("map_mask")` is read too, by the masked bitmap's own parser
  `FUN_003a1a20` (vtable 0x62f930 slot +0x14; D:398511 compares `-0x1888ba97` = 0xe7774569 = fnv("mask") →
  `FUN_005aa030`, and 0x7e272c84 `masktexcoords` → `FUN_005aa180`) — same texture either way.
- Backdrop UVs [V][O]: `FUN_003a5438` D:400521 sets the image's texcoords to the view's square in the Large square: u0,1
  = (cx ∓ R − cx_L)·k_L + 0.5, v0,1 = (cz ∓ R − cz_L)·k_L + 0.5 (called only on the rebuild path: every Small frame, on
  Large / Spawn only while the north angle ≠ 0; the config's backdrop `texcoords` is overwritten there; RAM float rect =
  the formula [M]). A negative k_L flips the rect (TAN1). The GS words are u·2048 + 1024/texture width (a half texel;
  `FUN_005a8810`).
- Draw `FUN_005aa1f0` [V]: pass 1 draws `map_mask` into destination alpha (ALPHA 0x55 = no colour change), pass 2 the
  map image blended by destination alpha (ALPHA 0x54; 0x58 = a FIX blend when +0x64 == 1) at the element colour and
  alpha (BackdropSmall α 0.5 × Map α 0.85); with no mask it falls back to the plain bitmap draw; a Shadow style adds a
  black half-alpha pass (0x7da3b0 = 0x44). RAM: +0x60 = +0x64 = 0 on all three backdrops.
- [M] `bdfit2.py`: the port's baked `<world>_map` sampled through the model vs the frame's lower disc (below the FOV
  wedge, icons removed): NCC at the model UTA1 0.766 / 0.943 (two posts), TAN1 0.896 (the mirrored rect), POL1 0.908,
  NAB2 0.700, MUS1 0.899, DEA1 0.941, KAM1 0.754 — each the best of a scale × offset search; KAS2 0.107, GEO1 0.339
  (featureless images, no scale wins clearly).

## 3. What the Small / Large / Spawn views draw
- **Posts** `FUN_003a6a48` [V][O] (16 slots, PTR_DAT_005fb2c8, 0x30 B: CP pointer, MapScale, then the class map info;
  D:401465): drawn while the CP's entity is live and its +0x228 bit 3 is set (D:401491; [M] HOT1's cp6 has it clear and
  is not drawn) and CPs are not hidden (or the view is Spawn, D:401466). Texture = the CP entity class's MapTexture
  (refreshPost `FUN_003a9098` D:402846: `hud_flag_icon` 2.5, the shield generator `hud_dest_icon` 3.0, HOT1's walkers
  `atat_icon` 3.5 [M]); size 6 px × MapScale (D:401620 / 401661; turned by the entity's heading unless its vfunc +0xb4
  returns 1). Colour = the local team's table entry for the owner (D:401508): friend 0xffd55601 = (1, 86, 213), enemy
  DAT_0076c228 = (200, 100, 100), neutral DAT_0076c208 = (192, 192, 192), a team of type 5 with no alliance DAT_0076c250
  = (223, 223, 32) (`FUN_0036d9f8` D:366999–367012; palette `FUN_0057ca38`). Capture blink: while the capturing team
  (cp+0x50) is set and not the owner, w = 8·f for f < 0.125, 8·(0.25 − f) for f < 0.25, else 0, f = the real-time clock
  mod 1 s, colour = lerp(owner, capturing team (neutral when −1), w) (D:401520–401545). [M] 26 contested posts in 12
  states (f from the RAM clock `FUN_004240f8`'s inputs): YAV1 post1 spawn (f 0.160, w 0.72) shows all three in the
  capturing team's colour (red-owned post 7 drawn blue: pred (56, 90, 182) vs frame (47, 89, 175)); 21 outside the
  window show the owner's hue; one (MYG1 spawn map, f 0.045, predicted w 0.36) shows the owner's pure colour — a frame a
  couple of frames off the RAM clock would explain it [H]; one is neutral → neutral. Clamp to the rim (D:401595, flag
  1). Post number: `"%d"` of cp+0x1c (D:401685) above the icon, only when the Map's +0xc3d0 bit 36 (default on,
  D:401666) and cp+0xb40 bit 1 (D:401684), alpha × (1 − 4·(u² + v²)) on the Small view (D:401671) — so only posts well
  inside the disc show a number. PostSelect ring: on the Spawn view the post the player has selected (`FUN_003de978(cp)`
  = player +0x144) gets `hud_icon_flash` at MapScale × 2 in +0xc3cc blue (D:401694 and the view-2 branch below it); the
  timer is re-set to 2 or 0 every frame before use, so its decay (D:401711) and the MapScale + 3·t branch of the other
  views (D:401707) never show [D]. [M] NAB2 spawn map: the selected post 5 carries the larger ring.
- **Units** `FUN_003a5940` [V] (128 slots, PTR_DAT_005fb2c4, 0x38 B: entity, generation, a "friendly-shown" byte,
  MapScale, then the class map info; D:400818): texture = class MapTexture (refreshTarget, D:402802), size 6 px ×
  MapScale, turned by the entity's heading (D:401137), hidden outside the circle (flag 0, D:401013), none without a
  controlled entity (the spawn screen). Shown when friendly with the byte set; otherwise (enemy, or map objects like
  turrets and recharge droids) only when detected (D:400893): the player's class ranges `FUN_003a64b8` (default 15 m
  always, up to 60 m inside a ±24° view cone), while the unit fires (`FUN_003a68d0`), or special team abilities. Colour
  = team colour, halved toward (64, 64, 64) when the entity's +0x228 bit 3 is clear (D:401054). [M] HOT1: turrets
  (`turret_icon` 1.2) and recharge droids (0.75) drawn grey; the player's own soldier icon is not visible at the centre
  (the FOV mesh sits there) [H: the rule that hides it is not traced].
- **Markers** `FUN_003a7630` [V] (32 Lua-placed markers at DAT_00679d60, 0x28 B: entity or fixed position, team mask,
  texture +0x10, scale +0x14, colour +0x18 (alpha 0 → team colour), pulse +0x20, flags +0x24 (bit 0 blink alpha, bit 1
  pulse scale)); clamped to the rim, kept upright.
- **FOV / direction** `FUN_003a5650` [V]: `hud_map_fov` at the eye (the centre in Small), turned heading − π (net: up in
  Small), Scale 1.15; the direction mesh (yellow, α 0.5) only when the vehicle / turret aim differs from the camera
  heading by ≥ 15° (D:400697).
- **North pointer** `FUN_003a54f0` [V]: at (0, −h/2) of the backdrop — its top edge — scale from its config (2 in
  Small), turned with the group; [M] frames show it just outside the disc on the image's −Z side.

## 4. Views and events [V]
Select = `player1.map.modeToggle` (`FUN_003a8d18`, nothing on the Spawn view or a zero event): hidden map → shown in
Small, or Large when `allowminimap` is off (D:402687); shown → view + 1 (Small → Large; the decomp drops the argument —
ELF 0x3a8d70–0x3a8dc4, verifier); Large → Small, hidden instead when `allowminimap` is off (bit 34, D:402690–402693).
`FUN_003a80e8` hides rather than places the map for Small with bit 35 clear and bit 34 set (D:402239–402241).
`player1.map.mode` sets a view directly (the spawn screen: Spawn); `hideCPs` → bit 33 = the event value & 1 (sets or
clears; `FUN_003a8f40` D:402750 → `FUN_003a5400`); Lua `DisableSmallMapMiniMap` (D:37288) sets bit 34 and clears 35 —
called only by the six space-assault missions (PS2 MISSION.LVL search), none a port map;
`refreshPost` / `refreshTarget` / `refreshMarker` rebind a slot's texture (MapTexture) and scale. [M] Select in HOT1
and NAB2 (captures `cap/*_large`): the big map at α 0.25 with numbered posts, "Return to Game [SELECT]".

## 5. The Target element (`Target("player1target")`, type 0x32608848) — world markers [V]
- Factory D:405133, vtable 0x6302d8 (update `FUN_003ad180`). Styles = template children by name: 8 objective / flag
  styles (0x5ff958: ObjectiveOnScreen, …Behind, …OffScreen, …OffScreenBehind, then Flag…; D:405219) and 16 attacker /
  locked / hint / hint-flag styles (0x5ff918, D:405228); template keys ColorFriendly +0x1ac, MinScale +0x1a0, MaxScale
  +0x1a4, 0x937114c6 +0x1a8 (a colour-change rate) (`FUN_003ac9b0` D:405104–405119). Pools: 32 objective sprites, 64
  attacker/locked/hint sprites, 4 lock-on corner brackets.
- Style [V][O]: flags of the marker record: bit 1 on screen, bit 2 in line of sight, bit 3 attacker, bit 4 locked,
  bit 8 flag / hint-flag. Objective list: (bit 8 ? 4 : 0) + (bit 2 ? 0 : 1) + (bit 1 ? 0 : 2) (D:405599); tracker list:
  base 4 locked / 0 attacker / 12 hint-flag / 8 hint, + the same two bits (D:405622). "Behind" = occluded, not
  behind the camera.
- Projection `FUN_00179c10` [V][O]: the anchor (the entity, + 1.3 m up for objectives D:20343, + a height offset for
  units) → NDC; toward the rear (cos < 0.2 and |y| > 0.3|x|) |y| becomes 0.3|x| + (|y| − 0.3|x|)·5·max(cos, 0) (D:20133;
  skipped when the player's entity is of class DAT_00689368); off screen when behind or |x| or |y| > E = 0.9 (tracker
  +0x96c [M]) → scaled so max(|x|, |y|) = E, bit 1 cleared (D:20146, 20155). Hidden beyond the record's max distance
  (D:20372); line of sight by one ray per tracker per frame (D:20461).
- Placement `FUN_003abc60` [V][O]: screen = ((x·0.5 + 0.5)·W, (y·0.5 + 0.5)·H) of the viewport (D:404981–404982); scale
  = lerp(MinScale, MaxScale, clamp((d − near)/(far − near))) (D:404866, 404927; near / far from the record) — except
  when the player's vehicle (+0x14c) is of class DAT_00689368 / DAT_006897c0 and the record is on screen: a fixed 200 →
  1700 m ramp between the 0x5ff98c.. constants (0.25 / 0.5 / 0.1 and 0.7 / 0.8 / 1.5), ×1.6 on the player's own vehicle
  with bit 8 (D:404836–404864); on screen: no rotation; the extra HintFlag sprite drawn over a locked flag record (bits
  4 and 8, `FUN_003ad180` D:405371–405376) is raised by the icon height (D:405015); off screen: turned atan2(x, −y)
  toward the target (D:405004); locked: four corner brackets at 0, 90, 180, 270° (0x5ff978, D:405060). Colour: the
  template's, or ColorFriendly when the target is friendly, or the team colour when the template's alpha is 0; locked
  markers pulse.
- Scope boundary [H for the build]: WHICH entities become attacker / locked / hint / objective markers is the game's
  tracker logic (`FUN_00179278` per-player tracker, `FUN_0017a908` / `FUN_0017a900` fill, the objective list
  DAT_0066af58 written by `FUN_0017cc00`) — not traced here; no live record in any saved state.
  **Traced 2026-10-21 (card HATK, `re-hud-attacker.md` §2):** no code sets the attacker bit [receipt:9028bdc3] (the Attacker templates
  are dead data), and the 64 tracker records are the `bSoldierHints` option's candidates (off by default). The PS2's
  hit-direction marker is a code-drawn piece, `ambush_icon` at the screen edges (`re-hud-attacker.md` §1).

## 6. The BF1 twin [V][M]
BF1's HUD is code (no `hud_` chunk, re-hud.md §1.1); its map element (`*(0x564490)`) keeps the same record: centre
+0x2a4 / +0x2b0, R +0x2bc, k +0x2c8, set by `FUN_002cc128` (B:256487–256495) with only the post-bounding-box rule (no
`mapbounds`, no MapView keys, no SetMapNorthAngle, no `map_mask` in the BF1 ELF — §9). Texture `"%s_map"` (0x540308)
of the world's `TNAM` (terrain name) chunk (`FUN_001767e0` B:17700 → `FUN_002cc9e8` B:256873). [M] RAM on six BF1
maps equals the rule from the baked posts on BES1 (36.78, −8.00) R 345.43, BES2, KAM1 exactly; END1, GEO1 and HOT1
include posts of other classes (the list is every CP, not only `com_bldg_controlzone`).

## 7. Numbers for the build's acceptance
- HOT1 postA spawn (`bf2_hot1_cp1_cp6_postA_spawn.p2s`, minimap centre = eye (1.484, −10.097), θ 150.1°): posts
  predicted / measured (640×480): p1 (534.3, 79.7) / (534.4, 79.8); p2 (606.6, 70.6) / (606.7, 70.7); p5 dest (608.1,
  84.8) / (608.0, 84.9); p3+p4 pair (595.8, 113.3) / (595.9, 113.7); the minimap centre (551.9, 82.1).
- HOT1 walk cp1 b000: p1 (551.9, 82.1) / (552.2, 83.0); p2 (510.6, 115.9) / (510.9, 116.2); p5 (501.9, 104.8) /
  (502.2, 104.4); p3+p4 (496.2, 75.8) / (496.5, 76.1).
- Large view scale 0.948 about the display centre; the Spawn view (NAB2 north 180) posts p1 (194.0, 325.2) / (194.0,
  326.1), p2 (90.7, 273.0) / (91.1, 273.8), p4 (194.0, 228.2) / (194.3, 229.2).

## 8. For the build (port decisions — reversible, not PS2 facts)
- Feed per frame: camera eye + forward (the render camera), the controlled entity's class MapView*/MapSpeed* and speed,
  every CP (position, owner, capturing team, number, MapTexture / MapScale of its entity class), units (position,
  heading, team, MapTexture / MapScale, detected), the map image, `mapbounds` (else the CP box + 100), the mission's
  north angle (bake SetMapNorthAngle from the mission script; else the §2.2 table).
- Draw in BHUD2's interpreter under the same screen mapping; the minimap is a top-right element (anchor right, Hor+),
  never stretched. The PS2's ellipse (the HUD's 1.128 horizontal stretch) follows BHUD2's stretch decision.
- BF1 maps: `<terrain>_map`, the CP-box rule, north 0, the class values of BF1 ODFs (no MapView keys → the BF2
  defaults 125 / 250) — [H] BF1's own minimap range is not traced (BF1's `+0x2c4` radius, B:258013).

## 9. Checks run (paste in `lead_tmp/re/bmap/checks.txt`) and what stays [H]
1. `re_check.py fn b2` on 118 cited lines and `fn b1` on 5: every line sits in the function named here (e.g.
   *(decompiler detail omitted)*, *(decompiler detail omitted)*, *(decompiler detail omitted)*,
   `B1:256487 is in FUN_002cc128 … * 0.5 + 100.0`).
2. Absent: `866e09f4` B1 0 / B2 1 (FUN_003a7af0; the verifier also tried sign-extended, negative, decimal, `lui 0x866e`
   and data-word spellings in B1: none); `SetMapNorthAngle` B1 0; the BF1 ELF bytes hold `%s_map` (0x540308) but no
   `map_mask`, `mapbounds`, `SetMapNorthAngle`, `MapViewMin` strings. My first draft also claimed `e7774569` (`mask`)
   absent — WRONG: Ghidra printed it as `-0x1888ba97` (D:398511); a hex search misses signed spellings.
3. Twin: BF1 `FUN_002cc128` (callers FUN_002d00f0, FUN_0033e590) = the BF2 fallback without `mapbounds`. Callers:
   `FUN_003a7af0` ← FUN_003a44e8 only; `FUN_003a5228` ← FUN_003a5650 / 5940 / 6a48 / 7630 (flags 1 / 0 / 1 / 1);
   `FUN_003a8b98` ← SetMapNorthAngle only; `FUN_00179c10` ← FUN_00179f60 only.
4. Words: 0x5ff8e8 → Small / Large / Spawn; 0x5ff8b8.. = 0.8, 0.82, 0.22, 0.5, 0.5, 0.75, 0.3, 0.5, 0.65; 0x62fb20 =
   6.0, 4.0; 0x5ff8f8 → Circle / Rectangle; the ten sub-element formats (`PostSelected%s`, `Marker%s`, `Backdrop%s`,
   `PlayerFOV%s`, `PlayerDirection%s`, `Target%s`, `Post%s`, `PostText%s`, `PostSelect%s`, `North%s`), `hud_icon_flash`,
   `hud_map_northpointer`, `%d`, `%s_map`, `map_mask`; 0x5ff978 = 0, π/2, π, 3π/2; 0x5ff988.. = 1700, 0.25, 0.5, 0.1,
   200, 0.7, 0.8, 1.5; vtables 0x62fc90 and 0x6302d8 slot by slot.
5. Writers: DAT_005ff8e0 / e4 — ctor (0) and FUN_003a8b98 only; DAT_005ff8b8 / c0 — the ctor only; the palette
   DAT_0076c208 / 218 / 228 / 250 — FUN_0057ca38 only. Pointer writes are invisible to these searches; RAM agrees.
Still [H]: (a) the Large view's 0.948 vs the depth model's 0.935; (b) why the player's own unit icon is not seen;
(c) the enemy-detection details (`FUN_003a64b8`'s class fields 15 / 60 m / cos 24° are defaults, their key names are
unrecovered); (d) the marker feed (§5); (e) BF1's own minimap range; (f) the names of the unnamed Map / map-info keys.
PROVISIONAL for code: every [D]/[M] constant (0.948; the colours are RAM values of ELF-written globals).

## 10. Verifier (one Opus agent, foreground, claims + citations only — `lead_tmp/re/bmap/claims.md`, 2026-09-30)
25 claims: **20 SUPPORTED, 4 PARTLY / with caveats, 1 NOT SUPPORTED**. NOT SUPPORTED: C24's first half (the `mask` key
is read, §2.3). PARTLY: C6 (horizontal speed; the entity list; the 125 m gate), C19 (modeToggle / hideCPs / the hide
path, §4), C21 (the vehicle-mode scale ramp, §5), C25 (predict.py reads the RAM record rather than recomputing it, uses
the circle shape only and the entity position at +0x150 — parameter-free; re-run: p1 0.20, p2 0.12, p5 0.11 px). Also
folded in: the post number's bit-36 gate, the extra PostSelect pool, MapViewMin's +0x54 side effect, the backdrop draw's
no-mask and Shadow passes, the rebuild-path-only UV update, the Spawn copy taking the shape word, and my misquoted postA
eye (I had copied the v1 frame's). Its ELF checks: every store to Map+0xc3d0 (base + 0x8000, offset 0x43d0) is in the
functions listed in §4 (+1 unreachable `sd` at 0x3a9428); the only stores to 0x5ff8e0 are the ctor's and FUN_003a8b98's;
FUN_003a5228's four jal sites pass 1 / 0 / 1 / 1. Verifier scratch: `/tmp/bmapv/`.

## 11. The flyer's lock events (2026-11-05)

Slots, panel and placement for the Target element's lock states: `re-space-assault.md` §14. `missileLock*` has no writer found.
