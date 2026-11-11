//! The AI's randomness (Phase C design decision D3): every draw a bot makes
//! is a per-unit HASH of (unit index, tick, generation, salt), never the sim
//! LCG. The named salts; [`route_rand`], the route chooser's word, and
//! [`unit_rand`], its shape with a salt folded in, with the two draws built on
//! it ([`unit_unit`], [`unit_dir`]); and [`stance_hold`], the D13 draw of the
//! stance a unit takes at a hint — with the D13 stance tests, end to end.

use crate::unit::UnitHandle;
use glam::Vec3;

// Only for `stance_hold`'s docs, which link `AiState`.
#[cfg(doc)]
use super::state::AiState;

// ---------------------------------------------------------------------------
// Phase C determinism (design decision D3): every draw the AI makes is a
// per-unit HASH of (unit index, tick, generation, salt) — never the sim LCG.
// `think` takes `&Sim` and must not mutate shared state, and two draws made on
// the same tick by the same unit must not share a word, so every use site has
// its own named salt below. The WEAPON's own dispersion keeps the LCG (that
// draw is the PS2's own, in `Sim::step` phase 4).
// ---------------------------------------------------------------------------

/// Burst length at the end of a burst (D11).
pub const SALT_BURST: u32 = 0xB0_0057;
/// Charge-weapon release point (D11).
pub const SALT_CHARGE: u32 = 0xC4_A736;
/// A single-shot weapon's re-arm time and a sniper's cooldown (card STTAP).
pub const SALT_TAP: u32 = 0x7A_9517;
/// Dodge-on-hit coin (D11).
pub const SALT_DODGE: u32 = 0xD0_D6E5;
/// Direction of a BF1 round's tracked-position error offset (D10).
pub const SALT_ERR_DIR: u32 = 0xE7_7D18;
/// The walk of the axis a unit's aim-error direction turns about (card
/// CLSE2, `unit::AimBlock::think`).
pub const SALT_ERR_AXIS: u32 = 0xA8_15E7;
/// The grenade REACTION's coin and delay (card HOLD, `Sim::hazards`).
pub const SALT_GRENADE: u32 = 0x67_E14D;
/// Card UTALK: the coins of the AI's own squad talk (a trooper's "follow
/// me", a seated AI's need-gunner call). Sound only: no rule reads them.
pub const SALT_FOLLOW: u32 = 0xF0_11A0;
pub const SALT_NEED_GUNNER: u32 = 0x6E_ED60;
/// Card GRAI: the bot's own number 0..=100 (the PS2 controller's, drawn once:
/// its grenade window) and the coin between two weapons of one score.
pub const SALT_NUMBER: u32 = 0x9E_4A0B;
pub const SALT_ARM: u32 = 0xA2_3E71;
/// Card HOLD: the dwell and the spot of a hold.
pub const SALT_HOLD: u32 = 0x40_1D00;
/// Card REACT: what a bot on a position does about a hit, and where to.
pub const SALT_REACT: u32 = 0x2E_AC70;
/// Card FGHT: a bot's fixed fighting style (stand or circle; `^ 1` its
/// crouch), the circle's ring points and stops, the stander's own spot.
pub const SALT_STYLE: u32 = 0x57_71E5;
pub const SALT_CIRCLE: u32 = 0xC1_2C1E;
/// Which of a hint's allowed stances this unit actually takes ([`stance_hold`]).
pub const SALT_STANCE: u32 = 0x57_A9CE;
/// A follower's standing point in its leader's frame
/// ([`crate::squad::stand_offset`]).
pub const SALT_FORMATION: u32 = 0xF0_6A11;
/// The point of a post's capture region a bot walks to
/// ([`super::route::post_point`], card PARR): one try per added index.
pub const SALT_POST_POINT: u32 = 0x9A_22B1;

/// A leg's point of the next hub ([`super::route::hub_point`], card HUBP):
/// three per try.
pub const SALT_HUB_POINT: u32 = 0x4B_9017;
/// Card TPOS: a bot's own bias per position (`^ 1` its cycle's phases, `^ 2`
/// the wait for the next position, `^ 3` the move on a hit).
pub const SALT_TPOS: u32 = 0x7B_05C0;
/// Card DROIDAI: a hidden droid carrier's launch draw, one per
/// `remote::BOT_ROLL` window.
pub const SALT_DROID: u32 = 0xD8_01D5;

