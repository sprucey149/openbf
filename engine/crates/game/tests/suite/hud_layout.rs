//! Card BHUD2's acceptance, on the CPU: BF2's HUD laid out for a 640×480
//! window with the values of the PS2 frame
//! `engine/artifacts/pcsx2/ref/bf2_hot1/cp1_cp6_postA_v1.png` (health full,
//! 250 + 4 rounds, reinforcements 147 / 145) lands where that frame draws it
//! (`analysis/re-hud.md` §7), and a 16:9 window keeps every group's 4:3
//! offset from its own edge. Card EVMSG's: the kill line of
//! `engine/artifacts/pcsx2/ref/evmsg/bf2_hot1_kill_line.png` and the statistics
//! pop-up of `bf2_hot1_cp_tooltip_xshot.png` land where those frames have them
//! (`analysis/re-event-messages.md` §2, §5).
//!
//! Card BMAP's: the minimap puts HOT1's posts where two PS2 frames have
//! them (`analysis/re-hud-map.md` §7).
//!
//! Card SQHUD's: BF1's squad order cross, its highlight and its labels land
//! where three PS2 frames have them
//! (`engine/artifacts/lead_tmp/sqack/scenes/{F3,H3,A3}/hud.png`,
//! `analysis/re-squad-orders.md` §2b), fade by the PS2's timer and keep their
//! place at the right edge of a 16:9 window.
//!
//! Card HUDW's: the weapon panel of a clone trooper on BF2 Kamino — the
//! rifle, then the pistol (the infinity ring alone, no count), then the rifle
//! again — lands where the PS2 frames
//! `engine/artifacts/lead_tmp/hudw/ps2/{s2_rifle,s4_r2_once_later,s5_r2_twice}.png`
//! have it, and a BF1 soldier's grenade row sits in its own pill.
//!
//! Skips without `engine/assets_baked/common_bf2/hud` (run `bake common`).

use game::hud_feed::HudView;

fn hot1_post_a() -> ui::hud::HudModel {
    ui::hud::HudModel {
        team_names: ["all".into(), "imp".into()],
        tickets: [147, 145],
        hp: 300.0,
        hp_max: 300.0,
        energy: 100.0,
        energy_max: 100.0,
        weapon: Some(ui::hud::HudWeapon {
            key: "all_weap_inf_rifle".into(),
            class: String::new(),
            icon: None,
            name: String::new(),
            total: Some(250),
            clip_fraction: 1.0,
            bf2: true,
        }),
        reticle: Some(0),
        reticle_y: game::walk::ps2_reticle_y("bf2"),
        ..Default::default()
    }
}

/// The bake: `OPENBF_ASSETS`, else `engine/assets_baked`.
fn baked() -> std::path::PathBuf {
    std::env::var_os("OPENBF_ASSETS")
        .map_or_else(game::assets::baked_root, std::path::PathBuf::from)
}

fn view(size: [u32; 2]) -> Option<HudView> {
    let root = baked();
    if !root.join("common_bf2/hud/1playerhud.ron").is_file() {
        eprintln!("skip: no baked BF2 HUD");
        return None;
    }
    let mut v = HudView::load(&root, render::HEADLESS_FORMAT, None).expect("hud bake loads");
    v.update(&hot1_post_a(), 0.0, size);
    Some(v)
}

/// HOT1's posts 1 and 2 by instance name, and the pair 3 + 4, whose icons
/// touch on the rim and were measured as one blob. (The PS2's post 5, the
/// shield generator's `hud_dest_icon`, is not a post in the port.)
const POSTS: [&[&str]; 3] = [&["cp6"], &["cp3"], &["cp4", "cp5"]];

fn near(what: &str, got: f32, want: f32, tol: f32) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: {got:.1}, the PS2 frame has {want:.1} (± {tol})"
    );
}

#[test]
fn the_4_3_layout_is_the_ps2_frames() {
    let Some(v) = view([640, 480]) else { return };
    // Health fill: PS2 x 39–169, rows 406–413 (the card: x 38–170, rows
    // 405–414).
    let h = v.bounds("player1health_fill").expect("health fill drawn");
    println!("health fill {h:?}");
    near("health fill left", h[0], 39.0, 1.5);
    near("health fill right", h[2], 170.0, 1.5);
    near("health fill top", h[1], 406.0, 1.5);
    near("health fill bottom", h[3], 414.0, 2.0);
    // Reinforcement counts: PS2 centres x 172.5 / 466.5.
    for (name, want) in [
        ("player1teams_team1reinforcements", 172.5),
        ("player1teams_team2reinforcements", 466.5),
    ] {
        let b = v.bounds(name).expect("count drawn");
        println!("{name} {b:?}");
        near(name, (b[0] + b[2]) * 0.5, want, 2.0);
    }
    // Reticle outline: PS2 76 × 70.
    let r = v.bounds("player1reticule_outline").expect("reticle drawn");
    println!("reticle outline {r:?}");
    near("reticle width", r[2] - r[0], 76.0, 3.0);
    near("reticle height", r[3] - r[1], 70.0, 3.5);
    // The ammo count and the weapon on its backdrop are there.
    assert!(v.bounds("player1weapon1ammo").is_some());
    assert!(v.bounds("player1weapon1backdrop").is_some());
    assert!(v.bounds("player1weapon1mesh").is_some());
}

