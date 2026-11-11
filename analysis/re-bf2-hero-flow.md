# BF2 hero flow, health, presentation and AI

BF2 PS2 retail evidence. Addresses are EE virtual addresses in
`SWBFRNT2.ELF`; `B2:` numbers refer to `analysis/SWBFRNT2.ELF-decomp.c`.
`[V]` means a source or live observation checked against the original;
`[D]` identifies a deliberate port rule; `[H]` remains unresolved. Source
evidence and an implementation test do not establish visual acceptance.

## 1. Manager and session defaults

[V] The hero manager is `FUN_003ef9d0`, B2:448939–449407.
`FUN_003e6cf8`, cited by older notes, dispatches network messages; it is not
the hero manager. The manager owns the offer, eligibility and cooldown.

[V] The default arrays at ELF `0x5fa8f0` are indexed by **session ruleset**.
The name pointers at `0x5fa7c0` resolve to `instantaction`, `mp`,
`campaign`, `metagame` at `0x615a38/48/50/60`. `ScriptCB_SetGameRules`
(B2:43364–43400) hashes those names and calls `FUN_0019bfc8`
(B2:43336–43343). They are not conquest/CTF mode rows.

In that order, the arrays contain:

- enabled: `1, 1, 1, 0`;
- unlock rule: `3, 3, 7, 1`, unlock value: `10, 10, 1, 1`;
- team rule: `3, 3, 3, 3`, player rule: `1, 1, 1, 1`;
- respawn rule: `2, 2, 1, 1`, respawn value: `90, 90, -1, -1`.

Instant Action and MP therefore share the ordinary 10-point unlock and
90-second respawn defaults. Campaign uses the scripted unlock rule.

[V] `SetHeroClass` (`0x18cb18`, B2:32700) records the team's class through
`0x36e800`. `SetHeroUnlockRule` (`0x18cba0`) and `EnableSPHeroRules`
(`0x18ced8`) are return-zero stubs on this PS2 build. The team/player/
respawn setters (`0x18cba8`, `0x18cc90`, `0x18ce18`) have real gated
implementations. `EnableSPScriptedHeroes` (`0x18cee0`) sets enabled=1,
unlock=7, respawn=1/value=-1 when its SP gate permits it.
`UnlockHeroForTeam` (`0x18cf40`) sets that team's unlock flags.

## 2. Shipped mission scripts

[V] The disc's 128 extracted Lua chunks were undumped independently of PC
source names, retaining each chunk's SHA-256, every hero-related call and
the `SetupTeams` tables. This covers mission and helper chunks; the 19
`mcfg_*.bin` files are not Lua chunks. The abstract interpreter enumerates
potential calls across proto trees; it does not claim every callback runs
at mission start.

- All 33 conquest missions assign two hero classes: 66 assignments,
  18 normalized classes, 19 raw spellings. Felucia's `rep_HERO_aalya`
  needs case-insensitive class resolution. Dagobah, Death Star and
  Mustafar also contain hero assignments outside the infantry faction's
  source side, so roster loading must include the hero's own side.
- The 17 ground CTF scripts assign 34 heroes. The 22 one-flag scripts
  include 16 ground scripts with 32 assignments and six space scripts
  with no hero assignment.
- The six hunt scripts and six space-assault scripts contain no
  hero-related engine call in the retained census.
- The 18 campaign scripts contain ten `EnableSPScriptedHeroes` calls,
  11 class assignments and 11 unlock calls. Geonosis additionally calls
  `AcceptHero(0)`; Coruscant assigns cloaked Anakin. Unlock calls can be
  in objective callbacks rather than unconditional startup code.
- `tat2g_eli` implements hero assault through two 12-member normal
  rosters containing six heroes/villains each. It does not use a single
  `SetHeroClass` per team and explicitly disables hero music and VO.
  This is distinct from the conquest offer manager.

[V, limited absence] Neither the complete Lua constant tables nor the raw
chunk string scan contains `SetHeroUnlockRule`, `SetHeroTeamRule`,
`SetHeroPlayerRule` or `SetHeroRespawnRule`; positive controls find
`SetHeroClass` and `EnableSPHeroRules`. Thus none of the supported conquest
scripts overrides the session's 10/90 rule through those APIs. This does
not make a claim about another executable build or a dynamically loaded
mod. `[receipt:75241124]`

