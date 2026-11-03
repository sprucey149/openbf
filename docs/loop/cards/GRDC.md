---
id: GRDC
title: claude_guard.sh's deny rules miss `git -C <dir> push|add -A|checkout main` (the merge rule already allows for -C); its header/messages cite the gone briefs/OPUS_LOOP_PROMPT.md and HANDOFF-OPUS.md
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Scripts cleanse 2026-09-29 (card CLNS). VERIFIED by piping hook JSON into scripts/claude_guard.sh: `git push origin main` and `git add -A` are denied, `git -C /x push origin main` and `git -C /x add -A` print nothing (allowed); with OPENBF_LOOP=1 `git checkout main` is denied, `git -C /x checkout main` is not. The ff-merge rule's regex already carries `git([[:space:]]+-[Cc][[:space:]]+[^[:space:]]+)*[[:space:]]+merge`.
Stale text: the header cites engine/artifacts/briefs/OPUS_LOOP_PROMPT.md §3 (gone; autoloop.sh uses briefs/LOOP_PROMPT.md) and HANDOFF-OPUS.md (gone from the repo root; also in LEADFILES and a deny message).

## Acceptance
The same -C group in the push, add and checkout/switch regexes; the doc lines repointed; the JSON pipe test above denies all six.

## Notes
Left to the maintainer: this is the hook that guards the sessions themselves, so a session does not edit it on its own say-so. The fix only tightens it.
