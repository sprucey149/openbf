# PERF-2 result: DTEAM rows held for the read-only AI passes (2026-11-27)

## What changed

- `Sim::team_rows_cache: Option<TeamRowsCache>` (the humans per team and the two DTEAM rows),
  not hashed, not in the static digest. `Sim::with_team_rows_cached(&mut self, pass)` fills it,
  runs `pass(&Sim)` and clears it. While it is set, `humans_per_team` and `team_rows` return
  the held values. In debug builds each held read asserts it equals a fresh count.
  `step_with` also clears it on entry, so a pass that panicked can never leave stale rows.
- It is held for two passes:
  - the thinks' `think_pool::run` (step.rs), the pass the plan named;
  - the hit-token deal (`ai::tokn::deal`). This pass was not in the plan, but the per-thread
    profile names it (below). Its body moved into `dealt(&Sim, tokens) -> tokens`. The deal
    already ran with `sim.tokens` taken out, so it reads the same sim as before. With 4 seats
    it calls `governed` once per human per unit, about 2,100 times a tick on kas2 XL, and each
    call scanned every unit.
- Every other caller computes as before: there is no whole-tick cache and no `ai_tier` cache.
- `AUTO_CAP` stays 4. PERF-0b's probe measured 6 threads at 0.39 ms and 8 at 0.50 ms saved per
  tick, under the 1 ms bar.

## Tests

- `scripts/one_player_gate.py`: every row matches (lan_soak ok, 1559 ticks, 4312 hash checks).
- Filtered core lib tests in a debug build, so the held-read asserts were on: `think_pool`
  (it steps scenes on 1..n threads), `team_rows`, `humans_per_team`, `human_proxy`,
  `netplay_round`, `space_round_keeps`, `team_tier`, `proxy::`, `tokn`, `dfocus`. 60 passed.
  This includes the new `team_rows_held_for_a_pass_match_a_fresh_read`.
- `sim::state::tests::a_human_surplus_lowers_its_own_teams_bots_at_their_next_spawn` fails at
  "2v1 is 1v0". It fails the same way on main db4a2d1f1, and it reads no held rows. It is not
  this packet's.
- Hashes, before (main, 4 threads) against after at 1, 4 and 8 think threads, 32 rows, 1152
  hashes, 0 differ:
  - kas2 XL and hot1 XL at Team Size 200%, 4 AI seats, elite: seeds 1-8 each, 1200 ticks,
    checkpoints every 30 ticks;
  - bf2_hot1 Hunt and bf2_mus1, 200%: seeds 1-4 each, 1500 ticks;
  - LAN with a proxy (`round_to_outcome`, `SWEEP_NETPLAY=1 SWEEP_PROXY=0:good`): bf2_cor1 and
    bf2_kas2, seeds 1-4, 3000 ticks, checkpoints every 300 ticks.
  - The littlemallard rows below also agree at every common checkpoint (59 per pipelined row;
    110 and 120 in the sim-only rows).

## Measurements

littlemallard, headless, ES-DE idle, nothing on the TV, /tmp/openbf-bench only (deleted
afterwards). Setup: 3840x2160, elite, seed 7, Team Size 200%, 4 think threads. The before and
after binaries ran alternately in one session. The pipelined rows have 4 views, split UI and AI
seats, and run 60 s after 1500 (kas2) or 1800 (hot1) warm-up ticks.

