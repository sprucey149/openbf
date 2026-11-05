# Space Instant Action: source survey, 2026-11-01

Superseded for scoring and capital ships by `re-space-assault.md`; kept as the survey it was.

Surveyed source `f20c40c994476c1f73b0187d5a6ac969ba8a55fc`. This is a
static source/data survey, not a claim of playable space battles or PS2 runtime
parity. Numeric facts below were read from the PS2 mission bytecode or ELF;
the mod-tools Lua was used to understand the declarations, not to replace them.

## What exists and what is missing

The Instant Action catalog already contains the six PS2 space entries, with the
correct era and Assault/one-flag offerings. They are disabled because
`ui::shell::PORT_MODES` contains only Conquest. Assault's two option fields
already exist (`AssNumBots`, `AssScore`); their group is disabled too. Neither
the map list nor the basic launch-queue model needs replacing.

The bake can read nested map layers, world instances, collision and navigation,
and the scene accepts a map without terrain. The normal recipe explicitly
excludes SPA maps. The mission generator emits only Conquest. There are no
space map bakes or space mission RONs in the surveyed asset tree.

`main/battle.rs::make_walk_with_counts` installs mission layout, spawn posts,
vehicle pads, rosters, collision and the opening wave, but always constructs
the reinforcement/command-post battle. The current `TeamDef::tickets` is
unsigned, so the space scripts' `-1` infinite reinforcements cannot be written
unchanged into this schema. The sim has no space score objective. Critical
systems need damageable, name-addressed world entities; collision alone does
not supply health, shields, death or a scoring callback.

`assets::mode_layer_index` is a provisional name heuristic. Mode `ass` does
not match `*_Assault`, and the other space scripts request `*_Obj`. Carry the
actual mission-requested sublevel instead of adding increasingly broad guesses.
Space skies also come from a separate `SPA_SKY.LVL` sublevel, not merely the
map LVL; `bake map --sky-sublevel` adds it (`re-space-assault.md` §10).

## Confirmed PS2 mission data

The six Instant Action Assault missions are:

- Yavin: `spa1g_ass`, GCW, `spa1_Assault`, sky `yav`, height band −1900 to 2000 m.
- Kashyyyk: `spa3c_ass`, CW, `spa3_Obj`, sky `kas`, height band −1900 to 2000 m.
- Mygeeto: `spa6c_ass`, CW, `spa6_obj`, sky `myg`, height band −480 to 1800 m.
- Felucia: `spa7c_ass`, CW, `spa7_obj`, sky `fel`, height band −460 to 1300 m.
- Hoth: `spa8g_ass`, GCW, `spa8_obj`, sky `hot`, height band −480 to 1800 m.
- Tatooine: `spa9g_ass`, GCW, `spa9_obj`, sky `tat`, height band −480 to 1700 m.

The player and AI bands match in these six scripts. The last four load their
initialization from `spa6c_cmn`, `spa7c_cmn`, `spa8g_cmn`, `spa9g_cmn`.
All six set 16 units per team and infinite reinforcements (`-1`), with
`SetSpawnDelay(10, 0.25)`. Team 1 is Empire in GCW and Republic in CW. The
pilot class entry has count 25; the marine entry has count 6 except Yavin's
8. These are the script's class-count fields, not a proved probability.

`objectivespaceassault.luac` assigns 18 points to each life-support, bridge,
communications, sensor and frigate object. Engines total 18 per capital ship:
3 for each of six Rebel engines, 6 for each of three Imperial engines, 9 for
each of two CIS engines, and 18 for the Republic engine. Each internal
life-support/engine system is also registered for 18 points. Linked destruction
must preserve killer attribution, so an external/internal chain cannot award
an invented duplicate bonus. `FUN_0036adb8` (BF2) negates a critical object's
score when its team equals the killer's team. It returns zero without a killer
or object name. The scoring caller is `FUN_003616f0`; the Lua bridge is
`AddSpaceAssaultDestroyPoints` at `0x0018c848`.

The default score limit is 180: `SpaceAssaultGetScoreLimit` at `0x0018e6e8`
reads the session-indexed words beginning at `0x005fa898`; the five populated
ELF defaults there are all 180. The PS2 objective completes immediately from
the points-change callback once the limit is reached, then releases that
callback. Unlike the PC mod-tools source, it has no 4-second victory voice
timer: its matching prototype contains the limit check, `Complete` and
`ReleaseTeamPointsChange`, and no timer calls. Exact interaction with the base
objective's final victory delay still needs tracing. Fighter value 3 is stated by the existing
PS2 shell description; its executable kill-category path still needs checking.

