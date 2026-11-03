---
id: UORDE
title: Emitter weapons (BF1 clone pilot's arc caster; BF2 dark trooper / Jawa arc casters, the officer's incinerator, the hero flamethrowers) fire no bolt: each shot is one instant cone query at the muzzle that chains damage from target to target (MaxTargets, JumpDeduction, DamageThreshold, MaxJumpDistance, LOS per hop); BF2 adds hit-all, team / alive filters, class scales, a blast per hit, pull and burn damage over time
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
analysis/re-ordnance-kinds.md §8 (card UORD; full reading lead_tmp/uord/helpers/H2_emitter.md): the emitterordnance class's create slot (B1 FUN_00278208, B2 FUN_00315f78) runs one query per weapon shot and returns 0 - no projectile. The port built these as straight bolts before this branch (see What landed).

## Evidence
- BF1 (B1:198922-199272): box gather at muzzle + 0.9 L x aim, mask 0x812 (no vehicles), cone tan < sin(ConeAngle/2) at the body origin with no front test, nearest along the aim first; D = lerp(Damage, MaxDamage, charge); hop range ConeLength then MaxJumpDistance (axial); LOS mask 0x4c from the chain origin to body + 1 m; FirstRadius on the first; all type / shield scales forced to 1.0; D -= JumpDeduction; stop at MaxTargets or D < DamageThreshold. Verifier batch 4: 8 / 8 SUPPORTED.
- BF2 (B2:305952-307007): mask 0x102a (vehicles), 3-D cone (cos 0.1 within 2.5 m), alive bit, AffectFriends / AffectEnemies, the class's scales, hit-all mode (D = max(Damage, D/N)), Explosion blast per hit, pull (Push < 0), burn timer = max(timer, T), rate x dt per update while alive (FUN_00236f50).
- Fielded: BF1 rep_weap_inf_arccaster (1 / 2 / 3 targets by charge: 100 -> 250, JD 75, DT 75); BF2 imp / tat arc casters, all_weap_inf_incinerator (30 per shot, 15-shot burst), cis / imp_weap_hero_flamethrower (5 per shot + burn 50/s for 5 s).

## Acceptance
- Unit tests on the read rules, mutation-checked: the BF1 arc caster's target count by charge (1 below 1/3, 2 below 5/6, 3 above), a skipped (LOS-blocked) candidate ends nothing, no vehicle in BF1's gather; BF2 hit-all split, the burn's damage over 5 s, team filter.
- A scripted scene: a clone pilot's full charge into three soldiers in a line deals 250 / 175 / 100.
- 8 x 32 sweep 256 / 256 (bots hold these weapons: the clone pilot, the dark trooper, the Rebel officer).

## Notes
- Visual only (no sim): the lightning segments, BF2's extra arcs, the flame effect, the burn bone effects.
- Named-deviation candidates (H2 §6): BF1's missing front test, its feet-point cone test, its dead scale keys, corpses as targets - measure on the PS2 before keeping or dropping any.

## What landed 2026-10-30 (branch `uord-missiles`, 70da755 through 1b1e72b, pull request #84)
An `emitterordnance` class is now one instant query per shot, not a bolt. Line numbers are the working tree's.

- **The class** (`engine/crates/core/src/weapon/emitter.rs:27-95`): `EmitterOrdnance` reads `ConeLength`,
  `ConeAngle` (degrees -> radians), `MaxTargets`, `JumpDeduction`, `DamageThreshold`, `MaxJumpDistance`,
  `FirstRadius`, `Radius`, and the hashed `Push`, `LightningEffect`, `AffectFriends`,
  `AffectEnemies` and `0x4c387ca1` (hit-all). `bf2` selects the rule set; `is_cone()` is `ConeLength > 0`.
  Loaded on a `WeaponClass` when the ordnance's engine base is `emitterordnance`
  (`engine/crates/game/src/roster.rs:897-902`), and its `Damage` becomes the hit-all floor
  (`roster.rs:760-766`).
