# BF2 saber combat and movement

Primary sources are the BF2 PS2 executable and shipped side data. `[V]` means
fresh source/ELF or stated RAM evidence, `[D]` combines those observations,
and `[H]` marks an unresolved claim. This covers BF2. It does not establish
BF1 hero behavior or exact combat parity for the port.

## Blade contact

[V] Hand melee is FUN00340c60. The class supplies each edge's actual node,
length and width; an attack selects an edge and optional length/width
multipliers. DamageDistance and DamageLength write the same packed field,
with the last authored value winning. The caller transforms the posed edge
and starts0.2m behind its root along its own direction.

[V] Soldier collision001691a0→00168318→00160518→0016a710 expands sphere
radius, box half extents, or cylinder radius/half height by Width, shifts
another Width backward, and keeps the original length. Width is a radius.
This is an expanded-primitive ray, not a rounded capsule. A start inside
counts; a new hit at the exact far endpoint does not. Mesh slot00160d60
bypasses the expansion helper and uses the plain ray. Raw MIPS is required
for00166328: the decompiler loses the VU hit branch.

[V] Hand melee gathers at most8 candidates with mask0x3a (FUN00340c60,
B2:336191). Hover collision type8 and building type0x10, including armed
buildings, are included; exposed riders have type0x20.
The per-attack cache records a victim before asking it to block, so a block
consumes that contact. Cache identity is the same authored attack record,
or a matching nonzero AttackId packed into20bits. A cached record expires
after its damage-window end plus0.3s; state exit rebases its clock. Merely
entering IDLE does not clear every cache. Animation-sharing transitions
need the same native time rebasing as the combo evaluator.

[V] The hand-melee descriptor explicitly sets all health-type and shield
scales to1. Hit distance locates the contact point, not a head/limb damage
multiplier. The post-contact dispatch has no extra perception or generic
LOS test. Native primitives decide contact. The target's ordinary friendly
fire, health and damage bookkeeping still applies.

The port queries posed blade segments, units, dynamic vehicle hulls and
bound station and building-turret primitives. Static instance resolution
shares the bolt lookup: station first, then the bound turret's vehicle pool.
Building turrets have no dynamic hull; their authored map bodies carry the
contact. No extra shape or reach is added. Its earlier map-only body iteration
omitted vehicle hulls; a production-consumer regression exposed that defect.
`CollisionWorld::bodies_and_hulls` now includes both collections. Linear
interpolation of baked endpoints replaces the native subframe bone
evaluator and is named in FEEL_NOTES. Native crowded broadphase ordering
has not been reproduced.

## Block and redirected projectiles

[V] FUN003074a8 tests signed incoming angle in the owner's transform frame.
FUN0033e8b8 separately uses horizontal motion/facing to select the block
animation. An impact pays the packed Deflect EnergyCost through001e2010.
There is no full-amount affordability precheck: a partial, undepleted bar
can pay once and become depleted. Positive cost fails when already
depleted; zero cost still succeeds. The same rule applies to projectiles,
melee and immediate emitter damage.

[V] A redirected projectile is aimed around the defender weapon's aim
vector, not geometrically mirrored. Spread is0.25*(1-p)+0.05*p below
precision1; at precision1 it is0.02 before state age0.4 and0.05 afterward.
Entering deflect sets its timer/precision field to1; its update adds dt,
and exit clears it. The earlier identification of341e28 as a melee-contact
callback was wrong: its vtable slot is deflect entry. Constructor00314568
creates the same projectile class with new owner/team, age0 and charge1.

[V] The projectile virtual RTTI call at334603 checks OrdnanceBeam
(DAT699c70; name at6267d0). Bullet315248 and Laser31c220 accept their own
and inherited Bullet identities, not Beam. The native factory labels bolt
and laser construct Laser31af80, bullet constructs Bullet3130d8, and beam
constructs Beam. A fresh raw hero class closure finds six bolt ordnance
chains, Jango's bullet chain, and Leia's target-pistol Beam. Other ordnance
kinds remain separate. Ordinary Bullet/Laser consumes four global
DAT73d868 LCG steps for independent axis offsets and resets precision to0
(334605–334651). Beam uses two address-seeded packed uniforms for phase
and time wobble; its precision update first clamps p-dt to1, then subtracts
2dt and clamps to0 (334654–334728).

