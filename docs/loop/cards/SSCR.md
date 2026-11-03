---
id: SSCR
title: Splitscreen: local play on one machine — one sim, N cameras, N input devices, N viewports and HUDs, no netcode (not part of the multiplayer hold; before PACK; only after the alpha and M1-M3's open cards)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer's INBOX 2026-09-30: **splitscreen is NOT part of the multiplayer hold.** The hold is LAN / network / lockstep /
lobby only. Splitscreen comes BEFORE PACK, and the loop may work it out and build it without asking him — no `needs:`. (Superseded 2026-11-02: the scope is Instant Action, so this card now waits on the header's `needs:`.)
It is a much later task: start it only after the alpha cards and M1–M3's open cards are done (so value 1 +
`defer: post-alpha`: the picker takes it only when nothing above is eligible). PLAN §Milestones: M5.

## What it is
Local play for the maintainer and friends: ONE sim, N cameras, N input devices, N viewports, N HUDs. No netcode — the `net`
crate stays the stub it is; PACK's design note (room for netcode later) stands.

## Evidence / leads
- The PS2 ships it: BF2's INGAME.LVL carries `1playerhud` … `4playerhud` layouts (BHUD baked all five trees:
  `common_bf2/hud/`, BHUD b06686f). [H, not measured] how many players each PS2 game really splits for and how the
  screen is divided — a PCSX2 frame with two pads settles it (step 1). re-event-messages §8 lists the split-screen HUD
  layouts as unread.
- Per-viewer state the port already names: EVMSG's per-viewer team colour; BPNT lights posts "for the first player's
  team" (FEEL_NOTES) — a per-view colour is this card's; PADM / `input` maps one pad.
- PLAN §Platform & meta (host = P1 controls the session; cheats host-only).

## Do
1. A short design note in the card (not an RE): how many players (PS2: 2; the HUD data allows 4), viewport layout on
   16:9 (the PS2's own division from the PCSX2 frame; keep each view Hor+, no stretching), which pad joins how, the spawn screen
   per player, what a paused / dead player sees. Feel choices → FEEL_NOTES.
2. `Sim`: N human-controlled units (today one player slot); `input`: N devices; `render`: N view passes of the same
   scene (per-view camera, culling, post-relation colours); `ui`: the `<n>playerhud` layout per view.
3. Audio: one listener per view, mixed (the PS2's rule is unread — a bounded RE only if it changes what is heard).

## Acceptance
- `game --players 2` (two pads, or one pad + a scripted second input) on tat1: a golden `split2.png` with two viewports,
  each its own camera and HUD, nothing stretched (a circle test: a round object's bounding box is square ±2 px in both).
- A replay test: two input scripts drive two soldiers in ONE sim; the sim hash equals the same two scripts fed through
  the headless path (the views do not touch the sim).
- Frame time with 2 views on HOT1 ≤ 2 × the 1-view time + 10 % (`--bench`).
- No `net` crate change (`git diff --stat` shows none).

## Notes
- Planner 2026-09-30 21:45: filed as the maintainer asked. NO `needs:` line (then; see Why). XDET (cross-OS determinism, LAN) stays held.- From GLOW / BFLT (status_20260930_1330.md): the PS2 drops glow, bloom and the frame filter with two players (a fill-rate limit). Default for this card: keep them on in every view (a hardware limit we improve on); one line in FEEL_NOTES when built.
- 2026-10-13 06:42 (lane 2, card CPBAR's RE; re-capture-display.md 3 and 4): with more than one viewport the PS2 scales the capture icon's group by 0.75 about its anchor (B2:377740, B1:249192; read, not seen - one pad). The PS2's ELF registers HUD events for two players and builds two capture displays; BF2's 3 / 4-viewport layout (`4playerhud`) carries a ring the PS2 never feeds.
