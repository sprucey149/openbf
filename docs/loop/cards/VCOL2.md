---
id: VCOL2
title: The leak census's residue after VCOL's ceiling stop: a GROUNDED head into a low overhang (step-up or slope under a low ceiling, a head walked sideways into a sloped underside: 2,862 walk/sprint ceiling runs) and the floor class (5,338: a steep upward face, n.y 0.5-0.73, crossed on a tick the wall pass pushes 0.17-0.6 m) - name the wall-pass / ground-snap rule that lets each through, fix, drive both down
status: archived
issue: 98
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #98](https://github.com/sprucey149/openbf/issues/98).

## Why
VCOL (branch vcol b3da13b) stops a RISING soldier under a face: leak census ceiling 45,309 -> 8,750. What it does not answer (traced, engine/artifacts/lead_tmp/vcol/v_b3.err and LEAK_TRACE runs in the VCOL report):
- ceiling, grounded: bf1_yav2 (0.07,160.02) jump t69: the ground query lifts the feet 0.39 m onto a step and the head crosses a horizontal face; bf2_hot1 (-210.43,188.25) t6: walking up a slope (vel.y 1.86) under a horizontal face; bf2_myg1 (-54.97,41.33) walk t28 and bf2_hot1 (-422.01,-134.47) t36: a head walked sideways into a sloped underside (n.y -0.81/-0.83), push 0.08 m (the wall pass takes a ceiling's horizontal part only, card WTUN).
- ceiling, airborne: bf1_nab1 (46.09,-9.11) jump t77 (vel.y already 0, a lower axis point at a face edge, n (0.39,-0.78,-0.49)); bf2_hot1 (-449.03,-133.99) t81.
- floor: bf1_kas2 (192.21,146.91) spam t126 push 0.39; bf1_nab1 (-89.14,77.39) jump t62 push 0.60; bf2_hot1 (-434.01,-128.10) walk t40 push 0.49; bf2_hot1 (-463.12,-136.43) walk t19 push 0.17: each a face with n.y 0.51-0.73 crossed on the tick the wall pass pushed.
## Acceptance
- leak census ceiling (walk + sprint modes) and floor counts down from VCOL's landed column (engine/artifacts/lead_tmp/vcol/leak_b3.txt, v_b3.err) to an explained residue; a pinned test per fixed mechanism; declare leak/sweep/arena/sim.
