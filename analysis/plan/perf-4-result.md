# PERF-4 result: bloom in split views without the frame copy

Date 2026-11-27. Branch `claude/perf-4`, based on main e232c7e3b.

## Step 0: sub-marks, and the saving written down before building

The GPU timer's single `bloom` section is now six: `bloom gather` (including the emissive sky
domes), `bloom flare`, `bloom copy`, `bloom extract`, `bloom blur`, `bloom composite`. Their
sum is the old `bloom` mark. Timing only; nothing changes without `OPENBF_GPU_TIMING=1`.

littlemallard (Radeon 780M, RADV), headless, 3840x2160, scale 1.0, elite, seed 7, Team Size
200% (kas2 XL), 900 ticks then 120 waited frames; mean GPU ms per frame, all views summed:

| row | gather | flare | copy | extract | blur | composite | bloom |
|---|---|---|---|---|---|---|---|
| mus1 1 view | 1.52 | 0.00 | 0.72 | 0.38 | 0.88 | 0.73 | 4.23 |
| mus1 4 views | 2.44 | 0.02 | 0.79 | 0.80 | 1.47 | 0.83 | 6.35 |
| kas2 XL 1 view | 1.45 | 0.00 | 0.68 | 0.60 | 1.04 | 0.71 | 4.48 |
| kas2 XL 4 views | 1.94 | 0.02 | 0.74 | 0.79 | 1.45 | 0.82 | 5.77 |
| cor1 1 view | 0.89 | 0.01 | 0.67 | 0.39 | 0.82 | 0.65 | 3.44 |
| cor1 4 views | 0.89 | 0.03 | 0.73 | 0.77 | 1.45 | 0.79 | 4.66 |

The extra cost of 4 views is mostly the gather (+0.9 ms on mus1; in these readings `bloom
gather` also holds the particle (heat) blur pass just before it, so the cause is not yet split
between the per-view sky domes and that blur; the gather stays as it is in this packet), then the blur (+0.6), the extract (+0.4) and the
composite (+0.1). The copy costs about 0.75 ms at any view count.

Probes (an env-gated build, not committed), 4 views at 1.0, two runs each, all within 0.02 ms:

| probe | mus1 bloom | kas2 XL bloom | image |
|---|---|---|---|
| as now | 6.33 | 5.79 | - |
| no frame copy | 5.48 | 4.97 | (would be the same with a second target) |
| extract and blur load instead of clear | 5.96-6.14 | 5.59 | same (they write every texel) |
| composite loads instead of clears | composite +0.18 ms | +0.17 ms | - |
| 4-tap extract (one texel per 2x2) | 5.88-5.94 | 5.38 | changes: 1-px bolts glow double or not at all as they move |
| half the blur passes | 5.62 | 5.08 | changes: shorter glow |
| no blur | 4.86 | 4.34 | - |