/// [`route_rand`]'s FNV shape with a per-use-site salt folded in.
pub fn unit_rand(unit_index: u32, tick: u64, gen: u32, salt: u32) -> u32 {
    let mut h: u32 = 0x811C_9DC5 ^ gen.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    for b in unit_index
        .to_le_bytes()
        .iter()
        .chain(tick.to_le_bytes().iter())
        .chain(salt.to_le_bytes().iter())
    {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// [`unit_rand`] mapped to [0, 1).
pub fn unit_unit(unit_index: u32, tick: u64, gen: u32, salt: u32) -> f32 {
    // 24 bits: exactly representable in f32, so the map is uniform.
    (unit_rand(unit_index, tick, gen, salt) >> 8) as f32 / 16_777_216.0
}

/// [`unit_rand`] mapped to a UNIT VECTOR (three draws, normalised;
/// [`Vec3::X`] when the draw is degenerate). The PS2 draws its error vector
/// the same way — ±0.5 per axis, re-drawn each think (RE §4.3).
pub fn unit_dir(unit_index: u32, tick: u64, gen: u32, salt: u32) -> Vec3 {
    let ax = |k: u32| unit_unit(unit_index, tick, gen, salt.wrapping_add(k)) - 0.5;
    let v = Vec3::new(ax(0), ax(0x51ED_2701), ax(0xA3DA_4E03));
    let len = v.length();
    if len < 1e-6 {
        Vec3::X
    } else {
        v / len
    }
}

/// The chooser's random word: the PS2 pulls it from the game LCG, which would
/// make `think` a mutation of shared state. Ours is a per-unit hash of (unit
/// index, tick, generation) — same role, replay-exact, and `think` stays
/// `&Sim`.
pub fn route_rand(unit_index: u32, tick: u64, gen: u32) -> u32 {
    let mut h: u32 = 0x811C_9DC5 ^ gen.wrapping_mul(0x9E37_79B9);
    for b in unit_index
        .to_le_bytes()
        .iter()
        .chain(tick.to_le_bytes().iter())
    {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The stance CODE a unit holds at a hint whose allowed-stance mask is `mask`
/// (0 = no hint, no wish), given the code it already holds.
///
/// `PrimaryStance` is a BITMASK and the PS2 draws ONE of its set bits UNIFORMLY
/// when the unit arrives at the hint, then keeps that stance for the whole hold
/// (RE lead-verified 2026-09-08 evening, `analysis/RE_LOG.md` +
/// `engine/artifacts/lead_tmp/s15/re/primary_stance.md` §1 "the picker",
/// §"When the AI adopts the hint's stance": the pick is made in the behaviour's
/// state-entry function — BF2 Snipe `FUN_002d7d60` 262206, Cover `FUN_002d06e8`
/// 257478 — and pushed unchanged every tick after that; there is no per-tick
/// re-roll anywhere). So the draw must NOT be re-made every think: the held
/// code is KEPT whenever the mask still allows it, and a bit is drawn only when
/// the unit has no pick yet, or its pick is not on offer where it is now.
///
/// The draw is `unit_unit` over the SET bits, which is the PS2's rejection
/// sampler (`do r = rand(0,2); while (((mask >> r) & 1) == 0)`, with `rand`
/// uniform and inclusive) with the rejected draws removed — same distribution,
/// and deterministic per (unit, tick, generation) like every other AI draw.
///
/// DEVIATION, stated: two hints with the same mask are ONE hold — a unit that
/// walks from a stand-or-crouch hint to another does not re-draw. The port
/// re-asks for the defence spot every think and has no "arrived at THIS hint"
/// edge to hang the draw on; the alternative is a per-hint identity in
/// [`AiState`], which costs a hash-format change for a stance that would not
/// look any different.
pub fn stance_hold(held: u8, mask: u8, me: UnitHandle, tick: u64) -> u8 {
    let mask = mask
        & (crate::commander::MASK_STAND
            | crate::commander::MASK_CROUCH
            | crate::commander::MASK_PRONE);
    if mask == 0 {
        return 0;
    }
    if held != 0 && crate::commander::stance_bit(held) & mask != 0 {
        return held;
    }
    let n = mask.count_ones();
    // `unit_unit` is in [0, 1), so `k < n` already; the min is belt and braces
    // against a rounding surprise at the top of the range.
    let k = ((unit_unit(me.index, tick, me.gen, SALT_STANCE) * n as f32) as u32).min(n - 1);
    let mut seen = 0;
    for bit in 0..3 {
        if mask >> bit & 1 == 1 {
            if seen == k {
                // Code = bit index + 1 (`commander::stance_bit`'s inverse).
                return bit as u8 + 1;
            }
            seen += 1;
        }
    }
    // Unreachable: `mask` is non-zero in bits 0..2, so the loop returns.
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::l_sim;
    use crate::commander::{MASK_CROUCH, MASK_PRONE, MASK_STAND};
    use crate::sim::{PadState, Sim};
    use crate::soldier::Stance;

    #[test]
    fn route_rand_is_per_unit_and_per_tick() {
        assert_eq!(route_rand(3, 100, 1), route_rand(3, 100, 1));
        assert_ne!(route_rand(3, 100, 1), route_rand(4, 100, 1));
        assert_ne!(route_rand(3, 100, 1), route_rand(3, 101, 1));
        assert_ne!(route_rand(3, 100, 1), route_rand(3, 100, 2));
    }

    /// D13's stance controller, end to end: a defender standing on a Cover
    /// hint whose allowed-stance mask is PRONE ALONE (a BF1 hint authoring
    /// `PrimaryStance 4`) is prone within 2 s, and is STANDING again within 2 s
    /// of being given somewhere to walk.
    ///
    /// The timings are the soldier's, not a tuning choice
    /// (`Soldier::stance_tap_target`): a stand takes a tap after
    /// `STANCE_TAP_WAIT`, and the crouch it lands in goes prone on a second
    /// tap inside `PRONE_DOUBLE_TAP`, so going down is two taps two ticks
    /// apart; coming up is one tap, after the prone's lock.
    #[test]
    fn a_cover_hint_puts_a_defender_prone_and_a_route_stands_it_up() {
        use crate::commander::Objective;
        let (mut sim, h) = l_sim();
        let spot = sim.cps[0].def.pos;
        // One authored Cover hint, on the post, allowing prone and nothing
        // else — `PrimaryStance 4` on a BF1 map, the only shape that can
        // (`commander::stance_mask`).
        sim.nav.as_mut().expect("graph").hints = vec![cover_hint(spot, MASK_PRONE)];
        // The unit is already at its post, DEFENDING it. The plan is pinned so
        // the commander cannot take the objective back mid-test.
        sim.units[h.index as usize].soldier.pos = spot;
        for t in 0..2 {
            sim.commander[t].next_plan_tick = u64::MAX;
        }
        sim.commander[0].assignment = vec![Some(Objective::Defend(0))];
        sim.commander[0].spots = vec![Some((spot, MASK_PRONE))];

        let stance = |s: &Sim| s.unit(h).expect("alive").soldier.stance;
        let mut prone_at = None;
        for t in 0..60u64 {
            sim.step(&[PadState::default()]);
            if prone_at.is_none() && stance(&sim) == Stance::Prone {
                prone_at = Some(t);
            }
        }
        assert_eq!(
            sim.unit(h).expect("alive").ai.want_stance,
            3,
            "the only stance the hint's mask allows is the wish"
        );
        let prone_at = prone_at.expect("never went prone inside 2 s");
        assert!(prone_at < 60, "prone at tick {prone_at}");
        assert_eq!(stance(&sim), Stance::Prone, "and it stays down");
        // And down it stays, for 5 s more: the wish is met, nothing presses.
        for _ in 0..150u64 {
            sim.step(&[PadState::default()]);
            assert_eq!(stance(&sim), Stance::Prone, "it got up on its own");
        }

        // Now give it somewhere to be: back at hub 0, 40 m from the post, with
        // the same Defend objective. It has to stand up to walk.
        sim.units[h.index as usize].soldier.pos = Vec3::ZERO;
        let mut stood_at = None;
        for t in 0..60u64 {
            sim.step(&[PadState::default()]);
            if stood_at.is_none() && stance(&sim) == Stance::Stand {
                stood_at = Some(t);
            }
        }
        let stood_at = stood_at.expect("never stood up inside 2 s of a route");
        assert!(stood_at < 60, "stood at tick {stood_at}");
        assert!(
            sim.unit(h).expect("alive").ai.route().is_some(),
            "it is walking back to its post"
        );
    }

    /// The overshoot, gone (2026-10-28): a defender CROUCHED on a crouch-only
    /// Cover hint that is given somewhere to walk stands up without ever
    /// passing through prone. Under the old Stand -> Crouch -> Prone cycle
    /// its one tap put it prone first (571 prone entries over 4 maps' rounds,
    /// none wanted). It crouches with one tap and never goes prone while it
    /// holds the crouch either.
    #[test]
    fn a_crouched_defender_given_a_route_stands_without_going_prone() {
        use crate::commander::Objective;
        let (mut sim, h) = l_sim();
        let spot = sim.cps[0].def.pos;
        sim.nav.as_mut().expect("graph").hints = vec![cover_hint(spot, MASK_CROUCH)];
        sim.units[h.index as usize].soldier.pos = spot;
        for t in 0..2 {
            sim.commander[t].next_plan_tick = u64::MAX;
        }
        sim.commander[0].assignment = vec![Some(Objective::Defend(0))];
        sim.commander[0].spots = vec![Some((spot, MASK_CROUCH))];

        let stance = |s: &Sim| s.unit(h).expect("alive").soldier.stance;
        let mut crouched_at = None;
        for t in 0..90u64 {
            sim.step(&[PadState::default()]);
            assert_ne!(
                stance(&sim),
                Stance::Prone,
                "prone on the way down, tick {t}"
            );
            if crouched_at.is_none() && stance(&sim) == Stance::Crouch {
                crouched_at = Some(t);
            }
        }
        assert_eq!(sim.unit(h).expect("alive").ai.want_stance, 2);
        crouched_at.expect("never crouched inside 3 s");
        assert_eq!(stance(&sim), Stance::Crouch, "and it stays crouched");

        // A route: it stands, at once in the crouch's terms (JUMP, after the
        // rise wait), and is never prone on the way.
        sim.units[h.index as usize].soldier.pos = Vec3::ZERO;
        let mut stood_at = None;
        for t in 0..60u64 {
            sim.step(&[PadState::default()]);
            assert_ne!(stance(&sim), Stance::Prone, "prone on the way up, tick {t}");
            if stood_at.is_none() && stance(&sim) == Stance::Stand {
                stood_at = Some(t);
            }
        }
        let stood_at = stood_at.expect("never stood up inside 2 s of a route");
        assert!(stood_at < 60, "stood at tick {stood_at}");
    }

    /// A Cover hint at `pos` whose allowed-stance mask is `mask` — what
    /// `game::layout::nav_graph` would have built from the authored value.
    fn cover_hint(pos: Vec3, mask: u8) -> crate::nav::Hint {
        crate::nav::Hint {
            name: "cover".into(),
            kind: crate::commander::HINT_COVER,
            pos,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            radius: None,
            mode: None,
            stance_mask: mask,
            secondary_stance_mask: MASK_STAND,
            peek_sides: 0,
            command_post: None,
        }
    }

    /// The draw over a hint's allowed-stance mask: one SET bit, uniformly, and
    /// the same pick again for as long as the mask still allows it (the PS2
    /// draws on entering the hold and pushes that stance every tick after —
    /// RE `primary_stance.md` §1, §"When the AI adopts the hint's stance").
    #[test]
    fn stance_hold_draws_one_allowed_bit_and_keeps_it() {
        let me = |i: u32| UnitHandle { index: i, gen: 1 };
        // No hint, no wish — and a mask with nothing in bits 0..2 is the same.
        assert_eq!(stance_hold(0, 0, me(3), 100), 0);
        assert_eq!(stance_hold(2, 0, me(3), 100), 0);
        assert_eq!(stance_hold(0, 0xF8, me(3), 100), 0);
        // A single-bit mask is not a draw at all: it is that stance, for every
        // unit and every tick.
        for (mask, code) in [(MASK_STAND, 1), (MASK_CROUCH, 2), (MASK_PRONE, 3)] {
            for i in 0..8 {
                for t in [0u64, 1, 97, 100_000] {
                    assert_eq!(stance_hold(0, mask, me(i), t), code, "mask {mask}");
                }
            }
        }
        // A two-bit mask answers one of ITS bits, never the third.
        let m = MASK_STAND | MASK_CROUCH;
        for i in 0..200 {
            let c = stance_hold(0, m, me(i), 40);
            assert!(c == 1 || c == 2, "unit {i} drew {c} from stand-or-crouch");
        }
        // Seeded: the same (unit, tick, generation) draws the same bit, and the
        // draw is not the same for every unit.
        assert_eq!(stance_hold(0, m, me(5), 40), stance_hold(0, m, me(5), 40));
        let drew: Vec<u8> = (0..400).map(|i| stance_hold(0, m, me(i), 40)).collect();
        let stand = drew.iter().filter(|&&c| c == 1).count();
        // Uniform over the set bits: a fair coin over 400 draws is not going to
        // land outside 40-60 % (this is a fixed seed set, so it is a constant).
        assert!(
            (160..=240).contains(&stand),
            "{stand}/400 stand — not a fair draw over the two allowed bits"
        );
        // HELD: a pick the mask still allows survives every later think.
        for t in 0..500u64 {
            assert_eq!(stance_hold(1, m, me(9), t), 1, "stand held at tick {t}");
            assert_eq!(stance_hold(2, m, me(9), t), 2, "crouch held at tick {t}");
        }
        // ... and a pick the mask does NOT allow is re-drawn (the unit has
        // moved to a hint that does not offer it).
        for i in 0..64 {
            assert_eq!(stance_hold(3, MASK_CROUCH, me(i), 7), 2);
            let c = stance_hold(3, m, me(i), 7);
            assert!(c == 1 || c == 2, "prone was not on offer, drew {c}");
        }
    }

    /// End to end, the BF2 half of the RE: a hint authoring `PrimaryStance 3`
    /// (47.8 % of the takeable hints) is stand-or-crouch, so the defender NEVER
    /// goes prone, and whichever it drew it keeps for the whole hold.
    #[test]
    fn a_bf2_stand_or_crouch_hint_never_prones_and_holds_its_draw() {
        use crate::commander::{stance_mask, Objective};
        let (mut sim, h) = l_sim();
        let spot = sim.cps[0].def.pos;
        // The mask comes from the RE's rule, not from a literal: value 3 on a
        // BF2 map. Value 4 there is the ctor default, stand alone.
        let m3 = stance_mask(Some(3), "bf2");
        assert_eq!(m3, MASK_STAND | MASK_CROUCH);
        assert_eq!(stance_mask(Some(4), "bf2"), MASK_STAND);
        sim.nav.as_mut().expect("graph").hints = vec![cover_hint(spot, m3)];
        sim.units[h.index as usize].soldier.pos = spot;
        for t in 0..2 {
            sim.commander[t].next_plan_tick = u64::MAX;
        }
        sim.commander[0].assignment = vec![Some(Objective::Defend(0))];
        sim.commander[0].spots = vec![Some((spot, m3))];

        let mut wish = None;
        for t in 0..600u64 {
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).expect("alive");
            assert_ne!(u.soldier.stance, Stance::Prone, "prone at tick {t} on BF2");
            assert_ne!(u.ai.want_stance, 3, "wished prone at tick {t} on BF2");
            if u.ai.want_stance != 0 {
                // Drawn once and held: it never becomes the other allowed bit.
                let w = *wish.get_or_insert(u.ai.want_stance);
                assert_eq!(u.ai.want_stance, w, "the draw moved at tick {t}");
            }
        }
        let wish = wish.expect("the hint asks for a stance");
        assert!(wish == 1 || wish == 2, "drew {wish}");
        // ... and the soldier ends up in the stance it drew.
        assert_eq!(
            sim.unit(h).expect("alive").soldier.stance,
            crate::commander::stance_of_code(wish).expect("a stance"),
        );
        // The same map on BF1 CAN prone, and only from 4-7 (the same hint
        // authoring 4): the two games differ here, and this is the difference.
        assert_eq!(stance_mask(Some(4), "bf1"), MASK_PRONE);
    }
}
