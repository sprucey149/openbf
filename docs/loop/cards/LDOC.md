---
id: LDOC
title: `loopctl commit-docs <paths>` commits a finished RE doc atomically under the MAIN claim (docs never sit dirty in main), and autoloop.sh journals a dead session (its phase and the claim it left)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX note 2026-09-29 22:40 (approved loop improvements), item 5. Uncommitted RE docs in main blocked another lane's landing: TALPH waited about 30 min (memory "RE drafts
off the main checkout"). A session that dies leaves no journal line saying where it stopped. deps LRUL only because
both edit loopctl.py.
