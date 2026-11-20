//! BF2's final campaign screen opens its credits after the last victory.
//! See `ifs_campaign_end` and `analysis/re-bf2-menus.md` §3.

use std::path::Path;
use ui::egui::{self, Color32, Pos2, Rect};

const LINE_STEP: f32 = 22.0;
const SCROLL_SPEED: f32 = 40.0;
const FAR_DEPTH: f32 = 700.0;

#[derive(serde::Deserialize)]
struct Line {
    text: String,
    heading: bool,
}

pub(super) struct Crawl {
    lines: Vec<Line>,
    offset: f32,
    backdrop: Backdrop,
}

impl Crawl {
    pub(super) fn load(assets_dir: &Path) -> Option<Self> {
        let path = game::shell_movie::shell_dir(assets_dir).join("credits.json");
        let lines: Vec<Line> = match pak::read(&path)
            .map_err(anyhow::Error::from)
            .and_then(|data| Ok(serde_json::from_slice(&data)?))
        {
            Ok(lines) => lines,
            Err(error) => {
                eprintln!("campaign credits {}: {error:#}", path.display());
                return None;
            }
        };
        let texts = game::event_feed::Texts::load(assets_dir, "bf2");
        let lines = lines
            .into_iter()
            .map(|mut line| {
                if let Some(text) = texts.bf2_text(&line.text) {
                    line.text = text.to_owned();
                }
                line
            })
            .collect();
        Some(Self {
            lines,
            offset: 0.0,
            backdrop: Backdrop::load(assets_dir),
        })
    }

    pub(super) fn advance(&mut self, dt: f32) -> bool {
        self.offset += SCROLL_SPEED * dt;
        self.offset >= self.lines.len() as f32 * LINE_STEP + FAR_DEPTH
    }

    pub(super) fn draw(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let projection = CrawlProjection::new(screen);
        let scale = projection.scale;
        let painter = ctx.layer_painter(egui::LayerId::background());
        self.backdrop.draw(ctx, &painter, screen);
        let mut visible = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            let depth = self.offset - index as f32 * LINE_STEP;
            if line.text.is_empty() || !(-LINE_STEP..FAR_DEPTH).contains(&depth) {
                continue;
            }
            let (size, color) = if line.heading {
                (24.0, Color32::from_rgb(255, 230, 65))
            } else {
                (19.0, Color32::from_rgb(245, 242, 229))
            };
            let fade = ((FAR_DEPTH - depth) / 170.0).clamp(0.0, 1.0);
            let font = egui::FontId::proportional(size * scale);
            let galley =
                painter.layout_no_wrap(line.text.clone(), font, color.gamma_multiply(fade));
            visible.push((depth, galley));
        }
        // Layout may grow the font atlas. Read its size after all glyphs are placed.
        let [width, height] = ctx.fonts(|fonts| fonts.font_image_size());
        let mut lettering = egui::Mesh::default();
        for (depth, galley) in visible.into_iter().rev() {
            let max_width = screen.width() * 0.84;
            for row in &galley.rows {
                let fit = (max_width / row.size.x.max(1.0)).min(1.0);
                let start = lettering.vertices.len() as u32;
                lettering
                    .indices
                    .extend(row.visuals.mesh.indices.iter().map(|index| start + index));
                for vertex in &row.visuals.mesh.vertices {
                    let x = (vertex.pos.x - row.size.x * 0.5) * fit / scale;
                    let y = (row.pos.y + vertex.pos.y - galley.size().y * 0.5) / scale;
                    lettering.vertices.push(egui::epaint::Vertex {
                        pos: projection.point(x, depth - y),
                        uv: egui::pos2(vertex.uv.x / width as f32, vertex.uv.y / height as f32),
                        color: vertex.color,
                    });
                }
            }
        }
        painter.add(egui::Shape::mesh(lettering));
    }
}

struct CrawlProjection {
    anchor: Pos2,
    scale: f32,
    rise: f32,
    recession: f32,
}

