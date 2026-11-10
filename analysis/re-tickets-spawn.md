# RE: Conquest ticket / reinforcement / spawn-reserve system (BF1 + BF2)

Sources: `analysis/SWBFRONT.ELF-decomp.c` (BF1, SLUS-208.98), `analysis/SWBFRNT2.ELF-decomp.c`
(BF2, SLUS-212.40), raw MIPS from the ELFs (capstone) where the decomp was ambiguous,
extracted mission Lua bytecode (`extracted/{bf1,bf2}/lua/`, disassembled with
`scratchpad lua40dis.py` / `lua50dis.py` written for this task), and LVL data
(`bake classes`, raw PROP-hash scans). Confidence tags: **[V]** = verified by reading
decomp/asm/bytecode directly; **[D]** = derived (algebra over verified pieces);
**[H]** = hypothesis.

---

## 0. Executive summary / the key answers

| Question | BF1 | BF2 |
|---|---|---|
| Where do tickets live | team obj `+0x10` cur / `+0x14` initial (mgr `DAT_00518388+0x18+t*4`) [V] | team obj `+0x24` cur / `+0x28` initial (`PTR_DAT_005fee78+t*4`) [V] |
| **Spawn or death consumes a ticket?** | **DEATH** — engine, `FUN_00297440`: `if (tickets>0) tickets--` [V] | **DEATH** — Lua, `Objective.lua` `OnCharacterDeath`: `if GetReinforcementCount(t)>0 then AddReinforcements(t,-1)` [V] |
| Spawning consumes | nothing (spawn only bumps the alive counter) [V] | nothing (same) [V] |
| No-overspawn invariant | **engine**: AI respawn blocked while `tickets <= alive` (`FUN_00295c40`) [V] | **engine**: AI respawn blocked while `tickets <= alive` (`FUN_0034cea8`) [V] |
| Alive counter | team `+0x2c`, ++ at spawn / −− at death (`FUN_002966f0` / `FUN_00297440`) [V] | team `+0x38`, ++ at spawn / −− at death (`FUN_0034e1f0` / `FUN_0034f650`) [V] |
| CP-majority bleed | **engine bleed, LIVE** (`FUN_002ad830`): 0.1875 / 0.5625 / 0.75 tickets·s⁻¹ at two / one / no posts — §2.4 as CORRECTED 2026-09-30 [V][L] (this row said "provably inert") | **Lua-driven** (`ObjectiveConquest.lua`), engine `SetBleedRate` is display/replication only; default 1 ticket / 3 s at CP-value majority [V] |
| Loss at 0 tickets | tickets forced to 0 by 20-s no-CP timer; end-of-game dispatcher not pinned (open §5) | Lua `OnTicketCountChange`: `count <= 0 → MissionDefeat(team)` [V] |

The openbf "no-overspawn invariant" is not BF1-only — **both** engines gate AI
(re)spawn on `alive < tickets`, and both make death (not spawn) consume the ticket.
BF1 additionally *culls* units when the reserve is exhausted (§2.6).

---

## 1. BF2 (SWBFRNT2.ELF) — shipping baseline for openbf

### 1.1 Team object (size 0x2a4, ctor `FUN_0036d468`, array `PTR_DAT_005fee78[8]`, vtable 0x62d080 at +0x2a0) [V]

| offset | meaning | evidence |
|---|---|---|
| +0x04 | team index 0..7 | ctor |
| +0x24 | current reinforcements (int) | `GetReinforcementCount` @0018c548 reads it |
| +0x28 | initial reinforcement count (int) | `SetReinforcementCount` @0018c3e8 writes both |
| +0x2c | bleed rate (float) — HUD/VO/replication only, engine never drains from it | `SetBleedRate` @0018d210 → `FUN_0036ea18`; `GetBleedRate` @0018d270 |
| +0x30 / +0x34 / +0x3c | member capacity / member count / member (controller) array | `FUN_0036e3e0` (add member, vtbl+0x1c), `FUN_0036e470` |
| +0x38 | **alive count** (units whose class is not ticket-exempt) | `FUN_0034e1f0` ++ / `FUN_0034f650` −− |
| +0x44 / +0x4c / +0x50 / +0x54 | unit-class count / class array / max-per-class / min-per-class | `FUN_0036f198` (AI class picker) |
| +0x5c | team points | `SetTeamPoints` @0018c610 |
| +0x60 | faction id (1 alliance, 2 empire, 3 republic, 4 CIS, 5+ locals) | `FUN_0036d9f8` name-hash switch |
| +0x84.. | per-team relation bytes | `SetTeamAsFriend/…` |
| +0x2a0 | vtable (0x62d080); +0x14 entry `0x36de28` is just an "is networked" getter — **there is no per-frame engine team-update touching tickets** | raw dump of 0x62d080 + hand-disasm of 0x36de28 |

