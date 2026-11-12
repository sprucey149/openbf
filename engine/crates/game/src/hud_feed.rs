//! The BF2 HUD on screen (card BHUD2): loads what `bake::hud` wrote, runs
//! `ui::hud_tree` / `ui::hud_draw` over a [`ui::hud::HudModel`] each frame and
//! paints the result through `render::hud::HudPass`. Art is found by name and
//! loaded the first time an element shows it: sheets and font pages as KTX2,
//! meshes as GLB — the HUD's own in `common_bf2`, the weapons' in the side
//! bakes (the PS2's weapon icons are the weapons' own models, `re-hud.md` §1.2).

use anyhow::{Context, Result};
use render::hud::{HudBatch, HudPass, HudTexture, HudVertex};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use ui::hud::{HudGauge, HudModel, HudWeapon};
use ui::hud_draw::{HudArt, HudMesh, HudMeshPart};
use ui::hud_tree::{Hud, Kind};

/// The sniper scope's overlay for the local player (`ui::hud_scope`,
/// `analysis/re-zoom.md` §8): up while he is alive on foot, zoomed on a
/// `ZoomFirstPerson` weapon (`core::unit::Unit::scoped`), and the camera's
/// zoom S is above 1.0. BF2's `weapon_scope` (INGAME.LVL) on every map, BF1
/// maps included: BF2's HUD everywhere (`analysis/plan/FEATURE_SET.md`).
pub fn scope_view(walk: &crate::walk::WalkSim) -> Option<ui::hud_scope::ScopeView> {
    let me = walk.sim.unit(walk.player)?;
    if !(me.alive && me.mount.is_none() && me.scoped() && walk.zoom_s > 1.0) {
        return None;
    }
    Some(ui::hud_scope::ScopeView)
}

/// The HUD's two weapon rows for the local player: on foot, the weapon in
/// hand and channel 1's (GREN); flying a remote droid, the droid's blaster
/// and destruct (`analysis/re-remote-droid.md` §8); else (riding, or
/// flying a guided rocket) none.
pub fn weapon_rows(walk: &crate::walk::WalkSim) -> [Option<HudWeapon>; 2] {
    let sim = &walk.sim;
    let me = &sim.units[walk.player.index as usize];
    if !me.alive || sim.controlled_guided(walk.player).is_some() {
        return [None, None];
    }
    if me.mount.is_some() {
        return seat_weapon_rows(walk);
    }
    let visual = walk.teams[usize::from(me.team.min(1))].class(me.spec.class);
    let fill = |clip: u32, rounds: u32| {
        if rounds > 0 {
            clip as f32 / rounds as f32
        } else {
            1.0
        }
    };
    let row = hud_weapon;
    if let Some(d) = sim.controlled_droid(walk.player) {
        // The remote's slot: a held slot, else the primary.
        let slot = me
            .held
            .iter()
            .find(|w| w.class == d.weapon_class)
            .map_or(me.weapon_slot, |w| w.slot);
        let Some(hud) = visual.hud_of(slot).droid.as_deref() else {
            return [None, None];
        };
        let blaster = sim.weapon_classes[d.weapon_class]
            .remote
            .as_ref()
            .map(|r| &sim.weapon_classes[r.blaster_index]);
        let primary = blaster.map(|c| {
            row(
                &hud.blaster,
                d.blaster.rounds(c),
                fill(d.blaster.clip, c.rounds_per_clip),
            )
        });
        let secondary = hud
            .destruct
            .as_ref()
            .map(|w| row(w, hud.destruct_rounds, 1.0));
        return [primary, secondary];
    }
    // The weapon in the hands: the primary, or the channel-0 slot the
    // player switched to (GREN).
    let c = sim.weapon_classes.get(me.weapon_class);
    let primary = Some(row(
        visual.hud_of(me.weapon_slot),
        c.and_then(|c| me.weapon.rounds(c)),
        c.map_or(1.0, |c| fill(me.weapon.clip, c.rounds_per_clip)),
    ));
    // A droideka's secondary is its shield, an infinite weapon: the charge
    // is on the stamina bar (the maintainer, 2026-10-30).
    if me.droideka.is_some() {
        let shield = visual.droideka_shield.as_ref();
        return [primary, shield.map(|h| row(h, None, 1.0))];
    }
    // The secondary channel's current weapon: the grenade row (GREN).
    let secondary = me
        .cur1
        .and_then(|k| me.held.get(usize::from(k)))
        .and_then(|w| {
            let c = sim.weapon_classes.get(w.class)?;
            Some(row(
                visual.hud_of(w.slot),
                w.state.rounds(c),
                fill(w.state.clip, c.rounds_per_clip),
            ))
        });
    [primary, secondary]
}

