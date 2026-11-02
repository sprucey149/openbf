# LEAD VERIFICATION (Fable lead, session 14, 2026-09-08): VERIFIED.
Re-read by the lead: the raw ELF words at 0x2a2e20 / 0x2a2e28 / 0x2a2e68 / 0x2ae85c (PT_LOAD read of
extracted/bf2/SWBFRNT2.ELF, decoded by hand — COP1 fmt S funct 0x28 = the EE `max.s`, `mtc1 $zero,$f1`,
`mov.s $f1,$f0`); SWBFRNT2.ELF-decomp.c lines 224868–224877, 224734–224738, 224752, 233314–233320 (the
floor getter, the skill clamps, the [floor, 2.0] clamp), 323339–323416 (the whole yaw-spread predicate and
step table), 320352–320361 (ZoomMax loader, 0 → 1.0), 319497–319500 (ctor default 1.0 at +0xb4);
fnv1a('zoommax') = 0x2a96e8b4, 'zoommin' = 0x1881b1a6, 'zoomrate' = 0x9573fb20, 'triggersingle' =
0xed9d56db recomputed; odf_dump.txt's imp_weap_inf_sniperrifle ZoomMax 6.0 / TriggerSingle 1. NOT re-read
by the lead: the BF1 lines (B1:*), the ctor-identification chain through the "cannon" string, the VU0
distance block. The port rule and the corrections in §4 stand; Phase C addendum 4 carries them into the
build.

# RE: the AI aim-error floor is a FLOAT (`max.s`, no int cast), and the AI sniper distance table's ODF keys and predicate — BF1 + BF2

Corpora: `analysis/SWBFRNT2.ELF-decomp.c` (BF2, "B2:" line numbers), `analysis/SWBFRONT.ELF-decomp.c`
(BF1, "B1:"), raw instruction words and strings from `extracted/bf2/SWBFRNT2.ELF` / `extracted/bf1/SWBFRONT.ELF`
(PT_LOAD reader + capstone, `scratch/disas.py`, listing saved as `disasm.txt`), and the `wpnc`/`entc` PROP
chunks of the PS2 SIDE LVLs read directly (`scratch/props.py`, dump saved as `odf_dump.txt`). Hashes = fnv1a
over `byte|0x20` (`classdb.rs::fnv1a_lowercase`; every hash below was recomputed from the key name). Tags:
**[V]** read in the decomp / raw words / chunk bytes, **[D]** derived (arithmetic, or a meaning inferred from
use), **[H]** hypothesis. Written for HOLD 19(g) and 19(e) (`HANDOFF-OPUS.md`), i.e. for
`brief_phase_c.md` D9 / D10 and `re-ai-difficulty.md` §4.1.

## §0 Headline

