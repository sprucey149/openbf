---
id: STALL
title: Why BF2 KAM1, POL1, DEA1 and GEO1 never finish the round sweep, and the long stalls on BF1 KAS2 / NAB1 / BES2 and BF2 MUS1 — classify every stuck unit per map (no route / wall / vehicle wait / goal flip / enclosed post / rules stalemate), check the PS2's bots at the same spots, and fix each cause (the maintainer, INBOX 2026-09-30 08:50; JKAM merged here)
milestone: M3
value: 5
phase: re
status: done
deps:
files:
declares:
needs:
branch:
resume: closed: cause table analysis/re-round-stalls.md (7b8e3b0); build cards STTAP STDOR STFAL STLVL STARC STEND
created: 2026-09-30
alpha: yes
---
## Why
The maintainer (INBOX 2026-09-30 08:50): M3's definition of done is a full conquest round on every map ending by tickets, no unit
stuck, the sweep `finished 32/32`. The maintainer wants the real cause per map, AI logic or otherwise. Research first (lane 3 or 2).

## Evidence
- Seed-42 sweep at main after VTUR2 (`lead_tmp/loop_baselines/20260930_1203_VTUR2/sweep_main_1.txt`, 60,000 ticks):
  `finished 28/32, >10k 3 maps, sum 162,000`. NO OUTCOME: **BF2 KAM1** (stuck_max 27,840; tickets 15/9, alive 15/9),
  **BF2 POL1** (41,940; tickets 130/13, alive 16/13), **BF2 DEA1** (1,560; tickets 2/16, alive 2/16 — almost nobody
  alive and the round still does not end), **BF2 GEO1** (51,600; tickets 12/110; lost at VTUR2 by the hover-jitter
  re-seed, `reports/20260930_1215_resume_VTUR2.md`). The maintainer's 08:50 figures (KAAD baseline) had 29/32 and KAM1 17,220.
- Long stalls that still finish: BF1 KAS2 5,820, **BF1 NAB1 4,740 (showcase)**, BF1 BES2 3,480, BF2 MUS1 2,760.
- History: JWAL took POL1 and KAS2 out of their stalls (`>10k` 2 → 0, sum 72,600 → 45,360); WPSH, WTUN, KAAD put POL1
  back. WTUN (`reports/20260929_2337_card_WTUN.md`, `lead_tmp/wtun/reseed/`): seeds 1–8 — KAM1 NO OUTCOME 7/8 under the
  old rules, 8/8 under the new; stuck spot (−71, 50.5, 268.5) under a ramp underside (body 270) 0.72 m over the floor,
  a nav-arc pocket; MUS1 1 → 2 no-outcome seeds, POL1 6 → 8; the MUS1 seed-4 and POL1 seed-3 stuck poses touch NO body
  (navigation stalls). One seed is a noisy instrument.
- JKAM (merged): KAM1 unit 24 pinned at (108.04, 52.25, 149.85), push (−0.27, 0.10) every tick at hub 88; climb legs
  hubs 90→88, 53→66 under JWAL's ledge rule (`lead_tmp/jwal/u24.txt`).

## Outcome (lane 2, 2026-09-30 13:55–15:40, RE max)
The cause table is `analysis/re-round-stalls.md` (verifier 17/2/0 of 19, folded in). Measured on main 027dad9 over 9 seeds
× 32 maps, with a read-only stall-episode census (`lead_tmp/stall/`), and set beside live PCSX2 runs of BF2 KAM1, DEA1 and
POL1 (16 AI a side, 420 s each).

Seven port causes, all absent from the PS2 runs:
- **C1 falls** (KAM1 99 % of deaths; the PS2 1.6 %);
- **C2 the wrong floor of the XZ graph** (KAM1 rims, POL1 hangar ramps, TAT2, MUS1);
- **C3 DEA1's blast-door rig** (the leaves open inward; PS2 soldiers cross the centre 39 times);
- **C4 a single-shot weapon fires once per trigger hold** (the PS2 taps: BF2 FUN_002baf70 / FUN_002baa38, BF1
  FUN_0023cc20);
- **C5 doorways approached off the arc**;
- **C6 last legs into walls**;
- **C8 parked AT-STs at KAS2's spawn**.

Plus **C7**, the bleed clamps at alive = tickets, so any of the above ends a round in a stall. And **C9**, crowds that only
inflate `stuck_max`.

Build cards, one per cause, each with its acceptance:
- **STTAP** (5) — C4;
- **STDOR** (4) — C3;
- **STFAL** (5) — C1 + C8;
- **STLVL** (5) — C2;
- **STARC** (4) — C5 + C6;
- **STEND** (4) — C7 + C9.

The maintainer's acceptance (32/32, 7 of 8 seeds, `stuck_max` < 10k) is carried by those cards.
