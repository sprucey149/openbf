# OPUS RE (2026-10-13) - [O] UNAUDITED

# RE: unit chatter (BF1 + BF2) - every line a unit says, what asks for it, and what thins it out

Card USND (loop lane 2, max effort). Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1), `analysis/SWBFRNT2.ELF-decomp.c`
(B2), both ELFs, every LVL of both discs (class chunks and sound scripts), three BF2 save states, and twelve live
PCSX2 captures of four minutes each (BF1 Naboo Plains and Dune Sea, BF2 Utapau and Mygeeto, Clone Wars; eight are
tabled in section 5, three early ones lost the player and serve only section 2.6). Tags: [V]
read (lines cited; each found inside the named function by `re_check.py fn`, 66 of 66), [L] measured live on the PS2,
[D] derived, [H] hypothesis; `[O]` marks the load-bearing claims. Already read elsewhere and not repeated: the voice
queue (`re-announcer.md` 1), the squad-order lines and the AI's own orders (`re-squad-orders.md` 2, 3, 5, 7), the
3-D level (`re-sound-distance.md`), the dead movement keys (`re-foley.md` 5), the jet pack and droideka loops
(`re-vehicle-sounds.md` 3-5). Scratch, tools, captures: `engine/artifacts/lead_tmp/usnd/`.

## 0. Headline

**BF1 talks all the time, BF2 only round the player.** In BF1 a unit's line is a class key, played at the unit,
whoever is near; the battle's one voice slot is busy without a break - 32 lines a minute in all four BF1 captures,
whether the player stands at his post or in the fight. In BF2 a line is a `VOSound` entry picked by event and
context, and the speaker must stand within 50 m of the event's subject, which nearly every caller leaves as the
local player: standing at his spawn post the PS2 says one line in four minutes, in the fight 8 to 12 a minute.
Hurt and death cries are sample-bank one-shots outside the voice queue in both games, 30 to 175 a minute in range.

| BF1 event | class key | what asks for it | gates (data) | PS2, per minute [L] |
|---|---|---|---|---|
| hurt cry | `HurtSound` | a hit that takes more than 2 % of the health left and does not kill (2.7) | chance 0.85 (Wookiee, natives 0.8); 7-200 m | 41-70 in range; 17-19 within 50 m in the fight |
| death cry | `DeathSound` | the killing hit, or a forced kill (2.7); cuts the unit's own hurt cry | chance 1 (Wookiee, natives 0.8) | 17-35; 9-11 within 50 m |
| "there he is" | `AcquiredTargetSound` | the AI's target tracker takes a target (2.2) | one per 20 +- 5 s per property (`all_`, `imp_`: 10 +- 5) | asked 5-7.75, said 3.5-4.5 |
| "grenade!" | `FleeSound` | a live grenade, thermal, haywire, beacon or sticky has an AI soldier within 7.5 m (2.3) | 20 +- 5 (`all_`, `imp_`: 1 +- 0) | asked 2.5-5.25, said 1.75-2.75 |
| "heard something" | `HeardEnemySound` | the AI's hearing check turns the unit to a heard enemy (2.4) | 20 +- 5 (`all_` 1 +- 5, `imp_` 10 +- 5) | asked 3.75-5.5, said 2-3.75 |
| the AI's squad talk | `AISC...Sound` | `re-squad-orders.md` 5 | 20 +- 5; the "yes" 0 | follow 3-5.75, need-gunner 1-3.5, yes 1-2 (Naboo) / 12 (Dune Sea) |
| a native's idle voice | `LowHealthSound` with `LowHealthThreshold` 1.1 | always "low on health": asked again whenever its handle is idle (2.8) | chance 0.2, takes locked 1 s | Gungan takes: 16 of Naboo's 32 lines (the heartbeat shares takes with their other calls) |
| never said | `HidingSound`, `ApproachingTargetSound`, `PreparingForDamageSound` | no code calls them (2.6) [receipt:69e3eb1c] | - | 0 in six captures (their gates never armed) |
| never said | `ChargeSound`, `ShockSound`, key 0x5625fc0b | called; no soldier class authors them (2.5) | - | 0 |

| BF2 event (index) | what asks for it (3.5) | speaker | PS2 in the fight, per minute [L] |
|---|---|---|---|
| hurt / death cry | as BF1 plus a damage-flag bit (3.6) | the unit | 84-139 / 26-36 in range; 32-50 / 10-12 within 50 m |
| AcquiredTarget (1) | the target tracker takes a target; 71 in 101 | the unit | 6.25-8.5 |
| FriendlyFire (2) | hit by a friend | the victim | 1.25-1.5 |
| RepairStart / RepairEnd (0x10 / 0x11), GivePowerup (0x12) | an AI engineer starts / ends a repair; an AI hands out a powerup | the unit | 0.25-1 / 0 / 0 |
| Grenade (0xa) | BF1's "grenade!" rule | the first AI soldier in 7.5 m | 0.25 |
| NearbyEnemy / FriendlySlaughter (0x24 / 0x23) | four deaths of one kind inside 8 s within 25 m of an AI unit | that unit's nearest team-mate | 0-0.25 |
| HeadshotKill (9), KillingSpree4 / 8 (0xc / 0xd) | a HUMAN's kill (3.5) | a team-mate near him / his own unit | not seen (the player did not fire) |
| NotShootingCriticalHit (0x21), PathFollowWait (6), RebelsShootDeadBody (0xb) | 3.5 | 3.5 | not seen |
| SC_... (0x16-0x20) | `re-squad-orders.md` 7 | - | responses 0-0.75 |
| never said | `MissileIncoming` (5) and the eight `...VO` request names: authored, never fired; Kill, KillingSpree12 / 16, NearbyTeamKillSlaughter and seven unnamed events: fired, no line authored (3.5) | - | 0 |