#[test]
fn a_16_9_window_anchors_each_group_to_its_edge() {
    let (Some(narrow), Some(wide)) = (view([960, 720]), view([1280, 720])) else {
        return;
    };
    let extra = 1280.0 - 960.0;
    // Left group: where 4:3 has it. Centre: half the extra width over.
    for (name, shift) in [
        ("player1health_fill", 0.0),
        ("player1weapon1ammo", 0.0),
        ("player1teams_team1reinforcements", extra * 0.5),
        ("player1teams_team2reinforcements", extra * 0.5),
    ] {
        let (a, b) = (narrow.bounds(name).unwrap(), wide.bounds(name).unwrap());
        near(name, b[0] - a[0], shift, 0.01);
        // Nothing stretched: the same size at the same height.
        near(name, b[2] - b[0], a[2] - a[0], 0.01);
        near(name, b[1], a[1], 0.01);
    }
    // The reticle sits on the aim ray (card AIMA): the middle column, and
    // the PS2's line 153.35 of 480 (BF2) as a fraction of any height.
    let frac = game::walk::ps2_reticle_y("bf2");
    near("reticle fraction x 480", frac * 480.0, 153.35, 0.5);
    near(
        "BF1's reticle line",
        game::walk::ps2_reticle_y("bf1") * 480.0,
        178.5,
        0.5,
    );
    let r = narrow.bounds("player1reticule_bullseye").unwrap();
    near("reticle centre x, 4:3", (r[0] + r[2]) * 0.5, 480.0, 0.5);
    near(
        "reticle centre y, 4:3",
        (r[1] + r[3]) * 0.5,
        frac * 720.0,
        0.5,
    );
    let r = wide.bounds("player1reticule_bullseye").unwrap();
    near("reticle centre x", (r[0] + r[2]) * 0.5, 640.0, 0.5);
    near("reticle centre y", (r[1] + r[3]) * 0.5, frac * 720.0, 0.5);
    println!(
        "reticle anchor: line {:.2} of 480 (BF2), {:.2} (BF1); 1280x720 bullseye centre y {:.2}",
        frac * 480.0,
        game::walk::ps2_reticle_y("bf1") * 480.0,
        (r[1] + r[3]) * 0.5
    );
}

/// Card EVMSG: "Ben Pulastra killed Player 1" at 640×480 sits where the PS2
/// frame has it — x 49–359, y 90–98 (`re-event-messages.md` §2 [M]) — within
/// 4 px, and a second line one line pitch (≈ 18 px) under it.
#[test]
fn the_kill_line_is_where_the_ps2_frame_has_it() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let line = |text: &str| ui::hud::HudMessage {
        text: text.into(),
        color: ui::hud::message_color::OWN,
    };
    let said = ui::hud::HudModel {
        messages: vec![line("Ben Pulastra killed Player 1")],
        ..hot1_post_a()
    };
    v.update(&said, 0.0, [640, 480]);
    assert!(
        v.bounds("player1systemMessages").is_none(),
        "not admitted yet"
    );
    // A second of frames: the line is admitted and fully shown.
    for _ in 0..30 {
        v.update(&hot1_post_a(), 1.0 / 30.0, [640, 480]);
    }
    let b = v
        .bounds("player1systemMessages")
        .expect("the line is drawn");
    println!("kill line {b:?}");
    near("line left", b[0], 49.0, 4.0);
    near("line right", b[2], 359.0, 4.0);
    near("line top", b[1], 90.0, 4.0);
    near("line bottom", b[3], 98.0, 4.0);

    // A second message goes under the first.
    let said = ui::hud::HudModel {
        messages: vec![line("Player 1 died")],
        ..hot1_post_a()
    };
    v.update(&said, 1.0 / 30.0, [640, 480]);
    for _ in 0..30 {
        v.update(&hot1_post_a(), 1.0 / 30.0, [640, 480]);
    }
    let two = v.bounds("player1systemMessages").expect("two lines");
    println!("two lines {two:?}");
    near("second line bottom", two[3] - b[3], 18.0, 1.0);
    // Both gone again after their time.
    for _ in 0..300 {
        v.update(&hot1_post_a(), 1.0 / 30.0, [640, 480]);
    }
    assert!(v.bounds("player1systemMessages").is_none());
}

/// Card EVMSG: a kill opens the statistics pop-up — the kills icon at about
/// (72, 227) and the points star at about (72, 256) of the PS2's 640×480
/// frame (`re-event-messages.md` §5 [M]), the numbers and "+1" beside them —
/// and it is gone again 6 s later (FadeHoldTime 5 + FadeOutTime 1).
#[test]
fn a_kill_opens_the_statistics_popup_where_the_ps2_has_it() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let quiet = ui::hud::HudModel {
        stats: Some(ui::hud::HudStats::default()),
        ..hot1_post_a()
    };
    // The battle's first frame publishes the zeros (the PS2 does too) and
    // the pop-up of that has run out by the time this view is settled.
    let mut settled =
        HudView::load(&baked(), render::HEADLESS_FORMAT, None).expect("hud bake loads");
    settled.update(&quiet, 0.0, [640, 480]);
    assert!(settled.bounds("player1statskills.icon").is_none());
    v.update(&quiet, 0.0, [640, 480]);
    v.update(&quiet, 7.0, [640, 480]);
    assert!(v.bounds("player1statskills.icon").is_none());

    let scored = ui::hud::HudModel {
        stats: Some(ui::hud::HudStats {
            points: 1,
            kills: 1,
            ..Default::default()
        }),
        ..hot1_post_a()
    };
    v.update(&scored, 1.0 / 30.0, [640, 480]);
    let centre = |b: [f32; 4]| ((b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5);
    let kills = centre(v.bounds("player1statskills.icon").expect("kills icon"));
    let star = centre(v.bounds("player1statspoints.icon").expect("points icon"));
    println!("kills icon {kills:?}, points icon {star:?}");
    near("kills icon x", kills.0, 72.0, 4.0);
    near("kills icon y", kills.1, 227.0, 4.0);
    near("points icon x", star.0, 72.0, 4.0);
    near("points icon y", star.1, 256.0, 4.0);
    for name in [
        "player1statskills",
        "player1statskillsDelta",
        "player1statspoints",
        "player1statspointsDelta",
    ] {
        assert!(v.bounds(name).is_some(), "{name} drawn");
    }
    // Held 5 s, faded over 1 s.
    v.update(&scored, 4.9, [640, 480]);
    assert!(v.bounds("player1statskills.icon").is_some());
    v.update(&scored, 1.2, [640, 480]);
    assert!(v.bounds("player1statskills.icon").is_none());
}

