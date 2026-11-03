---
id: WPSR
title: The LEAK census's push residue after WPSH (1,212 runs of 1.83M, 32 maps): name each mechanism — sampled so far: a head grazing an eave's lower edge (BF1 KAS2 City Hall, tri 74 of body 100, 0.42 m under it), a push while stepping onto a sloped face (BF2 DAG1 body 13, n.y 0.36), a far-side push from a sliver triangle the prism test calls 'beside' (the cp5 hut, tri 16 via the ledge path, 0.79 m)
status: archived
issue: 98
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #98](https://github.com/sprucey149/openbf/issues/98).

## Why
WPSH (sub-steps + no far-side push, and the census's in-contact start skip) took the census's push class from 48,415 to
1,212 runs. The rest is not one mechanism. Three samples, each traced with lead_tmp/wpsh/tools/wpsh_probe.rs (a scratch
example; needs parry3d as a game dev-dependency):
- KAS2 (114.47, -121.88) sjump tick 57: the capsule's top sphere grazes the lower edge of an overhanging face
  (tri 74 of body 100, face n (-0.98,-0.15,0.13)); the edge contact pushes 0.42 m under the eave. Likely VCOL's
  (heads through eaves) — lead_tmp/wpsh/kas2_ch_contacts.txt.
- DAG1 (-47.38, 46.28) walk tick 66: 0.5 m push while stepping up onto a sloped face (n.y 0.36) — not traced.
- KAS2 cp5 hut (5.61, 2.85) sjump tick 14: tri 13 (floor-like, n.y 0.53) sends body 386 to the ledge path, whose
  wall answer is tri 16, a sliver; pushes_past's prism test puts the start just outside it, so the far-side push
  stands (0.79 m into the hut) — lead_tmp/wpsh/hut2_contacts.txt. A prism margin fixes it but traps MYG1 soldiers in
  crevices (census float 47 -> 150 at 0.25 m and at 0.5 m).
## Acceptance
Classify the residue by mechanism (counts per class), fix what is the wall pass's own, and move what is VCOL's /
WTUN's to those cards; the census accepted line '* push' re-points accordingly.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — no soldier out of bounds. Value 3 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## AUDIT note (2026-09-30, LEAK → LEAK3)
The push residue is counted by a sampler that reaches only part of the walls.
- With samples at body height (the audit's band sampler, `lead_tmp/audit-leak/`), push has 112 more sites / 396 runs
  where the census never goes.
- Examples: BF1 RHN1's four fall-through-a-face sites at x −114/−82/−18/−10, z −30 (a 15 m slide down a wall, then a
  0.55 m push through a face on tick 64); BF2 NAB2 body 482.
- Re-count with LEAK3's sampler before calling the residue explained.