[D] The port currently supports conquest battles. This research does not
add CTF, hunt, campaign or hero-assault modes or impose conquest's manager
on an all-hero assault roster.

## 3. Offer, candidate and respawn

[V] The offer remains valid through age 30 seconds and expires only at
`age > 30` (B2:449003–449015). HUD `0x382ee0` also sets the 30-second
length. The cooldown is likewise strict `age > 90`, not `>= 90`
(B2:449044). Fixed-tick boundaries must preserve acceptance at exactly
30 seconds and require a later tick for cooldown completion.

[V] Candidate arrays separate humans and AI and prefer humans
(B2:449123–449218). Rank 1 uses ordinary score and draws game RNG for
ties. The ordinary SP loop first requires a non-declined human on the
team (B2:449058–449106), resetting its cycle otherwise. The presence of
an AI candidate branch does not prove that normal SP automatically fields
AI-only heroes. An explicit candidate rule may admit bots; idle test
controllers are not AI candidates.

[V] A normal single-player Mos Eisley Clone Wars trial retained 7,262
unique game-clock rows over 241.974 seconds. The AI-only opposing team's top
ordinary score rose 11→16; member 21 crossed 5→10 while remaining an
ordinary AI soldier. Its hero offer, owner and AI-candidate fields stayed
-1. The human on the other team received a fresh offer for 29.993 seconds under
the same 10-point/90-second session rules. Tickets were pinned to keep
the battle available, and the human's health was staged for the death
comparison. This supports the ordinary SP human-presence gate for this
session; it does not establish every multiplayer or campaign policy.

A second trial retained 7,262 unique clocks over 241.974 seconds with ordinary
human infantry alive throughout. The AI-only team's top ordinary score
rose 20→25 without a hero offer, owner or candidate. Its observation
started after the earlier natural ten-point crossing. Only the human's
health and both ticket pools were held to keep the scene available.


[V] `AcceptHero` (`0x18cfc8`, B2:32904–32937) calls `0x3ef6c0` and
removes the old controlled unit with respawn flags. Hero selection reads
ordinary points (`0x361900`, selector 1, stats+0x1fc); hero-earned points
have a separate subtotal at stats+0x1fe. A hero victim uses the five-point
event rather than the ordinary AI/human victim score.

[V] Accepting the offer removes the old controlled actor and opens the
normal spawn display (`AcceptHero` 0x18cfc8, B2:32904–32937). The spawn
map still requires a command post selection and Cross confirmation;
class confirmation then creates the actor (`0x398000`, B2:393045–393135).
`0x39a888` checks that heroes are enabled, the team is unlocked and its
owner is the viewport member. `0x399dd8` exposes the hero row only under
that predicate; `0x396a00` selects the first hero row for an owner and
resets a stale hero selection for everyone else. Owners may browse the
ordinary rows before confirming.

[D] Windowed play uses that existing map/class flow. The class list,
3-D lineup and automatic respawn share the same current-member ownership
check. A dead hero's old class choice cannot grant another hero life.
The existing screen-disabled/headless path still performs automatic
replacement for an accepted owner.

[V] Who is offered, and how long a hero is kept (FUN_003ef9d0 and its
hooks; playtest 2026-10-30):

- A human without a soldier is not offered: the selection clears a pick
  whose member+0x148 is 0 and tries again next update (B2:449210-449214;
  death clears +0x148, FUN_0034f650 B2:346045). The port does the same.
- A human who loses his soldier while the offer is up is accepted
  (B2:448998-449000). The port does the same.
- An unused hero is kept with no time limit. Spawning in another class leaves
  the owner word alone (FUN_0034e1f0 B2:345046-345058). Only the loss of a
  soldier of the hero class ends the ownership and starts the respawn clock
  (FUN_0034f650 B2:346003-346033 -> FUN_003ef328). The health drain runs only
  on the hero soldier, so the saved hero spawns with full health.
- The Yes press removes the soldier and flags the controller for the spawn
  screens (FUN_002b0548 B2:234886-234899), the same path as AcceptHero.
