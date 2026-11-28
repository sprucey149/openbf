//! `[ours]` The enabled mods mount (`game::mods_runtime::mount_enabled_mods`)
//! on a worker thread behind the title screen: while it runs, the "Press
//! START" line says what is loading and Start does nothing. After an update
//! the mount includes converting each mod again, which takes minutes; before
//! this the game had no window until it was done. The originals mount no mods.

use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Instant;

use game::mods_runtime::MountStatus;

use super::App;

/// The mount the shell has yet to start or finish.
pub(super) enum ModBoot {
    /// Asked for: starts once the window is up.
    Pending { no_mods: bool },
    Running {
        worker: JoinHandle<()>,
        started: Instant,
    },
}

impl ModBoot {
    fn start(no_mods: bool, assets: PathBuf, missions: PathBuf) -> std::io::Result<Self> {
        let worker = std::thread::Builder::new()
            .name("mod-mount".into())
            .spawn(move || {
                let _ = game::mods_runtime::mount_enabled_mods(
                    &extensions::profile::discover_data_dir(),
                    &assets,
                    &missions,
                    no_mods,
                );
            })?;
        Ok(Self::Running {
            worker,
            started: Instant::now(),
        })
    }
}

/// The most of a mod's name the title's line shows: with more, the line is
/// wider than the 4:3 screen it is centred on.
const NAME_CHARS: usize = 22;

/// The title screen's line while the mods mount. A conversion takes minutes,
/// so it counts the time to show that it is still working.
fn loading_line(status: &MountStatus, seconds: u64) -> String {
    let Some(name) = &status.converting else {
        return "Loading...".to_owned();
    };
    let mut short: String = name.chars().take(NAME_CHARS).collect();
    short.truncate(short.trim_end().len());
    format!("Converting {short}... {}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_line_names_the_mod_being_converted_and_keeps_it_short() {
        let mut status = MountStatus::default();
        assert_eq!(loading_line(&status, 3), "Loading...");
        status.converting = Some("A Mod With A Very Long Folder Name".into());
        assert_eq!(
            loading_line(&status, 125),
            "Converting A Mod With A Very Long... 2:05"
        );
    }
}

impl App {
    /// The shell's start: the title screen, with the pending mount behind it.
    pub(super) fn begin_shell(&mut self) {
        self.init_shell();
        let Some(ModBoot::Pending { no_mods }) = self.mod_boot else {
            return;
        };
        match ModBoot::start(no_mods, self.assets_dir.clone(), self.missions_dir.clone()) {
            Ok(boot) => self.mod_boot = Some(boot),
            Err(e) => {
                eprintln!("mods: the loading thread did not start: {e}");
                self.mod_boot = None;
            }
        }
    }

    /// One title frame's share of the mount: the line to show in the prompt's
    /// place while it runs. In the frame it ends, `front` takes the mods'
    /// texts and icons and this returns `None`.
    pub(super) fn step_mod_boot(&mut self, front: &mut super::front::FrontShell) -> Option<String> {
        let Some(ModBoot::Running { worker, started }) = &self.mod_boot else {
            return None;
        };
        if !worker.is_finished() {
            let status = game::mods_runtime::mount_status();
            return Some(loading_line(&status, started.elapsed().as_secs()));
        }
        let Some(ModBoot::Running { worker, started }) = self.mod_boot.take() else {
            return None;
        };
        if worker.join().is_err() {
            eprintln!("mods: loading them failed; see the lines above");
        }
        println!("mods: ready after {:.1} s", started.elapsed().as_secs_f64());
        front.reload_menus(&self.assets_dir);
        None
    }
}