## 1. What thins the lines out [O]

1. **The play gate** [V]. Every sound property holds four floats {probability, interval, dev, next} (B1 props +0x54,
   B2 props +0x5c). B1 `FUN_0046bf10(gate, noProb)` (B1:532405-532436), B2 `FUN_0055c9a0` (B2:693678-693709), the
   same statement for statement: a request while next > now (the sound clock) is refused; otherwise, when interval or
   dev is non-zero, next = now + interval + 2 (rand - 0.5) dev - and only THEN, when the probability is not 1, the
   request is refused with chance 1 - probability. So a refused roll still spends the interval, and a property with
   interval 0 never writes `next`. The gate belongs to the PROPERTY, not the unit: one `cis_inf_com_chatter_acquired`
   for the whole battle. It runs for sample one-shots (B1 `FUN_004599e0` 518936, B2 `FUN_0054cba8` 681929) and stream
   lines (B1 `FUN_0045ec68` 522515, B2 `FUN_005519c8` 685326); two more play paths use it (B1:518868, 522429).
2. **The take's own lock** [V]. A take is the same four floats, asked with noProb = 1. BF1 `FUN_0046b290`
   (B1:531791-531840) makes ONE draw - the take after the last one when the list's byte +8 is set, else by weight -
   and a locked take makes the whole request silent. BF2 `FUN_0055bf60` (B2:693184-693246) steps to the next take
   until one is open or it is back at the first (693234-693238), under four draw modes (byte +8 = 1..4). The stream
   path draws the take BEFORE the property gate (B1:522512 then 522515), the one-shot path after it (518936 then
   518940): a stream line the property gate refuses has already spent its take.
3. **Stream or sample** [V disc]. The combat calls, the AI's squad talk and BF2's `VOSound` lines are
   `soundstreamproperties` on the battle's voice stream: they go through the queue of `re-announcer.md` 1 (one line
   at a time, four pending, descending 3-D level, a commander line ahead of all of them). Hurt and death cries, every
   Wookiee line and the natives' cries are `soundproperties` in the sample bank: 3-D one-shots on the 78 voices.
4. **The data** [V disc] (`tools/sndcensus.py`, every sub-level of every SOUND LVL, `Inherit` followed into CORE.LVL;
   `bf1_snd_all.tsv`, `bf2_snd_all.tsv`):

   | family | kind, range | chance | interval | takes |
   |---|---|---|---|---|
   | BF1 `cis_` / `rep_` acquired, approach, flee, heard, hide, predamage | stream `cw_vo`, gain 0.8, 16-150 m | 1 | 20 +- 5 | 2-4; some locked 5-25 s (`rep_` acquired 25 +- 5, `rep_` flee 5, heard 15) |
   | BF1 `all_` / `imp_` the same six | stream `gcw_vo`, gain 0.6, 16-150 m | 1 | acquired, approach 10 +- 5; flee, hide, predamage 1 +- 0; heard `all_` 1 +- 5, `imp_` 10 +- 5 | 2-5, unlocked |
   | BF1 Gungan `gungan_chatter_*` | stream, gain 0.5, 7-150 m | 1 (heartbeat 0.2) | 20 +- 5 (heartbeat 0) | 3-7, locked 1 s |
   | BF1 `ai_<s>_..._tac_*` | stream (the voice slot), 7-150 m | 1 | 20 +- 5; yes / no 0 | 2-7, locked 1 s (`all`, `imp` yes: 2 s) |
   | BF1 + BF2 `<s>_inf_com_chatter_wound` | sample, gain 0.9, 7-200 m | 0.85 | 0 | 3 |
   | BF1 + BF2 `..._death`, `droid_death` | sample, gain 0.9, 7-200 m | 1 | 0 | 3 (droid 6) |
   | BF1 Wookiee `..._wookie`, natives' hurt / die | sample, 7-200 m | 0.8 | - | 1-2 |
   | BF2 `<s>N_inf_chatter_<Event>...` | stream `<s>_unit_vo_slow`, gain 0.6, 16-150 m | 1 | `cis`, `imp`, `rep` 40 +- 20; `all` 80 +- 40; `wok` 30 +- 5 | 3-9; `cis` locked 10 +- 3 s, `rep` mostly unlocked |
   | BF2 `<s>_command_*`, `<s>_response_*` | stream `<s>_unit_vo_quick`, 16-150 m | 1 | 0 | 3-7, locked 1 s |

   [L] Each `cis_` / `rep_` / Gungan `acquired`, `flee` and `heard` property re-armed 1.25 to 3.0 times a minute; a
   20 +- 5 s gate asked without a pause gives 3.0. The requests arrive every few seconds: what the listener hears is
   the property's interval, not the AI event's rate.