Mode/session manager `DAT_005fee60`: `+0x18+t*4` = SetUnitCount value (@0018c2c8);
`+0x58` = CP-derived spawn budget for locals; per-team spawn-wave timers +0x9c.., refill
delays +0x5c.. (`SetSpawnDelay` @0018d348, `FUN_0036c750` = manager update). [V]

### 1.2 Ticket writes — complete list [V]

Replication channel `0x72100c` is attached to team+0x24; grepping every reader/writer of
that channel proves the only writers of tickets in the whole binary are:

- `SetReinforcementCount` (Lua): negative arg → `0x7fffffff` (infinite). A one-shot
  percentage rescale by mission-suffix (`_con`/`_dm`/`_tdm`) × table
  `DAT_005fa7d8/f0[DAT_005fa7d0]`/100 exists but both tables are all-100 in the shipping
  ELF → no-op. [V] (**With the stock options only:** the Instant Action options screen sets
  the `_con` table to 10–500 % — `analysis/re-time-limit.md` §5, 2026-09-24, `[O]`.)
- `AddReinforcements` (Lua) → `FUN_0036e0d8` (guarded: server only).
- `FUN_0034bdb0` — galactic-conquest-style bonus: when `tickets <= 0.2 * initial`, calls
  the add-helper once (bonus hash 0x96f36950); latch byte prevents repeats. [V, arg count
  lost by decomp]

**The engine never decrements tickets on death or spawn.** Death consumption is done by
mission Lua (below).

### 1.3 Lua layer (verified from PS2 bytecode, RK-base 128 build of Lua 5.0.2)

`objective.luac` (`Objective.lua`), registered in `Objective:Start` when the objective is
not inside a container [V]:

```lua
OnCharacterDeath(function(character, killer)
  local team = GetCharacterTeam(character)
  if GetReinforcementCount(team) > 0 then AddReinforcements(team, -1) end
end)
OnTicketCountChange(function(team, count)
  if count <= 0 then MissionDefeat(team) end
end)
```

→ **death consumes the ticket (floor 0); hitting 0 is immediate defeat.**
Time-limit expiry (`GameOptionsTimeLimitUp`, ObjectiveConquest main.f6): more tickets
wins; tie → `MissionVictory({1,2})`. [V] Stock SP conquest never arms that timer — the
options value is 0 = Off and no conquest mission sets a limit (§6 below; the record is
`analysis/re-time-limit.md`).

`objectiveconquest.luac` (`ObjectiveConquest.lua`) [V]:

- Class constants: `defaultBleedRate = 0.33333334` (1/3 ticket/s), `defeatTimerSeconds = 20`.
- `AddCommandPost` accumulates `totalBleedValue[team] += GetCommandPostBleedValue(cp, team)`.
- `AddBleedThreshold(team, threshold, rate)` fills `bleedRates[team][threshold] = rate`
  (PS2 stock conquest missions do **not** call it — checked tat2c_con; the default branch
  is the shipping path).
- Bleed recompute (main.f7.f0), run per team on every capture/neutralize event **and only
  when `multiplayerRules` is true** — and PS2 retail SP missions pass
  `multiplayerRules = true` (verified in tat2c_con.luac ScriptPostLoad), so this IS the
  shipping SP path:
  - `enemyBleedValue = Σ GetCommandPostBleedValue(cp, owner)` over CPs owned by the
    opposing team.
    A post held by a locals (native) team is not the opposing team, so it adds nothing to
    either army's enemy value, but it stays in `totalBleedValue` (openbf `cp::bf2_bleed_rate`).
  - with thresholds: rate of the highest threshold ≤ enemyBleedValue;
  - default: `if totalBleedValue[team]/2 < enemyBleedValue then rate = 1/3 else 0`
    (strict majority of CP bleed value).
  - `SetBleedRate(team, rate)` (HUD/VO), then a per-team Lua timer:
    `SetTimerValue(1); SetTimerRate(rate)` → elapses every `1/rate` seconds.
