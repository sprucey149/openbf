# Forest leaves and leafpatch bird anchors (LEAFF)

Airborne and ground particles are separate from canopy sprites. This note extends
`re-leafpatch.md` section 8 and corrects its tentative sound-anchor identification.

## Source reading

[V] BF1 `FUN_001b1f00` fills a drawn patch's missing slots, using byte +0x80
of its class. Property `0xfd7fae90` caps positive counts at 50
(`FUN_001b27b8`, lines 57630–57637). Positions are independent world-axis
samples around the patch centre: x/z ±Radius, y ±Height×Scale. The pool has
100 records (`FUN_001b0900` / `FUN_001b0c30`). This is a refill as slots
become free, not a one-time emission on the first draw.

[V] `FUN_001af8d0(...,4)` chooses mode 1 with probability 0.2, otherwise
mode 0. Birth yaw and phase are uniform through 2π; scalar r is uniform
through [0,1). The initializer stores the terrain height below the birth
point. Mode 3 belongs to ground leaves and snaps y to terrain+0.1.

[D] For an airborne leaf, let B be its birth yaw, p its phase, and r its
random scalar. The update reads p before advancing it by dt modulo 2π:

- Mode 0: s=sin(2p), Q=B·Rz(r·π/4·s). If r<0.15, append
  Ry((r−1)·p·10). Velocity is lerp((0,−0.5,0), Q·X·(−1.8s), |rs|).
- Mode 1: Q=B·Rx((r+2)·p·2π). Velocity is B·Z·(0.3r+0.3)−Y·(r+1).

[V] The sine helper is the interpolated 2048-entry table
`FUN_004452c0`; the table is initialized in RAM, not present at that
address in the ELF. The generic update also supports external impulse
vectors and their decay, now used by gusts and decorative stirring.

[V] The three-second word at BF1 0x5174c0 is **time without a draw**:
`FUN_001b03e0` increments +0x120, but `FUN_002eda78` clears it whenever
that leaf draws. A visible leaf may live longer. Falling below cached
terrain−0.5 retires it. Its owning patch's count is decremented on removal.

[V] The draw selects `leaf` (BF1 string 0x52d5e8), grey 0.4+0.6r and full
vertex alpha.

[D] The horizontal quad's packed coordinates correspond to
±0.17 inside bounds ±0.25, with the whole texture. Its GS state is
0x100044 / TEST 0x5120d / PRIM attributes 0x50. BF2's constructor and two
motion functions are twins (`FUN_001c80e0`, `FUN_001c85c0`,
`FUN_001c8938`); its generic state initializer `FUN_003c7d50` writes the
same draw state.

[V] ALPHA 0x44 is source-over; TEST 0x5120d means depth tested, alpha greater
than 32 on the GS 128 scale, no depth write. PRIM 0x50 has TME | ABE and no
FGE: particles are unfogged. The truncated vertex grey is (0.4+0.6r)*128.
Packed UV corners put U along +Z and V along +X. The port uses particle quads
with axes R*Z*0.17 and R*(-X)*0.17 and discards alpha <= 0.25.

## Live airborne check (BF2 Yavin, 2026-10-29)

[O] The maintainer's Yavin Temple slot 10 provides 63 flutter and 15 tumble
leaves across an original update. The production update reproduces position
within 3.1e-5 m and orientation axes within 2.6e-5. A frozen leaf placed 3 m
ahead in the original and fed through the port agrees in size and texture
orientation (difference-mask bounds 59x39 vs 57x36 px, filtering fringe included).
Local comparison: engine/artifacts/lead_tmp/leaff/yavin-slot10/ps2-port-leaf.png.

[V] Both-disc map census finds six-per-patch falling counts only on BF1 YAV1,
YAV2 (and YAV2_H), and BF2 DAG1, KAS2, YAV1. Bird counts on those classes are 3.

## Checks and limits

`python3 scripts/re_check.py fn b1 55519 55673 55707 55870 56020 56234 56461 57236 57265 276931 277024`
anchors initialization, motion, retirement, spawning and draw. Also:
`word b1 0x5174bc 2 --f32`, `word b1 0x52d5e8 1 --str`,
`fn b2 73535 73682 73721 73885 422977`.

