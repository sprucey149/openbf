---
id: HASHN
title: Hash-annotated decompiles: every FNV constant in both PS2 decomps followed by its name, and re_check.py hash <name> lists the functions that read a property
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
(the maintainer's INBOX 2026-10-19, "a tool to build") The RE often asks "which functions read ODF property X": today that is a hand fnv-hash of the name and a grep for the constant in both of its forms (RULES: the signed / 64-bit forms; `lead_tmp/veh0/*.py`). An annotated copy of each decomp, every FNV constant followed by its name as a comment, makes it one grep.

## Evidence
The decomps `analysis/SWBFRONT.ELF-decomp.c` / `SWBFRNT2.ELF-decomp.c`; name sources: the discs' ODF keys, Locl keys, class names, the `StringsFromExecutable` lists, the bake's property tables; the hash forms Ghidra prints (unsigned hex, signed decimal, 64-bit sign-extended, split lui/addiu pairs).

## Notes
Tooling only (no game code); value 3.
## Planner checkpoint (2026-10-24 13:06 UTC)
Fixture-first development is possible, but both full corpora, original name sources and the five real-property reader controls must be restored before acceptance. Keep annotation/name-query work separate from RECHK search normalization; no existing receipt is rewritten.

## GitHub checkpoint recovery (2026-10-24)