- **The law** (`engine/crates/core/src/emitter.rs:70-239`): BF1's gather (soldiers and riders only, no vehicles,
  32 cap), the cone at the body point, nearest along the aim first, `FirstRadius` on the first link, LOS per
  hop, `D -= JumpDeduction` with stops at `MaxTargets` and `DamageThreshold`; BF2 adds vehicles, the team filter
  (`AffectFriends` / `AffectEnemies`), the class's damage scales, and hit-all (`max(Damage, D/N)`, nearest
  first). A skipped (LOS-blocked) candidate does not end the chain.
- **The sim** (`engine/crates/core/src/sim/emitter.rs:24-152`): one `Event::Shot` (the fire sound, muzzle flash
  and trigger animation read it), the gather of alive soldiers and hulls at body + 1 m, the query, BF1's forced
  1.0 scales, per-hit damage with the friendly-fire scale, BF2's `Explosion` per body hit, `Push` through
  `kick_and_push`, and one `Event::Emitter` link per hit (`sim/state.rs:188-199`; headless skips it,
  `main/headless.rs:277`).
- **The draw** (`engine/crates/game/src/fx.rs:1244-1268`): each `Event::Emitter` link whose class names a
  `LightningEffect` becomes an `fx::arc::Arc` (`engine/crates/game/src/fx/arc.rs:117-176`), seeded from the sim
  tick and the link's index, alive for the record's 18 ticks and drawn as the PS2 ribbon (`arc.rs:1-28`, `186`);
  a class that names none (the flamethrowers) starts no arc.
- **The dispatch** (`engine/crates/core/src/sim/ordnance.rs:192-203`): a cone emitter with `max_damage > 0`
  spends the shot in `fire_emitter` and spawns no bolt; a zero-damage cone (Force push / pull) stays on the old
  bolt-and-blast path.
- **Tests, run 2026-10-30** from `engine/`: `cargo test -p core --lib emitter` -> 22 passed, 0 failed (10 law
  tests in `emitter.rs`, 9 scene tests in `sim/emitter.rs`, 3 loader tests in `weapon/emitter.rs`; 1,384
  filtered). The card's own scene is asserted twice, law and sim: a clone pilot's full charge deals 250 / 175 /
  100 down a line (`emitter.rs:355`, `sim/emitter.rs:292`); 1 / 2 / 3 targets by charge
  (`emitter.rs:329`, `sim/emitter.rs:276`); a wall skips one link (`sim/emitter.rs:312`); BF1 zaps an ally
  (`sim/emitter.rs:336`); BF2 hit-all splits 15 into 7.5 / 7.5 and never goes under `Damage` when split
  (`emitter.rs:392`, `:401`); the team filter both ways (`emitter.rs:409`, `:419`; `sim/emitter.rs:371`); a
  vehicle is a candidate on BF2 only (`emitter.rs:428`, `sim/emitter.rs:395`); the zero-damage class stays a
  bolt (`sim/emitter.rs:431`).
- **The arc draw test, run 2026-10-30** from `engine/`: `cargo test -p game --lib
  emitter_links_draw_their_lightning` -> 1 passed, 0 failed (576 filtered): two links become two arcs, a class
  with no `LightningEffect` starts none, and the arc is gone at tick 23 after its 18 (`fx.rs:2042-2079`).

## Burn, the shot effect and the burn bone effects (2026-10-30, traced in the BF2 decomp)
The keys were in `analysis/SWBFRNT2.ELF-decomp.c` all along: the emitter class setter `FUN_003156e0`
(B2:293400-293680) parses them, the create slot `FUN_00315f78` uses them, the soldier keeps the burn.
- **Class keys** (names untraced, hashes and slots read from the setter; ctor `FUN_003153a8` zeroes the floats):
  `0x6824f471` burn time -> class+0x130; `0x1076dfd6` burn rate -> class+0x160; `0x6cf895fb` a bone and
  `0x592fb160` an effect, each appended to its own 5-slot array (+0x138 / +0x14c); `0xc0e20834` an effect
  name -> class+0x12c; `0x348506dc` a bool -> class+0x134 bit 0 (copied to the soldier's +0x1060 bit 0; no
  reader found, not ported). Disc values: hero flamethrowers 5.0 s, 50.0/s, fire on ribcage / head / pelvis,
  effect `com_sfx_weap_flamethrower_ord`; incinerator 1.5 s, no rate (0), disintegrate on both forearms,
  effect `com_sfx_weap_incinerate_ord`.