[V] Native vtable629838 slots100/104/108 point to341e28/341e60/341e70.
These are deflect entry/exit/update: set precision1, clear it, and add dt.
Update restores the base animation only when precision is strictly >1.
A hand melee contact does not set the attacker's precision. Motion is used
only to choose a directional pose: horizontal velocity longer than0.3m/s
replaces the owner's forward axis, reversed when its dot with forward is
nonpositive. FUN3074a8 uses strict0.92388/0.38268 and inclusive-0.70711
buckets, ordered direction-mask fallbacks, and one packed DAT73d868 draw
when more than one animation matches. The draw occurs before energy
payment, including a failed payment.

[V] Builder3448d8 caches the FIRST Deflect state's attachment matrices at
0.1s. PS2 hero DeflectAnimation entries use attachment group0: usually
hp_weapons, but Grievous uses the first registered bone_rf_hand binding.
At impact,334384–334576 transforms the cached pose by the owner's frame,
chooses the group0 edge whose midpoint is closest to the local impact,
and uses clamp((dot(impact-root,dir)/length+1)*0.5,0,1) along that edge.
Without a cached matrix, origin remains the impact; without an edge of the
selected attachment group, origin is the cached matrix translation.
FUN33f600 additionally rotates the cached pose frame using retained thrust.
Physical W/D/A/S+D controls establish raw forward/right signs.
33bcf8 copies the raw pair before factors unless the previous state has
Initial bit21. 33f600 negates strafe for its output rotation, reverses both
coefficients for current backward input, applies literal0.7071 caps, and
normalizes only the interior. Its input rotation omits that strafe negation.

[D] The port now has separate typed Beam dispatch, directional block poses,
cached .1s attachment-origin projection, and a dedicated melee LCG stream.
[ours] Round seed XOR0x53414252 replaces the native global's external seed;
Beam address entropy uses stable shot/handle fields. The stream becomes
hashed before its first draw when a saber holder appears, and remains
hashed after that holder dies. Ordinary no-saber simulations retain their
old random streams and hash layout. Actual-data checks cover all15 native
group0 bindings and24 original cached poses (maximum component error
0.0001052022 against preselected0.002). Focused checks also cover alignment,
interruption, incoming-axis contact, precision, and nonunit-basis transpose.
A source mutation replacing that transpose with a general inverse makes
the dedicated regression fail; restoring it passes.

[V, RAM] A controlled original enemy gun uses normal weapon update and
physical hero R2 input. Control/unblocked/blocked trials retain105/104/103
advancing-clock rows. The scratch filenames call this a pistol, but later
raw packet and packed-slot auditing disproved that identity: staging both
desired and active selection skipped the weapon-change transition, and
observed incoming Laser packets belong to the2000m/s rifle class. The
following block/pose observations remain scoped to the actual fired gun;
no pistol-selection acceptance is inferred. The block selects animation
142 beside base140, and precision drops1.4666 to0.033333331 after contact.
Unblocked damage exceeds the no-input drain control by about600HP; blocked
health stays within one drain tick of that control. This establishes
accepted block lifecycle and directional-pose selection.

[H] Live redirected packet origin/jitter numerical parity is not established.
The initial scanner missed the Laser leaf, and later candidate groups were
unpaired, including unrelated shotgun ordnance. A fixed-pool retry did not
retain the intended weapon selection and produced no measured trials.
It is rejected as a numerical referee. The source/RAM cached-pose and
native accepted-block lifecycle comparisons above are separate evidence.

## Saber throw

