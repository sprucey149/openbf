//! Maintainer-selected Instant Action defaults (2026-10-29; issue #86 retune 2026-11-03).
//! Combatants include humans. Mission files retain the original disc values.
//! Tickets: Normal + Good proxy, seeds 1–3 on both sides; 6–9 minute medians.
pub use ui::population::Defaults;

/// Map/era pairs preserve the mission's side order, including asymmetric sides.
const fn entry(
    map: &'static str,
    cw: bool,
    combatants: [u32; 2],
    tickets: [u32; 2],
) -> (&'static str, bool, Defaults) {
    (
        map,
        cw,
        Defaults {
            combatants,
            tickets,
        },
    )
}
pub const TABLE: &[(&str, bool, Defaults)] = &[
    // OURS: 13.12 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    entry("bf1_bes1", false, [20, 20], [107, 107]),
    // OURS: 13.03 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    entry("bf1_bes1", true, [20, 20], [95, 95]),
    entry("bf1_bes2", false, [16, 16], [124, 124]),
    entry("bf1_bes2", true, [16, 16], [142, 142]),
    entry("bf1_end1", false, [24, 24], [258, 258]),
    entry("bf1_geo1", true, [27, 21], [237, 237]),
    entry("bf1_hot1", false, [32, 32], [232, 232]),
    // OURS: pace after the blast change. measure: repeat both proxy sides after sim changes.
    entry("bf1_kam1", true, [16, 16], [164, 181]),
    // OURS: 10.15 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    entry("bf1_kas1", false, [27, 21], [169, 169]),
    entry("bf1_kas1", true, [27, 21], [230, 230]),
    entry("bf1_kas2", false, [31, 25], [250, 250]),
    entry("bf1_kas2", true, [31, 25], [250, 250]),
    entry("bf1_nab1", false, [32, 32], [283, 283]),
    entry("bf1_nab1", true, [32, 32], [281, 281]),
    // OURS: 9.05 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    entry("bf1_nab2", false, [19, 21], [168, 168]),
    entry("bf1_nab2", true, [20, 20], [200, 200]),
    entry("bf1_rhn1", false, [20, 20], [213, 213]),
    // OURS: 9.04 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    entry("bf1_rhn1", true, [20, 20], [184, 184]),
    entry("bf1_rhn2", false, [16, 16], [200, 200]),
    entry("bf1_rhn2", true, [16, 16], [200, 200]),
    entry("bf1_tat2", false, [16, 16], [128, 128]),
    entry("bf1_tat2", true, [16, 16], [135, 135]),
    entry("bf1_yav1", false, [20, 20], [200, 200]),
    entry("bf1_yav1", true, [20, 20], [200, 200]),
    entry("bf1_yav2", false, [16, 16], [200, 200]),
    entry("bf1_yav2", true, [16, 16], [222, 222]),
    entry("bf2_cor1", false, [16, 16], [207, 207]),
    entry("bf2_cor1", true, [16, 16], [170, 170]),
    entry("bf2_dag1", false, [16, 16], [185, 185]),
    entry("bf2_dag1", true, [16, 16], [201, 201]),
    entry("bf2_dea1", false, [16, 16], [251, 251]),
    entry("bf2_dea1", true, [16, 16], [259, 259]),
    entry("bf2_end1", false, [24, 24], [171, 171]),
    entry("bf2_fel1", false, [16, 16], [245, 245]),
    entry("bf2_fel1", true, [16, 16], [217, 217]),
    entry("bf2_geo1", true, [28, 28], [250, 250]),
    entry("bf2_hot1", false, [32, 32], [150, 150]),
    entry("bf2_kam1", false, [24, 24], [150, 150]),
    entry("bf2_kam1", true, [24, 24], [150, 150]),
    entry("bf2_kas2", false, [24, 24], [198, 198]),
    entry("bf2_kas2", true, [24, 24], [170, 170]),
    entry("bf2_mus1", false, [16, 16], [184, 184]),
    entry("bf2_mus1", true, [16, 16], [227, 227]),
    entry("bf2_myg1", false, [20, 20], [231, 231]),
    entry("bf2_myg1", true, [20, 20], [246, 246]),
    entry("bf2_nab2", false, [20, 20], [203, 203]),
    entry("bf2_nab2", true, [20, 20], [217, 217]),
    // OURS: pace after the blast change. measure: repeat both proxy sides after sim changes.
    entry("bf2_pol1", false, [16, 16], [187, 187]),
    entry("bf2_pol1", true, [16, 16], [150, 150]),
    entry("bf2_tan1", false, [16, 16], [223, 223]),
    entry("bf2_tan1", true, [16, 16], [227, 227]),
    entry("bf2_tat3", false, [16, 16], [226, 226]),
    entry("bf2_tat3", true, [16, 16], [275, 275]),
    entry("bf2_uta1", false, [16, 16], [150, 150]),
    entry("bf2_uta1", true, [16, 16], [166, 166]),
    entry("bf2_yav1", false, [20, 20], [182, 182]),
    entry("bf2_yav1", true, [20, 20], [189, 189]),
    // OURS: 9.96 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    entry("tat1", false, [24, 24], [138, 138]),
    entry("tat1", true, [24, 24], [208, 208]),
    entry("tat2", false, [16, 16], [150, 150]),
    // OURS: 5.96 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    entry("tat2", true, [16, 16], [182, 182]),
];

