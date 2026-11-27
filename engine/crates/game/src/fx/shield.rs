//! Ordnance shells (`TrailEffect` naming a `Shield` record): the DLC Force
//! ball's `fisto_bubble` is a droideka-type shell, not a particle effect
//! (`mungefs::shield`), drawn round the ball for as long as it flies. The
//! shell is the droideka's (`crate::viewer::droidekas::shell_draws`, B2
//! `FUN_0046e378`) with the record's own colours and size, growing over its
//! `TurnOnTime` from the round's launch (B2 `FUN_00460030`). Presentation
//! only: it reads the sim and writes nothing back.

use super::Fx;
use crate::viewer::droidekas::{shell_draws, shell_scale, ShellLook};
use render::{Camera, ParticleDraw, Renderer, TextureHandle};

/// The rim pass's flat sheet and the `specular` table, uploaded once.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sheets {
    white: Option<TextureHandle>,
    table: Option<TextureHandle>,
}

/// The shell record `w`'s rounds wear: its ordnance's `TrailEffect`, when
/// that names a shield in `lib`.
pub fn shell_of<'a>(
    lib: &'a super::Library,
    w: &core::weapon::WeaponClass,
) -> Option<&'a manifest::FxShield> {
    let trail = w.ordnance.impact.as_ref()?.trail.as_deref()?;
    lib.shield(trail)
}

/// The shell's scale `age` seconds into the round's flight.
pub fn grown(s: &manifest::FxShield, age: f32) -> f32 {
    shell_scale(true, age, s.turn_on_time, s.turn_off_time).unwrap_or(1.0)
}

impl Fx {
    /// Append the shells of the rounds in flight to `out`.
    pub(super) fn shell_draws(
        &mut self,
        r: &mut Renderer,
        cam: &Camera,
        sim: &core::sim::Sim,
        out: &mut Vec<ParticleDraw>,
    ) {
        let Some(lib) = &self.library else { return };
        if lib.shields.is_empty() {
            return;
        }
        let rounds: Vec<_> = sim
            .bolts
            .iter()
            .filter_map(|b| {
                let s = shell_of(lib, sim.weapon_classes.get(b.weapon_class)?)?;
                Some((b.pos, s.clone(), b.age))
            })
            .collect();
        if rounds.is_empty() {
            return;
        }
        let table_path = lib.shield_table.clone();
        let sheets = *self.shell_sheets.get_or_insert_with(|| Sheets {
            white: crate::viewer::upload_cpu_texture(
                r,
                &crate::assets::CpuTexture {
                    width: 1,
                    height: 1,
                    rgba: vec![255; 4],
                    extra_mips: Vec::new(),
                    detail: None,
                    bump: None,
                    env: None,
                },
                "ordnance_shell",
            )
            .map_err(|e| eprintln!("ordnance shell: {e:#}"))
            .ok(),
            table: table_path.and_then(|p| {
                pak::read(&p)
                    .map_err(anyhow::Error::from)
                    .and_then(|b| crate::assets::load_ktx2(&b))
                    .and_then(|t| crate::viewer::upload_cpu_texture(r, &t, "ordnance_shell_table"))
                    .map_err(|e| eprintln!("ordnance shell table: {e:#}"))
                    .ok()
            }),
        });
        let sun = r.lighting.sun_dir;
        for (pos, s, age) in rounds {
            let look = ShellLook {
                color: s.color,
                specular_color: s.specular_color,
                texture: sheets.white,
                table: sheets.table,
            };
            shell_draws(pos, s.radius, grown(&s, age), cam, sun, &look, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bubble() -> manifest::FxShield {
        manifest::FxShield {
            hash: crate::classdb::fnv1a_lowercase("fisto_bubble"),
            name: "fisto_bubble".into(),
            shape: "droideka".into(),
            texture: Some("cis_inf_droideka_shield".into()),
            color: [0, 192, 220, 255],
            specular_color: [140, 150, 100, 255],
            radius: 0.75,
            height: 0.75,
            turn_on_time: 2.0,
            turn_off_time: 0.15,
            scroll_rate: 1.0,
            unread_b28af6f4: 7.5,
        }
    }

    /// A round whose ordnance trails `fisto_bubble` wears the shell; the
    /// shell grows over the record's 2 s `TurnOnTime`, not the droideka's
    /// 0.15 s; any other trail (a particle effect) wears none.
    #[test]
    fn a_trail_naming_a_shield_record_wears_its_shell() {
        let lib = super::super::Library::from_sets(
            super::super::emitter::Era::Bf2,
            vec![manifest::FxSet {
                game: "bf2".into(),
                effects: Vec::new(),
                beams: Vec::new(),
                lightning: Vec::new(),
                shields: vec![bubble()],
            }],
            &mut super::super::Lcg(1),
        );
        let mut w = core::weapon::WeaponClass::default();
        assert!(shell_of(&lib, &w).is_none());
        w.ordnance.impact = Some(core::impact::ImpactLook {
            trail: Some("fisto_bubble".into()),
            ..Default::default()
        });
        assert_eq!(shell_of(&lib, &w), Some(&bubble()));
        w.ordnance.impact.as_mut().unwrap().trail = Some("com_sfx_dusttrail".into());
        assert!(shell_of(&lib, &w).is_none());
        let s = bubble();
        assert_eq!(grown(&s, 0.0), 0.0);
        assert!((grown(&s, 0.5) - 0.25 * 1.127_659_6).abs() < 1e-5);
        assert!(grown(&s, 0.5) < 0.3, "still small a quarter of the way");
        assert!((grown(&s, 2.0) - 1.0).abs() < 1e-5);
        assert_eq!(grown(&s, 9.0), 1.0);
    }
}