- **The hit** (`FUN_00315f78` B2:294190-294207, the same block at each of its three hit sites): when the hit's
  damage > 0, the body is a soldier (vtable type `DAT_00689828`) and burn time > 0, the five (bone, effect)
  slots are written onto the soldier (`FUN_00236f38`) and `FUN_00236e28` sets timer = max(timer, T), stops
  the running burn effects, OVERWRITES the rate and copies the hit's damage desc (the attacker).
- **The update** (`FUN_00236f50`, from the soldier updates `FUN_00240bf8` / `FUN_0024de60`): while alive,
  timer -= dt (floor 0); while > 0 and rate != 0, rate x dt with all nine scales 1.0 through the stored desc;
  each slot's effect runs at its bone (`FUN_00237170`); at 0 the effects stop.
- **The port**: `EmitterOrdnance::{effect, burn}` (`core/src/weapon/emitter.rs`, `BurnClass`), loaded with
  repeated keys through `PropSource::prop_hash_all` (`ClassDb::props_all_hash`); `Unit::burn`
  (`core/src/sim/burn.rs`, hashed only while it runs) set by `Sim::ignite` from `fire_emitter` before the
  hit's damage (BF2 rules, not a droideka), ticked by `Sim::tick_burns` after `tick_buffs` (step 5c), the
  kill credited to the shooter, no `Event::Hit` per tick (the poison precedent). The draw: `fx/flame.rs`
  starts the class's effect at each `Event::Shot`'s muzzle facing the shot, and keeps one effect per burn
  slot on the drawn bone (`SoldierVisuals::burn_frames`; body + 1 m when undrawn), stopped when the burn
  ends, the soldier dies or the class changes.
- **Deviation, named**: the PS2 restarts the burn effects at every hit; the port restarts them only when the
  burning class changes (a flamethrower's stream would otherwise restart them ten times a second).
- **Tests, run 2026-10-30** from `engine/`: `cargo test -p core --lib burn` -> 8 passed (50/s for 5 s after a
  15 hit, max timer and overwritten rate, the incinerator's no-damage mark, BF1 / no-burn classes set none,
  the burn kill credited with no per-tick `Hit`, the hash only while it runs, the loader for both disc
  classes); `cargo test -p game --lib flame` -> 2 passed (the shot effect along the shot; one bone effect
  per slot that follows, stops at the end and at death).

## Miss lightning (2026-10-30)
- An emitter whose query struck nothing still draws its lightning: BF1 one bolt (B1:199011), BF2 NumBolts
  (0x63384b21, ctor 1) less the hits, from the muzzle to the aim ray's end within ConeLength, stopped by terrain and
  the world (`sim/emitter.rs`, `perception::reach`). The dark trooper's arc caster is seen on every shot.

## What is deferred (not built, with the reason)
- **Deflection, per-hit buffs, Force push / pull**: §8 names each in a clause - "deflection by the target's
  weapon", "pull (Push < 0)", "buffs" - with no key, unit or class table, and the impact section's deflector
  vcall (+0xd0 / +0xdc, §3.2) is the impact handler's, not the emitter's. No code models them: `kick_and_push`
  returns on `push <= 0.0` (`sim/blast.rs:311`), so a negative `Push` (a pull) does nothing; the zero-damage
  (Force push / pull) emitter is kept on the bolt path (`sim/ordnance.rs:192-199`).

## Evidence (2026-10-30)
- Source: the files and lines above; the test runs quoted above (`22 passed, 0 failed` in core; `1 passed` in
  game for the arc draw).
- The PS2 side the card cites is **ABSENT**: `Test-Path` false for
  `lead_tmp/uord/helpers/H2_emitter.md`, and there is no `lead_tmp` at the repo root
  (`engine/artifacts/lead_tmp/` holds ten other directories and no `uord`). The read rules are therefore
  checked against the doc's text (`analysis/re-ordnance-kinds.md` §8) only.
- Mutation check (the card's "mutation-checked" clause): NOT RUN. `lead_tmp/audit-uord/mut/M1` is false.
- 8 x 32 sweep 256 / 256: NOT RUN.
- Status stays `blocked`: the sim build, burn and the flame draw are in, but deflection / buffs / pull and
  the two acceptance runs (mutation, sweep) are not.
