---
id: AUDPRE
title: An incomplete audit remembers its missing inputs and retries after restoration, while remaining unaudited
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Two fresh audit sessions selected re-seat-hud.md on unchanged main 3d487d9 and
stopped at the same missing primary inputs. Their checkpoints remain incomplete,
so the target correctly stays unaudited; the briefing has no prerequisite hold
and lists it again. Preserve the audit debt while avoiding identical empty retries.

## Evidence
- reports/20261024_0729_audit_seat_hud.md and reports/20261024_0800_audit_seat_hud.md:
  both corpora, ELFs, VSHUD/BHUD/VRIDE fixtures and approved rendering inputs missing;
  neither writes a verdict to audits.md. The later report supplies RULE-PROPOSAL.
- scripts/loopctl.py audit_sources/audit_candidates/audit_briefing use the audit
  ledger and risk/sample order. An unfinished audit is not in that ledger.
- This is tooling state, not evidence about the original game's behaviour.

## Do
1. Add a small explicit checkpoint operation for an incomplete target, recording
   target, exact missing prerequisites, source revision relevant to the target,
   reason and report path under the shared loop directory. Do not write a verdict,
   close the owning gameplay card, reset counters or alter the audit ledger.
2. An unchanged blocked target remains visible with its reason as a skip row in
   the audit briefing; the first ready candidate keeps the existing risk/sample
   order. Retry when all recorded prerequisites are present or an explicit reviewed
   reset clears the hold. A new revision alone does not restore missing inputs.
3. Show holds in planner/orient output with the exact restore/reset step. Handle
   all-held candidates explicitly: report incomplete coverage and actionable
   prerequisites, without fabricating an audit or repeatedly assigning one target.
4. Keep this separate from RETRYCAP's card-attempt counters and PKHD's file claims.
   Start an isolated loopctl worktree before edits. Use fixture directories only;
   no production next/claim/state mutation while validating the selection logic.

## Acceptance
- loopctl self-test fixtures: unchanged missing inputs skip the target and select
  the next ready candidate; restoring only one of two missing inputs still skips;
  restoring both makes it eligible; reviewed reset removes only the named hold.
- With every target held the briefing names every blocker and no row is a pick;
  the fixture supervisor/worker path reports the hold rather than claiming a
  successful audit. Existing risk order and the render sample remain unchanged.
- Mutation controls: ignore the hold -> repeated-target fixture fails; retry when
  only one prerequisite returns -> partial-restoration fixture fails; mark the
  target audited on checkpoint -> exact audits.md byte comparison fails.
- A real-queue read-only before/after briefing using the two named report paths
  retains re-seat-hud.md as UNAUDITED with the restore reason. No fictional ready
  candidate may be inferred from an untested prerequisite set.
- loopctl self-test/check and all applicable CI/landing gates pass. Gate code,
  thresholds, GPU requirements and audit verdict standards remain unchanged.

## Notes
No implementation in the filing session. Fixtures can be developed without the
missing game data; full landing is still subject to existing gates. Any required
worker-prompt change is proposed to the planner, not an edit of the running launcher.

## GitHub checkpoint recovery (2026-10-24)
