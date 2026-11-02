# A5 — PS2 Sim Tick Rate & Frame Pacing Audit

**Scope:** SWBF1 (`SWBFRONT.ELF-decomp.c`, NTSC-U, internal name `frontline.elf`) and
SWBF2 (`SWBFRNT2.ELF-decomp.c`, internal name `Battlefront2.elf`).
All addresses are the Ghidra `FUN_00xxxxxx` names in the decomp files; line numbers refer to those files.

## Conclusion (both games — same engine, same timing architecture)

| Question | Answer |
|---|---|
| Nominal render rate | **29.97 fps** (NTSC): main loop blocks until **2 vblanks** have elapsed (swap interval global, default `2`) |
| Sim dt model | **Variable, measured dt** from the EE COP0 `Count` cycle counter — *not* a fixed step, *not* tick-skipping |
| Sim steps per frame | **Exactly one** game `Update(gameDt, realDt)` per render frame; no accumulator/substep loop at top level |
| dt on a dropped frame | dt simply grows (3/60, 4/60 … of a second, measured in cycles). Game stays real-time speed with a coarser integration step |
| dt clamps | Hitch guard: raw delta > ~0.5 s → substitute ~62.5 ms. Final float dt clamped to **0.1 s** (i.e., below ~10 fps the game goes slow-motion) |
| Timescale / pause | Per-clock float timescale (slow-mo capable); pause freezes the *game* clock only — hence the two dt args (game dt, real dt) |
| Deterministic mode | Shipping binaries contain a record/replay verification harness; in that mode time advances in **whole vblank counts** (quantized), not measured cycles |
| Fixed-rate mode | BF2 (and BF1 `/framerate` string) has a **dev command-line option** forcing dt = 1/N s (default 30.0 fps) — not active in retail play |

**Confidence: HIGH** for the render pacing (2-vblank wait) and the variable measured-dt model in both games — the full chain (vblank INTC handler → main loop → vsync wait → timer → Update virtual call) was traced end-to-end in both binaries with matching structure. **MEDIUM** on subsystem-internal behavior below the Update virtual (not audited; see "Remaining ambiguities").

---

## BF1 (SWBFRONT.ELF) evidence chain

