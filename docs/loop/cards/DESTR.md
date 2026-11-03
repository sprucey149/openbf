---
id: DESTR
title: Map objects that break or move when shot: BF2 DEA1's vent to the trash compactor and the panels that move floor panels, TAN1's turbine, and every other destructible or moving prop the PS2 missions name (census both games)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
INBOX 2026-10-21 20:00 item 3. FLNKB's "flank routes walk through the closed compactor" is the same object; PRIMB holds
DEA1's compactor prims.

## Acceptance
- A census (both discs): every class with destructible / animated-on-damage behaviour and the missions that place it,
  with the RE of DEA1's compactor chain and TAN1's turbine; a build spec on the card for the ones a player meets on a
  showcase map first.

## Cloud RE checkpoint (2026-10-24 12:27 UTC) — incomplete
- Primary-input inventory: 39 exact probes, 0 available; bounded checkout
  raw-input filename scan found 0 alternatives (rg exit 1). Filesystem
  availability only, not a PS2 behavior absence result. Both corpora/ELFs,
  full LVL/mission bytecode trees, named DEA1/TAN1 data and raw cited evidence
  are absent. No new PS2 [V]/[O] finding, class/mission census or build spec.
- Historical leads: re-time-limit.md:94-101 (TAN1 console timer, DEA1 lethal
  region cycle), re-build-zones.md:54-101 (building lifecycle), PRIMB
  (compactor prim alignment), FLNKB (blocked groups), TRAP (Endor trap).
  Reproduce them against restored primary evidence before using their values.
- Current source navigation and census/spec requirements:
  lead_tmp/destr_cloud/source_notes.md. Startup arc extraction omits merely
  registered callbacks; Hit::World lacks prop identity; rest_prims guards on
  door geometry. Its compactor-named assertion copies door prims, so it is
  not the real compactor fixture PRIMB requires. These are port-source reads.
- Independent review spawn failed at thread initialization; no agent ran.
  The primary-evidence verifier and five RE self-checks remain required.
- Original Acceptance retained. Restore inputs, census both discs and mission
  load sets, trace both chains, then specify the actual showcase placements
  first (DEA1/TAN1 are not showcase maps). No new cards or code edits.
- Report reports/20261024_1227_card_DESTR.md; exact continuation and input purposes
  in lead_tmp/destr_cloud/next_prompt.md and preflight.json.
