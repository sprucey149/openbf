# AI class selection in both original games

AICLS / issue #9, 2026-10-29. **[O] UNAUDITED**: static primary evidence
was independently checked; the original timed PS2 live-AI census remains open.
This is research only. The port's selected 4:2:1 policy stays unchanged.

[V] marks an inspected instruction or source path; [D] marks its consequence;
[H] marks an unmeasured explanation. Original executable references below are
to the BF1/BF2 PS2 corpora, abbreviated B1/B2. Port observations are marked
[ours]. The conclusions concern ordinary infantry; flagged appended classes
have additional branches described below.

## BF1: fill deficits in assigned class quotas

[V] `AddUnitClass` (`FUN_00184e58`, B1:27129–27150) passes its count through
`FUN_002ae088` to `FUN_002ae030`. The team holds class pointers at +0x40,
class count at +0x38 and quotas at +0x50. `FUN_002ad490` sums the quotas and
increments them until their sum reaches team member count +0x28. With
`m = class_count - appended_class_count`, each increment selects
`max(0, unsigned_random % (4*m) - 3*m)`. This strongly favors index zero.
The initialization calls vtable slot +0x10; its ELF word is `0x002ad490`.

[V] `FUN_002ae260` counts every member with a class pointer at member +0xac,
using its selected index +0xa8. For each class it computes
`deficit = max(quota - assigned_count, 0)`. A positive total produces a
cumulative-deficit draw using an unsigned remainder. A zero total returns the
requesting member's existing class index. A member whose assigned class has
class +0x7c set instead takes an early previous-index return.

[V] The ordinary AI respawn helper `FUN_002976a0` calls this picker, then
`FUN_002970b8` stores its result. The member's class assignment participates in
the selector's count; entity pointer +0xb4 is a separate field. [D] Once all
quotas are filled, a normal respawn therefore retains the member's class.
Positive deficits elsewhere can cause a different selection. This conditional
retention is distinct from unconditionally keeping a bot's class.

## BF2: shuffle, search below minimum, then below maximum

[V] `AddUnitClass` (`FUN_0018c978`) passes minimum/maximum through the class
registration/update paths to `FUN_0036fd98`: +0x50 holds minimum and +0x54
maximum. This corrects the reversed labels in `re-tickets-spawn.md` section
1.1. `re-spawn-flow.md` section 4.3 already has their order right.
A subsequent type-specific pool-capacity clamp can alter the
effective arrays, so authored limits alone are insufficient for a RAM census.

[V] `FUN_0036f198` counts class-assigned members at member +0x140, by class
index +0x13c. Define `reserve` as any member having a nonnegative AI/player
marker at +0x130, **or** the game clock being below 60.0 seconds. It shuffles
the class indices once, then searches that same order twice:

1. Ordinary class: accept `count < minimum`, additionally requiring
   `count < maximum - 1` while `reserve` is active.
2. Ordinary class: accept `count < maximum`, additionally requiring
   `count < maximum - 1` while `reserve` is active.

The first eligible class wins; exhausted searches return -1. With reserve
inactive, the first pass uses the minimum comparison even for unusual inputs
where minimum exceeds maximum. Flagged classes (bit 33 at class +0x330) have
a designated-member first-pass branch and a separate second-pass exclusion.
This note makes no claim about their full designation lifecycle.

[V] `FUN_0034cea8` reaches `FUN_0034ffe0` when the respawn timer expires and
readiness is clear. For an AI member, that helper calls the selector and stores
its result through `FUN_0034f1f8`. [D] A respawn can change the class. Randomized
priority among eligible classes differs from a draw proportional to deficits;
assigned-member counts differ from the port's living-AI counts.

## Primary checks

Run these from the repository root. Each `fn` result identifies the enclosing
function as well as the cited expression; the summaries after each command are
its relevant output. Raw words are little-endian executable words, not fitted
values. Both selector instruction ranges were also read with
`scripts/tools/eedis.py bf1 0x2ae260 0x2ae490` and
`scripts/tools/eedis.py bf2 0x36f198 0x36f508`.