## 2. BF1 [O]

### 2.1 The keys [V]

`EntitySoldierClass`'s setter `FUN_0020f2e0` stores one sound handle per key; the seven combat handles are each played
by one function, a slot of the `EntitySoldier` vtable 0x532d50 (ELF words 0x532dfc-0x532e14), as
`FUN_00354fe0(class + off, unit position, 0, 0, 1)` - 3-D at the unit, queue flag set. No `jal` reaches any of them.

| key (hash) | class | line | slot, function | called from |
|---|---|---|---|---|
| `AcquiredTargetSound` 0x399ee477 | +0x1da8 | B1:123048 | +0xac `FUN_0020cd18` | `FUN_0024d6d0` B1:167335 |
| `HidingSound` 0xc007b675 | +0x1db0 | 123567 | +0xb0 `FUN_0020cd68` | nothing (2.6) |
| `ApproachingTargetSound` 0xeb6089a1 | +0x1db8 | 123523 | +0xb4 `FUN_0020cdb8` | nothing |
| `FleeSound` 0x94353bf8 | +0x1dc0 | 123310, 123493 | +0xb8 `FUN_0020ce08` | `FUN_00236be8` 150924 |
| `PreparingForDamageSound` 0x130971ae | +0x1dc8 | 122871 | +0xbc `FUN_0020ce58` | nothing |
| `HeardEnemySound` 0xd3c54780 | +0x1dd0 | 123589 | +0xc0 `FUN_0020cea8` | `FUN_00252dc8` 171455 |
| `ChargeSound` 0x4ee58508 | +0x1dd8 | 123201 | +0xc4 `FUN_0020cef8` | `FUN_00253a18` 171910 |
| `AmbientSound` 0x65a920ba | +0x1de0 | 122853 | - | the soldier update, 116840 |
| `ShockSound` 0xf66784c8 | +0x1df4 | 123543 | - | `FUN_002051a0` 115948 |
| 0x5625fc0b (name not found) | +0x1dfc | 123118 | - | 116054, 116217, 116319 |
| `LowHealthSound` 0xf8d3e5d2 | +0x1e04 | 123548 | - | the soldier update, 116861 |
| `HurtSound` 0x72981af7, `DeathSound` 0x6a9d502c | +0x70, +0x68 (base class, `FUN_00221e48` 135984, 136071) | | - | 2.7 |

### 2.2 "There he is": AcquiredTargetSound [V]

`FUN_0024d6d0` (B1:167320-167338) stores the tracker's new target and, when it is not null, calls slot +0xac on the
controller's entity. Its only caller is the tracker's update `FUN_0024d7a0` (B1:167390, 167409; `jal` 0x24d814,
0x24d8c4): with no current target; when the current one fails `FUN_002218b0`; when another candidate rates better
than the stored value (167419); and when the kept target's time has run out (167427, tested from the second pass
with a different candidate). There is no test that the new target differs from the old, and none for a soldier: only
`EntitySoldier` fills the slot with a sound.

### 2.3 "Grenade!": FleeSound [V]

`FUN_00236be8` (B1:150870-150939), called from slot +0x40 of `OrdnanceGrenade`, `OrdnanceBeacon`, `OrdnanceHaywire`
and `OrdnanceSticky` (`FUN_0027b428`, `FUN_0027f988`; vtables 0x53a1f8, 0x539868, 0x53a390, 0x53aa18). Per live
ordnance: after a delay from its first timer - uniform in (1.1, 2.1) s when the enemy difficulty word `DAT_005875f4`
is under 14, (0.6, 1.3) under 17, else (0, 0.8); the word's own default is 14 (`FUN_0023a540` 153290) - and at most
once a second (150903), the entities within 7.5 m (`lui 0x40f0` at 0x236cf0) are listed; every AI controller on them
is told the position (virtual +0x30), and the FIRST listed entity that has an AI controller and is a soldier class
(`DAT_005862e0`) says the line, once per pass. A human's soldier never says it.

### 2.4 "Heard something": HeardEnemySound [V]

`FUN_00252dc8` (B1:171354-171480), the fourth of the eight checks an agent's update runs in order (`FUN_00253cd0`
171972-171978; the fifth is the squad order of `re-squad-orders.md` 3). After two early returns (171375-171400) it
walks the controller's list of heard things (`FUN_00263678`); for the first that `FUN_002522a0` accepts and that lies
inside the unit's range `FUN_00239cc0` (halved for class kind 3) it builds a goal toward the place (171450-171452)
and calls slot +0xc0.

### 2.5 Called, and silent [V]