[V] Constructor0031c370 uses primary saber-edge templates and the actual
launch attachment. The initial spin axis is normalized cross(edge world
direction, shot direction). Ord class com_weap_inf_sabre_throw_ord authors
Velocity30, LifeSpan1.333, return duration0.8, RotationRate4 revolutions/s,
DamageWidth0.75 and DamageLength1.2 FromEdge. Runtime angular speed is
25.132741928 rad/s. Secondary/offhand blade groups are not thrown.

[V] FUN0031cb78 keeps a signed remaining clock. On the last outgoing
update it subtracts dt, clears the eight-victim cache when crossing zero,
then uses the **new signed remainder** for contacts, travel and spin. A
small backward step is real. On return, the position increment is
`(owner_target-position)*dt/(duration-old_return_age)`; return spin is half
the outgoing rate. A world-clipped outgoing duration scales return time
by the same ratio to the authored lifespan. Exact zero also clears the
cache before its zero-time contact pass.

[V] FUN0031d688 uses substeps with
`count=floor(dt/min(0.5/(velocity+rotation_rate),0.0333))+1`, actual primary
edges, the same0.2m root offset, native Width query, and block before
damage. Per-leg victim count is at most8, so the same target can be struck
once outgoing and once returning. Its incoming direction is the stored
flight velocity. The throw descriptor differs from hand melee: class
vtable6273b8+30 resolves00310178 and retains authored health-type scales.

[V] The throw independently gathers eight candidates with the same0x3a
mask (FUN0031d688, B2:311800-311801), so armed buildings are included here
too. It tests each candidate's own collision at311975-311978, asks it to
block at311999, and dispatches the ordinary ordnance health descriptor at
312002-312007. Sharing the static binding lookup does not change this
descriptor to hand melee's uniform scales. The port retains its existing
return-on-vehicle-contact policy below; this is not native throw acceptance.

[V, RAM] A controlled Anakin throw recording contains133 advancing game
clock rows and60 live flight rows, with an untargeted Choke control yielding
no flight. It records the original class values above and the primary
attachment offset. Thirty-five outgoing, two boundary, and twenty-three
return observations reproduce the integration from their recorded previous
points to within0.000011m. The positive0.032999903 clock becomes
-0.000333428; position moves about0.01m backward on that boundary. This
validates the native integration law. It does not compare the port's rig,
contact ordering or final hilt-return animation.

[V] Weapon constructor003450c0 samples ThrowAnimation at the throw class
InitialSalvoDelay into its own cached matrix+130..160. The flight
instance+108 points to that throw weapon, not the primary saber. Return
0031cb78 transforms the same cached translation by the current owner
frame; it does not resample the live animation. The port now uses that
constructor pose. Its Anakin translation matches saved RAM within1mm.

[D] The production flight step reproduces all60 recorded observations
(45unique states) from their native previous positions and cached target,
within four component ULPs to allow EE truncation versus host rounding.
That referee covers35outgoing,2signed-boundary and23return rows; it does
not compare spin, rig/contact sampling or crowded candidate ordering.
[V] Flight0031cb78 stops for owner generation mismatch or completion, not
for zero health, mounting or a hidden body. Death2379f0 and mount24af28
invoke weapon deactivation; the audited throw slots326058/3274c0/3269f8
neither clear the flight nor its in-flight flag. Owner frame getter2560d0
returns the current entity+120 matrix without a health/visibility test.
Independent draw31d5a8 calls primary33e018 with no owner skeleton and the
flight matrix. It keeps group0 hilt/blade/glow; held-body drawing suppresses
that group while flying. Retraction33dd40 refuses while in-flight, including
a dead owner's saber. Generation retirement remains a distinct boundary.
Ordinary native death retains the entity generation through3s and invalidates
it at4.2s in the observed control. This is source/caller and separate-death
RAM evidence, not an original throw-at-death capture.

[D] Flight state is hashed and survives secondary selection. Tests now
separate ordinary health death, current mounted return frame, natural catch
and explicit despawn. Health-death continuation, current mounted return,
Despawn/recycle retirement and independent flight drawing pass their
production-consumer regressions.

## Authored movement