```text
python3 scripts/re_check.py fn b1 238955 238497 238503 239099 239126 222195 221863
238955 FUN_002ae030: quota store through team +0x50
238497 FUN_002ad490: class count minus appended-class count
238503 FUN_002ad490: quota top-up random index
239099 FUN_002ae260: member +0xac class-pointer test
239126 FUN_002ae260: previous index from member +0xa8
222195 FUN_002976a0: negative AI/player marker
221863 FUN_002970b8: selected index store

python3 scripts/re_check.py callers b1 FUN_002ae260
2 occurrences: FUN_00296fd8 line 221820; FUN_002976a0 line 222196
python3 scripts/re_check.py callers b1 FUN_002976a0
4 occurrences: FUN_00295b60 line 220803; FUN_00295c40 line 220997;
FUN_00297a50 line 222446; FUN_00297be0 line 222528

python3 scripts/re_check.py word b1 0x2ae3e8 1 --u32
0x002ae3e8: 0x0069001b (divu)
python3 scripts/re_check.py word b1 0x53ef08 1 --u32
0x0053ef08: 0x002ad490 (initialization vtable slot)

python3 scripts/re_check.py fn b2 368219 368220 367867 367877 367916 367943 346293 344002
368219 FUN_0036fd98: minimum store through +0x50
368220 FUN_0036fd98: maximum store through +0x54
367867 FUN_0036f198: assigned count using member +0x13c
367877 FUN_0036f198: clock < 60.0
367916 FUN_0036f198: minimum comparison
367943 FUN_0036f198: reserve/max-1 comparison
346293 FUN_0034ffe0: selector call
344002 FUN_0034cea8: respawn helper call

python3 scripts/re_check.py callers b2 FUN_0036f198
2 occurrences: FUN_0034f100 line 345554; FUN_0034ffe0 line 346293
python3 scripts/re_check.py callers b2 FUN_0034ffe0
5 occurrences: FUN_0034cde0 line 343820; FUN_0034cea8 line 344002;
FUN_0034e1f0 line 345010; FUN_00350188 line 346441; FUN_00350350 line 346532

python3 scripts/re_check.py word b2 0x36f248 1 --u32
0x0036f248: 0x3c014270 (lui builds 0x42700000 = 60.0)
python3 scripts/re_check.py word b2 0x36f338 1 --u32
0x0036f338: 0x004a001b (divu)
```

Caller counts above describe the corpus reader's returned occurrences. They
are bounded observations, not proofs that the executable has only those callers.
The rule conclusions use the positively traced respawn paths. Original timed
class counts and game-wide negative claims are outside this evidence.

## Port full-round measurements

[ours] `Sim::ai_class_weights` in `core/src/sim/spawn/runtime.rs` uses
`max(cap * class_share / sum_shares - alive_AI_in_class, 0) + floor`,
with `floor = 0.05 * cap / class_count`; a class at its authored maximum gets
weight zero. Shares are 4 for index zero, 2 for standard indices 1–3 and 1
for later indices. `respawn_slot` draws again with the dead bot excluded from
the living count. The floor is a **probability-weight floor**, rather than a
hard guarantee that each class has one living soldier. Literal interpretation
of the issue's “at least one soldier” wording would require a separate decision.

[ours] Seed 42, default Normal, `round_to_outcome 60000 42 bf1_nab1 bf2_nab2`,
16 AI per side plus the ordinary slot-0 replacement (17 versus 16).
BF1 Naboo Plains ended at tick 11679 (389 s); BF2 Naboo ended at tick 11853
(395 s). These are the example's unit counts, rather than the missions' authored
counts. Both default missions field Empire versus Rebels.

Every row below gives class **spawn counts**, then percentages of its band's
spawns in parentheses. The opening wave is separate; later bands count
`Event::Respawn` using the resulting unit's class. “All” includes the opening.
Spawn shares reflect class lifetime as well as the selector: they are not live
composition or predicted probabilities. Class order is printed per team.

### bf1_nab1, team 0

Class order: `imp_inf_storm_trooper`, `imp_inf_shock_trooper`, `imp_inf_pilotatst`, `imp_inf_scout_trooper`, `imp_inf_dark_trooper`.

```text
opening      N= 17: 6 (35.29%), 3 (17.65%), 3 (17.65%), 3 (17.65%), 2 (11.76%)
0-120s       N= 26: 8 (30.77%), 3 (11.54%), 7 (26.92%), 4 (15.38%), 4 (15.38%)
120-300s     N= 53: 20 (37.74%), 10 (18.87%), 8 (15.09%), 10 (18.87%), 5 (9.43%)
300s-end     N= 26: 11 (42.31%), 3 (11.54%), 4 (15.38%), 5 (19.23%), 3 (11.54%)
all          N=122: 45 (36.89%), 19 (15.57%), 22 (18.03%), 22 (18.03%), 14 (11.48%)
```

### bf1_nab1, team 1

Class order: `all_inf_soldier`, `all_inf_vanguard`, `all_inf_pilot`, `all_inf_marksman`, `all_inf_smuggler`.