/// Card BMAP's acceptance (`re-hud-map.md` §7): the port's own baked HOT1
/// posts through the Map element land within 3 px of the icon centres
/// measured in two PS2 frames — `bf2_hot1_cp1_cp6_postA_spawn` (eye
/// (1.484, −10.097), θ 150.1°) and the walkabout's cp1 b000 (eye
/// (16.307, −15.772), back (−0.0084, ·, 0.9999)) — and a 16:9 window keeps
/// them at the same offset from its right edge.
#[test]
fn the_minimap_puts_hot1s_posts_where_the_ps2_frames_have_them() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let dir = baked().join("bf2_hot1");
    if !dir.join("manifest.ron").is_file() {
        eprintln!("skip: no baked bf2_hot1");
        return;
    }
    let mut scene = game::assets::load_scene(&dir).expect("scene");
    scene.apply_mode_layer("con");
    let layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let post = |n: &str| {
        let cp = layout
            .cps
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(n))
            .unwrap_or_else(|| {
                panic!(
                    "no post {n}: {:?}",
                    layout.cps.iter().map(|c| &c.name).collect::<Vec<_>>()
                )
            });
        [cp.pos.x, cp.pos.z]
    };
    // (eye x, z, θ), then (post, the PS2 frame's icon centre).
    type Pose = ((f32, f32, f32), [(&'static [&'static str], [f32; 2]); 3]);
    let poses: [Pose; 2] = [
        (
            (1.484, -10.097, 150.1_f32.to_radians()),
            [
                (POSTS[0], [534.4, 79.8]),
                (POSTS[1], [606.7, 70.7]),
                (POSTS[2], [595.9, 113.7]),
            ],
        ),
        (
            (16.307, -15.772, (0.0084_f32).atan2(0.9999)),
            [
                (POSTS[0], [552.2, 83.0]),
                (POSTS[1], [510.9, 116.2]),
                (POSTS[2], [496.5, 76.1]),
            ],
        ),
    ];
    for ((x, z, theta), want) in poses {
        let model = ui::hud::HudModel {
            map: Some(game::minimap::still(
                &scene,
                glam::Vec3::new(x, 0.0, z),
                -theta,
            )),
            ..hot1_post_a()
        };
        v.update(&model, 0.0, [640, 480]);
        let at = |v: &HudView, names: &[&str], size: [u32; 2]| {
            let sum = names.iter().fold([0.0; 2], |a, n| {
                let p = v.map_point(post(n), true, size).expect("a post clamps");
                [a[0] + p[0], a[1] + p[1]]
            });
            [sum[0] / names.len() as f32, sum[1] / names.len() as f32]
        };
        let narrow: Vec<[f32; 2]> = want
            .iter()
            .map(|(n, w)| {
                let got = at(&v, n, [640, 480]);
                println!(
                    "{n:?} θ {:.1}: port ({:.1}, {:.1}), PS2 ({:.1}, {:.1})",
                    theta.to_degrees(),
                    got[0],
                    got[1],
                    w[0],
                    w[1]
                );
                near(&format!("{n:?} x"), got[0], w[0], 3.0);
                near(&format!("{n:?} y"), got[1], w[1], 3.0);
                got
            })
            .collect();
        // 16:9 at the same height: the same place, measured from the right.
        v.update(&model, 0.0, [1280, 720]);
        for ((n, _), p) in want.iter().zip(narrow) {
            let wide = at(&v, n, [1280, 720]);
            near(
                &format!("{n:?} from the right"),
                1280.0 - wide[0],
                (640.0 - p[0]) * 1.5,
                0.01,
            );
            near(&format!("{n:?} down"), wide[1], p[1] * 1.5, 0.01);
        }
    }
}

/// The cross `seconds` after a press of arm `arm`.
fn cross(arm: u8, count: Option<u32>, seconds: f32) -> Option<ui::hud_squad::SquadCross> {
    Some(ui::hud_squad::SquadCross {
        labels: ["MOVE OUT", "HOLD POSITION", "FOLLOW ME", "AT EASE"].map(str::to_owned),
        arm,
        count,
        timer: ui::hud_squad::SquadCross::timer_at(seconds)?,
    })
}

fn squad_view(size: [u32; 2], sq: Option<ui::hud_squad::SquadCross>) -> Option<HudView> {
    let mut v = view(size)?;
    if !baked()
        .join("common_bf1/fonts/bf1_gamefont_tiny.ron")
        .is_file()
    {
        eprintln!("skip: no baked BF1 font (rebake common)");
        return None;
    }
    let mut m = hot1_post_a();
    m.squad = sq;
    v.update(&m, 0.0, size);
    Some(v)
}

