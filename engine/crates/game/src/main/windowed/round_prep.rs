//! The round's CPU side, built on the loading worker while the loading
//! screen animates (LAN §10.5, §8b.3, issue #168).
//!
//! Rules:
//! - [`RoundJob`] is everything the round build reads, captured when the
//!   loading starts (the playlist row, the Session Options, the profile's
//!   seed and tier, a LAN round's setup and local slot). Nothing it reads can
//!   change while the loading screen shows.
//! - [`RoundJob::build`] runs on the loading worker after the scene parse:
//!   the battle setup, the walk (`make_walk`, or every LAN peer's
//!   `round_setup::build_round`), the sound library (loaded beside the walk
//!   build, then decoded for the battle's levels), and the CPU-only battle
//!   resources (foley, engine sounds, ambience, the feed, the effects
//!   library and the weather), the last two groups side by side.
//! - It is the same code the event thread ran before, on the same inputs,
//!   in the same order for the sim: the walk is built first and nothing else
//!   writes it here. The single-player installs (cheats, medal career,
//!   assist, handicap), the spawn screens and every GPU upload stay on the
//!   event thread, after the worker, as before. So the tick-0 `Sim::hash`
//!   equals the serial build's (`tests::worker_round_equals_the_serial_build`).
//! - The renderer, the window, the audio device and the net link are never
//!   touched here; the net thread keeps its heartbeats on its own thread.

use super::BattleSetup;
use anyhow::Result;
use game::walk::WalkSim;
use std::path::PathBuf;
use std::time::Instant;

/// What the round build reads (module rules).
#[derive(Clone)]
pub(super) struct RoundJob {
    pub missions_dir: PathBuf,
    pub assets_dir: PathBuf,
    /// The map key.
    pub name: String,
    pub mode: String,
    pub row_armies: [String; 2],
    pub session: ui::shell::SessionOptions,
    pub seed: u32,
    pub difficulty: core::skill::Difficulty,
    /// A LAN round's setup and this peer's slot.
    pub net: Option<(game::netplay::RoundSetup, u8)>,
}

/// The CPU-only battle resources the event thread used to load after the
/// walk (`App::battle_resources_for`).
pub(super) struct Extras {
    pub sounds: Option<game::sounds::SoundLib>,
    pub foley: Option<game::foley::Foley>,
    pub engines: game::engine_sound::EngineSounds,
    pub ambience: game::ambience::Ambience,
    pub feed: game::event_feed::EventFeed,
    pub fx: game::fx::Fx,
    pub weather: Option<game::weather::Weather>,
}

/// The built round (module rules).
pub(super) struct PreparedRound {
    pub setup: BattleSetup,
    pub teams: [String; 2],
    pub walk: WalkSim,
    pub extras: Extras,
    /// The worker's laps, for the `loading:` line.
    pub laps: String,
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

impl RoundJob {
    /// The setup and the walk, exactly as `App::start_battle` built them.
    pub fn walk(&self, scene: &game::assets::Scene) -> Result<(BattleSetup, [String; 2], WalkSim)> {
        use super::super::round_setup::{battle_setup, build_round, shell_setup};
        match &self.net {
            Some((round, local)) => {
                let (setup, teams) = battle_setup(round, &self.missions_dir, &self.assets_dir)?;
                let walk = build_round(scene, round, &self.missions_dir, &self.assets_dir, *local)?;
                Ok((setup, teams, walk))
            }
            None => {
                let (setup, teams) = shell_setup(
                    &self.missions_dir,
                    &self.assets_dir,
                    &self.name,
                    &self.mode,
                    &self.row_armies,
                    &self.session,
                    self.seed,
                    self.difficulty,
                )?;
                let walk = super::make_walk(scene, &setup)?;
                Ok((setup, teams, walk))
            }
        }
    }

