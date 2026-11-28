//! `[ours]` The one table of the human players' names in a battle, by sim player slot (issue
//! #194, `SPLITSCREEN.md` 18.1 item 3). Every place that names a human reads it: each view's
//! message feed (kill, team-kill, suicide, death, hero and award lines; the stats pages and the
//! scoreboard through `EventFeed::unit_name`), the name tags over heads and the lost-controller
//! notice.
//!
//! Writers: a single-player round (the profile's name on slot 0), a split round (each local
//! slot by its Players screen seat's profile) and a LAN round (the session names). A set merges:
//! a slot not given keeps its name, so a player who left is still named on the stats pages.
//! The generation moves only when a name changes, so the feeds copy the table only then.
//!
//! Presentation only: the sim never reads a name.

use std::collections::BTreeMap;

/// The humans' names by player slot, with a generation that moves on every change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerNames {
    by_slot: BTreeMap<u8, String>,
    generation: u64,
}

impl PlayerNames {
    /// Name each `(slot, name)`; a slot not given keeps the name it had. True when a name
    /// changed (the generation moved).
    pub fn set<'a>(&mut self, names: impl IntoIterator<Item = (u8, &'a str)>) -> bool {
        let mut changed = false;
        for (slot, name) in names {
            if self.by_slot.get(&slot).map(String::as_str) != Some(name) {
                self.by_slot.insert(slot, name.to_owned());
                changed = true;
            }
        }
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// Forget every name, for a new battle. The generation still moves on, so a reader that
    /// copied the old table copies again.
    pub fn clear(&mut self) {
        self.by_slot.clear();
        self.generation += 1;
    }

    /// Slot `slot`'s name, if it was given one.
    pub fn get(&self, slot: u8) -> Option<&str> {
        self.by_slot.get(&slot).map(String::as_str)
    }

    /// Slot `slot`'s name, else the original's "Player N" (`Player %d`, slot + 1).
    pub fn name_or_default(&self, slot: u8) -> String {
        self.get(slot)
            .map_or_else(|| format!("Player {}", u32::from(slot) + 1), str::to_owned)
    }

    /// Every named slot, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (u8, &str)> {
        self.by_slot.iter().map(|(s, n)| (*s, n.as_str()))
    }

    /// Moves on every change ([`PlayerNames::set`], [`PlayerNames::clear`]).
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_merges_and_a_leaver_keeps_his_name() {
        let mut n = PlayerNames::default();
        assert!(n.set([(0, "Rex"), (1, "Kai")]));
        let g = n.generation();
        // Kai left the roster: his name stays.
        assert!(!n.set([(0, "Rex")]));
        assert_eq!(n.generation(), g, "nothing changed, the generation stays");
        assert_eq!(n.get(1), Some("Kai"));
        // A rename moves the generation.
        assert!(n.set([(1, "Kai2")]));
        assert_eq!(n.generation(), g + 1);
        assert_eq!(n.iter().collect::<Vec<_>>(), vec![(0, "Rex"), (1, "Kai2")]);
    }

    #[test]
    fn an_unnamed_slot_is_player_n() {
        let mut n = PlayerNames::default();
        assert_eq!(n.name_or_default(0), "Player 1");
        assert_eq!(n.name_or_default(3), "Player 4");
        n.set([(3, "Han")]);
        assert_eq!(n.name_or_default(3), "Han");
        assert_eq!(n.get(2), None);
    }

    #[test]
    fn clear_forgets_and_moves_the_generation() {
        let mut n = PlayerNames::default();
        n.set([(0, "Rex")]);
        let g = n.generation();
        n.clear();
        assert_eq!(n.get(0), None);
        assert!(n.generation() > g);
    }
}