/// The picture of the PS2 frames is 824 x 618 window pixels of the 512 x 448
/// frame; a 640 x 480 window is that picture at 640 / 824.
const PS2_PICTURE: f32 = 640.0 / 824.0;

/// Card SQHUD acceptance 1 and 2: at 640 x 480 the cross, the chosen arm's
/// highlight and its label land within 2 PS2 pixels (2.5 / 2.1 of this
/// window) of where the PS2 frames F3 (FOLLOW ME), H3 (HOLD POSITION) and A3
/// (AT EASE) have them; MOVE OUT's label is F3's dim one.
#[test]
fn the_squad_cross_is_where_the_ps2_frames_have_it() {
    use ui::hud_squad::{Part, DOWN, LEFT, RIGHT, UP};
    let size = [640, 480];
    let (tx, ty) = (2.5, 2.2);
    // The sheets' opaque texels: the cross's 3..=124 of 128 both ways, the
    // highlight's 8..=54 of 64 across its arm.
    let inset = |b: [f32; 4], fx: f32, fy: f32| {
        let (dx, dy) = ((b[2] - b[0]) * fx, (b[3] - b[1]) * fy);
        [b[0] + dx, b[1] + dy, b[2] - dx, b[3] - dy]
    };
    let ps2 = |b: [f32; 4]| b.map(|p| p * PS2_PICTURE);
    let check = |what: &str, got: [f32; 4], want: [f32; 4]| {
        println!("{what}: port {got:.1?}  PS2 {want:.1?}");
        for (k, tol) in [(0, tx), (1, ty), (2, tx), (3, ty)] {
            near(&format!("{what} [{k}]"), got[k], want[k], tol);
        }
    };
    // (arm, the PS2's highlight box, its label box) in picture pixels, the
    // boxes' far edges one past the last lit pixel.
    let frames = [
        (
            "F3 FOLLOW ME",
            RIGHT,
            [588.0, 513.0, 611.0, 528.0],
            [628.0, 516.0, 772.0, 533.0],
        ),
        (
            "H3 HOLD POSITION",
            DOWN,
            [577.0, 520.0, 595.0, 542.0],
            [490.0, 563.0, 676.0, 580.0],
        ),
        (
            "A3 AT EASE",
            LEFT,
            [561.0, 512.0, 585.0, 527.0],
            [438.0, 516.0, 538.0, 533.0],
        ),
    ];
    for (name, arm, highlight, label) in frames {
        let Some(v) = squad_view(size, cross(arm, None, 0.6)) else {
            return;
        };
        let (c, a) = v.squad_part(Part::Cross).expect("the cross");
        assert!((a - 178.0 / 255.0).abs() < 1.0e-6, "timer 0.7: {a}");
        check(
            &format!("{name} cross"),
            inset(c, 3.0 / 128.0, 3.0 / 128.0),
            ps2([561.0, 500.0, 610.5, 539.5]),
        );
        let (h, _) = v.squad_part(Part::Highlight).expect("the highlight");
        let across = 8.5 / 64.0;
        let h = if arm == DOWN {
            inset(h, across, 0.0)
        } else {
            inset(h, 0.0, across)
        };
        check(&format!("{name} highlight"), h, ps2(highlight));
        // The glyph boxes hold one empty row under the capitals.
        let (mut l, la) = v.squad_part(Part::Label(arm)).expect("the label");
        assert_eq!(la, a, "the chosen label is the group's alpha");
        l[3] -= 480.0 / 448.0 * ui::hud_squad::Y_UNIT;
        check(&format!("{name} label"), l, ps2(label));
        let (dim, da) = v
            .squad_part(Part::Label(if arm == UP { DOWN } else { UP }))
            .expect("a dim label");
        assert!((da - a * 127.0 / 255.0).abs() < 1.0e-6);
        if arm == RIGHT {
            let mut dim = dim;
            dim[3] -= 480.0 / 448.0 * ui::hud_squad::Y_UNIT;
            check("F3 MOVE OUT label", dim, ps2([519.0, 469.0, 642.0, 486.0]));
        }
        assert!(v.squad_part(Part::Count).is_none(), "nobody in reach");
    }
}

/// Acceptance 1: the fade at 0, 1 and 2 s, and the 16:9 anchor.
#[test]
fn the_squad_cross_fades_in_two_seconds_and_keeps_the_right_edge() {
    use ui::hud_squad::{Part, RIGHT, UP};
    for (seconds, group) in [(0.0, 255.0), (1.0, 127.0)] {
        let Some(v) = squad_view([640, 480], cross(RIGHT, Some(2), seconds)) else {
            return;
        };
        let alpha = |p| v.squad_part(p).expect("drawn").1 * 255.0;
        println!(
            "t {seconds}: cross {:.0} chosen {:.0} count {:.0} other {:.1}",
            alpha(Part::Cross),
            alpha(Part::Label(RIGHT)),
            alpha(Part::Count),
            alpha(Part::Label(UP))
        );
        assert_eq!(alpha(Part::Cross), group);
        assert_eq!(alpha(Part::Highlight), group);
        assert_eq!(alpha(Part::Label(RIGHT)), group);
        assert_eq!(alpha(Part::Count), group);
        near(
            "dim label",
            alpha(Part::Label(UP)),
            group * 127.0 / 255.0,
            0.01,
        );
    }
    let v = squad_view([640, 480], cross(RIGHT, Some(2), 2.0)).unwrap();
    assert!(v.squad_part(Part::Cross).is_none(), "t 2.0: timer 0");
    assert!(cross(RIGHT, None, 2.1).is_none(), "hidden after 2 s");

    // 16:9: every part keeps its 4:3 offset from the right edge.
    let narrow = squad_view([960, 720], cross(RIGHT, Some(2), 0.0)).unwrap();
    let wide = squad_view([1280, 720], cross(RIGHT, Some(2), 0.0)).unwrap();
    for p in [
        Part::Cross,
        Part::Highlight,
        Part::Label(RIGHT),
        Part::Label(UP),
        Part::Count,
    ] {
        let (n, w) = (
            narrow.squad_part(p).unwrap().0,
            wide.squad_part(p).unwrap().0,
        );
        println!("{p:?}: 4:3 {n:.1?}  16:9 {w:.1?}");
        for k in 0..4 {
            let shift = if k % 2 == 0 { 320.0 } else { 0.0 };
            near(&format!("{p:?} [{k}]"), w[k], n[k] + shift, 0.01);
        }
    }
    let c = wide.squad_part(Part::Count).unwrap().0;
    assert!(c[2] <= 1280.0, "the count stays on screen: {c:?}");
}