[V] Combo state parser00307a80 packs Gravity as a signed byte in tenths
and GravityVelocityTarget as signed12bit tenths. Impulse writes the gravity
byte sentinel-128; a later Gravity property replaces it, so authored order
matters. ZeroGravity is a separate target flag. Helper0033c4d0 keeps a
per-state target latch: an impulse sets Y velocity once; ordinary modified
gravity resets to1g, or0g with ZeroGravity, after the directional target
condition is reached. It does not clamp velocity to the threshold.

[V] The native soldier calls weapon vtable+6c at00240bf8 before energy
and stance movement, passing its current Y velocity by reference. The
ordinary mover then applies drag, acceleration, gravity and position.
Applying the impulse after integration changes the result.

[V] Root motion supplies desired local movement, not an immediate final
velocity. Stand/crouch/prone/sprint blend the ordinary desired velocity
with the authored target before acceleration. Jump/thrown air movement
uses the authored acceleration times its blend weight when active;
roll and jet/hover cases do not read that local movement override.
The animated-move weight approaches0 or1 at6.6666665/s, using the previous
runtime active flag before this update's transitions. Until can clear
that flag while the state's target remains available during fade-out.

[V] Loader0030a480 resolves FromAnim velocities once from the animation
record's packed centimeter values, multiplies authored factors and packs
again at1/50m/s. Animation loader00282700 first samples DummyRoot at zero
and HALF of `(frames−1)*0x3d088889`, then divides the displacement by that
half-duration. Each axis is packed with the native EE sequence `*100`,
`+16383`, `+.5`, clamp0..32767, truncate, subtract16383, then signed15-bit
extraction. The upper-edge wrap and negative tie behavior are retained.
Parser00307a80 supplies factor50 and FromAnim for both axes even when
AnimatedMove has no Velocity children. Native30a704..30a730 decodes that
factor by EE multiply with `.02`, multiplies the decoded centimeter speed,
then multiplies by50/truncates/clamps signed12. Getter0033c4d0 decodes by
EE multiply with `.02`. Division and implicit-default bypass both change
this chain: decoded40cm is bits3ecccccc, and the final movement after a
factor1 repack is bits3ec28f5b, about.38m/s. Actual Maul jumpattack_end_full
uses the implicit form. It does not sample a different instantaneous
root-track derivative every update. The port's former instantaneous/post-integration
prototype was replaced by this desired-motion path and production-step
gravity/ordinary movement regressions.

[H] A complete matched native moving-actor replay across flips and
interrupted same-animation states is not established. Source laws, retained
input/state clocks and the named movement regressions have narrower evidence. The combo resource and event-clock analysis is in
[re-combo.md](re-combo.md); hero eligibility and health are in
[re-bf2-hero-flow.md](re-bf2-hero-flow.md).


## Independent weapon alignment

[V] Physical right attack rows keep the actor owner frame at yaw0 while
active states8/9 carry AlignedToThrust bit20. FUN33c9e0 calls33f600 on
weapon aim+50 then3018b8, which stores that weapon direction. It does not
rotate the actor or camera; Deflect states skip this aim adjustment. The
previous port prototype wrote an aligned yaw directly into Soldier.yaw;
that was incorrect and is removed. The earlier initial_thrust_yaw field
was not native future state and is removed; the actual retained pair is
hashed in MeleeState.

[V] For aligned AnimatedMove,33c4d0 first rotates local raw input
[-right,0,forward] with33f600 parameter3=1, applies the authored strafe and
thrust factors to their separate axes, adds those to the resolved animation
velocity, then rotates the final local target with parameter3=0. The
unaligned branch keeps local X=-rawRight*factor and Z=forward*factor.
The Soldier MotionTarget conversion negates artist-local X back to physical
right; that existing sign is supported and remains unchanged. Previous
AnimatedMove fallback keeps its previous state's alignment and resolved
FromAnim clip speed while the movement blend decays.