- `ChargeSound`: `FUN_00253a18` (B1:171847-171919; 12 call sites in the goal functions) counts soldier entities with
  an AI controller of class `DAT_00588348` within 7.0 m (`lui 0x40e0` at 0x253ae0), stops at three (`slti 3` at
  0x253bd4) and with three calls slot +0xc4, again at every new time stamp. No class with a `soldier` root authors
  the key: 0 of 59; the disc's 33 `ChargeSound` chunks are weapon and command-post classes [receipt:acaecc7a].
- `ShockSound` and key 0x5625fc0b have play sites (table) and no author: 0 PROP chunks and 0 raw words on either
  disc (two verifiers' own scans) [receipt:acaecc7a].

### 2.6 Parsed, and never called [V] [receipt:69e3eb1c]

No code calls slot +0xb0, +0xb4 or +0xbc on a unit [receipt:69e3eb1c]. The ELF holds 11, 1 and 2 virtual calls
through those offsets (`tools/vcall.py`: `lw rX, off(rY)` + `jalr rX`; two verifiers' wider scans - moves followed, `jr` tail calls -
return the same sites): 12 load the vtable from `obj + 0x14` of a weapon (`FUN_00284318` ... `FUN_002940d8`; RTTI
`Weapon`, `WeaponCannon`, ...), one is a stored function pointer (`FUN_003eb4f8`). The three functions' addresses
occur once each in the ELF, in the soldier vtable. Not excluded by any word scan: a call by pointer-to-member with a
computed slot; none was found. [L] In six BF1 captures the 6 to 9 `..._hide`, `..._approach` and `..._predamage`
properties in RAM had never armed their gate three minutes into the battle (`next` = 0) and did not in the four
minutes sampled, against 42 to 74 re-arms of `acquired`, `flee` and `heard`. Their takes ARE heard: several are
shared with `flee` (`cis`, `rep`, `all`, `imp`) and with the Gungans' heartbeat, so a census by audio names them
(`audio-census.md` 2 lists `gungan_chatter_approach`: a take the heartbeat shares).

### 2.7 Hurt and death [V]

- `FUN_002219d0` (B1:135782-135802) plays class +0x70 and `FUN_00221930` (135756-135776) class +0x68 on ONE handle
  per entity (entity +0xec): a sound still on it is stopped first, so a new hurt cuts the last and death cuts hurt.
- The damage function `FUN_001bc190` (B1:64287-64448): armour and shield first; the health damage is subtracted at
  64384; `tiny = damage <= health_after x 0.02` (64387; 0x3ca3d70a at 0x1bc498); alive bit (flags +0xa4, bit 3)
  clear -> death (64432-64433); else not tiny -> hurt (64435-64436). A hit the shield swallows, or one with no health
  damage (64380-64382), plays neither.
- A soldier enters through `FUN_00200c30` (B1:112668-112786): nothing for a soldier already dead (112695), then
  `FUN_001bc748` -> `FUN_001bc190` (112767, 64504). It does not take the generic object's `FUN_001c0858`, which plays
  class +0x70 a second time as a one-shot with no 2 % test (67195; the generic objects' path, e.g. `EntityTrap`).
- Death is also played by the kill routine `FUN_001bbc98` (B1:64127-64191; 29 call sites, e.g. the soldier update):
  health above 0 -> the death sound, health = 0. `FUN_001bbe00` is the network twin of the damage function.
- NOT these keys: the +0x68 / +0x70 sounds of `FUN_00208b00` / `FUN_00209018` (B1:118158, 118473) belong to the
  soldier's `FoleyFXSoldier` object (`FUN_00204fd8`, hash 0xd098618c at 115840) - foley, card FOLY.

### 2.8 Low health is the natives' idle voice [V][L]

The soldier update `FUN_00205cc8` (B1:116846-116861): not alive, or health above MaxHealth x `LowHealthThreshold`
(class +0x1e0c, key 0xc865523e, constructor default 0.3 at 121978) -> an active handle (soldier +0xc30) is stopped;
otherwise, when the handle is idle, `LowHealthSound` is asked on it. The 12 classes that author the sound all set the
threshold to 1.1 (Vader 1.0): three Gungans (`gungan_chatter_heartbeat`), three Ewoks (`end_inf_ewok_breathing`), the
Jawa (`tat_inf_jawa_chatter`), four lightsaber hums and Vader's breath - so the "low health" voice runs for as long
as they live, asked again whenever the handle is idle. [L] On Naboo the Gungans' line takes half the voice slot (16 of
32 lines a minute; the pending queue stands at its cap of 4). `AmbientSound` (116840-116844) starts the same way
whenever its handle is idle; one soldier class authors it (a saber hum).

### 2.9 Nothing follows a kill [L]

BF1's soldier class has no kill, streak or praise key (2.1; the 30 squad-order keys are `re-squad-orders.md` 2). Live,
Naboo with the PLAYER fighting (`ps2/run_kill.sh`: `ps2live.py`'s playing bot under the sampler; 15 kills and 48 hits
in 150 s, the longest run of kills inside 4 s steps was 2): 99 voice lines, each one a call of 2.2-2.4, the AI's
squad talk, a Gungan take or a commander line (`ps2/k1_nab1.csv`, `k1_nab1_fight.csv`). One map, 15 kills: a line
said only after a longer streak is not excluded. BF2's kill lines are 3.5's.

## 3. BF2 [O]

### 3.1 The class keys [V]

The game-object class setter `FUN_0026a340`:
- `VOSound` 0x172645b9 (B2:186712-186735): tokens split on ` ,+-;` - a sound name, an event name, then condition
  tokens. `FUN_002e5808` (271506-271541) makes a 24-byte entry {handle, [2] event index, [3] condition word, [4]
  next} appended to the class's list at +0x364. The index is the name hash's place in the 40-word table at 0x624ed8
  (`FUN_002e57a8` 271488-271503) and **0 for a name that is not in it**.
- `VOUnitType` 0x6196493c -> +0x368, 0..255 (186881-186894). `VORadius` 0xc9239720 -> +0x36c, max(0, x)
  (187069-187073); the constructor's default is **50.0** (`lui 0x4248` at 0x269ec8; B2:186496) and no class on the
  disc authors the key (0 raw hash hits in 481 files) [receipt:63a2d027]. [L] RAM: 50.0 on 89 of 89 soldier class
  objects in three states.
- `DeathSound` -> +0x314, `HurtSound` -> +0x31c, `HurtFallSound` 0xf3697818 -> +0x324 (186903, 186967, 187142; the
  last authored by no class [receipt:63a2d027]); `DamageRegionSound` 0x7241c3bb (186673-186704): `"%s %s"`, a sound
  and a name, into the list at +0x32c (`repmalechoke`, `impmalechoke`, `allmalechoke`, `allfemalechoke`; the CIS their
  wound sound) - the cry inside a damage region [H: its reader was not opened; no showcase map has one].
- **A child class shares its parent's list** [V, a verifier's finding; L]: the class copy `FUN_00269f38`
  (B2:186600-186602) copies the head pointer, and a child's own lines are appended to that shared chain. RAM, Mygeeto
  and Utapau: 13 soldier classes hang on one 197-entry chain; Hoth: ten chains of two classes. The side's
  `..._default` parents author no line, so each trooper family starts its own chain.