| row | before | after |
|---|---|---|
| kas2 XL sim only, main-thread tick wall p50/p95 (ticks 1500-3300) | 6.83 / 8.64 ms | 5.49 / 7.11 ms |
| kas2 XL sim only, main-thread instructions per tick p50 | 68.0 M | 51.9 M |
| hot1 XL sim only, tick wall p50/p95 (ticks 1800-3600) | 4.02 / 5.18 ms | 3.28 / 4.22 ms |
| kas2 XL 4v pipelined, tick (step_world) p50/p95 | 10.26 / 11.82 ms | 9.07 / 10.51 ms |
| kas2 XL 4v pipelined, frame p50/p95/p99 | 31.2 / 35.1 / 36.8 | 30.2 / 33.9 / 35.3 |
| same, repeat (after first) | 31.3 / 35.2 / 37.0 | 30.3 / 34.1 / 35.6 |
| kas2 XL frames within 33.3 ms (run / repeat) | 81% / 81% | 92% / 90% |
| kas2 XL two-tick frames (run / repeat) | 9 / 11 | 1 / 2 |
| kas2 XL main CPU p50/p95; draw p50 | 30.2 / 34.1; 20.2 | 29.2 / 32.9; 20.6 |
| hot1 XL 4v pipelined, tick p50/p95 | 6.62 / 7.93 ms | 5.99 / 7.49 ms |
| hot1 XL 4v pipelined, frame p50/p95/p99 | 24.4 / 28.8 / 30.4 | 23.4 / 27.8 / 29.4 |
| hot1 XL frames over 20 ms | 76% | 74% |

A first build held the rows for the thinks only. It gave kas2 XL sim-only 6.86 -> 6.56 ms p50
(-4%), hot1 4.03 -> 3.82, and pipelined kas2 p95 35.2 -> 34.9. Most of the cost was in the
token deal on the main thread, not in the parallel thinks. Adding the deal took the tick from
-4% to -20%.

Against the PERF-0 targets: kas2 XL p95 is 33.9-34.1 ms, still 0.6-0.8 ms over the proposed
33.3 step, with 90-92% of frames within it (80% before). hot1 XL still has most frames over
20 ms. Main-thread draw did not change.

## Profile notes for the next batch

Callgrind, workstation, kas2 XL 200%, 4 seats, 900 ticks, 4 think threads, `--separate-threads`.
The binary had the thinks-only cache.

- **Remaining DTEAM reads on the main thread.** These are the rows' callers left outside any
  pass:
  - `tokn::governed` (1.92M calls) and `dfocus::may_focus` (0.94M) are now held;
  - inlined in `step_with`, 480k calls (about 530 per tick, 3% of main-thread instructions).
    Most is the fire loop's `skill_of` (step.rs, `ai_spread_mult`), once per firing AI unit.
    That loop writes the sim, so under this packet's rule it computes as before. Next step:
    a counted `humans_per_team` kept in step with player slots and proxies, or rows read once
    before the fire loop, with its own hash proof.
- **memmove/memcpy.** Main thread: 2.6% of its instructions (load included). The top callers
  are `step_with` inlined copies (5.8M calls, 0.44 G instructions; the write-back's
  `ai_updates.iter().copied()` and `AiState` copies), `think_pool::run` collecting result
  tuples (0.31 G), and the main thread's own think partition (0.11 G). Each think thread:
  1.3%, almost all in the think closure (`let mut st = u.ai` and the returned
  `(.., AiState, PadState, NavTick)` tuple, about 480k calls a thread). A smaller `AiState`
  copy, such as returning only the changed parts or writing back in place by index, is the
  lever. Floor rays and `UnitGrid::within` are larger on the think threads (mesh_ray::cast
  6.7%, within 2.2%).

## Deviations

- The token deal is held as well as the thinks, because the per-thread profile named it as the
  main-thread hotspot (the plan's "any other &Sim loop the profile names").
- The 32 rows are split across maps rather than 32 seeds on one map. Hunt and LAN/proxy have 4
  and 8 rows, and XL has 16.
- The LAN case is `round_to_outcome` with `SWEEP_NETPLAY=1` plus a proxy, with the gate's
  `lan_soak` beside it. There was no two-machine LAN run.
- The callgrind memmove notes come from the thinks-only build. The deal change does not touch
  those copies.

## What is left

- The fire loop's per-unit `skill_of` (above).
- `AiState` copy volume in the think result and write-back.
- kas2 XL p95 under 33.3 ms: PERF-5 (per-view CPU) or the items above.
