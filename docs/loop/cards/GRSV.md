---
id: GRSV
title: Grass patches, the open [H]s: the GS blend / depth-write of the grasspatch sprites (port: source-over, no depth write) and the BF2 template twin (B2 FUN_003c6788's builder vs B1 FUN_001ae120) — a PCSX2 patched-state or GS read, then match
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

FOLGG landed the grass (c7bb5d7) with two PROVISIONAL calls: render::grass composites source-over with no depth write (re-foliage §7 [H]), and BF2 uses BF1's template/quad reading (its fields sit +0x20 later; the atlas and draw selection are checked, the builder is not). Also unread: the +0x58 = 10000 field. Pairs: engine/artifacts/lead_tmp/folgg/pairs/.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).

`acceptance: needed` (planner 2026-09-30): no measurable acceptance yet (a parity delta, a leak-census count, a sim replay hash or a pixel test). Write one before the card is picked.

## Note (cleanse CLNR, 2026-10-02)
The grass and leaf sway jump phase at the renderer's time wrap. `render::grass::sway` is `sin(phase + t * 0.8 * (1 + 0.2k))`
and `leaf::sway` `sin(phase + t * wind[0])`; the windowed game wraps `t` at `render::TIME_WRAP` = 2000 s
(`game/src/main/windowed.rs`, `r.time = (r.time + dt) % TIME_WRAP`), and 2000 s x 0.8 rad/s = 254.65 turns, so every blade
snaps in one frame once every 33 min of play. Read, not seen. The five grass rates are multiples of 0.16 rad/s: a sway clock
wrapped at a multiple of 2*pi/0.16 s would be seamless for the grass; a leaf class has its own rate. The docs now say so
(`grass::sway`, `leaf::sway`, `Renderer::time`); no behaviour was changed.