1. **Question A — there is no integer cast in `FUN_002a2df8`.** Ghidra's *(decompiler detail omitted)*
   (B2:224876) is its rendering of the EE-only `max.s $f0,$f0,$f1` with `$f1 = 0` (word `46010028` at
   0x2a2e28, `mtc1 $zero,$f1` at 0x2a2e20) [V]. The function returns the float `max(0, 1.6 − 1.2·t)`,
   `t = (skill−1)·(1/19)` in single precision, and the consumer `FUN_002ae7b8` uses it AS a float
   (`mov.s $f1,$f0` at 0x2ae85c, then `c.ole.s`/`min.s` against 2.0) [V]. So the floor is CONTINUOUS:
   1.6 m at skill 1, **0.968 m at skill 11**, 0.4 m at skill 20 — never 0 inside 1..20. Neither
   `cvt.w.s` nor `trunc.w.s` occurs anywhere in 0x2a2df8..0x2a2e34, so the EE FCSR rounding question is
   moot. The same idiom is the error CAP (`FUN_002a2e38`, word `46010028` at 0x2a2e68; flyer cap
   `FUN_002a2e78` at 0x2a2ea4), consumed as a float too (`mov.s $f22,$f0` after `jal 0x2a2e38` at
   0x2ae5a4) — so the cap is `max(0, 2.2 − 1.0·t)`, 2.2 → 1.2 m, not "2 m then 1 m" [V].
   `re-ai-difficulty.md` §4.1 (rows 201–202, the per-level table's last column), §4.3, §7 item 4 and §0
   are wrong on this and the exact replacement lines are in §4 below. All three of the conflicting answers
   in HOLD 19(g) (1 m / 0 m / `s < 12`) were wrong: the formula is `max(0, lerp(1.6, 0.4))` as a float.
2. **Question B — the sniper table is FULLY decoded, identical in both games.** `GetYawSpread`
   (BF2 `FUN_0032e4b8`, BF1 `FUN_002889e8`) applies the seven-entry table when ALL of: the weapon class's
   `YawSpread` is 0.0; the fire point's owner index is negative (an AI shooter); `TriggerSingle` is set;
   `ZoomMax ≥ 6.0` (inclusive); and the shooter's AI has a current target. Then the table value, indexed by
   the SQUARED 3-D distance to the target with strict `<` against 400/1600/3600/6400/10000/14400 (no
   interpolation), times the AI dispersion multiplier (weapon+0x138 BF2 / +0x158 BF1), goes into the
   `YawSpread` SLOT of the half-angle: `max(spreadState, 0) + postureBase + tableValue`. It does NOT
   replace `max(s, 0)` — `re-blaster-damage.md` §4.3 and brief D9 say it does; correction in §4.
   Keys: `ZoomMax` 0x2a96e8b4 → BF2 class+0xb4 / BF1 +0x7c, loader coerces 0 → 1.0, ctor default 1.0;
   `ZoomMin` 0x1881b1a6 and `ZoomRate` 0x9573fb20 are loaded beside it and NOT read by the table;
   `TriggerSingle` 0xed9d56db → BF2 +0x1e0 bit 0x32 / BF1 +0xcc; `YawSpread` 0xebefdcd7 (BD §7.2) [V].
3. **The four roster sniper primaries all satisfy the predicate on disc** [V]: `all_weap_inf_sniperrifle`
   (TriggerSingle 1, ZoomMax 8.0, YawSpread 0.0), `imp_weap_inf_sniperrifle` (1, **6.0**, 0.0 — the
   inclusive `≥` is load-bearing), `com_weap_inf_sniper_rifle` (1, 8.0, 0.0; the base of both
   `rep_weap_inf_sniper_rifle` and `cis_weap_inf_sniper_rifle`, whose own 13 props are geometry, ordnance
   and sounds only). Controls `all_weap_inf_rifle` and `com_weap_inf_rifle` (base of `rep_weap_inf_rifle`)
   fail all three class conditions (TriggerSingle 0, ZoomMax 2.5, YawSpread 0.25).

## §1 Question A — `FUN_002a2df8`, the error-radius floor (BF2)

**The getter chain** [V]:
- `FUN_002a29d8` (B2:224654) returns the skill: base table + deltas + auto-balance, then B2:224734–224735
  *(decompiler detail omitted)* and B2:224737–224738 *(decompiler detail omitted)*
  — the lower clamp is unconditional (skill 0 never exists on the PS2), the upper clamp only when the
  getter asks for it.
- `FUN_002a2bb8` (B2:224744): B2:224752 *(decompiler detail omitted)* — words 0x2a2bd8..0x2a2be8
  `mtc1 $v0,$f0; cvt.s.w $f0,$f0; mul.s $f0,$f0,$f1` with `$f1 = 0x3d579436` (= 0.0526315793, the f32
  nearest 1/19). This `cvt.s.w` (int → float) is the ONLY conversion in the whole chain.
- `FUN_002a2bf8` (B2:224756): `t·(b − a) + a` — words 0x2a2c14 `sub.s`, 0x2a2c1c `mul.s`, 0x2a2c24 `add.s`.
- `FUN_002a2df8` (B2:224868–224877): B2:224875 *(decompiler detail omitted)*
  (1.6f, 0.4f; 4th arg 0 = no upper clamp — word 0x2a2e1c `0000282d` = `daddu $a1,$zero,$zero`),
  B2:224876 *(decompiler detail omitted)*. Raw words:

```
002a2e18  0c0a8afe  jal   0x2a2bf8          ; lerp(1.6, 0.4) -> $f0
002a2e1c  0000282d  daddu $a1,$zero,$zero   ; (delay slot) clamp flag = 0
002a2e20  44800800  mtc1  $zero,$f1         ; $f1 = 0.0
002a2e24  dfbf0000  ld    $ra,0($sp)
002a2e28  46010028  max.s $f0,$f0,$f1       ; COP1 fmt=S ft=$f1 fs=$f0 fd=$f0 func=0x28 = EE MAX.S
002a2e2c  03e00008  jr    $ra               ; returns the FLOAT in $f0
```

  Decode of `46010028`: opcode 010001 (COP1), fmt 10000 (S), ft 00001, fs 00000, fd 00000, function
  101000 = 0x28. On the EE the COP1.S function codes are 0x24 CVT.W.S, 0x28 MAX.S, 0x29 MIN.S (EE Core
  Instruction Set Manual; capstone's MIPS32 model does not know MAX.S/MIN.S, hence `<undecoded>`) [V for
  the word; the mnemonic is the EE manual's]. Ghidra models MAX.S as a select and prints it as a
  multiply-by-boolean; the "`(int)`" is a reinterpretation of the float's bits, not a conversion — the same
  print appears where a float is unmistakably meant, e.g. `GetYawSpread`'s `max(spreadState, 0)` at
  B2:323416 `(float)((int)a local * (uint)(0.0 <= a local))`.

**The consumer** `FUN_002ae7b8(growth, tracker)` (B2:233282–233321) [V]:
- B2:233299 `a local = 1.5;` — the floor when the tracker has NO target (`*argument 2 == 0`, words 0x2ae7f4/f8
  `lui $at,0x3fc0; mtc1 $at,$f1` before `beqz $a0` at 0x2ae7fc).
- B2:233314 `a local = (float)…` — words 0x2ae854 `jal 0x2a2df8`, 0x2ae85c `mov.s $f1,$f0`:
  the result is taken from `$f0` as a float; there is no `mtc1`/`cvt.s.w` of `$v0`.
- B2:233317–233320: `if (floor <= err) err = min(err, 2.0) else err = floor` — words 0x2ae86c
  `c.ole.s $f1,$f0`, 0x2ae874 `bc1f`, 0x2ae878 `mov.s $f2,$f1`, 0x2ae87c `460300a9` = `min.s $f2,$f0,$f3`
  (`$f3` = 2.0 from 0x2ae864/68), 0x2ae880 `swc1 $f2,0x20($s0)`. So `err = clamp(err, floor, 2.0)` in floats.

**The cap, same idiom** [V]: `FUN_002a2e38` (B2:224880) = `max(0, lerp(2.2, 1.2))`, word `46010028` at
0x2a2e68; `FUN_002a2e78` (B2:224892) = `max(0, lerp(4.0, 1.2))`, word at 0x2a2ea4. Consumer `FUN_002ae4d0`
(B2:233158): 0x2ae59c `jal 0x2a2e38` / 0x2ae5a4 `mov.s $f22,$f0` (flyer arm 0x2ae58c/0x2ae598), and the
decomp's `min(a local, a local)` at B2:233233–233235 (cap fetched at B2:233199 / 233202) is a float `min` of `err × (1 + 0.6·|v|/maxspeed)` against
the cap.

**The exact floor and cap, skills 0..20** [D from the verified chain; `scratch/floor_table.py`, saved as
`floor_table.txt`]. Single precision throughout; "RN" = IEEE round-to-nearest, "RZ" = round toward zero
(the EE FPU's rounding — hardware fact from the EE manual, not from this repo); the two differ by
≤ 2.4e-7, so the ruling does not depend on it:

| skill | t | floor (RN) | floor (RZ) | cap (RN) | flyer cap |
|---|---|---|---|---|---|
| 0 † | −0.052632 | 1.6631579 | 1.6631578 | 2.2526317 | 4.1473684 |
| 1 | 0.000000 | 1.6000000 | 1.6000000 | 2.2000000 | 4.0000000 |
| 2 | 0.052632 | 1.5368421 | 1.5368421 | 2.1473684 | 3.8526316 |
| 3 | 0.105263 | 1.4736842 | 1.4736842 | 2.0947368 | 3.7052631 |
| 4 | 0.157895 | 1.4105263 | 1.4105263 | 2.0421052 | 3.5578947 |
| 5 | 0.210526 | 1.3473685 | 1.3473685 | 1.9894737 | 3.4105263 |
| 6 | 0.263158 | 1.2842106 | 1.2842106 | 1.9368422 | 3.2631578 |
| 7 | 0.315789 | 1.2210526 | 1.2210526 | 1.8842106 | 3.1157894 |
| 8 | 0.368421 | 1.1578947 | 1.1578947 | 1.8315790 | 2.9684210 |
| 9 | 0.421053 | 1.0947368 | 1.0947369 | 1.7789474 | 2.8210526 |
| 10 | 0.473684 | 1.0315790 | 1.0315790 | 1.7263159 | 2.6736841 |
| 11 | 0.526316 | 0.9684210 | 0.9684212 | 1.6736842 | 2.5263157 |
| 12 | 0.578947 | 0.9052632 | 0.9052632 | 1.6210527 | 2.3789473 |
| 13 | 0.631579 | 0.8421053 | 0.8421054 | 1.5684211 | 2.2315791 |
| 14 | 0.684211 | 0.7789474 | 0.7789475 | 1.5157895 | 2.0842104 |
| 15 | 0.736842 | 0.7157895 | 0.7157896 | 1.4631579 | 1.9368422 |
| 16 | 0.789474 | 0.6526315 | 0.6526317 | 1.4105263 | 1.7894735 |
| 17 | 0.842105 | 0.5894736 | 0.5894738 | 1.3578948 | 1.6421053 |
| 18 | 0.894737 | 0.5263158 | 0.5263159 | 1.3052633 | 1.4947369 |
| 19 | 0.947368 | 0.4631579 | 0.4631581 | 1.2526317 | 1.3473685 |
| 20 | 1.000000 | 0.4000000 | 0.4000001 | 1.2000000 | 1.2000000 |

† skill 0 cannot occur on the PS2 (B2:224734); the row is what the formula gives if the port evaluates it.
Above 20 (the floor getter passes clamp = 0, and auto-balance is ±4, RE §6): skill 21 → floor 0.3368,
cap 1.1474; the `max.s` only bites at skill ≥ 27 [D].

**Answer to HOLD 19(g)**: the floor is `max(0.0, 1.6 − 1.2·t)` as an f32, continuous; at skill 11 it is
0.968 m. The doc's "1 m at skill 11" and "1 m up to skill 12", the brief's `s < 12`, and commit 1's
`int(lerp)` truncation are all wrong — and not only at skill 11: Easy's enemy (4) is 1.41 m not 1 m,
Elite's enemy (17) 0.59 m not 0, and the cap at 17 is 1.36 m not 1 m.

**BF1**: this trace is BF2's. `re-ai-difficulty.md` gives BF1 only the four curves A–D
(`FUN_00236700`); whether BF1 has a tracked-position floor at all is untraced (§Not closed).

## §2 Question B — the AI sniper distance table (both games)

**Where**: the yaw-spread getter, weapon-instance vtbl+0x40 (BD §2): BF2 `FUN_0032e4b8` (B2:323323–323417),
BF1 `FUN_002889e8` (B1:211230–211317). *(decompiler detail omitted)* = the weapon instance; *(decompiler detail omitted)* = its class
(B2:323339 / B1:211247). The structure is line-for-line the same in both; only offsets differ.

**The predicate** — every condition must hold, in this order [V]:

| # | condition | BF2 | BF1 |
|---|---|---|---|
| 1 | class `YawSpread` == 0.0 | B2:323340–323341 *(decompiler detail omitted)* | B1:211248–211249 `class+0x1ec` |
| 2 | AI shooter: fire point's owner index < 0 | B2:323342 `*(firepoint(+0x6c)+0xc0) < 0` | B1:211250 `*(firepoint(+0x68)+0xe8) < 0` |
| 3 | `TriggerSingle` set | B2:323343 `(class+0x1e0 >> 0x32) & 1` | B1:211251 `class+0xcc != 0` |
| 4 | `ZoomMax ≥ 6.0` (inclusive: `6.0 <= zoom`) | B2:323346 `6.0 <= class+0xb4` | B1:211254 `6.0 <= class+0x7c` |
| 5 | a current target | B2:323347–323359: `… both non-null | B1:211255–211260: `FUN_0023d320(firepoint)` = target `+0x188` validated by `+0x18c` (B1:155455–155467) |

The meaning of condition 2 ("index < 0 = no human owner") is BD §2's reading of the weapon-instance ctor
(spread state −SpreadThreshold when `firepoint+0xc0 ≥ 0`, else 0) [D]; the sign test itself is [V].
When any condition fails the getter falls through to the class `YawSpread` (which is 0 under condition 1)
— i.e. for a human on a sniper rifle the yaw term is `max(s,0) + postureBase + 0`.

**The distance** [V arithmetic, D naming]: `d² = |P_target − P_shooter|²` on VU0 (`_vsub`/`_vmul`/`_vaddbc`/
`_vmaddbc`, B2:323362–323370 / B1:211263–211271) between the `+0x30` vectors of the object returned by the
target's vtbl+0x48 (BF2; BF1 vtbl+0x40) and the fire point's `+0x14` vtbl+0x30 (BF2; BF1 `+0x294` vtbl+0x28)
— the entities' positions [D]. It is the squared distance that is compared; there is no sqrt.

**The table** [V] — B2:323373–323398, B1:211274–211299, byte-identical constants, strict `<`, a step
function with NO interpolation:

| d² < | d < | value (rad) |
|---|---|---|
| 400 | 20 m | 0.0997 |
| 1600 | 40 m | 0.0499 |
| 3600 | 60 m | 0.0333 |
| 6400 | 80 m | 0.025 |
| 10000 | 100 m | 0.02 |
| 14400 | 120 m | 0.0167 |
| else | ≥ 120 m | 0.0143 |

**The multiplier and the sum** [V]: B2:323400 *(decompiler detail omitted)* (B1:211301 `× weapon+0x158`),
then B2:323416 / B1:211316
*(decompiler detail omitted)*
— every branch reloads the spread state (*(decompiler detail omitted)* at B2:323401 etc.), so `max(s,0)` is ALWAYS
in the sum and the table value occupies the `YawSpread` slot. `weapon+0x138` / `+0x158` is the AI
distance-dispersion multiplier: instance default 1.0 (BD §2 ctor note), overwritten every AI aim tick with
BF2 `lerp(2, 0)` (RE §4.1 `FUN_002a2f48` → `FUN_002a83e0`) / BF1 curve D 2.0 / 1.0 / 0.0 at skills 1 / 14 / 20
(RE §2.2 `FUN_00237a20`). That curve is the ONE BF1/BF2 difference in this path: at skill 14 BF1 multiplies
by 1.0, BF2 by 0.632.

**The keys and their loaders** [V]:

| key | fnv1a | BF2 loader `FUN_00328fb0` (B2:319917) | BF1 loader `FUN_002864f0` (B1:209793) | ctor default |
|---|---|---|---|---|
| `ZoomMax` | 0x2a96e8b4 | B2:320352–320361 → `class+0xb4`; `if (+0xb4 == 0.0) +0xb4 = 1.0`; mirrored to `+0xc0` | B1:209974–209983 → `+0x7c`; 0 → 1.0; mirrored to `+0x88` | 1.0: BF2 `FUN_00327e98` B2:319499 *(decompiler detail omitted)*; BF1 `FUN_00285a98` B1:209461 *(decompiler detail omitted)* |
| `ZoomMin` | 0x1881b1a6 | B2:320430–320439 → `+0xb0`; 0 → 1.0; mirror `+0xbc` | B1:209888–209898 → `+0x78`; 0 → 1.0; mirror `+0x84` | 1.0: B2:319498 / B1:209460 |
| `ZoomRate` | 0x9573fb20 | B2:320582–320584 → `+0xb8`, no coercion | B1:210277–210281 → `+0x80` | 0: B2:319510 / B1:209474 |
| `TriggerSingle` | 0xed9d56db | B2:321001, 321026 `(v != 0) << 0x32` into `+0x1e0` | B1:210357–210360 → `+0xcc = (v != 0)` | cannon ctor clears it: BF2 `FUN_003304e0` B2:324647; BF1 `FUN_002899c0` B1:211881 (the BF1 base ctor sets 1 at B1:209496; the cannon ctor wins) |
| `YawSpread` | 0xebefdcd7 | → `+0x250`, deg→rad (BD §7.2); `MaxSpread` 0xaedcbd12 also writes it (0.05·v) | → `+0x1ec` | 0: B2:324646 / B1:211884 |

The base WeaponClass ctors were found through the `"cannon"` label registration (string 0x60fa00 → B2:16254
`FUN_003304e0`, 0x300-byte class; 0x527048 → B1:15453 `FUN_002899c0`, 0x250 bytes), whose first call is the
base ctor (`FUN_00327e98` B2:319439 / `FUN_00285a98` B1:209418). Note the earlier candidate `FUN_001bd570`
(B2:65976, `+0xb4 = 1.0`) is the `"dusteffect"` class (string 0x60fcd8 → B2:16461) — a coincidence, not
evidence.

`ZoomMin` and `ZoomRate` are loaded beside `ZoomMax` but nothing in either `GetYawSpread` reads them; the
table's only zoom input is `ZoomMax`. No other "scope" flag or class field is consulted — the predicate IS
the scope test as far as the AI accuracy path goes. (The `+0xbc`/`+0xc0` mirrors are the runtime zoom
state; `ZoomFirstPerson` 0x14ec7e1d → BF2 `+0x1e0` bit 0x33 / BF1 `+0x8c` is a camera flag, unread here.)

**Both games**: predicate, thresholds, constants and the sum are identical; offsets differ as tabulated;
the multiplier's skill curve differs (above). Interpolation: none, in either.

## §3 The four sniper primaries and two controls, from the discs [V]

`scratch/props.py` walks the `ucfb` tree of a SIDE LVL, parses every `entc`/`wpnc` chunk (TYPE, BASE,
PROP = `[u32 fnv1a][NUL string]` — the same layout `mungefs::entity::EntityClass::parse` reads), follows
`WeaponName1` (BF1) / first `WeaponName` (BF2) as `roster::primary_weapon` does, and every BASE. Full dump:
`odf_dump.txt`.

| class (LVL) | primary → base | TriggerSingle | ZoomMin | ZoomMax | ZoomRate | ZoomFirstPerson | YawSpread / PitchSpread | Min/Opt/MaxRange | AI table? |
|---|---|---|---|---|---|---|---|---|---|
| `all_inf_marksman` (bf1 ALL.LVL) | `all_weap_inf_sniperrifle` → cannon | 1 | 2.0 | 8.0 | 0.0 | 1 | 0.0 / 0.0 | 20 / 64 / 128 | YES |
| `imp_inf_scout_trooper` (bf1 IMP.LVL) | `imp_weap_inf_sniperrifle` → cannon | 1 | 6.0 | **6.0** | 0.0 | 1 | 0.0 / 0.0 | 32 / 64 / 128 | YES — only because the test is `≥` |
| `rep_inf_ep3_sniper` → `rep_inf_default_sniper` (bf2 REP.LVL) | `rep_weap_inf_sniper_rifle` → `com_weap_inf_sniper_rifle` → cannon | 1 | 2.0 | 8.0 | 0.0 | 1 | 0.0 / 0.0 | 8 / 64 / 128 | YES |
| `cis_inf_sniper` → `cis_inf_default_sniper` (bf2 CIS.LVL) | `cis_weap_inf_sniper_rifle` → `com_weap_inf_sniper_rifle` (REP.LVL copy) | 1 | 2.0 | 8.0 | 0.0 | 1 | 0.0 / 0.0 | 8 / 64 / 128 | YES |
| control `all_inf_soldier` (bf1 ALL.LVL) | `all_weap_inf_rifle` → cannon | 0 | 2.5 | 2.5 | 0.0 | absent | 0.25 / 0.25 | 0 / 16 / 48 | no (fails 1, 3, 4) |
| control `rep_inf_ep3_rifleman` → `rep_inf_default_rifleman` (bf2) | `rep_weap_inf_rifle` → `com_weap_inf_rifle` | 0 | 2.5 | 2.5 | 0.0 | absent | 0.25 / 0.25 | 0 / 24 / 64 | no (fails 1, 3, 4) |

All zoom/spread keys of the two BF2 sniper rifles live on the shared base `com_weap_inf_sniper_rifle`
(55 props, also `ScopeTexture = weapon_scope3`); the derived `rep_`/`cis_` classes carry 13 props
(GeometryName, HighResGeometry, OrdnanceName, FirePointName, sounds). Every sniper rifle sets
`SpreadPerShot 1.1`, `SpreadRecoverRate 4.8`, `SpreadThreshold 1.6`, `SpreadLimit 6.0` and all six posture
spreads 0.0 — so for an AI sniper the whole yaw half-angle is `max(s, 0) + table × mult` with `s` the AI's
0-floored spread state (BD §4.1), and for a human it is `max(s, 0)` alone.

## §4 Corrections to existing documents (exact text)

### 4.1 `analysis/re-ai-difficulty.md`

Line 26 (§0), old: `… capped at 1–2 m, ×(3−2·visibility)); …`
new: `… clamped to a skill-continuous floor 1.6 → 0.4 m and cap 2.2 → 1.2 m (floats, §4.1), ×(3−2·visibility)); …`

Line 201, old:
`| `FUN_002a2df8` cross | `int(lerp(1.6, 0.4))` | `FUN_002ae7b8` | error-radius floor: 1 m up to skill 12, then 0 |`
new:
`| `FUN_002a2df8` cross | `max(0, lerp(1.6, 0.4))` — a FLOAT (`max.s` at 0x2a2e28; no int conversion; re-ai-sniper §1) | `FUN_002ae7b8` | error-radius floor, continuous: 1.6 m (skill 1), 1.41 (4), 0.97 (11), 0.78 (14), 0.59 (17), 0.4 (20); 1.5 m with no target |`

Line 202, old:
`| `FUN_002a2e38` / `FUN_002a2e78` cross | `int(lerp(2.2,1.2))` / `int(lerp(4.0,1.2))` | `FUN_002ae4d0` | effective error cap: 2 m (skill ≤ 4) else 1 m; flyer targets 4/3/2/1 m |`
new:
`| `FUN_002a2e38` / `FUN_002a2e78` cross | `max(0, lerp(2.2, 1.2))` / `max(0, lerp(4.0, 1.2))` — floats (same `max.s` idiom, 0x2a2e68 / 0x2a2ea4) | `FUN_002ae4d0` | effective error cap, continuous: 2.2 → 1.2 m (2.04 at skill 4, 1.67 at 11, 1.52 at 14, 1.36 at 17); flyer targets 4.0 → 1.2 m |`

Lines 221–226, the last column "error floor / cap", old → new:
`1 / 2` → `1.41 / 2.04` (skill 4); `1 / 1` → `1.28 / 1.94` (6); `1 / 1` → `0.97 / 1.67` (11);
`0 / 1` → `0.78 / 1.52` (14); `0 / 1` → `0.59 / 1.36` (17); `0 / 1` → `0.40 / 1.20` (20).

Line 256, old: `` `FUN_002ae7b8` clamps `err` to [floor `int(lerp(1.6,0.4))`, 2.0]; ``
new: `` `FUN_002ae7b8` clamps `err` to [floor `max(0, lerp(1.6,0.4))` (a float; 1.5 with no target), 2.0]; ``

Line 260, old: `` cap `int(lerp(2.2,1.2))` (flyers `int(lerp(4.0,1.2))`), `vis = ai+0x7c` (§5.3). ``
new: `` cap `max(0, lerp(2.2,1.2))` (flyers `max(0, lerp(4.0,1.2))`), both floats, `vis = ai+0x7c` (§5.3). ``

Lines 370–371 (§7 item 4), old: `` floor\n   `int(lerp(1.6,0.4))` (1 m below skill 12), cap 2 m/1 m, ``
new: `` floor\n   `max(0, lerp(1.6,0.4))` m (1.6 → 0.4, continuous; 0.97 at skill 11), cap `max(0, lerp(2.2,1.2))` m (2.2 → 1.2), ``

### 4.2 `analysis/re-blaster-damage.md` §4.3 (line 276 ff.)

Old: `IF the weapon is TriggerSingle with ZoomMax ≥ 6 (i.e. sniper rifles), the yaw spread getter replaces
`max(s,0)` with a distance-based dispersion (radians) × weapon+0x158 (BF1) / +0x138 (BF2, default 1.0;
per-difficulty writer not traced — OPEN):`
New: `IF the class's YawSpread is 0, the weapon is TriggerSingle with ZoomMax ≥ 6.0 (inclusive — the
Imperial scout rifle is exactly 6.0) and the AI has a current target, the yaw spread getter puts a
distance-based dispersion (radians, step table on the SQUARED distance, no interpolation) × weapon+0x158
(BF1) / +0x138 (BF2; written every AI aim tick, re-ai-difficulty §4.2) into the YawSpread slot:
yaw = max(s,0) + postureBase + table×mult. `max(s,0)` is NOT replaced (re-ai-sniper §2).`
Also in §7.1 line 448 add: "ZoomMax is the only zoom key the AI table reads; ctor default 1.0 (BF2
FUN_00327e98 :319499, BF1 FUN_00285a98 :209461)".

## §Reproduction

All commands from the repo root; nothing under `extracted/` is written.

```
S=engine/artifacts/lead_tmp/s14/re/sniper/scratch
python3 $S/disas.py extracted/bf2/SWBFRNT2.ELF 2a2df8 0x40      # 46010028 at 0x2a2e28, no 0x46..0024/0d
python3 $S/disas.py extracted/bf2/SWBFRNT2.ELF 2ae7b8 0xe0      # mov.s $f1,$f0 after jal 0x2a2df8; c.ole.s; 460300a9
python3 $S/disas.py extracted/bf2/SWBFRNT2.ELF 2a2bb8 0x40      # cvt.s.w + mul.s by 0x3d579436
python3 $S/disas.py extracted/bf2/SWBFRNT2.ELF 2ae4d0 0x210 | grep -A3 'jal        0x2a2e38'
python3 $S/floor_table.py                                       # the skills 0..20 table
python3 $S/findstr.py extracted/bf2/SWBFRNT2.ELF cannon         # 0x60fa00 -> B2:16254 -> FUN_003304e0
python3 $S/findstr.py extracted/bf1/SWBFRONT.ELF cannon         # 0x527048 -> B1:15453 -> FUN_002899c0
python3 $S/elfrd.py extracted/bf2/SWBFRNT2.ELF str 60fcd8       # "dusteffect" (the false ctor candidate)
sed -n '224868,224877p;233282,233321p;323323,323417p' analysis/SWBFRNT2.ELF-decomp.c
sed -n '211230,211317p;209974,209983p;210357,210360p' analysis/SWBFRONT.ELF-decomp.c
python3 $S/props.py all_inf_marksman imp_inf_scout_trooper all_inf_soldier -- \
  extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL
python3 $S/props.py rep_inf_ep3_sniper cis_inf_sniper rep_inf_ep3_rifleman -- \
  extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL extracted/bf2/lvl/DATA/_LVL_PS2/INGAME.LVL
python3 $S/make_artifacts.py                                    # regenerates disasm.txt, odf_dump.txt, floor_table.txt
```

Hash check: `python3 -c` over fnv1a(`byte|0x20`) gives ZoomMax 0x2a96e8b4, ZoomMin 0x1881b1a6,
ZoomRate 0x9573fb20, TriggerSingle 0xed9d56db, YawSpread 0xebefdcd7, MaxSpread 0xaedcbd12 — all equal to
the constants in the loaders.

## §Not closed

1. BF1's tracked-position error floor/cap: not traced (the RE doc covers BF1 only through curves A–D);
   the port uses BF2's numbers by D1's choice anyway.
2. Condition 2's meaning ("fire point `+0xc0`/`+0xe8` is the owner's player index") is BD §2's reading of
   the instance ctor; the sign test is verified, the field's name is [D].
3. The two `+0x30` vectors are read as entity positions [D]; which reference point (eye, hard-point,
   capsule centre) is a ≤ 2 m question that only moves a target across a 20 m band edge.
4. `GetPitchSpread` (BF2 0x32e720 / BF1 `FUN_00288c20`) having no table is BD §2's claim, not re-read here.
5. D13's Snipe-hint "has a scope" predicate: the AI-accuracy path has exactly one scope test (this one);
   whether the hint/stance logic uses another was not searched.
6. What the mirrors `+0xbc`/`+0xc0` (BF2) / `+0x84`/`+0x88` (BF1) do at runtime (zoom state) — irrelevant
   to the table, not traced.