```text
opening      N= 16: 5 (31.25%), 4 (25.00%), 3 (18.75%), 2 (12.50%), 2 (12.50%)
0-120s       N= 34: 12 (35.29%), 5 (14.71%), 9 (26.47%), 3 (8.82%), 5 (14.71%)
120-300s     N= 59: 17 (28.81%), 9 (15.25%), 14 (23.73%), 12 (20.34%), 7 (11.86%)
300s-end     N= 11: 4 (36.36%), 4 (36.36%), 1 (9.09%), 2 (18.18%), 0 (0.00%)
all          N=120: 38 (31.67%), 22 (18.33%), 27 (22.50%), 19 (15.83%), 14 (11.67%)
```

### bf2_nab2, team 0

Class order: `imp_inf_rifleman`, `imp_inf_rocketeer`, `imp_inf_engineer`, `imp_inf_sniper`, `imp_inf_officer`, `imp_inf_dark_trooper`.

```text
opening      N= 17: 5 (29.41%), 3 (17.65%), 3 (17.65%), 3 (17.65%), 1 (5.88%), 2 (11.76%)
0-120s       N= 30: 7 (23.33%), 4 (13.33%), 8 (26.67%), 5 (16.67%), 2 (6.67%), 4 (13.33%)
120-300s     N= 51: 14 (27.45%), 10 (19.61%), 7 (13.73%), 9 (17.65%), 6 (11.76%), 5 (9.80%)
300s-end     N= 30: 9 (30.00%), 8 (26.67%), 7 (23.33%), 2 (6.67%), 1 (3.33%), 3 (10.00%)
all          N=128: 35 (27.34%), 25 (19.53%), 25 (19.53%), 19 (14.84%), 10 (7.81%), 14 (10.94%)
```

### bf2_nab2, team 1

Class order: `all_inf_rifleman`, `all_inf_rocketeer`, `all_inf_engineer`, `all_inf_sniper`, `all_inf_officer`, `all_inf_wookiee`.

```text
opening      N= 16: 5 (31.25%), 3 (18.75%), 3 (18.75%), 2 (12.50%), 1 (6.25%), 2 (12.50%)
0-120s       N= 28: 7 (25.00%), 4 (14.29%), 6 (21.43%), 5 (17.86%), 3 (10.71%), 3 (10.71%)
120-300s     N= 52: 15 (28.85%), 8 (15.38%), 9 (17.31%), 10 (19.23%), 6 (11.54%), 4 (7.69%)
300s-end     N= 22: 6 (27.27%), 3 (13.64%), 5 (22.73%), 5 (22.73%), 1 (4.55%), 2 (9.09%)
all          N=118: 33 (27.97%), 18 (15.25%), 23 (19.49%), 22 (18.64%), 11 (9.32%), 11 (9.32%)
```

The instrument asserts 33 opening bots per map, unique `(slot,generation)`
spawn identities, and successive generations increasing by one for every
observed slot. The instrumented and pristine runners' **entire stdout matches
byte for byte**, including final hashes `3c71872af7f937bb` / `53930e830e88373b`.
Their shared footer is:

```text
sweep: finished 2/2  stuck_max median 180 (finished-only 180)  >10k 0 maps  sum 480
```

[ours] The first class has the largest whole-round spawn share in each team
(31.67–36.89% BF1; 27.34–27.97% BF2). BF1's special is 11.48–11.67%; the two
BF2 special classes together are 18.64–18.75%. Late-round bands do not show a
consistent specialist increase across the four teams. A single round per game
is a bounded observation, not evidence of a long-run drift rate.


## Judgment and proposed action

[D] There is a real rule difference: BF1 draws against assigned quotas and retains its current class when they
are filled; BF2 prioritizes minimums and caps using assigned members;
[ours] the port continuously redraws against living deficits and the selected
4:2:1 weights. The original rules and the port's selected rule have different
purposes, and exact class-count parity conflicts with the selected policy.

The measured rounds do not establish an unwanted specialist drift worth a
behavior change. **Proposed action: retain the existing selector.** Clarify the
probability-floor wording separately if a hard live minimum is intended. A
future proposed change should measure living class shares and class-specific
lifetimes over paired multi-seed full rounds first; replacing the selected
policy with PS2 assignment rules solely for parity would reverse the decision.

The original AICLS 0/120/300-second **PS2 live-AI** census and its within-one
class-count prediction remain uncompleted. This port-only census is a different
measurement. Archived PS2 class-name readings previously failed identity
validation; they are deliberately excluded from these findings. An original
live census still needs validated class identity, health/human exclusion,
assigned/waiting counts, effective arrays and consistent sampling clocks.


## Later port rule — 2026-10-30 erratum

The authored minimum/maximum census above describes the original data and the
port rules at the time of the investigation. The maintainer removed per-class
maximum restrictions on 2026-10-29. Current `Sim::ai_class_weights` retains a
positive floor for every class, including those above their target share;
`Sim::class_refusal` checks points, not class population. The team's total
alive-AI cap still applies. This is a deliberate port rule, not a revised
claim about the original game's allocator.
