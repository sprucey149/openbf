# Bothan invisibility, BF2

2026-10-28, CLOAK, [O] UNAUDITED. Static disc/executable evidence plus a controlled live PS2 third-person visibility/flicker comparison below; broader acceptance remains open.

`analysis/INDEX.md` was read before this investigation. Existing references are `re-ai-difficulty.md` section 5, `re-soldier-ai.md` section 4.8, `re-heat-hud.md` and `re-officer-abilities.md`. The historical ENGR and equipment issues do not describe the current implementation of packs, buffs, explosives or remote droids.

## Disc census [D]

Across the locally baked side class databases, derived from the local PS2 LVLs, BF2 has one invisibility weapon: `all_weap_inf_invisibility`. Four soldier records inherit it: `all_inf_default_officer`, `all_inf_officer`, `all_inf_officer_jungle`, `all_inf_officer_snow`. The default record is an inheritance template; the three concrete records are the Bothan Spy. Slot 2, channel 0, unlimited clips. BF1 has zero invisibility engine-base weapons in those databases. The integration test repeats the census through ClassDb inheritance rather than matching names in source text.

The weapon authors EnergyDrain 17, activation threshold/cost (hash b4932ba7) 40, InitialSalvoDelay 0.3, mode 2f19df96 = Toggle. Its concealment parameters are moving 0.3, stationary 1.0, fast speed 9.5, slow speed 6.5, fade-in rate 0.6, fade-out rate 0.9, release linger 1.0, flicker amount 0.6 and random interval 5..10 seconds. Names of the hashed concealment keys remain unresolved and are kept as hashes in the loader.

## Executable evidence [O]

BF2 `SWBFRNT2.ELF`, local decompiler corpus and raw ELF:

- `FUN_00339358` initializes the cloak class. `FUN_003394e0` dispatches the hashed keys above. `FUN_003398b0` registers the cloak types at strings 629238/629250; `FUN_00339938` returns the invisibility type global 699f48.
- `FUN_00338ba8` checks the owner's current energy against class+270 before activation. `FUN_003388b8` charges class+270 for an inactive cloak; the Toggle mode takes the active cloak through deactivation instead.
- `FUN_003389c8` waits for class+26c (InitialSalvoDelay); `FUN_00338e40` / `FUN_00338ee0` enable the cloak. `FUN_00338ec8` begins the release linger; `FUN_00338f00` clears active/linger flags. The put-away override `FUN_003387e8` invokes release; the owner-change override `FUN_00338ad0` clears it.
- `FUN_00339018` drains class+1cc (EnergyDrain) while active through `FUN_001e2010`. That function uses the common energy depletion latch. This is the soldier's shared energy pool, not a separate fixed-duration cloak timer. The same update blends concealment between the stationary and moving strengths using squared speed and a squared interpolation weight; it fades toward the result at the authored rates. A change in the owner's health field resets accumulated concealment. An occupied vehicle clears the cloak.
- `FUN_00338f10` subtracts the flicker amount and draws its next interval from the two-step gameplay LCG. The damage receiver `FUN_0023cfd0` calls it for damage descriptor flags intersecting 142a. The port currently reveals on positive health damage, including poison. Exact descriptor-category parity remains an acceptance item.
- `FUN_002376a8` stores concealment as a byte, truncating strength*255. `FUN_002aead8` multiplies AI visibility by 1-concealment. `FUN_00239bb0` exits the soldier render path when that byte is nonzero. Buff effect updates `FUN_002366c8`, `FUN_002368a0`, `FUN_00236a78`, `FUN_00236c50` suppress their aura while concealed. HUD targeting `FUN_003a5940` also excludes it.
- `FUN_002b8160` selects WeaponInvisibility when the soldier has no threat. The port uses that rule; normal weapon selection explicitly excludes cloaks as damaging weapons. Existing three-second contact memory retains the last known position without seeing or following the concealed unit. This memory policy belongs to openbf's existing AI design.

Reproducible reads: `python scripts/re_check.py word b2 0x629140 36 --u32` locates cloak instance overrides; `python scripts/tools/eedis.py bf2 0x32b5b0 0x32b5b8` reads the inherited energy modifier at vtable+74 (a return-zero stub). The shared soldier energy update `FUN_00240bf8` calls this modifier before restoring energy. Cloak does not replace the class's normal restoration with a guessed fixed recharge time.

## Implementation and remaining comparisons

Select the cloak on the primary weapon channel and tap fire. A held trigger is one activation. Tap again to release; switching away, using secondary equipment, mounting or dying cleans up the activation. Damage reveals without consuming another activation; concealment can recover while the effect remains active. The normal energy HUD displays the shared resource, the existing stealth HUD key identifies the selected equipment, and the authored FireSound plays at activation. No projectile, blaster flash or invented particle effect is emitted.

The initial implementation had no live comparison. A follow-up found the project's existing emulator and performed the controlled third-person comparison below. Remaining comparisons: damage category mask versus actual hit types, exact frame ordering of activation/deactivation and audio, health-change flicker, corpse and first-person rendering, and original AI retention timing. These broader original-game acceptance items are not claimed complete.

## Live PS2 visibility and flicker follow-up [M]

The maintainer reported a remembered intermittent transparent/shimmering character and was unsure whether that memory was PS2 or PC. The project's PS2 baseline was therefore retained. PCSX2 2.8.2, local BF2 USA v2.01 disc, SLUS-21240; boot ELF CRC 02F4B541, game ELF CRC 62390B9E. Ran an isolated portable copy under engine/artifacts/cloak_ps2, with copied BIOS, memory cards and state files; the original profiles/states were not overwritten. Staged a Rebel Bothan through the original spawn request using PINE fields, bypassing the points gate only for this capture. Did not patch the original rendering instructions.

Unmodified native cloak update, stationary: captured 3,937 PINE samples over about 21 s. Periodic strength drops were 1.0 to 0.399999976 (float bits 3ecccccc), byte 101, with newly drawn intervals 5.281849861, 5.219724655 and 7.126437664 seconds. Recovery adds about 0.02 per 30 Hz update. Raw local evidence: engine/artifacts/equipment/cloak-native-flicker.json. This agrees with the existing port's 0.6 subtraction, 0.6/s recovery and two-step LCG interval, rather than establishing a transparent body.

For a controlled draw comparison, held strength at 0, 0.4 and 1.0 by temporarily zeroing the local class's fade rates, flicker amount and energy drain. At zero the native Bothan body is fully drawn; at 0.4 (byte 102) and 1.0 (byte 255) it is absent, with no transparent silhouette in these third-person captures. Native F8 images stay local in engine/artifacts/equipment/cloak-reference/native-visible.png, native-partial.png and native-full.png. These are parameter-isolation probes, not claims about unmodified activation timing, energy duration, first-person or audio.

Raw disassembly confirms the draw gate: 00239c00 `lbu v0, 0x7e3(s3)`, 00239c04 `bne v0, zero, 0023cf88` (the render epilogue). Thus a stationary flicker from 1.0 to about 0.4 does not make the PS2 body visible. At the authored moving strength 0.3, subtracting 0.6 reaches zero and permits a brief full-body reveal before strength recovers. This is the verified PS2 behavior; the remembered transparent effect in another version remains unverified. No arbitrary alpha/refraction effect was added in response to the memory.

The port regression covers stationary partial concealment, exact native float/byte after a flicker, recovery, moving-strength reveal/re-hide and deterministic interval RNG consumption. This validates the existing PS2 rule without changing gameplay or rendering.