/// What one weapon shows on the ring (`analysis/re-heat-hud.md` 2, `FUN_003b3398`):
/// an unlimited weapon its heat, a counted one its charge while held. The refire
/// bits show reload/firing progress or overheat recovery. Only reads the sim's
/// state. BF1 maps show BF1's heat on BF2's ring, as PLAN puts BF2's HUD
/// on every map.
pub fn gauge_of(c: &core::weapon::WeaponClass, s: &core::weapon::WeaponState) -> HudGauge {
    let ring = if c.infinite_ammo() {
        s.heat.clamp(0.0, 1.0)
    } else {
        s.charging(c).unwrap_or(0.0)
    };
    // FUN_003b3398 B2:409971-409992 publishes remaining progress on
    // the reticle's refire bits. Our reload timer counts down, whereas
    // the firing accumulator counts up. Neither changes the heat/charge ring.
    let refire = if c.infinite_ammo() && s.heat < c.heat_threshold {
        // B2:410020-410025 suppresses refire below an unlimited gun's clear mark.
        0.0
    } else if s.overheated && s.heat > 0.0 {
        1.0 - (c.heat_threshold / s.heat).clamp(0.0, 1.0)
    } else if s.reloading() && c.reload_time > 0.0 {
        (s.reload_timer / c.reload_time).clamp(0.0, 1.0)
    } else if c.display_refire && s.state == core::weapon::FireState::Firing && s.refire_time > 0.0
    {
        1.0 - (s.refire_accum / s.refire_time).clamp(0.0, 1.0)
    } else {
        0.0
    };
    HudGauge {
        ring,
        refire,
        overheated: s.overheated,
    }
}

/// The gauge for the rows [`weapon_rows`] shows: on foot the hands' weapon and
/// the secondary channel's, flying a droid its blaster, in a seat its guns.
pub fn weapon_gauge(walk: &crate::walk::WalkSim) -> HudGauge {
    let sim = &walk.sim;
    let me = &sim.units[walk.player.index as usize];
    if !me.alive || sim.controlled_guided(walk.player).is_some() {
        return HudGauge::default();
    }
    let of = |class: usize, s: &core::weapon::WeaponState| {
        sim.weapon_classes.get(class).map(|c| gauge_of(c, s))
    };
    let rows: [Option<HudGauge>; 2] = if let Some(m) = me.mount {
        let v = sim.vehicle(m.vehicle);
        let setup = v.and_then(|v| sim.seat_setup.get(v.class as usize)?.as_ref());
        let gun = setup.and_then(|s| s.guns.get(usize::from(m.seat))?.as_ref());
        let state = v.and_then(|v| v.guns.get(usize::from(m.seat))?.as_ref());
        std::array::from_fn(|i| of(gun?.weapons.get(i)?.class, state?.weapons.get(i)?))
    } else if let Some(d) = sim.controlled_droid(walk.player) {
        let blaster = sim.weapon_classes[d.weapon_class]
            .remote
            .as_ref()
            .map(|r| r.blaster_index);
        [blaster.and_then(|b| of(b, &d.blaster)), None]
    } else {
        let held = me.cur1.and_then(|k| me.held.get(usize::from(k)));
        [
            of(me.weapon_class, &me.weapon),
            held.and_then(|w| of(w.class, &w.state)),
        ]
    };
    let mut first = rows[0].unwrap_or_default();
    // On foot a launcher's dwell fills the ring (`FUN_003b3398` sends the
    // lock clock as heat while the weapon dwells), or this later send
    // would zero the lock box's fill.
    if me.mount.is_none() {
        let slot = me.spec.player_slot.map_or(0, usize::from);
        if let Some(f) = sim.foot_lock_charge(slot) {
            first.ring = f;
        }
    }
    HudGauge {
        overheated: rows.iter().flatten().any(|g| g.overheated),
        ..first
    }
}

pub fn armed_seat(walk: &crate::walk::WalkSim) -> bool {
    walk.sim.unit(walk.player).is_some_and(|u| {
        u.alive
            && u.mount
                .and_then(|m| walk.sim.vehicle(m.vehicle))
                .is_some_and(|v| v.kind != core::vehicle::VehicleKind::Flyer || v.health.alive())
    }) && seat_weapon_rows(walk).iter().any(Option::is_some)
}

fn seat_weapon_rows(walk: &crate::walk::WalkSim) -> [Option<HudWeapon>; 2] {
    let sim = &walk.sim;
    let Some(m) = sim.unit(walk.player).and_then(|u| u.mount) else {
        return [None, None];
    };
    let Some(v) = sim.vehicle(m.vehicle) else {
        return [None, None];
    };
    let class = v.class as usize;
    let seat = m.seat as usize;
    let Some(gun) = sim
        .seat_setup
        .get(class)
        .and_then(Option::as_ref)
        .and_then(|s| s.guns.get(seat))
        .and_then(Option::as_ref)
    else {
        return [None, None];
    };
    std::array::from_fn(|channel| {
        let weapon = gun.weapons.get(channel)?;
        let c = sim.weapon_classes.get(weapon.class)?;
        let visual = walk
            .vehicle_weapon_hud
            .get(class)?
            .get(seat)?
            .get(channel)?;
        let state = v
            .guns
            .get(seat)
            .and_then(Option::as_ref)
            .and_then(|s| s.weapons.get(channel));
        Some(hud_weapon(
            visual,
            if weapon.infinite {
                None
            } else {
                state.and_then(|s| s.rounds(c))
            },
            state.map_or(1.0, |s| {
                if c.rounds_per_clip > 0 {
                    s.clip as f32 / c.rounds_per_clip as f32
                } else {
                    1.0
                }
            }),
        ))
    })
}

/// A weapon row of the HUD model for `hud`.
pub fn hud_weapon(
    hud: &crate::walk::WeaponHud,
    total: Option<u32>,
    clip_fraction: f32,
) -> HudWeapon {
    HudWeapon {
        key: hud.key.clone(),
        class: hud.class.clone(),
        bf2: hud.bf2,
        icon: hud.icon.clone(),
        // The localised weapon name waits for the Locl tables.
        name: String::new(),
        total,
        clip_fraction,
    }
}