`linkedshields.luac` defaults are **200000 maximum, 1000 add-shield**. The PC
mod-tools source says 10000 add-shield: do not copy that value. Yavin overrides
maximum to 500000; Kashyyyk explicitly uses 200000. The remaining four use the
PS2 default. Each linked object's AddShield receives the aggregate rate divided
by the number of linked objects. Damage synchronizes shield levels, zero drops
the controller and regeneration, and controller respawn restores shields.
The underlying AddShield tick/time law remains to be confirmed in the ELF.

The actual mission declarations also specify linked internal/external engine
and life-support destruction, shield controllers, defense mainframes and their
turrets, plus per-frigate turret deaths. Those are part of the objective,
not optional visual decoration.

## Ordered implementation after flight and bots

1. Extract an Assault mission with the existing Lua-5 reader. Resolve the
   explicit common-script includes and global `myGameMode` before reading
   `ScriptInit`; do not flatten every callback into startup actions. Preserve
   ground generation byte-for-byte. Add mission fields for requested world
   layer, sky, flight bands, infinite reinforcements and structured space
   targets/linkages. Prefer a focused `scripts/gen_space_missions.py` consumer
   of `gen_missions` helpers to changing ground semantics.
2. Add a targeted space bake path. Keep PS2 collision/gameplay properties;
   read the requested sky sublevel and preserve world-layer identity in the
   manifest. Load Yavin headlessly with actual hangar posts, pads, units and
   collision before enabling the menu; extend the same checks to all six.
3. Select Assault from the existing IA catalog and pass its bot/score options
   through the launch setup. Initialize a separate opt-in space rule state;
   leave ground battle update/hash streams unchanged when it is absent.
4. Install damageable systems by mission instance name and class properties,
   with projectile/blast hits routed from real collision bodies to system
   health/shields. Process linked destruction and repair/mainframe events once
   per state transition. Connect flyer kills and critical-system kills to
   team score and the actual round-ending path. Update HUD state from that
   same sim state; avoid a parallel display-only score.
5. Headless loaded-content checks: launch all six entries, board/take off,
   fly/fight, hit shields and systems using ordinary weapons, disable shields
   from inside and outside, destroy linked systems/frigates, and end at the
   score limit. Check repair and friendly/self/environment-kill controls.
   A direct helper-call fixture cannot establish those gameplay chains.

No one-flag implementation is proposed by this sequence; that remains the
catalog's existing disabled offering. No campaign space levels are added.

## Reproduction and remaining evidence

`python3 scripts/gen_missions.py --dump bf2 spa1g_ass` reads the original
bytecode and prints the recovered mission declarations. Repeat for the other
five and their common scripts. For defaults, call `calls50` with a caller-owned
`globs` dict and `recurse=False` on `objectivespaceassault` and `linkedshields`;
inspect the named template globals before child closures mutate them.
`python3 scripts/re_check.py word b2 0x5fa898 6 --u32` checks score defaults.
Review the actual callbacks and executable scoring chain before implementation.
No emulator/live-controller capture, rendering check or measured flight/objective
parity was performed by this survey.

## First isolated map load

A subsequent PS2-only SPA1 bake completed successfully: 163 models, 112
textures, 114 base instances, 46 regions, 89 paths and 104 collision geometry
records. The baseline game's CPU scene loader read it and constructed its
collision world without a renderer. This proves basic content compatibility,
not playability. It exposed these specific integration gaps:

- The baked sky was `None` then; the sublevel is baked now.
- The Assault wrapper contains worlds `spa1_swingassault`, `spa1_mp_turrets`
  and `spa1_mp_flyers`, with 65 additional instances. The existing mode
  selector picks it for `ass`, `assault` and `obj` by its size fallback.
  Preserve the actual resource/sublevel identity when baking.
- Two hangar posts, `mp_cp1` and `mp_cp2`, exist in that wrapper, but
  `MapLayout::from_manifest` finds **zero** posts: its classifier requires a
  `CaptureRegion`; these uncapturable posts have `ControlRegion` and
  `SpawnPath` instead. Eight vehicle pads reference those posts. Make the
  space installation recognize spawn-only posts without making them
  conquest capture objectives.
- `mungefs::path::parse_config_paths` discards all BF2 path/node properties.
  Positions survive; flyer path classes, speed and branch/landing properties
  do not. `ManifestPath` also lacks node properties and string-valued path
  properties. Add those fields before claiming authored spline flight.
- The Imperial space chaingun model has a soft segment spanning two nodes,
  which the existing turret-rig baker refuses. No visual correctness claim
  is made for it.
