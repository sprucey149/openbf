---
id: CFOV
title: The PS2 soldier camera's field of view: BF1 48.0° × 37.69°, BF2 65.0° × 52.06° (h × v), measured in RAM; the port's chase camera uses 70° vertical
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## The maintainer's decision (2026-09-25)
Use each game's PS2 VERTICAL FOV (BF1 37.69 deg, BF2 52.06 deg) as the default, widened horizontally for the window (Hor+, never stretched), and add an FOV option to the settings so a modern screen can open it up.

## Outcome (2026-09-28, lane 1)
LANDED cded403: `WalkSim::camera` takes `ps2_fov_y(source_game)` × the player's FOV option (pause menu, profile
`fov_scale`, 0.75–2.0, capped at 100°); Hor+ unchanged. No committed golden frames through `WalkSim::camera` (the goldens
use GOLDEN_CAM / overview / gallery cameras), so nothing moved; sim/simhash/sweep/arenas PASS. Pairs: BES1 and KAS2 spawn
pairs line up; 213/214 PCSX2 states carry the exact value (outlier → CFOVM). Report `reports/20260928_1435_card_CFOV.md`.

## Why
Every port frame on every map is framed at 70° vertical; the PS2's soldier camera is 37.69° (BF1) / 52.06° (BF2)
vertical. It is the widest-reaching look gap left (every PCSX2 pair is misframed by it) and it sets feel.

## Evidence
PS2 RAM on every state in `pcsx2/ref/states/` (third person, standing, zoom 1): BF1 cam+0x88/+0x8c = 0.44523/0.34131,
BF2 cam+0x138/+0x13c = 0.63707/0.48838 (FINDINGS F5; offsets named by `analysis/re-sky-pass.md` §2/§4). The port lines
up with the PS2 frames only at those values (`lead_tmp/re/pxc/pose/p_s1_fov.png`).

## Original QUEUE.md entry (verbatim, 2026-09-25)
CFOV. [TODO — **[2026-09-25 the maintainer] SCOPE: this is a modern PC port. The measured PS2 FOV (BF1 48.0° × 37.69°, BF2 65.0° × 52.06°, 4:3) gives the intended VERTICAL FOV/framing; the port must keep that vertical FOV and widen the horizontal FOV with the window's aspect ratio (Hor+), fitting any resolution/aspect without stretching or 4:3 letterboxing — never hard-code the 4:3 horizontal FOV. See `engine/artifacts/pcsx2/ref/COMPARISON_RULES.md`.**] — ADDED 2026-09-25 (lane 1) by PXC batch 1 (`engine/artifacts/pcsx2/ref/FINDINGS.md` F5); **[RE] FIRST** (job D, at max), then a build [GOLDENS]; the RE is FREE (disc + `analysis/`); the build touches `game/src/walk.rs` (and a comment in `render/src/camera.rs`) — shared with no queued entry today] **The PS2 soldier camera's field of view: BF1 48.0° × 37.69°, BF2 65.0° × 52.06° (h × v), measured in RAM; the port's chase camera uses 70° vertical** [RE] — measured: `engine/artifacts/pcsx2/pose.py` on every state in `engine/artifacts/pcsx2/ref/states/` (third person, standing, zoom S = 1): BF1 cam+0x88 / +0x8c = 0.44523 / 0.34131 (near 0.63), BF2 cam+0x138 / +0x13c = 0.63707 / 0.48838 (near 0.5) — the offsets `analysis/re-sky-pass.md` §2 (BF2 `FUN_0057b8b0`: cam+0x138 = tan(fov·0.5), +0x13c = that · aspect) and §4 (BF1 `FUN_00491008` / `FUN_00490268`) name; the port renders line up with the PS2 frames only at those values (`engine/artifacts/lead_tmp/re/pxc/pose/p_s1_fov.png`: PS2 | 37.7° | 70°). The port: `WalkSim::camera` (`game/src/walk.rs`) takes `Camera::default()`'s `fov_y` = 70° (`render/src/camera.rs:36`), the placeholder PLAN §Presentation says the traced value "drops into when camera RE lands". Leads from PXC's verifier (2026-09-25, all eight citations SUPPORTED): BF2 `FUN_0052a968` is fdlibm `tanf` (SWBFRNT2 657906; `FUN_0052a308` is `atanf`); the aspect argument is `… one PCSX2 pair per game at a spawn pose lines up (`pcsx2/pairs.py`). Files: RE — `analysis/re-camera-fov.md`, `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/camera-fov/`; build — `game/src/walk.rs`, `render/src/camera.rs`.
