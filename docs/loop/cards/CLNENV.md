---
id: CLNENV
title: Isolate cleanse deletion self-test from inherited repository selection
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / evidence
On bb974e5, `OPENBF_REPO="$PWD" bash scripts/cleanse_delete.sh --self-test`
exits 1: `livebr_target` appears in got but not want. The fixture creates the
livebr branch in its temporary repository, but the child loopctl target sweep
inherits OPENBF_REPO and reads another repository's branches. The same fixture
without that override is the control. No actual repository data was deleted.

## Acceptance
1. Exact parent fails with OPENBF_REPO selecting the real repository; control
   with it unset passes. Candidate self-test exits 0 with it unset, selecting
   the real repository, or selecting a nonexistent path. Every fixture
   assertion still executes, including deletion in the temporary repository.
2. Dropping the dry-run isolation reproduces the livebr_target failure;
   dropping deletion isolation fails the final assertions. Restore and rerun.
3. Shell syntax and existing source-tool/workflow/gate-boundary tests pass.
   Full project CI and applicable unchanged gates remain required; do not
   claim missing content/GPU validation from this fixture.