- The combo state hash slots use FUN_00539308, FNV-1a over `c | 0x20`, which
  is not lowercasing: `_` hashes as 0x7f. Under it combo+0x14 (0x8718d358) is
  CATCH_SABRE. The melee update enters that state on a caught saber when the
  posture field is 0 or 1, and holds IDLE while the saber flies
  (FUN_0033cda8 B2:333218-333249). [H] Field values 0 and 1 are read as stand
  and crouch; no RAM row checked them.

[V] The Hero Options rows reach the manager through
ScriptCB_SetNetHeroDefaults (B2:44471-44497): unlock rule, value, player rule
and respawn value are copied; a negative respawn value selects respawn rule 1
(no new offer after a hero was accepted), any other rule 2. Rule 5 unlocks at
`start + value < now` (B2:449356-449367). Player rule 7 has no case in the score
switch (B2:449132-449160): all score 0 and the LCG picks among them.

## 4. Health and death

[V] `FUN_00362268` contains the kill-heal rule (B2:359062–359071,
increment at 359249; MIPS `0x36240c`–`0x362488`). Before increment, the
first 15 enemy kills in the hero life heal 150 HP each; kills 16–30 heal
100; later kills heal 50. The class MaxHealth clamps the result.
The original stores an incrementing byte; death resets it in `0x363390`
(B2:359670).

[V] Live boundary trials at prekill counts 0/14/15/29/30, consecutive
14→15→16 kills and a near-max-health trial support those amounts and
the clamp. The original's simultaneous health drain explains the measured
149.666687/99.666687/49.666687 changes at approximately 30 Hz. A no-kill
control falls 480→420.331512 across 5.99853515625 retained game-clock
seconds, approximately 10 HP/s. This is PS2 behavior, not the port rule.

[V] Member update FUN_0034cea8 (B2:343951–343982;
0x34d0f4–0x34d1ac) subtracts game delta ×10 HP from an active human
hero's soldier health through FUN_001df3d8 with arguments 1,0. This retains
prior damage attribution and bypasses ordinary damage/armor modifiers. A
mounted hero still loses body health; exhaustion invokes its dismount path.
The health setter's authority/replay globals are not the damage invincibility
cheat: the latter is DAT_00600b1c in ordinary damage/conditional kill consumers.
Exact source/word/writer receipts are retained with the follow-up report.

[D] The 2026-10-30 maintainer decision supersedes no-time-drain for regular
BF2 conquest. Port heroes use authored MaxHealth, a time drain (the PS2's
10 HP/s, `heroes::PS2_TIME_DRAIN`; the port's default is 5 HP/s, [ours]
maintainer 2026-10-30),
normal damage and the original kill-heal gains. Zero and other finite
nonnegative rates remain data. [ours] Explicitly enabled AI heroes use the
same rate as humans for fairness; the native member drain is human-gated.
The port drains before replacements/respawn, leaving a new actor at full
health on its creation tick. Retail equal-bucket update order depends on
object addresses, so this is a deterministic port phase rather than a full
retail scheduler-equivalence claim. Health exhaustion uses normal death,
seat, ticket, drop and respawn bookkeeping without inventing a damage hit.

[V] Ordinary health pickups and stations call `0x369770` with bypass=0
(B2:361726/362473), so the SetHeroClass bit refuses their health. Kill-heal
passes bypass=1 (B2:359071). Ammo and energy use separate give functions
and remain eligible. A buff item is consumed even though the hero setters
zero its timers: `0x24eb18/70/c8/ec20` reject offense, defense,
regeneration and timed poison on the same hero bit. This applies to
officer abilities as well as item buffs; it does not identify ordinary
burn damage as a buff. The port uses these same eligibility rules.

[V] This is also the weapon-buff path: emitter `0x317838`
(B2:307028–307046) calls those four setters directly. Leia's
`all_weap_inf_buff_invuln_ord` inherits the ordinary buff emitter and
authors defense 10 seconds ×0.05 and regeneration 10 seconds ×20 HP/s.
The name does not select a separate invulnerability mechanism. It can
protect and heal ordinary allies; the hero bit still refuses its timers
on Leia and other hero recipients. Han, Chewbacca and Grievous likewise
carry ordinary offense-buff emitters.