fn weapon(key: &str, class: &str, icon: Option<&str>, total: Option<u32>) -> ui::hud::HudWeapon {
    ui::hud::HudWeapon {
        key: key.into(),
        class: class.into(),
        icon: icon.map(str::to_owned),
        name: String::new(),
        total,
        clip_fraction: 1.0,
        bf2: false,
    }
}

/// `v` after `model` has been up for a second (past the rows' fades).
fn shown(v: &mut HudView, model: &ui::hud::HudModel) {
    v.update(model, 0.0, [640, 480]);
    v.update(model, 1.0, [640, 480]);
}

#[test]
fn the_weapon_panel_is_the_ps2_frames_on_kamino() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let grenades = Some(weapon(
        "rep_weap_inf_thermaldetonator",
        "rep_weap_inf_thermaldetonator",
        None,
        Some(4),
    ));
    let rifle = ui::hud::HudModel {
        weapon: Some(weapon(
            "rep_weap_inf_rifle",
            "rep_weap_inf_rifle",
            None,
            Some(200),
        )),
        weapon2: grenades.clone(),
        ..hot1_post_a()
    };
    let pistol = ui::hud::HudModel {
        weapon: Some(weapon(
            "rep_weap_inf_pistol",
            "rep_weap_inf_pistol",
            None,
            None,
        )),
        ..rifle.clone()
    };
    // The rifle (s2_rifle.png): x 93–224, rows 377–397; "200" beside it.
    let the_rifle = |v: &HudView| {
        let b = v.bounds("player1weapon1mesh").expect("rifle drawn");
        println!("rifle {b:?}");
        near("rifle left", b[0], 93.0, 3.0);
        near("rifle right", b[2], 224.0, 3.0);
        near("rifle top", b[1], 377.0, 3.0);
        near("rifle bottom", b[3], 397.0, 3.0);
        assert!(v.bounds("player1weapon1ammo").is_some(), "the count");
        assert!(
            v.bounds("player1weapon1ammoInfinite").is_none(),
            "no infinity ring beside a count"
        );
    };
    shown(&mut v, &rifle);
    the_rifle(&v);
    // The pistol (s4_r2_once_later.png): the ring x 36–65, rows 373–399,
    // and NO count; the gun x 78–175, rows 374–401 (sight to grip).
    shown(&mut v, &pistol);
    let ring = v
        .bounds("player1weapon1ammoInfinite")
        .expect("the infinity ring");
    println!("ring {ring:?}");
    near("ring left", ring[0], 36.0, 2.5);
    near("ring right", ring[2], 65.0, 2.5);
    near("ring top", ring[1], 373.0, 2.5);
    near("ring bottom", ring[3], 399.0, 2.5);
    assert!(
        v.bounds("player1weapon1ammo").is_none(),
        "an infinite weapon shows no count (the PS2 prints an empty string)"
    );
    let gun = v.bounds("player1weapon1mesh");
    println!("pistol {gun:?}");
    let baked = baked()
        .join("side_bf2/weapons/rep_weap_inf_pistol.glb")
        .is_file();
    match gun {
        Some(b) => {
            assert!(baked, "the rifle's mesh stayed up under the pistol: {b:?}");
            near("pistol left", b[0], 78.0, 3.0);
            near("pistol right", b[2], 175.0, 3.0);
            near("pistol top", b[1], 374.0, 3.0);
            near("pistol bottom", b[3], 401.0, 3.0);
        }
        // A bake from before the card: nothing in the rifle's place.
        None => assert!(!baked, "the baked pistol was not drawn"),
    }
    assert!(v.bounds("player1weapon1backdrop").is_some(), "the pill");
    // And back (s5_r2_twice.png): the count returns, the ring goes.
    shown(&mut v, &rifle);
    the_rifle(&v);
}