### 3.2 A line's conditions [V]

`FUN_002e5c08(subject, object)` (B2:271654-271782) builds the context word: bits 0-1 the object above / below
(more than 3 m and a slope over 0.25), bits 2-3 behind / left / right of the subject's facing (+-0.7071), bits 4-5
nearer than 10 m / 30 m / beyond 100 m (`FUN_002e5920` 271546-271649); bit 6 the object under a quarter of its
health, bit 7 the subject; bits 8-10 what kind of thing the object is (hover, walker, flyer, turret, ...), bits 11-13
what the subject sits in, bits 14-15 from the subject's controller; byte 2 the OBJECT's class `VOUnitType`. An entry
passes (`FUN_002e5f90` 271787-271815) when each of its condition fields is 0 or equal to the context's; among the
passing entries the one with the most matching fields wins (`FUN_002e6060`, sorted by the comparator at 0x2e6130).
So `rep1_inf_chatter_AcquiredTarget_VOUnitNum83` is the clone's line for a target of unit type 83, and the plain
`..._AcquiredTarget` the fallback. 2,131 of the disc's 11,667 lines carry no condition.

### 3.3 Who speaks [V]

`FUN_002e6180(event, speaker, subject, context)` (B2:271860-272084):
1. no speaker given -> the member of the SUBJECT's team, not the subject, nearest to it among those nearer than their
   own class's `VORadius`; none -> nothing (271886-271947);