| Addr / global | Finding |
|---|---|
| `FUN_00341978` (main, line 329408) | `AddIntcHandler(2, 0x341ca0)` and `AddIntcHandler(3, 0x341ca0)` — INTC causes 2/3 = VBLANK start/end. Main loop: *(decompiler detail omitted)*. Default argv0 `"frontline.elf"`. |
| `FUN_00341ca0` (line 329525) | VBlank interrupt handler (`SYNC(0); EI();`). On cause 2 (vblank start, 59.94 Hz): `DAT_005190c4++` (frame counter), `DAT_005190cc++` (audio-tick counter). |
| `FUN_00341e70` (line 329591) | **VSync wait**: `while (DAT_005190c4 - DAT_005190c8 < DAT_005190c0) SleepThread();` — blocks until `DAT_005190c0` vblanks elapsed. |
| `DAT_005190c0` | Swap interval. Set to **2** in `FUN_002fdd40` (line 286937) and `FUN_002fdf70` (line 286965, replay teardown). Set to 1 or a config value (`DAT_005e9aa0`) only under the record/replay flags (`DAT_00518be4/be8`); set to 0 temporarily around line 253563 (loading). Retail gameplay = 2 → **29.97 fps cap**. |
| `FUN_00173778` (line 15904) | Per-frame driver: audio tick (`FUN_00171b48`) → vsync wait (`FUN_00341e70`) → timer (`FUN_00173190`) → `(**(vtable+0x10))(DAT_0056dd30, DAT_0056dd34)` = GameState::Update(gameDt, realDt). Wrapped in record/verify markers (`'d' 'c' 'e' 'a'`). |
| `FUN_00173190` (line 15708) | **The timer.** Normal mode: extends COP0 `Count` to 64-bit (`DAT_00521c80` high word, wrap check `Count < DAT_00564478`); raw cycle delta *(decompiler detail omitted)*; hitch guard: if delta > `DAT_0056dd20` use `DAT_0056dd28`; advances two clocks: `0x56dcc8` (game — skipped when pause flag `DAT_0051908c` set) and `0x56dce0` (real). dt floats: `(clockDelta * 0x2000000) >> 32` (= /128) `* DAT_0056447c`. Final clamp: `if (0.1f < dt) dt = 0.1f;` (0.5f in replay mode). Replay mode instead advances clocks by **vblank counts** (`DAT_005190c4` delta via `FUN_00442f38`). |
| `DAT_00521c88 = 0x232800` (`FUN_00442df8`, line 502899) | Clock frequency 2,304,000 Hz = **294,912,000 / 128** — exact EE clock. `DAT_0056447c = 1/2304000` (`FUN_00442e50`, line 502927). So dt = cycles / 294,912,000 s, exact. |
| `DAT_00564450 = 300000000` (`FUN_00442d70`, line 502862) | Rounded 300 MHz used only for the hitch-guard thresholds: `DAT_0056dd20 = >>1` (~0.5 s), `DAT_0056dd28 = >>4` (~62.5 ms) (`FUN_001730c8`, lines 15685–15686). |
| `FUN_00442fe8` (line 502997) | Clock advance: clock struct = {u64 ticks, u64 ticksPerVblank, float **timescale** @+0x10}. Fast path if scale == 1.0, else delta × scale. Slow-motion is applied at the clock, upstream of dt. |
| `FUN_00171b48` (line 14932) | **Audio pacing**: runs only when audio vblank counter `DAT_005190cc >= 2` (then resets to 0); dt = `vblanks * 0.016666668` (n/60); feeds `FUN_0035f8f0`/`FUN_003560e8` (sound/music update cluster). Audio quantizes to whole vblanks; the sim does not. |
| `0.033333335` mass hits (e.g. lines 67971–146562) | `(float)(ushort frames) * 1/30` — **animation data authored at 30 fps** converted to seconds. Not a sim step. |
| Line 280194 | `DAT_00518b1c += DAT_00518b20 * 0.033333335` inside VU (`lqc2/vsub`) render code — a per-frame effect phase that *assumes* 30 fps. Cosmetic only. |

## BF2 (SWBFRNT2.ELF) evidence chain — same architecture