- Bleed timer elapse (main.f7.f3.f0):
  ```lua
  if GetNumTeamMembersAlive(team) < GetReinforcementCount(team) then
      if GetReinforcementCount(team) > 0 then AddReinforcements(team, -1) end
      SetTimerValue(timer, GetTimerValue(timer)+1); StartTimer(timer)
  else SetBleedRate(team, 0); StopTimer(timer) end
  ```
  → **bleed clamps at the alive count** (never drains a team below its currently alive
  units) and stops at 0.
- Domination: when every alive CP belongs to one team → 20 s defeat timer →
  `Complete(winner)`. `disallowDefensiveVictory = not multiplayerRules` (false on PS2
  retail → defenders may also win this way). [V]

`GetNumTeamMembersAlive` @00190af0 → `FUN_0036e4e8`: counts non-null member slots. [V]

### 1.4 CP bleed values [V]

`GetCommandPostBleedValue` @00191b38 → `FUN_00356858`:
`return *(cp + 0xb8 + factionId(team)*4)` — per-faction int array, ODF keys parsed as
"%d" in `FUN_00355950`:
`ValueBleed_Alliance`→+0xbc, `_Empire`→+0xc0, `_Republic`→+0xc4, `_CIS`→+0xc8,
`_Locals`→+0xcc.
TAT2 conquest instances (wrld PROP): **ValueBleed = 10 for all four main factions, 0 for
Locals**; the CTF/1flag/eli variants set 0. So on tat2 (6 conquest CPs):
`totalBleedValue = 60`, bleed starts when the enemy holds ≥ 4 CPs (40 > 30). [V]

### 1.5 Spawning, no-overspawn, and the "16"

Controller (member) object: 0x1a4 bytes, pool `DAT_00661330`. Key fields [V]:
`+0x130` chosen unit-class index (<0 = AI awaiting auto-pick), `+0x134` team index,
`+0x138` team ptr, `+0x148` alive entity ptr, `+0x158` respawn timer (death sets 4.0 s;
5.0 s when AI cap reached; 1e-4 for entities with flag bit 4), `+0x165` ticket-exempt
flag = bit 33 of entity+0x330 (hero-class marker [H — bit identity verified, "hero"
semantics inferred]).

Respawn decision `FUN_0034cea8` [V]:
1. **AI ticket gate**: `if (classIdx < 0 && team->tickets(+0x24) <= team->alive(+0x38))
   → wait` — the no-overspawn invariant. Players (classIdx ≥ 0) skip this gate;
   `MissionDefeat` at 0 tickets ends the round anyway.
2. Team overfull (`+0x30 < +0x34`) → member is moved to the other team (autobalance).
3. **AI alive cap**: counts alive AI on the team; `if (DAT_006004fc <= aliveAI) → wait
   5 s`. `DAT_006004fc` (default **16** in the ELF image) is set by
   `ScriptCB_SetNumBots`; `ObjectiveConquest:Start` does
   `ScriptCB_SetNumBots(ScriptCB_GetCONNumBots())` and the CON table falls back to the
   16 default. **This is the "16 per team" from PLAN — it is the BF2 alive-AI cap, not a
   unit count.** SetUnitCount (=20 in tat2c_con via `SetupTeams{units=20}`) sizes the
   member pool (total capped at 64, `FUN_0036c1a0`).
4. Locals teams (idx > 2) additionally capped by `DAT_005fee60+0x58` (Σ CP spawn value of
   friendly CPs).

Spawn `FUN_0034e1f0`: creates entity into `+0x148`, `if (!exempt) team->alive++`. [V]
Death `FUN_0034f650`: clears `+0x148`, `if (!exempt) team->alive--`, respawn timer 4.0,
kill scoring/awards; **no ticket touch**. [V]

`SetSpawnDelay(10, 0.25)` in missions; when networked the first arg is forced to 15.0.
Manager update `FUN_0036c750` runs per-team wave timers (delay/interval slots), same
scheme as BF1 §2.5. [V]

