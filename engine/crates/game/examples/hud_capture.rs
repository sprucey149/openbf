//! Card BHUD2's picture: BF2's HUD over a map's golden camera at any window
//! size, with the values of the PS2 reference frame
//! `engine/artifacts/pcsx2/ref/bf2_hot1/cp1_cp6_postA_v1.png` — for laying a
//! 640×480 render beside that frame, and a 16:9 one beside the 4:3.
//!
//!   cargo run --release -p game --example hud_capture -- <map> <w> <h> <out.png> [weapon key]
//!
//! `OPENBF_ASSETS` names a bake other than `engine/assets_baked`. Card
//! EVMSG's pictures: `HUD_LINE="Ben Pulastra killed Player 1"` puts that line
//! (own-team blue; `;` separates several) in the message box, `HUD_KILLS=1`
//! scores a kill and a point so the statistics pop-up shows. Card GREN's:
//! `HUD_GRENADES=4` (and `HUD_GRENADE_KEY=<weapon key>`) shows the grenade row.
//! Card HUDW's: `HUD_CLASS` / `HUD_ICON` (and `HUD_GRENADE_CLASS` /
//! `HUD_GRENADE_ICON`) are the weapon class's name and `IconTexture`,
//! `HUD_AMMO=inf` an infinite weapon, `HUD_FIRST="key[,class[,icon]]"` a
//! counted weapon held before it (what a switch leaves behind).
//! Card BMAP's: the minimap is drawn from the golden camera, or from
//! `HUD_MAP_EYE="x,z,heading degrees"` (re-hud-map's eye and θ).
//! Issue #78's: `HUD_HEAT=0.5` lights the ring, `HUD_REFIRE=0.75` overheats the
//! weapon, with the line and the red bits.