/// How long a first frame lets the fades run before it is drawn: a capture
/// shows the settled HUD, not the first instant of its 3 s fade-in.
const SETTLE_SECONDS: f32 = 10.0;

pub struct HudView {
    hud: Hud,
    art: HudArt,
    /// Built at the first paint, where the device is at hand.
    pass: Option<HudPass>,
    format: wgpu::TextureFormat,
    display: Option<wgpu::TextureFormat>,
    /// Where sheets and font pages are looked for, in order.
    texture_dirs: Vec<PathBuf>,
    native_art: HashSet<String>,
    common_textures: PathBuf,
    native_art_dir: PathBuf,
    distance_pages: HashMap<String, crate::assets::CpuTexture>,
    /// Every drawable mesh by lower-cased name.
    mesh_files: HashMap<String, PathBuf>,
    textures: HashMap<String, Option<HudTexture>>,
    /// The control prompts no bake supplied: each sheet at the sizes drawn.
    drawn_icons: HashMap<String, Vec<(u32, Option<HudTexture>)>>,
    leaving: ui::hud::LeavingPulse,
    missing: HashSet<String>,
    verts: Vec<HudVertex>,
    indices: Vec<u32>,
    /// The laid-out batches, their sheets still by name.
    batches: Vec<ui::hud_draw::HudBatch>,
    /// Per drawn element of the last layout: its index and window box.
    boxes: Vec<(usize, [f32; 4])>,
    /// The squad cross's parts of the last layout.
    squad: Vec<(ui::hud_squad::Part, [f32; 4], f32)>,
    settled: bool,
    /// The shell movie's texture and the frame waiting to be written into
    /// it (card BSHLA).
    movie: Option<HudTexture>,
    movie_frame: Option<(Vec<u8>, u32, u32)>,
}

fn read_ron<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    ron::from_str(&pak::read_to_string(path).with_context(|| path.display().to_string())?)
        .with_context(|| path.display().to_string())
}

fn read_hud_sheet(path: &Path) -> Option<crate::assets::CpuTexture> {
    let bytes = pak::read(path).ok()?;
    let header = ktx2::Reader::new(&bytes).ok()?.header();
    let max_levels = 32 - header.pixel_width.max(header.pixel_height).leading_zeros();
    if header.level_count > max_levels {
        return None;
    }
    let image = crate::assets::load_ktx2(&bytes).ok()?;
    let size = (image.width as usize)
        .checked_mul(image.height as usize)?
        .checked_mul(4)?;
    (size != 0 && image.rgba.len() == size).then_some(image)
}

fn add_distance_font(
    art: &mut HudArt,
    pages: &mut HashMap<String, crate::assets::CpuTexture>,
    dir: &Path,
    name: &str,
) {
    let Ok(mut font) =
        read_ron::<manifest::hud::HudDistanceFont>(&dir.join(format!("{name}.sdf.ron")))
    else {
        return;
    };
    // A partial or damaged optional bake must still render bitmap glyphs.
    let used: HashSet<_> = font
        .glyphs
        .iter()
        .map(|glyph| usize::from(glyph.page))
        .collect();
    let valid: Vec<_> = font
        .pages
        .iter()
        .take(usize::from(u8::MAX) + 1)
        .enumerate()
        .map(|(i, page)| {
            let atlas = if manifest::hud::MENU_FONTS
                .iter()
                .any(|(menu, _)| *menu == name)
            {
                manifest::hud::MENU_ATLAS
            } else {
                name
            };
            if !used.contains(&i) || *page != format!("{atlas}.sdf{i}") {
                return false;
            }
            if pages.contains_key(page) {
                return true;
            }
            let Some(mut image) = read_hud_sheet(&dir.join(format!("{page}.ktx2"))) else {
                return false;
            };
            // The bake writes fixed-size RGBA8 atlas pages.
            if image.width != 1024 || image.height != 1024 {
                return false;
            }
            image.extra_mips.clear();
            pages.insert(page.clone(), image);
            true
        })
        .collect();
    font.glyphs.retain(|glyph| {
        valid.get(usize::from(glyph.page)) == Some(&true)
            && glyph.rect.iter().all(|v| v.is_finite())
            && glyph.uv.iter().all(|v| (0.0..=1.0).contains(v))
            && glyph.rect[0] < glyph.rect[2]
            && glyph.rect[1] < glyph.rect[3]
            && glyph.uv[0] < glyph.uv[2]
            && glyph.uv[1] < glyph.uv[3]
    });
    if !font.glyphs.is_empty() {
        art.distance_fonts.insert(name.to_ascii_lowercase(), font);
    }
}

