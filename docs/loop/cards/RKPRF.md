---
id: RKPRF
title: Immediate rocket safety: regression tests for the non-charge press and human exemption
milestone: M0
value: 4
phase: cleanup
status: done
deps: RKSLF
files: engine/crates/core/src/sim/step/tests.rs, engine/CRATES.md
declares:
needs:
effort:
branch:
resume: Source 95d0807 on main; core863/game367/fmt/Clippy pass, both independent mutants862pass/1intended failure, production bytes restored. Pending full CI/cleanup gates; Acceptance unchanged. reports/20261025_0923_card_RKPRF.md
created: 2026-10-25
alpha: yes
tries: 1
---

## Why
RKSLA's immediate-shot and human-exemption safety cases have no effective core
regression referee. The runtime guard already handles both on current main;
this card adds tests for that existing behavior, with no runtime change.

## Evidence
RKSLA item1 and its first Acceptance bullet name the missing press-tick and
AI-only cases. RKSLF owns the deliberate own-side blast-safety behavior;
`core/src/sim/step.rs` applies it before `WeaponState` fires. The existing
`a_bot_fires_no_rocket_into_the_wall_before_it` scene covers a latched salvo
and a charge release. Immediate non-charge fire and a human's own trigger
must be tested independently of those paths.

## Acceptance
- On current main, an AI at the near wall produces zero shots, zero health
  loss and a positive blast-hold count on the immediate firing step. The
  otherwise identical open-air control produces one shot and zero holds.
  A human's fresh press produces one shot and zero holds in both scenes.
  Check ammunition and event ownership, not only a total shot count.
- Fixtures exercise actual `Sim::step` and weapon-cycle output, with no
  injected Shot/Impact/Kill events or forced safety result. Document any
  initialized perception/tap state; prove the AI reaches the fresh-press
  branch and the weapon has no pre-latched salvo or charge.
- Two independent production mutants: omit only the immediate non-charge
  press from the release guard; apply that guard to human controllers too.
  Each must compile, pass the old latched/charge scene, and fail the specific
  new assertion. Restore production bytes after each. The unmodified
  fixtures pass. No mutation of an oracle, threshold or expected value.
- Full core/game libraries, fmt, workspace all-target Clippy and unchanged
  normal cleanup gates pass, with executed test counts and exact restoration
  proof. Runtime production prefix is byte-identical; existing default sim
  and trace pins stay unchanged. Missing full gates leave the card incomplete.