    /// The whole CPU side (module rules).
    pub fn build(self, scene: &game::assets::Scene) -> Result<PreparedRound> {
        let started = Instant::now();
        let dir = scene.dir.clone();
        let (built, (mut sounds, sounds_load_ms)) = std::thread::scope(|s| {
            let loader = std::thread::Builder::new()
                .name("loading-sounds".into())
                .spawn_scoped(s, || {
                    let t = Instant::now();
                    (game::sounds::SoundLib::load(&dir), ms(t))
                });
            let built = self.walk(scene);
            let sounds = match loader {
                Ok(h) => h.join().unwrap_or((None, 0.0)),
                Err(_) => {
                    let t = Instant::now();
                    (game::sounds::SoundLib::load(&dir), ms(t))
                }
            };
            (built, sounds)
        });
        let (setup, teams, walk) = built?;
        let walk_ms = ms(started);
        let rest = Instant::now();
        let source_game = scene.manifest.source_game.clone();
        let (decode, extras) = std::thread::scope(|s| {
            let decode = std::thread::Builder::new()
                .name("loading-decode".into())
                .spawn_scoped(s, || {
                    let t = Instant::now();
                    // The bank's foley table comes with its selection.
                    let mut table = None;
                    let preloaded = sounds.as_mut().map(|sl| {
                        table = sl.for_walk(&scene.dir, &walk);
                        sl.preload()
                    });
                    (preloaded, ms(t), table)
                })
                .ok();
            let t = Instant::now();
            let mut fx = game::fx::Fx::for_walk(&self.assets_dir, &scene.dir, &source_game, &walk);
            fx.set_water(&scene.manifest);
            fx.set_scene_effects(scene);
            let extras = (
                game::engine_sound::EngineSounds::new(&scene.dir, &walk),
                game::ambience::Ambience::load(&scene.manifest, &scene.dir, &setup.classes_dir),
                game::event_feed::EventFeed::for_walk(&self.assets_dir, &source_game, &walk),
                fx,
                game::weather::Weather::load(
                    &scene.manifest,
                    &self.assets_dir,
                    &scene.dir,
                    super::WEATHER_SEED,
                ),
                ms(t),
            );
            let decode = match decode {
                Some(h) => h.join().ok(),
                None => None,
            };
            (decode, extras)
        });
        // A failed decode thread decodes here, so the battle never plays a
        // take it did not decode.
        let (preloaded, decode_ms, foley_table) = match decode {
            Some(d) => d,
            None => {
                let t = Instant::now();
                let mut table = None;
                let p = sounds.as_mut().map(|sl| {
                    table = sl.for_walk(&scene.dir, &walk);
                    sl.preload()
                });
                (p, ms(t), table)
            }
        };
        // Foley reads the bank's table, so it loads once the decode is in.
        let foley = game::foley::Foley::load(&scene.dir, &walk, foley_table);
        if let Some((n, bytes, ms)) = preloaded {
            println!(
                "sounds: {n} takes decoded at load ({:.1} MB, {ms:.0} ms)",
                bytes as f64 / (1024.0 * 1024.0)
            );
        }
        let (engines, ambience, feed, fx, weather, extras_ms) = extras;
        let laps = format!(
            "round_build_ms={walk_ms:.3} sounds_load_ms={sounds_load_ms:.3} \
             sounds_decode_ms={decode_ms:.3} resources_ms={extras_ms:.3} \
             overlap_wall_ms={:.3}",
            ms(rest)
        );
        Ok(PreparedRound {
            setup,
            teams,
            walk,
            extras: Extras {
                sounds,
                foley,
                engines,
                ambience,
                feed,
                fx,
                weather,
            },
            laps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn job(map: &str, net: Option<(game::netplay::RoundSetup, u8)>) -> RoundJob {
        RoundJob {
            missions_dir: Path::new("../../missions").to_owned(),
            assets_dir: Path::new("../../assets_baked").to_owned(),
            name: map.into(),
            mode: "con".into(),
            row_armies: ["rep".into(), "cis".into()],
            session: ui::shell::SessionOptions::default(),
            seed: 42,
            difficulty: core::skill::Difficulty::Normal,
            net,
        }
    }

    /// The round built on a worker thread (with the sounds and resources
    /// beside it) has the serial build's tick-0 hash and static digest, in
    /// single player and as a LAN guest; and the worker's laps are printed
    /// (the event thread's time this moved off it).
    #[test]
    fn worker_round_equals_the_serial_build() {
        let root = Path::new("../../assets_baked");
        for map in ["bf2_kas2", "bf2_nab2"] {
            if !root.join(map).is_dir() || !Path::new("../../missions").is_dir() {
                eprintln!("skipped: {map} is not baked here");
                continue;
            }
            worker_round_equals_the_serial_build_on(root, map);
        }
    }

    fn worker_round_equals_the_serial_build_on(root: &Path, map: &str) {
        let (scene, _load) = game::assets::load_battle_scene(&root.join(map), "con").unwrap();
        let session = ui::shell::SessionOptions::default();
        let host = game::profile::Profile::default();
        let mut round = game::netplay::RoundSetup::instant_action(
            map,
            "con",
            ["rep".into(), "cis".into()],
            &session,
            &host,
        );
        for (slot, name) in [(0u8, "Host"), (1, "Guest")] {
            round.humans.push(game::netplay::HumanSeat {
                slot,
                name: name.into(),
                team: None,
                cheats: 0,
                profile: game::netplay::PlayerProfile::default(),
            });
        }
        for net in [None, Some((round, 1u8))] {
            let lan = net.is_some();
            let j = job(map, net);
            let serial = {
                let t = Instant::now();
                let (_, _, w) = j.walk(&scene).unwrap();
                println!("{map}: serial round build {:.1} ms", ms(t));
                w
            };
            let worker = std::thread::scope(|s| {
                let j = j.clone();
                let scene = &scene;
                s.spawn(move || j.build(scene).unwrap()).join().unwrap()
            });
            println!("{map}: worker laps {}", worker.laps);
            assert_eq!(worker.walk.sim.tick, 0);
            assert_eq!(worker.walk.sim.hash(), serial.sim.hash());
            assert_eq!(worker.walk.sim.static_digest(), serial.sim.static_digest());
            assert_eq!(worker.walk.local_slot, serial.local_slot);
            // The stats page's opening medals: a shell battle's career goes
            // in on the event thread after the worker built the feed, which
            // reads them again then (`App::battle_resources_for`), so they
            // are the serial build's (career first, then the feed); a LAN
            // guest's are its own slot's.
            let medals = [1, 0, 2, 0, 0, 0, 0, 0, 3];
            let (mut walk, mut feed, mut serial) = (worker.walk, worker.extras.feed, serial);
            if !lan {
                assert_eq!(feed.round().opening_medals(), None);
                walk.sim.install_medal_career(walk.player, medals);
                serial.sim.install_medal_career(serial.player, medals);
            }
            feed.set_opening_medals(&walk);
            let assets = Path::new("../../assets_baked");
            let serial_feed = game::event_feed::EventFeed::for_walk(assets, "bf2", &serial);
            let opening = feed.round().opening_medals();
            assert_eq!(opening, serial_feed.round().opening_medals());
            assert_eq!(opening, Some(if lan { [0; 9] } else { medals }));
        }
    }
}
