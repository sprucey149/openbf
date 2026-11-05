# Spawn ammo and reload investigation, 2026-10-26

Status: UNAUDITED. Direct source and ELF checks below; no new live emulator measurement claimed.

## Report and snapshot

The maintainer compared BF1 Bespin Platforms in PCSX2 with the port: the Imperial blaster rifle starts at
55 loaded / 220 reserve in BF1, whereas the port shows 330. Shooting and reloading did not make the supply
feel scarce. The single total follows the locked BF2 HUD decision; the additional clip is a gameplay bug.

Read `dist/openbf_data/playtest/last.json` with `blackbox_read` first: BF1 BES1, seed 63, human unit 0,
build `001e3aa0e648+dirty`, compiled 2026-10-26 11:49:13 UTC. Then read `prev1.json`: same map, seed 62,
build `69c77d58f235+dirty`, compiled 2026-10-26 03:56:08 UTC. Both builds predate checkout `e5ca9e3`.
These observations describe those edited builds, not a new executable. The digest is saved locally at
`engine/artifacts/lead_tmp/ammo-prev1-digest.log`.

`prev1.json` human row tick 90 has `imp_weap_inf_rifle`, clip 55, spare 275: total 330. Tick 540 has
39 + 275 = 314; tick 1440 has 32 + 275 = 307. Thus firing does spend rounds. In `last.json`, tick 3690
has 55 + 254 = 309 and tick 4140 has 22 + 254 = 276. A reload conserves total ammunition; it should
not subtract another clip from the combined HUD number.

## Original rule [V]

`WeaponAmmo` counts total clips, including the loaded one. BF1 `FUN_002838d8` constructs one loaded
clip and N-1 spare clips. BF2 `FUN_00303670` initializes N clips in its ammo object, then calls
`FUN_00303958` to load one from the pool. A slot value below one means an endless reserve. This
agrees with `re-grenades.md` section 2.5 and applies to the primary weapon too.

BF1 disc class `imp_inf_storm_trooper`: `WeaponAmmo1 = 5`; its `imp_weap_inf_rifle` has
`RoundsPerClip = 55`. Correct issue: 55 loaded + 220 spare = 275 total. The live emulator numbers
reported by the maintainer agree. The new integration regression reads the original IMP and COMMON
LVLs through the real class baker and roster registration path, rather than substituting BF2 stats.

BF1 reload entry `FUN_00285398` returns the partial loaded clip to the finite reserve and clears the
clip; exit `FUN_002854c0` loads up to one clip. BF2 uses `FUN_003274f8` / `FUN_00327658` with ammo
helpers `FUN_00303928` / `FUN_00303958` for the same finite-round accounting. Neither discards the
remaining rounds in a partial magazine.

BF1 update `FUN_00284318` checks the selected idle weapon's empty predicate independently of fire
input. After firing, `FUN_002891c0` requires the refire period to finish and treats an empty clip as
a reason to leave the firing state; the base update then enters reload, or out-of-ammo when both
clip and reserve are empty. BF2 `FUN_00326170` has the corresponding idle/fire/reload transitions.
The rifle's firing delay must finish before its reload delay starts. The old port instead started
reload early when the next held/release trigger input saw an empty clip. An idle empty weapon with
no trigger input could stay empty. This is more precise than the initial suspicion that every empty
gun needed another trigger press.

## Port cause and correction

- `UnitSpec::build` used the spare-clip constructor for the primary, while held slots already used
  the total-clip constructor. Use the latter for both, including respawn.
- Ammo pickup/station caps also treated the primary's total clip count as spare clips. Use the
  BF1 `max clips - (1 if loaded else 0)` rule for primary and held slots on all maps. This gives N-1 spare
  clips with a loaded gun and permits the full N when empty; an exhausted one-clip weapon can be
  resupplied too. BF2 `FUN_003038c8` subtracts a clip only when the loaded magazine is full;
  using BF1's cap with a partial magazine is the named AMCAP merged-game deviation. It avoids
  gaining an extra reserve clip merely by firing one round before resupply. See `re-powerups.md`
  section 5.4 and BF1 `FUN_002ab4d8`.
