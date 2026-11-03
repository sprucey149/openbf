---
id: JKAM
title: BF2 KAM1 sweep loses its outcome after JWAL: team 0 won at tick 23,349, now NO OUTCOME (tickets 23/4, stuck_max 8,580) — find which AI units still stall on the climb legs (hubs 90->88, 53->66) under JWAL's ledge rule and unstick them without reopening a leak
status: archived
---

Historical technical record. Recorded state before archival was `done`.

JWAL (09b8c12) took City Hall leaks 8,903 -> 0, KAS2 and POL1 out of their stalls (sweep >10k maps 2 -> 0, sum 72,600 -> 45,360) but BF2 KAM1 went from a win at 23,349 (stuck_max 1,080) to NO OUTCOME with stuck_max 8,580. Evidence: lead_tmp/jwal/land/branch/sweep_1.txt vs lead_tmp/loop_baselines/20260929_1406_WALK/sweep_main_1.txt; unit 24 trace lead_tmp/jwal/u24.txt (pinned at (108.04,52.25,149.85), push (-0.27,0.10) every tick, hub 88). Suspect [H]: steep mesh stairs whose riser two steps up sits inside the full capsule's reach, so the ledge rule pushes a bot that should climb. Acceptance: KAM1 reaches an outcome again, stuck_max <= ~1,500; City Hall sweep still 0 leaks; leak census new 0.