Written down before building: drop the copy in split views (about -0.85 ms: copy 0.78 plus a
cheaper composite) and load rather than clear in the split extract and blur (about -0.3 ms):
**about -1.1 ms GPU per 4-view frame at scale 1.0 on mus1 and kas2 XL, with the same image.**
Not taken: the sparse extract (no exact fewer-tap form exists while the threshold comes before
the average; the sparse one makes thin bolts' glow flicker), and fewer or merged blur passes
(the blur is not the main per-view cost, about 0.3 ms of fixed cost is left after the load
change, and merged passes change the glow).

## What changed (OURS, split views only)

- `render/src/world_scale.rs`: `WorldScale::bloom_out`, a second colour target per size that
  shares the world target's depth (one extra 1920x1080 colour target at 4 views and 1.0).
- `render/src/bloom.rs`: `BloomState::prepare_direct` caches a composite bind group that reads
  the view's world target (display-unit view) instead of the `bloom frame` copy. `bloom_pass`
  takes `out`: when given, no `copy_texture_to_texture`, the composite reads the world target
  and writes `out`, and the extract and blur passes load instead of clearing. The composite
  still clears (black, alpha 1, the alpha every frame pixel keeps).
- `render/src/frame/submit.rs`: a split view (`slot` set, which always has a world target)
  with bloom on takes `bloom_out`; after the bloom it carries on in that target: the sun flare,
  filter, softness, underwater tint, world overlays and the upscale draw there, in the same
  order as before, against the same depth.
- One view (`slot` none), the external target and every scale keep the old path, copy included.
- The module note says why (measured cost); the "half-scale display units" wording in it is the
  PS2 colour scale and was not used as a reason.

## Tests

- `scripts/one_player_gate.py` (release build of this branch): every row matches (lan_soak ok,
  1559 ticks, 4312 hash checks).
- `cargo test -p render --test suite -- --include-ignored view bloom`: 10 passed, including the
  new `views::a_split_views_bloom_matches_one_view_of_its_size` (a lamp-marked glow quad in the
  left cell of a 2-view present equals one 128x128 view byte for byte; the right cell stays black).
- `cargo test -p render --lib bloom world_scale`: 2 passed.
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 3 passed; the 4-view
  capture was looked at.
- clippy on render: no new warnings; rustfmt clean on the touched files.

## Workstation captures (landing check)

Bench `--capture-frame`, 3840x2160, seed 7, Team Size 200% (kas2 XL), before (main rendering
code plus the sub-marks) and after: cor1, mus1 (lava) and kas2 XL (bolts) at 1, 2, 3 and 4 views,
scale 1.0 and 0.5, 24 pairs. **All 24 pairs are identical (0 differing pixels).** Sim hashes
match. Moving camera: kas2 XL, 4 views, scripted seats, frames 2-24 (12 frames, 70% of seat 0's
cell changes between the first and last): **all 12 pairs identical**, so bolt glow under motion is
unchanged. Files in the session scratchpad `p4/caps/`.

## littlemallard measurements

Headless, ES-DE idle, nothing on the TV, /tmp/openbf-bench only (deleted afterwards, the
installed openbf.pak read through a symlink, ~/Emulation/ports/openbf untouched), before and
after binaries alternated, performance profile. Same scenario as step 0. Bloom = the six
sub-marks summed; GPU total all views.

| map | views | scale | gather | copy | extract | blur | composite | bloom before -> after | GPU total before -> after |
|---|---|---|---|---|---|---|---|---|---|
| mus1 | 2 | 1.0 | 1.68 / 1.72 | 0.71 / 0 | 0.52 / 0.49 | 1.66 / 1.19 | 0.74 / 0.55 | 5.32 -> 3.96 | 10.73 -> 9.41 |
| mus1 | 2 | 0.5 | 0.70 / 0.70 | 0.21 / 0 | 0.21 / 0.19 | 0.42 / 0.35 | 0.22 / 0.21 | 1.77 -> 1.46 | 5.72 -> 5.42 |
| mus1 | 3 | 1.0 | 1.86 / 1.85 | 0.59 / 0 | 0.59 / 0.54 | 1.10 / 0.87 | 0.62 / 0.58 | 4.78 -> 3.86 | 11.04 -> 10.09 |
| mus1 | 3 | 0.5 | 0.71 / 0.71 | 0.17 / 0 | 0.17 / 0.16 | 0.41 / 0.37 | 0.19 / 0.18 | 1.66 -> 1.43 | 4.95 -> 4.73 |
| mus1 | 4 | 1.0 | 2.42 / 2.44 | 0.78 / 0 | 0.80 / 0.72 | 1.46 / 1.16 | 0.82 / 0.77 | 6.30 -> 5.11 | 14.08 -> 12.92 |
| mus1 | 4 | 0.5 | 0.94 / 0.94 | 0.22 / 0 | 0.23 / 0.21 | 0.54 / 0.49 | 0.25 / 0.24 | 2.20 -> 1.89 | 6.31 -> 6.01 |
| kas2 XL | 2 | 1.0 | 1.79 / 1.79 | 0.72 / 0 | 0.72 / 0.68 | 1.77 / 1.31 | 0.79 / 0.73 | 5.80 -> 4.53 | 22.87 -> 21.81 |
| kas2 XL | 2 | 0.5 | 0.68 / 0.68 | 0.20 / 0 | 0.21 / 0.19 | 0.42 / 0.35 | 0.23 / 0.21 | 1.74 -> 1.44 | 10.32 -> 9.93 |
| kas2 XL | 3 | 1.0 | 1.42 / 1.43 | 0.55 / 0 | 0.59 / 0.54 | 1.09 / 0.87 | 0.62 / 0.58 | 4.29 -> 3.43 | 18.92 -> 18.10 |
| kas2 XL | 3 | 0.5 | 0.72 / 0.72 | 0.16 / 0 | 0.17 / 0.16 | 0.40 / 0.37 | 0.19 / 0.18 | 1.66 -> 1.44 | 10.05 -> 9.85 |
| kas2 XL | 4 | 1.0 | 1.95 / 1.95 | 0.75 / 0 | 0.79 / 0.72 | 1.46 / 1.16 | 0.83 / 0.77 | 5.80 -> 4.61 | 27.07 -> 25.87 |
| kas2 XL | 4 | 0.5 | 0.95 / 0.95 | 0.21 / 0 | 0.23 / 0.21 | 0.54 / 0.49 | 0.25 / 0.24 | 2.20 -> 1.90 | 14.11 -> 13.80 |
| cor1 | 2 | 1.0 | 0.60 / 0.63 | 0.68 / 0 | 0.50 / 0.46 | 1.65 / 1.18 | 0.70 / 0.49 | 4.14 -> 2.78 | 11.00 -> 9.86 |
| cor1 | 2 | 0.5 | 0.26 / 0.26 | 0.19 / 0 | 0.21 / 0.18 | 0.41 / 0.35 | 0.22 / 0.21 | 1.31 -> 1.02 | 6.40 -> 6.17 |
| cor1 | 3 | 1.0 | 0.79 / 0.79 | 0.55 / 0 | 0.59 / 0.53 | 1.10 / 0.87 | 0.60 / 0.57 | 3.66 -> 2.79 | 11.60 -> 10.73 |
| cor1 | 3 | 0.5 | 0.32 / 0.32 | 0.16 / 0 | 0.17 / 0.15 | 0.41 / 0.37 | 0.18 / 0.17 | 1.26 -> 1.04 | 5.67 -> 5.46 |
| cor1 | 4 | 1.0 | 0.90 / 0.90 | 0.73 / 0 | 0.78 / 0.71 | 1.47 / 1.16 | 0.79 / 0.75 | 4.71 -> 3.55 | 15.79 -> 14.61 |
| cor1 | 4 | 0.5 | 0.38 / 0.38 | 0.21 / 0 | 0.23 / 0.20 | 0.54 / 0.49 | 0.25 / 0.24 | 1.63 -> 1.33 | 7.86 -> 7.52 |

One view (control): unchanged within 0.03 ms on every map at both scales (copy still ~0.7 ms at
1.0). The flare is 0.01-0.03 ms everywhere.

Pipelined mus1, 4 views, split UI, AI seats, scale 1.0, 60 s after 900 ticks; two runs each,
order base/new then new/base:

| row | before | after |
|---|---|---|
| frame p50 / p95 / p99 ms | 16.38 / 20.09 / 21.84; 16.39 / 20.21 / 21.96 | 15.11 / 18.96 / 20.41; 15.11 / 19.03 / 20.50 |
| GPU p50 / p95 ms | 15.53 / 18.03; 15.57 / 18.10 | 14.32 / 16.82; 14.34 / 16.86 |
| frames within 16.7 ms | 55.7%; 55.2% | 74.0%; 74.7% |
| main CPU p50 ms | 13.52; 13.81 | 12.88; 12.95 |

Gain: as set at step 0, about 1.2 ms GPU per 4-view frame at 1.0 (mus1 -1.16, kas2 XL -1.20,
cor1 -1.18), 0.9-1.3 ms at 2 and 3 views, 0.2-0.4 ms at 0.5. Mustafar's 4-view p95 is now about
19.0 ms: still about 2.3 ms short of 16.7.

## Deviations

- The plan's "fewer taps in fs_extract" was not built: the extract already goes straight from
  full-res light to quarter size, and with the threshold kept before the average every texel
  has to be read. The only fewer-tap form is sparse and was measured (-0.4 ms) but makes thin
  bolt glow flicker, so it was left out. The saving came instead from the copy and the clears,
  with an identical image.
- No fewer or merged blur passes (see step 0).
- The change applies to every split view (2, 3 and 4 views), not only cells at most half the
  window, because it does not change the image.
- Captures are bench stills and a scripted-seat sequence; only seat 0 walked in the sequence.
- The `bloom` GPU timer section is now six `bloom ...` sections; tables that read `bloom` sum them.

## What is left

- The 4-view gather growth (+0.9 ms on mus1) is the largest remaining per-view bloom cost.
  The readings above lump the particle blur into it; a separate `particle blur` mark now ends
  that pass (review fix), so the next packet first re-reads gather and particle blur on mus1 at
  1 and 4 views before choosing between the sky domes and the blur.
- About 0.3 ms of per-pass fixed cost in the 4-view blur (40 small passes); merging pairs would
  change the glow and needs a look check.
- The `bloom frame` copy texture is still made for split sizes though they no longer use it
  (one 1920x1080 colour target at 4 views, on top of the new `bloom_out`). It cannot simply be
  skipped: the extract and blur bind groups also bind it as their frame input, so dropping it
  needs a per-size stand-in or split layouts. A follow-up.
- Code size: the engine grows (about +200/-57 lines before the review fixes, plus a 65-line
  test) for the second composite path and target; the shared `inputs()` is the only dedupe.

## Review fixes

- `particle blur` GPU timer mark after the heat blur pass, so `bloom gather` is the gather only.
- `bloom_pass` with `out` now requires the direct bind group on `src` (`expect`) rather than
  falling back to a copy composite into the wrong target.
- One `bloom_on` gate shared by the `bloom_out` choice and the bloom block.
- Window reconfigure drops the bloom caches' direct world-target clones with the world-scale
  targets (`forget_bloom_direct`).
- Rechecked: one-player gate matches; render `view bloom` suite 10 passed; lib `bloom` and
  `world_scale` 1 each.
- TV look on the playtest build is a formality here (identical captures), noted for the playtest.