- Primary HUD totals bypassed the helper that recognizes an endless reserve. Use the same helper
  as the secondary channel; this also avoids adding the infinite sentinel to a loaded clip.
- Empty reload now waits for the firing cycle to finish, then starts without trigger input on a
  selected weapon. Refire and cycle-finished gates recognize an empty finite clip.
- Partial finite reloads return loaded rounds to reserve at entry and refill at completion, keeping
  the total constant throughout.

The integer `WeaponState::new` constructor still explicitly accepts spare clips for low-level fixtures
and vehicle guns. Soldier class/spec fields use `total_clips` for the authored total clip count,
including the loaded magazine. The gun's magazine capacity and `ReloadTime`
remain from its own disc data.

## Checks and limits

Re-runnable evidence checks:

```text
python scripts/re_check.py fn b1 207670 208134 208822 208862 211342
python scripts/re_check.py fn b2 282195 282344 282357 305785 306637 306685 306703
python scripts/re_check.py callers b1 FUN_002838d8
python scripts/re_check.py callers b2 FUN_00303670
python scripts/re_check.py word b1 0x53b2c4 14 --u32
python scripts/re_check.py word b2 0x6283d8 22 --u32
```

The BF1 cannon vtable maps reload entry/update/exit to 0x285398 / 0x285498 / 0x2854c0,
empty-clip predicate to 0x285608 and fire-finished predicate to 0x2891c0. The BF2 cannon table maps
reload entry/update/exit to 0x3274f8 / 0x327618 / 0x327658. Constructor callers include BF1 cannon
`FUN_00288858` and BF2 base weapon `FUN_00325448`. No mutable global or original constructor default
is introduced by this fix; constants are authored clip counts, the existing infinite sentinel and
existing weapon timing data. The older `re-blaster-damage.md` section 8 description of `WeaponAmmo 6`
as spare clips is superseded by the constructor evidence here and `re-grenades.md` section 2.5.

Four new regressions failed before the implementation and passed in the first corrected core run:
primary/held issue equivalence, selected empty reload without input, single-shot firing delay before
reload, and conservation through a partial reload. Additional checks exhaust a 55-round/five-clip
rifle and the actual BF1 disc-built Imperial rifle at exactly 275 shots. Existing pickup/station checks
cover refill intervals and capacity. The simulated state hash must change because every finite primary
starts with one less spare clip; the deterministic pins are updated explicitly.

Scope limits: this does not establish frame-perfect animation/sound synchronization, charged-weapon
interrupt rules or inactive-weapon reload parity. In particular the BF2 native reload update advances
its clock only when selected, while the BF1 base update advances it unconditionally; the port still
uses its shared reload clock. No full round sweep or new live PCSX2 reload timing measurement is claimed.

## Completion

Workspace formatting, Clippy with warnings denied, build, unit/integration/arena tests and doc tests
completed successfully (`ammo-ci.log`; 214 game suite cases passed and one existing case was ignored).
After the final empty-reserve-cap correction, reran workspace Clippy, all 985 core tests, all 398 game
library tests (two existing ignored), four game binary tests and the three disc weapon-class integration
tests. They pass. A concurrent attempt to relink the integration executable initially hit Windows
LNK1104 while the earlier suite was running; the retry after that suite exited passed. Logs:
`ammo-final-checks.log`, `ammo-game-final-tests.log` in `engine/artifacts/lead_tmp/`.

Built release and repacked 23,884 entries with `scripts/pack.py`, verified all index ranges/footer,
the release executable prefix byte-for-byte and the embedded BF1 Imperial/common classes and BES1
mission against the local inputs, then installed `dist/openbf.exe` (2,732,288,570 bytes). Release prefix
SHA256: `e27e2b000982bbd3aab00c7ed331d45300b7f52b5286dd4b497ce324c19bffb2`.
The installed executable's 100/600-tick smoke runs return `06da5e043dd4c755` /
`989d7c74c627273a`, matching the updated deterministic pins. `ammo-pack.log` and
`ammo-pack-verification.log` record the packaging checks. No temporary packed replacement remains.
`git diff --check` and the added-Rust-comment check pass. Changes are kept local; no public export or push was performed.

