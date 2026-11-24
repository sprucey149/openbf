//! The droid's chunk death (card DROIDDEATH, `analysis/re-droid-death.md`
//! [V]): a CIS soldier droid that dies bursts 6 times in 10 - its class's
//! `ExplosionName` at the body, the body out of the world at once, no corpse,
//! the item drop as on any death - and always when a blast's push marked it
//! earlier in its life. There is no damage or headshot threshold: the
//! overkill only scales the chunks' launch (§2).

use super::state::{Event, Sim};
use crate::soldier::Stance;
use crate::unit::{chunk_scale, ChunkDeath};

impl Sim {
    /// The explosion a [`Event::DroidBurst`] of `team`'s class `class`
    /// detonated (the game's effect and sound read its look).
    pub fn burst_explosion(&self, team: u8, class: u8) -> Option<&crate::weapon::ExplosionClass> {
        if team >= 2 {
            return self.native_chunk_death(team, class)?.explosion.as_ref();
        }
        let c = self
            .classes
            .get(usize::from(team))?
            .get(usize::from(class))?;
        c.chunk
            .as_ref()
            .and_then(|k| k.explosion.as_ref())
            .or_else(|| c.droideka.as_ref()?.explosion.as_ref())
    }

    /// The chunk death of the unit at `i`'s class, `None` for a class
    /// without one (no chunk geometry) or a sim without class tables. A
    /// native (team 2 and up) reads its own team's classes
    /// ([`Sim::set_native_chunk_deaths`]), never an army's: its class index
    /// is into the native table, and reading army 1's table at that index
    /// let Clone Wars Tuskens burst like the CIS droid class they lined up
    /// with.
    pub(super) fn chunk_death_of(&self, i: usize) -> Option<&ChunkDeath> {
        let u = &self.units[i];
        if u.team >= 2 {
            return self.native_chunk_death(u.team, u.spec.class);
        }
        self.classes
            .get(usize::from(u.team))?
            .get(usize::from(u.spec.class))?
            .chunk
            .as_ref()
    }

    /// The push's chunk roll (§1; B1 FUN_002009a0 step 2 B1:112603-112612,
    /// B2 FUN_00238b80): before the distance test, one draw U; `U <=
    /// ChunkFrequency` marks the unit and it is not pushed (true); else it
    /// is pushed and a dead one's health becomes -4.0. A class without a
    /// chunk death draws nothing and is pushed.
    pub(super) fn chunk_push_roll(&mut self, i: usize) -> bool {
        let Some(freq) = self.chunk_death_of(i).map(|c| c.frequency) else {
            return false;
        };
        if self.lcg.unit() <= freq {
            self.units[i].chunk_mark = true;
            return true;
        }
        if !self.units[i].alive {
            self.units[i].health.cur = PUSHED_DEAD_HEALTH;
        }
        false
    }

    /// The death block's chunk branch (§2; B1:116901-116962,
    /// B2:159606-159663): one draw U; `U <= ChunkFrequency` or the mark bursts
    /// the body - the class's explosion at its sphere centre
    /// ([`Event::DroidBurst`]), the body out of the world and hidden. Returns
    /// whether it burst; the caller's item drop follows either way.
    ///
    /// `[ours]`: the PS2 draws U at every soldier's death block, the port
    /// only for a class with a chunk death, so a battle without droids keeps
    /// its LCG stream.
    pub(super) fn chunk_death(&mut self, i: usize) -> bool {
        let Some(c) = self.chunk_death_of(i) else {
            return false;
        };
        let (freq, explosion) = (c.frequency, c.explosion.clone());
        let u = self.lcg.unit();
        if u > freq && !self.units[i].chunk_mark {
            return false;
        }
        let h = self.handle(i);
        let unit = &self.units[i];
        let (lift, _) = super::blast::soldier_blast_sphere(unit.soldier.stance);
        let pos = unit.soldier.pos + glam::Vec3::Y * lift;
        let (team, class, scale) = (unit.team, unit.spec.class, chunk_scale(unit.health.cur));
        // [H] the blast's owner block {S+0x380, team, class, 0}: no disc
        // droid explosion deals damage (Effect only), so none is credited.
        if let Some(e) = explosion {
            self.explode(&e, pos, None, Some(team));
        }
        let u = &mut self.units[i];
        u.body = false;
        u.burst = true;
        u.soldier.stance = Stance::Dead;
        u.soldier.time_in_stance = 0.0;
        self.events.push(Event::DroidBurst {
            unit: h,
            pos,
            team,
            class,
            scale,
        });
        true
    }
}

/// The health a pushed dead droid is left at (B1:112611, B2's twin: -4.0).
pub const PUSHED_DEAD_HEALTH: f32 = -4.0;

#[cfg(test)]
mod tests;
