# Walker aim over the walking pose — issue 67



Implementation on the walker-aim branch, based on `79275f6`; worktree: the walker-aim checkout. Source stays separate from droidekas (#65). No push or PR is authorized.



## Scope and ownership



Read AGENTS.md, PLAN.md, analysis/INDEX.md, re-walker-nodes.md, re-turret-draw.md 2.2/2.3, and VPOSW/VPOSE before implementation. The requested `engine/artifacts/loop/STATUS.md` is absent, including in the maintainer checkout; checked-in VPOSW says todo and VPOSE remains blocked. On 2026-10-29, GitHub #67 had no assignee or comments; #65 had LightVelox's claim. Upstream main was `95f7d03`, already included in the starting checkout. Open PR #77 is the effects/destruction work already present at this base. No walker implementation or competing walker worktree was found. Shared files receive narrow changes; no droideka loading or rendering is changed.



## Demonstrated AT-TE cause



[O] Local BF2 REP.LVL, animation bank and skeleton, plus baked GLB inspection. The idle clip's untranslated `aimer_maincannon_x` TNJA base is `(0.000057984, 2.6636579, 4.3353004)`. Its skeleton parent is `aimer_maincannon_y`; the model's correct parent-local pivot is `(0.000057984, 1.3075151, 0.7251549)`. The bank rig has the same parent, with rest position `(0.000057984, 1.0580754, 0.7251549)`. The stored track base includes the parent's `(0, 1.6055824, 3.6101456)` offset. Applying it as parent-local adds the mount offset twice. The resulting extra displacement is `(0, 1.3561428, 3.6101456)` in model axes, explaining the floating cannon even at zero aim.



This is already wrong in the baked walk clip, independently of the absent aim overlay. The original aimer writer replaces this table entry from the skeleton-local bind pivot through its current parent: re-walker-nodes 3, re-turret-draw 2.3. The bake now restores the untranslated, class-authored `aimer_maincannon_x` track to that model-local pivot. The repair is deliberately limited to this demonstrated cannon node. Animated translations, walk tracks and every other node stay unchanged. Both games have this AT-TE track issue. A targeted rebake reproduced byte-identical GLBs for the eleven other walker skins; only BF1/BF2 AT-TE GLBs changed.



The one-off `walker_trace` and `walker_rebake` examples that inspected the source bank and rebaked skins alone were removed in the sprint's code cleanse (in git history before `sprint/org-code`). The regular `bake vehicles` path applies the same fix.



## Runtime behavior



`SeatSpec::aimers` retains every authored aimer, per-node yaw/pitch limits and barrel associations alongside the existing simulation-facing first-aimer fields. No existing simulation values or parsing semantics are replaced. Skeleton names resolve parents, local axes and offsets. The visual rig reads each side's class database.



`WalkerAim` resolves parents before children, including skeletons whose storage order puts children first. It starts from the existing walk pose, uses the posed parent for aimer pivots and clamp frames, and uses the existing turret/aimer/barrel matrix laws. Turret nodes use seat look, authored limits and PitchTurnFactor; turret-section aimers use the full, unscaled turret aim frame. Body aimers clamp in their bind frame carried through the current walk parent. Body aimers take priority over mounted turret nodes at the same name, implementing BF1 AT-AT's head rule. BODY TurretNodeName is ignored, preserving BF2 one-man AT-ST's walk-posed aimer_y.



Affected classes: BF1/BF2 `imp_walk_atst` plus jungle/snow variants, `imp_walk_atat`, `cis_walk_spider`, `rep_walk_atte`, and BF2 `rep_walk_oneman_atst`.



The draw palette, `WalkerRig::node_now`, PilotPosition and AnimatedPilotPosition riders, and walker muzzle frames all consume the same final nodes. Visual bolt starts and muzzle flashes use the posed fire point; projectile simulation and hit endpoints are unchanged. Other vehicle types keep their existing path.



Zero look/aim, or missing gun state, retains the walking palette exactly. The AT-TE's independently broken baked cannon offset is corrected even at zero aim. There is no turn-rate simulation or recoil-state addition.



## Limits and original-game evidence



Steady-state directions use the seat's existing model-space aim, projected 512 m from its bind eye/pivot, then clamped per aimer toward that point. This is the documented provisional per-seat proxy: the original uses per-weapon aim points, collision ray results and rate-limited direction state. This implementation does not add those simulation fields. No gameplay, input, weapon damage, projectile trajectories or core hash code is changed.



Existing original-game evidence is re-walker-nodes' recorded PS2 node-table fits (BF1 1205/1206, BF2 residual <=0.04) and its BF1 AT-AT priority/one-man inventory, plus the checked-in turret node-law fixtures. The 1798-row `walker_nodes_ps2.txt`, original walker save states and the maintainer's cited screenshot/snapshot are absent locally. No fresh PCSX2 walker capture, independent recheck of those archived RAM claims, or matched PS2/port image comparison is claimed. That acceptance remains open.



## Checks and local artifacts



Artifacts are local only under `engine/artifacts/walker67/`; copyrighted geometry and captures are not committed or uploaded.



- `before/rep_walk_atte_rest.png`: original source and bake reproduce the floating cannon.

- `after/rep_walk_atte_rest.png`, `after/rep_walk_atte_aim.png`: corrected mount at rest and while aiming, with a boarded gunner.

- `walk-control/imp_walk_atst_rest.png` and `after/imp_walk_atst_aim.png`: matched camera, walk-only control versus 60-degree aim. Rest outputs are byte-identical; changed RGB pixels are confined to the head/guns rectangle `(441,288)..(535,372)` in the 960x720 image. Synthetic skin tests also prove the animated leg matrices are unchanged.



Final check results and distribution receipt are appended at handoff.



### Final results



- `cargo test -p game --test walker_nodes -- --include-ignored`: 8 passed, including the local real-data test over all nine walker class families. This covers piloted 60-degree AT-ST skinning, animated legs, child-before-parent storage, per-aimer limits in a rotated walk frame, exact zero-aim palettes, driver-over-gunner BF1 AT-AT head priority, parsing, hash stability and the AT-TE mount at rest/aimed.

- `cargo test -p game --test suite vehicle_rider`: 5 passed. The end-to-end rider case now includes BF2 Geonosis AT-TE's boarded gunner at nonzero aim and checks his posed fire point through `SoldierVisuals`.

- Library suites: bake 185 passed; core 1279 passed / 2 ignored; game 530 passed / 5 ignored. The bake regression proves that the static cannon repair leaves ordinary walk and animated translations alone.

- `cargo clippy --workspace --all-targets -- -D warnings`: passed. `git diff --check`: passed. Changed Rust files are formatted and stay below the 1500-line ceiling.

- Full game integration suite: 299 passed, 6 ignored, one unrelated navigation table timing assertion failed at 0.323 seconds while other builds ran. That exact test passed when rerun alone. No navigation code was changed.

- Full `cargo fmt --all -- --check` reports existing formatting in untouched `bake/src/effects.rs`, `mungefs/src/lightbeam.rs` and `render/src/light_beam.rs`; those unrelated files were preserved.

- Headless captures were inspected. `after-close/rep_walk_atte_aim.png` shows the cannon on its mount; capture logging confirms a boarded player at seat 1, a posed rider matrix, and one submitted skinned soldier draw at both rest and aim. There is no manual playtest or new original-game comparison.



### Distribution



Release game rebuilt successfully, then its existing `--write-pack` command packed the current `engine/assets_baked` and `engine/missions` into `dist/openbf-walker67-ready.exe`. The staged result was checked before replacement: 23,997 entries, 2,755,787,212 bytes; every path and source size matched; all walker skin and mission payloads (77 files) matched byte-for-byte. Executable prefix matched the release game exactly, SHA256 `caf84ad08c28ac945deb4894b6bb41ea4bb998562ed0c498cd9aa06910defdf7`.



Installed successfully as `dist/openbf.exe`. No staged executable remains. `dist/openbf_data` profile/playtest paths, lengths and modification times were unchanged across replacement. Receipt: `engine/artifacts/walker67/pack-receipt.json`.



Source remains uncommitted on the dedicated feature branch/worktree; the maintainer checkout's source stays clean. No push or PR was performed. Original-game row replay and matched PCSX2 pictures remain open acceptance, so VPOSW is not marked complete. (Superseded by the 2026-10-29 merge section at the end of this note.)



## Close-range disappearance and animation stepping follow-up, 2026-10-29



The maintainer reported two screenshots of the AT-TE disappearing when approached, followed by laggy/jittery animation. Read the packed game's black box before editing and preserved `last.json` / `prev1.json` under `engine/artifacts/walker67/close-followup/`. They are BF2 GEO1, seeds 157 / 156, build `79275f6170a8+dirty`, built 2026-10-29 22:02:51 UTC, the previous walker feature build. Checkout HEAD is still that base. In seed 157 at tick 1477, vehicle 12 `rep_walk_atte` is alive at `(9.31,-62.94,305.18)`, health 37324, three riders. In seed 156 at tick 2226 it is alive at `(9.48,-63.05,280.51)`, health 39660.5, three riders. The vehicle was not being despawned.



The camera's vehicle fade code treated the box enclosing every hull body as filled: entering any space within that aggregate box set the entire vehicle opacity to zero. A reproduced AT-TE has aggregate bounds `(3.786,-62.940,313.261)` through `(15.282,-54.702,333.517)`, including the open space under its belly. The close-camera fixture under the belly changed from opacity zero to fully visible after replacing the broad-box containment test with actual primitive/surface overlap. Real line-of-sight obstruction still uses the existing hull cast. Additionally, a camera intersecting one foot can make a walker translucent, but cannot hide the whole walker outright; other vehicle types retain their previous opacity policy.



The animation issue is separate: render_blend interpolated the vehicle position/rotation while explicitly leaving the walker animation clock, leg twist, body pitch/roll and leg layers at 30 Hz. It now captures and interpolates those visual fields and walker gun aim along with the existing vehicle pose. Clock interpolation handles forward/reverse clip restarts; switching to a different clip keeps that clip's own clock. Respawns and teleports keep the existing snap rules. Saved tick values are restored exactly before simulation resumes. Riders now resolve their walker attachment at draw time from the same interpolated nodes, since their animation update happens before render interpolation in the windowed loop. Simulation and input remain unchanged.



`walker_visibility` (removed in the sprint's code cleanse; in git history) captured the original bounding-box policy and corrected policy at identical close-camera poses; images and logs are in `close-followup/captures/`. Tests cover empty space between legs, sphere-AABB corners, genuine obstruction, camera-inside-foot visibility, fractional-frame leg vertex movement, angle seams, forward/backward clock wrap, clip changes, rider and muzzle attachment during interpolation, and exact tick/hash restoration. This corrects 30 Hz visual stepping; it does not claim to eliminate the independent GPU/frame-time spikes recorded by the snapshot.



Final follow-up checks and distribution receipt are recorded below.



### Follow-up results and installed build



- Walker regressions: 10 passed, including the real-data class check and new between-tick vertex/hash checks. Close-camera regressions: 2 passed. Render interpolation tests: 9 passed (clock wrap, angle seams, existing input preview and replay/hash checks). Rider integration: 5 passed, including attachment and muzzle checks while the displayed walker pose is interpolated.

- Game library: 533 passed, 5 ignored. Includes the renderer guard preventing a camera inside a foot from hiding an entire walker. Game all-target Clippy passed; the final capture example also passes Clippy. Game formatting, diff whitespace, added-comment and changed-file size checks pass. No new line or block comments were added to Rust.

- Inspected headless before/after close-camera captures. Under-belly old policy: `(vehicle 12, opacity 0)`; corrected policy: no fade. A real foot intersection uses the walker's nonzero translucent floor in the draw path. No fresh manual playtest or live original-game comparison was performed.

- Rebuilt release and packed current assets_baked/missions into a temporary executable, verified archive paths/sizes, its release executable prefix and all 77 walker-skin/mission payloads. Runtime-only follow-up: no additional baked asset change was needed; the previous AT-TE asset repair remains packed.

- Replaced `dist/openbf.exe` successfully; profile/playtest file metadata was unchanged. 23,997 embedded files, 2,755,800,524 bytes. Prefix SHA256: `36f9e699e227ecf17cb14b60e7953fa936d7e226ff1bc8efbba30b2f877c1f08`. Receipt: `engine/artifacts/walker67/close-followup/pack-receipt.json`. No staged executable remains.

- Droideka worktree source was not changed; its touched paths do not overlap these follow-up implementation files. Maintainer checkout source remains clean. No commit, push or PR was made.


## Riding controls and armed-seat follow-up, 2026-10-29

The maintainer confirmed the visibility repair, then requested original-like walker riding: free horizontal look with the body following slowly, and access to both other AT-TE gun seats. This explicitly expands the earlier visual-only scope to these demonstrated control and gunner issues. Corrected control/gunner cases intentionally change simulation results; the earlier visual-pass hash claim applies to that pass, not to these input corrections. Ground handling already uses the BF2 walker law across source games; this does not add BF1's separate native locomotion model.

Read the current black box first: BF2 GEO1 seed 158, tick 1404, build `79275f6170a8+dirty`, built 2026-10-29 22:24:25 UTC. Human unit 0 occupies vehicle 12 seat 0 throughout 193 stored rows; its occupants are `[0,8,9,-1,-1,-1]`. Both gun seats are held by bots. The snapshot belongs to the previous close-range feature executable, not the new control code. Preserved snapshot/digest: `engine/artifacts/walker67/controls-followup/`. Upstream and issue #67 were checked again before this follow-up: no new comments or competing walker change.

### Original-source findings

Current local corpus line numbers differ from older research references. Rechecked enclosing functions with:

```
python scripts/re_check.py fn b2 177638 177643 164686 164807 188615 188670 190941 190950 190490 190496
```

- BF2 `FUN_0025d5e0` (header 177335; rows 177638/177643) uses the yaw limits as a body-follow trigger: outside the window it commands twice the angular excess, limited by the authored turn speed. The yaw helper rotates the body by twice its half-angle, and the aim compensation preserves the world viewing direction. This locomotion already existed. The mouse path incorrectly hard-clamped the view before it could request that following turn.
- BF2 `FUN_0024b4d0` (header 164595, player-index gate 164686, displaced-occupant handling 164807 onward) permits a human to take an AI-held seat and reseats the displaced occupant in a free boardable seat. Other players are protected; passenger-to-passenger cycling is excluded. BF1 `FUN_0020b5e8` has the same human/AI distinction (rows 120014-120016 and reseating 120102-120118). The port only accepted empty seats, explaining the reported inability to reach either gun.
- MountedTurret class setter `FUN_00270770` (header 190729, rows 190935-190954) sets byte K+0x844 when yaw min is positive and at most pi, and yaw max is at least pi and below 2pi. Tick `FUN_0026cea0` (header 188053, rows 188615-188629 and 188670-188699) uses that positive-angle domain and clamps it. BF2 AT-TE rear guns author 120-240 degrees; treating those limits as a full circle was wrong. Incremental look clamps directly; absolute signed target directions are mapped into the positive domain before clamping.
- Unzoomed MountedTurret camera `FUN_00270018` (header 190339, rows 190487-190497) passes the turret aim frame T+0x1c0 to the camera section. That frame's translation is the turret mount, as described by re-turret-draw 2.2. The riding capture demonstrated that our main-cannon camera instead used the vehicle origin, placing it inside the belly.

### Implementation and class data

Mouse pilot yaw can pass the physical gun window; pitch retains the class clamp. The existing original-law locomotion follows at its authored rate and preserves the world view as the hull turns. No new turn integrator or hand-tuned follow speed was introduced. Unconsumed mouse preview uses the same rule.

A human can cycle into friendly AI-held boardable seats in either direction. The displaced bot remains aboard, moves to a free boardable seat, and preserves its bail draw/RNG state. Other players and enemies cannot be displaced. Q / wheel down advances; wheel up reverses. The normal edge gate, zoom reset and passenger rules remain. Seat switching stays in its own module; droideka-owned `sim/seats.rs` is unchanged. Its old hard-clamp assertion in `sim/seats/tests.rs` was updated to the newly authorized rule.

Walker gunner sections now use their authored MountedTurret rates, filters and camera limits, with the aimers retaining their separate limits. The main AT-TE gun is unrestricted in yaw, with turn speed 1.25 and pitch -15..15 degrees. The rear seat uses yaw 120..240, pitch -17..20, rates 1/1 and filters 3.75/3.75. Its aimer has a backward bind frame; retaining that basis fixes its resting aim, chase frame and muzzle direction. Pilot aimer limits now come from the authored pilot aimer rather than the hover-only zero-yaw setup. Hovers and flyers retain their existing setup paths.

The walker main gunner camera receives its turret mount translation instead of starting at the vehicle origin. This is a bounded repair of the demonstrated wrong origin. The existing provisional camera section behavior (including chase FOV and flattened heading) remains; a full replay of the original section formula and animated camera-mount position has not been implemented or claimed. Other vehicles' cameras are unchanged.

### Evidence, checks and remaining acceptance

Headless riding views are in `controls-followup/captures/`: pilot front, pilot looking 115 degrees while the hull is still sideways, the same world view after 180 ticks of hull following, main gun and rear guns. `main_gun_before_mount.png` preserves the demonstrated inside-belly camera. These captures use actual loaded Geonosis class/mission data and real seat switching; they are not original-game captures. The fixture keeps all three riders aboard and fires the actual pilot, main-cannon and rear-gun weapon classes.

No fresh manual playtest or matched PCSX2 ride capture was performed. Original RAM-row node acceptance from VPOSW remains open. The general per-weapon aiming/raycast limitations and camera-section limitations above remain explicit; this follow-up does not claim full original-game equivalence.

Shared-file integration note: `core/src/ai/ride.rs` has only the new identity `GunSetup::aimer_bind` fixture field, while #65 edits its AI implementation. Preserve that contributor's changes when combining branches. `game/src/walk.rs` has only the walker gunner camera-origin fix; #65 also changes that file. The existing ride/walk files exceed the size goal at the base; these narrow additions avoid an unrelated split of contributor-owned files. Code-map changes are additive. A local coordination note is kept under `engine/artifacts/loop/walker67-coordination.md`. No accessible active droideka chat was identified; no message was sent to an unrelated chat and no #65 source was overwritten.

Final check counts and distribution receipt follow below.

### Control follow-up check results

- Core library: 1287 passed / 2 ignored. Covers free mouse yaw plus pitch limits, world-view preservation while the hull follows at the authored rate, original full-circle wrapping, positive-domain rear arc limits and AI tracking across the signed seam, bind-direction/muzzle consistency, AI-seat takeover/reseating without RNG consumption, protected occupants, reverse cycling, passenger exclusion and existing input/replay rules.
- Game library: 533 passed / 5 ignored. Walker suites: 14 passed (10 node/pose, 2 close-camera, 2 real Geonosis control/weapon cases, including the normally ignored local-data cases). Rider integration: 5 passed. Turret-view integration: 6 passed. Both real gun seats fire their authored weapons with all riders still aboard; rear aim points backward and its mouse view clamps to 120/240 degrees; the main gunner camera is above the hull at its mount.
- `cargo clippy -p core -p game --all-targets -- -D warnings`, package formatting and diff whitespace checks passed. No new line/block comments were added. The existing oversize exceptions are narrowly changed `ai/ride.rs` (1745 to 1746 lines, one fixture field) and `game/walk.rs` (1669 to 1676, walker gunner camera origin); see coordination note.
- Full core regression initially caught an obsolete hard-clamp assertion and the previous visual follow-up's standard-math calls in camera overlap/interpolation. Updated the obsolete control assertion, used multiplication/square root for hull overlap and routed interpolation trigonometry through the existing core math wrapper. The final complete core/game runs above pass. No ignore/allowlist was added to bypass these checks.
- Inspected the final headless riding views. Main gunner before: camera at body height +3 m, inside the belly. After: mount offset raises it to +9.0892 m, with a clear view over the cannon. The 115-degree pilot view is held at world yaw -2.0 radians while the body follows over 180 ticks; all three riders remain aboard through both switches. The images and logs are local artifacts. Original-game manual/row acceptance remains unperformed as stated above.
- This follow-up changes runtime code only. The previously completed BF1/BF2 AT-TE cannon skin rebake remains current; no additional affected baked asset requires rebaking.

### Control follow-up distribution

Release was rebuilt successfully from the isolated walker branch and copied to `controls-followup/game-unpacked.exe` before packing, protecting the selected build from later shared-target activity. Its `--write-pack` command packed the current main-checkout `engine/assets_baked` and `engine/missions` into a temporary executable. The current source tree contains 24,003 packed files; all paths and sizes match, the executable prefix is byte-identical to this release build, and all 79 walker-skin/mission payloads were checked byte-for-byte. Packed size: 2,757,977,797 bytes. Prefix SHA256: `782ab3b9347b015a783564cdd09655f2d5b9e603ed8bb7ba2054f569a26e03f4`.

Replaced `dist/openbf.exe` successfully only after those checks. `dist/openbf_data` profile/playtest file lengths and modification times are unchanged. No staged executable remains. Receipt: `engine/artifacts/walker67/controls-followup/pack-receipt.json`.

Source remains uncommitted on the walker-aim branch in the walker-aim checkout. Droideka #65 remains in its own worktree/branch and is not included in this executable's source. Current shared baked assets were packed as requested. No push or PR was made. This handoff fixes the demonstrated riding controls and camera origin; it does not close the unperformed original-game acceptance or the provisional camera/aiming gaps listed above.

## Seat camera, tracers, crosshair and gun sounds follow-up, 2026-10-29

The maintainer asked for the rest of a manned walker's presentation: the gun seat's view should ride the animated mount, walker shots should draw their authored tracers from the barrel, an armed seat should show the crosshair and its own weapon rows, and the guns should be heard. This is the presentation half of the same issue; simulation, damage, projectile paths and core hashes are unchanged.

### Seat camera

`WalkSim::walker_seat_camera` (`game/src/walk/walker_view.rs`) replaces the shared vehicle camera for walkers. It builds the seat frame from the posed walker: the pilot's aim frame for seat 0, a rig seat frame (animated turret parent plus bind local, then the turret look) for the other seats, and the existing turret node at the gun pivot as the fallback when no rig is loaded. `camera_on_frame` takes that frame's forward/up/right axes, applies the seat's `TiltValue`, and derives yaw, pitch and roll from them, so the view inherits the hull's and the mount's real orientation instead of a flattened heading. The camera sits at the seat's `TrackCenter`/`TrackOffset` in that frame, and `fov_y` already resolves the seat's `ThirdPersonFOV` through the PS2's 0.7666 vertical squash and the zoom blend, so the authored gun-seat FOV is used. The HUD reticle is placed where the aim ray lands on screen (`hud::reticle_y`), not on a fixed line.

The unit test `walk::walker_view::tests::camera_matches_paused_geonosis_main_gunner_frame` pins the paused Geonosis main-gunner frame; the real-seat checks below exercise all three AT-TE seats.

### Tracers and gun sounds

`game/src/walker_shots.rs` reads each walker weapon's authored presentation: the tracer look from `SecondaryOrdnanceName` (class, glow and blur lengths), the fire sound from the class table's `0x2298cae8` property (the AT-TE rear gun resolves to `rep_weap_walk_atte_chaingun_fire`), and `SecondaryOrdnancePeriod` as the tracer interval (3 on that gun, so every third shot draws a streak). `frame` resolves a shot's fire point through the same posed rig the muzzle flash uses, and `ordinal` maps the event to the weapon's shot counter so a multi-barrel seat alternates its authored points. The drawn streak starts at that point and ends at the sim's impact; the sim's projectile path is untouched.

`SoundLib::walker_fire_loops` (`game/src/sounds/walker.rs`) keeps one positioned loop voice per firing seat/weapon, gated on the same `fire_cmd`, `shots_fired`, reload and overheat state as the sim, and drops it when the trigger is released. One-shot `Event::Shot` sounds keep their existing path, so a burst is a loop plus its shot reports, positioned at the walker.

`WarmUpSound` (`rep_weap_walk_atte_chaingun_spin` on the AT-TE rear gun) is authored on the same guns but is deliberately not read: the sim has no spin-up phase, and firing starts on the first held tick. Playing it on an invented trigger would misreport the original, so the field is not carried; this remains a known gap.

Update 2026-11-13: the streak presentation above was a port invention and is gone. On the PS2 each round draws as its primary ordnance's own bolt (`com_weap_veh_chaingun_ord`: `com_sfx_laser_orange`, LaserLength 45, FadeOutTime 0.25, LightColor `255 224 160 150`, LightRadius 3, so BF2 lights the ground under it), and `SecondaryOrdnanceName` `com_weap_veh_chaingun_tracer` is a `beam` (width 0.05, Range 40) that the sim already launches on rounds 1, 4, 7, ... and `BoltVis` already draws with the beam's own look. The port hid every regular round and drew a 40 m streak every third round, so the gun showed the same two streaks, a gap, and no light. `walker_shots::Style` now carries only the fire sound; `walker_shots::turn` replaces `ordinal` and also counts the barrels that passed their turn. The sim now has the spin phase (`wind_up` / `wind_down`, see `re-blaster-damage.md` §3.1), but `WarmUpSound` is still not played.

Update 2026-11-13 (second playtest): the rear gun still drew leftover streaks and fired "from the left weapon" when turned right. The seat has no `TurretNodeName`, so `GunSetup::pivot` was the second barrel's fire point and the sim turned both barrels about it; the drawn rig turns each barrel about its own aimer (`aimer_Rgun_1` (-0.834, 4.97, -6.096), axes turned pi; `aimer_Rgun_2` (0.819, 4.97, -6.096); `analysis/re-turret-draw.md` 2.3). Turned 1 rad, the first barrel's rounds and their tracer beam left 2.9 m from the drawn barrel, beside the other one. `GunWeapon::fire_pivots` now holds each barrel's own aimer node (filled by `vehicle_pads::fire_ring` when no aimer carries another, cleared on seats with a turret node), and `GunSetup::muzzle_about` turns the point about it. Still left: the drawn aimers converge on the aim point while the sim's per-barrel aim is parallel, about 0.37 m apart at 1 rad. The fire loop now plays only with rounds and the spin motor is played (`re-blaster-damage.md` §3.1). Tests: `walker_gun_aim::an_atte_gatling_round_leaves_from_the_barrel_drawn_firing_it`, `a_pressed_atte_gatling_gun_plays_its_fire_loop_only_with_its_rounds`.

### Armed-seat HUD

`hud_feed::armed_seat` reports a live mounted player whose seat has a weapon. `weapon_rows` then returns the seat's own rows (icon, key, clip fill, rounds or heat from `Vehicle::guns`) instead of hiding the HUD when mounted, and `main/battle.rs` shows the reticle for such a seat. Empty passenger seats keep the previous HUD behavior.

### Evidence and checks

- `engine/artifacts/walker67/presentation-followup/` holds the run logs, the seven-entry `checks.json` (all exit 0) and the captures.
- At that pass: core 1287 passed / 2 ignored, game library 534 passed / 5 ignored, walker suites 14 passed with `--include-ignored` (2 close-camera, 2 real Geonosis control/weapon cases, 10 node/pose), seat-camera unit test 1 passed, and `cargo clippy -p game -p render --all-targets -- -D warnings` clean.
- After the droideka merge below: core 1328 passed / 2 ignored, game library 549 passed / 5 ignored, the same 14 walker cases pass, and the same Clippy command is clean.
- Headless Geonosis captures (`captures/`): main gun, rear guns and the three pilot views. The main-gun camera sits at `(5.9531, -53.8508, 324.8017)` above the hull and looks along the barrel; the rear-gun camera sits at the rear mount (`(15.5708, -52.9400, 322.3424)`), with all three riders still aboard.
- These are port captures from real class/mission data, not original-game pictures. No fresh manual playtest or matched PCSX2 capture was made for this pass. The provisional per-weapon aim proxy and the unplayed `WarmUpSound` remain open as recorded above.

### Merge with the droideka work

The walker-aim branch (`baa84e9`, based on `79275f6`) is merged into local `main` on top of the droideka commit `8bea4fd` (#65) by the commit that adds this section. Only two conflicts appeared, both additive: `main/windowed.rs` keeps both `play_shield_loops` and `walker_fire_loops`, and `sounds.rs` keeps both the `Shield` and `WalkerFire` loop sources. `battle.rs`, `lib.rs` and `vehicle_pads.rs` merged automatically; the combined tree passes the counts above, so walkers and droidekas now share one executable.

Final pack receipt follows below.

### Distribution

Release was rebuilt from the merged tree (`d8881a86c058`, not dirty) and its existing `--write-pack` command packed the current `engine/assets_baked` and `engine/missions` into a staged executable. The archive holds 24,003 paths over 14,216 stored bodies (1,470 bodies are shared by identical content), 2,758,181,061 bytes. Every path and size matches the source trees, every entry's bytes were compared against its source file (3,626,550,572 entry bytes because shared bodies are read once per path), and the executable prefix is byte-identical to the release build: SHA256 `f5a5c4e5b24767d2550ec97d0c2890716352619f45a025b57ab5f173f11b1f0c`. The staged file was then installed as `dist/openbf.exe`; the installed file's whole-file SHA256 is `5ad6fc5d0fa6917b9780d4f725fa79baaf1889118eef27f57538658350a5c930`, identical to the staged copy, and `dist/openbf_data` profile/playtest file lengths and modification times are unchanged. The staged executable was removed. Receipt: `engine/artifacts/walker67/merged-pack/pack-receipt.json`.

The installed executable was then run headless from `dist` against its own archive (`--sim-trace engine/artifacts/walker67/merged-pack/smoke.trace 60 --map assets_baked/bf2_geo1 --seed 158`): it loaded the map and class data, fielded both factions (the droideka class included), 7 posts and 10 vehicle pads, and stepped 60 ticks. That proves the packed data mounts and the merged walker and droideka code run together from the single file.

## Seat-camera shake follow-up, 2026-10-30

The maintainer reports considerable shake while riding. The newest black box `dist/openbf_data/playtest/last.json` is BF2 GEO1 seed 164, tick 1031, build `449a1ec0deee+dirty` (built 2026-10-30 01:59:45 UTC): the human moves between AT-TE vehicle 12 seats 1 and 2 while the walker travels. It reports 75 fps and p99 15.4 ms, so frame pacing is not evidence for this shake. The snapshot does not record the animated mount's per-frame matrix, so the gait explanation follows from reproducing the camera calculation, not directly from the snapshot.

The gunner camera used the turret node's walk-animated parent as its anchor and up axis. The camera consequently took the leg animation's pitch and roll, even as its gun look was steady. The camera now takes the same authored turret mount in the bind pose, retaining the seat's aim, camera offsets, tilt and FOV. Walker drawing, the gunner, muzzle/effect frames and simulation continue to use the actual animated pose. This deliberately removes the mount's gait motion from the camera; it is a stabilization tradeoff, not a claim of exact original-game camera motion.

`main_gunner_camera_stays_steady_while_the_mount_walks` uses the actual Geonosis AT-TE rig. It changes walk-pose pitch, roll and clip time, verifies that the drawn turret actually moves while the camera remains steady, and checks that mouse aim still moves the camera. Work is isolated on a seat-camera branch, based on merged `a71b44d`; the primary checkout's uncommitted, overlapping alternative camera work is not overwritten.

Checks: actual-asset walker control tests 3 passed, walker pose tests 10 passed, rider suite 5 passed, game all-target Clippy passed, release build succeeded and `git diff --check` passed. No new code comments were added. The remaining original-game camera-motion comparison is unperformed.

Release packing completed and verified 24,003 current assets/missions, the executable prefix, and 79 walker/mission payloads. The first replacement attempt received Windows error 32; a retry succeeded and updated `dist/openbf.exe`. The verified stage was consumed, and `dist/openbf_data` profile/playtest metadata was unchanged. Receipt: `engine/artifacts/walker67/camera-shake/pack-receipt.json`.

## Video follow-up: cannon moving in the gunner view

The maintainer's 2.17 s recording `openbf-shaking-camera.mp4` shows the main cannon moving against the fixed crosshair. The newest black box is BF2 GEO1 seed 165, tick 1981, vehicle 12 seat 1, build `a71b44d04c82+dirty`, built 2026-10-30 03:21:08 UTC, confirming it is the executable from the previous fix. Mean frame time was 13.3 ms, p99 14.9 ms. A contact sheet and the snapshot are under `engine/artifacts/walker67/shake-video/`.

The previous stabilization test only asserted that the camera stayed fixed relative to the vehicle. That omitted how the animated cannon moves in the camera view. The camera now follows the posed turret node's position, while deriving its orientation from the stable mount reference and the gunner's look. The updated Geonosis regression varies walk animation and checks that the camera-to-mount offset remains fixed, the camera does not inherit mount roll, and mouse aim still changes the view. This is a visual change; it does not change simulation or the drawn turret/rider pose.

This corrects the demonstrated relative motion in the prior build. It has not yet been verified by a new human recording or a matched original-game moving-camera capture.

Checks: actual Geonosis controls 3 passed, walker nodes 10 passed, game all-target Clippy passed, release rebuilt and packed. Archive paths and source sizes matched, its executable prefix matched the release, and 79 walker/mission files matched byte-for-byte. After a transient Windows sharing error, replacement succeeded: `dist/openbf.exe` is updated and `dist/openbf_data` metadata was unchanged. Receipt: `engine/artifacts/walker67/shake-video/pack-receipt.json`. A fresh manual playtest has not yet verified the result.

## AI-driven side-to-side shaking, 2026-10-30

The maintainer reports the preceding build still shakes side-to-side. The newest snapshot is BF2 GEO1 seed 166, tick 908, build `a71b44d04c82+dirty`, built 2026-10-30 03:35:54 UTC: vehicle 12 rear gun seat 2 with AI pilot unit 9. Human mouse/stick inputs are zero while the camera alternates between yaw 3.13 and 3.14 and its x position alternates about 0.06 m every tick. Mean frame time 13.3 ms; p99 15.3 ms. Snapshot/digest: `engine/artifacts/walker67/shake-repro/`.

`walker_jitter` (removed in the sprint's code cleanse; in git history) created the actual Geonosis AT-TE, AI pilot and human rear gunner and records 600 steps. It reproduces the cause: the AI-driven vehicle's body yaw alternates by 0.008667 radians every tick, while the gunner's local look remains fixed. This body yaw feeds the camera, and the camera offset amplifies it. The earlier fixes addressed the animated mount and did not address this oscillation.

The visual interpolation layer now filters the drawn rotation of an AI-driven walker while the player rides a gunner seat, using the same filtered rotation for camera, vehicle and posed attachments. Filtering is confined to the temporary drawn pose; it is restored before simulation advances. A new 600-step Geonosis test observes the raw oscillation, checks the displayed rotation's tick changes are under 15% of it, and verifies the simulation hash is restored exactly each frame. In the reproduced last 150 ticks the camera yaw step averages 0.008667 radians before and 0.000465 after, a 94.6% reduction. Driver and on-foot views keep their existing paths. Continuous intentional AI turns are visually eased by the filter, a presentation tradeoff; mouse gunner look remains separate.

Checks: four Geonosis control tests including the AI-pilot case passed, render-blend unit tests passed, and game all-target Clippy passed. The numerical reproduction is not a new manual video or original-game comparison.

The release was rebuilt and packed with the current assets and missions. Verified 24,003 paths/sizes, the executable prefix and 79 walker/mission payloads; the replacement succeeded after a transient Windows sharing error. `dist/openbf.exe` was updated; `dist/openbf_data` metadata was unchanged. Receipt: `engine/artifacts/walker67/shake-repro/pack-receipt.json`.

The maintainer confirmed on 2026-10-30 that the installed build works. The heading-filter follow-up was reviewed and rebased onto `55fb5a3` before the requested commit; the filter logic was consolidated and one extra per-frame pose clone removed.


## Spider Walker aim and destruction follow-up, 2026-11-13

Scope authorized by the maintainer on `feature/vehicle-parity`: the Geonosis
Spider's shots below the reticle and a check of its apparent pre-explosion
animation. Starting revision `1f07b8858a2aff3d5215a43033a4a950b6c1ac99`, with the
existing vehicle-parity edits retained. No issue, ownership claim, commit, push or
PR was created by this follow-up.

### Report and cause

Read `dist/openbf_data/playtest/last.json` first: BF2 GEO1 Conquest, seed 420,
build `1f07b8858a2a+dirty`, tick 10283; human unit 0 in vehicle 6, seat 0.
The matching real-data reproduction found the Spider's lower cannon 6.186 m off
the reticle's terrain target at a settled look of yaw 0, pitch -0.12 radians.
The lower cannon and upper beam have one fire point apiece, about 6.1 m apart
vertically. `Sim::seat_gun` applied barrel-to-target convergence only to rings
with multiple fire points, so the lower gun fired parallel to the other gun's
line. One fire point now receives the same convergence calculation. Sustained
beams also retain this calculation during their per-tick fire-point update;
previously that update replaced their corrected launch direction with the
seat's shared direction. No gameplay numbers, animation durations or baked
assets change.

This extends the existing per-barrel aiming approximation described by
`ring_aim`: separate authored limits and a common camera target, without adding
per-aimer lag state. Original node writers use each weapon's target and aimer
frame (`re-turret-draw.md` 2.3 and `re-walker-nodes.md` 3). The native firing
screenshots below support the observed convergence, not an exact transient
turn-rate comparison.

### Original-game evidence and destruction finding

[O] New local BF2 PS2 observation, SLUS-21240 v2.01, disc CRC `02f4b541`,
ROM SHA1 `b385edc89d053ca8238b47c6549e78f45eb88ca4`. User save slot 10
(`2026-11-13 20:02:58` local) was read and converted to a separate compatible
copy with the `pcsx2-state` skill; the original save was untouched. Tracking run
`01M7BKC2CFCPYWFG7Y0FWESW9X`; local artifacts in
`engine/artifacts/spider-parity/` include the state, EE dumps and screenshots.

- Native firing: `ps2-slot10.png`, `ps2-fire.png` (R1 held 30 video frames),
  `ps2-beam.png` (L1 held 30 frames). Each firing trial restored slot 10.
  The upper red beam reaches the reticle in the native game.
- Spider entity `0x1ce8db0`, vtable `0x61f850`, class `0x1ae8950`.
  The class's animation pointer table at K+0x670 points to `0x1ae7c30`;
  entry 3 points to `0x1aea510` and its duration at +0x0c is zero.
  The local BF2 bake likewise has no duration-bearing `death01` clip.
- Controlled idle-death intervention: clear the health alive bit at
  E+0x228 (`0x2b` to `0x23`), then advance the native game. This is a forced
  death condition, not a natural damage/kill timing measurement. After one
  video frame E+0x21e8 remains 0; after five it is 4, E+0x2190 is `0x1a`
  (dead and both explosion latches). `ps2-death-35.png` shows flying chunks.
- This agrees with BF2 `FUN_0025ce68` (176956-177030: the immediate first
  explosion) and `FUN_0025f4a0` (179231-179307: final explosion and chunks at
  animation entry 4). Locomotion `FUN_0025d5e0` (177925-177952) lets an active
  step finish before the death transition; that behavior already exists in
  the port. No artificial collapse clip or delay is added. The BF1 bank has
  a death clip, so the zero-duration finding and regression are BF2-specific.

Focused check results and remaining acceptance follow below.


### Focused checks and limits

Runtime source was built with `cargo rustc --manifest-path engine/Cargo.toml
-p game --test suite -- -C extra-filename=-spider`. A simultaneous existing
suite process locked the ordinary Windows test executable, so this follow-up
used a separate output name. After the final fixture correction, the original
`seat_camera.rs` and `walker_gun_aim.rs` modules were also compiled into a small
local harness against those same Cargo-built libraries, avoiding another game
build-stamp rebuild. Reproduction command and exact library paths are retained
in `engine/artifacts/spider-parity/compile-suite.py`; no harness source is
published. `source-receipt.json` records the final source SHA256s and base.

- `python engine/artifacts/spider-parity/compile-suite.py`: passed for the final
  formatted source. The fixture disables random spread only for its alignment
  measurement; runtime weapon spread remains authored. Its target includes map
  and other-vehicle occlusion, rather than assuming every aim ray hits terrain.
- `engine/artifacts/spider-parity/spider-suite.exe walker_gun_aim:: --nocapture`:
  **8 passed**, including the existing AT-TE and camera regressions; 72 gunner
  seats checked. Spider: 24 shots and 111 live-beam samples across BF1/BF2
  Geonosis and three look directions meet the reticle (shot axis within 0.25 m
  of the target; live beam endpoint within 1% of screen height/width).
- Post-format rerun `.../spider-suite.exe walker_gun_aim::spider --nocapture`:
  **2 passed**. BF2 idle destruction: 2 simulation ticks; killed halfway through
  a walk clip: 16 ticks. These are port fixtures, not measured PS2 durations.
- `cargo test --manifest-path engine/Cargo.toml -p core --lib sim::beam::tests
  -- --test-threads=1`: **8 passed**. The same current core test executable's
  `sim::guns::tests` and `vehicle::walker::tests` filters: **28 + 15 passed**.
- Focused rustfmt and `git diff --check`: passed. The size-report script's exact
  embedded Python was run with the installed `python` (Git Bash's `python3`
  resolves to the Windows Store alias): map coverage passes, existing stale
  counts reported. Touched runtime files: guns 845 lines, beam 312 lines.
- `python scripts/check_provisional.py` still reports the pre-existing
  `vehicle/gun.rs` eye-offset comment without a replacement measurement; no new
  provisional value was introduced by this follow-up.

Unchecked: full alignment acceptance, GPU captures of this change, manual
playtest of a rebuilt port, and a natural lethal-hit PS2 walking-death timing
trace. No packed executable was rebuilt, and no full vehicle-parity acceptance
is claimed. All save states, captures, RAM and test binaries remain local.

Commit preparation: the two Spider regressions now live in `game/tests/suite/spider_feedback.rs` (selectors `spider_feedback::`), isolating them from the concurrently edited AT-TE test module. The maintainer confirmed the result works and authorized committing this chat's fixes. Other staged vehicle-parity and AT-TE edits remain outside this commit.