## Seat guns, 2026-11-05

Report: the maintainer could fire the TIE Bomber's bomb without limit (`last.json`, BF2 spa8, seed 256:
28 bombs 9 to 13 ticks apart, no pause).

**Original rule [V].** The disc's `imp_weap_fly_tiebomber_bomb` has `RoundsPerClip` 6, `ReloadTime` 5, `ShotDelay` .33;
the ship's slot has `WeaponAmmo` 0. The flyer setter `FUN_00263f68` (BF2 line 183088) records a value under 1 as a flag, and
the flyer and turret object constructors (lines 174922 and 187341, `FUN_0026bf88`) turn the flag into 0x7fffffff and pass it to
the weapon. Every other seat class has the same step. The base weapon `FUN_00325448` hands that count to `FUN_00303670`, the
ammo object soldiers use: it marks an endless reserve and loads one clip. Reload entry `FUN_003274f8` and exit `FUN_00327658`
take no account of the owner, and `FUN_00303958` fills the clip from the endless reserve. So `WeaponAmmo` 0 means reserve clips
never run out, not that the clip never empties. A slot with `WeaponAmmo` N holds N clips, one of them loaded.

**Port cause.** `guns.rs` passed `no_reload` and `infinite_ammo` for an endless slot, which are the two cheats together, so
shots cost nothing. `vehicle_pads.rs` also issued N spare clips where the disc has N - 1. No reload sound could start either.
All three are corrected, for every seat gun on both games.

| Ship weapon (disc) | Clip | Reload s | Refire s | Salvo |
|---|---:|---:|---:|---:|
| TIE Bomber bomb | 6 | 5 | .33 | 1 |
| TIE Fighter torpedo | 6 | 5 | 2 | 2 |
| TIE Interceptor missile | 10 | 10 | .08 | 1 |
| X-wing torpedo | 6 | 4 | 2 | 2 |
| Y-wing bomb | 6 | 5 | .66 | 2 |
| A-wing missile | 8 | 10 | 0 | 8 |
| Droid fighter torpedo | 6 | 4 | 2 | 2 |
| Tri-fighter missile | 9 | 9 | .3 | 3 |
| Grievous fighter bomb | 6 | 6 | .15 | 1 |
| ARC-170 bomb | 6 | 4 | 2 | 2 |
| V-wing torpedo | 6 | 5 | .33 | 1 |
| V-wing cannon | 10 | 3 | .6 | 2 |
| Jedi fighter missile | 10 | 5 | .1 | 2 |
| Anakin's fighter missile | 10 | 10 | .2 | 2 |
| Republic and Alliance gunship missile (`WeaponAmmo` 6: 36 rounds, then dry) | 6 | 4 | 1.25 | 2 |

The other fighter cannons have `RoundsPerClip` 0: no clip at all, limited by heat. That includes the TIE Bomber's primary,
which the disc names `ptorpedo` but builds on the recoilless base (`HeatPerShot` 0, .5 s refire, .2 s initial delay).

**Heat was already right.** Human shots in the black boxes against the disc's values: TIE Interceptor cannon runs of 39 shots
then a 49-tick gap (`HeatPerShot` .05, `ShotDelay` .1, recover .25, clear at .6 gives 40 and 48); X-wing cannon runs of 19
then 23 (.1, .163, .3, .8 gives 19.6 and 20). Refire gaps also match: bomb 10 ticks, torpedo salvo 9 then 61.

**Channels.** The flyer class constructor defaults each slot's channel to its section index (line 181697) and no ship authors
`WeaponChannel`, so section 1 and 2 are channels 0 and 1. The TIE Bomber's third weapon, the guided rocket, is on its TURRET1 seat.
The shared ammo and shared heat props (0x8585bb8a, 0x9455a446, lines 182799 and 182846) appear in no baked side class file, BF1 or BF2,
and are not ported.

**Limits.** `FUN_00327618` advances the reload clock only while the weapon's active flag is set; a port seat gun always counts
as active. Which channel's weapon the original deactivates, if any, was not traced. No PCSX2 run confirms the timing. The reload
bar for a ship weapon uses the same clip fraction as on foot, so it empties and refills instead of filling smoothly.