2. the speaker must be within its class's `VORadius` of the subject (271950-271963);
3. a speaker whose team holds the subject's as an enemy (`FUN_002692b0` < 0) goes on with chance 31 in 101
   (`…
4. the speaker needs a controller; an AI's must be at detail level 3 or 4 (+0x560; `slti 3` at 0x2e6414) - the
   levels that think every 1.0 and 0.25 s (`FUN_002b6fe8` 238942-238946) - unless the unit is of class
   `DAT_00689368` (271980-272003);
5. the entries of the speaker's class list (a seated speaker: the class of the entity behind its controller's +0xbc)
   with this event that pass 3.2, at most 64, best first; the first is played at the speaker, queued - or at once
   and without its 3-D bit when the speaker IS the subject (272041-272079).

### 3.4 The entry point [V]

`FUN_002e6778(time, event, speaker, subject, object)` (B2:272148-272200; `FUN_002e6610` the same with a position for
the object): events 1 and 6 are dropped with chance 30 in 101 (`slti 71` at 0x2e67c8); a call with a time ahead of
now waits in a table of 20 (`FUN_002e68c0`; replayed by `FUN_002e69c0`, a vanished entity becoming 0; a full table
loses the call); **a subject of 0 becomes local player 0's unit** - no player unit, no line. These two are the only
callers of `FUN_002e6180`, and the 36 sites that call them are the whole list of triggers [receipt:bd3e7209].

### 3.5 The events [V]

Names by hash against the table (ELF 0x624ed8); "lines" = `VOSound` chunks on the disc.

| index, name | lines | fired from | rule |
|---|---|---|---|
| 1 AcquiredTarget | 5771 | `FUN_002cde30` B2:255580 | the tracker takes a non-null target (BF1's 2.2); speaker the unit, object the target |
| 2 FriendlyFire | 190 | `FUN_002c25e0` 247644 (8 sites) | the attacker's team holds the victim's as a friend; speaker the victim, object the attacker |
| 6 PathFollowWait | 4 | `FUN_002f1778` 281125 | the path follower lets another unit pass; speaker = object = the unit |
| 9 HeadshotKill | 189 | `FUN_00238e20` 152609 | a kill by an enemy whose character is HUMAN (+0x130 >= 0); speaker none (a team-mate near the player), object the victim [H: the headshot test itself is not on the cited lines] |
| 0xa Grenade | 197 | `FUN_002a89b8` 229141 | BF1's 2.3 with a delay of `FUN_002a2f70(0)` + up to 0.3 s; the soldier also calls its `FleeSound` slot |
| 0xb RebelsShootDeadBody | 249 | `FUN_002cf100` 256573 | a soldier of faction 1 at detail level 4, a dead enemy soldier within 5.0 m in sight: it aims at the body |
| 0xc-0xf KillingSpree4 / 8 / 12 / 16 | 214, 190, 0, 0 | `FUN_00362268` 359344, 359333 | a HUMAN's kills each within 4.0 s of the last (`lui 0x4080` at 0x362948), at the 4th, 8th, 12th, 16th; speaker his own unit |
| 0x10, 0x11 RepairStart / End | 668, 668 | `FUN_002d6828` 261557, `FUN_002d76a8` 262105 | an AI engineer's repair goal; object the thing repaired |
| 0x12 GivePowerup | 69 | `FUN_002d7510` 261904 | an AI hands a powerup to a unit |
| 0x16-0x20 SC_... | 65-87 each | `re-squad-orders.md` 7 | 0x1e is unnamed and fired (268859) |
| 0x21 NotShootingCriticalHit | 931 | `FUN_002b4db8` 237283 | a unit within 50 m of a target with critical spots, not aiming at one [H: the rule is read in part]; speaker none, subject that unit |
| 0x23, 0x24, 0x25 Nearby Friendly / Enemy / TeamKill Slaughter | 196, 193, 0 | `FUN_002b5240` 237348 | per AI controller within 25 m of a death: four of one kind inside 8.0 s (`lui 0x4100` at 0x2b52e8); speaker none, subject that unit |
| 8 Kill | 0 | `FUN_0034f650` 345957 | a human's kill; no line |
| 0x13, 0x14, 0x15, 0x26, 0x27 (unnamed) | 0 | `FUN_002d3aa8`, `FUN_002f1778`, `FUN_002c3fb0`, `FUN_002cd928` | no line |
| 4 MissileLocked, 5 MissileIncoming, 7 CollisionDamage, 3, 0x22 | 0, 186, 0, 0, 0 | **never** [receipt:e6dfb8fe] | no site passes these indices (both verifiers decoded a0 at all 36 sites) |
| 0: SpottedVO, NeedMedicVO, NeedRepairVO, NeedAmmoVO, NeedPickupVO, NeedBackupVO, AttackPositionVO, DefendPositionVO | 124 each | **never** | not in the table -> index 0, which no site passes; [L] 16 such entries sit in every trooper class's list in RAM |

### 3.6 Hurt and death [V]

`FUN_001dfb38` (B2:91227-91470) is BF1's damage function with two changes: the hurt call needs damage-flag bit 2
(*(decompiler detail omitted)*, 91449) beside the same 2 % test (91408; 0x3ca3d70a at 0x1e0010), and it passes "has an attacker":
`FUN_002694e8` (186086-186113) plays `HurtSound` for a hit with an attacker and `HurtFallSound` (an empty handle on
this disc) for one without; `FUN_00269448` plays `DeathSound`; one handle per entity (+0x25c) as in BF1.

### 3.7 The older keys [V]

BF2's soldier class still parses BF1's keys and holds the seven functions (B2:167687-167777; vtable 0x61eb88 slots
+0xb8-+0xd0). `FleeSound` is called beside the Grenade event (229140), `HeardEnemySound` at 248272, `ChargeSound` at
247545; the tracker no longer calls `AcquiredTargetSound` [D: no call through +0xb8 from an entity's first word in
the export]. Authors: the two Tuskens, the Jawa, the Gamorrean, `gar_inf_vanguard` and the Ewoks (4 to 8 classes a
key). `LowHealthSound` and `AmbientSound` run as in BF1 (159534-159558) inside a network-mode test that is open
offline; 7 classes author the first (Ewoks, acklay, wampa, Jawa, Vader).

## 4. Who has which voice [V disc]

`tools/classcensus.py` (every `entc` chunk of every LVL, the effective value through the BASE chain; re-counted by two
verifiers' own readers): BF1 59 classes with a `soldier` root, BF2 162.

- **BF1** (`bf1_classes.tsv`): hurt / death on 54 (not the four Jedi and Vader): `imp_` 11 classes, `all_` 9 (+ 3
  `..._female`, 5 `..._wookie`), `cis_` 8 (death `droid_death`), `rep_` 5, `gar_` 3, Gungan 3, Ewok 3, Tusken 2,
  Geonosian 1, Jawa 1. The combat keys follow the same prefixes on 42-50 classes; the five Wookiee classes use the
  sample-bank `all_inf_com_chatter_<x>_wookie` and `wok_inf_com_chatter_tac_*`.
- **BF2** (`bf2_classes.tsv`): hurt / death on 132; `VOSound` on 128 - 84 classes with 162 to 211 lines over 32
  event names, 34 with none (the `..._default` parents, the super battle droid, the MagnaGuard, the Jedi, natives);
  the voice is in the sound names (`rep1`, `cis1`, `all1`-`all3`, `imp1`...), so a class's own entries are its voice.

## 5. The PS2, live [L]

`ps2/usnd_live.py` samples a running battle every 4 ms: sample one-shot starts, stream starts, and - for every
property in the engine's own lists - each re-arm of its gate (the property named exactly, where a segment is shared
by several). The player's health is held at 30000; `--follow 20` puts him beside the team-mate nearest an enemy every
20 s. 240 game seconds each, player never lost (`live_bf1.txt`, `live_bf2.txt`; `tools/live_events.py`):

| per minute | NAB1 post | NAB1 fight | TAT1 post | TAT1 fight | UTA1 post | UTA1 fight | MYG1 post | MYG1 fight |
|---|---|---|---|---|---|---|---|---|
| lines started on the voice slot | 31.75 | 31.75 | 32.25 | 32.25 | 0.25 | 8.25 | 0.25 | 11.0 |
| hurt cries, in range / within 50 m | 42.8 / 0.3 | 58.0 / 17.3 | 41.3 / 7.5 | 70.5 / 19.3 | 72.5 / 6.0 | 139.3 / 50.3 | 20.8 / 0.5 | 83.8 / 31.8 |
| death cries, in range / within 50 m | 27.0 / 0.8 | 33.0 / 10.8 | 16.5 / 5.0 | 34.8 / 8.8 | 18.8 / 0 | 35.8 / 11.8 | 9.3 / 0.8 | 26.5 / 10.5 |
| gate: acquired / AcquiredTarget | 7.75 | 7.5 | 5.0 | 5.25 | 0.25 | 6.25 | 0 | 8.25 |
| gate: flee / Grenade | 5.25 | 4.0 | 3.25 | 2.5 | 0 | 0.25 | 0 | 0.25 |
| gate: heard | 5.5 | 5.5 | 3.75 | 3.75 | - | - | - | - |
| gate: `ai_` follow / need-gunner / move out | 4.0 / 2.75 / 0.75 | 4.75 / 3.75 / 0 | 4.5 / 4.25 / 1.0 | 4.5 / 3.75 / 0.75 | - | - | - | - |
| BF2 FriendlyFire / RepairStart / SC response | - | - | - | - | 0 | 1.25 / 0.25 / 0 | 0 / 0 / 0.25 | 1.5 / 0.5 / 0.5 |
| lines: the "yes" answer | 2.0 | 1.0 | 12.0 | 12.25 | - | - | - | - |
| lines: Gungan heartbeat (any share) | 16.0 | 18.5 | - | - | - | - | - | - |
| longest pending queue | 4 | 4 | 4 | 4 | 1 | 4 | 1 | 4 |

A third Mygeeto fight run (the first build of the follow mode) read 12.0 lines, 8.5 AcquiredTarget, 1.5 FriendlyFire.
The captures are one battle state each, the same four states twice; they are a baseline, not a distribution.

## 6. What the port needs (four build cards, `deps: USND`)

The port plays none of this (`game/src/event_sounds.rs`: fire, reload, impact, doors, the player's order line; the
order's "yes" through `core::announcer::VoiceQueue`). Its sound bank already applies a property's interval and
probability in the PS2's order (`game/src/sounds.rs` 1153-1162); the bake keeps no per-take lock.

1. **UCALL - BF1's combat calls.** `acquired` at a bot's new target, `flee` at the first bot within 7.5 m of a live
   grenade-class ordnance (once a second per ordnance), `heard` when a bot turns to a heard enemy; the class's own
   property through the voice queue; the take locks baked. Never `hide` / `approach` / `predamage`.
2. **UCRY - hurt and death, both games.** One voice per unit; hurt on a surviving health hit over 2 % of what is
   left, death on the kill (cutting the hurt); the class's `HurtSound` / `DeathSound`; no queue.
3. **UTALK - BF1's squad talk and the natives' idle voice.** The AI's `follow` / `need gunner` / `move out` with
   their answers (`re-squad-orders.md` 5) and `LowHealthSound` under its threshold rule.
4. **UVO2 - BF2's `VOSound`.** The class lists (a child on its parent's chain), the context word, the speaker rule
   of 3.3 (50 m of the player, 31 in 101 for an enemy, the nearest team-mate when no speaker is named), the events
   of 3.5 that have lines.

Each card's referee is section 5's row for its events on the same map with the player in the same place (parked at
his post; parked beside a front-line team-mate), within 30 % where the PS2 reads 5 a minute or more.

## 7. Checks run

- **fn**: 66 cited lines, each inside its named function (`checks_fn.txt`).
- **absent**: `re_check.py absent --card USND --elf`: the three uncalled functions (2 hits each: their own
  definition) [receipt:69e3eb1c]; ShockSound / 0x5625fc0b (the setter only) [receipt:acaecc7a]; the never-fired
  event hashes (0 in code) [receipt:e6dfb8fe]; HurtFallSound / VORadius (the setter only) [receipt:63a2d027];
  `FUN_002e6180` / `FUN_002e68c0` (their definition and the two wrappers) [receipt:bd3e7209]. Disc absences are raw
  PROP scans of every file (`tools/classcensus.py`, `vosound.py`; `re_check --src` reads ASCII only).
- **twin**: BF2's tracker, grenade, damage, gate and soldier-update twins opened beside BF1's (3.5-3.7; differences
  stated there).
- **word**: 7.5 / 1.0 (0x236cf0, 0x236cd0), 7.0 and `slti 3` (0x253ae0, 0x253bd4), 0.02 (0x1bc498; B2 0x1e0010),
  `slti 31` / `slti 3` (0x2e638c, 0x2e6414), `slti 71` (0x2e67c8), 4.0 (0x362948), 8.0 (0x2b52e8), 0.3 / 1.0 / 7.5
  (0x2a8a00, 0x2a8a44, 0x2a8a64), 5.0 (0x2cf270), 50.0 (0x269ec8), the 40 table words at 0x624ed8, 0x517dd0 = 14,
  0x517dd4 = 17 (`checks_word.txt`).
- **writers**: `DAT_005875f4` (19 writes: the `SetEnemyTeamDifficulty*` functions, `FUN_00237ba0`, and
  `FUN_0023a540` which sets the default 14); `DAT_00517dd0` / `DAT_00517dd4` (none) (`checks_writers.txt`).
- **Verifier workflow** `usnd-re-verify` (8 agents, medium effort, claims and citations only: six batches and two
  second readers for the 13 absence and twin readings; `wf/claims.md`, `wf/VERDICTS.md`): 40 claims, first reading
  **29 SUPPORTED / 11 PARTLY / 0 NOT**; second readings 8 SUPPORTED / 5 PARTLY, none overturned, three read more
  strictly than the first. 16 claims re-stated here. What the verifiers found that this pass had not: BF2's line
  interval is 40 +- 20 s for three of four armies (the draft said 80 +- 40 for all); HeadshotKill, the killing sprees
  and Kill fire for a human's kills only; RebelsShootDeadBody is faction 1 at detail level 4; a child class shares
  its parent's chain (then seen in RAM); death is also played by the kill routine; only an AI-controlled entity can
  say the grenade line; BF1's takes carry locks of 5 to 25 s; the one-shot path gates before it draws.

## 8. Stayed [H] / not done

- A call of the three dead BF1 slots through a computed slot index is excluded by no scan (live: never armed).
- BF2: whether event 9 needs a headshot (the test is not in `FUN_00238e20`'s cited lines); the reader of
  `DamageRegionSound`; the unnamed events 3, 0x13, 0x14, 0x15, 0x1e, 0x22, 0x26, 0x27; character +0x130 >= 0 as
  "human" is the repo's standing reading, not re-derived; `FUN_002692b0`'s second relation byte (a second team field
  of the subject) was not followed.
- Not run live: a BF2 capture in which the player kills (HeadshotKill, the sprees), any GCW battle (`all_`'s 80 +- 40
  and BF1's 1 +- 0 gates would change the rates), a map with Wookiees or Ewoks.
- The voice queue's part (which of several requests wins a busy slot) is `re-announcer.md`'s; here only its length.

## 9. Reproduction

```
python3 engine/artifacts/lead_tmp/usnd/tools/vcall.py b1 0xac 0xb0 0xb4 0xb8 0xbc 0xc0 0xc4     # the slot calls
python3 engine/artifacts/lead_tmp/usnd/tools/classcensus.py bf1|bf2 [--tsv out]                 # keys per class
python3 engine/artifacts/lead_tmp/usnd/tools/vosound.py --table                                 # BF2 events / lines
python3 engine/artifacts/lead_tmp/usnd/tools/sndcensus.py bf1|bf2 [--planet NAB] [--tsv out]    # the sound data
python3 engine/artifacts/lead_tmp/usnd/tools/ram_vo.py <bf2 state.p2s>...                       # VO lists in RAM
bash engine/artifacts/lead_tmp/usnd/ps2/run_live3.sh bf2 MYG bf2_myg1_cp2_cp1_con_post1_spawn.p2s <tag> 240 "--follow 20"
python3 engine/artifacts/lead_tmp/usnd/tools/live_events.py engine/artifacts/lead_tmp/usnd/ps2/<tag>...
```
