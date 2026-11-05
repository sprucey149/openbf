# Soldier hit feedback — issue #104

The animator uses BF2's directional choice on every map, by maintainer decision
(2026-11-03). It samples frame zero of the current set's standing hit clip.
`FUN_00280e38` (B2:203467-203508) stores the contact side, chooses that side
half the time, each neighbour one fifth, and the opposite one tenth, then resets
its clock. `FUN_0027caf0` (B2:200920-200929) computes half the clock, capped
at half strength, before subtracting eight times the elapsed seconds. The pose
therefore lasts 0.125 seconds. `FUN_0027dfe0` (B2:201979-202008) gates the
upper body to normal locomotion, and the lower body to idle or turning, excluding
prone. Sprint, airborne, roll, jet and thrown states do not show it. Hits do not
interrupt weapon actions or change movement, aim or AI.

The visual animator reads direct round contacts, including shielded contacts,
from `Event::Hit.direct_round`. Splash, Force and emitter damage leave this
presentation bit clear. It adds no unit state or simulation RNG draw; events
are outside the simulation hash. Unit generation, index and simulation tick
key the render-only pose draw. A retained event is consumed once. Repeated hits
reset the clock. The overlay is applied to a copy of the ordinary pose so its
fade cannot accumulate in the kept pose.

The same animator retains contact direction for directional standing deaths,
including a killing contact and blast damage. BF2 `FUN_0027bae0`
(B2:200635-200667) reads the stored side before choosing simple slots 0x1e
through 0x21 in front/left/back/right order. Missing directional death clips
fall back to the set's forward death. Thrown deaths keep their existing thrown
rest animation. trace: PCSX2 T3 must settle front/back sign against the original
model axes, for both the hit pose and the death naming. The implementation
interprets the hit direction as projectile travel and negates it to find the
source side, then compares its planar forward/left components, following the
axis comparison in `FUN_00238e20` (B2:152435-152463).

First-person arms keep their separate animation chooser. Its eleven columns
cover weapon actions and locomotion, with no standing hit pose (BF1
`FUN_001a38e0` B1:47224-47462, BF2 `FUN_001bfcb0` B2:67155-67385; see re-first-person-arms.md §2-3).
The body still reacts when the player uses first person. The body hit clip is
not retargeted onto the FPM arm/camera rig.

BF2's damage flash matches BF1: `FUN_001dfb38` (B2:91408-91437), compared
with BF1 `FUN_001bc190` (B1:64387-64420). A surviving hit taking more than
2% of health remaining flashes red. Strength is damage divided by 20% of
health remaining, capped at one. Alpha is 32 plus 96 times strength, truncated
to a byte; duration is 0.15 plus 0.15 times strength seconds. The fade setup
`FUN_0045e7f0` (B2:521848-521895) gives immediate onset and a fade over that
duration. Only the local viewer's current life receives the overlay, on BF1
and BF2 maps, from both direct and splash damage. trace: PCSX2 T2 for
shake/rumble, any aim motion, and visual comparison of first-person feedback.

A vehicle's damage flashes the same way since 2026-11-05. `FUN_001dfb38` (B2:90723-90751) walks the damaged object's `+0x240`
controller list (`FUN_002690c8` returns `p + 0x240`) and flashes each entry with a player index of at least 0, for damage above
2% of the health left, while the object's alive bit is set. The port takes the pilot of the hull from `Event::VehicleHit`, which
crash damage also raises. `trace:` who else is on that list: `re-cheat-codes.md` §7 has it unconfirmed, so gunners and
passengers do not flash.
