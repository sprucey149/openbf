//! Foot-bot jet requests. BF2 FUN_002d9890 / FUN_002d99c0 schedule
//! 6–12 s hops and 3–8 s jump hints; FUN_002dc0b8 gates hover attacks by
//! 20 m and fuel for the trip. See analysis/re-jetpack.md §3.1 and §10.
use super::{
    behave::Tick,
    draw::{unit_rand, unit_unit},
    mover::MoveRequest,
    state::AiState,
};
use crate::{
    collision::{HullFilter, Layers},
    fmath, nav, perception,
    sim::{buttons, PadState, Sim},
    soldier::{BotJet, Stance},
    unit::UnitHandle,
};
use glam::{Vec3, Vec3Swizzles};

const HOP_SALT: u32 = 0x4a4554;
const HINT_SALT: u32 = 0x4a484e;
const SPOT_SALT: u32 = 0x4a5350;
const HOP_TRIES: u32 = 10;
const HOP_SPREAD: f32 = 40.0;
const HOP_MIN: f32 = 20.0;
const HOP_SKY: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JetState {
    pub next_hop: u64,
    pub next_hint: u64,
    pub flight: Option<Flight>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    pub goal: Vec3,
    pub kick: Vec3,
    pub height: Option<f32>,
    /// A jet-only route must clear its destination platform before jet-off.
    pub route: bool,
    pub hop: bool,
    pub since: u64,
    pub launched: bool,
    pub stop: bool,
}

impl JetState {
    /// The brain can end a tick before reaching the mover (idle/zero aim).
    /// Stop an active jet on its next update; discard an unlaunched request.
    pub(crate) fn cancel(&mut self, stance: Stance) {
        if stance.jetting() {
            if let Some(flight) = self.flight.as_mut() {
                flight.stop = true;
            }
        } else {
            self.flight = None;
        }
    }
    pub fn command(&self) -> Option<BotJet> {
        self.flight.map(|f| BotJet {
            kick: f.kick,
            height: f.height,
            stop: f.stop,
        })
    }
    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if *self == Self::default() {
            return;
        }
        eat(&[0x4a]);
        eat(&self.next_hop.to_le_bytes());
        eat(&self.next_hint.to_le_bytes());
        eat(&[u8::from(self.flight.is_some())]);
        if let Some(f) = self.flight {
            for v in [f.goal.x, f.goal.y, f.goal.z, f.kick.x, f.kick.y, f.kick.z] {
                eat(&v.to_bits().to_le_bytes());
            }
            eat(&[
                u8::from(f.height.is_some()),
                u8::from(f.route),
                u8::from(f.hop),
                u8::from(f.launched),
                u8::from(f.stop),
            ]);
            if let Some(h) = f.height {
                eat(&h.to_bits().to_le_bytes());
            }
            eat(&f.since.to_le_bytes());
        }
    }
}

fn hop_spot(sim: &Sim, tk: &Tick) -> Option<Vec3> {
    let me = tk.me;
    let pos = tk.u.soldier.pos;
    let radius = tk.u.consts.aim_size * 0.5;
    (0..HOP_TRIES).find_map(|k| {
        let draw = |axis: u32| {
            (unit_unit(me.index, sim.tick, me.gen, SPOT_SALT + k * 3 + axis) - 0.5)
                * 2.0
                * HOP_SPREAD
        };
        let (x, z) = (draw(0), draw(2));
        if x * x + z * z < HOP_MIN * HOP_MIN {
            return None;
        }
        let spot = pos + Vec3::new(x, 0.0, z);
        let barred = sim.nav.as_ref().is_some_and(|g| {
            g.barriers.iter().any(|b| {
                b.enabled
                    && b.stops(nav::SOLDIER)
                    && b.contains_r(glam::Vec2::new(spot.x, spot.z), radius)
            })
        });
        let line_blocked = !sim.mission_ai.allow_blind_jet_jumps
            && sim.nav.as_ref().is_some_and(|g| {
                g.barriers.iter().any(|b| {
                    b.enabled && b.stops(nav::SOLDIER) && b.crosses_r(pos.xz(), spot.xz(), radius)
                })
            });
        (!barred && !line_blocked && hop_sky_clear(sim, me, spot)).then_some(spot)
    })
}

fn hop_sky_clear(sim: &Sim, me: UnitHandle, spot: Vec3) -> bool {
    // BF2 FUN_002dbb68 B2:265074 / BF1 FUN_0025aa08 B1:177125
    // include soldiers and vehicles as well as buildings and terrain, and
    // exclude the requesting soldier. Keep this separate from ordinary sight.
    // OURS: static bodies use both port subsets conservatively; trace: retail
    // object-layer versus body-subset selection (re-jetpack §17).
    perception::los_terrain(sim, spot, spot + Vec3::Y * HOP_SKY)
        && !sim.collision.ray_blocked(
            spot,
            Vec3::Y,
            HOP_SKY,
            Layers::ALL,
            HullFilter::All,
            [None; 2],
        )
        && !sim.alive_units().any(|(h, u)| {
            h != me && u.mount.is_none() && u.bolt_hit(spot, Vec3::Y, HOP_SKY).is_some()
        })
}

fn after(me: UnitHandle, tick: u64, low: u32, high: u32, salt: u32) -> u64 {
    tick + u64::from(low + unit_rand(me.index, tick, me.gen, salt) % (high - low + 1))
        * crate::TICK_HZ as u64
}

