---
id: ORAUTH
title: Committed cards must not authorize their own oracle or fidelity changes
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why

Tracked cards let an ordinary worker publish the same oracle/fidelity declaration
that the gate subsequently treats as approval. RULES requires a separate
planner/the maintainer approval; committing a worker's own header does not provide it.

## Evidence

`scripts/oracle_guard.py::allowed_classes` and `card_declares_for` union committed
`declares: oracle=...` with approvals; `scripts/gates.py::card_fidelity_allow` reads
committed `declares: fidelity=...`. The comments still assume cards are untracked.
This is source/control-flow evidence, not a new runtime or PS2 audit verdict.

## Do

After explicit guards authorization, make guards/oracle and fidelity/exposes
authority come from the separate approvals channel. Preserve ordinary gameplay
move declarations and the intended scoped golden/bake/landing flows. Reconcile
existing guard fixtures that currently enshrine self-authorization; classify
historical guarded hunks independently instead of silently blessing them.

## Acceptance

- Isolated git fixture: a builder commits its own `oracle=guards`,
  `oracle=any` or `fidelity=metric@map` header with no approval; hook/protect and
  fidelity checks still refuse the corresponding protected change/regression.
  Test both explicit job card and branch-discovery paths, including a subsequent
  records commit; missing/malformed/mismatched approval remains refusal.
- An explicit exact-card approval grants only its named class/metric; another
  card/class/map remains denied. Test intended golden/rebake/move declarations
  and authorized planner/landing paths as independent controls.
- Restore each former header-based authority reader separately: its negative
  fixture must fail; restored tightened code and all existing guard/tool tests
  pass. No blanket authorization, weaker threshold, baseline/golden rewrite or
  automatic historical approval is part of this repair.
