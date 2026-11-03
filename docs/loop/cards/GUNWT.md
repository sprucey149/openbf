---
id: GUNWT
title: BF1's AI driver waits in its seat for a gunner and the AI that answer board (UTALK's behaviour half): on the PS2 an AI in a vehicle seat with a free gun runs UnitWaitSecondaryAgent and calls 'need gunner' (chance 0.3) each time it enters the agent's state 2, and every friendly AI on foot within 30 m answers yes and gets a board goal for that vehicle (goal 5) - TAT1's yes answers are 12 of the voice slot's 32 lines a minute; the port's pilot drives off at once and its answerers carry on (UTALK census: yes 0.97 / min, need-gunner gate 1.00 against 4.25)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card UTALK (2026-10-16) landed BF1's AI squad lines on the port's own seat event: a bot that takes a pilot seat with a free gun calls once (chance 0.3) and the AI on foot within 30 m answer yes. The PS2's rate needs the PS2's behaviour: TAT1 yes lines 12.0 / min (port 0.97), need-gunner gate re-arms 4.25 (port 1.00); NAB1 2.0 / 2.75 (port 0.84 / 1.50). Census: `lead_tmp/utalk/census/tab_utalk.txt` (run.sh + tab.py, 8 seeds x 240 s, player parked).

## Evidence
- `analysis/re-squad-orders.md` 1 (step 6: type 4 reaches every friendly AI on foot within 30 m, not droidekas), 4.4 (goal 5 = board the order's vehicle), 5.
- B1 `FUN_0024d020` (167011-167041) = UnitWaitSecondaryAgent's vtable slot +4 (ELF word 0x537308, vtable 0x537304, RTTI string before it): on entering state 2 it issues type 4 with chance 0.3. The agent's UPDATE (slot +8, 0x24d0e8) and BF2's twin (vtable 0x6239a8: 0x2cd6b0, 0x2cd6f8) are holes in both decomp exports: disassemble them (no capstone on this machine; `lead_tmp/re/*/mipsdis.py` needs it) for when state 2 is entered and how long the agent waits (re-soldier-ai 1.4: 'wait N ms').
- [L] re-soldier-ai 1.4: 'wait in a seat' is 3-5 % of AI rows (BF2).

## Acceptance
- RE: the wait agent's states and timer [V] with the verifier; who starts it (the board agent's seat taken? a vehicle with free gun seats?).
- Build: the port's AI pilot waits in its seat as the PS2's does, re-issuing the call; the answerers within 30 m claim the vehicle (the port's claim rules: one takes the gun seat, the rest lose the claim).
- Census on UTALK's kit: TAT1 yes lines and need-gunner gate passes within 30 % of 12.0 / 4.25; the sweep / fidelity gates' boarding counts sorted NOISE / EXPOSED / CAUSED.
## Notes
- 2026-10-16 planner (from UTALK's report): this RE pass also corrects `analysis/re-squad-orders.md` 5 / 9b - the follow line fires when the trooper "sets out for a post it is not at" (FUN_00235258 = inside the goal's radius, not "its first path leg fails"), and the lines are now built (UTALK, e22e702). The edit needs the MAIN claim (RULES: RE drafts off the main checkout).
