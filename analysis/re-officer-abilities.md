# BF2 officer abilities

2026-10-28, issue #47. Status: [O] UNAUDITED. Local class data and original PS2 decompilation comparison; tests and live PCSX2 comparison have not been run. The maintainer's Astra instruction leaves output validation to the maintainer. Hero copies and Force powers remain deferred.

## Disc definitions and port comparison

The existing `engine/assets_baked/classes/bf2_{all,cis,imp,rep}.ron` preserve the weapon and ordnance BASE chains. `com_weap_inf_buff` is a cannon, not a grenade; its ordnance inherits `com_weap_inf_buff_ord`, an emitterordnance with Radius 10, ConeLength 0, ConeAngle 120 and MaxTargets 10. The zero cone length selects the separate radius path.

| Ability | Officer | Original authored effect | Port |
|---|---|---|---|
| Rage | Imperial officer and variants | BuffOffenseTimer 15, BuffOffenseMult 1.5 | Recipient's outgoing damage is multiplied by 1.5 |
| Rally | Clone commander and default variant | BuffDefenseTimer 20, BuffDefenseMult 0.5 | Active incoming multiplier halves bolt and blast damage |
| Regeneration | Bothan spy, jungle, snow and default variant | BuffHealthTimer 30, BuffHealthRate 20 | Health increases by rate times fixed dt, capped at max |
| Neuro poison | MagnaGuard, hunt and default variant | DebuffDamageTimer 15, DebuffDamageRate 30 | Timed damage retains the attacker handle and normal kill credit |
| Invulnerability | Leia, deferred | BuffDefenseTimer 10, BuffDefenseMult 0.05; BuffHealthTimer 10, BuffHealthRate 20 | Definition parses, with no hero slot enabled; this is 95% reduction plus healing |

ALL and CIS hero damage copies author 12 seconds rather than the Imperial officer's 15. Those copies are not inserted into hero loadouts. Four officer weapon definitions cover fourteen infantry class entries in the existing weapon-slot census, including the default, climate and hunt variants. The issue's larger count includes hero copies.

Every officer slot authors one clip; the shared cannon has RoundsPerClip 1, ReloadTime 10, ShotDelay 1, InitialSalvoDelay 0.4, TriggerSingle 1 and FireSound `com_buff_fire`. The port preserves finite issue, ordinary ammo resupply and the authored reload. It does not invent an inexhaustible reserve or a passive recharge timer. Ammo accounting is established in `re-ammo-reload.md`: BF2 `FUN_00303670` issues N clips and loads one, `FUN_00303890` consumes loaded ammunition, and reload entry/exit `FUN_003274f8` / `FUN_00327658` use `FUN_00303928` / `FUN_00303958` to transfer the finite reserve. Live comparison must settle any special ability supply behaviour beyond this shared path.

## Original execution path

The local reference is `analysis/SWBFRNT2.ELF-decomp.c`, with the same addresses in its annotated counterpart. This document records function addresses instead of transplanting decompiled implementation.

- `FUN_003153a8`, emitter constructor: Radius and all four buff timers begin at zero; AffectFriends and AffectEnemies begin enabled; bit 2 at class+0x164 begins clear.
- `FUN_003156e0`, emitter property dispatcher: Radius writes +0x104; the four timer/value pairs occupy +0x108 through +0x124. Property hash `0x39b3648c` writes bit 2 at +0x164. The shared buff ordnance authors 1 and poison overrides it to 0. Property `0x76f3beea` resolves the activation particle effect at +0x128.
- `FUN_00315f78`, shot: ConeLength > 0 runs the cone/chain path, with MaxTargets. The Radius > 0 path instead gathers up to 128 bodies with mask 0x102a, excluding the shooter, checks alive state and visibility through `FUN_002a7520`, then checks relationship and calls `FUN_00317838`. MaxTargets belongs to the cone loop; it does not limit radius recipients to ten.
- `FUN_002a7520`: visibility uses a 0x9e collision ray, excluding owner/target objects and a rider's vehicle as appropriate.
- `FUN_00317838`: applies the four timers only to the living soldier RTTI classes. A plain vehicle/building is not a buff recipient. Its return indicates an applicable timed effect, even when an existing longer timer prevents replacement.
- `FUN_00315f78` final block: only after another recipient succeeds, and only with bit 2 and AffectFriends enabled, apply the same effect to the living caster. An isolated friendly caster therefore receives nothing; poison never affects its caster.
- `FUN_0024eb18`, `FUN_0024eb70`, `FUN_0024ebc8`, `FUN_0024ec20`: replace a timer and value together only when the incoming timer exceeds the remaining time. Hero-class bit 33 and the immune-class bit at class+0x778 suppress these soldier buffs. Hero recipients are not implemented in this task.
- `FUN_0024ece0`: clear timers on death; decrement timers of at most 300 seconds; apply healing and poison only if their decremented timer remains positive. Poison has a stored damage attribution record and unit/shield scales of 1. Award timers of 340 seconds remain permanent. The port follows this ordering, including the final partial-tick boundary.

