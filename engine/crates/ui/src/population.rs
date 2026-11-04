//! Shared map-relative population math used by menus and battle setup.

/// The largest Team size percent: the smallest map default is 16 per side, so
/// every map has reached the 32-per-side ceiling here (maintainer 2026-10-30).
pub const MAX_TEAM_PCT: u32 = 200;

/// Round to nearest, halves upward. The 32-combatant ceiling limits the entire
/// pair together so asymmetric missions keep their ratio at large percentages.
pub fn team_counts(defaults: [u32; 2], pct: u32) -> [u32; 2] {
    let largest = defaults.into_iter().max().unwrap_or(1).max(1);
    let pct = pct.clamp(10, MAX_TEAM_PCT);
    let (num, den) = if largest * pct > 3200 {
        (32, largest)
    } else {
        (pct, 100)
    };
    defaults.map(|n| ((n * num + den / 2) / den).max(1))
}
pub fn tickets(defaults: [u32; 2], pct: u32) -> [u32; 2] {
    defaults.map(|n| ((u64::from(n) * u64::from(pct.clamp(10, 500)) + 50) / 100).max(1) as u32)
}

/// One map/era's Instant Action defaults. Combatants include humans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Defaults {
    pub combatants: [u32; 2],
    pub tickets: [u32; 2],
}

/// (map key, Clone Wars era, defaults): `game::conquest_defaults::TABLE`.
pub type Entry = (&'static str, bool, Defaults);

impl Defaults {
    pub fn scaled(self, team_pct: u32, ticket_pct: u32) -> Self {
        Self {
            combatants: team_counts(self.combatants, team_pct),
            tickets: tickets(self.tickets, ticket_pct),
        }
    }
    pub fn initial_ai(self, human_team: u8) -> [u32; 2] {
        [0, 1].map(|t| {
            self.combatants[t].saturating_sub(u32::from(t == usize::from(human_team.min(1))))
        })
    }
}

/// What a match looks like on average over every map/era entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    /// Mean combatants per side, as battles resolve them (cap and rounding).
    pub team: f32,
    /// Mean reinforcements per side.
    pub tickets: f32,
    /// Mean match length in minutes ([`match_minutes`]).
    pub minutes: f32,
}

/// Every entry weighs the same; both sides of an entry are averaged.
pub fn summary(table: &[Entry], team_pct: u32, ticket_pct: u32) -> Summary {
    let n = table.len().max(1) as f32;
    let mean = |f: fn(Defaults) -> [u32; 2]| {
        table
            .iter()
            .map(|(_, _, d)| f(d.scaled(team_pct, ticket_pct)).iter().sum::<u32>() as f32 / 2.0)
            .sum::<f32>()
            / n
    };
    Summary {
        team: mean(|d| d.combatants),
        tickets: mean(|d| d.tickets),
        minutes: match_minutes(team_pct, ticket_pct),
    }
}

/// The measured model (`match_length.rs`, from `scripts/calibrate_match_length.py`):
/// the team-size curve on log-log axes, flat at its population cap, times
/// the measured ticket-size curve (its end slopes extrapolate).
pub fn match_minutes(team_pct: u32, ticket_pct: u32) -> f32 {
    use crate::match_length::{TEAM_MINUTES, TICKET_SCALE};
    curve(&TEAM_MINUTES, team_pct.clamp(10, MAX_TEAM_PCT), true)
        * curve(&TICKET_SCALE, ticket_pct.clamp(10, 500), false)
}

fn curve(axis: &[(u32, f32)], pct: u32, flat_above: bool) -> f32 {
    let x = pct as f32;
    let last = axis[axis.len() - 1];
    if flat_above && x >= last.0 as f32 {
        last.1
    } else {
        let i = axis
            .windows(2)
            .position(|w| x < w[1].0 as f32)
            .unwrap_or(axis.len() - 2);
        let ((x0, y0), (x1, y1)) = (axis[i], axis[i + 1]);
        let slope = (y1 / y0).ln() / (x1 as f32 / x0 as f32).ln();
        y0 * (x / x0 as f32).powf(slope)
    }
}