pub(super) fn run(
    sim: &Sim,
    tk: &Tick,
    st: &mut AiState,
    mv: &MoveRequest,
    pad: &mut PadState,
) -> bool {
    let u = tk.u;
    let c = &u.consts;
    let s = &u.soldier;
    if !c.has_jet() {
        st.jet = JetState::default();
        return false;
    }
    // Service and boarding must not launch a bot away from its work. Lost
    // recovery keeps its existing ground policy instead of inventing an air route.
    let blocked = mv.boarding
        || mv.throwing
        || mv.ret.is_some()
        || sim.controlled_droid(tk.me).is_some()
        || super::fix::serve(sim, tk.me, st).is_some();
    if let Some(mut f) = st.jet.flight {
        f.launched |= matches!(s.stance, Stance::Jet | Stance::Hover);
        if (f.launched && !s.stance.airborne())
            || s.stance == Stance::Fall
            || (!f.launched && sim.tick > f.since + crate::TICK_HZ as u64)
        {
            st.jet.flight = None;
        } else {
            let arrived = if f.route {
                nav::xz_dist(s.pos, f.goal) < super::CORNER_ARRIVE && s.pos.y >= f.goal.y
            } else {
                nav::xz_dist(s.pos, f.goal) < 3.0 && (f.height.is_some() || f.hop)
            };
            f.stop |= blocked || (f.launched && arrived);
            st.jet.flight = Some(f);
        }
    }
    if st.jet.next_hop == 0 {
        st.jet.next_hop = after(tk.me, sim.tick, 6, 12, HOP_SALT);
    }
    if st.jet.next_hint == 0 {
        st.jet.next_hint = after(tk.me, sim.tick, 3, 8, HINT_SALT);
    }
    if st.jet.flight.is_none()
        && !blocked
        && s.grounded
        && s.lock_timer <= 0.0
        && s.fuel > c.jet_fuel_min_border
    {
        let jet_leg = st.route.and_then(|r| {
            let graph = sim.nav.as_ref()?;
            let arc = graph.arcs.get(r.via_arc? as usize)?;
            if arc.flags & nav::Arc::JET_JUMP == 0 {
                return None;
            }
            // Some authored endpoint hubs have y=0 (Mustafar Hub41/44).
            // Probe below the higher endpoint, retaining the waypoint when
            // no surface exists. This is the port's surface projection.
            let probe = graph.hubs[arc.a as usize]
                .pos
                .y
                .max(graph.hubs[arc.b as usize].pos.y)
                .max(s.pos.y);
            let mut goal = r.waypoint;
            let floor = sim.ground_under(Vec3::new(goal.x, probe, goal.z), false);
            if floor > -300.0 {
                goal.y = floor;
            }
            Some(goal)
        });
        let attack = st
            .target
            .and_then(|t| perception::contact_of(st, t))
            .map(|t| t.last_pos)
            .filter(|p| {
                let distance = p.distance(s.pos);
                c.jet_hover
                    && distance >= 20.0
                    && c.jet_fuel_cost > 0.0
                    && c.max_speed > 0.0
                    && distance / c.max_speed <= (s.fuel - c.jet_fuel_initial) / c.jet_fuel_cost
            });
        let hint = if !c.jet_hover && sim.tick >= st.jet.next_hint {
            st.jet.next_hint = after(tk.me, sim.tick, 3, 8, HINT_SALT);
            // Authored radius is the hint's activation range. No radius means
            // the bot must be at the node (ours: ordinary route arrival slack).
            sim.nav.as_ref().and_then(|g| {
                g.hints
                    .iter()
                    .find(|h| {
                        h.kind == 4
                            && nav::xz_dist(s.pos, h.pos)
                                <= h.radius.unwrap_or(super::CORNER_ARRIVE)
                    })
                    .map(|h| h.pos)
            })
        } else {
            None
        };
        let hop_due = st.target.is_some() && sim.tick >= st.jet.next_hop;
        let hop = hop_due.then(|| hop_spot(sim, tk)).flatten();
        let launch = jet_leg.is_some() || attack.is_some() || hint.is_some() || hop.is_some();
        if launch || hop_due {
            st.jet.next_hop = after(tk.me, sim.tick, 6, 12, HOP_SALT);
        }
        if launch {
            let goal = jet_leg.or(hint).or(attack).or(hop).unwrap_or(s.pos);
            let directed = jet_leg.is_some() || attack.is_some();
            let kick = if directed {
                ((goal - s.pos).normalize_or_zero() + Vec3::Y) * std::f32::consts::FRAC_1_SQRT_2
            } else {
                Vec3::ZERO
            };
            // Hover attack's wanted height is target +10 (FUN_002dc448).
            let height = if attack.is_some() && jet_leg.is_none() {
                Some(goal.y + 10.0)
            } else if jet_leg.is_some() {
                Some(goal.y)
            } else {
                None
            };
            st.jet.flight = Some(Flight {
                goal,
                kick,
                height,
                route: jet_leg.is_some(),
                hop: jet_leg.or(hint).or(attack).is_none(),
                since: sim.tick,
                launched: false,
                stop: false,
            });
        }
    }
    let Some(f) = st.jet.flight else {
        return false;
    };
    if !f.launched && s.grounded && s.prev_buttons() & buttons::JUMP == 0 {
        pad.buttons |= buttons::JUMP;
    }
    // In the air the ground avoider's ledge veto must not cancel a jet link.
    // The regular collision pass still resolves walls and ceilings.
    let to = Vec3::new(f.goal.x - s.pos.x, 0.0, f.goal.z - s.pos.z).normalize_or_zero();
    let (sy, cy) = fmath::sin_cos(s.yaw);
    pad.ly = PadState::quantize(to.dot(Vec3::new(-sy, 0.0, -cy)));
    pad.lx = PadState::quantize(to.dot(Vec3::new(cy, 0.0, -sy)));
    pad.buttons &= !buttons::ROLL_STANCE;
    true
}

#[cfg(test)]
mod tests;