/// Load the optional menu family atomically. Missing/corrupt sidecars retain
/// the legacy face, including old bakes. Unsupported symbols borrow the legacy
/// role's bitmap glyphs; supported letters and their metrics stay canonical.
fn add_menu_fonts(
    art: &mut HudArt,
    pages: &mut HashMap<String, crate::assets::CpuTexture>,
    dir: &Path,
) {
    let mut menu_art = HudArt::default();
    let mut menu_pages = HashMap::new();
    for (name, legacy) in manifest::hud::MENU_FONTS {
        let Ok(mut font) = read_ron::<manifest::hud::HudFont>(&dir.join(format!("{name}.ron")))
        else {
            return;
        };
        add_distance_font(&mut menu_art, &mut menu_pages, dir, name);
        let Some(distance) = menu_art.distance_fonts.get(name) else {
            return;
        };
        if font.name != name
            || font.line_height == 0
            || font.pages != distance.pages
            || font.glyphs.iter().any(|g| {
                usize::from(g.page) >= font.pages.len()
                    || (g.x1 > g.x0
                        && g.y1 > g.y0
                        && !distance
                            .glyphs
                            .iter()
                            .any(|d| d.code == g.code && d.page == g.page))
            })
        {
            return;
        }
        if let Some(base) = art.fonts.get(legacy) {
            let offset = font.pages.len();
            if offset + base.pages.len() > usize::from(u8::MAX) + 1 {
                return;
            }
            let supported: HashSet<_> = font.glyphs.iter().map(|g| g.code).collect();
            font.glyphs.extend(
                base.glyphs
                    .iter()
                    .filter(|g| !supported.contains(&g.code))
                    .map(|g| {
                        let mut g = *g;
                        g.page = (usize::from(g.page) + offset) as u8;
                        g
                    }),
            );
            font.pages.extend(base.pages.clone());
        }
        menu_art.fonts.insert(name.into(), font);
    }
    art.fonts.extend(menu_art.fonts);
    art.distance_fonts.extend(menu_art.distance_fonts);
    pages.extend(menu_pages);
}

/// `name` → file for every `*.glb` in `dirs`; the first directory wins.
fn glb_index(dirs: &[PathBuf]) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    for d in dirs {
        let Ok(files) = pak::read_dir(d) else {
            continue;
        };
        for p in files {
            if p.extension().is_some_and(|x| x == "glb") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.entry(stem.to_ascii_lowercase()).or_insert(p);
                }
            }
        }
    }
    out
}

/// BF1 pilots carry their Cutter in row 2, which BF2's layout never places.
/// Use BF2's authored Cutter rotation/scale at its grenade-row position.
/// This is an openbf presentation choice (BF2 HUD everywhere), not a change
/// to repair behavior. Existing placements and older/missing-model bakes keep
/// their normal selection and icon fallback.
fn place_secondary_cutters(hud: &mut Hud, has_bf2_mesh: impl Fn(&str) -> bool) {
    let (Some(primary), Some(secondary)) = (
        hud.find("player1weapon1mesh"),
        hud.find("player1weapon2mesh"),
    ) else {
        return;
    };
    let Some(position) = hud.elems[secondary]
        .mesh_info
        .iter()
        .find(|info| info.mesh == "hud_thermaldetonator")
        .map(|info| info.pos)
        .filter(|position| *position != glam::Vec3::ZERO)
    else {
        return;
    };
    for side in ["all", "imp", "rep", "cis"] {
        let name = format!("{side}_weap_inf_fusioncutter");
        if !has_bf2_mesh(&name)
            || hud.elems[secondary]
                .mesh_info
                .iter()
                .any(|info| info.mesh == name)
        {
            continue;
        }
        if let Some(mut pose) = hud.elems[primary]
            .mesh_info
            .iter()
            .find(|info| info.mesh == name && info.pos != glam::Vec3::ZERO)
            .cloned()
        {
            pose.pos = position;
            hud.elems[secondary].mesh_info.push(pose);
        }
    }
}

mod grenade;

/// The HUD's own art (`hud_*`) composites by its sheets' alpha
/// (`re-hud-icons.md` §2); any other model the HUD shows is a world model
/// drawn by its material, and an opaque part's sheet alpha is not coverage
/// (`[M]` the PS2 draws the clone rifle and pistol solid, card HUDW's frames;
/// their sheet's alpha plane is the gloss of MTRL flags 0x10089).
fn load_mesh(path: &Path) -> Result<HudMesh> {
    let cpu = crate::assets::load_glb(path)?;
    let art = path
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.to_ascii_lowercase().starts_with("hud_"));
    let parts = cpu
        .submeshes
        .iter()
        .enumerate()
        .map(|(k, (start, count, material, layer, _))| {
            // Re-index the part's own vertices: a weapon model is laid out
            // on the CPU every frame, so carry only what it draws.
            let mut remap: HashMap<u32, u32> = HashMap::new();
            let mut texture = material.to_ascii_lowercase();
            if !art && *layer == render::DrawLayer::Opaque {
                texture.push_str(ui::hud_draw::OPAQUE_SHEET);
            }
            let scroll = cpu.uv_scroll(k);
            if scroll != [0.0; 2] {
                texture.push_str(ui::hud_draw::SCROLL_SHEET);
            }
            let mut part = HudMeshPart {
                texture,
                scroll,
                ..Default::default()
            };
            for i in &cpu.indices[*start as usize..(*start + *count) as usize] {
                let k = *remap.entry(*i).or_insert_with(|| {
                    part.positions.push(cpu.positions[*i as usize]);
                    part.uvs
                        .push(cpu.uvs.get(*i as usize).copied().unwrap_or([0.0; 2]));
                    part.colors.push(
                        cpu.colors
                            .get(*i as usize)
                            .map_or([1.0; 4], |c| c.map(|b| f32::from(b) / 255.0)),
                    );
                    part.positions.len() as u32 - 1
                });
                part.indices.push(k);
            }
            part
        })
        .collect();
    Ok(HudMesh { parts })
}

/// How many window pixels high a batch's quads stand.
fn batch_height(verts: &[HudVertex], indices: &[u32], first: u32, count: u32) -> f32 {
    let (first, count) = (first as usize, count as usize);
    let (lo, hi) = indices[first..first + count]
        .iter()
        .map(|i| verts[*i as usize].pos[1])
        .fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
    (hi - lo).max(0.0)
}

