---
id: RECHK
title: Reliable hash absence scans: normalize Ghidra constant forms, find ELF code-built hashes and scan raw --src little-endian words with per-term controls
milestone: M0
value: 3
phase: build
status: done
deps:
files: scripts/re_check.py
declares:
needs:
effort:
branch: rechk
resume: (private workflow note omitted)
created: 2026-10-16
---
## Batch scope (planner 2026-10-24)
RECHK and RCSRC are one bounded scanner/receipt change in scripts/re_check.py;
all original checks remain below as two independent acceptance sets. Run each
scanner path with its own present-key and absent-key controls; fixtures alone do
not replace the corpus/ELF/LVL reproductions. RCSRC's no-corpus-change bar means
that enabling raw-source word needles adds no corpus hits relative to RECHK's
normalized-text control. The RECHK text-form change is intentional and measured
separately. No receipt or existing absence conclusion is silently rewritten.
HASHN's annotation and name-query outputs remain a separate card.

## Why
Card LOD (2026-10-16) found BF1 FUN_0020f2e0 123451 / BF2 FUN_00252bb0 171132 comparing *(decompiler detail omitted)* (GeometryLowRes): `re_check absent b1 0xa490eba1` returns 0 hits (the term with its 0x prefix is not a substring of the 64-bit form), and `--elf` scans only whole LE u32 data words, so a hash built by lui/ori in code is invisible too (the GeometryName control 0x47c86b4a: 53 decomp hits, 0 raw ELF hits). re-softskin 7 called the property dead on that evidence.

## Evidence
`engine/artifacts/lead_tmp/re/lod/checks.txt`; `lead_tmp/re/lod/tools/luiscan.py` (lui + ori/addiu pair scan: GeometryLowRes 1 build per ELF, GeometryName 19 / 25); analysis/re-lod.md 5.

## RCSRC scope transferred in full (implementation and acceptance still open)
## Why
AUDIT of re-ordnance-kinds.md (2026-10-22): `re_check.py absent b1 0x6d4a8840 0x129f08c1 --src .../SIDE/IMP.LVL --src .../SIDE/REP.LVL` [receipt:e0ba1c6f] read 0 raw hits in BOTH files, yet entc.py reads WaverRate 0x6d4a8840 and WaverTurn 0x129f08c1 = 0.5 in REP.LVL's rep_weap_inf_torpedo_launcher_ord. `_needles()` (scripts/re_check.py ~81) adds the struct.pack('<I') needle only when elf=True; a raw --src source gets the ASCII form only. Census of the 309 receipts: 4 pair a 0x term with a raw --src source, all read 0 (AUDITCLSE d92490df, CLSE2 03bc27c2, MIXH 78f2aa0a, UORD e0ba1c6f); the claims citing them stand on other scanners (propscan: AIBubble* 0 BF1 PROP chunks with a TurnRate control of 99; re-mix-headroom notes its receipt's raw scan is ASCII), so no doc is wrong today.

## Notes
- Byte search of a 4-byte word in a whole LVL can hit inside unrelated data (float / pointer collisions): print the hit offsets so a reader can see whether each sits in a PROP chunk, or point the user to propscan.py for disc keys.
## Planner checkpoint (2026-10-24 13:06 UTC)
Fixture-first scanner controls are possible; real BF1/BF2 corpus/ELF and REP/IMP LVL reproductions and four old receipts remain required. Keep all merged RCSRC acceptance checks. Neither a synthetic raw word nor a fixture text replaces the actual GeometryLowRes/GeometryName and Waver cases.

## GitHub checkpoint recovery (2026-10-24)
