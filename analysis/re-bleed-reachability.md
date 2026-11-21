# RE (2026-10-24): tick_bleed reachability — [O] UNAUDITED

Card BLEED; source baseline `2d38a467d6f6c8173586e2c31cf26b787c2c6a13`.
This investigation concerns openbf's current Rust control flow. `[V-Rust]` means
read in that source; `[D]` is derived; `[T]` is an executed test. Original-game
rules remain owned by `re-tickets-spawn.md` and `re-time-limit.md` §6.

## Decision

**[O] Keep the unconditional AI-shedding loop and its Sim consumer.** Ordinary
fixed-step command-post bleed preserves the ticket/live invariant, but a mission
extra death drain reaches the same loop with its bleed rate at zero. The public
`Battle` API also accepts directly assigned, unclamped bleed rates.

**Update (2026-11-19, #171):** openbf no longer runs the mission death drain
(analysis/plan/FEATURE_SET.md "Every death costs one ticket"), so the zero-bleed
path below no longer arises in play. The loop stays as a guard for directly assigned
bleed rates. The drain test is now
`sim::round::tests::a_death_through_step_costs_one_ticket_and_sheds_no_ai`.

## Source chain

- `[V-Rust]` `core/src/cp.rs:280–308`, `bf2_bleed_rate` / `apply_bleed`:
  the CP rate is zero or `BF2_BLEED_TICKETS_PER_SEC`; each update overwrites it,
  arming it only when `alive() < tickets()` and tickets are positive.
- `[V-Rust]` `core/src/battle.rs:26,43–59`: rate = 1/3 ticket/s; a new team
  starts with zero bleed; `tickets()` truncates the nonnegative float pool.
- `[V-Rust]` `core/src/skill.rs:68–83,124–180`: Normal is the default tier;
  Easy/Normal/Hard have handicap 1, Elite 1.25. `Sim::empty` defaults the option
  on (`sim/state.rs:870–880`); windowed setup reads the profile Boolean
  (`game/src/main/windowed.rs:964`).
- `[V-Rust]` `core/src/sim/round.rs:65–86,140–147`: handicap multiplication
  follows the CP overwrite, once per update, for a human team facing a team
  without a human. Then `tick_bleed` runs, and positive returned counts call
  `shed_ai`. `update_cps:118–120` returns early for an empty post list.
- `[V-Rust]` `core/src/lib.rs:40–43`, `sim/step.rs:111–113`: `Sim::step`
  uses a fixed 1/30-second timestep.
- `[V-Rust]` `core/src/battle.rs:111–131,137–162,170–181`: spawning admits
  units through the ticket gate. A death removes one live unit and one ticket,
  then applies the configured additional drains. The repair loop follows the
  bleed subtraction independently of whether the rate is positive.
- `[V-Rust]` `core/src/sim/step.rs:594–603,835–836`, `sim/combat.rs:309–329`:
  a death-region victim takes the normal death path before the same step's CP
  update. `sim/spawn/runtime.rs:538–558` consumes the shed count: marks live AI
  dead, clears its respawn timer, unseats it and emits `Event::Despawn`.
- `[V-Rust]` `game/src/mission.rs:333–340`, `main/battle.rs:215–233`:
  mission setup loads `death_drains` into `battle.death_drain`.

All `core/src` paths above are relative to `engine/crates/`.

## Why CP bleed preserves the invariant

`[D][O]` Let A be live units and T = floor(R) the whole tickets before an armed
bleed tick. Its guard requires A <= T − 1. The largest current drain is
(1/3) × 1.25 × (1/30) = 1/72 < 1 ticket, so floor(R − drain) >= T − 1 >= A.
At equality the next CP update sets the rate to zero. This bound applies to
finite gameplay pools, fixed sim ticks and the current tier table; arbitrary
public-field mutation, a larger rate/timestep or an existing shortfall needs
separate treatment. Direct `Battle::tick_bleed` callers bypass the CP guard;
`battle::tests::bleed_drains_and_sheds_ai` already exercises that API.

## Legal mission-drain witness

The new test `sim::round::tests::mission_death_drain_sheds_ai_through_step_without_cp_bleed`
admits a player and two AI on three tickets. Two distant, equally valued posts
are split between the teams. One death region contains exactly one AI. One real
`Sim::step` performs the death and the subsequent ticket repair:

| Same-team extra death drain | Tickets after death | Live before repair | Shed AI | Final player / AI |
|---|---:|---:|---:|---:|
| 0 (control) | 2 | 2 | 0 | 1 / 1 |
| 1 | 1 | 2 | 1 | 1 / 0 |

Both have zero CP bleed, exactly one `EnvDeath`, and an undecided outcome. The
extra-drain case requires exactly one `Despawn` for the surviving AI, its cleared
respawn timer, a surviving player and matching battle/unit counts. Shedding
spends no additional ticket. This is a legally admitted starting census; the
older low-level floor test's hand-written three-AI/one-ticket setup alone did
not establish that path.

The second test `conquest_bleed_stops_before_shedding_live_ai` runs 120 fixed
steps for each of 32 setups: both teams × four tiers × handicap off/on × human
off/on. Each starts with three legal units and four tickets, with an
enemy CP-value majority. After the first tick all three units remain live on
three whole tickets, the fractional pool remains above three, and subsequent
rates are zero.

## Verification boundary

`[T]` Both new regressions pass (six round tests total). Three mutations are
caught by the intended runtime assertions: positive-bleed-only repair, ignored
unit despawn, and removed CP alive guard. After restoring each source with fresh
mtimes, the clean suite passes 846 library and five integration tests; formatting
and workspace Clippy also pass. Production code matches the baseline after
removing comment-only lines before the test modules.

Independent source review supported the clamp bound, legal drain path and need
for both the counter repair and Sim despawn consumer. These are current-runtime
findings; this session does not revalidate original-game parity or the content
of generated missions. The empty-CP early return remains outside the witness.