### 1.6 BF2 reference vectors [D from verified pieces]

With tat2c_con (150 tickets, 20 units, 6 CPs à value 10, defaults):
1. First death: 150 → 149 (floor 0; exempt/hero classes still decrement — Lua doesn't
   check the flag).
2. Enemy holds 4/6 CPs: enemyValue 40 > 60/2 → `SetBleedRate(t, 1/3)`; −1 ticket every
   3.0 s while `alive < tickets` and `tickets > 0`.
3. Enemy holds 3/6: 30 > 30 false → rate 0 (majority must be strict).
4. tickets = alive = 16: bleed timer stops (SetBleedRate 0); AI cannot respawn; each
   death −1 ticket and −1 alive; a player respawn makes alive 17 > 16 and bleed can
   resume.
5. tickets reach 0 → `MissionDefeat(team)` immediately (even with units alive).
6. One team holds all 6 CPs (others dead/neutralized): 20 s timer → `Complete(winner)`.

---

## 2. BF1 (SWBFRONT.ELF) — spawn-reserve model for openbf's invariant

### 2.1 Team object (size 0x2a4, ctor `FUN_002acf40`, mgr `DAT_00518388` size 0xb0, ctor `FUN_002ab7f8`) [V]

Teams at mgr `+0x18+t*4` (8 teams). Manager: `+0x38/+0x3c` member count / member pool
(stride **0xe8**, one per unit slot, alloc = Σ unit counts), `+0x44..` per-team
SetUnitCount values (default 0), `+0x64..` created-member counts, `+0x84` locals spawn
budget (recomputed per frame), `+0x88/+0x8c/+0x90` spawn delay / interval / slots-per-wave
(defaults **15.0 / 0.5 / 30**; missions call `SetSpawnDelay(10, 0.25)` → 40),
`+0x94/+0x98/+0xa0` wave timer / wave counter / slot counter, `+0xac` = 600.0.

| team offset | meaning |
|---|---|
| +0x04 | team index |
| +0x10 / +0x14 | current / initial reinforcements (`SetReinforcementCount` @00184df0 writes both) |
| +0x18 / +0x1c / +0x20 | active bleed-threshold index (−1 none) / bleed rate (float) / bleed accumulator |
| +0x24 / +0x28 / +0x30 | member capacity (ctor 40) / member count / member array |
| +0x2c | **alive count** |
| +0x44 / +0x48 / +0x4c | bleed-threshold capacity (ctor: 4 for teams 1–2, 0 otherwise) / count / array of pairs {int threshold, float rate} (`AddBleedThreshold` @00185090) |
| +0x58 | reserve-exhausted latch |
| +0x5c / +0x60 | no-CP defeat countdown (init 20.0) / countdown-active flag |
| +0x64 | faction id |
| +0x88.. | relation bytes; +0x90 ally index |
| +0x2a0 | vtable; +0x14 = `FUN_002ad830` team update (called with dt from `FUN_002ac378`) |

Locals teams (3..7) are constructed with `tickets = initial = 0x7fffffff`. [V]

### 2.2 Member object (stride 0xe8, ctor `FUN_00295af0`) [V]

`+0x9c` unit-class index (<0 = AI), `+0xa0` team idx, `+0xa4` team ptr, `+0xb0` chosen CP,
`+0xb4` alive entity ptr, `+0xc0` respawn timer, `+0xc4/+0xc8` assigned wave / slot,
`+0xd0` ticket-exempt-from-alive flag (copied at spawn from `*(entityWrapper+0xe4)+0x7c`,
a class field — identity open, §5).

### 2.3 Spawn / death — ticket flow [V]

- Spawn `FUN_002966f0`: `member+0xb4 = entity`; `if (flag(+0xd0)==0) team->alive(+0x2c)++`.
  **No ticket change.**
- Death `FUN_00297440`: awards/scoring; `respawnTimer = 4.0`; `member+0xb4 = 0`;
  `if (flag==0) alive--`; **`if (tickets > 0) tickets--`** (unconditional w.r.t. flag).
  The ticket comes from the dying member's own team, so a native's death spends only its
  locals team's unlimited pool; BF2's `OnCharacterDeath` likewise charges the dying
  character's own team. Killing an allied native never costs the army a ticket.