#[test]
fn a_bf1_soldiers_grenade_row_sits_in_its_own_pill() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let health = v.bounds("player1health_fill").expect("health fill");
    // BF2's own row: the thermal detonator's HUD mesh.
    let bf2 = ui::hud::HudModel {
        weapon2: Some(weapon(
            "imp_weap_inf_thermaldetonator",
            "imp_weap_inf_thermaldetonator",
            None,
            Some(4),
        )),
        ..hot1_post_a()
    };
    shown(&mut v, &bf2);
    let want = v.bounds("player1weapon2mesh").expect("BF2's grenade");
    // BF1's stormtrooper: the same class name with its own geometry and an
    // icon sheet (the maintainer's blob over the health bar, 2026-10-04).
    let rifle = Some(weapon(
        "imp_weap_inf_rifle",
        "imp_weap_inf_rifle",
        Some("imp_blasterrifle_icon"),
        Some(250),
    ));
    let bf1 = ui::hud::HudModel {
        weapon: rifle.clone(),
        weapon2: Some(weapon(
            "com_weap_inf_grenadethermal",
            "imp_weap_inf_thermaldetonator",
            Some("hud_com_grenade_icon"),
            Some(4),
        )),
        ..hot1_post_a()
    };
    let mut v = view([640, 480]).expect("bake");
    shown(&mut v, &bf1);
    let got = v.bounds("player1weapon2mesh").expect("BF1's grenade");
    println!("grenade: BF2 {want:?}, BF1 {got:?}");
    for k in 0..4 {
        near("BF1 thermal detonator = BF2's", got[k], want[k], 0.5);
    }
    // A grenade BF2 has no mesh for: its icon sheet, in the grenade's place.
    let crack = ui::hud::HudModel {
        weapon: rifle,
        weapon2: Some(weapon(
            "com_weap_inf_grenadethermal",
            "imp_weap_inf_crackgrenade",
            Some("hud_crack_grenade_icon"),
            Some(2),
        )),
        ..hot1_post_a()
    };
    let mut v = view([640, 480]).expect("bake");
    shown(&mut v, &crack);
    let icon = v.bounds("player1weapon2mesh").expect("the icon sheet");
    println!("crack grenade icon {icon:?}, health fill {health:?}");
    assert!(
        icon[3] < want[3] + 4.0 && icon[1] > want[1] - 4.0,
        "the icon {icon:?} is in the grenade's rows {want:?}"
    );
    near(
        "icon centre x",
        (icon[0] + icon[2]) * 0.5,
        (want[0] + want[2]) * 0.5,
        4.0,
    );
    assert!(icon[3] < health[1] - 20.0, "clear of the health bar");
}

/// Issue #33 (`analysis/re-round-end.md` §1.2): the BF2 layout's
/// domination countdown. While the viewer's side holds every post the
/// VictoryTimerGroup shows "%m:%02s" of the seconds left; the other side
/// sees the DefeatTimerGroup; when it ends both are hidden.
#[test]
fn the_domination_countdown_shows_victory_in_or_defeat_in() {
    let common = game::assets::baked_root().join("common_bf2/hud");
    if !common.join("1playerhud.ron").exists() {
        eprintln!("SKIP: no baked BF2 HUD");
        return;
    }
    let read = |f: &str| -> manifest::hud::HudLayout {
        ron::from_str(&std::fs::read_to_string(common.join(f)).unwrap()).unwrap()
    };
    let mut hud = ui::hud_tree::Hud::new(&read("1playerhud.ron"), &read("hudtransforms.ron"));
    // The labels over the numbers, from BF2's text table.
    let locl: manifest::hud::HudLocl =
        ron::from_str(&std::fs::read_to_string(common.join("../locl/english.ron")).unwrap())
            .unwrap();
    hud.set_texts(&|key| locl.get(manifest::hud::locl_key(key)).map(str::to_owned));
    let (win, lose) = (
        hud.find("victorytimer").unwrap(),
        hud.find("defeattimer").unwrap(),
    );
    let label = |name: &str| hud.elems[hud.find(name).unwrap()].text.to_ascii_uppercase();
    assert_eq!(
        [label("victorytext"), label("defeattext")],
        ["VICTORY IN", "DEFEAT IN"]
    );
    ui::hud::place_countdown_labels(&mut hud);
    let pos = |name: &str| hud.elems[hud.find(name).unwrap()].pos;
    assert_eq!(
        pos("defeattext"),
        pos("victorytext"),
        "both labels over the number"
    );
    let shown = |hud: &ui::hud_tree::Hud, i: usize| hud.elems[i].fader.state != 0;
    let mut m = ui::hud::HudModel {
        countdown: Some((true, 17.4)),
        ..Default::default()
    };
    let step = |hud: &mut ui::hud_tree::Hud, m: &ui::hud::HudModel| {
        ui::hud::feed(hud, m, &|_| false);
        hud.advance(1.0 / 30.0);
    };
    step(&mut hud, &m);
    assert_eq!(hud.elems[win].text, "0:17");
    assert!(shown(&hud, win) && !shown(&hud, lose));
    m.countdown = Some((false, 9.0));
    for _ in 0..30 {
        step(&mut hud, &m);
    }
    assert_eq!(hud.elems[lose].text, "0:09");
    assert!(
        shown(&hud, lose) && !shown(&hud, win),
        "the other side's is hidden"
    );
    m.countdown = None;
    for _ in 0..60 {
        step(&mut hud, &m);
    }
    assert!(
        !shown(&hud, win) && !shown(&hud, lose),
        "both hidden when it ends"
    );
}

/// Every `<game>_*.ron` side file of the bake in one class db.
fn side_db(game: &str) -> game::classdb::ClassDb {
    let mut files: Vec<_> = std::fs::read_dir(baked().join("classes"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{game}_")))
        })
        .collect();
    files.sort();
    let mut db = game::classdb::ClassDb::default();
    for f in &files {
        db.load(f).unwrap();
    }
    db
}

/// The HUD row a soldier's weapon class sends (`game::roster::weapon_hud`).
fn hud_row(db: &game::classdb::ClassDb, class: &str) -> ui::hud::HudWeapon {
    game::hud_feed::hud_weapon(&game::roster::weapon_hud(db, class), Some(1), 1.0)
}