fn minutes_text(m: f32) -> String {
    match m.round().max(1.0) as u32 {
        1 => "1 minute".into(),
        n => format!("{n} minutes"),
    }
}

/// The help box under Team size and under Reinforcements, in that order.
pub fn notes(s: Summary) -> [String; 2] {
    let length = format!("Average match length: {}", minutes_text(s.minutes));
    [
        format!("Average team size: {} units\n{length}", s.team.round()),
        format!(
            "Average reinforcements: {} units\n{length}",
            s.tickets.round()
        ),
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentages_round_halves_up_and_cap_the_pair_together() {
        assert_eq!(team_counts([31, 25], 50), [16, 13]);
        assert_eq!(team_counts([31, 25], 500), [32, 26]);
        assert_eq!(team_counts([19, 21], 100), [19, 21]);
        assert_eq!(team_counts([16, 16], 10), [2, 2]);
        assert_eq!(tickets([200, 220], 125), [250, 275]);
        assert_eq!(tickets([25, 25], 10), [3, 3]);
    }

    fn d(c: [u32; 2], t: [u32; 2]) -> Entry {
        (
            "x",
            false,
            Defaults {
                combatants: c,
                tickets: t,
            },
        )
    }

    #[test]
    fn summary_averages_resolved_sides_over_every_entry() {
        let table = [d([16, 16], [100, 100]), d([31, 25], [200, 220])];
        let s = summary(&table, 100, 100);
        assert_eq!((s.team, s.tickets), (22.0, 155.0));
        // 200%: 32/32 and the capped pair 32/26; 50% tickets: 50/50, 100/110.
        let s = summary(&table, 200, 50);
        assert_eq!((s.team, s.tickets), (30.5, 77.5));
    }

    #[test]
    fn notes_read_in_plain_units_and_minutes() {
        let s = Summary {
            team: 21.4,
            tickets: 205.6,
            minutes: 7.2,
        };
        assert_eq!(
            notes(s),
            [
                "Average team size: 21 units\nAverage match length: 7 minutes".to_string(),
                "Average reinforcements: 206 units\nAverage match length: 7 minutes".to_string(),
            ]
        );
        assert!(notes(Summary { minutes: 0.7, ..s })[0].ends_with("1 minute"));
    }

    #[test]
    fn curves_keep_the_population_cap_and_extrapolate_ticket_end_slopes() {
        let axis = [(50, 1.0), (100, 2.0), (200, 8.0)];
        assert_eq!(curve(&axis, 400, true), 8.0);
        assert!((curve(&axis, 400, false) - 32.0).abs() < 1e-4);
        assert!((curve(&axis, 25, false) - 0.5).abs() < 1e-4);
    }

    /// The model goes through its own measured points: interpolation and
    /// exponent read the generated table the way the script fits it.
    #[test]
    fn model_reads_the_generated_table() {
        use crate::match_length::{MEASURED, TEAM_MINUTES, TICKET_SCALE};
        assert!(TEAM_MINUTES.windows(2).all(|w| w[0].0 < w[1].0));
        for (pct, minutes) in TEAM_MINUTES {
            assert!((match_minutes(pct, 100) - minutes).abs() < 1e-3);
        }
        let last = TEAM_MINUTES[TEAM_MINUTES.len() - 1];
        assert_eq!(match_minutes(500, 100), last.1);
        assert!(TICKET_SCALE
            .windows(2)
            .all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1));
        for (pct, scale) in TICKET_SCALE {
            let ratio = match_minutes(100, pct) / match_minutes(100, 100);
            assert!((ratio - scale).abs() < 1e-4);
        }
        assert!(match_minutes(100, 500) > match_minutes(100, 300));
        // Every measured cell the fit used sits on the model within 10%,
        // and the check cells (not fitted) within 15%.
        for (team, ticket, rounds, unfinished, minutes) in MEASURED {
            assert!(rounds > 0);
            if unfinished > 0 {
                continue;
            }
            let fitted = ticket == 100 || team == 100;
            let err = (match_minutes(team, ticket) / minutes - 1.0).abs();
            assert!(
                err < if fitted { 0.10 } else { 0.15 },
                "{team}/{ticket}: {err}"
            );
        }
    }
}
