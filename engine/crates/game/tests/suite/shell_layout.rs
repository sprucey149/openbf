//! The BF2 shell's stacks against the PS2's frames (card BSHLA acceptance 1,
//! `analysis/re-bf2-shell.md` §3.2): the port's layout rule, fed the PS2's
//! own rows and the baked `gamefont_large`, drawn at 640 x 480 (1 unit =
//! 1.25 px across, 480/448 down) puts the bars where
//! `lead_tmp/re/bshl/ref_frames/` shows them. Needs the BF2 common bake.

use ui::shell_ps2::{stack_layout, RowId, Stack, StackRow, MAIN_Y_ADD};

fn font() -> Option<manifest::hud::HudFont> {
    let p = game::assets::baked_root().join("common_bf2/fonts/gamefont_large.ron");
    let text = std::fs::read_to_string(&p).ok()?;
    Some(ron::from_str(&text).expect("font"))
}

/// A stack of the PS2's labels (the row ids only name them).
fn stack(title: &str, labels: &[&str], left: bool, y_add: f32) -> Stack {
    Stack {
        title: title.into(),
        rows: labels
            .iter()
            .map(|l| StackRow {
                id: RowId::Options,
                label: (*l).into(),
                enabled: true,
            })
            .collect(),
        focus: 0,
        left,
        y_add,
        note: None,
    }
}

const PX_X: f32 = 640.0 / 512.0;
const PX_Y: f32 = 480.0 / 448.0;

#[test]
fn the_stacks_sit_where_the_ps2_draws_them() {
    let Some(font) = font() else {
        if !game::assets::baked_root().join("common_bf2").is_dir() {
            eprintln!("SKIP: no BF2 common bake");
            return;
        }
        panic!("stale common bake: no gamefont_large in the BF2 common bake");
    };
    let main = stack(
        "Main Menu",
        &["Singleplayer", "Multiplayer", "Splitscreen", "Options"],
        false,
        MAIN_Y_ADD,
    );
    let sp = stack(
        "Singleplayer",
        &[
            "Training",
            "Space Overview",
            "Rise of the Empire",
            "Galactic Conquest",
            "Instant Action",
        ],
        true,
        0.0,
    );
    let opts = stack(
        "Options",
        &[
            "Game Options",
            "Audio Options",
            "Controls",
            "Online Options",
            "View Trailer",
            "Credits",
            "Career Stats",
        ],
        true,
        0.0,
    );
    // (stack, bar left px, bar right px, title top px) from the frames
    // (`main_menu.png`, `singleplayer.png`, `options_top.png`).
    let cases = [
        ("main", &main, 143.75, 501.25, 206.0),
        ("singleplayer", &sp, 43.0, 479.5, 178.0),
        ("options", &opts, 43.0, 420.0, 143.5),
    ];
    for (name, s, left, right, top) in cases {
        let lay = stack_layout(&font, s);
        let [x0, y0, x1, _] = lay.title;
        let (l, r, t) = (x0 * PX_X, x1 * PX_X, y0 * PX_Y);
        println!("{name}: bar {l:.2} .. {r:.2} px, title top {t:.2} px");
        assert!((l - left).abs() <= 2.0, "{name} left {l} vs {left}");
        assert!((r - right).abs() <= 2.0, "{name} right {r} vs {right}");
        assert!((t - top).abs() <= 2.0, "{name} title top {t} vs {top}");
        // Every row the same bar, one pitch apart.
        for w in lay.rows.windows(2) {
            let pitch = (w[1][1] - w[0][1]) * PX_Y;
            assert!((pitch - 34.25).abs() <= 0.5, "{name} pitch {pitch}");
            assert_eq!([w[0][0], w[0][2]], [x0, x1]);
        }
    }
}
