---
id: FIDB
title: land.py re-seeds the fidelity baseline when a landing makes a value BETTER past tolerance (split from FIDA: the planner's 2026-10-12 19:30 second ask) - today a gain leaves the old, lower baseline and the next card can give it back unseen (CLSE2B passed at 11.46 % contact20 while main stood at 12.73)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
`land.py` re-seeds [fidelity] only for a declared or approved move. A landing that makes a value better leaves the old baseline;
the next card may give the gain back unseen (CLSE2B, 2026-10-12 19:05: contact20 11.46 % passed against a stale 11.14 while main
stood at 12.73). The planner re-takes the baseline by hand whenever a report lists "better" rows (RULES, Landing).

## Evidence
- FIDA's card, section "Planner 2026-10-12 19:30" (engine/artifacts/loop/cards/FIDA.md).

## Acceptance
- At `--commit`, when the landing's own fidelity rows list values better past tolerance, those metrics are written as the
  baseline (seeds 1-8 from the landing's run; the confirm block from a `--fidelity-both` run of main, ~5 min) and `source`
  names the landing. A self-test on a fixture gates.toml: a better row is re-seeded, a worse-but-allowed row is unchanged as
  today, the head above the fidelity marker is byte-identical.

## Notes
- land.py / gates.toml are protected: needs [approvals.FIDB] oracle = "guards" (the maintainer's yes; FIDA's was for gates.py's
  confirm rule). Filed 2026-10-16 12:22 by the FIDA build session.
