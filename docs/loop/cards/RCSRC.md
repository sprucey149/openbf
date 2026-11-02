---
id: RCSRC
title: re_check.py absent --src searches a 0x term only as ASCII text in raw files, so a disc-hash absence receipt can never find a PROP key in a LVL: add the little-endian word needle for --src (as --elf has) and a control term in the receipt
milestone: M0
value: 3
phase: cleanup
status: done
deps:
files: scripts/re_check.py
declares:
needs:
effort:
branch:
resume: Scope transferred to RECHK by planner 2026-10-24 08:25 UTC; no implementation, passing receipt or landing. Original evidence/acceptance preserved here and verbatim in RECHK; completion of those bars belongs to RECHK. See engine/artifacts/loop/reports/20261024_0825_planner.md.
created: 2026-10-22
---
## Why
AUDIT of re-ordnance-kinds.md (2026-10-22): `re_check.py absent b1 0x6d4a8840 0x129f08c1 --src .../SIDE/IMP.LVL --src .../SIDE/REP.LVL` [receipt:e0ba1c6f] read 0 raw hits in BOTH files, yet entc.py reads WaverRate 0x6d4a8840 and WaverTurn 0x129f08c1 = 0.5 in REP.LVL's rep_weap_inf_torpedo_launcher_ord. `_needles()` (scripts/re_check.py ~81) adds the struct.pack('<I') needle only when elf=True; a raw --src source gets the ASCII form only. Census of the 309 receipts: 4 pair a 0x term with a raw --src source, all read 0 (AUDITCLSE d92490df, CLSE2 03bc27c2, MIXH 78f2aa0a, UORD e0ba1c6f); the claims citing them stand on other scanners (propscan: AIBubble* 0 BF1 PROP chunks with a TurnRate control of 99; re-mix-headroom notes its receipt's raw scan is ASCII), so no doc is wrong today.

## Notes
- Byte search of a 4-byte word in a whole LVL can hit inside unrelated data (float / pointer collisions): print the hit offsets so a reader can see whether each sits in a PROP chunk, or point the user to propscan.py for disc keys.
