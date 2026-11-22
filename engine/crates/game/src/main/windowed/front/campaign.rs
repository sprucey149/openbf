//! Rise of the Empire's launch flow: the queued intro / transition movies,
//! their synced one-shot audio, the campaign menu's save-state sync and the
//! spinning-planet battle screen. A child of `front` so it reaches `App` and
//! the private `FrontShell` fields; no behaviour changes here, only the
//! extraction of code that used to sit in `front.rs` (and the shared one-shot
//! movie reader in `crate::round_end`).

use super::{App, Camera, FrontShell};
use game::round_end::step_one_shot_movie;

impl FrontShell {
    /// Queue a mission's transition and intro cutscenes, then wait on them
    /// before starting the battle.
    pub(in super::super) fn queue_campaign_launch(&mut self, index: usize) {
        let entry = &ui::campaign::RISE_OF_THE_EMPIRE[index];
        self.cutscenes.clear();
        if index > 0 && !entry.transition_movie.is_empty() {
            self.cutscenes.push_back(entry.transition_movie.to_owned());
        }
        self.cutscenes.push_back(entry.intro_movie.to_owned());
        self.pending_campaign = Some(index);
        if let Some(m) = self.movie.as_mut() {
            m.stop();
        }
        self.stop_music();
    }

    /// A new game opens on the first mission's crawl and battle screen.
    pub(in super::super) fn queue_campaign_new_game(&mut self) {
        self.cutscenes.clear();
        if let Some(crawl) = ui::campaign::RISE_OF_THE_EMPIRE[1].crawl_movie {
            self.cutscenes.push_back(crawl.to_owned());
        }
        self.pending_campaign = None;
        self.menus.select_campaign_mission(1);
        self.menus.open_page(ui::shell_ps2::Page::CampaignBattle);
        if let Some(m) = self.movie.as_mut() {
            m.stop();
        }
        self.stop_music();
    }

    pub(in super::super) fn queue_credits(&mut self, assets_dir: &std::path::Path) {
        self.credits = super::credits::Crawl::load(assets_dir);
    }

    /// Play the front clip in the queue, with its audio, and expose its
    /// newest frame. Clips without a bake are skipped; a finished one is
    /// stopped and the next tried on the same frame.
    pub(in super::super) fn step_cutscene_movie(
        &mut self,
        dt: f32,
        mut audio: Option<&mut audio::Audio>,
        volume: f32,
    ) -> (Option<(Vec<u8>, u32, u32)>, bool) {
        let Some(m) = self.movie.as_mut() else {
            self.cutscenes.clear();
            self.movie_audio = None;
            return (None, false);
        };
        while let Some(clip) = self.cutscenes.front().cloned() {
            if !m.has(&clip) {
                self.cutscenes.pop_front();
                self.movie_audio = None;
                continue;
            }
            m.play_once(&clip);
            let frame = step_one_shot_movie(
                m,
                &clip,
                audio.as_deref_mut(),
                &mut self.movie_audio,
                volume,
                dt,
            );
            if m.is_finished() {
                m.stop();
                self.cutscenes.pop_front();
                self.movie_audio = None;
                continue;
            }
            return (frame, m.shown);
        }
        (None, false)
    }

    pub(in super::super) fn skip_cutscene(&mut self) {
        if let Some(m) = self.movie.as_mut() {
            m.stop();
        }
        self.cutscenes.pop_front();
        self.movie_audio = None;
    }
}

impl App {
    /// Sync the campaign menu with the profile before stepping its screen.
    pub(super) fn sync_campaign_menu_state(&mut self, front: &mut FrontShell, edges: u32) {
        if front.menus.training_prompted() != self.profile.campaign.training_prompted {
            front
                .menus
                .set_training_prompted(self.profile.campaign.training_prompted);
        }
        let unlocked = self
            .profile
            .campaign
            .unlocked
            .clamp(1, ui::campaign::RISE_OF_THE_EMPIRE.len() as u8);
        if front.menus.campaign_unlocked != unlocked {
            front.menus.set_campaign_unlocked(unlocked);
        }
        self.sync_campaign_saves(front);
        if edges & game::kbm::MENU_SQUARE != 0 {
            front.menus.campaign_save_delete();
        }
    }