pub fn for_map(map: &str, cw: bool) -> Option<Defaults> {
    TABLE
        .iter()
        .find(|(m, era, _)| *m == map && *era == cw)
        .or_else(|| TABLE.iter().find(|(m, _, _)| *m == map))
        .map(|(_, _, d)| *d)
}

impl crate::walk::WalkSim {
    /// Apply the same percentage to the opening roster and later replenishment.
    /// Grow only missing roster slots; existing dead bots keep their spawn waves.
    pub fn set_team_size_percent(&mut self, pct: u32) {
        let Some(defaults) = self.conquest_defaults else {
            return;
        };
        self.team_size_pct = pct.clamp(10, ui::population::MAX_TEAM_PCT);
        if self.sim.outcome.is_some() {
            return;
        }
        let counts = defaults
            .scaled(self.team_size_pct, self.reinforcement_pct)
            .combatants;
        self.sim.set_combatants(counts);
        let additions = self.roster_gaps();
        self.grow_roster(additions);
    }

    /// Bot slots each team is short of its limit (alive or waiting to respawn).
    fn roster_gaps(&self) -> [u32; 2] {
        [0, 1].map(|team| {
            let roster = self
                .sim
                .units
                .iter()
                .filter(|u| !u.is_player && u.team == team && (u.alive || u.respawn_at.is_some()))
                .count() as u32;
            self.sim.ai_limit(team).saturating_sub(roster)
        })
    }

    /// A human who changes side frees a slot on his old team: fill it with a
    /// bot (only on a side change, so spent reinforcements are never refilled).
    pub(crate) fn refill_after_side_change(&mut self) {
        let humans = [0u8, 1].map(|team| {
            self.sim
                .players
                .iter()
                .flatten()
                .filter(|h| {
                    self.sim
                        .units
                        .get(h.index as usize)
                        .is_some_and(|u| u.gen == h.gen && u.team == team)
                })
                .count() as u32
        });
        let before = self.human_teams.replace(humans);
        let Some(before) = before else { return };
        if self.conquest_defaults.is_none() || self.sim.outcome.is_some() {
            return;
        }
        let gaps = self.roster_gaps();
        let additions = [0, 1].map(|t| gaps[t].min(before[t].saturating_sub(humans[t])));
        if additions != [0, 0] {
            self.grow_roster(additions);
        }
    }
}

/// The Team size and Reinforcements help: averages over every map/era entry
/// at these percentages (no single map: Instant Action plays them all).
pub fn menu_note(team_pct: u32, ticket_pct: u32) -> [String; 2] {
    ui::population::notes(ui::population::summary(TABLE, team_pct, ticket_pct))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_cover_all_maps_and_keep_ratios_and_human_slot() {
        assert_eq!(TABLE.len(), 61);
        for (i, (map, cw, d)) in TABLE.iter().enumerate() {
            assert!(!TABLE[..i].iter().any(|(m, e, _)| m == map && e == cw));
            assert!(d.combatants.iter().all(|n| (16..=32).contains(n)));
            assert_eq!(d.scaled(100, 100), *d);
            for team in 0..2 {
                let ai = d.initial_ai(team);
                assert_eq!(ai[team as usize] + 1, d.combatants[team as usize]);
            }
        }
        let d = for_map("bf1_kas2", false).unwrap();
        assert_eq!(d.combatants, [31, 25]);
        assert_eq!(d.scaled(50, 100).combatants, [16, 13]);
        assert_eq!(for_map("bf1_nab2", false).unwrap().combatants, [19, 21]);
        assert_eq!(for_map("bf1_nab2", true).unwrap().combatants, [20, 20]);
        let at = |t, r| ui::population::summary(TABLE, t, r);
        assert_eq!(at(100, 100).team, 20.0);
        // More team size means more units and shorter matches; more
        // reinforcements, more tickets and longer matches; 200% caps every map.
        assert!(at(50, 100).team < at(100, 100).team && at(100, 100).team < at(200, 100).team);
        assert_eq!(at(500, 100), at(200, 100));
        assert!(at(50, 100).minutes > at(100, 100).minutes);
        assert!(at(100, 100).minutes > at(200, 100).minutes);
        assert!(at(100, 50).tickets < at(100, 100).tickets);
        assert!(at(100, 50).minutes < at(100, 100).minutes);
        assert!(at(100, 200).minutes > at(100, 100).minutes);
        let [team, reinf] = menu_note(100, 100);
        assert!(team.starts_with("Average team size: 20 units\nAverage match length: "));
        assert!(reinf.starts_with("Average reinforcements: "));
        assert!(reinf.ends_with(team.lines().nth(1).unwrap()));
        assert_eq!(ui::population::tickets([25, 25], 10), [3, 3]);
    }
}
