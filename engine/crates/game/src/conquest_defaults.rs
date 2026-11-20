//! Maintainer-selected Instant Action defaults (2026-10-29; issue #86 retune 2026-11-03).
//! Combatants include humans. Mission files retain the original disc values.
//! Tickets: Normal + Good proxy, seeds 1–3 on both sides; 6–9 minute medians.
//!
//! Issue #171 side compensation (OURS, a measured last resort): on most maps the
//! side that holds the post majority early bleeds the other for the whole round
//! (bot kills stay near even), so the map side, not the players, decided it. The
//! entries marked `OURS #171` move tickets from the favoured side to the other,
//! keeping the pair's total, until bot-only Hard rounds come out near even on both
//! sides. Each names the cause found in the sweep's `sides` census. measure: the
//! bot-only Hard side sweep (`round_to_outcome` with `BOT_CENSUS=1`, 16 seeds,
//! every Conquest entry) after any sim, AI or map change; replace: fix the cause
//! and return the entry to an even pair.
pub use ui::population::Defaults;

/// Map/era pairs preserve the mission's side order. Both sides field the same
/// number of combatants on every map (issue #171: the disc's uneven BF1 Geonosis,
/// Kashyyyk and Naboo Theed populations decided those rounds for the bigger side;
/// the totals are kept). measure: bot-only Hard side sweep after sim changes.
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
    // OURS #171: side compensation, was [107, 107] (team 1 never took a team-0 platform post in the census).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_bes1", false, [20, 20], [77, 137]),
    // OURS: 13.03 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [95, 95] (team 1 never took a team-0 platform post in the census).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_bes1", true, [20, 20], [80, 110]),
    // OURS #171: side compensation, was [124, 124] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_bes2", false, [16, 16], [114, 134]),
    // OURS #171: side compensation, was [142, 142] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_bes2", true, [16, 16], [167, 117]),
    // OURS #171: side compensation, was [258, 258] (team 0 starts on three posts, team 1 on two).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_end1", false, [24, 24], [208, 308]),
    // OURS #171: side compensation, was [237, 237] (the Republic starts on one post of four).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_geo1", true, [24, 24], [292, 182]),
    // OURS #171: side compensation, was [232, 232] (the Empire starts on one post of five).
    //   Halved from [282, 182] with the layout.rs CONQUEST_PARITY neutral starts (with them
    //   the full shift gave the Empire 6/8, seeds 1-8). [257, 207] is unmeasured.
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_hot1", false, [32, 32], [257, 207]),
    // OURS: pace after the blast change. measure: repeat both proxy sides after sim changes.
    // OURS #171: side compensation, was [164, 181] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_kam1", true, [16, 16], [119, 226]),
    // OURS: 10.15 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [169, 169] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_kas1", false, [24, 24], [179, 159]),
    entry("bf1_kas1", true, [24, 24], [230, 230]),
    // OURS #171: side compensation, was [250, 250] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_kas2", false, [28, 28], [200, 300]),
    // OURS #171: side compensation, was [250, 250] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_kas2", true, [28, 28], [190, 310]),
    entry("bf1_nab1", false, [32, 32], [283, 283]),
    // OURS #171: side compensation, was [281, 281] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_nab1", true, [32, 32], [271, 291]),
    // OURS: 9.05 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [168, 168] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_nab2", false, [20, 20], [183, 153]),
    entry("bf1_nab2", true, [20, 20], [200, 200]),
    // OURS #171: side compensation, was [213, 213] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_rhn1", false, [20, 20], [263, 163]),
    // OURS: 9.04 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [184, 184] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_rhn1", true, [20, 20], [244, 124]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_rhn2", false, [16, 16], [150, 250]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_rhn2", true, [16, 16], [155, 245]),
    // OURS #171: side compensation, was [128, 128] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_tat2", false, [16, 16], [113, 143]),
    entry("bf1_tat2", true, [16, 16], [135, 135]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_yav1", false, [20, 20], [230, 170]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_yav1", true, [20, 20], [225, 175]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf1_yav2", false, [16, 16], [245, 155]),
    entry("bf1_yav2", true, [16, 16], [222, 222]),
    // OURS #171: side compensation, was [207, 207] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_cor1", false, [16, 16], [182, 232]),
    // OURS #171: side compensation, was [170, 170] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_cor1", true, [16, 16], [155, 185]),
    // OURS #171: side compensation, was [185, 185] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_dag1", false, [16, 16], [170, 200]),
    // OURS #171: side compensation, was [201, 201] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_dag1", true, [16, 16], [211, 191]),
    // OURS #171: side compensation, was [251, 251] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_dea1", false, [16, 16], [376, 126]),
    // OURS #171: side compensation, was [259, 259] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_dea1", true, [16, 16], [384, 134]),
    // OURS #171: side compensation, was [171, 171] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_end1", false, [24, 24], [161, 181]),
    // OURS #171: side compensation, was [245, 245] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_fel1", false, [16, 16], [225, 265]),
    // OURS #171: side compensation, was [217, 217] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_fel1", true, [16, 16], [202, 232]),
    entry("bf2_geo1", true, [28, 28], [250, 250]),
    entry("bf2_hot1", false, [32, 32], [150, 150]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   Halved from [100, 200] with the layout.rs CONQUEST_PARITY goal values (with them
    //   the full shift gave the Empire 6/8, seeds 1-8). [125, 175] is unmeasured.
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_kam1", false, [24, 24], [125, 175]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_kam1", true, [24, 24], [105, 195]),
    entry("bf2_kas2", false, [24, 24], [198, 198]),
    // OURS #171: side compensation, was [170, 170] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_kas2", true, [24, 24], [155, 185]),
    // OURS #171: side compensation, was [184, 184] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_mus1", false, [16, 16], [209, 159]),
    // OURS #171: side compensation, was [227, 227] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_mus1", true, [16, 16], [262, 192]),
    // OURS #171: side compensation, was [231, 231] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_myg1", false, [20, 20], [266, 196]),
    // OURS #171: side compensation, was [246, 246] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_myg1", true, [20, 20], [311, 181]),
    entry("bf2_nab2", false, [20, 20], [203, 203]),
    // OURS #171: side compensation, was [217, 217] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_nab2", true, [20, 20], [247, 187]),
    // OURS: pace after the blast change. measure: repeat both proxy sides after sim changes.
    // OURS #171: side compensation, was [187, 187] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_pol1", false, [16, 16], [147, 227]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_pol1", true, [16, 16], [110, 190]),
    // OURS #171: side compensation, was [223, 223] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_tan1", false, [16, 16], [228, 218]),
    // OURS #171: side compensation, was [227, 227] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_tan1", true, [16, 16], [247, 207]),
    // OURS #171: side compensation, was [226, 226] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_tat3", false, [16, 16], [171, 281]),
    // OURS #171: side compensation, was [275, 275] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_tat3", true, [16, 16], [220, 330]),
    // OURS #171: side compensation, was [150, 150] (authored post values: team 0 only attacks, team 1 only defends).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_uta1", false, [16, 16], [195, 105]),
    // OURS #171: side compensation, was [166, 166] (authored post values: team 0 only attacks, team 1 only defends).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_uta1", true, [16, 16], [211, 121]),
    // OURS #171: side compensation, was [182, 182] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_yav1", false, [20, 20], [167, 197]),
    // OURS #171: side compensation, was [189, 189] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("bf2_yav1", true, [20, 20], [164, 214]),
    // OURS: 9.96 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [138, 138] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("tat1", false, [24, 24], [153, 123]),
    // OURS #171: side compensation, was [208, 208] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("tat1", true, [24, 24], [238, 178]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("tat2", false, [16, 16], [115, 185]),
    // OURS: 5.96 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [182, 182] (post layout and routes; no single cause found).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    entry("tat2", true, [16, 16], [152, 212]),
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
        assert_eq!(d.combatants, [28, 28]);
        assert_eq!(d.scaled(50, 100).combatants, [14, 14]);
        assert_eq!(for_map("bf1_nab2", false).unwrap().combatants, [20, 20]);
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