impl HudView {
    /// Hit-test the shell's actually laid-out, visible menu rows.
    pub fn shell_row_at(
        &self,
        menus: &ui::shell_ps2::ShellMenus,
        point: [f32; 2],
        size: [f32; 2],
    ) -> Option<usize> {
        let font = self.art.menu_font(ui::spawn_draw::BUTTON_FONT)?;
        ui::shell_ps2::row_at(font, menus, point, size)
    }

    /// Hit-test the title screen's rendered Start prompt.
    pub fn shell_prompt_at(
        &self,
        menus: &ui::shell_ps2::ShellMenus,
        point: [f32; 2],
        size: [f32; 2],
    ) -> bool {
        self.art
            .menu_font(ui::spawn_draw::BUTTON_FONT)
            .is_some_and(|font| ui::shell_ps2::prompt_at(font, menus, point, size))
    }

    /// Hit-test Instant Action using the same font metrics as its draw pass.
    pub fn instant_target(
        &self,
        ia: &ui::shell::InstantAction,
        point: [f32; 2],
        size: [f32; 2],
    ) -> Option<ui::shell::PointerTarget> {
        let font = self.art.menu_font(ui::spawn_draw::BUTTON_FONT)?;
        ui::shell_ia::pointer_target(ia, point, size, font)
    }

    /// [`HudView::load`] for `r`'s frames.
    pub fn for_renderer(assets_dir: &Path, r: &render::Renderer) -> Result<Self> {
        Self::load(assets_dir, r.surface_format(), r.display_view_format())
    }

    /// Load the 1-player HUD from the bake at `assets_dir`, to be painted
    /// into frames of `format` through a `display` view where there is one
    /// (`render::Renderer::display_view_format`). Touches no GPU: the pass is
    /// built at the first paint. An error means the bake holds no BF2 HUD
    /// (`bake common` writes it).
    pub fn load(
        assets_dir: &Path,
        format: wgpu::TextureFormat,
        display: Option<wgpu::TextureFormat>,
    ) -> Result<Self> {
        let common = assets_dir.join("common_bf2");
        let layout: manifest::hud::HudLayout = read_ron(&common.join("hud/1playerhud.ron"))?;
        let transforms: manifest::hud::HudLayout = read_ron(&common.join("hud/hudtransforms.ron"))?;
        let index: manifest::hud::HudIndex = read_ron(&common.join("hud/index.ron"))?;
        let mut art = HudArt::default();
        let mut distance_pages = HashMap::new();
        let fonts_dir = common.join("fonts");
        for f in &index.fonts {
            let font: manifest::hud::HudFont = read_ron(&common.join(format!("fonts/{f}.ron")))?;
            art.fonts.insert(f.to_ascii_lowercase(), font);
            add_distance_font(&mut art, &mut distance_pages, &fonts_dir, f);
        }
        add_menu_fonts(&mut art, &mut distance_pages, &fonts_dir);
        // BF1's own tiny font for the squad cross's labels (card SQHUD); a
        // bake without it falls back to BF2's.
        let bf1_fonts = assets_dir.join("common_bf1/fonts");
        let tiny = ui::hud_squad::FONT;
        if let Ok(font) = read_ron(&bf1_fonts.join(format!("{tiny}.ron"))) {
            art.fonts.insert(tiny.to_owned(), font);
            add_distance_font(&mut art, &mut distance_pages, &bf1_fonts, tiny);
        }
        let mesh_files = glb_index(&[
            common.join("models"),
            assets_dir.join("side_bf2/weapons"),
            assets_dir.join("side_bf1/weapons"),
        ]);
        let mut layout = layout;
        ui::hud::add_leaving_line(&mut layout);
        let mut hud = Hud::new(&layout, &transforms);
        let bf2_weapons = assets_dir.join("side_bf2/weapons");
        place_secondary_cutters(&mut hud, |name| {
            mesh_files
                .get(name)
                .is_some_and(|path| path.parent() == Some(bf2_weapons.as_path()))
        });
        if let Some(thermal) = mesh_files
            .get("hud_thermaldetonator")
            .and_then(|path| load_mesh(path).ok())
        {
            grenade::install(&mut hud, &mut art, &thermal);
        }
        // The two faded pills belong behind the weapon images, including
        // bitmap fallbacks. Our per-model depth slices follow draw order,
        // so the source ZOrder(8) would shade the default-128 item groups.
        // Change only these siblings' presentation order; retain their
        // transforms, alpha and internal mesh depth (also in flat PC trees).
        for name in ["player1weapon1backdrop", "player1weapon2backdrop"] {
            if let Some(i) = hud.find(name) {
                if hud.elems[i].kind == Kind::Model3D {
                    hud.elems[i].z_order = u8::MAX;
                }
            }
        }
        hud.set_fonts(&art.fonts);
        // The layout's own labels ("Victory In"), from BF2's text table.
        if let Some(locl) = crate::event_feed::read_locl(&common) {
            hud.set_texts(&|key| {
                let t = locl.get(manifest::hud::locl_key(key))?;
                Some(t.trim().to_owned())
            });
        }
        ui::hud::place_countdown_labels(&mut hud);
        Ok(Self {
            hud,
            art,
            pass: None,
            format,
            display,
            texture_dirs: vec![
                common.join("textures"),
                common.join("fonts"),
                // The shell's own sheets (`logo`, card BSHLA).
                crate::shell_movie::shell_dir(assets_dir).join("textures"),
                bf1_fonts,
                assets_dir.join("side_bf2/textures"),
                assets_dir.join("common_bf1/textures"),
                assets_dir.join("side_bf1/textures"),
            ],
            native_art: read_ron(&common.join("hud/native-art.ron")).unwrap_or_default(),
            common_textures: common.join("textures"),
            native_art_dir: common.join("hud/native-art"),
            distance_pages,
            mesh_files,
            textures: HashMap::new(),
            drawn_icons: HashMap::new(),
            leaving: Default::default(),
            missing: HashSet::new(),
            verts: Vec::new(),
            indices: Vec::new(),
            batches: Vec::new(),
            boxes: Vec::new(),
            squad: Vec::new(),
            settled: false,
            movie: None,
            movie_frame: None,
        })
    }

