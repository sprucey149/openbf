//! `[ours]` A single-player load that failed says so on screen.
//!
//! A worker error or panic, a failed GPU upload or round start, or a map that
//! is not baked used to print only to the terminal, and the playlist moved on
//! to its next row or the menu without a word. The failure now leaves a short
//! line ("Couldn't load <map>: <reason>") over the menu and the next loading
//! screen, with the session log's path for a report (`game::session_log`).
//! The originals have no such screen; FEATURE_SET.md (Platform & meta) names
//! it. A LAN round keeps its own refusal texts (`lan.rs`).

use std::time::{Duration, Instant};

/// `[ours]` How long the lines stay once first drawn.
/// measure: whether players read it in time in the next playtest.
pub(super) const SHOWN_FOR: Duration = Duration::from_secs(20);
/// Failures listed at once (a playlist can fail several rows in a row).
const MAX_LINES: usize = 3;
/// A reason longer than this is cut (the whole one is in the log).
const MAX_REASON: usize = 140;

#[derive(Default)]
pub(super) struct LoadNotice {
    lines: Vec<String>,
    /// When the current lines were first drawn.
    since: Option<Instant>,
}

impl LoadNotice {
    /// A failed load of `map` (its shell label, or key) for `reason`.
    pub(super) fn push(&mut self, map: &str, reason: &str) {
        if self.lines.len() == MAX_LINES {
            self.lines.remove(0);
        }
        self.lines.push(text(map, reason));
        self.since = None;
    }

    #[cfg(test)]
    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Draw the lines (none once [`SHOWN_FOR`] has passed since they were
    /// first drawn), with `log` (the session log's path) under them.
    pub(super) fn draw(
        &mut self,
        ctx: &ui::egui::Context,
        now: Instant,
        log: Option<&std::path::Path>,
    ) {
        if self.lines.is_empty() {
            return;
        }
        let since = *self.since.get_or_insert(now);
        if now.saturating_duration_since(since) >= SHOWN_FOR {
            self.lines.clear();
            self.since = None;
            return;
        }
        use ui::egui::{Align2, Area, Frame, Id, Order};
        Area::new(Id::new("load_notice"))
            .anchor(Align2::CENTER_TOP, [0.0, 24.0])
            .order(Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(720.0);
                    for line in &self.lines {
                        ui.label(ui::typography::egui_text(
                            ctx,
                            &ui::typography::BODY,
                            ui::typography::tint::WARNING,
                            line,
                        ));
                    }
                    if let Some(log) = log {
                        ui.label(ui::typography::egui_text(
                            ctx,
                            &ui::typography::BODY,
                            ui::typography::tint::HELP,
                            &format!("Details: {}", log.display()),
                        ));
                    }
                });
            });
    }
}

/// `Couldn't load <map>: <reason>`, the reason on one line and cut to
/// [`MAX_REASON`] characters.
pub(super) fn text(map: &str, reason: &str) -> String {
    let flat = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    let reason = if flat.chars().count() > MAX_REASON {
        let cut: String = flat.chars().take(MAX_REASON - 3).collect();
        format!("{}...", cut.trim_end())
    } else {
        flat
    };
    format!("Couldn't load {map}: {reason}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_load_reads_as_one_short_line_and_keeps_the_newest_three() {
        assert_eq!(
            text("Hoth", "failed to prepare:\n  manifest.ron missing"),
            "Couldn't load Hoth: failed to prepare: manifest.ron missing"
        );
        let long = "x".repeat(400);
        let t = text("Hoth", &long);
        assert!(t.ends_with("..."));
        assert_eq!(t.chars().count(), "Couldn't load Hoth: ".len() + MAX_REASON);
        let mut n = LoadNotice::default();
        for m in ["a", "b", "c", "d"] {
            n.push(m, "not baked");
        }
        assert_eq!(n.lines().len(), 3);
        assert!(n.lines()[0].starts_with("Couldn't load b:"));
        // Drawn: it goes once it has been up for SHOWN_FOR.
        let ctx = ui::egui::Context::default();
        let t0 = Instant::now();
        let _ = ctx.run(Default::default(), |ctx| n.draw(ctx, t0, None));
        assert_eq!(n.lines().len(), 3);
        let _ = ctx.run(Default::default(), |ctx| n.draw(ctx, t0 + SHOWN_FOR, None));
        assert!(n.lines().is_empty());
    }
}
