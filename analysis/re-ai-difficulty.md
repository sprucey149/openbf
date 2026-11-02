# RE: AI difficulty, the skill scale, the feel constants and the AI perception/aim model (BF1 + BF2)

Sources: `analysis/SWBFRNT2.ELF-decomp.c` (BF2, SLUS-212.40), `analysis/SWBFRONT.ELF-decomp.c`
(BF1, SLUS-208.98), raw ELF data reads (PT_LOAD reader `scratchpad/re_difficulty/elfrd.py`, used
for every `.data` initial value quoted below), `analysis/*-bindings.txt`, `analysis/*-stringrefs.txt`,
`analysis/a4-*-edges.txt`. Confidence tags: **[V]** read directly from decomp / raw bytes, **[D]**
derived from verified pieces, **[H]** hypothesis. PC-derived material is never used as evidence.
Written for the openbf AI decision (PLAN Log "AI decision", 2026-09-01): the DECISION LOGIC is not
described beyond what is needed to place a constant; the numbers are.

Notation: `skill` is the 1..20 integer described in §2; `t = (skill-1)/19`; `lerp(a,b)` means
`a + (b-a)*t`; `U(a,b)` a uniform random draw (LCG `x*0x19660D+0x3C6EF35F`, `FUN_002a3e48`).

---

## 0. Executive summary