impl CrawlProjection {
    fn new(screen: Rect) -> Self {
        // ifs_credits main pc 1981 tilts the line container 40 degrees.
        let (recession, rise) = 40.0_f32.to_radians().sin_cos();
        Self {
            anchor: screen.center_bottom(),
            scale: screen.height().max(1.0) / 480.0,
            rise,
            recession,
        }
    }

    fn point(&self, x: f32, depth: f32) -> Pos2 {
        let scale = self.scale / (1.0 + depth * self.recession / 750.0);
        self.anchor + egui::vec2(x * scale, -depth * self.rise * scale)
    }
}

struct Backdrop {
    stars: Vec<(f32, f32, f32, Color32)>,
    surface: Option<egui::ColorImage>,
    planet: Option<egui::TextureHandle>,
}

impl Backdrop {
    fn load(assets_dir: &Path) -> Self {
        let textures = assets_dir.join("bf2_gal1").join("textures");
        let surface = game::assets::load_texture(&textures, "gal_prp_kamino_hi_land")
            .ok()
            .filter(|t| t.rgba.len() == t.width as usize * t.height as usize * 4)
            .map(|mut t| {
                if let Ok(clouds) =
                    game::assets::load_texture(&textures, "gal_prp_kamino_hi_clouds")
                {
                    if (clouds.width, clouds.height) == (t.width, t.height) {
                        for (ground, cloud) in
                            t.rgba.chunks_exact_mut(4).zip(clouds.rgba.chunks_exact(4))
                        {
                            let alpha = u16::from(cloud[3]);
                            for channel in &mut ground[..3] {
                                *channel = ((u16::from(*channel) * (255 - alpha) + 245 * alpha)
                                    / 255) as u8;
                            }
                        }
                    }
                }
                egui::ColorImage::from_rgba_unmultiplied(
                    [t.width as usize, t.height as usize],
                    &t.rgba,
                )
            });
        let mut seed = 0x4352_4544_u32;
        let mut sample = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1 << 24) as f32
        };
        let stars = (0..1900)
            .map(|_| {
                let (x, y, light) = (sample(), sample(), sample().powi(5));
                let luminance = (34.0 + light * 175.0) as u8;
                let color = Color32::from_rgb(luminance, luminance, luminance.saturating_add(20));
                (x, y, 0.24 + light * 0.62, color)
            })
            .collect();
        Self {
            stars,
            surface,
            planet: None,
        }
    }

    fn draw(&mut self, ctx: &egui::Context, painter: &egui::Painter, screen: Rect) {
        painter.rect_filled(screen, 0.0, Color32::from_rgb(3, 6, 12));
        let scale = screen.height() / 480.0;
        for &(x, y, radius, color) in &self.stars {
            let position = screen.left_top() + egui::vec2(x * screen.width(), y * screen.height());
            painter.circle_filled(position, radius * scale, color);
        }
        if let Some(surface) = self.surface.take() {
            self.planet =
                Some(ctx.load_texture("credits_planet", surface, egui::TextureOptions::LINEAR));
        }
        if let Some(planet) = &self.planet {
            let radius = screen.width().max(screen.height()) * 0.92;
            let center = egui::pos2(
                screen.center().x + screen.width() * 0.07,
                screen.bottom() + radius - screen.height() * 0.26,
            );
            painter.add(egui::Shape::mesh(planet_mesh(planet.id(), center, radius)));
        }
    }
}

