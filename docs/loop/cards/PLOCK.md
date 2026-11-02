---
id: PLOCK
title: parity.py takes gpu.lock EXCLUSIVE (`flock GPU_LOCK`, scripts/parity.py 170) where every other holder takes it shared since 2026-10-11 - a parity gate waits for every PCSX2 session to close (2026-10-28: 40 min behind a menu-reference session, with ci.lock held and every ci run queued behind it); take it shared
milestone: M0
value: 5
phase: build
status: done
deps:
files: scripts/parity.py
declares: no gate result moves (lock mode only)
needs:
effort:
branch:
resume:
created: 2026-10-28
---
## Evidence
- `scripts/gates.py` `sh()`: gpu.lock SHARED since 2026-10-11, measured (3 HW PCSX2 beside the render tests + goldens: 18 of
  18 suites same verdict, 73-76 s vs 63-65 s; PCSX2 99.9 % speed). `pcsx2_session.py` header: "Everyone takes gpu.lock SHARED".
- `scripts/parity.py:170`: `subprocess.run(["flock", GPU_LOCK, "bash", sh], ...)` - exclusive.
- 2026-10-28 09:30: `lslocks` showed PCSX2 (session `menuref_ia2`) holding gpu.lock READ, parity's render flock waiting
  WRITE for 32 min under a parity gate that held ci.lock; two ci.sh runs queued behind it.