- HUD (`FUN_0033f3c8`) renders team1/team2 `+0x10` directly, so the on-screen counter
  drops by exactly 1 per death.

### 2.4 Engine bleed `FUN_002ad830` — algorithm verified, retail data makes it a no-op

> **CORRECTION (2026-09-30, card ANNC, `analysis/re-announcer.md` §5): the "no-op" conclusion below is WRONG — BF1's
> conquest bleed is live on the PS2.** The steps 1–3 and the registration order are right, but the table is not
> scanned in registration order: the team's vtable slot +0xc, `FUN_002ad2f0` (data word at 0x53ef04), calls
> `FUN_0042a150(team+0x4c array, team+0x48 count, 8, 0x2ad2e0)` (B1 238416) — a qsort whose comparator at 0x2ad2e0 is
> `lw v1,0(a0); lw v0,0(a1); jr ra; subu v0,v1,v0` (ascending by threshold). Live PCSX2 RAM (BF1 NAB1, both teams):
> `[(1, 3.0), (11, 2.25), (21, 0.75), (31, 0.0)]`. So the scan of step 2 selects the TIGHTEST threshold: value ≤ 1 →
> 3.0, ≤ 11 → 2.25, ≤ 21 → 0.75, ≤ 31 → 0, above → none; drain = rate / 4 = **0.75 / 0.5625 / 0.1875 / 0
> tickets·s⁻¹** — exactly the mapping the [H] paragraph below calls "intended". Measured: team 1 on 2 of 5 posts
> (value 20, threshold index 2, rate 0.75) lost one ticket at each of 6 accumulator wraps in 32.2 s of game time
> (expected 6.04), beside its death decrements; team 2 on 3 posts had rate 0. The bleeding voice line does play
> (`cis_off_com_report_us_overwhelmed`, every 30 s). Open question §5.2 is closed by this. The port plays BF2's
> bleed rule on every map (PLAN §Conquest), so its sim is unaffected; only this text and `core/src/battle.rs`'s
> module comment were wrong.

Verified against raw MIPS (threshold block at 0x2ada14–0x2ada78):

1. `value = Σ cp->ValueBleed(+0x9c)` over CPs **owned by this team** `+ 1` per neutral
   CP. (`cp` iterated from list `DAT_0051833c`; owner = cached-flag-entity `+0xd8`;
   ValueBleed parsed "%d" by `FUN_0029d1d0`, class default 100 (`FUN_002a0b28`
   `[0x3c]=100`), overridden per instance in the world files — TAT1: 10 on the 6 real
   CPs, 0 on 3 auxiliary ones.)
2. Threshold pick, only while `alive(+0x2c) < tickets(+0x10)`:
   `if value <= t[0] → rate = r[0]; else first i≥1 with value <= t[i] → r[i]; else none`.
3. Drain: `accum(+0x20) += rate * dt; while accum >= 4.0: accum -= 4.0; if tickets > 0:
   tickets--` → **drain = rate/4 tickets per second**.

Every PS2 conquest mission (checked 10+: bes1a/2a, kas1c, nab1c, yav1c, geo1r, end1a,
hot1i, rhn2c, tat1r, tat2i) registers the identical thresholds, in this order:
`(31, 0.0), (21, 0.75), (11, 2.25), (1, 3.0)` [V].

