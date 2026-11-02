---
id: HCON
title: Hover contacts and two missed drive terms (HOVR audit): the terrain contact bounded to a real sphere-surface hit with the cut test at the sphere (POL1's hangar AAT is catapulted where the PS2 holds it still); contacts re-queried after each push-out as FUN_00160da8 does (the TAT1 skiff pops 5.2 m at spawn); re-hover §6.19 g = 0 for SetAltitude <= 0; §6.10's BoostSpeed turn scale (live on every BF2 hover)
milestone: M2
value: 5
phase: build
status: done
deps:
files: core/src/sim/vehicles.rs,core/src/vehicle/hover.rs,core/src/vehicle/hover/tests.rs,game/src/vehicle_class.rs,CRATES.md
declares: sweep
needs:
branch: hcon
resume: landed f2fdcda7 2026-09-28
created: 2026-09-28
---
## Why
The HOVR audit (2026-09-28, PARTLY) found the transcription of BF2's integrator faithful, but four gaps that change what a player sees:
1. **Terrain contact.** WorldContacts (sim/vehicles.rs) treats the heightfield as an unbounded half-space: depth = r - (c.y - h)*n.y for ANY sphere below the surface, and it tests is_cut at (c.x, h, c.z), the surface point, not at the sphere. At POL1 hangerdockcon the hangar floor is at y 10.2 under the asteroid's terrain (h 18.6-21.2). The rear spring sphere gets depth 8-9 m, and K5 pushes the vehicle out by depth - len. The AAT is flung 8.4 m up and 5.6 m sideways; the combat speeder is flung 12.5 m up and 28 m sideways. PS2 RAM holds the AAT still at (97.99, 10.79, -102.0) in bf2_pol1_sideselect/spawnmap.
2. **Contact resolution.** contacts() finds every contact at pos0 and sums the push-outs. The PS2 re-queries: FUN_00160da8 (B2:711-800) feeds each handled response's displacement to FUN_0016beb8, which moves the query origin DAT_0066abf0 before FUN_0016bf10 finds the next contact. The TAT1 skiff spawns on the ground probe with its three spheres bottomed: the port sums 3 x 1.7 m and pops 5.2 m; sequential resolution gives about 1.7 m.
3. **re-hover §6.19** (B2:134798-134800, ELF 0x21f44c-0x21f464): the drive ends with g = 0 when SetAltitude <= 0. The port lets g fade by dt per tick, so the airborne hailfire and snail tank keep traction and 1x gravity for about 1 s. RAM: the one airborne SetAltitude-0 hover (GEO1 hailfire) holds g = 0.0000.
4. **re-hover §6.10's BoostSpeed turn scale** (B2:134384-134397): when BoostSpeed > 0 and |v| > ForwardSpeed, the target is multiplied by 1 - clamp((|v| - FS)/(BS - FS), 0, 1)*(1 - K+0xf78) (ctor 0.5). This applies whether or not the vehicle is boosting, e.g. full throttle plus strafe. BoostSpeed is 17-40 on all 138 live BF2 hovers in RAM. HOVR left it out as 'boost'.
## Evidence
reports/20260928_1100_audit_HOVR.md; lead_tmp/audit-hovr/: pad_census.txt (216 pad vehicles, 5 flagged), ram_vs_port.txt, hovers.jsonl (ramscan.py: 138 hovers / 30 BF2 states), verify/VERDICTS.md (Opus verifier: 20 SUPPORTED, 4 PARTLY), examples/*.rs (scratch examples: audit_pad_census, audit_hover_ram, audit_hover_probe; copy them into game/examples to rerun).
## Notes
- The PROVISIONAL terrain model may stay a plane, but a sphere must actually meet the surface: count it only when |signed distance| < r, and never when the centre is more than r below. Sample the cut where the soldier's ground probe does.
- Whether the contact pass runs before the drive or between the drive and the springs is still [H]. RAM: E+0x1cf0 = 0 on 509/509 springs at every snapshot, so the contacts that feed a spring tick run earlier in the same frame. A fast-hover pose test leans 'drive -> contacts -> springs' (34/34 vs 27/34 in the jitter range) but is confounded. The port's drive -> springs -> contacts is equivalent to 'contacts -> drive -> springs' as a sequence, and one tick late against the other order.
- FUN_00169668 (B2:7937-8054) pushes out fully only for other-class 0x10/0x80 (a mass split for 4/8, none otherwise). This matters when HOVX adds the hull; the terrain's class is [D] > 8.
**Planner (2026-09-28):** value 4 → 5 and made a dep of VSEAT and HOVX. VDRAW now draws the pad vehicles, so the TAT1
skiff's 5.2 m spawn pop and POL1's catapulted hangar AAT are visible in every battle, and VSEAT drives exactly that skiff.
HOVX edits the same hover.rs. Reverse: `set HCON value=4`, drop HCON from VSEAT/HOVX deps.