The port uses the existing deterministic soldier blast sphere and world/terrain ray helpers. Its per-unit traversal, foot-sphere shape, rider exclusion and existing damage/shield pipeline are approximations of the PS2 body's query machinery. Dense-query ordering, exact edge-of-radius contact and shield interaction require live comparison. The friend and enemy tests use the mission's team relations (`Sim::allied` / `Sim::hostile`), so a native team friendly to the caster takes AffectFriends effects and never AffectEnemies ones (2026-11-04, issue #82: BF2 officers had been poisoning the Jawas both armies are friends with).

## Presentation

The weapon's IconTexture now reaches the secondary HUD row through the ordinary weapon loader. The activation event plays the authored FireSound and attached effect from property 0x76f3beea: `com_sfx_buff_offense`, `com_sfx_buff_defense`, `com_sfx_buff_regen` or `com_sfx_buff_poison`.

Original soldier constructor defaults at +0x8a8/+0x8ac/+0x8a4/+0x8b0 are the effect hashes `c17ab11a`, `28b9f3ac`, `bb3e84e9`, `caa14018` for offense, defense, regeneration and poison. `FUN_002368a0`, `FUN_00236a78`, `FUN_002366c8`, `FUN_00236c50` attach the matching aura while its timer remains positive and stop it when the effect ends. The port resolves those existing baked effects by hash, follows the recipient and stops emission at expiry or death. It uses the original default bindings; class-specific aura overrides, cloaking suppression and first-person visibility parity await comparison. No new asset baking is required.

## Bot policy

PROVISIONAL port policy, not a claim about the original AI: a bot can select its secondary ability when a visible recipient needs it. Healing requires missing health and no current regeneration; poison requires no current poison. Damage and defense require the corresponding timer to be absent and an enemy within 30 m of that recipient with clear terrain and world visibility. Ordinary collision visibility still gates the actual recipient query. An exhausted slot is not selected. These abilities are excluded from generic attack-weapon scoring, so a friendly boost cannot displace a gun as an attack weapon. This policy consumes no random draws.

## Validation to run

Added thirteen asset-free core regressions: radius and relationship filtering, self-on-success, walls, more than ten recipients, refresh precedence, regeneration cap and expiry, poison effects, exact expiry and death credit, multiplied blast damage and independent expiry, secondary release timing and finite supply, state hashing, bot support decisions, and the ten-second resupplied reload. Added two data-gated integration cases covering all fourteen officer variants and the deferred hero/invulnerability definitions.

From `engine/`:

```text
cargo test -p core
cargo test -p game --test suite officer_abilities -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
```

These tests have been written, not executed or independently reviewed. No rendered output, gameplay output or packed-executable smoke test was validated in this session.

For the original-game comparison, use BF2 Republic/CIS and Alliance/Empire conquest:

1. Select each officer and activate with one ally/enemy inside the radius, one outside it, and one behind a wall. Repeat alone and behind the caster. Confirm whether the caster receives the friendly effect only after a successful recipient.
2. Compare matched shots during Rage and Rally and immediately after 15/20 seconds. Record damage from PS2 RAM and the port's black box; use the same target class and hit location.
3. Wound an ally, apply regeneration and record health for 30 seconds. Poison an enemy and record health/death credit for 15 seconds, including a caster death before expiry.
4. Spend the single use, wait without ammo supply, then receive ammo and measure reload/availability. Confirm the finite supply interpretation and the authored ten-second reload against live behaviour.
5. Record paired screenshots/video of the activation, each timed aura and its expiry. Compare recipient particle placement, class-specific overrides and local first-person visibility.

Do not mark issue #47 complete until those required tests and the live original-game comparison are accepted.

## Local playtest build

Release compilation succeeded. `game.exe --write-pack` packed the current assets and missions (23,923 files) into `dist/openbf-officer-abilities.staged.exe`; packing succeeded and the staged file replaced `dist/openbf.exe`. `dist/openbf_data`, profiles and playtest files were preserved. No affected asset bake was needed. Build/pack logs: `engine/artifacts/lead_tmp/officer-abilities-release.log` and `officer-abilities-pack.log`. The packed output was not smoke-tested or otherwise validated.

## Cleanup review before commit

At the maintainer's follow-up request, reviewed module ownership and removed avoidable duplication. Bot decisions now live in `core/src/ai/abilities.rs`, read an immutable simulation and reach the game solely through the foot gunner's ordinary secondary/switch inputs. They no longer select simulation weapon slots directly. The radius iterator is shared by activation and support selection and does not allocate during bot selection. It uses the existing perception visibility helper.

`core/src/sim/buffs.rs` owns timed state, refresh precedence, hash representation and healing/poison ticking. Pickups and abilities share multiplier refresh; the step orchestrator advances buffs independently of pickups. Bolt, blast and poison consequences share hit notification, attacker memory and death bookkeeping through the combat module. The roster parses each accepted weapon once and preserves the common metadata path. Particle effects share their spawn constructor and unit-follow operation. No dependencies, baked assets or code comments were added.

Tests, Clippy, rendered/gameplay output and the live original comparison remain unvalidated per the Astra instruction. Release compilation and packing are performed to refresh the local playtest executable, not as a claim that feature acceptance passed.