    /// Persist the campaign changes the menu made this frame.
    pub(super) fn save_campaign_menu_state(&mut self, front: &mut FrontShell) {
        let mut dirty = false;
        if front.menus.training_prompted() != self.profile.campaign.training_prompted {
            self.profile.campaign.training_prompted = front.menus.training_prompted();
            dirty = true;
        }
        if front.menus.campaign_unlocked > self.profile.campaign.unlocked {
            self.profile.campaign.unlocked = front.menus.campaign_unlocked;
            dirty = true;
        }
        if dirty {
            if let Err(e) = self.profile.save() {
                eprintln!("campaign profile save failed: {e:#}");
            }
        }
    }

    /// The campaign-side shell commands, dispatched from the menu's command
    /// match. The other commands stay in `front::redraw_shell`.
    pub(super) fn campaign_command(
        &mut self,
        front: &mut FrontShell,
        cmd: ui::shell_ps2::ShellCommand,
    ) {
        match cmd {
            ui::shell_ps2::ShellCommand::StartCampaignNewGame => {
                self.start_campaign_new_game(front)
            }
            cmd @ (ui::shell_ps2::ShellCommand::EditCampaignSaveName
            | ui::shell_ps2::ShellCommand::WriteCampaignSave
            | ui::shell_ps2::ShellCommand::LoadCampaignSave
            | ui::shell_ps2::ShellCommand::DeleteCampaignSave) => {
                self.campaign_save_command(front, cmd)
            }
            ui::shell_ps2::ShellCommand::LaunchCampaignMission { index } => {
                let Some(entry) = ui::campaign::RISE_OF_THE_EMPIRE.get(index) else {
                    return;
                };
                match game::campaign_runtime::launch_problem(
                    &self.missions_dir,
                    &self.assets_dir,
                    entry,
                ) {
                    Some(problem) => {
                        eprintln!("campaign: {} cannot start: {problem}", entry.script);
                        front.menus.campaign_notice = Some(problem);
                    }
                    None => front.queue_campaign_launch(index),
                }
            }
            _ => {}
        }
    }

    /// One shell frame of the campaign cutscene queue. Returns whether a
    /// movie was shown and, once the queue empties, the mission to launch.
    pub(super) fn step_shell_cutscene(
        &mut self,
        front: &mut FrontShell,
        dt: f32,
        skip: bool,
        volume: f32,
    ) -> (bool, Option<usize>) {
        if skip {
            front.skip_cutscene();
        }
        front.music = None;
        let (frame, shown) = front.step_cutscene_movie(dt, self.audio.as_mut(), volume);
        if let (Some((rgba, w, h)), Some(view)) = (frame, front.view.as_mut()) {
            view.set_movie_frame(rgba, w, h);
        }
        let launch = if front.cutscenes.is_empty() {
            front.pending_campaign.take()
        } else {
            None
        };
        (shown, launch)
    }
}

/// The spinning `bf2_gal1` planet drawn behind the battle screen, or an empty
/// scene. Loading is lazy and a failed load is not retried until the screen is
/// left. A free function so the caller can hold the renderer it already borrowed.
pub(super) fn planet_for_frame(
    front: &mut FrontShell,
    assets_dir: &std::path::Path,
    renderer: &mut render::Renderer,
    dt: f32,
) -> (Camera, Vec<render::DrawCmd>) {
    let show_planet = front.menus.over_planet() && !front.menus.covered;
    if show_planet {
        if front.gal.is_none() && !front.gal_load_failed {
            front.gal = super::gal::GalPlanetView::load(assets_dir, renderer);
            front.gal_load_failed = front.gal.is_none();
        }
        if let Some(gal) = front.gal.as_mut() {
            (renderer.time, renderer.water_time) =
                render::advance_clocks(renderer.time, renderer.water_time, dt);
            let planet = ui::campaign::RISE_OF_THE_EMPIRE
                .get(front.menus.campaign_selected)
                .map_or("myg", |e| e.planet);
            let (cam, draws, lights, particles) = gal.advance_and_build(planet, dt);
            renderer.set_point_lights(lights);
            renderer.set_particles(particles);
            (cam, draws)
        } else {
            (Camera::default(), Vec::new())
        }
    } else {
        front.gal_load_failed = false;
        if front.gal.take().is_some() {
            renderer.clear_scene();
            renderer.set_particles(Vec::new());
        }
        (Camera::default(), Vec::new())
    }
}
