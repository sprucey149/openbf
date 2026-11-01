# OPUS RE (2026-10-31) - [O] UNAUDITED

# RE: the soldier's first-person view (BF1 + BF2) — the eye, the aim, the FOV, the toggle, what is drawn

Card FPV. Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1), `analysis/SWBFRNT2.ELF-decomp.c` (B2), read at the
function heads named below. Builds on `re-zoom.md` §5 (the camera and FOV virtuals) and `re-aim-assist.md` §10.2
(the aim path). Tags: [V] read, [D] derived, [H] hypothesis. No live (PCSX2) check yet: the default pad has no
first-person control (§4), so a trial needs the record's flag poked in RAM.

## 0. Headline [O]

| | BF1 | BF2 |
|---|---|---|
| eye point | K+0x100: the shoulder pivot M·T0; on the zoomed path the fire point F (prone M·T0 + row0·(−0.12)) | the posture's pivot M·T0 (table 0x689840), zoomed or not |
| eye | eye point − 0.3·d | eye point − 0.3·d, **0.6·d while the zoom flag is set** |
| frame | looks along the aim d, no tilt; right = normalize(M.up × −d) | the same |
| FOV | the class's `FirstPersonFOV` (+0x1bc), zoomed or not | the same |
| aim, unzoomed | A = F, then the ordinary H ray from A + d·(TrackOffset.z + 2) | the same [D by the twin] |
| aim, zoomed (any weapon) | the scope path: aimer (F, d), no ray | the scope path: aimer (M·T0 + d·T2.z, d) |
| own body | not drawn in his own view | the same |
| suspended | from the thrown state (5) until back in state 0 | set on the change to state 9 or 10 |
| toggle | the press of logical action 5 (K+0x56), gated by `DAT_00519098`; no default control | action 6; no table-0 control; Game Options "Viewpoint" (`bFirstPerson`) |

## 1. The camera [V]