**Algebraic consequence [D]:** with CP values of 10, `value` is 0/10/20/30/40/50(+small
neutral noise). `value ≤ 31` always selects the *first stored* pair → rate **0.0**;
`value > 31` never matches a later (smaller) threshold → no rate. The selected bleed
rate is therefore always 0: **BF1 PS2 conquest has no passive ticket bleed; tickets fall
only on deaths.** This also matches `FUN_002b1320` (announcer): its "bleeding" VO fires
only on a rate > 0 transition — i.e. never in retail conquest.
[H] The data was clearly authored for a tightest-threshold ("last match / ascending
scan") rule — intended mapping value≤1→3.0, ≤11→2.25, ≤21→0.75, ≤31→0 t/s, i.e.
drain 0.75/0.5625/0.1875/0 tickets·s⁻¹ — and the shipped first-match scan defeats it.
Verify once in PCSX2 before porting a "fixed" version anywhere.

### 2.5 Spawn gating / no-overspawn / caps — `FUN_00295c40` (member update) [V]

While alive: if the unit's class flag ≠ 0 **and** `alive >= tickets` → entity health is
forced to 0 (`FUN_001bbc98`) — flagged units are culled the moment the reserve is
exhausted.

Respawn path (timer `+0xc0` expired):
1. **AI ticket gate**: `if (classIdx(+0x9c) < 0 && team->tickets <= team->alive) → wait.`
2. Overfull team (`count(+0x28) > capacity(+0x24)`) at that moment → member defects to
   the other main team (`3 − teamIdx`).
3. Wave alignment: member spawns only when
   `(memberWave − curWave)*slotsPerWave + (memberSlot − curSlot) < 0` — one spawn slot
   released every `interval` (0.25 s), wave every `delay` (10 s); member wave assigned at
   death = current wave + 1 (classic BF1 respawn waves).
4. AI crowd caps: teams 1–2 with < 3 slot-overruns spawn freely; MP cap
   `DAT_00518bc4` (default 32, `ScriptCB_SetNumBots`); locals (idx > 2) capped by
   `mgr+0x84` = Σ SpawnValue(+0x80) of CPs owned by teams friendly to the locals,
   clamped by `mgr+0x50`.

### 2.6 Exhaustion + defeat mechanics (`FUN_002ad830` tail) [V]

- Reserve exhausted (`alive >= tickets`): first frame sets latch `+0x58` and
  `accum = −900.0`; the accumulator then rises by dt; once ≥ 0 (i.e. **900 s** after
  exhaustion) the engine kills the worst-scoring alive AI member (score
  `statA − 2·statB + 5·statC` via FUN_002a64e8/2a6618/2a6660, minimum chosen) and sets
  `accum −= 15.0` → one forced kill every 15 s (each kill burns a ticket) —
  an anti-stalemate.
- Holds **zero** CPs and no neutrals exist: 20-s countdown (`+0x5c`, VO "defeat
  imminent" at start); at 0 → `tickets = initial = 0` → team can no longer spawn and
  every remaining death is terminal. If the team re-takes a CP the timer resets to 20.
- Locals teams: each update finds the allied main team (relation byte > 0); when the
  ally's reserve is exhausted (`ally alive >= ally tickets`), locals' tickets are zeroed.

> **Corrected 2026-10-27 (`re-round-end.md` §1.4):** this reads the team update alone. The BF1 game-over driver
> (`FUN_0033f718`) ends the round 2.0 s after a main team's reinforcements drop below 1, units alive or not.
> There is no last stand. [V for the code; one live sample owed, capture C5]

### 2.7 Mission-side constants (Lua 4.0 bytecode, verified) [V]

PS2 retail conquest (representative):

| mission | SetUnitCount (T1/T2/locals) | SetReinforcementCount | SetSpawnDelay |
|---|---|---|---|
| tat1r | 25 / 25 / 14 | 250 / 250 | 10, 0.25 |
| bes1a | 28 / 28 | 200 / 200 | 10, 0.25 |
| kas1c | 32 / 25 / 7 | 200 / 200 | 10, 0.25 |
| nab1c | 25 / 25 / 7 | 200 / 200 | 10, 0.25 |
| geo1r | 32 / 25 / 7 | 250 / 250 | 10, 0.25 |
| tat2r | 25 / 25 / 14 | 200 / 200 | 10, 0.25 |

**Correction to PLAN**: "default unit counts are 16/team on PS2" is not BF1 — BF1 PS2
fields 25–32 members/team. The 16 is BF2's default alive-AI cap (`DAT_006004fc`, §1.5).

`SetLowReinforcementsVoiceOver(t, o, snd, 0.1, 1)`-style calls: threshold stored as
`0.1 × tickets-at-load` (binding at 0018cb28); played once when `tickets <= threshold`
(`FUN_002b1320`, latch `+0x8c`). A separate hard-coded `tickets < 26` helper exists
(`FUN_00236e20`, HUD blink [H]).

### 2.8 BF1 reference vectors [D]

tat1r (250 tickets, 25 units/team):
1. Load: 25 AI members created per main team (plus human), alive ≤ tickets holds
   trivially (25 « 250).
2. Death: 250 → 249; the dead AI re-enters the next 10-s wave, one release per 0.25 s.
3. No bleed ever (see §2.4) — count moves only on deaths.
4. tickets = 25 = alive: AI respawns blocked; each death −1/−1; players may still
   respawn (gate is AI-only).
5. Team loses all CPs, none neutral: 20 s → tickets 0 → no spawns; remaining units are
   the last stand.

   > **Corrected 2026-10-27 (`re-round-end.md` §1.4):** the round ends 2.0 s after tickets reach 0, units
   > alive or not; there is no last stand (see the erratum under §2.6).

6. 900 s after reserve exhaustion: engine starts shooting its own worst AI every 15 s.

---

## 3. Function / address table

### BF2 (SWBFRNT2.ELF)
| addr | name | role | conf |
|---|---|---|---|
| 0018c2c8 / 0018c340 | SetUnitCount / GetUnitCount | mgr+0x18+t*4 | V |
| 0018c3e8 / 0018c548 / 0018c5a8 | Set/Get/AddReinforcements | team +0x24/+0x28 | V |
| 0018d210 / 0018d270 | Set/GetBleedRate | team +0x2c (display only) | V |
| 0018d348 / 0018d3f8 | SetSpawnDelay(±team) | mgr wave delays; MP forces 15.0 | V |
| 00190af0 | GetNumTeamMembersAlive | → FUN_0036e4e8 | V |
| 00191b38 | GetCommandPostBleedValue | → FUN_00356858 (cp+0xb8+faction*4) | V |
| 00193e28/0019ba28 | ScriptCB_SetNumBots | DAT_006004fc (default 16) | V |
| 0019b908 | ScriptCB_GetCONNumBots | table 0x5fa808 / default 16 | V |
| 0036d468 | team ctor (0x2a4) | | V |
| 0036de60 / 0036e0d8 | ticket set / add (+replication 0x72100c) | only ticket writers | V |
| 0036c750 | spawn-manager update (dt) | wave timers; calls team vtbl+0x14 | V |
| 0036f198 | AI unit-class picker | min/max per class, random shuffle | V |
| 0034cea8 | controller respawn decision | ticket gate, NumBots cap, autobalance | V |
| 0034e1f0 / 0034f650 | spawn / death | alive ++/−−, respawn 4.0 s | V |
| 0034bdb0 | low-ticket bonus (20% / add) | | V |
| 00355950 / 00355648 | CP class prop parser / capture handler | ValueBleed_* +0xbc.. | V |

### BF1 (SWBFRONT.ELF)
| addr | name | role | conf |
|---|---|---|---|
| 00184d98 | SetUnitCount | mgr+0x44+t*4 | V |
| 00184df0 | SetReinforcementCount | team +0x10/+0x14 | V |
| 00185090 | AddBleedThreshold | team +0x44/48/4c pairs {int t, float r} | V |
| 001851a8 | SetSpawnDelay | mgr +0x88/+0x8c, +0x90 = delay/interval | V |
| 001868b0 | ScriptCB_SetNumBots | DAT_00518bc4 (MP AI cap, default 32) | V |
| 002ab7f8 | manager ctor (0xb0) | defaults 15.0/0.5; locals ∞ tickets | V |
| 002acf40 | team ctor (0x2a4) | member cap 40; 4 threshold slots (teams 1–2) | V |
| 002abb98 | level init | member pool Σunits×0xe8; fill AI members; MP tickets ÷ DAT_00518394 | V |
| 002ac378 | manager update (dt) | locals budget mgr+0x84; wave timers; team updates | V |
| 002ad830 | team update (dt) | bleed select+drain (÷4), no-CP 20 s wipe, exhaustion cull (−900/15 s) | V |
| 00295c40 | member update | AI ticket gate `tickets<=alive`, wave align, caps, flag-cull | V |
| 002966f0 / 00297440 | spawn / death | alive ++/−− ; **death: tickets−− (floor 0)** | V |
| 002ade48 / 002adf20 | add member / count alive | +0x28/+0x24/+0x30, +0xb4 | V |
| 0029d1d0 / 002a1138 | CP instance / class prop parsers | ValueBleed "%d" → +0x9c / +0xf0 (default 100) | V |
| 002b1320 | announcer/VO controller | bleeding VO (rate>0), low-reinf VO (≤ 0.1×initial) | V |
| 0033f3c8 | HUD ticket readout | draws team1/2 +0x10 | V |

---

## 4. Port guidance (openbf = BF2 rules + BF1-modeled invariant)

- Ticket consumption: **on death**, floor 0 (both games agree; implement engine-side
  since openbf runs modes natively).
- No-overspawn: block AI (re)spawn while `alive >= tickets` — this exact predicate is in
  both engines. BF1's extra behavior when players occupy the last slots is the
  exhaustion path: flagged units culled instantly, others culled by the anti-stalemate
  timer; the simple honest port is "AI cannot spawn, and if alive would exceed tickets
  (e.g. players joining), despawn excess AI" — which is what PLAN already locked.
- Bleed (BF2 baseline): per-CP per-faction bleed value (retail: 10, locals 0); enemy
  strict majority of total value → drain 1 ticket per 3 s (rate 1/3), clamped so tickets
  never drop below the current alive count, stop at 0; recompute on every
  capture/neutralize. Optional `AddBleedThreshold(team, thresh, rate)` table = rate of
  the highest threshold ≤ enemy value.
- Defeat: tickets ≤ 0 → team defeated immediately. Domination: sole owner of all alive
  CPs for 20 s wins.
- Spawn waves: delay 10 s / interval 0.25 s (mission data), death → next wave, respawn
  timer 4 s; MP forces delay 15 s (BF2).
- Caps: BF2 alive-AI cap 16 (options-adjustable), member pool = SetUnitCount (20 typ.),
  global 64.

## 5. Open questions

1. **BF1 exempt-class flag** (`*(class-ish+0x7c)` → member+0xd0; BF2: entity+0x330 bit
   33 → ctrl+0x165): which ODF property sets it (hero/AI-only marker?). Affects who is
   excluded from the alive counter and who gets insta-culled at exhaustion.
2. **BF1 bleed no-op**: confirm in PCSX2 (hold 1 of 5 CPs for ~2 min; counter should
   move only on deaths). If PC BF1 differs, the PS2 engine's first-match scan is the
   divergence point (0x2ada14).
3. BF1 final game-over dispatcher (who reacts to tickets==0 + none alive) — candidates
   around the mission-status object `DAT_005644a8` / shell state machine; the mechanics
   (no spawns, forced kills) are pinned regardless.
4. BF1 `FUN_00236e20` (`tickets < 26`) consumer (HUD blink?).
5. BF2 `ScriptCB_SetNumBots` runtime table 0x5fa808 (per-difficulty/option values) is
   BSS-filled from the profile — dump in PCSX2 to record the option mapping.
6. BF2 exact behavior of exempt (hero) deaths on tickets: Lua OnCharacterDeath does not
   check the flag, so a hero death should cost a ticket — worth one PCSX2 confirmation.
7. BF1 MP quirk: at level init tickets are divided by `DAT_00518394` when > 1 (some
   session count) — untraced, MP-only.

## 6. The conquest time limit (appended 2026-09-24 by queue TL, Opus loop lane 1, `[O]`)

A pointer; the record is **`analysis/re-time-limit.md`**. In short: stock PS2 BF2 SP conquest
sets **no** time limit — every `*_con` mission passes `multiplayerRules = true` and no
`timeLimit`, so `Objective:Start` takes `ScriptCB_GetCONMaxTimeLimit()` = `iCONTimer[rules] × 60`,
which is 0 = Off in the ELF image, and no `loseTimer` is created. The timer is an Instant Action
option (Off, 5..60 min), Off at every power-on; §1.3's `GameOptionsTimeLimitUp` is what ends the
round when a player sets one. A round that stalls at tickets = alive therefore has no timer exit
on the stock disc either. On the way: the `_con` reinforcement percentage of §1.2 is a player
option (10–500 %), and Death Star conquest (`dea1{c,g}_con`) takes a SECOND ticket from team 1
on each team-1 death (`OnCharacterDeathTeam(…, 1)` → `AddReinforcements(1, -1)`).