/// The maintainer's report (2026-10-29): the Republic engineer's detpack
/// showed the time bomb's model. BF2 sends the detpack's `HUDTag`
/// `com_weap_inf_detpack` (`re-hud.md` §3 [V][O]); `hudtransforms` has no
/// entry for it, so the PS2 falls back to the model of that name, which
/// 1playerhud places in `player1weapon2mesh`. The table's
/// `rep_weap_inf_detpack -> com_weap_inf_seismic_mine` row keys a class
/// name the PS2 never sends for this class. The time bombs (`HUDTag`
/// `com_weap_inf_seismic_mine`) keep the seismic mine.
#[test]
fn the_detpack_shows_its_own_model_and_the_time_bomb_the_seismic_mine() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let db = side_db("bf2");
    for side in ["rep", "cis", "imp", "all"] {
        for (weapon, want) in [
            ("detpack", "com_weap_inf_detpack"),
            ("timebomb", "com_weap_inf_seismic_mine"),
        ] {
            let class = format!("{side}_weap_inf_{weapon}");
            if db.get(&class).is_none() {
                continue;
            }
            let row = hud_row(&db, &class);
            let (key, mesh) = v.weapon_mesh(2, &row);
            println!(
                "{class}: sends {:?}, table key {key}, model {mesh:?}",
                row.key
            );
            assert_eq!(mesh.as_deref(), Some(want), "{class}");
            let model = ui::hud::HudModel {
                weapon2: Some(row),
                ..hot1_post_a()
            };
            shown(&mut v, &model);
            assert!(v.bounds("player1weapon2mesh").is_some(), "{class} drawn");
        }
    }
    // The same rule elsewhere: the TIE bomber's torpedo sends
    // `hud_vehicle_recoiless` (a model), not its class's table row; the
    // officer's defense buff sends `hud_rep_defense_buff`, a REP.LVL model
    // the bake lacks, so it keeps its class's row (`[ours]`).
    for (row, class, want) in [
        (
            1,
            "imp_weap_fly_tiebomber_ptorpedo",
            "hud_vehicle_recoiless",
        ),
        (
            2,
            "rep_weap_inf_buff_defense",
            "hud_rep_weap_defense_buff_icon",
        ),
    ] {
        let mesh = v.weapon_mesh(row, &hud_row(&db, class)).1;
        assert_eq!(mesh.as_deref(), Some(want), "{class}");
    }
    // BF1's one bomb, the rebels' remote charge: its own geometry, the
    // seismic mine, which BF2's layout places.
    let bf1 = side_db("bf1");
    let row = hud_row(&bf1, "all_weap_inf_remote_charge_dispenser");
    assert_eq!(
        v.weapon_mesh(2, &row).1.as_deref(),
        Some("com_weap_inf_seismic_mine")
    );
}

/// Census (run by hand, `--ignored --nocapture`): for every weapon class
/// of each game's side files and both HUD rows, the model the port shows
/// against the PS2's rule on the class's own key (`hudtransforms[key]`,
/// else the model named `key`; `re-hud.md` §2.5, §3), and against the old
/// rule that read the table with the class name whenever the key had no
/// entry. A BF2 weapon the port shows another weapon's model for is a bug;
/// a BF1 one is the `[ours]` class key, listed to be read.
#[test]
#[ignore]
fn census_weapons_shown_with_another_weapons_hud_model() {
    let Some(v) = view([640, 480]) else {
        return;
    };
    for game in ["bf2", "bf1"] {
        let db = side_db(game);
        let mut names: Vec<_> = db
            .names()
            .filter(|n| db.get(n).is_some_and(|d| d.kind == "wpnc"))
            .map(str::to_owned)
            .collect();
        names.sort();
        let (mut old_off, mut now_off) = (0, 0);
        for row in [1u8, 2] {
            for n in &names {
                let w = hud_row(&db, n);
                let own = ui::hud::HudWeapon {
                    class: String::new(),
                    bf2: true,
                    icon: None,
                    ..w.clone()
                };
                let old = ui::hud::HudWeapon {
                    class: n.clone(),
                    bf2: false,
                    ..w.clone()
                };
                let ps2 = v.weapon_mesh(row, &own).1;
                let (now_key, now) = v.weapon_mesh(row, &w);
                let was = v.weapon_mesh(row, &old).1;
                if was != ps2 && was.is_some() {
                    old_off += 1;
                    println!(
                        "{game} row{row} {n}: sends {}, own model {ps2:?}, old rule {was:?}",
                        w.key
                    );
                }
                if now != ps2 && now.is_some() {
                    now_off += 1;
                    println!(
                        "{game} row{row} {n}: sends {}, own model {ps2:?}, NOW {now:?} (table key {now_key})",
                        w.key
                    );
                }
            }
        }
        println!("{game}: {old_off} rows off under the old rule, {now_off} now");
    }
}

fn gauge_model(ring: f32, refire: f32, overheated: bool) -> ui::hud::HudModel {
    ui::hud::HudModel {
        gauge: ui::hud::HudGauge {
            ring,
            refire,
            overheated,
        },
        ..hot1_post_a()
    }
}

/// The lit segments the layout's segmented bar shows for its value, as
/// `hud_draw::segmented` draws them: k of n while k / n < value.
fn lit(hud: &ui::hud_tree::Hud, name: &str) -> usize {
    let e = &hud.elems[hud.find(name).unwrap_or_else(|| panic!("no {name}"))];
    (0..e.segments)
        .take_while(|k| (*k as f32 / e.segments as f32) < e.value - 1.0e-4)
        .count()
}