    /// The icons the control prompts draw from the next layout on.
    pub fn set_icons(&mut self, icons: ui::input_icons::Icons) {
        self.art.icons = icons;
    }

    fn texture_file(&self, name: &str) -> Option<PathBuf> {
        self.texture_dirs
            .iter()
            .map(|d| d.join(format!("{name}.ktx2")))
            .find(|p| pak::is_file(p))
    }

    /// Feed `model`, advance the fades by `dt` seconds and lay the HUD out
    /// for a `size` window. The first call settles the fades.
    pub fn update(&mut self, model: &HudModel, dt: f32, size: [u32; 2]) {
        // Each weapon row's model, else its icon sheet ([`Self::weapon_mesh`]).
        let mut model = model.clone();
        for (row, w) in [(1, &mut model.weapon), (2, &mut model.weapon2)] {
            let Some(w) = w else {
                continue;
            };
            let (key, mesh) = self.weapon_mesh(row, w);
            w.key = key;
            let drawable = mesh.is_some();
            w.icon = match (&w.icon, drawable) {
                (Some(icon), false) => self.texture_file(icon).map(|p| {
                    if !self.art.sheet_aspect.contains_key(icon) {
                        let aspect = pak::read(&p)
                            .ok()
                            .and_then(|b| crate::assets::load_ktx2(&b).ok())
                            .map_or(2.0, |t| t.width as f32 / t.height.max(1) as f32);
                        self.art.sheet_aspect.insert(icon.clone(), aspect);
                    }
                    icon.clone()
                }),
                _ => None,
            };
        }
        {
            let files = &self.mesh_files;
            let meshes = &self.art.meshes;
            ui::hud::feed(&mut self.hud, &model, &|n: &str| {
                files.contains_key(n) || meshes.contains_key(n)
            });
        }
        if let Some(dir) = model.space.as_ref().and_then(|m| m.sheet_dir.as_ref()) {
            if !self.texture_dirs.contains(dir) {
                self.texture_dirs.insert(0, dir.clone());
            }
        }
        // The minimap's image is the map's own sheet.
        if let Some(dir) = model.map.as_ref().and_then(|m| m.sheet_dir.as_ref()) {
            if !self.texture_dirs.contains(dir) {
                self.texture_dirs.insert(0, dir.clone());
            }
        }
        let first = !self.settled;
        self.settled = true;
        self.hud.advance(if first { SETTLE_SECONDS } else { dt });
        if let Some(alpha) = self.leaving.step(model.leaving.is_some(), dt) {
            if let Some(i) = self.hud.find(ui::hud::LEAVING_ELEM) {
                self.hud.elems[i].alpha = alpha;
            }
        }

        // Load the meshes the shown elements name.
        let wanted: Vec<String> = self
            .hud
            .elems
            .iter()
            .filter(|e| e.kind == Kind::Model3D && e.fader.state != 0)
            .filter_map(|e| e.mesh.clone())
            .chain(model.map.is_some().then(|| "hud_map_fov".to_owned()))
            .filter(|m| !self.art.meshes.contains_key(m) && !self.missing.contains(m))
            .collect();
        for name in wanted {
            match self.mesh_files.get(&name).map(|p| load_mesh(p)) {
                Some(Ok(mesh)) => {
                    self.art.meshes.insert(name, mesh);
                }
                Some(Err(e)) => {
                    eprintln!("hud: mesh {name:?}: {e:#}");
                    self.missing.insert(name);
                }
                None => {
                    self.missing.insert(name);
                }
            }
        }

        let frame = ui::hud_draw::draw(&self.hud, &self.art, [size[0] as f32, size[1] as f32]);
        self.install_frame(frame);
    }

    /// Lay the shell's page `menus` out for a `size` window (card BSHLA):
    /// the movie's last frame under it when `movie` (see
    /// [`HudView::set_movie_frame`]), the HUD tree not at all.
    pub fn update_shell(&mut self, menus: &ui::shell_ps2::ShellMenus, movie: bool, size: [u32; 2]) {
        let movie = movie && (self.movie.is_some() || self.movie_frame.is_some());
        self.hud.shell = Some((menus.clone(), movie));
        let frame = ui::hud_draw::draw(&self.hud, &self.art, [size[0] as f32, size[1] as f32]);
        self.hud.shell = None;
        self.install_frame(frame);
    }

    /// [`HudView::update_shell`] with the Instant Action screen `ia` over
    /// the page (`ui::shell_ia`).
    pub fn update_instant(
        &mut self,
        menus: &ui::shell_ps2::ShellMenus,
        ia: &ui::shell::InstantAction,
        movie: bool,
        size: [u32; 2],
    ) {
        self.hud.instant = Some(Box::new(ia.clone()));
        self.update_shell(menus, movie, size);
        self.hud.instant = None;
    }

