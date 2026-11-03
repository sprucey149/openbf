---
id: DCHK
title: Droid soldiers blow apart as the PS2's do (BF1 CW, BF2): a class with chunk geometry rolls ChunkFrequency 0.6 at every push call - no push and a sticky mark, or pushed with a dead one's health set to -4 - and again at its death: the chunk death (ExplosionName's effect at the body, the class's chunk thrown at 0.1 + 0.018 x overkill, no corpse) - today a droid is pushed every time and always leaves a corpse
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
On the PS2 a battle droid near a blast often does not fly: 6 times in 10 it stands, and when it dies it bursts into
its chunk with the droid explosion and leaves no body. The port (after KNOCK) pushes every droid and always leaves a
corpse.

## Evidence
- `analysis/re-knockback.md` section 0 row 7, section 1 step 2 (the roll in the push, before the distance test; the
  -4.0 health), section 3 (the death block's roll, the mark, what the chunk death calls), section 4 (BF2's twin:
  class+0x68 / class+0x121c).
- Live, BF1 Tatooine CW (`engine/artifacts/lead_tmp/knock/out/bf1_tat1d_knock.json`, `droid_ana.py`): 34 droid
  subjects inside a push blast: 17 not pushed, each with the mark; 17 pushed; the 8 pushed dead at health -4.0; the 7
  unpushed dead in state 3 one update later with their body out of the world; 2 of 4 droids outside the radius but
  inside the gather box marked.
- Disc and RAM: the four BF1 droid soldier classes author ChunkFrequency 0.6, CHUNKSECTION CHUNK1 (`bdroid_chunk1` /
  `sbdroid_chunk1`) and ExplosionName `com_inf_droid_exp` (an Effect `droid_explosion` only); other classes have no
  chunk geometry and never roll (`analysis/unit-audit-bf1.md`).
- The port: vehicle chunks exist (card VDTH: `game/src/vehicle_class.rs` CHUNKSECTION, the chunk bodies); soldier
  classes read neither key today.

## Acceptance (run these, paste the output)
1. Class reader test: ChunkFrequency and the chunk geometry of every soldier class of both discs - 0.6 and a chunk on
   exactly the droid soldier classes (the four BF1 ones by name), none elsewhere.
2. Core tests on a seeded sim: of 1,000 droids in a push blast 600 +- 46 are not pushed and each of those is marked;
   a marked droid that dies later takes the chunk death; an unmarked one takes it 6 times in 10; a pushed dead droid's
   health is -4.0 and its chunk scale 0.172; a clone in the same blast is always pushed and never chunks.
3. A chunk death leaves no corpse in the blast gather and none drawn; the chunk flies with VDTH's chunk rules at the
   scale 0.1 + 0.018 x overkill (1.0 from 50).
4. One PCSX2 picture pair of a battle droid's chunk death (the chunk and the effect) - `knock_probe.py --enemy --droids
   --knockin 10,4.5,4.5` on `tat1_cp0_cp10_postA_spawn.p2s` puts droids round a grenade; grab at the blast.
5. Sweep on main and branch (8 seeds x 32 maps, paired by seed): the BF1 CW maps' rows.

## Notes
- Filed 2026-10-07 by KNOCK's RE. BF2's droid classes: check `unit-audit-bf2.md` for which author the two keys before
  building the BF2 half.
- 2026-10-20 (the maintainer): in BF2 a CIS droid killed by an explosion or a headshot bursts into a small fire / disintegrates with a noise (the explosion does no damage); every class had its own death noise (UCRY has the noises). This card is the droid break-up; whether the PS2 has other death-TYPE effects (a headshot, a burn, a dissolve) is open - RE them here or file a sibling card.