[V] Hero death selects soldier state 16 / simple animation 26,
`stand_deadhero`. `FleeLikeAHero` is soldier-class+0x778 bit 4; class bit
33 also selects the hero death path. The source removes the body after
age >4 seconds. The authored item-drop tables remain in the ordinary
death path; there is no established hero-specific pickup suppression.

[V] In a controlled Instant Action Obi-Wan exhaustion, current health 20
was allowed to drain through zero without changing its class or death
flags. State 16 was visible at ages 0.3667, 1.0333 and 3.0333 seconds: the actor
kneels, retains the hilt in the hand and has no active saber blade. The
entity's old generation was gone by 4.2 seconds. The observed pictures show an
opaque kneeling body through 3 seconds, followed by removal and the spawn-screen
transition; they do not establish a gradual body-alpha fade. This is a
saber hero's exhaustion, not a gun-hero or thrown-death comparison.

## 5. Audio and caster animation

[V] Soldier-class hero sound tables are offer +0x1254 (key
`0x4ac2b220`), `SndHeroSpawned` +0x1258, `SndHeroDefeated` +0x125c,
`SndHeroKiller` +0x1260 and `SoldierMusic` +0x1264. The actual use of
`SndHeroKiller` is not established here. Melee key `0xde71b19f` stores
the looping hum handle at class+0x278; its property name is not proven.
It must not be mislabeled `AmbientSound`.
The 22 faction hero BASE closures contain zero authored offer-key
`0x4ac2b220` properties, with 20 `SndHeroSpawned` positive controls
[receipt:ad685817] [receipt:fa4ef411]. The constructor initializes the
offer handle to 0. No substitute voice is invented for these absent
properties. Vader authors `LowHealthThreshold=1.0`, so his breathing
continues at full health while alive; its own sample handle determines
when it repeats.

[V, bounded content gap] The 22 retained hero classes name 32 distinct
spawn/defeat properties. Both original TAT and MYG sound banks contain
all 32 definitions. Mundi's `hero_mundi_spawn` names segments
`0xe26ef839` and `0xdf6ef380`; `hero_mundi_exhausted` names `0x6772082f`,
all on `global_vo_slow` (`0xfe6eae40`). None of these three IDs occurs in
the original GLOBAL stream metadata's 31 streams / 6,523 segment records;
Obi-Wan's `0xa9eb1581` is a positive control in that same index
[receipt:974e7a32] [receipt:09e2d139]. These properties remain silent; no replacement voice
is invented. This is a scoped metadata observation, not a claim that the
property definitions or identifiers are absent from the executable.

The original BF2 Mos Eisley sound bank also defines
`hero_mundi_AcquiredTarget` four times. Each points to the same five
`global_vo_slow` segments: `e06ef513`, `e56efcf2`, `e66efe85`,
`e36ef9cc` and `e46efb5f`. The PS2 disc's GLOBAL and TAT stream
indexes contain 6,523 and 20 records respectively, with none of the
five IDs. The checked English, German and French PC GLOBAL indexes and
the Xbox disc's GLOBAL and TAT indexes also lack all five. Xbox has 6,074
GLOBAL and 20 TAT records. In each GLOBAL index, the Anakin call
`c02b98f3` and Obi-Wan spawn `a9eb1581` both resolve.

BF2 `FUN_005519c8` B2:685320–685390 selects the entry and can send
it to the stream queue. The actual segment lookup is later, when
`FUN_00551d08` B2:685446–685449 calls `FUN_00551128`
B2:684995–685034 to start it. The current baked Mos Eisley port accepts
Mundi's call at five metres, then finds no playable take, while all five
Anakin controls decode. The November 5 windowed exit also records a Mundi
playback attempt without a mixer handoff. The exact archived package's
audio was outside this bake comparison. Relative voice loudness
against the original remains open.