    fn install_frame(&mut self, frame: ui::hud_draw::HudFrame) {
        self.verts.clear();
        self.verts.extend(frame.verts.iter().map(|v| HudVertex {
            pos: v.pos,
            uv: v.uv,
            color: v.color,
        }));
        self.indices = frame.indices;
        self.batches = frame.batches;
        self.boxes = frame.boxes;
        self.squad = frame.squad;
    }

    /// The shell movie's newest frame (RGBA8 display values), written into
    /// its texture at the next paint.
    pub fn set_movie_frame(&mut self, rgba: Vec<u8>, width: u32, height: u32) {
        self.movie_frame = Some((rgba, width, height));
    }

    /// The sheets the last [`HudView::update`] drew from, in draw order.
    /// Send one layout event straight to the HUD tree, past the model: for
    /// a picture of an element the game does not light (hitmark's plain
    /// `target.hit` arc, `analysis/re-target-hud.md` §12). The next
    /// [`HudView::update`] lays it out.
    /// The model weapon row `row` (1 or 2) shows for `w`: the key its
    /// `hudtransforms` table is read with and the model that names; `None`
    /// when the row shows `w.icon` in its place (or nothing). `[ours]` per
    /// row: a BF1 class keys BF2's table by its own name when its geometry
    /// has no entry, and a weapon with no BF2 mesh shows its icon sheet.
    pub fn weapon_mesh(&self, row: u8, w: &HudWeapon) -> (String, Option<String>) {
        if let Some(mesh) = grenade::mesh_for(&self.hud, &self.art, row, w) {
            return (mesh.to_owned(), Some(mesh.to_owned()));
        }
        let files = &self.mesh_files;
        let has = |n: &str| files.contains_key(n);
        let event = format!("player1.weapon{row}.change");
        let listed = |key: &str| {
            self.hud
                .name_mesh(&event, key, &has)
                .filter(|mesh| !mesh.eq_ignore_ascii_case(key))
        };
        let mut key = w.key.clone();
        // A BF2 class's key that names a model keeps it, as on the PS2: the
        // engineer's detpack sends `com_weap_inf_detpack`, while the table's
        // `rep_weap_inf_detpack` row is the time bomb's model.
        let unresolved = if w.bf2 {
            self.hud.name_mesh(&event, &key, &has).is_none()
        } else {
            listed(&key).is_none()
        };
        if unresolved && !w.class.is_empty() && listed(&w.class).is_some() {
            key.clone_from(&w.class);
        }
        // A model the layout does not place (a BF1 geometry drawn by the
        // fallback alone: no MeshInfo, or one with a rotation and no
        // Position, which puts it at the group's origin) loses to the
        // weapon's icon.
        let placed = |mesh: &str| {
            self.hud
                .find(&format!("player1weapon{row}mesh"))
                .is_some_and(|i| {
                    let infos = &self.hud.elems[i].mesh_info;
                    infos
                        .iter()
                        .any(|k| k.mesh == mesh && k.pos != glam::Vec3::ZERO)
                })
        };
        let mesh = self
            .hud
            .name_mesh(&event, &key, &has)
            .filter(|m| w.icon.is_none() || !m.eq_ignore_ascii_case(&key) || placed(m));
        (key, mesh)
    }

    pub fn send(&mut self, event: &str, value: ui::hud_tree::EventValue) {
        let files = &self.mesh_files;
        self.hud.send(event, value, &|n| files.contains_key(n));
    }

    pub fn drawn_sheets(&self) -> impl Iterator<Item = &str> {
        self.batches.iter().map(|b| b.texture.as_str())
    }

    /// Requested sheets that had no usable texture in the last paint.
    pub fn unresolved_sheets(&self) -> impl Iterator<Item = &str> {
        self.drawn_sheets().filter(|sheet| {
            self.textures.get(*sheet) == Some(&None)
                || self
                    .drawn_icons
                    .get(*sheet)
                    .is_some_and(|sizes| sizes.last().is_some_and(|(_, texture)| texture.is_none()))
        })
    }

