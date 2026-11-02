# OPUS RE (2026-10-19) - [O] UNAUDITED

# RE: the AI driver waits for a gunner - UnitWaitSecondaryAgent, the need-gunner call's two sources, the answerers' boarding (BF1 + BF2, card GUNWT)

Card GUNWT, lane 2. Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`; ELF bytes
read with `scripts/tools/eedis.py`; live structures read out of 396 BF1 and 179 BF2
PCSX2 save states (`engine/artifacts/pcsx2/ref/states/` and every other `.p2s` under `engine/artifacts/`). Tags: [V]
read in the decomp / disassembly (lines cited), [M] measured in PS2 RAM, [D] derived, [H] not confirmed; `[O]` = the
load-bearing claims. Tools, outputs, claims and verdicts: `engine/artifacts/lead_tmp/gunwt/` (`tools/`, `ram/`, `wf/`).
Extends `re-squad-orders.md` 1, 4.4 and 5 (the type-4 order) and corrects its 5 / 9b (section 8 here).

## 0. Answer table

| | BF1 | BF2 |
|---|---|---|
| who waits | the AI that takes the pilot seat of a walker, hover or flyer: a new controller is made for the vehicle | the same three classes |
| window | 4.0 s from the controller's creation (controller +0x280) | 4.001 s from the wait agent's creation (agent +0x60) |
| starts | at the driver's first full think inside the window whose agent allows it (trooper, scout, repair, random, wait, fly agents) | when the controller's goal map next handles a goal 8-0xc |
| holds past the window | while a team-mate on foot holds a live boarding claim on the vehicle (refreshed each think, dead 3 s after the last) | the same; and, before any seat test, while the human player stands on foot within 12 m, or within 150 m facing the vehicle within 30 deg, up to 15 s after the start |
| ends | every seat occupied (at once, even inside the window); window over with no claim; MOVE OUT / AT EASE in reach (window := now - 2) | every seat occupied (after the human hold); window over with no claim |
| the vehicle | stands where it is (the mover's target = its own position) | not traced |
| what is heard | on entering the wait, "need gunner" with chance 0.3, whatever the seats | nothing (event 0x27 at 7-10 s has no line) |
| a second call source | a seated driver's trooper agent setting out for a post it is not at: its FOLLOW (0.3) becomes the same type-4 call | - |
| who answers | every friendly AI on foot within 30 m (not droidekas): "yes", then a board goal for that vehicle | - |

## 1. Who starts the wait (BF1) [O]

- **The window** [V]. The UnitController base constructor `FUN_002623c8` stores the float now + 4.0 at controller
  +0x280 when the controlled object's class (its +0x294 interface, slot +0x10) IsKindOf `EntityWalker`
  (`DAT_005868d8`), `EntityHover` (`DAT_00584ba8`) or `EntityFlyer` (`DAT_005849f0`), else 0 (B1:182703-182722; ELF
  0x26261c `lui 0x4080`, 0x26262c `swc1 $f0,640(s1)`; names B1:134740, 103029, 90743).
- **The controller is new at every AI pilot** [V]. The hover's pilot-changed virtual `FUN_001ef4c0` (data words
  0x5314d0 / 0x53db58 through the thunk `FUN_001f2408`) builds a `UnitMobileController` (`FUN_00264f08`, RTTI
  `20UnitMobileController`) for the controllable at entity +0x2a0 when the pilot at entity +900 has +0xe8 < 0, else a
  PlayerController (B1:100717-100760); the flyer's `FUN_001de160` builds a `UnitFlyController` (`FUN_00264a48`,
  B1:88659-88700). (When the globals `DAT_00518be4` / `DAT_00518c08` are set it builds none.)
- **The push** [V]. `FUN_002538a0`, the 7th of the 8 checks at the top of every agent update (`FUN_00253cd0`,
  B1:171977), builds goal 4 (`FUN_00263eb8`) when now < controller +0x280 and the current agent's vtable +0x60
  answers non-zero (`FUN_00252328`, B1:171781-171806). The +0x60 answers (ELF, `tools/agents.py`): trooper
  (`FUN_0024c370`: 1 except in its states 2 / 4 / 5, where a helper answers), fly, random, repair, scout and
  UnitWait agents 1; board, combat, follow, hold, unboard and WaitSecondary 0. The push replaces a combat / board /
  random / unboard / WaitSecondary goal and queues others behind it (`re-squad-orders.md` 4).
- Goal 4 -> controller state 4 in both the mobile (`FUN_00264f58`, B1:184855) and the fly map (`FUN_00264a98`,
  B1:184592); state 4 -> `FUN_00262bb8` builds `UnitWaitSecondaryAgent` (`FUN_0024cfa0`, B1:183004-183008). That
  constructor has one caller, goal 4 one builder and that builder one caller [receipt:b9427611] (`refs.py`: one `jal`
  each, no `j`, no data word).
- [M] 396 BF1 states: the 15 live waiting drivers have window - clock between -1.91 and +2.53 s (never above 4); 8
  drivers still in their trooper agent have it between +2.10 and +3.67 s, 0.3-1.9 s after their controller was made -
  the wait starts one to three thinks after the seat is taken (`ram/winhist.txt`).

## 2. The agent (BF1 `UnitWaitSecondaryAgent`, vtable 0x537300) [O]

- **States** [V]. The constructor sets state 1, next 2 (B1:166980-166993); the generic state tick `FUN_0023ef20`
  exits, switches and enters on a full tick (B1:156773-156806); the update slot runs only on full updates
  (`FUN_00243eb8`, B1:160411-160425: the 0.5 s think, `re-squad-orders.md` 3). So the agent enters state 2 once, at
  its first tick, and ends in state 100. Exit (+0xc, 0x24d0e8) is empty.
- **Enter state 2** (`FUN_0024d020`, B1:167009-167041) [V]: the move helper's +0x44 (ground: `FUN_0024fb48` -> its
  state 7, whose enter and update call `FUN_0023b450`: the mover's path dropped and its target set to its own
  position and facing, B1:153958-153975 - the vehicle stops; fly helper: state 5), then the type-4 order {the
  character, 4, now, the vehicle} through `FUN_002613f8(0.3, ...)` (ELF 0x24d0bc / 0x24d0c0 = 0x3e99999a). No seat
  test (section 4).
- **Update** (`FUN_0024d0f0`, B1:167045-167101) [V], after the 8 checks pass and in state 2, with the character's
  vehicle (character +0xb8):
  1. occupied seats (`FUN_002361e0`: the vehicle's seat list +0xc0, entries whose +0xdc is set, B1:150321-150342) ==
     the vehicle's seat count (+0xcc) -> window := now, done;
  2. else window < now (ELF 0x24d1bc `c.lt.s`) and no live kind-1 claim on the vehicle by a claimer of the
     character's team (`FUN_002416d0(vehicle, character +0xa0, 1)`) -> done;
  3. else wait on. The move helper then ticks (it holds the stop).
- **The flyer's wait** [V]: the agent's +0x4c, +0x50 and +0x58 slots (`FUN_0024d240` / `FUN_0024d2a8` /
  `FUN_0024d388`, B1:167104-167180) store 0 in the window and refuse when the unit is an `EntityFlyer`; they are asked
  by the 1st, 2nd and 4th checks (`FUN_002521e0` from `FUN_00252350`, `FUN_00252230` from `FUN_00252520` <-
  `FUN_00252998`, `FUN_002522a0` from `FUN_00252dc8`), each only when `FUN_00237fd8(..) == 0`. So a flyer waits until
  one of those checks has something to offer; a ground vehicle's wait is interrupted by them (it allows them).
- **MOVE OUT / AT EASE** [V]: the order handler, for types 0 / 1 and a current WaitSecondary agent (`DAT_00587970`,
  named at B1:167195), stores now - 2.0 in the window (`FUN_0024cfe8`, B1:171549-171557, 166996-167006; ELF 0x24cffc
  `lui 0x4000`): the wait then ends unless a claim holds it.
- **The window's writers** [V]: only the constructor (0 / now + 4), the update (now), the order poke (now - 2) and the
  three flyer slots (0); the setter at 0x264470 (`swc1 $f12,640(a0)`) is never referenced; the 19 other stores of
  displacement 0x280 in the ELF (27 stores, 7 loads) are to entities and other classes (`offscan_b1_0x280.txt`; both absence readers
  checked each base register) [receipt:d5b9d620].

## 3. The claims and the boarders (BF1) [O]

- **Records** [V]: `DAT_00517e70`, 60 records of 0x18 bytes {target, id, stamp, claimer, id, kind}
  (`FUN_00241358`); a record lives while now < stamp + 3.0 and both ids still match +0xb4 (`FUN_00241400`,
  B1:158523-158526), else it is removed; `FUN_00241930` adds one stamped now or restamps it; `FUN_002416d0` counts
  the live ones on a target with the claimer's +0xd8 == the given team and the given kind (B1:158680).
- **The board agent** (`UnitBoardAgent`, vtable 0x536730) [V]: entering state 2 with no claim of its own and
  `FUN_00241898(vehicle, team, 1)` < 1, it ends; else it claims (kind 1, B1:160800-160811). Each update restamps the
  claim (B1:160862-160866) and goes on only while the vehicle can be entered and the squared length of the vehicle's
  +0x48 vector is < 4.0 (B1:160874; [H] that vector is the velocity: it stops at 2 m/s), within reach it asks to
  enter (state 3), else it ends.
- **Free seats** [V]: `FUN_00241898` = the vehicle's +0x98 slot (EntityHover: its +0xcc, the seat count, ELF
  0x1efdd0) - live team claims - seats held by a PlayerController (`FUN_00241810`, `DAT_005882c0`), at least 0. AI
  occupants are not subtracted: a 2-seat hover with its AI driver offers 2.
- [M] RAM: every claim found on a waiting driver's vehicle is kind 1 on the entity at character +0xb8 - 0x2a0; the 5
  drivers past their window hold one aged 0.0-0.9 s, or are within one think of the window (`ram/census.txt`).

## 4. The call (BF1 only) [O]

- **Source 1, the wait** [V]: the enter above, chance 0.3, once per wait agent.
- **Source 2, a seated trooper's set-out** [V]: `FUN_002613f8` turns a type-3 order (FOLLOW) whose character has a
  vehicle (+0xb8) into type 4 with the vehicle as its entity, before the roll (B1:181862-181885). The trooper agent's
  enter issues that FOLLOW (0.3) each time it starts away from its goal (`FUN_0024b7a0`, B1:165886-165905;
  `FUN_00235258` = "inside the goal's radius", card UTALK). A vehicle's AI drives with the trooper agent: [M] of 1,756
  live agents whose character sits in a vehicle (every one controls that vehicle), 551 are troopers, 635 combat, 454
  UnitWait, 100 fly, 15 WaitSecondary (`ram/census2.txt`). Every new trooper agent of a driver - after a wait, after
  a fight, after a post - is a 0.3 chance of the call.
- **The issuer's gate** [V][M]: `FUN_00260f38`'s type-4 arm returns unless an entry of the ISSUER's own seat list
  (the soldier, character +0xb4 - 0x2a0, +0xc0) has an empty +0xdc (B1:181668-181683) - the soldier's one seat, whose
  controller moves to the vehicle when the soldier sits (`re-squad-orders.md` 1 step 2). RAM: all 9,800 live agents on foot
  have a one-entry list with +0xdc set, all 1,755 seated ones have it empty (`ram/issuergate.txt`). So the call goes
  out whatever the vehicle's seats: a full one-seat vehicle's driver calls too (two readers checked the enter: no seat
  test).
- **The answer** [V]: every friendly AI on foot within 30 m, not a droideka, answers "yes" and gets goal 5, board the
  order's vehicle (`FUN_00253118` case 4, B1:171669-171676; goal 5 -> state 5 -> `UnitBoardAgent`,
  B1:183010-183016). Then section 3: as many claim as there are free seats; the rest end at once.
- So the PS2's driver: takes the seat, stands, calls (0.3), the answerers walk over and claim, the driver waits while they
  come, leaves when the seats are full or 3 s after the last claim lapses; and each later set-out is another 0.3.

## 5. BF2 - the twin [O]

- **Constructor** `FUN_002cd610` (B2:255220-255243) [V]: state 1, next 2, vtable 0x6239a8; +0x60 = now + t, +0x64 =
  now + 15.0, +0x68 = now + rand(7.0, 10.0) (ELF 0x2cd650 / 0x2cd668 / 0x2cd670). One caller, the controller's
  state 3 in `FUN_002b70d0`, with t = goal +0x40 / 1000 (B2:239001-239007) [receipt:82af7252].
- **Who starts it** [V]: the BF2 controller constructor `FUN_002b6c20` sets byte +0x580 for an `EntityWalker`,
  `EntityHover` or `EntityFlyer` (B2:238808-238825); the goal map `FUN_002b88d8`, for goals 8-0xc with the byte set,
  clears it and queues goal 3 built by `FUN_002b6828(4.0)` = +0x40 4001 ms (B2:238484-238506, 240183-240190). The
  other byte clear (`FUN_002b7530`, a request of type 4 with 4.0) is a second route [H]; the shared request handler
  `FUN_002c3730` case 4 also builds the wait with a request's own time (the player's "wait for pickup").
- **Update** `FUN_002cd928` (B2:255346-255407) [V]: in state 2 with a vehicle: (1) when `FUN_002cd718` finds the
  human team-mate near it, it raises event 0x27 once after 7-10 s and waits while now < +0x64 - before any seat test,
  so even a full vehicle waits for the player; (2) else, seats not all occupied: waits while now <= +0x60, then while a
  kind-1 team claim lives (`FUN_002e0180`); (3) otherwise done.
- **The human test** `FUN_002cd718` (B2:255255-255344) [V][M]: a member (`DAT_00661330`, 0x1a4 each) with +0x130 >= 0,
  on foot (+0x14c, +0x150 zero), alive (+0x148), on the driver's team (the driver member's +0x134), within 150 m of
  the vehicle and either within 12 m or facing it (the member entity's matrix +0x20 row against the unit offset: > 0.866, 30 deg).
  [M] in all 179 BF2 states exactly one member has +0x130 >= 0, also in the 66 with up to 31 AI alive: the player
  (`ram/bf2human.txt`).
- **No call** [V]: the enter (0x2cd6b0, a hole) only stops the controller's +0x430 sub-agent; event 0x27 has no line
  (`re-unit-chatter.md` 4). Both readers of the twin agree on the shared substance.

## 6. The port today (main c85ac27)

- `Sim::need_gunner_call` (`core/src/sim/seats.rs`): one call (0.3) when a bot takes a pilot seat with a free GUN
  seat; the AI within 30 m answer "yes" and carry on; BF2 none.
- Card VTAI's hold (`ai/ride.rs::gunner_coming`, `GUNNER_WAIT` 8 s): a pilot holds the throttle for 8 s after a bot
  on foot claims the pilot's gun seat. The PS2 waits from the seat taken, holds on live claims, ends on a full vehicle.
- A seated bot's set-out calls nothing; nobody boards on a call; BF2's driver never waits for the player.

## 7. What the port needs (build)

Step A (both games): a pilot that takes the seat of a hover or walker (a flyer when the port flies them) starts a
wait at its next think: the vehicle stands; it ends when every seat is occupied; past 4.0 s from the seat taken
(BF2: 4.0 s from the start) only while a team-mate's board claim made in the last 3 s holds (the claim restamped on
each think of its walker); BF1 MOVE OUT / AT EASE in reach sets the window to now - 2 s. Replaces `GUNNER_WAIT`.
Step B (BF1): the need-gunner call (0.3) on the wait's start AND on every seated set-out; no seat test (the PS2 has
none; the port's free-gun test becomes a named deviation only if the maintainer wants it). Every friendly AI on foot within 30 m
answers and, at its next think, claims a free seat (seats - live claims - human-held seats) through the port's claim
rules, walks to it, and gives up when the vehicle moves faster than 2 m/s.
Step C (BF2, card GUNWH): the human hold - the player on foot within 12 m, or within 150 m facing it within 30 deg,
holds the wait up to 15 s from its start, full or not.
Acceptance: UTALK's census, TAT1 yes 12.0 / min and need-gunner gate 4.25 / min within 30 % (NAB1 2.0 / 2.75); a
scene: a 2-seat hover's new driver stands, a bot 20 m off boards it, it leaves when full; nobody near - it leaves
4 s after the seat was taken; the sweep's boarding counts sorted NOISE / EXPOSED / CAUSED.

## 8. Corrections

- Card GUNWT said the agent's update is a hole at 0x24d0e8 (slot +8): 0x24d0e8 is the empty exit (slot +0xc); the
  update is slot +0x38, `FUN_0024d0f0`, decompiled. BF2's update `FUN_002cd928` is decompiled; only its enter is a hole.
- `re-squad-orders.md` 5: the trooper's FOLLOW fires when it "sets out for a post it is not at" (`FUN_00235258` =
  inside the goal's radius, card UTALK), not when "its first path leg fails"; and from a seated trooper it IS the
  need-gunner call (section 4). The waiting driver's call is one of two sources, and nothing gates the waiting
  driver's line on a free seat. 9b: the AI lines are built (UTALK, e22e702); the boarding is card GUNWT.
- UTALK's note "the port's moment is the seat taken [H]": the PS2's is the wait's start, one to three thinks later.

## 9. Checks run

- **fn**: every cited line in its function (`checks_fn_b1.txt` 43 rows, `checks_fn_b2.txt` 17 rows, 0 misses).
- **absent** [receipt:b9427611] (the constructor, goal 4, its builder, the poke), [receipt:d5b9d620] (the setter /
  getter and the vtable words, `--elf`), [receipt:82af7252] (BF2's constructor and goal builder); `refs.py` for `jal` /
  `j` / data words; `offscan.py b1 0x280` (34 hits), `b2 0x580` (5).
- **twin**: BF2's constructor, update, human scan, goal builder and flag opened with their callers.
- **word**: vtable slots 0x537308 / 0x53730c / 0x537338 / 0x53734c-0x537360 / 0x537018, BF2 0x6239b0 / 0x6239e0;
  the names; 4.0 / 2.0 / 0.3 / 3.0 / 15.0 / 7.0 / 10.0 as `lui` immediates (`elf_evidence.txt`).
- **writers**: the window (section 2), BF2's +0x580 (B5).
- **Verifier** (workflow `gunwt-re-verify`, 6 agents, claims and citations only, `wf/verdicts.json`): 29 claims;
  three batches (24) + a second twin reader (7) + two absence readers (5 each). 24 SUPPORTED by every reader; 5
  PARTLY, all narrowed or corrected here: the 2nd (not 3rd) check asks slot +0x50, plus the `FUN_00237fd8` gate; the
  team and facing words of the human scan; "human" and BF2's start (now [M] and the goal-8-0xc moment); the issuer
  gate a second reader read as a seat count (settled by the RAM census of section 4); the update's event 0x27 call.
  On 4 claims the two readers split (one SUPPORTED, one PARTLY): the issuer gate, the BF2 update's event call, the
  human scan's words and the twin's start - each re-read and narrowed here. None NOT SUPPORTED.

## 10. Stayed [H] / not done

- The board agent's +0x48 vector as the velocity (2 m/s); `FUN_00237fd8`'s meaning; when exactly a flyer's wait is
  cut (which check fires first); BF2's `FUN_002b7530` route; how BF2's vehicle holds while waiting.
- No live PCSX2 run: the call and boarding rates are from the code and the save states, not watched.
- The 30 m answer set is `re-squad-orders.md` 1 step 6 [O], not re-read here.

## 11. Reproduction

```
python3 engine/artifacts/lead_tmp/gunwt/tools/agents.py                       # the 13 agent vtables and slots
xargs -a engine/artifacts/lead_tmp/gunwt/ram/all_states.txt python3 engine/artifacts/lead_tmp/gunwt/tools/agentram.py
xargs -a engine/artifacts/lead_tmp/gunwt/ram/all_states.txt python3 engine/artifacts/lead_tmp/gunwt/tools/issuergate.py
python3 engine/artifacts/lead_tmp/gunwt/tools/bf2human.py
python3 scripts/tools/eedis.py bf1 0x24d020 0x24d238   # enter + update
```