| Question | Answer | Conf |
|---|---|---|
| What is "difficulty" in both engines | ONE integer skill per team on a **1..20** scale (BF1 clamp `DAT_00517db8`=1 / `DAT_00517dbc`=20; BF2 clamps in `FUN_002a29d8`). The shell level (1/2/3) only selects the per-team BASE skill; everything else is a function of the skill. | V |
| Sign convention | **Higher = better AI**: skill 20 gives the fastest aim tracking, spread multiplier 1.0, AI dispersion multiplier 0.0, longest bursts, full charge, highest grenade/dodge chances. Settled by the curve knots (§2.2, §1.3) — not by the scripts. | V |
| BF2 per-level base skill (`FUN_002a29d8`) | human (co-op) team's AI teammates `DAT_00621390` = **{6, 11, 11}**; every other team `DAT_006213a0` = **{4, 14, 17}** for Easy / Normal / Elite. Netplay and split-screen versus use the second table for both teams. | V |
| BF1 per-level base skill (`FUN_00237ba0`) | player team {`DAT_00517dc0`=6, `DAT_00517dc4`=4, `DAT_00517dc8`=4}; enemy team {`DAT_00517dcc`=4, `DAT_00517dd0`=14, `DAT_00517dd4`=17}. The enemy table is byte-identical to BF2's. | V |
| BF2 `SetAIDifficulty(a, b [, level])` | per-level ADDITIVE skill deltas: `DAT_005fdf88[level-1] = a` (human team's AI), `DAT_005fdf98[level-1] = b` (other teams); no level string = all three levels. Same thing as BF1's `Set{Player,Enemy}TeamDifficulty{Easy,Medium,Hard}` deltas. | V |
| What BF2 Elite changes | enemy skill 17 (vs 14) and friendlies 11; plus three level-3-only switches: AI attackers get the >1 damage multiplier in `FUN_001e0580`; combat-mode weights become uniform in `FUN_002aba38`; the shell can clear `bAutoAim`/`bAimAssist`/`bStickyReticule` for the human (`ScriptCB_SetDifficulty` 2nd arg). | V |
| The aim model (both games) | second-order aim-direction tracker (accel clamp + damping) + the two weapon multipliers (spread ×, AI distance-dispersion ×) + a per-target **tracked-position error radius** (grows with target motion, decays at a skill rate, clamped to a skill-continuous floor 1.6 → 0.4 m and cap 2.2 → 1.2 m (floats, §4.1), ×(3−2·visibility)); the AI then presses the SAME trigger words the human input path presses (`soldier+0x38+slot*4`). | V/D |
| Perception (BF2) | view cone: everything inside 10 m, else half-angle 77.5° (`DAT_00697e38`); view distance = (20·bound_radius + 30 m) × `SetAIViewMultiplier` × visibility × 2 for human-driven observers × 2 flyer targets; scan radius = max weapon range + 5·speed; acquisition = best of "120000 − distance metric" (nearest first) after a weapon-suitability test; 6-slot threat list with +10 hysteresis and a 1.5 s retarget timer. No LOS raycast located in the acquisition path. | V/D |
| BF2 auto-balance | shifts SKILL, not bot count: ±4 on the 1..20 scale between the human team's AI and the enemy AI from the ticket/score gap, re-evaluated every 15 AI ticks; inert in netplay and split-screen versus. | V |
| Knob defaults | §3 table (aggressiveness 1.0, stationary wait 10 s, vehicle notify 50 m, view multiplier 1.0, snipe suitability 70 m, BF1 snipe ranges 140). | V |

---

## 1. Function / address tables

### 1.1 BF2 (SWBFRNT2.ELF)

| addr | role | conf |
|---|---|---|
| `ScriptCB_SetDifficulty` 0019b588 | profile+0x3c = clamp(level,1..3) (default 2); optional 2nd arg true → profile+0x32 `bAutoAim`, +0x208 `bAimAssist`, +0x209 `bStickyReticule` = (level < 3) | V |
| `ScriptCB_GetDifficulty` 0019b688 | reads profile+0x3c (fallback `DAT_0069bc80`), clamps 1..3 | V |
| `ScriptCB_SetGeneralOptions` 0019cae8 | option key `iDifficulty` (0x616140) → `DAT_0069bc80` and profile+0x3c | V |
| `ScriptCB_SetNetGameDefaults` 0019db30 | net key `iDifficulty` → `DAT_006007d8` → `DAT_0069bc80` (netplay bots' level) | V |
| `SetAIDifficulty` 0018dfc8 → `FUN_002a2988` | hashes easy/medium/hard → 1/2/3; writes `DAT_005fdf88[l-1]`, `DAT_005fdf98[l-1]` | V |
| `FUN_002a4030` | AI system init at mission start: calls `FUN_002a28e0`, sets knob defaults (§3) | V |
| `FUN_002a28e0` | `DAT_00697e30` = level (profile+0x3c, or `DAT_0069bc80` in netplay via `DAT_00600548/DAT_0060054f/DAT_00600549`); zeroes the `SetAIDifficulty` tables; auto-balance on | V |
| `FUN_002a29c8` | returns `DAT_00697e30` (level 1..3) | V |
| `… false in netplay | V |
| `FUN_002a29d8(unit_or_flag, clamp)` | **skill** = base[level] + `SetAIDifficulty` delta[level] + auto-balance delta (+ goal query, always 0: `FUN_002f7cb8` is a stub), max(1), min(20) when `clamp` | V |
| `FUN_002a2bb8` / `FUN_002a2bf8` / `FUN_002a2c38` | `t=(skill-1)/19`; linear lerp; cubic lerp (`t³`) | V |
| `FUN_002a2c80` … `FUN_002a32e8` | the skill-parameter getters (§4.1 table) | V |
| `FUN_002a3318` / `FUN_002a33b8` / `FUN_002a3328` / `FUN_002a3400` / `FUN_002a3528` / `FUN_002a3c28` | auto-balance enable / active? / mode / score source / re-evaluation / nudges (§6) | V |
| `FUN_002af948` → `FUN_002af788` → `FUN_002a3528` | AI system tick; auto-balance re-evaluated every `DAT_005fe084` = 15 ticks | V |
| `FUN_002a83e0` | **AI aim tracking** (accel/damping, 30° slew, sets weapon+0x134/+0x138) | V |
| `FUN_0023ecb8` | soldier aim update; AI branch (`soldier+0x330 < 0`) calls `FUN_002a83e0` then the shared aim-point path (`FUN_003018b8`) | V |
| 0x0032e748 / 0x0032e750 | weapon vtbl (`&DAT_00628350`) +0x40 / +0x44: `swc1 f12,0x134(a0)` / `swc1 f12,0x138(a0)` — spread multiplier / AI dispersion multiplier setters (raw bytes 03e00008 e48c0134 / 03e00008 e48c0138) | V |
| `FUN_002ae010` / `FUN_002ae7b8` / `FUN_002ae4d0` | **target tracker**: error-radius decay+growth / floor+cap / effective radius | V |
| `FUN_002aec38` (+`FUN_002aeaa8`/`FUN_002aead8`/`FUN_002aebe0`) | visibility factor `ai+0x7c` | V |
| `FUN_002b7790` | tracker cadence: every 5th frame (LOD `ai+0x560` 0/1), 3rd (LOD 2), else every frame | V |
| `FUN_002b7530` | per-unit think (calls `FUN_002e3aa0` perception, `FUN_002b9a28` threats, …), scheduled by `+0x2c4` in `FUN_002af948` | V |
| `FUN_002bb1b0` | **fire controller**: decides fire, then drives the packed trigger words at `soldier+0x38+slot*4` | V |
| `FUN_002bac98` / `FUN_002baf70` / `FUN_002baac0` | burst controller (WeaponCannonClass) / single-shot tap (TriggerSingle, class flag bit 0x32 of `class+0x1e0`, B2:242171; corrected 2026-09-30, `re-round-stalls.md` §2 C4) / charge weapons | V |
| `FUN_0026cea0` | human input path writing the same trigger-word encoding (`0x1E0000`/s hold timer, states 0xc/0x1c) | D |
| `FUN_002e3aa0` / `FUN_002a7230` / `FUN_002e3910` / `FUN_002a9d90` / `FUN_002a9c30` / `FUN_002ac8f8` / `FUN_002e4ea8` | perception scan / view cone / view distance / scan radius / weapon range / suitability matrix / distance metric | V |
| `FUN_002b9a28` / `FUN_002b94a0` / `FUN_002ba088` / `FUN_002a9520` | threat list update / threat score / best slot / retarget interval | V |
| `FUN_002ab288` | grenade-use chance = `t` (WeaponGrenadeClass `DAT_00699f40`) | V |
| `FUN_0020a940` | soldier TakeDamage: AI dodge roll chance `lerp(0,0.2)` + the level gate `FUN_001e0580` | V |
| `FUN_001e0580` | damage apply: >1 multiplier suppressed for non-human attackers unless level 3 | V |
| `FUN_002aba38` (BF1 `FUN_00238b38`) | 4-way combat-mode pick weighted by level | V |
| `FUN_0020e7e0` + `FUN_002a3110` + `FUN_002a3078` | AI standoff-distance band from soldier class +0x834/+0x838/+0x83c/+0x840 (defaults 10/20/30/0, ctor line 124304) | V |
| `FUN_002a89b8` (BF1 `FUN_00236be8`) | proximity alert: notify entities within 7.5 m every `lerp(2,0)+U(0,0.3)` s | D |
| class ids: `DAT_00689828` EntitySoldier, `DAT_00689368` EntityFlyer, `DAT_00689300` EntityDroideka, `DAT_00688ec8` EntityBuildingArmed, `DAT_00699ec8` WeaponCannon, `DAT_00699ed0` WeaponCannonClass, `DAT_00699f40` WeaponGrenadeClass, `DAT_00699f88` WeaponMelee, `DAT_00699fc0` WeaponRepair, `DAT_006980b8` PlayerController | resolved via the `return DAT_…` class-id getters + stringrefs | V |

### 1.2 BF1 (SWBFRONT.ELF)

| addr | role | conf |
|---|---|---|
| `ScriptCB_SetDifficulty` 0017c7c0 / `ScriptCB_GetDifficulty` 0017c850 | `DAT_005899a0` and profile+0x4c (byte) = level 1..3 | V |
| `FUN_00237ba0(dP, dE)` | mission-start skill init (called `(0,0)` from `FUN_00174bd8`; `(4,0)` from the cheat handler `FUN_002a4650`): base by level (§2.1), + deltas, clamp [1,20], then `FUN_00237af0`; skipped when `DAT_00517df8` ≠ 0 (never set in code) | V |
| `SetPlayerTeamDifficulty` 00185360 / `SetEnemyTeamDifficulty` 00185558 | absolute set of `DAT_005875ec` / `DAT_005875f4`, clamp [1,20], recompute | V |
| `…Easy/Medium/Hard` 001853c0/00185448/001854d0 (player) and 001855b8/00185640/001856c8 (enemy) | ADD the delta to `DAT_005875ec` (player variants) / `DAT_005875f4` (enemy variants) only when profile+0x4c == 1/2/3; clamp; recompute. (Corrects A3, which had the player variants writing `DAT_005875f4`.) | V |
| `FUN_00237af0` | recompute: 4 curves × 2 teams → `DAT_00517dd8..de4` (enemy), `DAT_00517de8..df4` (player) | V |
| `FUN_002377e0` / `FUN_002378a0` / `FUN_00237960` / `FUN_00237a20` | curves A/B/C/D, piecewise-linear with knots at skill 1 / 14 (`DAT_00517dd0`) / 20 | V |
| `FUN_00236700` | **AI aim tracking** consumer (called from the soldier aim updates 00204748 / 0026d748): weapon vtbl+0x38/+0x3c ← C/D, A/B → accel/damping, halved for non-WeaponCannon (`DAT_00589410`) | V |
| `FUN_00236be8` / `FUN_00238b38` | enemy-skill tiered (<14 / 14–16 / ≥17) proximity-alert delay / combat-mode weights | V |
| `FUN_0023a540` | registers console vars `ai.PlayerSkillLevel` (0x535e78) → `&DAT_005875ec`, `ai.EnemySkillLevel` (0x535e90) → `&DAT_005875f4` (type 3 = int) | V |
| `FUN_00238090` / `FUN_002380a8` | `team+0x94` aggressiveness × weaponclass+0x60, capped at 50 → weapon-choice score | V |
| `FUN_002579e0` | attacker/defender snipe range consumer (range × 0.5 as a query radius; 70 when not applicable) | V |

---

## 2. The skill scale and the per-level bases (Q1 + Q2)

### 2.1 Level → base skill

Both games: shell level ∈ {1 Easy, 2 Medium/Normal, 3 Hard/Elite}; the level is only ever used to
pick a base skill per side (and the three BF2 level-3 switches in §2.4).

| side | Easy | Medium / Normal | Hard / Elite | where |
|---|---|---|---|---|
| BF2, the human team's AI teammates | 6 | 11 | 11 | `DAT_00621390..98` [V] |
| BF2, every other team | 4 | 14 | 17 | `DAT_006213a0..a8` [V] |
| BF1, player team | 6 | 4 | 4 | `DAT_00517dc0/dc4/dc8` [V] |
| BF1, enemy team | 4 | 14 | 17 | `DAT_00517dcc/dd0/dd4` [V] |

BF2 `FUN_002a29d8` picks the table by `FUN_002a8300(team)` — "is the (single-player / co-op)
human team". In netplay (`DAT_00600548` path) and in split-screen versus (humans on different
teams) it is false for every team, so **all bots use the {4,14,17} table** [V]. On top of the
base: BF2 adds `SetAIDifficulty`'s per-level delta (`DAT_005fdf88`/`DAT_005fdf98`, zero unless a
mission script calls it — none of the stock scripts do) and the auto-balance delta
(`DAT_005fdfa4`/`DAT_005fdfa8`, §6); BF1 adds the `Set*TeamDifficulty*` deltas. Result clamped to
[1,20] in both.

BF1 campaign math (yav1i_h.lua: Easy P+4/E+4, Medium P+2/E−2, Hard P+6/E−1) [D]:
Easy → player 10 / enemy 8; Medium → 6 / 12; Hard → 10 / 16. The enemy tier thresholds in
`FUN_00236be8`/`FUN_00238b38` are `<14`, `14..16`, `≥17`, so the campaign's Hard enemy (16) sits
in the MIDDLE tier of those two behaviours, and the campaign's Medium enemy (12) in the low tier.

Netplay BF2: `DAT_0069bc80` comes from the host's net-game defaults key `iDifficulty`
(`ScriptCB_SetNetGameDefaults` 0019db30, clamped 1..3, default 2) [V].

### 2.2 The BF1 recompute and the four curves (`FUN_00237af0`)

Each curve is piecewise-linear over skill with knots at 1 / 14 / 20 (`DAT_00517db8`,
`DAT_00517dd0`, `DAT_00517dbc`); values from `.data` [V]:

| curve | knots (skill 1 / 14 / 20) | data | consumer in `FUN_00236700` |
|---|---|---|---|
| A `FUN_002377e0` | 0.8 / 1.9 / 20.0 | `DAT_00517dfc/e00/e04` | aim-tracking acceleration clamp |
| B `FUN_002378a0` | 0.8 / 1.9 / 2.4 | `DAT_00517e08/e0c/e10` | aim-tracking damping |
| C `FUN_00237960` | 3.0 / 1.6 / 1.0 | `DAT_00517e14/e18/e1c` | weapon spread multiplier (vtbl+0x38 → weapon+0x154) |
| D `FUN_00237a20` | 2.0 / 1.0 / 0.0 | `DAT_00517e20/e24/e28` | AI dispersion multiplier (vtbl+0x3c → weapon+0x158) |

Enemy results land in `DAT_00517dd8/ddc/de0/de4`, player results in `DAT_00517de8/dec/df0/df4`
(A, B, C, D order). `FUN_00236700` chooses the set by `FUN_00236638(team)` (player team?) and halves
A and B when the weapon is not a WeaponCannon. This is exactly BF2's `FUN_002a83e0` with the
curves replaced by lerps (§4.1), so **the aim model is the same engine in both games** [V].

Sign convention, settled: at skill 20 the spread multiplier is 1.0 and the AI dispersion
multiplier 0.0 (no added error), the tracker accelerates 25× faster than at skill 1 → higher is
better. The enemy base {4,14,17} therefore makes Hard hard; the campaign deltas mostly lift the
player's teammates off the very low default of 4.

### 2.3 Console vars (BF1)

`ai.PlayerSkillLevel` / `ai.EnemySkillLevel` bind directly to `DAT_005875ec` / `DAT_005875f4`
(`FUN_0023a540`, `FUN_004bf718(name, {3, &var})`). Writing them does NOT recompute the derived
parameters — only `FUN_00237af0` does, which runs from the Lua bindings and at mission start — so a
console change is live only after the next `Set*TeamDifficulty*` call [D]. `DAT_00517df8` (skip the
mission-start reset) is only ever written 0.

### 2.4 BF2: everything the level touches directly [V]

1. `FUN_002a29d8` base tables (above).
2. `FUN_001e0580` (damage application, called from the soldier's TakeDamage `FUN_0020a940` and
   five other entity classes): when the health component's per-hit multiplier
   (`(+0xbc vtbl+0x24)(hit)`) is > 1.0 and the attacker (*(decompiler detail omitted)*) is absent or not
   human-driven (`+0x130 < 0`), the multiplier is forced to 1.0 unless the level is 3. So AI
   attackers only get the bonus damage on Elite. What the >1 multiplier is (re-blaster-damage §
   found no headshot multipliers in the ordnance chain; this one lives on the victim's health
   component) is [H] — see open questions.
3. `FUN_002aba38` combat-mode pick: four modes (codes 2,3,4,5; 4 needs `ctrl+0xb4→+0x60 > 10`,
   5 needs `> 5`) drawn with weights Easy {3,1,1,1}, Normal {2,2,1,1}, Elite {1,1,1,1}
   (BF1 `FUN_00238b38` does the same by enemy-skill tier). Mode meanings [H].
   **Correction (card SAIR, 2026-10-02, `re-soldier-ai.md` 4.4-4.5, 8):** the third row is {1,1,5,1} (ELF
   0x6215e8..f4), not {1,1,1,1}; and nothing is "drawn": the number is ai+0x5a0, fixed per controller, so a bot's
   mode is a fixed function of it, the level, the open space and the weapon. Modes: 2 stand, 3 circle, 4 zig-zag,
   5 flank.
4. `ScriptCB_SetDifficulty(level, true)` clears the human's `bAutoAim` (profile+0x32),
   `bAimAssist` (+0x208) and `bStickyReticule` (+0x209) when level == 3 and sets them otherwise;
   the PlayerController reads +0x208/+0x209 (`FUN_002b5968` region, lines 236851/236875). Whether
   the shipped shell script passes the second argument is not verified here (PROVISIONAL).

---

## 3. The Lua tuning knobs (Q3)

| binding | BF2 target (writer) | BF2 default | BF2 consumer | BF1 target | BF1 default | BF1 consumer |
|---|---|---|---|---|---|---|
| `SetTeamAggressiveness(team, v)` | `team+0x90` (`FUN_0036fb20`) | 1.0 (team ctor `FUN_0036d468`, line 366720) | not located (no float read of team+0x90 found in the AI region) | `team+0x94` (`FUN_002ae948`) | 1.0 (team ctor `FUN_002acf40`, line 238271) | `FUN_00238090`: aggressiveness × weaponclass+0x60, capped 50, in the weapon-choice score `FUN_002380a8` |
| `SetStationaryWaitTime(s)` | `DAT_00697e40` | 10.0 (`FUN_002a4030`); `SetStayInTurrets` 0018d550 writes 10000 | `FUN_002cd2e0`: a stationary unit (speed 0) idle longer than the wait is kicked (`… `SetStayInTurrets` → 10000 | line 166816, same shape |
| `SetAttackerSnipeRange(r)` / `SetDefenderSnipeRange(r)` | **stubs** (0018d540/0018d548 `return 0`) | — | — | `DAT_0058773c` / `DAT_00587740` (int) | 140 / 140 (0x8c, lines 157580/157578) | `FUN_002579e0`: `range*0.5` as the snipe-hint query radius, picked by the team's role byte (`team+DAT_00587744+0x88`); 70 when neither applies |
| `SetAIVehicleNotifyRadius(r)` | `DAT_00698454` (`FUN_002e3440`) | 50.0 (line 269370) | `FUN_002e2e38`: spatial query radius (`FUN_001aef10(r, pos, …, 0x1e, …)`) for the units told about a vehicle | `DAT_00587778` (`FUN_002422e8`) | 50.0 (line 158908) | line 159159, same shape |
| `SetAIViewMultiplier(m)` | clamp [0.1,1.0] → `DAT_005fe71c` (`FUN_002e37c0`) | 1.0 (.data) | `FUN_002e3910` view distance (§5.1) | — | — | — |
| `AISnipeSuitabilityDist(d)` | clamp [0,100] → `DAT_005fdffc` | 70.0 (.data and `FUN_002a4030`) | `FUN_002d8398`: radius of the entity query around a snipe hint (hint pos + dir × d) | — | — | — |
| `SetAIDamageThreshold(entity, v)` 00192690 | `entity+0x18c` | 0 | read only as a flag (`!= 0`) in the soldier AI (lines 120201, 121961, 123988 — halves the standoff band) and the planner (244087, 244517, 269625, 269640, 288862, 289320); semantics [H] | — | — | — |
| `Enable/DisableAIAutoBalance` | `DAT_005fdfac` (`FUN_002a3318`) | 1 (.data; `DAT_005fdfad` also 1) | §6 | — | — | — |

---

## 4. The accuracy / aim model (Q4)

### 4.1 BF2 skill-parameter getters (all in `0x2a2c80..0x2a32e8`) [V]

CORRECTED 2026-09-08 (session 14): the floor and cap are floats, not int casts — see analysis/re-ai-sniper.md §1.

`u` = the unit's AI object (its own team's skill); `0` = the non-human table regardless of team;
"cross" = the getter is called with the TARGET's team inverted, i.e. it still yields the
OBSERVER's team skill (`FUN_002ae010`, `FUN_002ae7b8`, `FUN_002ae4d0`).

| getter | value | consumer | meaning |
|---|---|---|---|
| `FUN_002a2eb0(u)` | cubic `lerp(0.8, 14.0)` | `FUN_002a83e0` | aim-direction acceleration clamp (BF1 A: 0.8/1.9/20) |
| `FUN_002a2ee0(u)` | `lerp(0.8, 2.4)` | `FUN_002a83e0` | aim-direction damping (BF1 B) |
| `FUN_002a2f18(u)` [clamp] | `lerp(3.0, 1.0)` | `FUN_002a83e0` → weapon+0x134 | spread multiplier (BF1 C → +0x154) |
| `FUN_002a2f48(u)` [clamp] | `lerp(2.0, 0.0)` | `FUN_002a83e0` → weapon+0x138 | AI distance-dispersion multiplier (BF1 D → +0x158) |
| `FUN_002a2dc0` cross | `lerp(-0.1, -1.1)` /s | `FUN_002ae010` | tracked-position error decay rate (× dt; halved for flyer targets) |
| `FUN_002a2df8` cross | `max(0, lerp(1.6, 0.4))` — a FLOAT (`max.s` at 0x2a2e28; no int conversion; re-ai-sniper §1) | `FUN_002ae7b8` | error-radius floor, continuous: 1.6 m (skill 1), 1.41 (4), 0.97 (11), 0.78 (14), 0.59 (17), 0.4 (20); 1.5 m with no target |
| `FUN_002a2e38` / `FUN_002a2e78` cross | `max(0, lerp(2.2, 1.2))` / `max(0, lerp(4.0, 1.2))` — floats (same `max.s` idiom, 0x2a2e68 / 0x2a2ea4) | `FUN_002ae4d0` | effective error cap, continuous: 2.2 → 1.2 m (2.04 at skill 4, 1.67 at 11, 1.52 at 14, 1.36 at 17); flyer targets 4.0 → 1.2 m |
| `FUN_002a2c90(u)` | `lerp(0, 1)` | `FUN_002bac98` | burst length = min + U(0,max−min)·t, ≥ 2 (min/max = weapon class vtbl+0x34/+0x38) |
| `FUN_002a2cb8(u)` | `lerp(0.1, 0.9)` | `FUN_002bac98` | inter-burst pause = (1.6 − v) × weaponclass+0x21c (`FUN_0032b0d8`) → 1.5× … 0.7× |
| `FUN_002a2cf0(u)` | `lerp(0.1, 0.9)` | `FUN_002baf70` | single-shot tap: press one frame, release, re-arm after U(0.2·max(0,1−v), 2·max(0,1−v)) s (corrected 2026-09-30, `re-round-stalls.md` §2 C4) |
| `FUN_002a2d28` / `… expected-damage evaluation of a charge stage |
| `FUN_002a2d68(u)` [clamp] | `lerp(0, 1)` | `FUN_002ab288` | chance to use the grenade class against the current target = t (rolled once per 3 s window, always for droideka targets) |
| `FUN_002a2d90()` | `lerp(0, 0.7)` | `FUN_002b94a0` | threat-score facing floor = max(0, 2v−1) (§5.2) |
| `FUN_002a2f70(0)` | `lerp(2.0, 0)` | `FUN_002a89b8` | proximity-alert period + U(0,0.3) s (BF1 tiers U(1.1,2.1)/U(0.6,1.3)/U(0,0.8)) |
| `FUN_002a2f98` / `FUN_002a2fc0` | `lerp(0,0)` = 0 | `FUN_00209220` | long-range target-notice probabilities — **inert on PS2** |
| `FUN_002a2fe8` / `FUN_002a3018` | `lerp(0,1.5)` / `lerp(0,0.2)` | `FUN_0020a940` | on being hit, AI soldier dodges with probability `lerp(0,0.2)`; cooldown 2 s (+0x1d80), +0x1d88 = 10, +0x1d84 = (U−0.5)·lerp(0,1.5)+2.25 [dodge/roll trigger `FUN_002fa478` — H] |
| `FUN_002a3048` | `lerp(0.5, 1.5)` s | `FUN_00202d08` | interval between target-notice evaluations |
| `FUN_002a3110` | curve at `lerp(0.3,0.8)` and `lerp(0.5,1.2)` | `FUN_0020e7e0` | standoff distance band over class (10, 20, 30, far) — §4.4 |
| `FUN_002a3200` / `FUN_002a3268` / `FUN_002a32d0` / `FUN_002a32e8` | U(lerp(45,15), lerp(100,60)) / U(lerp(25,10), lerp(45,12)) / 0.4 / `lerp(0.5,0.25)` | `FUN_002a3c28` | auto-balance nudge cadences and coin biases (§6) |
| `FUN_002a2c80` | 1.0 | `FUN_002d9620` | constant (a periodic 2–5 s probability check) |

Per-level values of the aim parameters (t = 0.158 / 0.263 / 0.526 / 0.684 / 0.842 for skill 4 / 6 / 11 / 14 / 17) [D]:

| skill | spread × (+0x134) | AI dispersion × (+0x138) | aim accel | aim damping | burst frac | pause × | error decay m/s | error floor / cap |
|---|---|---|---|---|---|---|---|---|
| 4 (Easy enemy) | 2.68 | 1.68 | 0.85 | 1.05 | 0.16 | 1.37 | 0.26 | 1.41 / 2.04 |
| 6 (Easy friendly) | 2.47 | 1.47 | 1.04 | 1.22 | 0.26 | 1.29 | 0.36 | 1.28 / 1.94 |
| 11 (Normal/Elite friendly) | 1.95 | 0.95 | 2.72 | 1.64 | 0.53 | 1.08 | 0.63 | 0.97 / 1.67 |
| 14 (Normal enemy) | 1.63 | 0.63 | 5.03 | 1.89 | 0.68 | 0.95 | 0.78 | 0.78 / 1.52 |
| 17 (Elite enemy) | 1.32 | 0.32 | 8.68 | 2.15 | 0.84 | 0.93 | 0.94 | 0.59 / 1.36 |
| 20 (max) | 1.00 | 0.00 | 14.0 | 2.40 | 1.00 | 0.70 | 1.10 | 0.40 / 1.20 |

BF1 at the same skills (curves of §2.2): spread × 2.68 / 2.46 / 1.92 / 1.60 / 1.30 / 1.00; AI
dispersion × 1.77 / 1.62 / 1.23 / 1.00 / 0.50 / 0.00; aim accel 1.05 / 1.22 / 1.65 / 1.90 / 10.95 / 20.

### 4.2 The aim tracker (`FUN_002a83e0`, BF1 `FUN_00236700`) [V]

Called from the soldier aim update (`FUN_0023ecb8`) only when the soldier is AI-driven
(`soldier+0x330 < 0`) and its flag bit 0x21 at `+0x330` of the controller object is clear.
Inputs: dt, the AI object, the aim controller at `soldier+0x580`, the current aim direction
`soldier+0x350`. Per call:

1. If the unit's vehicle carries a human-driven slot (`FUN_002a7fd8(unit) > 0`: any occupant
   controller with `+0xc0 ≥ 0`) the target direction is passed straight through (no lag) [D].
2. Weapon multipliers set every call: `weapon+0x134 = lerp(3,1)`, `weapon+0x138 = lerp(2,0)`
   (§4.1). `weapon+0x138` scales the AI distance table in `GetYawSpread` (re-blaster-damage §4.3).
3. If the angle to the target exceeds 30° (`FUN_0052a688` = cosf(π/6)) the current direction is
   rotated 30° toward it (`FUN_0053adf0(±π/6)`).
4. Then (in the same call, also after a step-3 slew, which zeroes `v` first) a second-order
   tracker: aim velocity `v` (aimctl+0xc0) gets `a = clamp_A(((target − dir)/dt − v)/dt) − v·B`
   (the acceleration that would reach the target next tick, its length clamped to A, THEN the
   damping subtracted outside the clamp), `v += a·dt`, `dir = unit(dir + v·dt)` — A and B from
   §4.1, both halved when the weapon is not a WeaponCannon (`DAT_00699ec8`). BF2 decomp lines
   228922-228983 (the *(decompiler detail omitted)* block), BF1 150727-150786. (Corrected
   2026-10-30: this step first read `a = (target − dir)/dt − v·B` with the sum clamped, which
   the port implemented until then.)
5. The result goes into the shared aim-point path exactly like the human branch: aim point =
   origin + dir × 200 m (`0x43480000`) → `soldier+0x3d0/+0x3c0` → `FUN_003018b8`.

### 4.3 The target tracker (`ai+0x00..`, BF2) [V]

Per AI think (`FUN_002b7790`: every 5th frame at LOD 0/1, 3rd at LOD 2, else every frame):

- `FUN_002ae010(dt)`: `err(+0x20) += dt·lerp(−0.1,−1.1)` (decay; ×0.5 for flyer targets),
  `err += 0.455·|Δpos| / max(maxspeed(vtbl+0x68), 0.1)` when the target moved, then
  `FUN_002ae7b8` clamps `err` to [floor `max(0, lerp(1.6,0.4))` (a float; 1.5 with no target), 2.0];
  a second accumulator (+0x24) clamped [0,5]; a random unit vector at +0x30 is re-drawn
  (±0.5 per axis, blended) each think.
- `FUN_002ae4d0` → `+0x28` effective radius =
  `min(err × (1 + 0.6·|v|/maxspeed), cap) × 2·FUN_002a76b8(target) × [0.4 if state 7 | 2 if state 2] × (3 − 2·vis)`,
  cap `max(0, lerp(2.2,1.2))` (flyers `max(0, lerp(4.0,1.2))`), both floats, `vis = ai+0x7c` (§5.3).
- `FUN_002aec38` refreshes `vis`.
- **CORRECTED 2026-10-09 (card CLSE, `re-soldier-ai.md` 17.2-17.3):** this block is the unit's OWN - its own
  velocity change grows `err`, its own speed, size and state make the radius, by the ENEMY team's skill (RAM: the
  vector at +0x10 is the unit's own velocity; the player's controller holds the block too) - and a shooter reads
  its TARGET's +0x28 in `FUN_002a4cc8`, which then scales the vector: x class +0x1e8, y >= 0, zero when the shooter
  rides with a human, **x 0.3 when the target carries no human**, and `FUN_002a50c0` multiplies by d / 20 + 0.5.
- **CORRECTED 2026-10-09 (card CLSE2, `re-soldier-ai.md` 18):** three lines above are wrong or short. (1) `err`
  grows by 0.455 x the unit's change of VELOCITY (not `|Δpos|`: the slot +0x4c vector is E+0x550), and a positive
  growth is PAID from the "second accumulator" +0x24, which fills at half the decay rate (at most 5). (2) The
  `min(err x ..., cap)` has one more factor inside it, a soldier's stance: crouch x 1.5, sprint / jump x 2, roll
  x 3 (`FUN_0024ea00`, the controllable's slot +0x34). (3) The `[0.4 if state 7 | 2 if state 2]` never applies to
  a soldier (its slot +0xe0 returns 1). The vector at +0x30 is not "re-drawn": it is the axis a quaternion is
  turned about by 2 x dt a think, and +0x50 is that quaternion's (0, 1, 0). `2 x FUN_002a76b8` is a quarter of the
  model box's horizontal diagonal, doubled (0.48-1.02); class +0x1e8 / +0x1ec / +0x1f0 are the ODF keys
  `AIBubbleSizeMultiplier` / `AIBubbleScaleDistDivider` / `AIBubbleScaleClamp`. **BF1 has none of this** (18.5).

The aim direction the tracker feeds is the target's tracked position, i.e. the AI aims at where
it believes the target is, ±radius, then the weapon adds its own dispersion (×+0x134, +0x138).
There is no explicit reaction-time timer; the reaction "feel" comes from the tracker's 5-frame
cadence, the error floor/decay, and the 1.5 s retarget interval (§5.2).

### 4.4 Fire control and the shared fire path [V/D]

**Addition (card CLSE4, 2026-10-12, `re-soldier-ai.md` 20.2-20.3):** the burst bounds and the pause constant are the
weapon class keys `BarrageMin` / `BarrageMax` / `BarrageDelay` (class `+0x214` / `+0x218` / `+0x21c`, constructor
defaults 3 / 6 / 2.0 s; only flyer weapons and the defence turret write them), BF2's draw never reaches max, BF1's
draw includes max and has no skill term on the draw or the pause, and the trigger fires a tracked target only inside
the weapon's `MaxRange`.

- `FUN_002bb1b0(dt, ai, …)` is called from each entity class's update with the AI object at
  `entity+0x430` (soldier: line 122042 in `FUN_0020b840`; others 102835, 110233, 135767, 164689,
  181580, 190801). It picks the "want to fire" bit via `FUN_002bac98` (cannon burst),
  `FUN_002baf70` (timed hold, class flag bit 0x32 of `class+0x1e0`) or `FUN_002baac0` (charge),
  then edits the packed trigger word at `soldier+0x38 + slot*4` (state in bits 0–4, held-time
  fixed-point in bits 8–28 advancing `0x1E0000` per second via `FUN_00490a78`, states 0xc/0x1c).
- The human input path (`FUN_0026cea0`, line 190001) edits words of the same layout and
  encoding (`+ dt·1.96608e6`, the 0xc/0x1c states), so the weapon fire path downstream of the
  trigger word is shared [D — the two writers were matched by encoding, not by a common callee].
- Burst controller `FUN_002bac98`: state 1 draws `n = max(2, min + U(0,max−min)·t)`; state 2
  waits `(1.6 − lerp(0.1,0.9)) × weaponclass+0x21c` after the previous burst and checks the heat
  budget (class vtbl+0x2c/+0x30, `FUN_0052a5b0`); state 3 fires until `n` shots or the heat
  target. Charge weapons: release at charge `U(0.15·C, t·C)`. Soldier-only: the repair weapon
  never auto-fires.
- Standoff band `FUN_0020e7e0`: `FUN_002a3078` maps x ∈ [0,1.5] over class (+0x834=10, +0x838=20,
  +0x83c=30, +0x840 or 30) — [10→20] for x<0.5, [20→30] for x<1, [30→far] above; band =
  [curve(lerp(0.3,0.8)), curve(lerp(0.5,1.2))] → skill 4: 17.6–22.2 m, 11: 21.3–27.4 m,
  14: 22.8–29.6 m, 17: 24.4–30 m [D]; a sinusoidal time jitter picks a point inside it. When
  `soldier+0x668 ≠ 0` the class values are capped at `DAT_005fcf24..30` = 60 / 20 / 10 / ×0.2.

---

## 5. Perception and target acquisition (Q5, BF2)

### 5.1 Seeing [V]

`FUN_002e3aa0` (per think): candidates = `FUN_001aef10` spatial query, radius
`FUN_002a9d90` = max over the unit's weapons of (`FUN_002a9c30` range + 5 × unit max speed), where
`FUN_002a9c30` = `weaponclass+0x118` (doubled for non-soldier/droideka units when class flag bit
0x35) × `FUN_002f5a08(ai)`, capped by `class+0x100`. Each candidate must pass, in order:
`FUN_002a7c30` (hostile/valid), the cone `FUN_002a7230` (inside 10 m always; else
`dot(view_dir ai+0xe0, offset) > 0` and `cos²θ·d² < dot²` with `DAT_00697e38 = FUN_00539ff8(1.3526 rad)`
— the table function is cos by plausibility [D], giving a **77.5° half-angle**; if it were sin the
cone would be 12.5° [H]), the distance test `d² < viewdist²` with
`FUN_002e3910` = `(20·bound_radius + 30) × DAT_005fe71c × vis(ai+0x7c) × 2 [human-driven observer] × 2 [flyer target] or × 10 [EntityBuildingArmed target]`,
and the attacker-kind × target-kind suitability matrix `FUN_002ac8f8` (`DAT_005fe734` = 1).
Candidates are ranked by `120000 − FUN_002e4ea8(ai, target)` (a distance metric, +60000 penalty
for the "flagged but unsuitable" case) into a heap — nearest first. No line-of-sight raycast was
found in this path (open question; `FUN_002aec38`'s `+0x70/+0x74` state and `FUN_002aea08` may
carry it).

### 5.2 Choosing [V]

`FUN_002b9a28`: six threat slots (0x30 bytes: entity, id, kind, …, timestamp). Score
`FUN_002b94a0` = `kind×18 + 100 + (100 − metric/600)`, minus `10 × age` for kinds ≠ 0 (twice for
kind 2); kind 4 (recent damage) = 255 for 4 s; 0 if a flyer observer has the target behind it.
For targets driven by a `PlayerController` (`FUN_002b0378`) the distance term is scaled by
`max(max(0, 2·lerp(0,0.7) − 1), clamp01((dot − 0.6)·2.5))` where `dot` compares the target's
facing row with the observer→target direction — read literally, humans looking AWAY from the
bot score full, humans looking AT it score 0 until skill ≈ 15, rising to 0.4 at 20 [D; the
axis convention of the transform row is H]; other targets get ×0.5. The current target changes
only when `FUN_002a9520` (1.5 s for non-flyer soldier units, else 0) has elapsed since the last
switch and the best slot beats the current one by more than 10.

### 5.3 Visibility factor `ai+0x7c` [V]

`FUN_002aec38` = `FUN_002aeaa8` (tracker +0x68 or +0x6c, state `FUN_002aea08`) ×
`(1 − target_soldier+0x7e3/255)` (a per-soldier 0..255 concealment byte; 1.0 for non-soldiers) ×
`FUN_002aebe0` (+0x70 or +0x74 for 1 s after the +0x78 event, else 1). It multiplies the view
distance and enters the aim error as `(3 − 2·vis)`.

### 5.4 Hearing / alerting [D]

`FUN_002a89b8`: every `lerp(2,0) + U(0,0.3)` s the unit queries entities within 7.5 m (mask
0xaf) and calls a notify virtual on each — a proximity alert, not a sound model. BF1
`FUN_00236be8` is identical with the tiered delays. No other hearing mechanism was found.

---

## 6. `EnableAIAutoBalance` / `DisableAIAutoBalance` (Q6, BF2) [V]

Active iff `DAT_005fdfad && DAT_005fdfac` (both 1 in `.data`) and `FUN_002a3328` ≠ 3, where the
mode is 1 when `DAT_005fed90 == 1` (uses `team+0x5c`, threshold 20), 2 when both teams have
`tickets(+0x24) < 1000` (threshold 15), 0 when a mission flag at `DAT_005fee38+0x621` is set
(`+0x5c`, threshold 0), 3 (off) for `DAT_005fed90` 2/3 or a 1000+ ticket team. Only in
single-player / co-op (`FUN_002a8300(1) xor FUN_002a8300(2)`). Every 15 AI ticks
(`FUN_002af948`, `DAT_005fe084`) `FUN_002a3528` computes
`diff = human_score − enemy_score (+20 if the human team's tickets < 25% of initial, +10 if < 50%)`;
when `|diff| > threshold`: `DAT_005fdfa8 (enemy AI delta) = (diff ∓ threshold)/10` clamped to
±4 and `DAT_005fdfa4 (human team's AI delta) = −DAT_005fdfa8`. So a human team 25 tickets ahead
gives the enemy bots +1 skill and its own bots −1; ≥ 55 ahead → ±4. Mode 1 additionally runs
`FUN_002a3c28`: on skill-scaled timers (`U(lerp(45,15), lerp(100,60))` s and
`U(lerp(25,10), lerp(45,12))` s, stretched by the balance delta) it hands one unit of the losing
side a new goal (`FUN_002a38a0` / `FUN_002a3980`). **It never changes bot counts**; the
`ScriptCB_SetNumBots` table 0x5fa808 (re-tickets-spawn §5) is independent of it.

---

## 7. What openbf should take from this (for the lead)

Values are the PS2 originals; the recommendation column is only where the code shape matters.

1. **Skill scale**: one integer 1..20 per unit (BF1 `DAT_00517db8/dbc`, BF2 `FUN_002a29d8`), `t=(s−1)/19`.
   Anchor "old Elite" = enemy 17, teammates 11 (`DAT_006213a0`/`DAT_00621390`); Normal = 14/11; Easy = 4/6.
   BF1 is 17/4, 14/4, 4/6 before the campaign deltas (`DAT_00517dc0..dd4`).
2. **Aim tracker**: accel clamp cubic `lerp(0.8,14)` (BF1 0.8/1.9/20), damping `lerp(0.8,2.4)`, 30°
   slew, both halved for non-cannon weapons (`FUN_002a83e0`, `FUN_00236700`).
3. **Weapon multipliers** written every aim tick: spread × `lerp(3,1)` → weapon+0x134/+0x154; AI
   distance-dispersion × `lerp(2,0)` → weapon+0x138/+0x158 (0x32e748/0x32e750; BF1 vtbl+0x38/+0x3c).
   At Elite-enemy 17 that is ×1.32 / ×0.32.
4. **Tracked-position error**: decay `lerp(0.1,1.1)` m/s, growth `0.455·Δpos/maxspeed`, floor
   `max(0, lerp(1.6,0.4))` m (1.6 → 0.4, continuous; 0.97 at skill 11), cap `max(0, lerp(2.2,1.2))` m
   (2.2 → 1.2), ×(1+0.6·speed ratio), ×(3−2·visibility)
   (`FUN_002ae010`/`FUN_002ae7b8`/`FUN_002ae4d0`). This is the originals' "aim error scaled by
   range and target motion" (PLAN layer 5); range enters only through the weapon dispersion.
5. **Cadence**: tracker/perception think every 5 frames at full LOD (`FUN_002b7790`), retarget
   ≥ 1.5 s with +10 score hysteresis (`FUN_002b9a28`, `FUN_002a9520`).
6. **Fire discipline**: bursts `min + U(0,max−min)·t` (≥2), pause `(1.5 − 0.8t) × weapon time
   constant`, charge to `U(0.15, t)·C`, grenade chance `t` per 3 s window, dodge-on-hit chance
   `lerp(0,0.2)` with a 2 s cooldown (`FUN_002bac98`, `FUN_002baac0`, `FUN_002ab288`, `FUN_0020a940`).
7. **Perception**: 10 m omni + 77.5° half-angle cone [D], view distance `(20·radius+30)·mult·vis`
   (soldier ≈ 40–50 m at mult 1), ×2 for flyer targets, scan radius = weapon range + 5·speed,
   nearest-first acquisition, facing-aware scoring vs humans (§5.2), proximity alert 7.5 m every
   `lerp(2,0)+U(0,0.3)` s. No LOS raycast located — openbf's LOS + hearing (PLAN layer 4) is new.
8. **Difficulty semantics to keep**: symmetric per-team skill; the level-3-only bonus damage for
   AI attackers (`FUN_001e0580`) is the originals' one "pressure" lever; auto-balance is a ±4
   skill shift from the ticket gap, not a bot-count change (§6) — PLAN's "balanced by bot COUNT"
   attribution to BF2 should read "BF2 balanced by skill; counts come from the 0x5fa808 table".
9. **Knob defaults** (§3): aggressiveness 1.0, stationary wait 10 s (10000 with StayInTurrets),
   vehicle notify radius 50 m, view multiplier 1.0 (clamped 0.1..1), snipe suitability 70 m
   (0..100), BF1 snipe ranges 140 (used as 70 m radii), BF2 snipe-range bindings are stubs.
10. **Hypotheses, not facts**: the identity of the >1 damage multiplier (item 8), the four
    combat-mode meanings, the facing-factor axis convention, the cone being 77.5° rather than
    12.5°, the dodge trigger being a roll, `SetAIDamageThreshold`'s semantics, the soldier class
    keys behind +0x834..+0x840.

---

## 8. Open questions

1. ~~The >1 multiplier gated by `FUN_001e0580` (health component `+0xbc` vtbl+0x24 on the hit record): which ODF/class value it is. re-blaster-damage found no headshot scaling in the ordnance chain, so this is on the victim side (a per-collision-zone or armour scale?).~~ — **closed, §9.2: the ODF key **`HitLocation`** (fnv1a 0xced77479, `"<collision node> <mult>"`), a per-class (node index → factor) table read by `FUN_002699a0`; on PS2 it ships only as the ×4 vehicle critical-hit prims (15 BF2 vehicle classes; BF1 two ×0 prims), default 1.0.**
2. `FUN_002aba38` / `FUN_00238b38` mode codes 2..5 (combat movement modes) — meaning. (Structure in §9.4: these are the engagement-mode codes of the attack-suitability matrix `FUN_002ac8f8`, not movement modes; the 2/3/4/5 handlers were not traced — meanings still open.)
3. Line-of-sight: neither `FUN_002e3aa0` nor `FUN_002a7230` casts a ray; check `FUN_002aea08`,
   the tracker `+0x68/+0x6c/+0x70/+0x74` states and `FUN_002ac8f8`'s per-kind handlers.
4. ~~The sin/cos table origin `DAT_00739860` is BSS; the 77.5° reading assumes `FUN_00539ff8` is the cos entry (+4). Confirm from the table-fill routine.~~ — **closed, §9.1: the fill `FUN_00539ed0` stores (sinf, cosf) pairs at 2π/2048 steps, `FUN_00539ff8` is the cosine entry, `DAT_00697e38 = cos(1.35263 rad) = 0.2164` → **77.5°** confirmed (BF1 `FUN_002357d0` identical, no 10 m omni).**
5. `SetTeamAggressiveness` in BF2: no reader of `team+0x90` located (BF1 reads `team+0x94` in
   the weapon-choice score). Either BF2 dropped the consumer or it reads through a getter not
   matched by the greps.
6. Soldier class fields +0x834/+0x838/+0x83c/+0x840 (defaults 10/20/30/0): the ODF keys
   (the class parser writes them through a path the `0x834) =` grep did not catch).
7. `SetAIDamageThreshold`: every reader tests `+0x18c != 0`; whether the value is ever compared
   to damage taken was not traced.
8. ~~Whether the shipped BF2 shell calls `ScriptCB_SetDifficulty(level, true)` (which would switch the human's auto-aim / aim-assist / sticky reticule OFF on Elite).~~ — **closed, §9.3: yes — `ifs_difficulty` compiles to `ScriptCB_SetDifficulty(CurButton, true)`; the four re-apply sites pass one argument and leave the flags alone; BF1 has no such switch.**
9. `FUN_002a7fd8 > 0` → perfect aim pass-through: confirm it is "a human occupies the same
   vehicle" (the loop counts controllers with `+0xc0 ≥ 0` over the entity's occupant list).

---

## 9. Follow-up (2026-09-01, session 8, RE 2)

Sources as in the header, plus raw vtable reads from the ELF (`scratchpad/re_phase_c/vtfind.py`),
the PS2 shell Lua 5.0 bytecode (`extracted/bf2/lua/shell/*.luac`, undumped with
`scripts/gen_missions.py`'s `undump50`/`calls50` plus a raw CALL-operand scan,
`re_phase_c/undump_diff.py`), and the PS2 LVL class chunks (`extracted/*/lvl/**/SIDE/*.LVL`, scanned
for the property hash). Closes §8 items 1, 4 and 8; gives item 2 and the dodge their structure.
Same tags: **[V]** read from decomp / raw bytes, **[D]** derived, **[H]** hypothesis.

### 9.1 The AI view cone is a 77.5° half-angle (closes §8.4) [V]

**The table and its fill.** `FUN_00539ed0(table)` (BF2, line 668931) writes 2048 pairs:
`table[2i] = FUN_0052a878(i·0.0030679617)`, `table[2i+1] = FUN_0052a688(i·0.0030679617)`, and
0.0030679617 = 2π/2048. It runs once at start-up: `FUN_0053a138` → `FUN_0053a108(1, 0xffff)` →
`FUN_00539ed0(0x739860)`, the BSS table `DAT_00739860` (`elfrd`: zero at load). The readers scale
their argument by 325.9493 = 2048/2π, so they take **radians**: `FUN_00539f88` reads entry +0
(line 668966), `FUN_00539ff8` reads entry +4 (668983), `FUN_0053a070` returns both (669000); the
index is masked `& 0x7ff` and linearly interpolated to the next entry.

**Which entry is which.** `FUN_0052a688` (657772) and `FUN_0052a878` (657865) are fdlibm
`cosf`/`sinf`: both test `|x| < 0x3f490fd9` (π/4) and reduce with `FUN_0052f0b8`
(`__ieee754_rem_pio2f`). The kernels: `FUN_00530560` (660689) returns 1.0 for tiny x and evaluates
`1 − x²/2 + x⁴·(0.041666668 − 0.0013888889·x² + …)` — `__kernel_cosf`; `FUN_00531008` (661082)
returns x for tiny x with S1 = −0.16666667 — `__kernel_sinf`. `FUN_0052a688` uses the cos kernel
below π/4 and `−kernel_sin` in quadrant 1 → **cosf**; `FUN_0052a878` uses `kernel_sin(x, 0, 0)`
below π/4 → **sinf**. Hence entry +0 = sin, entry +4 = cos: **`FUN_00539ff8` is the table
cosine** (`FUN_00539f88` sine, `FUN_0053a070` sincos).

**The constant.** `FUN_002a7230` (227877) lazily sets `DAT_00697e38 = FUN_00539ff8(0x3fad22fd)`
under the guard `DAT_00697e3c`; both are BSS (zero at load — there is no `.data` value, the number
exists only after the first call). 0x3fad22fd = 1.3526303 rad = **77.5000°**. The lookup lands at
index 440.889 (entries 440/441) and interpolates to 0.21644 (true cos 77.5° = 0.21644).

**The test, exactly** (`FUN_002a7230(ai, target, &d²)`):
```
p   = target.vcall(+0x58)(ai+0xd0 /*eye*/, ai+0xe0 /*view dir*/, -1)   // a point on the target
off = p − eye ;  d² = off·off
if d² < 100 (10 m)        → SEEN (d² written out)                       // omni
elif d² ≤ 0.0001          → SEEN
else dot = viewdir·off ;  SEEN iff dot > 0  &&  d²·c² < dot²,   c = DAT_00697e38
```
With c = cos 77.5° > 0 and dot > 0 this is `dot/d > cos 77.5°`: the angle between the view
direction and the target is **below 77.5°**, a 155° full cone. The 12.5° reading would need the
+0 (sine) entry and is ruled out.

**Callers** [V]: `FUN_002e3aa0` (perception scan, 270386) skips the cone for the active goal
record's current target (`goal+0x180`, id-checked against `+0x184`) when `ai+0x560 == 4`, and
every candidate still needs `d² < viewdist²` (`FUN_002e3910`); `FUN_002b7b00` (239602) scores a
target 4 (< 7 m) / 2 (> 20 m) and subtracts 1 when it is outside the cone; `FUN_00334ab8` (327622)
weights a query by `6400 − d²` (an 80 m budget).

**BF1** [V]: the same table code — `FUN_00445208` fills `DAT_0066aec0` (504768; entry +0 =
`FUN_00434218` = sinf, +4 = `FUN_00434028` = cosf, same fdlibm shapes; cos kernel `FUN_0043bfa8`
returns 1.0 at 0) and `FUN_00445330` (504823) is the table cosine. The cone test
`FUN_002357d0(ai, target, &d²)` (149801) recomputes `cos 77.5°` on every call (no cache) and has
**no 10 m omni branch**: `d² > 0.0001 → dot > 0 && dot² > d²·cos²(77.5°)`, smaller distances
always pass. Its perception caller `FUN_00242740` (159550) bypasses the cone for one pre-selected
target `a local` — the unit's current target when `FUN_0023a1d0` (153127) puts it inside the
unit's aim-arc `[+0x1124, +0x1128]` (a yaw-limit test: turrets and vehicles) [D]; otherwise the
scan radius is `clamp(FUN_00237f38(ai), 75, 350)` m (0x42960000 / 0x43af0000). Two more BF1 users
of the same constant are facing tests, not perception: `FUN_00234f20` (149354) and `FUN_00257910`
(174633) return `cos 77.5° < dot` of two unit vectors.

For openbf: keep the 77.5° half-angle; the 10 m omni is BF2-only.

### 9.2 The level-3 multiplier is `HitLocation` — the vehicle critical-hit zones (closes §8.1) [V]

> **Corrected 2026-10-27 (issue #41, `re-hitzones.md`):** soldiers DO have a per-node multiplier: every soldier
> class with no ODF `HitLocation` (all of them) gets a hard-coded default {head prim, x3.0} - BF2 `FUN_002517e8`
> (B2:170444-170450, node 0 = coll_head), BF1 `FUN_0020e2e0` (B1:122725, node 2). Droidekas (walkerdroid) and the
> acklay get none. The soldier's TakeDamage is BF2 `FUN_00238e20` (its health vtable `DAT_0061ea80`, slot +0x24 ->
> `FUN_0024ec88`), which this section's chain missed; `headShots` / the "headshot" effect count these soldier hits.


**Chain (BF2).** `FUN_0020a940` (soldier TakeDamage, 121219) → `FUN_001e0580(soldier+0x170, desc,
dir, pos, hit, …)` (91541). The "health component" is the sub-object at entity+0x170 with its vptr
at +0xbc (GCC 2.95 "vptr after the fields"; the tables carry two header words, so vtbl+0x24 is
word 9). `mult = (comp+0xbc)->[+0x24](comp, hit)`. Nine vtables hold `FUN_001e0580` at +0x1c
(`vtfind`): the base `DAT_006195e0` (ctor `FUN_001df1a0`; +0x24 = `FUN_0048f0d0`, a `j 0x490e20`
trampoline) and the entity-embedded ones `DAT_00618fe8 / 0061b400 / 0061cef8 / 0061d5d0 / 0061e2f8
/ 0061fc90 / 0062c1d8 / 0062c7c8` (offset-to-top −0x170; installed by `FUN_001d0eb0 / 001ecba0 /
00224fb8 / 0022b028 / 00232930 / 00267bf0 / 0035cb68 / 00366118`), every one of them with
+0x24 = **`FUN_001d41c8`** → `FUN_002699a0(comp − 0x170, hit)`, the entity method (186280):
```
if hit == −1: return 1.0
cls = entity+8 ; n = cls+0x2bc ; tbl = cls+0x2b8        // pairs {int node_index; float mult}
for i < n: if tbl[i].node == hit: return tbl[i].mult
return 1.0
```
**The property.** The class parser `FUN_00214600` (127577; reached from `FUN_001d1f58` 80730 and
`FUN_0022f6e8` 145954/146053) fills the table on hash **0xced77479 = fnv1a("hitlocation")** — the
ODF key `HitLocation = "<collision node> <multiplier>"` (format `"%s %f"` at 0x61c508, value
default 1.0): the node name is hashed (`FUN_005393c0`), resolved to the class's collision-node
index (`FUN_001681c8(cls+0x168, hash)`, must be ≥ 0), and the pair is appended only when the value
≠ 1.0, at most 10 (`FUN_0059d480(0x50)`, `9 < count → return`). **Default**: the class ctor
`FUN_002142d8` zeroes `+0x2b8/+0x2bc` (127448) — no entries, so every node not listed is ×1.0;
there is no data-segment constant. `hit` is the collision-node index of the prim the ordnance
struck, passed through from the impact handler (re-blaster-damage §6.2's "owner interface +0x1c" is
this slot). This corrects re-blaster-damage §6.2 ("no per-bodypart multipliers anywhere in the
chain"): the per-node multiplier is the `argument 1` of `FUN_001dfb38`, computed one frame up.

**What ships** (PS2 LVL class chunks, `HitLocation` PROPs found by hash bytes `79 74 d7 ce`):

| game | class (BASE) | value |
|---|---|---|
| BF2 | all_hover_combatspeeder, all_hover_bos_combatspeeder, cis_hover_aat, cis_tread_hailfire, imp_hover_fightertank, rep_hover_fightertank (hover); rep_walk_atte (commandwalker) | `p_crithit 4.0` |
| BF2 | cis_tread_snailtank (hover) | `p_-o_crithit 4` |
| BF2 | cis_walk_spider, cis_walk_spiderkas (walker) | `p_crithit1 4.0`, `p_crithit2 4.0` |
| BF2 | imp_walk_atat (commandwalker) | `p_neck_soldier 4.0` |
| BF2 | imp_walk_atst, imp_walk_atst_jungle, imp_walk_atst_snow (walker) | `p_crit_hit 4.0` |
| BF2 | rep_walk_oneman_atst (walker) | `p_-so_hips 4.0` |
| BF1 | cis_tread_hailfire (hover), rep_fly_vtrans (carrier) | `p_crithit 0` |

No soldier, hero, droideka, turret or flyer class carries one in either game: on PS2 the >1
multiplier is the **vehicle critical-hit zone, ×4** (BF1 uses the key only to make two prims
immune, ×0). It is what `hitCritical` / `headShots` count (`FUN_0035fd50(…, 1.0 < mult,
zone = comp+0xb8 & 7, …)`, 91673) and what plays the `"headshot"` feedback object
(`FUN_00539150(0x71fc88, 0x200, 0xd7a77563)`, 0xd7a77563 = fnv1a("headshot"), played at the hit
position by `FUN_004522a0`; BF1 `FUN_001bc748` 64474 uses the same hash).

**The gate, exactly** (`FUN_001e0580` 91570–91584):
`if mult > 1.0 and FUN_002a29c8() < 3 and (desc[0] == NULL or desc[0]+0x130 < 0): mult = 1.0`.
`desc[0]` is the attacker entity; `+0x130 ≥ 0` marks a human-driven entity — the same test lets
`FUN_001dfb38` (91321) exempt a human victim (controller `+0xc0 ≥ 0`) from an AI attacker's
friendly fire, and gates the client-side feedback in `FUN_001df7d0` (91170) [D]. So: a **human
attacker always** gets the ×4 on a critical prim; an **AI attacker** — any entity kind, the test
is the attacker's human flag, not its class, so AI-driven vehicles, turrets and heroes included —
gets it **only when the mission-start level is 3** (`DAT_00697e30` = profile+0x3c, or the host's
`iDifficulty` in netplay, §1.1). No other level test exists anywhere in the damage path. **BF1 has
no gate**: `FUN_001bc748` (64451) takes `mult = (comp+0xa8)->[+0x20](comp, hit)` and applies it for
every attacker; `DAT_005899a0` (the BF1 level) is never read by damage code.

**What it multiplies**: the `argument 1` of `FUN_001dfb38` — `dmg = desc.damage × mult × desc.scale2`
(91317) — damage in, before the friendly-fire scale (`DAT_005fcbe0`), before the shield phase and
before the HealthType (armour) scale.

For openbf: the PS2's level-3 lever is not a damage buff against humans; it is whether bots may
score ×4 critical hits on vehicle weak-point prims (and the "Critical Hit" feedback). PLAN Phase C
item 4's human damage multiplier (1.0 / 1.0 / 1.15 / 1.3) is therefore ours, not a port; the
faithful Elite extra is "AI critical hits on vehicle prims allowed" once vehicles exist.

### 9.3 Yes — the PS2 BF2 difficulty screen passes `true` (closes §8.8) [V]

`ScriptCB_SetDifficulty` (0019b588, line 42888): `nargs` must be 1 or 2 and arg 1 a number →
`profile+0x3c = clamp(level, 1..3)` (default 2). **Only when `nargs == 2` and arg 2 is a boolean
(`FUN_0049f530(L, 2) == 1`) that is true (`FUN_0049f868`)** does it write `profile+0x32`
(`bAutoAim`) = `+0x208` (`bAimAssist`) = `+0x209` (`bStickyReticule`) = `(level < 3)`; then
`FUN_004362f8(profile)` marks the profile dirty. With one argument the three flags are untouched.

Call sites in the shipped PS2 `SHELL.LVL` (`extracted/bf2/lvl/DATA/_LVL_PS2/SHELL.LVL`, 92 `scr_`
chunks, five mention the binding; `CORE.LVL` none):

| chunk | call (abstract-interpreted) | raw Lua 5.0 operands |
|---|---|---|
| **`ifs_difficulty`** (the PS2-only difficulty screen) | **`ScriptCB_SetDifficulty(this.CurButton, true)`** | `GETTABLE r2 ← .CurButton; LOADBOOL r3 = true; CALL r1 B=3` → **2 args** |
| `ifs_sp_briefing`, `ifs_instant_top`, `ifs_freeform_main`, `ifs_campaign_battle` | `ScriptCB_SetDifficulty(ScriptCB_GetDifficulty())` | `CALL B=0` (pass-through of the inner call's single result) → 1 arg |

So picking Elite on the difficulty screen switches the human's auto-aim, aim assist and sticky
reticule **OFF**, and picking Easy/Normal switches all three **ON**; the four re-apply sites leave
them alone, and no PS2 shell chunk references `bAutoAim` / `bAimAssist` / `bStickyReticule` at all
(constant scan over all 92 chunks; `iDifficulty` appears only in `ifs_instant_options`) — the
difficulty screen is the only shell writer of those flags. Consumers: `FUN_002b4598` reads `+0x209`
(236851) and `+0x208` (236875); `FUN_0034e068` (344713 — the attacker hit-feedback call at the top
of TakeDamage) and `FUN_004aa5c8` (569339) read `+0x32`. The PC shell differs and is hypothesis
only: PC BF2 `ifs_opt_general.lua` never calls `ScriptCB_SetDifficulty` (it stores
`GeneralOpts.iDifficulty`); PC BF1 calls it with one argument.

**BF1** [V]: `ScriptCB_SetDifficulty` (0017c7c0, 21891) requires exactly one argument
(`FUN_003a6378() == 1`), writes `DAT_005899a0` and `profile+0x4c`, and touches no aim flag; BF1's
auto-aim toggle is the separate `ScriptCB_SetAutoAimOn` → `profile+0x34` (re-aim-assist §1). **No**:
BF1 does not couple aim assist to Hard.

For openbf: "Normal feels like old Elite" on the PS2 also meant the human's aim assist was off —
a session/profile decision for the lead, not an AI parameter.

### 9.4 The dodge and the "combat modes" — structure only [D/H]

**Dodge** (`FUN_0020a940` 121236–121318, AI soldiers only: `+0x330 < 0`, `+0x5bd`/`+0x5be` clear,
`+0xa0` bit 0 clear, cooldown `+0x1d80 ≤ 0.0001`): draw `U`; dodge iff `U < lerp(0, 0.2)`
(`FUN_002a3018`) and the controller's `+0x6cc` byte is set. Then (a) if `+0x660` bit 3, `+0x60c
== 2`, `+0x228` bit 3, and `+0x660` bits 0/1 clear → `FUN_002fa478(soldier+0x4c0, ctrl+0x654)`;
(b) `+0x1d80 = 2 s`; (c) unless `+0x660` bits 0/1 or `+0x1d88 > 0`: `+0x1d88 = 10`,
`+0x1d84 = (U − 0.5)·lerp(0, 1.5) + 2.25`. `FUN_002fa478` (287793) is a **reset of the movement
sub-object embedded at soldier+0x4c0** (vptr at +0x4f0): it clears `+0xf0`, `+0xfc..+0x102`
(`FUN_002fa4c0`) and re-seeds `+0x58` from the controller (`FUN_002feaa8` → `FUN_00300d30`). The
soldier update `FUN_0020b840` issues the identical call when leaving `+0x60c` state 3 (122085), and
`FUN_00202880` arms the same `+0x1d84/+0x1d88` pair with 5 s / 1.5 outside the damage path
(116339). The roll itself is dispatched by `FUN_002475f0` → vtable+0xb0 from the input predicate
`FUN_00247b88` (re-stance-roll §4.2), which this path never reaches. **"Dodge = roll" is not
supported**: the on-hit dodge is a re-plan of the soldier's movement plus a 2.25 ± 0.75·lerp(0,1.5)
s evade parameter [D]; what `+0x1d84` drives (strafe direction / duration) stays [H].

**Modes** (`FUN_002aba38`, 231136) are not movement modes: the function is the last stage of the
**attack-suitability matrix** `FUN_002ac8f8` (231784, §5.1). Attacker kind × target kind picks a
handler (`FUN_002abd38` / `FUN_002abe18` / `FUN_002abda8` for a soldier-kind attacker against
target kinds 0/1/3; `FUN_002abc38(…, 4, …)` for attacker kind 1; …) → `FUN_002abc38` (231214) →
`FUN_002aba38`. Outputs: `out[0]` = engagement-mode code, `out[1]` = 0.5, `out[2]` = the
"can engage" byte from the situation query `FUN_002ab590` (230907: walks the unit's weapons against
the target's distance and the point from `vcall +0x58`; `DAT_00689828` soldier test). Codes: 0 =
cannot engage; 7 when the situation code is 4; otherwise a draw over {2, 3, 4, 5} with the level
weights Easy {3,1,1,1} / Normal {2,2,1,1} / Elite {1,1,1,1}, where 4 needs `ai+0xb4→+0x60 > 10`
and 5 needs `> 5` (the same float — a range [H]), and a drawn 2 becomes 3 unless the situation's
second flag is set. The code travels with the acquisition result (`FUN_002e3aa0` 270391; the planner
at 256266 stores it in the goal record) — the handlers that act on 2/3/4/5 were not traced, so the
**meanings stay [H]** (an attack style chosen per weapon set and range, not a movement mode).

Still open after this pass: §8.2 (mode handlers), §8.3 (LOS), §8.5–§8.7, §8.9.