fn planet_mesh(texture: egui::TextureId, center: Pos2, radius: f32) -> egui::Mesh {
    const RINGS: u32 = 24;
    const SECTORS: u32 = 88;
    let mut mesh = egui::Mesh::with_texture(texture);
    for ring in 0..=RINGS {
        // Sample the spherical surface uniformly from its centre to its limb.
        let (r, z) = (ring as f32 / RINGS as f32 * std::f32::consts::FRAC_PI_2).sin_cos();
        for sector in 0..=SECTORS {
            let angle = sector as f32 / SECTORS as f32 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            let (x, y) = (cos * r, sin * r);
            let uv = egui::pos2(
                0.47 + x.atan2(z.max(0.00001)) / std::f32::consts::TAU,
                0.50 + y.asin() / std::f32::consts::PI,
            );
            let sunlight = (x * 0.54 - y * 0.64 + z * 0.55).max(0.0);
            let light = 0.13 + sunlight * 0.75;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: center + egui::vec2(x, y) * radius,
                uv,
                color: Color32::from_gray((255.0 * light) as u8),
            });
            if ring > 0 && sector > 0 {
                let b = ring * (SECTORS + 1) + sector;
                let a = b - SECTORS - 1;
                mesh.indices.extend([a - 1, a, b, a - 1, b, b - 1]);
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_crawl_draws_and_finishes() {
        let mut crawl = Crawl {
            lines: vec![Line {
                text: "Credits".to_owned(),
                heading: true,
            }],
            offset: 0.0,
            backdrop: Backdrop::load(Path::new("unused")),
        };
        let context = egui::Context::default();
        assert!(!crawl.advance(1.0));
        let frame = context.run(egui::RawInput::default(), |ctx| {
            crawl.draw(ctx);
        });
        assert!(frame.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Mesh(mesh) if !mesh.vertices.is_empty())
        }));
        assert!(crawl.advance(30.0));
    }

    #[test]
    fn credit_rows_get_narrower_and_closer_together_in_the_distance() {
        let projection =
            CrawlProjection::new(Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0)));
        let width = |depth| projection.point(100.0, depth).x - projection.point(-100.0, depth).x;
        let gap =
            |depth| projection.point(0.0, depth).y - projection.point(0.0, depth + LINE_STEP).y;
        assert!(width(500.0) < width(0.0));
        assert!(gap(500.0) < gap(0.0));
        assert!(gap(500.0) > 0.0);
    }

    #[test]
    #[ignore = "requires a GPU adapter and baked BF2 credits and planet textures"]
    fn credit_crawl_renders_over_planet_and_stars() {
        let assets = game::assets::baked_root();
        let mut credits = Crawl::load(&assets).expect("baked credits");
        assert!(credits.backdrop.surface.is_some(), "baked planet texture");
        for (width, height) in [(1024, 768), (1280, 720)] {
            let mut renderer = render::Renderer::new_headless(width, height).expect("headless GPU");
            let mut overlay =
                ui::overlay::EguiOverlay::headless(renderer.device(), render::HEADLESS_FORMAT);
            ui::typography::face::install(overlay.ctx(), &game::hud_feed::menu_face(&assets));
            credits.offset = 0.0;
            assert!(!credits.advance(13.0));
            let paint = overlay.begin_headless([width, height], 1.0, |ctx| {
                credits.draw(ctx);
            });
            renderer
                .render_with_overlay(
                    &render::Camera::default(),
                    &[],
                    [0.0, 0.0, 0.0, 1.0],
                    Some(&mut |dev, q, enc, target, size| {
                        overlay.paint(&paint, dev, q, enc, target, size)
                    }),
                )
                .expect("render credits");
            let (rgba, _, _) = renderer.capture_rgba().expect("capture credits");
            let pixels = rgba.chunks_exact(4);
            let yellow = pixels
                .clone()
                .filter(|p| p[0] > 175 && p[1] > 145 && p[2] < 100)
                .count();
            let ocean = pixels
                .skip((width * height * 3 / 4) as usize)
                .filter(|p| p[2] > 60 && p[2] > p[0].saturating_add(25))
                .count();
            assert!(yellow > 200, "visible gold lettering at {width}x{height}");
            assert!(ocean > 5000, "visible blue planet at {width}x{height}");
            if let Some(dir) = std::env::var_os("OPENBF_CREDITS_PREVIEW_DIR") {
                let png = render::encode_png_rgb(&rgba, width, height).expect("encode credits");
                std::fs::write(
                    Path::new(&dir).join(format!("credits-{width}x{height}.png")),
                    png,
                )
                .expect("save credits preview");
            }
            // Texture IDs belong to the overlay's context.
            credits.backdrop = Backdrop::load(&assets);
        }
    }
}