[V, bounded content gap] Tantive's Leia hurt/death properties both name
sample IDs `0x88f8eee4`, `0x89f8f077`, `0x06fbf3d5`. The original TAN bank's
714 sample records contain none of those IDs, although they do contain
the three underlying recordings [receipt:85dbafda]. TAT explicitly maps
those IDs through alias records to `0xad8e7efc`, `0xae8e808f`, `0xaf8e8222`;
TAN omits the aliases. This is a missing alias declaration, unlike Mundi's
missing stream recordings. The sample parser `0x54d9a8` uses callback
`0x54dd08`, which calls the loaded-bank exact-ID search `0x53e668` /
`0x53dfa8`. The native loaded-bank state has not been sampled live.

[ours, 2026-10-30] The TAN bake now repairs those three missing mappings
using original TAT/MYG aliases to the byte-identical PS2 recordings already
present in TAN. Strict map/bank/property/record guards preserve supplied
mappings and all PCM; no runtime fallback or external audio is required.
Both Leia hurt and death now resolve and decode all three takes in the
actual 66-assignment mission check. PC common.bnk also contains the three
requested IDs as distinct direct 22050 Hz recordings; Xbox TAT/MYG provides
direct takes too. These are verified PC/Xbox retail data alternatives, not
proof that those clips are identical to the selected PS2 recordings.

[V, bounded PC/Xbox retail data search] No matching Mundi segment IDs were
found in all 21 installed PC and 21 retail Xbox sound LVLs, the PC shared
bank or available German/French GLOBAL/GAL files. Spelling/property-name candidates identify
only the canonical dangling declarations. No verified recording was found
for a replacement; this is not a proof against opaque media under an
unidentified hash. Mundi's exact missing takes remain explicit.


[V] The melee parser `0x342528` stores `TurnOnSound` at +0x280 and
`TurnOffSound` at +0x288. Visibility function `0x3804b0` directly asks
`0x344180`/`0x3441b0` on visibility changes (B2:379401/379426).
The thrown saber's `OrdnanceSound`, key `0xccf9be5c`, is separately
parsed by `0x310620` into ordnance-class+0x9c. The actual class value
is `saber_twirl`; the loop follows the independent flight until catch or
explicit retirement. Primary deactivation `0x33b9c8` → `0x33dd40`
refuses blade extinction while its related throw is in flight. A same-generation
health death therefore preserves flight presentation; generation replacement,
explicit despawn and abandonment retire it.

[V] Push, pull and lightning weapon classes do not author
`SoldierAnimation` in the retained disc closure. The melee combo's
top-level `OffhandAnimation` maps `stand_useforce` to `FIRE`, `FIRE2`
and `CHARGE`. `0x309d10` parses these mappings; `0x30ab48` stores the
four action slots and `0x30aae0` retrieves them through `0x33c438`.
The soldier's animator calls this getter in `0x239bb0`
(B2:153915/153917/154871). The baked human saber use-force clip has
36 frames; BF2 duration is `(frames-1)/30`, not a guessed cast time.

[V] Choke's sustained EnergyDrainRate replaces the ordinary recovery rate.
The area weapon vtable `0x627f68+0x74` points to `0x32b958`, which is omitted from
the decompiled function list. Its MIPS checks Hold mode 1, weapon state 1
and active flag +0x158, then stores `-class[0x26c]` through the output pointer.
Soldier `0x240bf8` initializes recovery before that virtual call and applies
a negative override through resource subtractor `0x1e2010`. The actual value 15
therefore means net −15 energy/s while holding a valid target. A zero-rate
knockout restores ordinary +25/s while leaving Choke damage unchanged.
The earlier no-target trial could not establish sustained energy cost.
The resource subtractor sets the depleted latch on an overdraw below zero.

[V] Choke's `SoldierAnimation=choking` is a victim animation. The parser
`0x32d698` stores its dynamic index at class+0x280, and `0x32d198`
passes that field to the **target** soldier's `0x239850`
(B2:322635). The victim default CRC `0x144ebf32` is exactly
`human_choking`. The caster instead uses the combo's offhand fire pose;
it must not be given the victim neck-clutch animation.

## 6. Hero AI

[V] Shared weapon category `0x2a9b78` identifies melee as category 4.
`0x2aaba0` gives the melee-class branch desirability 1000 against the
supported target kinds, rather than a zero-projectile damage-rate score.
`0x2abc38` chooses close mode 7 for a non-scout melee actor, dispatched
to `0x2dac80` by `0x2d8f30`. Shared aim `0x2ace58` uses the wider
45-degree melee gate; the ordinary gate is 22.5 degrees. The normal
trigger/line-of-fire controller is `0x2bb1b0`.