use anyhow::{anyhow, Context, Result};
use game::viewer::{camera_from, upload_scene, GOLDEN_CAM};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [map, w, h, out, rest @ ..] = args.as_slice() else {
        return Err(anyhow!(
            "usage: hud_capture <map> <w> <h> <out.png> [weapon key]"
        ));
    };
    let (w, h): (u32, u32) = (w.parse()?, h.parse()?);
    let root = std::env::var_os("OPENBF_ASSETS")
        .map_or_else(game::assets::baked_root, std::path::PathBuf::from);
    let mut scene = game::assets::load_scene(&root.join(map)).context("scene")?;
    // The conquest posts, for the minimap (card BMAP).
    scene.apply_mode_layer("con");
    let mut r = render::Renderer::new_headless(w, h).map_err(|e| anyhow!("{e}"))?;
    let gpu = upload_scene(&mut r, &scene)?;
    // `HUD_GAME=bf1` rows take the BF1 table rule (`ui::hud::HudWeapon::bf2`).
    let bf2 = std::env::var("HUD_GAME").map_or(true, |g| g != "bf1");
    let model = ui::hud::HudModel {
        tickets: [147, 145],
        hp: 300.0,
        hp_max: 300.0,
        energy: 100.0,
        energy_max: 100.0,
        weapon: Some(ui::hud::HudWeapon {
            key: rest
                .first()
                .cloned()
                .unwrap_or_else(|| "all_weap_inf_rifle".into()),
            class: std::env::var("HUD_CLASS").unwrap_or_default(),
            icon: std::env::var("HUD_ICON").ok(),
            name: String::new(),
            total: match std::env::var("HUD_AMMO").as_deref() {
                Ok("inf") => None,
                Ok(n) => n.parse().ok(),
                Err(_) => Some(250),
            },
            clip_fraction: 1.0,
            bf2,
        }),
        // Card GREN's picture: `HUD_GRENADES=4` shows the grenade row.
        weapon2: std::env::var("HUD_GRENADES")
            .ok()
            .and_then(|n| n.parse().ok())
            .map(|n| ui::hud::HudWeapon {
                key: std::env::var("HUD_GRENADE_KEY")
                    .unwrap_or_else(|_| "all_weap_inf_thermaldetonator".into()),
                class: std::env::var("HUD_GRENADE_CLASS").unwrap_or_default(),
                icon: std::env::var("HUD_GRENADE_ICON").ok(),
                name: String::new(),
                total: Some(n),
                clip_fraction: 1.0,
                bf2,
            }),
        gauge: {
            let num = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
            ui::hud::HudGauge {
                ring: num("HUD_HEAT").unwrap_or(0.0),
                refire: num("HUD_REFIRE").unwrap_or(0.0),
                overheated: num("HUD_REFIRE").is_some(),
            }
        },
        reticle: Some(0),
        // `HUD_GAME=bf1` puts the reticle at BF1's height.
        reticle_y: game::walk::ps2_reticle_y(
            &std::env::var("HUD_GAME").unwrap_or_else(|_| "bf2".into()),
        ),
        ..Default::default()
    };
    let mut hud = game::hud_feed::HudView::for_renderer(&root, &r)?;
    let cam = camera_from(GOLDEN_CAM);
    let (eye, yaw) = match std::env::var("HUD_MAP_EYE") {
        Ok(v) => {
            let n: Vec<f32> = v.split(',').filter_map(|t| t.trim().parse().ok()).collect();
            let [x, z, deg] = n.as_slice() else {
                return Err(anyhow!("HUD_MAP_EYE=x,z,degrees"));
            };
            (glam::Vec3::new(*x, 0.0, *z), -deg.to_radians())
        }
        Err(_) => (cam.position, cam.yaw),
    };
    let model = ui::hud::HudModel {
        map: Some(game::minimap::still(&scene, eye, yaw)),
        stats: Some(ui::hud::HudStats::default()),
        ..model
    };
    if let Ok(first) = std::env::var("HUD_FIRST") {
        let mut t = first.split(',').map(str::to_owned);
        let before = ui::hud::HudModel {
            weapon: Some(ui::hud::HudWeapon {
                key: t.next().unwrap_or_default(),
                class: t.next().unwrap_or_default(),
                icon: t.next(),
                name: String::new(),
                total: Some(146),
                clip_fraction: 1.0,
                bf2,
            }),
            ..model.clone()
        };
        hud.update(&before, 0.0, [w, h]);
        hud.update(&before, 1.0, [w, h]);
    }
    hud.update(&model, 0.0, [w, h]);
    // Past the weapon rows' fades.
    if std::env::var_os("HUD_FIRST").is_some() {
        hud.update(&model, 1.0, [w, h]);
    }
    // The first update's long settle outlasts the overheat line: send it again.
    if std::env::var_os("HUD_REFIRE").is_some() {
        hud.update(&model, 0.3, [w, h]);
    }
    let lines: Vec<ui::hud::HudMessage> = std::env::var("HUD_LINE")
        .unwrap_or_default()
        .split(';')
        .filter(|t| !t.is_empty())
        .map(|t| ui::hud::HudMessage {
            text: t.to_owned(),
            color: ui::hud::message_color::OWN,
        })
        .collect();
    let kills: i32 = std::env::var("HUD_KILLS")
        .ok()
        .and_then(|k| k.parse().ok())
        .unwrap_or(0);
    if !lines.is_empty() || kills != 0 {
        let scored = ui::hud::HudModel {
            stats: Some(ui::hud::HudStats {
                points: kills,
                kills,
                ..Default::default()
            }),
            ..model
        };
        let n = lines.len();
        hud.update(
            &ui::hud::HudModel {
                messages: lines,
                ..scored.clone()
            },
            0.0,
            [w, h],
        );
        // A line is admitted when the one before it is fully shown.
        for _ in 0..n.max(1) {
            hud.update(&scored, 0.6, [w, h]);
        }
    }
    r.render_with_overlay(
        &camera_from(GOLDEN_CAM),
        &gpu.draws,
        gpu.clear,
        Some(&mut |dev, q, enc, view, size| hud.paint(dev, q, enc, view, size)),
    )
    .map_err(|e| anyhow!("render: {e}"))?;
    let (rgba, cw, ch) = r.capture_rgba().map_err(|e| anyhow!("{e}"))?;
    std::fs::write(
        out,
        render::encode_png_rgb(&rgba, cw, ch).map_err(|e| anyhow!("{e}"))?,
    )?;
    for name in [
        "player1health_fill",
        "player1teams_team1reinforcements",
        "player1teams_team2reinforcements",
        "player1reticule_outline",
        "player1systemMessages",
        "player1statskills.icon",
        "player1statspoints.icon",
    ] {
        println!("{name}: {:?}", hud.bounds(name));
    }
    Ok(())
}
