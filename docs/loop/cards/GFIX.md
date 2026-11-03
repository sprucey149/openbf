---
id: GFIX
title: gates.py: golden_report skips the walkabout referee's non-image golden (it crashed every golden-moving gate run and landing since 61bfd0b)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

- **Why:** since 61bfd0b goldens.sh also regenerates goldens/walkabout_stats.ron; golden_report opens every moved golden as an image and dies with UnidentifiedImageError, so gates.py --auto and land.py phase 1 crash whenever a golden moves (found by WCOLB, 2026-09-25, lead_tmp/wcolb_gates.log).
- **Fix:** list non-.png moves as 'not an image' in QUANT.md and skip the sheet.
- **Acceptance:** every gate PASS, nothing declared.