/// Issue #78 on BF2's own layout (`analysis/re-heat-hud.md` 1, 2): the 30-segment
/// ring lights with the value, cold lighting none, and its colour runs by the
/// layout's own table from yellow to orange. Overheated, the line comes up and
/// the two 3-segment red bits show `refire`, and clear when the weapon does.
#[test]
fn the_heat_ring_and_its_overheat_signs_follow_the_gauge() {
    let common = baked().join("common_bf2/hud");
    if !common.join("1playerhud.ron").is_file() {
        eprintln!("SKIP: no baked BF2 HUD");
        return;
    }
    let read = |f: &str| -> manifest::hud::HudLayout {
        ron::from_str(&std::fs::read_to_string(common.join(f)).unwrap()).unwrap()
    };
    let mut hud = ui::hud_tree::Hud::new(&read("1playerhud.ron"), &read("hudtransforms.ron"));
    let at = |hud: &mut ui::hud_tree::Hud, ring, refire, overheated| {
        ui::hud::feed(hud, &gauge_model(ring, refire, overheated), &|_| false);
        hud.advance(0.05);
    };
    let ring = "player1weapon1heatbar";
    let i = hud.find(ring).expect("the ring");
    assert_eq!(hud.elems[i].segments, 30);
    for (value, want, color) in [
        (0.0, 0, [239.0, 255.0, 41.0]),
        (0.5, 15, [239.0, 174.0, 41.0]),
        (1.0, 30, [239.0, 106.0, 41.0]),
    ] {
        at(&mut hud, value, 0.0, false);
        assert_eq!(lit(&hud, ring), want, "ring at {value}");
        assert_eq!(hud.elems[i].color, color, "colour at {value}");
    }
    let line = hud.find("player1weapon1overheat").expect("the line");
    let bits = "player1weapon1refire_leftbit";
    assert_eq!(hud.elems[line].fader.state, 0, "off before it overheats");
    at(&mut hud, 1.0, 0.75, true);
    assert_ne!(hud.elems[line].fader.state, 0, "on while overheated");
    assert_eq!(lit(&hud, bits), 3, "1 - 0.25 / 1.0 of three");
    at(&mut hud, 0.5, 0.5, true);
    assert_eq!(lit(&hud, bits), 2, "1 - 0.25 / 0.5 of three");
    at(&mut hud, 0.2, 0.0, false);
    assert_eq!(lit(&hud, bits), 0, "cleared");
    for _ in 0..60 {
        at(&mut hud, 0.1, 0.0, false);
    }
    assert_eq!(hud.elems[line].fader.state, 0, "the line has faded");
}

/// The two red bits grow from 6 o'clock outward both ways, one the other's mirror
/// image round the reticle (`re-heat-hud.md` 1, 6), and the line reads "OVERHEATED!".
#[test]
fn the_refire_bits_meet_under_the_reticle_and_the_line_shows() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let hot = gauge_model(1.0, 0.75, true);
    v.update(&hot, 0.3, [640, 480]);
    let left = v.bounds("player1weapon1refire_leftbit").expect("left");
    let right = v.bounds("player1weapon1refire_rightbit").expect("right");
    println!("left {left:?} right {right:?}");
    near("the left bit's near end", left[2], 320.0, 8.0);
    near("the mirror's near end", right[0], 640.0 - left[2], 1.0);
    near("the mirror's far end", right[2], 640.0 - left[0], 1.0);
    near("the same height", right[1], left[1], 1.0);
    assert!(v.bounds("player1weapon1overheat").is_some(), "the line");
}

/// A cold or absent gauge draws nothing round the reticle, so the golden frames do
/// not move: the ring's own box stays empty until a value lights it.
#[test]
fn the_ring_draws_only_when_lit() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let ring = "player1weapon1heatbar";
    v.update(&hot1_post_a(), 1.0, [640, 480]);
    assert_eq!(v.bounds(ring), None, "cold");
    v.update(&gauge_model(0.5, 0.0, false), 1.0, [640, 480]);
    let b = v.bounds(ring).expect("half hot");
    println!("half hot: {b:?}");
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    assert!(w > 20.0 && h > 20.0, "a ring round the reticle: {b:?}");
    v.update(
        &ui::hud::HudModel {
            weapon: None,
            ..gauge_model(0.5, 0.0, false)
        },
        1.0,
        [640, 480],
    );
    assert_eq!(v.bounds(ring), None, "no weapon, no ring");
}

/// Issue #59 (`re-boundary.md` T4): "Leaving Battlefield" comes up at the screen's
/// centre while the flown droid counts down, pulses, and goes when it stops.
#[test]
fn the_leaving_line_shows_at_the_centre_and_pulses() {
    let Some(mut v) = view([640, 480]) else {
        return;
    };
    let name = ui::hud::LEAVING_ELEM;
    assert!(v.bounds(name).is_none(), "off inside the map");
    let leaving = ui::hud::HudModel {
        leaving: Some("LEAVING BATTLEFIELD".into()),
        ..hot1_post_a()
    };
    v.update(&leaving, 0.0, [640, 480]);
    let at = v.bounds(name).expect("the line");
    let (x, y) = ((at[0] + at[2]) * 0.5, (at[1] + at[3]) * 0.5);
    assert!(
        (x - 320.0).abs() < 8.0 && (y - 240.0).abs() < 20.0,
        "{at:?}"
    );
    v.update(&hot1_post_a(), 0.5, [640, 480]);
    assert!(v.bounds(name).is_none(), "back inside");
}