[D] The port's hand query basis takes the independent weapon vector for
aligned states; the renderer receives it as its aim input. Native153965
passes weapon aim through27cab8 to the animator. No whole-body rotation is invented. Native279780 locks the first SCOP
for a shared animation/layer; a forward declaration does not lock it.
Animation, OffhandAnimation and ThrowAnimation share this descriptor table.
Bare scopes copy to the lower layer with AimType cleared. Properties within
a scope apply in authored order; BlendInTime supports Frames, EE hundredth
packing, byte saturation and decoding. BlendTimeIn is a different key.

[V, RAM] The same Aayla ATTACK3, ATTACK3_LAND and RECOVER3 all refer to one
animation descriptor. In original FeluciaCW its upper word05220094 is
Torso and its lower word05300094 is None. In original Hero Assault the
upper05200095 is None, locked by a previously loaded hero. Fresh normal-menu
Felucia loading and an independent reader reproduce the context difference;
two authored Torso records do not mean only two effective Torso states.

[D, ours] The maintainer chose Hero Assault's behavior as Aayla's default
on 2026-10-30. The gameplay class therefore overrides only her shared
`stand_attack1c` upper AimType to None after native mission registration;
ATTACK3, ATTACK3_LAND and RECOVER3 share this clip. Other heroes, blend
metadata and raw mission/source tables keep their native values. The actual
model regression checks the default unadjusted torso/weapon/root and a
separate raw-Felucia Torso positive control. This is presentation only;
the physical attack-edge basis remains the source-derived query below.

[D] The port resolves first definitions from the mission's exact ordered
SIDE filename/request pairs and native lvl_ request tags. Baked indices retain
ordered COMB names and empty resources, including non-class aliases. It does
not infer load order from team rosters or ClassDb names. Old RON remains
readable; scope/load provenance is required for this native descriptor input.
All33 mission resource closures, both original descriptor contexts, reordered
first-definition controls and all three Aayla states pass. Custom None and
FullBody keep the raw pose/root treatment; Torso/Head adjust a copy through
the existing aim layer. Ordinary BF1 aiming is unchanged.

[V] Damage340c60 reads precomputed class edge samples from its attack record,
then constructs the actor or aligned weapon basis (B2:336082–336155). It
never queries the live torso-aimed render bones. The new descriptor table
is therefore presentation input and adds no hidden simulation state.
Full native moving visual and quaternion parity are not claimed.

Re-check anchors: `python3 scripts/re_check.py fn b2 198858 297352 297364
202437 202780 336092` verifies the parser lock, descriptor selection,
aim caller and physical-basis ownership before reading those functions.

## Blade contact presentation

[V] Melee parser FUN_00342528 (B2:336789-336887) stores `HitEffect` 0xc0929477
at class+0x24c (count +0x260 = 1), `DeflectEffect` 0xf1820751 at +0x26c,
`HitSound` 0xc5bdd741 at +0x264, `DeflectSound` 0xc20dfe6f at +0x270. Every
shipped saber class inherits com_sfx_sabrehit, imp_weap_lightsabre_hit,
com_sfx_sabredeflect and imp_weap_lightsabre_deflect from
com_weap_inf_lightsaber. Rows 1-4 of +0x24c are the comic-hit cheat's
(AddHitEffect, DAT_005ff0fb); no class authors them.

[V] Hand melee FUN_00340c60, after a block is refused: damage slot +0x1c, Push
through +0x94 when positive, the effect reserved once for the update and moved
to each victim's contact (336296-336333), one HitSound at the owner per update
(336346-336356). Block FUN_0033e8b8: DeflectSound at the owner, then the effect
at a local (334743-334751), the impact or its projection on the block edge
(334380, 334542). Thrown FUN_0031d688: same effect rule, the first victim's
position for HitSound (312042-312046), no Push. The world ray of FUN_0031cb78
(mask 0x90) only shortens the leg: it plays nothing.

[H] The victim's damage slot +0x1c was not opened for further presentation; the
port's Event::Hit consumers stand for it.

[D] The port's blade contact is `Event::SaberContact` at the same ray fraction.
The thrown blade's leg ends on contact: FEEL_NOTES, [ours].