- **B1 `FUN_0020c110`** (the soldier's camera slot). The first-person frame is taken when the soldier is alive and
  either (zoom flag K+0x20 set, a weapon in hand and `FUN_00283d18(weapon)`, i.e. `ZoomFirstPerson`) or the
  player's view record says first person (§3) — and the word O+0xc4c ("first person suspended") is clear. Then:
  - back = −d (d = O+0x3b0 = K+0x110), right = normalize(M.row1 × back), up = back × right;
  - `FUN_001ac6f8` builds the matrix at K+0x100 (O+0x3a0);
  - **position = K+0x100 − d · `DAT_005178b4`**, the float 0.3;
  - unzoomed only: `FUN_0020d040` (the player's index when the record says first person) → `FUN_001a53c8`, which
    copies the camera matrix to `DAT_00580c70` and runs the player's first-person model (`DAT_0057f730 + i·0x420`,
    `FUN_001a5df0`, `FUN_001a2b60`): the FPM arms and their view animations (§5).
  Otherwise the stance section's chase camera (`FUN_001b58f0`, section class+0x1920 + posture·0xc0 + zoom·0x60).
- **B2 `FUN_0024cd00`**. The eye point is M·(table `DAT_00689840` + posture·16), posture = (E+0x7e0 >> 0x16) & 3:
  (0.06, 1.7, 0), (−0.1, 1.2, ·), (0.06, 0.6, ·) — the same T0 as BF1's `DAT_005862f0`. The first-person frame is
  taken on the same two conditions (zoom flag E+0x3e0 byte and `FUN_00325a30`, or the record), unless bit 0x8000 of
  E+0x4e0 is set. **position = eye point − d · (0.3 = 0x3e99999a, or 0.6 = 0x3f19999a while the zoom flag is set)**.
  Unzoomed: `FUN_0024e7b0` → `FUN_001c1cf8` (the FPM model, as BF1).
  - Note: BF2's camera eye is M·T0 even in the sniper scope; the aimer's origin is M·T0 + d·T2.z (re-aim-assist
    §10.2 step 7). The scope's camera therefore stands 0.25 m (standing T2.z) further back than the shot's origin.
- **FOV** (`re-zoom.md` §5): B1 `FUN_0020c450` / B2 `FUN_0024d858` return +0x1bc (`FirstPersonFOV`) whenever the
  record says first person, zoomed or not; the camera eases toward it by exp(−25 dt).

## 2. The aim [V]

B1 `FUN_00204748`: K+0x100 = M·T0 for every unit (T0 at `DAT_005862f0` by posture). With a view object:
- zoom flag set and (`ZoomFirstPerson` or the record says first person) → aimer (F, d), K+0x00 = F, K+0x10 = F +
  200·d, K+0x100 = F — prone: K+0x100 = M·T0 + row0·`DAT_00586340` (−0.12);
- else the record says first person → K+0x00 = A = F (the third-person rebuild of step 4 is skipped), then the ray
  of step 5 from A + d·(section.TrackOffset.z + 2.0), section = posture·0xc0 + zoom·0x60 (the stance's own, zoom
  0), and the aimer (F, normalize(H − F));
- else the third-person path (step 4).

## 3. The view record and the toggle [V]

- The record is per player, static (O+0x558 points at it); its +0x14 word is the first-person flag. When it has a
  controlled entity (+0x10), the class word +0x1b8 (`ForceMode`) forces third (1) or first (2); else the entity's
  "takes the player's preference" virtual (+0x34) decides, else third (`re-turret-draw.md` §4).
- Writers: `FUN_001b72f0(record, player, flag)` (refuses while `ForceMode` is 1 or 2), from
  `ScriptCB_SetFirstPersonView` (the BF2 Game Options "Viewpoint" row, `re-bf2-menus.md`) and from the zoom routine
  `FUN_00204578`: when `DAT_00519098` and the session flag allow it and the press counter O+0x2f6 (K+0x56) is 1,
  the flag is flipped.
- K+0x56 counts logical action 5 (BF1); no control maps to it in any default table (`re-zoom.md` §1). BF2's is
  action 6, likewise unmapped in table 0. A fresh BF2 profile reads "Viewpoint: 3rd Person".

## 4. Suspended [V]

- BF1: O+0xc4c set by `FUN_00209758` on the change to state 5 (thrown) while in first person; cleared in state 0
  once the busy timer is out. While set, the camera takes the chase section and the body is drawn.
- BF2: E+0x4e0 |= 0x8000 on the change to state 9 or 10 in first person (and the FPM model is reset,
  `FUN_001c1538(.., 0)`); its clear was not found in this pass.

## 5. What is drawn [V]

- B1 `FUN_0020cf48`: the body is "drawn" when O+0xc4c is set or the soldier is dead, else "not drawn" in the
  player's own viewport while the record says first person (or zoomed on a scope weapon).
- The FPM model per class: the class key `FirstPerson` = `<FPM lvl>;<model>` (e.g. `IMP\impstrp;imp_1st_stormtrooper`,
  `re-softskin.md`), the arms posed by the 9-track first-person rig with clip tables of 11 columns
  (`re-grenade-throw-anim.md` §4), the `*_1st_weap_*` weapon models, and camera clips (RollLeft, RollRight,
  Crouch, CrouchToProne, StandToProne, ProneToStand, CrouchToStand: `re-stance-roll.md`, B1 `FUN_001911a0`).
- Footsteps: the first-person player steps by `FootstepStride` (`re-foley.md` §4.3).

## 6. The port (2026-10-31)

Built: §1 (eye point, eye distance, frame, FOV ease), §2 (both aim paths), §3 as a profile setting `viewpoint`
reaching the sim each tick as `buttons::FIRST_PERSON` (so replays carry it), toggled by the pause / shell
"Viewpoint" row and by the keyboard's V (ours: the PS2 has no default control), §4 as the stances Thrown, Tumble
and GetUp, §5's body rule. `Unit::first_person_view`, `Unit::eye_view`, `Unit::scope_path`, `Unit::eye_point`,
`core::unit::eye_back`, `WalkSim::eye_camera`. The FOV is the port's chase pair (BF2's 65 / 70 on every map, the
maintainer's 2026-10-28 choice), so first person is 70 everywhere.

The FPM arms, their weapon, clip table and placement: `re-first-person-arms.md` (card FPA). Not built: the camera clips (§5); vehicle
cockpits (the `FirstPerson` key on vehicle classes; seats stay third person); the droideka's first person
(`droidekafp`); the first-person footstep rule; BF2's suspended clear.