    /// The window box `[x0, y0, x1, y1]` the element called `name` was drawn
    /// in by the last [`HudView::update`]; `None` when it drew nothing.
    pub fn bounds(&self, name: &str) -> Option<[f32; 4]> {
        let i = self.hud.find(name)?;
        self.boxes
            .iter()
            .filter(|(e, _)| *e == i)
            .map(|(_, b)| *b)
            .reduce(|a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            })
    }

    /// The squad cross's part `part` as the last [`HudView::update`] drew
    /// it: its window box and alpha.
    pub fn squad_part(&self, part: ui::hud_squad::Part) -> Option<([f32; 4], f32)> {
        self.squad
            .iter()
            .find(|(p, ..)| *p == part)
            .map(|(_, b, a)| (*b, *a))
    }

    /// Where the minimap draws a world point (x, z) in a `size` window
    /// (`ui::hud_map::window_point`).
    pub fn map_point(&self, world: [f32; 2], clamp: bool, size: [u32; 2]) -> Option<[f32; 2]> {
        ui::hud_map::window_point(&self.hud, world, clamp, [size[0] as f32, size[1] as f32])
    }

    /// The layout's Map element `Scale`, which the spawn map page is drawn
    /// at (`ui::hud_map::map_scale`).
    pub fn spawn_map_scale(&self) -> Option<glam::Vec3> {
        ui::hud_map::map_scale(&self.hud)
    }

    /// Paint the last [`HudView::update`] (call from the overlay hook, before
    /// the egui overlay).
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size: [u32; 2],
    ) {
        let (format, display) = (self.format, self.display);
        let pass = self
            .pass
            .get_or_insert_with(|| HudPass::new(device, queue, format, display));
        if let Some((rgba, w, h)) = self.movie_frame.take() {
            let t = match self.movie {
                Some(t) => Some(t),
                None => pass.add_stream_texture(device, queue, w, h),
            };
            if let Some(t) = t {
                pass.write_stream_texture(queue, t, &rgba);
            }
            self.movie = t;
        }
        let mut batches = Vec::with_capacity(self.batches.len());
        let texture_limit = device.limits().max_texture_dimension_2d;
        for b in &self.batches {
            let texture = if b.texture == ui::shell_ps2::MOVIE_SHEET {
                self.movie
            } else if let Some(image) = self.distance_pages.remove(&b.texture) {
                let texture = pass.add_distance_texture(
                    device,
                    queue,
                    &image.rgba,
                    image.width,
                    image.height,
                );
                self.textures.insert(b.texture.clone(), texture);
                texture
            } else if let Some(sizes) = self.drawn_icons.get_mut(&b.texture) {
                let px = crate::input_icons::icon_px(batch_height(
                    &self.verts,
                    &self.indices,
                    b.first,
                    b.count,
                ));
                match sizes.iter().find(|(at, _)| *at == px) {
                    Some((_, t)) => *t,
                    None => {
                        let t = crate::input_icons::rasterize(&b.texture, px).and_then(
                            |(rgba, w, h)| pass.add_texture(device, queue, &[&rgba], w, h, false),
                        );
                        sizes.push((px, t));
                        t
                    }
                }
            } else {
                match self.textures.get(&b.texture) {
                    Some(t) => *t,
                    None => {
                        let (sheet, repeat) =
                            match b.texture.strip_suffix(ui::hud_draw::SCROLL_SHEET) {
                                Some(sheet) => (sheet, true),
                                None => (b.texture.as_str(), false),
                            };
                        let (sheet, opaque) = match sheet.strip_suffix(ui::hud_draw::OPAQUE_SHEET) {
                            Some(sheet) => (sheet, true),
                            None => (sheet, false),
                        };
                        // `<layer>/<sheet>`: that common layer's own copy
                        // (`common_bf1/ig_victory_defeat`, `ui::hud`).
                        let (layer, sheet) = match sheet.split_once('/') {
                            Some((l, s)) => (Some(l), s),
                            None => (None, sheet),
                        };
                        let baked = self
                            .texture_dirs
                            .iter()
                            .filter(|d| {
                                layer.is_none_or(|l| {
                                    d.parent().and_then(|p| p.file_name())
                                        == Some(std::ffi::OsStr::new(l))
                                })
                            })
                            .map(|d| d.join(format!("{sheet}.ktx2")))
                            .find(|p| pak::is_file(p))
                            .and_then(|p| {
                                let native = (p.parent() == Some(self.common_textures.as_path())
                                    && self.native_art.contains(sheet))
                                .then(|| self.native_art_dir.join(format!("{sheet}.ktx2")))
                                .and_then(|path| read_hud_sheet(&path))
                                .filter(|t| t.width.max(t.height) <= texture_limit);
                                native.or_else(|| {
                                    let mut texture = read_hud_sheet(&p)?;
                                    texture.extra_mips.clear();
                                    Some(texture)
                                })
                            });
                        // A control prompt no bake supplied is drawn here, at
                        // the size it shows; the next frame finds it above.
                        let icon = baked.is_none() && crate::input_icons::has_art(&b.texture);
                        let px = crate::input_icons::icon_px(batch_height(
                            &self.verts,
                            &self.indices,
                            b.first,
                            b.count,
                        ));
                        let t = if let Some(mut image) = baked {
                            let levels =
                                std::iter::once(&mut image.rgba).chain(&mut image.extra_mips);
                            let levels: Vec<&[u8]> = levels
                                .map(|level| {
                                    if opaque {
                                        for pixel in level.as_chunks_mut::<4>().0 {
                                            pixel[3] = 255;
                                        }
                                    }
                                    level.as_slice()
                                })
                                .collect();
                            pass.add_texture(
                                device,
                                queue,
                                &levels,
                                image.width,
                                image.height,
                                repeat,
                            )
                        } else {
                            crate::input_icons::rasterize(sheet, px).and_then(|(rgba, w, h)| {
                                pass.add_texture(device, queue, &[&rgba], w, h, repeat)
                            })
                        };
                        if t.is_none() {
                            eprintln!("hud: no sheet {:?}; drawn untextured", b.texture);
                        }
                        if icon {
                            self.drawn_icons.insert(b.texture.clone(), vec![(px, t)]);
                        } else {
                            self.textures.insert(b.texture.clone(), t);
                        }
                        t
                    }
                }
            };
            batches.push(HudBatch {
                texture,
                additive: b.additive,
                depth: b.depth,
                first: b.first,
                count: b.count,
            });
        }
        pass.paint(
            &self.verts,
            &self.indices,
            &batches,
            device,
            queue,
            encoder,
            view,
            size,
        );
    }
}

#[cfg(test)]
mod tests;