[V] The base weapon-class constructor `0x327e98` defaults TriggerSingle
bit 50 to one: MIPS `0x3280b0/0x3280b4` constructs `1 << 50`,
`0x328154` ORs it and `0x328188` stores class+0x1e0. Later masks preserve
that bit. The melee constructor `0x341ed8` calls the base constructor
and does not replace those flags. `com_weap_inf_lightsaber` and its
retained children do not author an override. Native AI `0x2bb1b0`
therefore calls the existing single-trigger routine `0x2baf70`, producing
normal taps for repeated attacks. The port preserves explicit false
values and supplies true only for a missing BF2 melee key; other weapon
defaults are outside this change.

[V] Close-mode entry `0x2dac28` delays the first jump by random 0–3 s.
Within planar distance 5 m, `0x2dac80` chooses three direction branches.
MIPS `0x2daf6c`–`0x2daf84` proves the unusual branch swaps x/z;
the other two rotate plus/minus pi/2. Its 25 m probe is a navigation
heading test, not melee damage range or a teleport. Failure retries in
4 s; success cancels the route, asks the controller for a jump and
schedules random 5–10 s. Human and AI actors retain the same authored
jump/energy/collision constants.

[D] The port's conservative capsule/floor/barrier probe implements this
navigation request with its existing collision system and ordinary Pad
input. It does not grant movement, damage or energy to AI actors.

The shared gunner also consumes a Force/saber-throw slot selected by the
existing chooser: it switches the normal secondary channel, aims using
that class, and sends `SECONDARY_FIRE`. A cone's physical reach comes from
`ConeLength`; it is not zero because it has no travelling projectile.
Energy, perceived target, range, normal trigger and friendly-line gates
still apply. The disc's Push/Pull classes explicitly zero ordinary AI
target masks; a manually selected Push slot still respects that refusal.
Actual Choke and Lightning provide positive shared-consumer controls.
`[D]` A selected cone or broad thrown blade does not fire at
a token-barred human: the ordinary harmless aim offset cannot guarantee
that these wide attacks miss. No new random power-use policy is added.

[V] Controlled human input trials distinguish held and repeated fire: a held
press performs ATTACK1→RECOVER1→IDLE, while repeated presses chain
ATTACK1→ATTACK2→ATTACK3. Held block remains in DEFLECT after a long
release; two short presses toggle it off. The exact native FIFO/time
comparison is documented in `re-combo.md`. This establishes input/runtime
behavior, not the native AI's choice of when to fire.

[H] Explicit Force/throw selection predicates and a live hero-AI
combat/jump comparison remain open.
The observed 27-chunk combo census contains ordinary button/posture/
energy/timing conditions; it is not proof that the executable has no
additional AI policy. Gun heroes retain the shared gun AI.

## Reproduction receipts

Run from the repository root:

```sh
python3 scripts/re_check.py fn b2 32724 32850 32859 32880 448939 449003 449044
python3 scripts/re_check.py word b2 0x5fa7c0 4 --u32
python3 scripts/re_check.py word b2 0x615a38 48 --str
python3 scripts/re_check.py callers b2 FUN_0019bfc8
python3 scripts/re_check.py fn b2 359062 359071 359249 359670
python3 scripts/re_check.py fn b2 298400 298507 298827 332871 302506 198485
python3 scripts/re_check.py callers b2 FUN_0033c438
python3 scripts/gen_missions.py --dump bf2 tat2g_eli
python3 scripts/gen_missions.py --dump bf2 geo1c_c
```

The actual-content audio regressions enumerate all 22 hero classes,
15 saber programs and 66 conquest assignments through each battle's own
sound priorities. They check decoded takes, combo/hit/deflect event
resolution, saber visibility and generation changes, paid throw flight
and catch, same-generation health death and explicit retirement, breathing
repeat time, same-tick spawn/death voice queues and theme
start/end. The exact Mundi stream omissions and Tantive alias omissions
above are asserted separately with positive content controls. Offline
decoding and event routing do not establish heard output or native
loaded-bank state.
