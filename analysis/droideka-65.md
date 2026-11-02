# Droideka, issue #65 - what the port does and where it rests (cards DEKU, DEKS, DEKD, DEKF)

Implementation notes for the droideka branch (issue #65). The RE is `re-droideka-unit.md` [O] (which corrects
`re-droideka.md`); this file only records what the code takes from it, the disc values read on top of it this pass,
the rules that are ours, and what stays open. Tags as in the RE: [V] read in the decomp or on the disc, [M] measured,
[D] derived, [H] hypothesis, OURS = a port decision with no original behind it.

## 1. Rules: decided vs proposed

Decided and followed:

- One gameplay, both games (PLAN, the maintainer 2026-10-27). The droideka's class, shield and weapon numbers are
  read from the BF2 play db on both discs (`WEAPON_DONORS` gains `cis_inf_droideka -> cis_inf_droideka`): health
  550, shield 3000 / -100 /s up / +300 /s down, the repeater's BF2 bolt (80 damage, 160 m/s, 0.8 s). BF2's full
  shield does not drain (`ShieldClass::drains_full` false); BF1's draining behaviour is kept in the type and the
  tests, not used.
- The model, clips, camera sections, sounds and death explosion stay the native disc's (`droideka_class(native,
  play, class)`), so BF1 still sounds and looks like BF1 (BF1's `droid_fx7_hum` shield loop; BF2 authors none).
- A droideka is its own entity: `Unit::droideka` holds the 14-state form machine, the ball integrator and the
  shield; soldier stance, movement, equipment and drawing skip it. `UnitSpec::droideka` rebuilds it whole on respawn.
- Fielding: BASE chain `walkerdroid` builds as a droideka (`is_droideka_chain`); the mix lists keep it in both CIS
  pairs; its geometry is not a soldier geometry.
- The AI (DEKF, RE section 5): the form order defaults to roll; a fight writes deploy; leaving the fight writes
  roll. A fight starts from sight only within 10 m, or when hit in the last second, never against a flyer.
  Deployed, in the fight, shield down and shield charge over half capacity -> raise it (independent of health). It never lowers it.
- The player: the crouch / roll-stance press folds and unfolds; the secondary press toggles the shield; fire is
  blocked in fold, ball and unfold (RE section 6).
- Unlocks: the existing `unlock_points` path, read from the class's `PointsToUnlock` like any other class
  (UNLK / UNLK1); nothing droideka-specific.

Proposals in the cards that this branch does not claim as decided: the DEKF census and bot-share thresholds are
acceptance measures, not rules; the bot design (`bot-design.md`) still has no droideka section.

## 2. Disc values read this pass

| what | where | BF1 | BF2 |
|---|---|---|---|
| Shield bubble, `fx__` `DroidekaShield`, type Shield, shape "droideka" (flag 0) | CIS.LVL | Color 96 160 192 192 | Color 96 160 192 255 |
| same, both | | Radius 1.25, Height 1.25, ScrollRate 1.0, SpecularColor 216 240 255, Grow 0.15 s, Shrink 0.15 s, key 0xb28af6f4 = 1.5 (unread) | |
| Bubble timing | B1 FUN_0037c390 / FUN_0037c268 | grow f = s < 0.94 ? s x 1.1276596 : (1 - s) x 0.999999 + 1; shrink f = 1 - t / 0.15 | |
| Bubble draw | B1 FUN_00388ee8 | shape 0: the "specular" table, camera rotation x scale at the centre; record 0x48 (Cs.As + Cd) outside Radius, 0x68 (Cs.0.5 + Cd) inside; colours halved | |
| Engine block (class +0x16f0) | ODF | EngineSound `cis_walk_droideka_engine_parameterized`, TurnOnSound `cis_droideka_fold`, TurnOffSound `cis_droideka_unfold`, TurnOffTime 3.0 default | same |
| Engine on | B1 FUN_001d1540 B1:79627-79690 | states 0xb, 0xc (IdleToRoll, Rolling) | same |
| Shield sounds | shield class 0xd85503ff / 0x5df6c1a4 | `droid_fx7_hum` / - | - / - |
| Camera sections (hashes 1967731491 / 2497170924 / 1313842571) | ODF CAMERASECTION | STAND eye 1.8, TrackOffset 0 0.5 4.5, tilt 5; STANDZOOM tilt 1; CROUCH eye 1.0, 0 0.3 4.5 | same |
| Footsteps | class +0x180c / +0x1814 / +0x181c | `cis_inf_droideka_step` x3 | same |

Every name above exists in the baked `names.ron` of bf2_kam1, bf1_nab1 and bf2_geo1; `droid_fx7_hum` only on BF1.

## 3. Deviations and open facts

PROVISIONAL [H] or OURS, each named so it can be replaced when read:

- Bubble: see "The bubble" below; one layer of the sphere is OURS.
- Camera: the ball uses CROUCH [H]; the SPLIT* sections are unused (no split screen).
- Sounds not played: the footsteps (no play site found, as the walkers' VSN2 gap); the shield weapon's FireSound
  `cis_droideka_shield_on` (the infinite-ammo fire path does not run [H]).
- HUD: the shield charge is the stamina bar and BF2's `hud_droideka_shield` the secondary row, on both games (the maintainer, 2026-10-30; OURS for the bar). The primary shows the repeater's `hud_vehicle_lasers`.
- Death: BF1's five CHUNKSECTIONs are not spawned (chunk models not baked).
- From the sim and draw stages: recover order deploys (RE: walk out of a stuck spot); the AI clears SPRINT and
  ACTION; the legs offset is not drawn; ball spin drives the roll clip time; the ball lean is a model rotation;
  ordnance prims are bind-pose spheres; BF1's untagged prims follow one rule; droideka voices are exempt; the walk
  uses the human yaw and mouse turn; the player spawns deployed; the main bar shows shield charge; issue #85 adds a refusal message for the ball jump's separate energy; the disc's sprint-button ball boost is fed by no button (the maintainer, 2026-10-30: no sprint in either form); the barrels alternate [H];
  explosions spend the shield first [H] (RE section 8); the ball snaps to the floor; no crash damage.
- RE section 8 items stay open: explosion damage flags, aimer alternation, BF1's fold button, the evade dodge-roll,
  first-person `droidekafp`.

## 4. Evidence

Before: `ClassRoster::build` rejected `cis_inf_droideka` (BASE `walkerdroid`), so no mission fielded one.

After:

- `game/tests/suite/roster.rs` `droideka_chains_build_as_droidekas`: both discs build the class as a droideka with
  no soldier geometry, the engine / fold / unfold names, the STAND and CROUCH sections above, and the shield loop
  (BF1 `droid_fx7_hum`, BF2 none).
- `core/src/sim/droidekas/tests.rs`: bot fold-at-spawn and roll, player walk / fire / fold / roll / unfold, the
  raised shield taking bolts with no body damage, the lowered shield passing them, a blast spending the shield
  first with no knockdown, burst death with no corpse and a whole respawn, water damage, replay hash determinism,
  and the three AI cases (stays rolled with a far enemy, deploys / shields / fires at a near one, rolls again after).
- `game/examples/droideka_look.rs` on bf2_kam1_con_cw and bf1_nab1_con_cw: frames of a bot droideka deployed,
  shielded, folding, folded, rolling, unfolding and bursting, and a player droideka walking and firing, shielding,
  folding, rolling about 11 m and unfolding, with the camera switching section (walk eye about 3.0 m over the feet,
  ball about 2.0 m).

## Audit clarification — 2026-10-30

Droidekas deliberately have no zoom option. Their shield AI tests remaining
shield charge, not soldier health. These are intended behaviors; these
audit fixes preserve both.

## The bubble - 2026-10-30

BF1 `FUN_00388ee8` / BF2 `FUN_0046e378` (records `FUN_0046e7e0`), mesh BF2 `FUN_0046ce30`, timing BF2 `FUN_00460030`. All [V] unless tagged.

- Mesh: a procedural whole sphere, 24 x 8 quads (15 and 22.5 degree steps), its pole along the camera's back axis, turned with the camera. No UVs, no authored texture: the `fx__` record's `Texture` (`cis_inf_droideka_shield`) and `ScrollRate` are read only by the shape-1 branch (BF1 NAB1_H's `fambaa_shield`), so nothing scrolls on the droideka. Key 0xb28af6f4 = 1.5 has no reader found.
- Pass 0: flat colour under vertex alpha 255 sin^6(theta), ALPHA 0x48 (Cs.As + Cd), Gouraud, depth tested, no depth write. Inside the unscaled radius: flat, ALPHA 0x68 FIX 0x40 (Cs.0.5 + Cd). Bytes halved `(b + 1) >> 1`: BF2's rim adds up to 48 80 96 and the inside adds 24 40 48 (measured on the port's frames, shield up minus down).
- Pass 1: the shared `specular` table, `s = 16 / 255`, `t = 0.5 + 0.5 d` (`render::spec_texcoord`), in `SpecularColor` 216 240 255, Cs + Cd.
- No ripple, flash or pulse on a hit, and the look reads no charge: only the sound's gain does. A hit's own impact effect is the existing `Surface::Shield` path, unchanged.
- [H] and open: VU1 0x2da-0x2fd reads literally as 255 + c(b - 255), a warm rim; the BF1 frame `bf1_yav1_cp2_viaduct_viaduct_spawn` measures (33, 57, 66) against c.b's (36, 60, 72), so the port takes c.b. Whether back faces draw is unread: OURS draws one layer (the half toward the camera, or ahead of it from inside), which that frame's falloff fits. BF2's colours are predicted from the same code, not measured on a clean BF2 frame.
- Rim against that frame (issue #85, 2026-11-01; its state's RAM has the shield on): fitted to the same size, 33.3 px at 640x480, the port's peak adds 30 50 60 against the frame's 29 50 59, and green through the fade from 0.94 to 0.70 of the radius is within 4 levels (`the_rim_matches_the_ps2_frame`). The outer edge falls in about 2 px on both. So the crisper edge the issue described is no longer there and nothing was retuned. It stays one BF1 frame: no existing frame has a BF2 shield near enough to measure.
- No Shield record exists on BF2's Polis Massa, Mygeeto or Hoth maps: a scan of every LVL's `fx__` finds three, `fambaa_shield` and the two `DroidekaShield`.
