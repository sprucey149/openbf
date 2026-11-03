---
id: DPIT
title: DEA1's pit falls are the PS2's own (3.0 / min, none under fire): its bots walk off the pit beside cp3's spawn node and run off the Tak bridge when its panel is down - the port has solid bridges and a floor probe, and 0 falls. Build the two script bridges, then decide the probe at the edges the PS2 leaves open
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT of STFAL (2026-10-01, PARTLY; `reports/20261001_1845_audit_STFAL.md`). STFAL's acceptance line 'DEA1's pit deaths
stay within +-50 % of the PS2's 3.0 / min' was not met (3.62 -> 0.54 / min; main: 0) and was handed to STDOR with the reason
'the PS2's falls come with its fighting'. The PS2's own samples say otherwise: of its 21 falls (420 wall seconds = 360
game seconds: one battle, a reload at 348-374 s, the start of a second), no faller lost health in the 6 s before leaving
the deck. They are route walk-offs, at four places:

| site | falls | what the PS2's bots do | the port (main 58fa0e3) |
|---|---|---|---|
| A (-125, 50, -238) and (-110..-115, 47, -235) | 12 | 11 spawn at cp3's node (-118.9, 52.0, -235.8) and walk into the pit 5-25 s later | same node, same pit (floor ends 3.4 m from the node, 11 m wide). The floor probe refuses the step |
| B (-170..-172.5, 52, -207..-196) | 6 | 4 run north off the deck's end at z -208 inside 13 s, 2 walk south into the same gap from z -196 | floor all the way: the bridge is solid (arc Connection128, hubs 82 - 61; Barrier222 at (-171.4, -202.7)) |
| C (-190.6, 48.8, -255.9) | 2 | step down 52 -> 50.8 and off | same edge |
| D (-149.3, 48.3, -99.2) | 1 | walk off | same edge |

Site B is a mission-script object [V: the PS2 bytecode of `dea1g_con` and `dea1c_con`, read with gen_missions.py's Lua 5.0
decoder; the arcs and barriers by name in the baked manifest]. ScriptPostLoad extends both bridges and registers
`OnObjectKillName(PlayAnimRetract, "Panel-Chasm")`, `OnObjectRespawnName(PlayAnimExtend, "Panel-Chasm")` and the same pair
for `Panel-Tak`. Two bridges:
- the chasm bridge: retract = PlayAnimation("bridgeclose"), BlockPlanningGraphArcs("Connection122") (hubs 20 - 73),
  EnableBarriers("BridgeBarrier") (at (-129.1, -67.6)); extend = "bridgeopen", unblock, disable;
- the Tak bridge: retract = PlayAnimation("TakBridgeOpen"), BlockPlanningGraphArcs("Connection128") (hubs 82 - 61),
  EnableBarriers("Barrier222") (at (-171.4, -202.7), 3.1 x 6.3 m); extend = "TakBridgeClose", unblock, disable.
The PS2's six fallers left the deck at z -207 to -205, a metre short of Barrier222's near edge. The port replays
ScriptPostLoad's barrier calls (BNDS) and none of the callbacks: both bridges are always solid.

The port's pit deaths before STFAL (3.62 / min) matched the PS2's rate by coincidence: 16 of 715 fall events were at
site A, none at B.

## What to do
1. RE (bounded): the two panels' classes (destructible, respawn time); what PlayAnimation moves (the bridge's collision
   with it?); what a bot already on the arc does at the enabled barrier. One PCSX2 check: destroy Panel-Tak, watch the
   bridge and a bot on it.
2. Build: the panels, the retract / extend with the collision, the arc block and the barrier.
3. Decide the floor probe (ours, STFAL) at A, C and D: the PS2's bots walk off there, the port's stand or turn. Default:
   keep the probe (bots do not walk into pits), logged in FEEL_NOTES.md as a call the maintainer may reverse.

## Evidence
- `lead_tmp/audit-stfal/README.md` (tables), `tools/ps2_falls.py` over `lead_tmp/stall/ps2/bf2_dea1_soldiers.csv`.
- Port ground lines and hubs: `AUDIT_LINE` / `AUDIT_NEAR` (`tools/add_line.py`).
- Port fall sites before / after STFAL: `tools/falls.py falls_before --map bf2_dea1 --sites`.

## Acceptance
- Test: with Panel-Tak destroyed the Tak bridge's collision is gone, Connection128 is blocked and Barrier222 is on;
  after the panel's respawn all three are back. The same for Panel-Chasm (Connection122, BridgeBarrier).
- `SWEEP_DEATHS` on bf2_dea1 over 8 seeds, after STDOR: pit deaths / min and their split by site, reported beside the
  PS2's (A 1.7, B 0.9, C 0.3, D 0.1 per wall minute; 3.4 per game minute in all in its first battle).
- The probe decision at A, C and D is one line in FEEL_NOTES.md.

## Notes
- DEA1 is not a showcase map: `defer: post-alpha` (the planner may lift it).
- Run scratch binaries under the memory cap (OFFG's note).
