//! Delivery of ordered vehicle calls and new autonomous claims.

use crate::ai::{ride, route, AiState, NavTick};
use crate::sim::Sim;
use crate::unit::{Unit, UnitHandle};

pub(in crate::ai) fn think(
    sim: &Sim,
    me: UnitHandle,
    u: &Unit,
    st: &mut AiState,
    nt: &mut NavTick,
) {
    if nt.released != 0 && (st.pending_board.is_some() || st.ride.is_some_and(|r| r.ordered_call)) {
        st.pending_board = None;
        st.cmd_clear();
    } else {
        ride::deliver_call(sim, me, u, st);
    }
    if st
        .ride
        .is_some_and(|r| r.ordered_call || r.player_board.is_some())
    {
        if let Some(at) = ride::board_point(sim, st) {
            if st.ride.is_some_and(|r| r.since == sim.tick) {
                route::clear(st);
                nt.claim = NavTick::CLAIM_NEW;
            }
            route::retarget_board(sim, me, st, at, nt);
        }
    }
    // Card VAI: a bot on foot in the commander's pool may go for a
    // vehicle (`ride::claim`); an ordered one stays with its order.
    if u.mount.is_none() && u.command.is_none() {
        let had = ride::board_point(sim, st).is_some();
        ride::claim(sim, me, u, st);
        // Card WAI2: a new claim routes to its vehicle at once - card
        // TURR: one the offer pass made this tick too.
        let fresh = st.ride.is_some_and(|r| r.since == sim.tick);
        if let Some(at) = ride::board_point(sim, st).filter(|_| !had || fresh) {
            nt.claim = NavTick::CLAIM_NEW;
            route::retarget_board(sim, me, st, at, nt);
        }
    }
}