| Addr / global | Finding |
|---|---|
| main (line ~483430) | `AddIntcHandler(2/3, 0x427cd0)`; loop *(decompiler detail omitted)*; default argv0 `"Battlefront2.elf"`. |
| `FUN_00427cd0` (line 483616) | VBlank handler: cause 2 → `DAT_00600b68++` (frame), `DAT_00600b70++` (audio). |
| `FUN_00427e68` (line 483679) | VSync wait: sleep until `DAT_00600b68 - DAT_00600b6c >= DAT_00600b64`. |
| `DAT_00600b64` | Swap interval. Default **2** (`FUN_003d5a90` line 432602; replay teardown `FUN_003d5cc0` line 432635). 0 during loading (line 382469); 1 or config `DAT_006d8660` only under record/replay flags (`DAT_00600548/549`). |
| `FUN_00174a70` (line 16699) | Frame driver: `FUN_00172010` (audio tick — identical vblank≥2, dt=n/60 pattern, line 15144) → `FUN_00427e68` (vsync wait) → `FUN_00174510` (timer) → `Update(DAT_0066af14, DAT_0066af18)`. |
| `FUN_00174510` (line 16526) | Timer, three modes: (a) replay → advance clocks by whole vblank counts; (b) **fixed-rate dev mode** if `DAT_006007e0 > 0` → dt = `1/DAT_006007e0` (falls back to `30.0`), optional scale `DAT_005f9078`, plus frame-step counters (`FUN_00426e30/e40`); (c) **normal** → 64-bit-extended `Count` delta, hitch guard (`DAT_0066af00`/`DAT_0066af08`), advance game clock `0x66ae90` (skipped when pause `DAT_00600b44`) and real clock `0x66aea8`. dt = `(clockDelta >> 15) * DAT_00661318`, clamp **0.1 s** (1.0 s replay). |
| `DAT_00661308 = 0x11940000` (line 667319) | = **294,912,000** — exact EE clock. `DAT_00661318 = 1/(freq>>15)` = 1/9000 s per tick (line 667388). Hitch guard: `>>1` = 0.5 s exact, `>>4` = 62.5 ms exact (lines 16518–16519). |
| `DAT_006007e0` setter (line 14758) | Set from a hashed **command-line option** (value parsed as float) — the fixed-rate mode is a dev switch (BF1's strings list `/framerate`, `debugmenu.framerate`). Not retail behavior. |
| `FUN_00538638/690/600` (line ~695k) | Clock advance trio identical to BF1: per-clock float timescale @+0x10; seconds-, cycles-, and vblank-count-based advancement. |

## Cross-check vs. community knowledge

Community consensus (30 fps target on PS2, with slowdown in heavy scenes; PC versions uncapped/variable) matches the code exactly: 2-vblank swap interval; and because dt is *measured* and clamped at 0.1 s, heavy scenes first take bigger steps at real-time speed (33→100 ms), then go slow-motion beyond ~10 fps. The decomp is the authority here and is unambiguous.

## Remaining ambiguities (would need the PCSX2 trace rig)

1. **Sub-Update stepping.** The Update virtual's callees (physics, vehicles, projectiles) were not audited for internal substepping or per-subsystem dt clamps. A PCSX2 trace logging dt at the Update call vs. inside the physics integrator would settle whether any subsystem subdivides large dt.
2. **In-game dt distribution.** Whether retail heavy scenes actually sit at 2 vblanks or oscillate 2↔3↔4 (and how often the 0.1 s clamp engages) is empirical — trace `DAT_0056dd30`/`DAT_0066af14` per frame.
3. **Swap interval = 1 paths.** The interval-1 assignments are gated by the record/replay flags; whether any retail-reachable mode (progressive scan, PAL60) ever runs interval 1 is unverified.
4. **Which dt arg is which.** "gameDt = first arg (pause-gated clock), realDt = second" is inferred from the pause-flag gating; a trace with the game paused would confirm in one frame.

## Recommendation for the Rust engine (determinism spine)

- **Fixed tick = 30 Hz, dt = 1/30 s exactly.** The PS2 game is variable-dt, but in the steady state it delivers one Update per 2 vblanks — dt ≈ 2/59.94 = 33.367 ms with sub-ms jitter. All gameplay tuning (and the entire animation corpus, authored at 1/30 per frame) targets that operating point. A fixed 1/30 step reproduces the tuned case and is 0.1% faster than real NTSC hardware — imperceptible, and cleaner than 0.0333667 s for determinism.
- **Do not scale dt on drops.** To match PS2 *feel* under load you would take one bigger step, but that breaks fixed-step determinism; ticking N times per rendered frame (standard accumulator) is the right engineering choice and only diverges from PS2 behavior in scenes that were already degraded on hardware. If feel-matching degraded scenes ever matters, emulate it at the *render* layer (drop render frames, keep sim at 30 Hz) — never by scaling dt.
- **Clamp accumulated frame time at 0.1 s** (the game's own clamp): under extreme stalls the PS2 game goes slow-motion rather than spiraling; capping the accumulator at 3 ticks reproduces that and prevents the spiral of death.
- **Model time as the engine does:** integer tick counters per clock with a per-clock timescale multiplier and a pause that freezes only the game clock; pass (gameDt, realDt) so UI/menus keep animating while paused — this mirrors the dual-clock design at `0x56dcc8/0x56dce0` (BF1) and `0x66ae90/0x66aea8` (BF2).
- **Precedent for the determinism spine:** both shipping binaries contain a record/replay harness that switches the time source to whole-vblank quantization and byte-verifies recorded state (the `FUN_00442ad8`/`FUN_00444058` pattern saturating BF1). Pandemic themselves made time deterministic to get replays — a fixed-step Rust sim is in-spirit, not a departure.