[D] Native quaternion/trig math approximates the original table. Two
original sampled frames constrain its orientation/velocity: the flutter
referee allows 1e−5 in the right vector and 4e−5 m/s in velocity; the tumble
referee allows 2e−4 in its up vector (the original table's rounding).
The rare r<0.15 flutter extension is source-derived, not separately sampled.

[ours] Cosmetic random seed is fixed. Clock discontinuities take at most
100 ms per presentation update. Invalid negative authored counts become
zero. The pool's frustum/bounds visibility is conservative; exact original
edge-cull timing is not claimed. Ground and anchor paths follow below.


## Ground gusts and impulses

[V] BF1 FUN_001b0c30 / BF2 FUN_001c9468 tests a camera-near leafpatch region
after 30 seconds. A successful query resets the timer and attempts 20 mode-3
births, subject to the shared 100-leaf budget. Each starts 15-45 m ahead and
15-45 m upwind, initially 1-2 m above terrain and snapped to terrain + 0.1 m on its next update. Drift is normalized wind * 5 m/s; the
initial kick uses that same vector. The sampled BF2 Yavin default wind is
(5, 0, 0); the port honors authored manifest wind when present.

[V] The kick BF1 FUN_001b0870 / BF2 FUN_001c9088 accumulates
impulse += 0.9*velocity + Y*(0.4*length(velocity)). After phase advancement,
the generic step uses turning = impulse + drift*16*(1-phase/TAU), rotates
about (turning*TAU) cross Y, moves by (impulse+drift)*dt, and decays impulse
by 1-2*dt. The angular argument has no dt factor. Mode 3 resamples terrain
and sets y to terrain+0.1; draw state and retirement are shared with airborne leaves.

[O] An isolated BF2 Yavin mode-3 leaf with impulse (1.8, 0.8, 0), drift (5, 0, 0)
advanced x from 209.144058 to 209.370712 in 1/30 s, decayed impulse to
(1.68, 0.746667, 0), and followed terrain from -58.360847 to -58.601242.
The live-sample regression reproduces position within 4e-5 m and axes within
3e-4 (original table rounding).

[ours] Soldier and vehicle proximity triggers apply the original kick using
horizontal heading * 2. Moving capsule/hull footprints stir once per contact
and rearm after separation. Dead units, mounted soldiers and stationary actors
do not kick. Positions and velocities come from &Sim; no physics, damage,
input, simulation RNG or hashes change. Both footprints and paused updates
are tested. This trigger is an explicit presentation extension: traced direct
native kick callers are gusts and projectile/leafpatch hits. Exact original
soldier/vehicle trigger parity is not claimed.

## The supposed sound anchor is a bird anchor

[V] Class byte +0x81 BF1 / +0xa1 BF2, property 0x3b1948f3, is a bird count
capped at five. Its consumer BF1 FUN_0019cd78 / BF2 FUN_001b7b28 belongs to
BirdParticleManager. BF1 ELF RTTI strings at 0x52c740 / 0x52c750 explicitly
name BirdParticle and BirdParticleManager. Lua bindings 0x516844 / 0x51684c
name SetNumBirdTypes / SetBirdType. This supersedes the sound-slot hypothesis
in re-leafpatch.md section 8 and the issue follow-up.

[V] The manager keeps five nearby leafpatch anchors. Every 30 seconds in BF1 (20 seconds in BF2) it picks
one of five slots and births that anchor's count, up to 15 live birds.
Mission types supply size and texture: BF1 Yavin Arena and BF2 Yavin use
bird (1.0), bird2 (1.5); BF1 Yavin Temple declares none. BF2 Dagobah GCW
uses those two; Kashyyyk uses bird (1.0). gen_missions.py reads these setters
from local PS2 bytecode.

[V] Anchor birds use mode 0, live five seconds, no fade. Birth yaw is uniform
through TAU, pitch is (-0.25-0.25*u)*PI/2, speed 8+8*r. Update BF1 FUN_0019b828 /
BF2 FUN_001b65f0 uses a=(8+8*r)*phase, inner flap sin(a)*PI/4, outer flap
sin(a-0.5654867)*1.0995574, each local rotation Rz(flap)*Ry(PI/4).
Position is origin+velocity*time-birth.up*0.1*sin(outer flap), before time/phase
advancement. BF1 draw FUN_002e8b88 emits five quads: a body on the bottom
atlas half and two hinged wing sections on each side on the top half.
Body grey is 0.8; wings white. FUN_002e9cc0 writes the same unfogged cutout
state as leaves: 0x100044 / 0x5120d / 0x50.

[O] Three live BF2 Yavin anchor birds were sampled across one update. Their
production update reproduces position within 4e-5 m and both wing matrices
within 3e-4. Original samples, RAM and saved states remain local and ignored.

[V] Separate SetSoundEffect bird/grass slots exist, but none of the shipped
missions sets them (re-ambience-music.md section 6). No extra leaf rustle loop
is warranted by this anchor. Existing map ambience remains in its own system.
The manager's independent flying-flock/ground-bird paths are not leafpatch
anchors and are outside this leaf effect.

## Reproducing views

The `forest_capture` example (removed in the sprint's code cleanse; in git history) took
<baked-map> <output-dir> [x,y,z,yaw,pitch]. It advanced production leaf
presentation for 30.67 seconds (also saving the 20.67-second view), then captures the same world with and without
forest particles. Non-emitting maps must have byte-identical RGBA buffers.
Outputs contain original artwork and must remain local.

These additions are gated to maps with authored falling emitters. Scene changes
reset pools, anchors and clocks. Battle presentation follows simulation time,
so pause freezes gusts, impulses and birds. Native samples are exercised by
data-gated tests; tests explicitly report when local references are absent.


## Final local checks (2026-10-29)

The 30.67-second before/after pairs change 106 pixels on BF2 Yavin, 212 on
BF1 Yavin Arena, 91 on BF1 Yavin Temple, 94 on Dagobah and 46 on Kashyyyk
at 960x540. The 20.67-second pairs also exercise BF2's earlier bird interval.
Tatooine and BF1 Naboo Plains controls change zero pixels at either time.
Files: engine/artifacts/lead_tmp/leaff/full-reference/{yavin,bf1_yav1,bf1_yav2,
bf2_dag1,bf2_kas2,tatooine,naboo-control}/, visual-metrics.json alongside.
These are port effect-isolation views; the original comparison is the separate
PS2 leaf view and sampled ground/bird motion above. They are not a claim of
pixel equality for the entire original game's scene.

Renderer library and leaf regressions, both-disc bake census, 23 mission-related
game tests, particle blend/cutout GPU referees and scene-clear GPU referee pass.
Formatting and Clippy with warnings denied pass. The five affected maps were
rebaked; BF1 Yavin Arena's donor cover hints were reapplied after its map bake.
Existing baked manifests otherwise match the previous packed release apart
from the new bird counts. No game assets or reference binaries enter the commit.
