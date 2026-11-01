use super::*;
use crate::vehicle::Mount;

impl Sim {
    pub fn next_seat(&self, u: UnitHandle) -> Option<u8> {
        self.seat_in_direction(u, false)
    }

    fn seat_in_direction(&self, u: UnitHandle, reverse: bool) -> Option<u8> {
        let unit = self.unit(u)?;
        let m = unit.mount?;
        let v = self.vehicle(m.vehicle)?;
        let setup = self.seat_setup.get(v.class as usize)?.as_ref()?;
        let n = v.seats.len();
        let from = usize::from(m.seat);
        let passenger = |k: SeatKind| matches!(k, SeatKind::Passenger(_));
        (1..n)
            .map(|k| {
                if reverse {
                    (from + n - k) % n
                } else {
                    (from + k) % n
                }
            })
            .find(|&j| {
                setup.boardable.get(j).copied().unwrap_or(false)
                    && !(passenger(v.seats[from].kind) && passenger(v.seats[j].kind))
                    && v.seats[j].occupant.is_none_or(|h| {
                        matches!(unit.controller, Controller::Player { .. })
                            && self.unit(h).is_some_and(|other| {
                                other.alive
                                    && other.controller == Controller::Ai
                                    && other.team == unit.team
                            })
                    })
            })
            .map(|s| s as u8)
    }

    pub(in crate::sim) fn switch_pass(&mut self, unit_pads: &[PadState]) {
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.alive || u.mount.is_none() || !matches!(u.controller, Controller::Player { .. }) {
                continue;
            }
            let Some(p) = unit_pads.get(i) else { continue };
            if !p.pressed(buttons::SWITCH_WEAPON)
                || u.soldier.prev_buttons() & buttons::SWITCH_WEAPON != 0
            {
                continue;
            }
            let h = self.handle(i);
            let (Some(seat), Some(m)) = (
                self.seat_in_direction(h, p.pressed(buttons::SPAWN_CLASS_PREV)),
                self.units[i].mount,
            ) else {
                continue;
            };
            let displaced = self.vehicles[m.vehicle.index as usize].seats[seat as usize].occupant;
            if let Some(bot) = displaced {
                self.vehicles[m.vehicle.index as usize].seats[seat as usize].occupant = None;
                self.units[bot.index as usize].mount = None;
            }
            self.unseat_unit(h);
            let entered = self.seat_unit(h, m.vehicle, seat);
            debug_assert!(entered);
            if let Some(bot) = displaced {
                self.reseat_displaced_ai(bot, m.vehicle, m.seat);
            }
        }
    }

    fn reseat_displaced_ai(&mut self, bot: UnitHandle, vehicle: VehicleHandle, from: u8) {
        let v = &self.vehicles[vehicle.index as usize];
        let setup = self.seat_setup[v.class as usize].as_ref().unwrap();
        let seat = v
            .seats
            .iter()
            .enumerate()
            .find(|(i, s)| {
                s.occupant.is_none() && setup.boardable.get(*i).copied().unwrap_or(false)
            })
            .map_or(from, |(i, _)| i as u8);
        let kind = v.kind;
        let hover_ai = v.hover.ai;
        let pilot = v.seats[seat as usize].kind == SeatKind::Pilot;
        self.vehicles[vehicle.index as usize].seats[seat as usize].occupant = Some(bot);
        let unit = &mut self.units[bot.index as usize];
        unit.mount = Some(Mount { vehicle, seat });
        unit.ai.pending_board = None;
        unit.ai.charge_until = 0;
        let ride = unit.ai.ride.get_or_insert_default();
        ride.pickup = None;
        ride.sticks = None;
        ride.hover = None;
        ride.flyer = None;
        ride.man = None;
        ride.use_req = false;
        if pilot
            && (matches!(kind, VehicleKind::Hover | VehicleKind::Walker)
                || (kind == VehicleKind::Flyer && self.flyer_bots))
        {
            ride.begin_pickup(vehicle, self.tick, kind, hover_ai);
        }
    }
}
