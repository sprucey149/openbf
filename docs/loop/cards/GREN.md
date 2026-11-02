---
id: GREN
title: Weapon slots and the thrown grenade, built: every class slot with its ammo and channel (BF2's section model), the two channels with one active, the secondary trigger + the two switch inputs, the grenade weapon's cycle (press, 0.30 / 0.51 s wind-up, 1.7 / 1.81 s) and the `sticky` ordnance (gravity, the 0.37 / 1.15 x 1.37 bounce, Stick* bits, rest, the fuse that starts at rest) exploding through Sim::explode
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/classdb.rs,engine/crates/game/src/roster.rs,engine/crates/core/src/weapon.rs,engine/crates/core/src/unit.rs,engine/crates/core/src/projectile.rs,engine/crates/core/src/sim/input.rs,engine/crates/core/src/sim/combat.rs,engine/crates/core/src/sim/step.rs,engine/crates/game/src/main/battle.rs,engine/CRATES.md,engine/artifacts/loop/FEEL_NOTES.md
declares: sim; sweep; arena
needs:
effort:
branch: gren
resume: landed d4886ba0 2026-10-02
created: 2026-09-28
alpha: yes
---

## Why
`analysis/re-grenades.md` (`[O]`, commit f0da5ad; verifier 36/6/0 of 42; two live PCSX2 runs). On the PS2 every soldier
holds all its class slots, fires the secondary channel with its own trigger and throws a grenade that bounces, comes to
rest and only then counts its fuse. The port holds one primary (`roster.rs` 309-323) and has no ordnance but the bolt.

## Notes
- Original card (2026-09-28, planner): "the maintainer (INBOX 2026-09-28 (d)): grenades, mines and detpacks have no card. The
  roster models one primary per class (the BF2 SBD lists four weapons); the AI has `Skill::grenade_chance` RESERVED with
  no consumer." Acceptance then: the RE ([O]) of the slots, the thrown / placed ordnance and the AI's grenade use, and
  build cards — done 2026-09-30 (this card -> build; GRAI; MINE).
- UAUD1 (2026-09-29, analysis/unit-audit-bf1.md §3.1, §3.4): every BF1 soldier has 2-4 slots and the PS2 instantiates all of them in slot
  order (RAM of 22 save states, `lead_tmp/unit-audit/ramcensus_bf1.txt`); the class-select screens list them as each
  class's equipment (`lead_tmp/unit-audit/pcsx2/*_sheet.png`). ch0 = primary + pistol (the SBD: blaster + Tri Shot; the
  Wookiee: bowcaster + grenade launcher); ch1 (WeaponChannel 1, slots 3-4) = thermal detonator + crack / haywire
  grenade (troopers), + mine dispenser (vanguard / shock / ARC / assault), + remote (recon) droid (snipers), fusion
  cutter + powerup dispenser (pilots), wrist rocket (SBD), remote charge = time bomb (Wookiee). The table per class is
  in analysis/unit-audit-bf1.md §3. Non-bolt PRIMARIES are split out to UORD.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §1.1, §3.1, §3.4): BF2 builds a class as a COPY of its parent (B2 `FUN_00250dc8`: slots
  +0x7f8, ammo +0x818, channels +0x838, count +0x840) and then applies the child's PROPs in order: `WEAPONSECTION n`
  selects slot n-1 (reset to 0 per class), `WeaponName`/`Weapon` fill it, `WeaponName1..4` / `WeaponAmmo1..4` /
  `WeaponChannel1..4` are fixed slots 0..3 (setter `FUN_00252bb0`). The port's `props_all` (whole list of one key,
  child-most definer) gives the same lists on today's data except the 18 classes written with numbered keys (16 heroes,
  `wok_inf_rocketeer`, `tat_inf_jawa`), where it sees slot 1 only. Its doc comment at classdb.rs 152-155 (at 781acbd) misreads
  `rep_inf_ep2_jettrooper_rifleman` (it authors WeaponName1..4). BF2 slots per class and the class-select lists:
  analysis/unit-audit-bf2.md §3, §3.1. Award weapons: re-grenades §2.4 has the gate (the medal bit); who sets it is PLAN M4.
