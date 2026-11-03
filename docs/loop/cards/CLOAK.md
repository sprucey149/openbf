---
id: CLOAK
title: Bothan spies can cloak, reveal on damage and recover concealment
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
The Bothan cloak was dropped from all four inherited class records. Scope assigned by the maintainer on 2026-10-28; issue #48 has no ownership comments. Auto-turrets remain another agent's task. Shared file edits are isolated in a managed worktree and must be merged by feature.

## Evidence
analysis/re-cloaking.md; local PS2 disc properties and executable bodies, no live PS2 claim.

## Acceptance
Core behavior tests cover energy gating, delay, trigger edges, drain/recharge, reveal/recovery, switch/death cleanup, perception memory and deterministic hashes. The disc census activates all four inherited Bothan records. Original-game comparisons remain open as recorded in the report.

## Notes
Keep packs, buffs, placed explosives and droids. No hero, Force or portable-turret implementation.

2026-10-28 follow-up: maintainer remembered intermittent transparency, platform uncertain. Controlled live PCSX2 comparison establishes that the PS2 draws no body at concealment 0.4 or 1.0 and draws it fully at zero. Recorded native 5–10 s flickers and added regression for exact float/byte, recovery, moving-strength reveal and RNG consumption. No speculative transparency was added. See analysis/re-cloaking.md and the implementation report; broader acceptance stays open.
