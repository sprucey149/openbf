//! Campaign map and objective browser, matching the maintainer's original-game
//! comparison. The map uses the existing whole-map projection and owned art.
use super::*;

#[derive(Clone, Debug)]
pub struct Objective {
    pub text: String,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub objectives: Vec<Objective>,
    pub selected: usize,
    pub footer: [String; 3],
}

pub fn draw(hud: &Hud, art: &HudArt, model: &Model, size: [f32; 2]) -> HudFrame {
    let mut ctx = Ctx {
        hud,
        art,
        out: HudFrame::default(),
        clip: None,
    };
    ctx.spawn_veil(size);
    ctx.map_campaign(size);
    let Some(i) = hud.elems.iter().position(|e| e.name == "player1objectives") else {
        return ctx.out;
    };
    let scale = frame_scale(size);
    let screen = Screen {
        origin: [
            OVERFILL[0].0 * 4.0 / 3.0 * size[1] + anchor_shift(0.533743, size),
            OVERFILL[1].0 * size[1],
        ],
        scale,
    };
    let Some(font) = art.fonts.get("gamefont_small") else {
        return ctx.out;
    };
    let wrapped = |row: &Objective| {
        super::text::wrap(&row.text.to_uppercase(), 205.0, &|s| {
            run_width(font, s, 0.8, false)
        })
    };
    let pitch = f32::from(font.line_height) * 0.8;
    let mut y = 0.055098 * FRAME_H;
    let height = 0.674066 * FRAME_H;
    let mut first = model.selected.saturating_sub(7);
    while first < model.selected
        && model
            .objectives
            .iter()
            .skip(first)
            .take(model.selected - first + 1)
            .map(|row| wrapped(row).len() as f32 * pitch + 4.0)
            .sum::<f32>()
            > height
    {
        first += 1;
    }
    for (index, row) in model.objectives.iter().enumerate().skip(first).take(8) {
        let color = if index == model.selected {
            [223.0, 223.0, 32.0]
        } else if row.active {
            [220.0; 3]
        } else {
            [145.0; 3]
        };
        let lines = wrapped(row);
        if y + lines.len() as f32 * pitch > 0.055098 * FRAME_H + height {
            break;
        }
        for line in lines {
            ctx.token_run(
                i,
                &Mat4::IDENTITY,
                &screen,
                font,
                &Run {
                    text: &line,
                    at: (273.0, y),
                    h_align: HAlign::Left,
                    scale: [0.8; 2],
                    color: super::color(color, 1.0),
                    additive: false,
                    shadow: false,
                    tracking: 0.0,
                },
            );
            y += pitch;
        }
        y += 4.0;
    }
    for (k, footer) in model.footer.iter().enumerate() {
        ctx.token_run(
            i,
            &Mat4::IDENTITY,
            &screen,
            font,
            &Run {
                text: footer,
                at: (495.0, 327.0 + k as f32 * 28.0),
                h_align: HAlign::Right,
                scale: [0.85; 2],
                color: [0.88, 0.88, 0.75, 1.0],
                additive: false,
                shadow: false,
                tracking: 0.0,
            },
        );
    }
    ctx.out
}
