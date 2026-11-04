// net: an intentionally empty placeholder for the LOCKED "LAN" feature
// (PLAN.md §LOCKED FEATURE SET: client-server UDP, cap 16, bots server-side;
// internet matchmaking is dropped). It is empty because the multiplayer phase
// has not started, NOT because it is dead code — the workspace member exists so
// the LAN work has a home. Kept deliberately by the 2026-09-08 cleanse
// (engine/artifacts/loop/CLEANSE_PLAN.md, Appendix 2).
//
// The determinism spine the LAN layer will need is already built and tested
// elsewhere: fixed 30 Hz ticks, `InputLog` per player slot, `Sim::hash`
// (PLAN.md §Determinism & capture/replay spine).