- GREN RE (2026-09-30, lane 3, report `reports/20260930_2135_card_GREN.md`): the live probes are re-runnable —
  `lead_tmp/gren/probe1.py <session>` (BF1) / `probe2.py` (BF2) on a SW `pcsx2_session.py` session (no gate lock), then
  `analyze1.py <json>`; states `ref/states/bf1_nab1_walk_cp1_cp2_b000.p2s` (spawn map: K, K) and
  `bf2_hot1_walk_cp1_cp6_b000.p2s`.

## Built (2026-10-02, lane 2, branch `gren`; report `reports/20261002_1600_card_GREN.md`)
Acceptance, run:
- **Replay test** `cargo test -p core sticky_ps2` — 5 of 5: launch (both PS2 rows within 2e-3 m/s; the aims are the
  RAM rows to 4 decimals), bounce ((-1.493, 4.644, 4.484) within 0.02), BF1 four floor bounces -> rest -> blast 0.9 s
  after rest, BF2 fast bounces uncounted / 4 slow -> rest -> 0.6 s, the stick mask (BF2 thermal sticks to a hull, BF1
  thermal bounces, BF1 crack sticks). Mutation-checked: Rebound 0.3 for 0.37, no 1.37, the fuse from the throw, the
  rest rules swapped — each fails a test.
- **Cycle test** `cargo test -p core ordnance` — 5 of 5: thrown at the first update at or past 0.30 s (BF1) / 0.51 s
  (BF2), second throw not before ShotDelay after the first, count 4 -> 3, a held trigger throws once; channel 1
  swallows the primary; the two switches; a throw to its blast through the sim (kill credited, the bots' warning).
- **Census** `cargo test -p game --test roster weapon_slots` — every census class in the four-side dbs (BF1 48 of 64, BF2 133 of 164;
  the rest are local sides) and every class a mission RON fields (BF1 35, BF2 50) resolves the disc's slots.
- **Class numbers** `cargo test -p game --test roster thrown` — the thermal out of the bake = the RAM rows: masks
  0x7fffff80 / 0x7fffffd8, gravity -12.25, 25 m/s, ShotDelay 1.4 / 1.3, wind-up 0.3 / 0.51, fuse 0.9 / 0.6,
  ShotElevate 0.21, explosion 320 / 450 at 4.5 / 6.5.
- **End to end** (headless game, a scripted pad, `lead_tmp/gren/e2e/`): NAB1 throw 9 ticks after the press, 4 contacts,
  blast 27 ticks (0.9 s) after the rest; R1 -> the crack grenade, blast 30 ticks (1.0 s) after rest; FIRE held with
  the throw: the rifle silent for 51 ticks (1.7 s). HOT1: 15 ticks, blast 18 ticks (0.6 s) after rest, rifle silent 55.
- HUD: `lead_tmp/gren/hud/pair.png` (the grenade row, "4" + the detonator mesh), `lead_tmp/gren/e2e/nab1_frames.png`.
Left out, each PROVISIONAL in the code:
- The PRIMARY keeps its old reserve (N spare clips; the PS2: N - 1, and 0 = endless). Held slots use the PS2's rule.
  Changing the primary moves the sweep -> its own small card (declares `sim; sweep`).
- A channel-0 grenade launcher (mortar) stays a bolt weapon (UORD); award slots are not held; BF2's hand offset is the
  one RAM constant; a map's static bodies never hold a grenade (which kind a plain prop carries was not read); the
  sticky's direct contact damage (no disc sticky authors a Damage) is not dealt; the sweep uses the bolt's subset.
- Drawn: a small orange glow, not the ordnance model; the gun is not hidden during the throw; the drawn gun does not
  follow a channel-0 switch (the HUD row does). BF1's grenades have no HUD mesh (the count shows alone).
- Gates (2026-10-02 16:05): `sweep` and `arena` are declared with `sim` because their rows carry each round's sim
  hash, and the held weapons are hashed. With the hash column stripped, all 32 sweep rows and the 4 arena_tat2 rows
  are byte-identical to the baseline (`lead_tmp/loop_baselines/20261002_1520_CONT`): no outcome, tick, ticket or
  stuck number moved. arena_flat (built-in troopers, no class slots) and `--sim-hash` did not move at all.
