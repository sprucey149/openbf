//! Soldier asset loading, animation updates and draw submission.

mod animation;

use super::{
    drawn, fire_point, held_or_primary, model_of, part_groups, part_spec, rider_now, seat_now,
    with_spec, ClassDraw, DrawPlan, FlashModel, HoverClock, NativeDraw, SeatClock, SoldierAsset,
    SoldierVisuals, UnitVisual, WeaponAsset, CORPSE_SECS,
};
use crate::animator::{AnimInput, Animator, ClassAnim, Rig};
use crate::assets::{load_visual_skinned_glb, SkinnedAsset};
use crate::viewer::turret_pose as tp;
use crate::walk::{TeamTemplate, WalkSim};
use glam::{Mat4, Vec3};
use render::{ParticleDraw, Renderer, SkinnedDrawCmd, TextureHandle};
use std::collections::HashMap;
use std::path::Path;

/// The bone a jet effect rides (card JETP2, PROVISIONAL: `crate::fx`
/// `jet.rs`).
pub const JET_BONE: &str = "bone_ribcage";

impl SoldierVisuals {
    pub fn append_test_class(
        &mut self,
        r: &mut Renderer,
        root: &Path,
        walk: &WalkSim,
        request: &crate::walk::ClassAssetRequest,
    ) {
        let crate::walk::ClassAssetRequest { team, class, game } = *request;
        let teams = request.templates(walk);
        let mut added = Self::load(r, root, game, &teams);
        added.fit_throw_to_map(root, self.bf2);
        let bodies = self.assets.len();
        let weapons = self.weapons.len();
        for draw in &mut added.per_class[team] {
            draw.body = draw.body.map(|b| b + bodies);
            draw.weapon = draw.weapon.map(|w| w + weapons);
        }
        for held in &mut added.held[team] {
            for (_, model) in held {
                *model = model.map(|w| w + weapons);
            }
        }
        self.fp.append_class(team, class, added.fp);
        self.assets.extend(added.assets);
        self.weapons.extend(added.weapons);
        self.per_class[team].append(&mut added.per_class[team]);
        self.held[team].append(&mut added.held[team]);
        self.anim[team].append(&mut added.anim[team]);
        self.skins[team].append(&mut added.skins[team]);
        self.low_skins[team].append(&mut added.low_skins[team]);
        self.sabers[team].resize_with(class, || None);
        self.sabers[team].push(added.sabers[team].pop().flatten());
        let mut cache = HashMap::new();
        for weapon in &walk.sim.weapon_classes[self.grenades.len()..] {
            // A BF2 class on a BF1 map throws BF1's grenade: BF1's model.
            let bf1_grenade = !self.bf2
                && weapon
                    .ordnance
                    .sticky
                    .as_ref()
                    .is_some_and(|sticky| !sticky.bf2);
            let model = weapon.ordnance.geometry.as_deref().map(|name| {
                let index = self.weapons.len();
                self.weapons.push(Self::load_weapon(
                    Some(&mut *r),
                    root,
                    if bf1_grenade { "bf1" } else { game },
                    name,
                    &mut cache,
                ));
                index
            });
            self.grenades.push(model);
        }
        self.sync_units(walk);
        for (v, u) in self.units.iter_mut().zip(&walk.sim.units) {
            if usize::from(u.team) == team && usize::from(u.spec.class) == class {
                *v = UnitVisual {
                    gen: u.gen,
                    ..Default::default()
                };
            }
        }
        self.update(walk, 0.0);
    }

    /// Load the battle's soldier models and install its turret and walker rigs.
    pub fn for_walk(
        r: &mut Renderer,
        assets_dir: &Path,
        scene: &crate::assets::Scene,
        walk: &WalkSim,
    ) -> Self {
        let mut vis = Self::load(r, assets_dir, &scene.manifest.source_game, &walk.teams);
        for (&team, template) in &walk.native_teams {
            vis.load_native_team(r, assets_dir, &scene.manifest.source_game, team, template);
        }
        vis.lod_view.near = scene.manifest.lighting.near_scene_range;
        vis.lod_view.far = scene.manifest.lighting.far_scene_range;
        vis.own_model = std::array::from_fn(|t| {
            let classes = &walk.sim.classes[t];
            classes.iter().map(|c| c.droideka.is_some()).collect()
        });
        let mut cache = HashMap::new();
        let mut models = HashMap::new();
        vis.grenades = walk
            .sim
            .weapon_classes
            .iter()
            .map(|w| {
                let name = w.ordnance.geometry.as_deref()?;
                Some(*models.entry(name).or_insert_with(|| {
                    let index = vis.weapons.len();
                    let game = &scene.manifest.source_game;
                    let native = format!("{game}/locals");
                    let native_model = assets_dir
                        .join(format!("side_{native}"))
                        .join("weapons")
                        .join(format!("{name}.glb"));
                    let model_game = if !walk.native_teams.is_empty() && pak::is_file(&native_model)
                    {
                        &native
                    } else {
                        game
                    };
                    vis.weapons.push(Self::load_weapon(
                        Some(&mut *r),
                        assets_dir,
                        model_game,
                        name,
                        &mut cache,
                    ));
                    index
                }))
            })
            .collect();
        vis.set_turrets(tp::rigged_turrets(scene));
        vis.set_walker_rigs(if walk.walker_rigs.is_empty() {
            crate::viewer::walker_rigs(&scene.manifest, assets_dir, &walk.vehicle_geometry)
        } else {
            walk.walker_rigs.clone()
        });
        vis.outlines = std::array::from_fn(|t| {
            (0..walk.sim.classes[t].len())
                .map(|c| vis.measure_outline(t as u8, c as u8))
                .collect()
        });
        vis
    }

    fn load_native_team(
        &mut self,
        r: &mut Renderer,
        root: &Path,
        game: &str,
        team: u8,
        template: &TeamTemplate,
    ) {
        let mut unused = template.clone();
        unused.classes.clear();
        let teams = [template.clone(), unused];
        let mut added = Self::load_with_side(r, root, game, &teams, true);
        let bodies = self.assets.len();
        let weapons = self.weapons.len();
        let mut per_class = std::mem::take(&mut added.per_class[0]);
        let mut held = std::mem::take(&mut added.held[0]);
        for draw in &mut per_class {
            draw.body = draw.body.map(|i| i + bodies);
            draw.weapon = draw.weapon.map(|i| i + weapons);
        }
        for (_, model) in held.iter_mut().flatten() {
            *model = model.map(|i| i + weapons);
        }
        self.assets.extend(added.assets);
        self.weapons.extend(added.weapons);
        self.native.insert(
            team,
            NativeDraw {
                per_class,
                held,
                anim: std::mem::take(&mut added.anim[0]),
                skins: std::mem::take(&mut added.skins[0]),
                low_skins: std::mem::take(&mut added.low_skins[0]),
            },
        );
    }

    /// Load every class model both teams name, from
    /// `assets_dir/side_<game>/soldiers/`. Classes whose GLB is missing take
    /// their team's first class's asset; a team with no model at all keeps
    /// the marker box.
    ///
    /// `game` is the MAP's source game — the same disc `game::roster` took
    /// the classes off, so the `GeometryName` a class carries is looked up in
    /// the side bake that holds it. Both discs bake all four sides, and the
    /// faction's OWN game is the wrong answer for a BF2 GCW map or a BF1 CW
    /// one: over the 322 class slots on such (map, faction) pairs the model
    /// resolves 309 times in the map's own side dir and 225 times in the
    /// faction's native one. That historical census's 13 misses are
    /// `cis_walk_droideka`; fieldable droidekas now use `DroidekaVisuals`
    /// in battle and in the spawn carousel, rather than infantry skins.
    pub fn load(
        r: &mut Renderer,
        assets_dir: &Path,
        game: &str,
        teams: &[TeamTemplate; 2],
    ) -> Self {
        Self::load_with_side(r, assets_dir, game, teams, false)
    }

    fn load_with_side(
        r: &mut Renderer,
        assets_dir: &Path,
        game: &str,
        teams: &[TeamTemplate; 2],
        native: bool,
    ) -> Self {
        let side_game = if native {
            format!("{game}/locals")
        } else {
            game.to_owned()
        };
        let plan = DrawPlan::of(teams);
        let mut cache: HashMap<String, Option<TextureHandle>> = HashMap::new();
        let bodies = plan
            .bodies
            .iter()
            .map(|geometry| {
                Self::load_body(Some(&mut *r), assets_dir, &side_game, geometry, &mut cache)
            })
            .collect();
        let weapons: Vec<WeaponAsset> = plan
            .weapons
            .iter()
            .map(|wg| Self::load_weapon(Some(&mut *r), assets_dir, &side_game, wg, &mut cache))
            .collect();
        let flash = if native {
            None
        } else {
            FlashModel::load(r, assets_dir, game)
        };
        let mut s = Self::assemble(plan, teams, bodies, weapons, flash);
        s.bf2 = game.eq_ignore_ascii_case("bf2");
        if !native {
            s.fp = super::first_person::FirstPersonVisuals::load(
                Some(&mut *r),
                assets_dir,
                game,
                teams,
                &mut cache,
            );
            if s.bf2 {
                s.load_sabers(Some(&mut *r), assets_dir, teams);
            }
        }
        // Each class's skins over its body's parts (card USKN).
        let side = assets_dir.join(format!("side_{side_game}"));
        for (team, t) in teams.iter().enumerate() {
            let mut rows = Vec::new();
            let mut low_rows = Vec::new();
            for (k, c) in t.classes.iter().enumerate() {
                let body = s.per_class[team].get(k).and_then(|d| d.body);
                let groups = body.map_or(&[][..], |b| s.assets[b].groups.as_slice());
                let row: Vec<Option<TextureHandle>> = groups
                    .iter()
                    .map(|g| {
                        let tex = c.skins.of_group(g.as_deref()?)?;
                        Self::texture(r, &side, Some(tex), [0.0; 2], &mut cache)
                    })
                    .collect();
                let low_groups = body
                    .and_then(|b| s.assets[b].low.as_ref())
                    .map_or(&[][..], |a| a.groups.as_slice());
                low_rows.push(
                    low_groups
                        .iter()
                        .map(|g| {
                            let tex = c.skins.of_group(g.as_deref()?)?;
                            Self::texture(r, &side, Some(tex), [0.0; 2], &mut cache)
                        })
                        .collect(),
                );
                rows.push(if row.iter().any(Option::is_some) {
                    row
                } else {
                    Vec::new()
                });
            }
            s.skins[team] = rows;
            s.low_skins[team] = low_rows;
        }
        s
    }

    /// The map's rigged armed buildings (`turret_pose::rigged_turrets`):
    /// a gunner seated in one is drawn at his seat (card VTURG).
    pub fn set_turrets(&mut self, turrets: Vec<tp::RiggedTurret>) {
        self.turrets = turrets;
    }

    /// Each vehicle class's walker rig (`viewer::walker_rigs`): a rider on
    /// an animated node rides it as it walks (card VRIDE).
    pub fn set_walker_rigs(
        &mut self,
        rigs: Vec<Option<std::sync::Arc<crate::viewer::walker_pose::WalkerRig>>>,
    ) {
        self.walker_rigs = rigs;
    }

    /// Bring every hover's rider numbers to this tick: one drive update of
    /// the tick's length per tick while it has a pilot, with his input
    /// (`tp::HoverNine::step`); a fresh vehicle starts at the PS2's -1.
    fn step_hover_nines(&mut self, walk: &WalkSim) {
        let tick = walk.sim.tick;
        let vs = &walk.sim.vehicles;
        self.hover_nine.resize(vs.len(), None);
        for (slot, v) in vs.iter().enumerate() {
            if !v.alive || v.kind != core::vehicle::VehicleKind::Hover {
                self.hover_nine[slot] = None;
                continue;
            }
            let fresh = HoverClock {
                gen: v.gen,
                tick,
                nine: tp::HoverNine::START,
            };
            let c = self.hover_nine[slot].get_or_insert(fresh);
            if c.gen != v.gen {
                *c = fresh;
            }
            let piloted = v.pilot_seat().is_some_and(|s| s.occupant.is_some());
            if piloted {
                let no_strafe = walk
                    .sim
                    .hover_params
                    .get(v.class as usize)
                    .and_then(Option::as_ref)
                    .is_some_and(|p| p.strafe_speed < 1e-4);
                let i = v.input;
                for _ in 0..tick.saturating_sub(c.tick).min(30) {
                    c.nine = c.nine.step(
                        [i.throttle, i.strafe, i.turn],
                        no_strafe,
                        self.bf2,
                        core::TICK_DT,
                    );
                }
            }
            c.tick = tick;
        }
    }

    /// The animators without a renderer (a census, a test): every body's
    /// skeleton, clips and sets, every hand weapon's nodes; nothing to draw.
    pub fn load_cpu(assets_dir: &Path, game: &str, teams: &[TeamTemplate; 2]) -> Self {
        let plan = DrawPlan::of(teams);
        let mut cache = HashMap::new();
        let bodies = plan
            .bodies
            .iter()
            .map(|geometry| Self::load_body(None, assets_dir, game, geometry, &mut cache))
            .collect();
        let weapons: Vec<WeaponAsset> = plan
            .weapons
            .iter()
            .map(|wg| Self::load_weapon(None, assets_dir, game, wg, &mut cache))
            .collect();
        let mut s = Self::assemble(plan, teams, bodies, weapons, None);
        s.bf2 = game.eq_ignore_ascii_case("bf2");
        s.fp = super::first_person::FirstPersonVisuals::load(
            None, assets_dir, game, teams, &mut cache,
        );
        if s.bf2 {
            s.load_sabers(None, assets_dir, teams);
        }
        s
    }

    /// `bodies` per `plan.bodies` entry, `None` for one that is not baked.
    fn assemble(
        plan: DrawPlan,
        teams: &[TeamTemplate; 2],
        bodies: Vec<Option<SoldierAsset>>,
        weapons: Vec<WeaponAsset>,
        flash: Option<FlashModel>,
    ) -> Self {
        // A body that is not baked leaves no asset: plan index -> `assets`.
        let mut assets: Vec<SoldierAsset> = Vec::new();
        let bodies: Vec<Option<usize>> = bodies
            .into_iter()
            .map(|a| {
                assets.push(a?);
                Some(assets.len() - 1)
            })
            .collect();
        let mut per_class = plan.per_class;
        for (i, t) in teams.iter().enumerate() {
            for c in per_class[i].iter_mut() {
                c.body = c.body.and_then(|b| bodies[b]);
            }
            // A class with no model of its own borrows the team's FIRST
            // class's; only a team whose first class has none keeps the box.
            let fallback = per_class[i].first().and_then(|c| c.body);
            for (k, c) in per_class[i].iter_mut().enumerate() {
                if c.body.is_none() && fallback.is_some() {
                    println!(
                        "soldier {}: no baked model — drawing {} instead",
                        t.classes[k].name, t.classes[0].name
                    );
                    c.body = fallback;
                }
            }
        }
        Self {
            own_model: Default::default(),
            outlines: Default::default(),
            lod_view: Default::default(),
            low_skins: Default::default(),
            assets,
            weapons,
            sabers: [Vec::new(), Vec::new()],
            saber_trails: Vec::new(),
            grenades: Vec::new(),
            per_class,
            native: Default::default(),
            held: plan.held,
            anim: teams
                .each_ref()
                .map(|t| t.classes.iter().map(|c| c.anim.clone()).collect()),
            skins: [Vec::new(), Vec::new()],
            units: Vec::new(),
            flash,
            turrets: Vec::new(),
            bf2: false,
            walker_rigs: Vec::new(),
            hover_nine: Vec::new(),
            cloth_meshes: HashMap::new(),
            fp: Default::default(),
        }
    }

    /// A part's texture from `side/textures/`, uploaded once per name.
    pub(super) fn texture(
        r: &mut Renderer,
        side: &Path,
        material: Option<&str>,
        scroll: [f32; 2],
        cache: &mut HashMap<String, Option<TextureHandle>>,
    ) -> Option<TextureHandle> {
        let name = material?.to_ascii_lowercase();
        let key = match scroll == [0.0; 2] {
            true => name.clone(),
            false => format!("{name}@{scroll:?}"),
        };
        if !cache.contains_key(&key) {
            let dir = side.join("textures");
            let h = crate::viewer::upload_texture_file_scrolling(r, &dir, &name, scroll)
                .unwrap_or_else(|e| {
                    eprintln!("{e:#}");
                    None
                });
            cache.insert(key.clone(), h);
        }
        cache[&key]
    }

    /// One skinned soldier model, or `None` if it is not baked. Without a
    /// renderer it has no meshes.
    pub(super) fn load_body(
        mut r: Option<&mut Renderer>,
        assets_dir: &Path,
        game: &str,
        geometry: &str,
        cache: &mut HashMap<String, Option<TextureHandle>>,
    ) -> Option<SoldierAsset> {
        // The side bake knows no map, so a BF1 part's sun-coloured
        // specular pass resolves against the lighting the map's
        // `upload_scene` already put on the renderer: every caller loads
        // the soldiers after the scene.
        let side = assets_dir.join(format!("side_{game}"));
        let glb = side.join("soldiers").join(format!("{geometry}.glb"));
        let asset = match load_visual_skinned_glb(&glb) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("soldier {geometry}: {e:#}");
                return None;
            }
        };
        let mut meshes = Vec::new();
        let mut groups = Vec::new();
        if let Some(r) = r.as_deref_mut() {
            match crate::viewer::upload_skinned(r, &asset.mesh) {
                Ok(parts) => {
                    // The model's override groups; a model baked before
                    // card USKN has none.
                    let path = side.join("soldiers").join(format!("{geometry}.skins.ron"));
                    let skins: manifest::SoldierSkins = pak::read_to_string(&path)
                        .ok()
                        .and_then(|t| ron::from_str(&t).ok())
                        .unwrap_or_default();
                    let mats: Vec<Option<&str>> = parts.iter().map(|p| p.1).collect();
                    groups = part_groups(&mats, &skins.groups);
                    for (k, (mesh, mat, layer)) in parts.into_iter().enumerate() {
                        let tex = Self::texture(r, &side, mat, [0.0; 2], cache);
                        meshes.push((mesh, tex, layer, part_spec(r, asset.mesh.spec_pass(k))));
                    }
                }
                Err(e) => {
                    eprintln!("soldier {geometry}: {e:#}");
                    return None;
                }
            }
        }
        let hp_weapons = asset.skeleton.index_of("hp_weapons");
        let is_low = geometry.ends_with(".low");
        let rig = (!is_low)
            .then(|| Self::load_rig(&side, geometry, &asset))
            .flatten();
        let cloths = if is_low {
            Vec::new()
        } else {
            Self::load_cloths(&side, geometry, &asset)
        };
        let (lod, low) = Self::load_low(r, assets_dir, game, geometry, &asset, cache);
        println!(
            "soldier {geometry}: {} bones, {} clips, {} parts",
            asset.skeleton.len(),
            asset.clips.len(),
            meshes.len()
        );
        Some(SoldierAsset {
            low,
            lod,
            asset,
            meshes,
            rig,
            hp_weapons,
            groups,
            cloths,
        })
    }

    /// A baked soldier's animation sets over its skeleton and clips.
    pub fn load_rig(side: &Path, geometry: &str, asset: &SkinnedAsset) -> Option<Rig> {
        let path = manifest::anim_sets::path(side, geometry);
        let sets = pak::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| ron::from_str(&t).map_err(|e| e.to_string()));
        match sets {
            Ok(sets) => Some(Rig::new(sets, &asset.skeleton, &asset.clips)),
            Err(e) => {
                eprintln!("soldier {geometry}: {}: {e} (not animated)", path.display());
                None
            }
        }
    }

    /// One hand weapon: a static GLB under weapons/, textures beside, and its
    /// nodes. A model that is not baked gives no parts (no weapon drawn);
    /// without a renderer only the nodes are read.
    pub(super) fn load_weapon(
        r: Option<&mut Renderer>,
        assets_dir: &Path,
        game: &str,
        wg: &str,
        cache: &mut HashMap<String, Option<TextureHandle>>,
    ) -> WeaponAsset {
        let mut side = assets_dir.join(format!("side_{game}"));
        let mut parts = Vec::new();
        let mut bounds = None;
        let mut wpath = side.join("weapons").join(format!("{wg}.glb"));
        let common = assets_dir.join(format!("common_{game}"));
        let shared = common.join("models").join(format!("{wg}.glb"));
        if !pak::is_file(&wpath) && pak::is_file(&shared) {
            side = common;
            wpath = shared;
        }
        if let Some(r) = r {
            match crate::assets::load_glb(&wpath) {
                Ok(cpu) => match crate::viewer::upload_submeshes(r, &cpu) {
                    Ok(up) => {
                        bounds = super::position_bounds(&cpu.positions);
                        for (k, (mesh, mat, layer, _)) in up.into_iter().enumerate() {
                            let tex = Self::texture(r, &side, mat, cpu.uv_scroll(k), cache);
                            parts.push((mesh, tex, layer, part_spec(r, cpu.spec_pass(k))));
                        }
                    }
                    Err(e) => eprintln!("weapon {wg}: {e:#}"),
                },
                Err(e) => eprintln!("weapon {wg}: {e:#} (no weapon drawn)"),
            }
        }
        let nodes = pak::read_to_string(manifest::weapon_nodes_path(&side, wg))
            .ok()
            .and_then(|text| ron::from_str(&text).ok())
            .unwrap_or_default();
        println!("weapon {wg}: {} parts", parts.len());
        WeaponAsset {
            parts,
            nodes,
            bounds,
        }
    }

    /// What a unit's class draws: its own pair, or the team's first-class
    /// body fallback that `load` resolved for it.
    fn class_draw(&self, team: u8, class: u8) -> ClassDraw {
        let row = self
            .native
            .get(&team)
            .map_or(&self.per_class[usize::from(team.min(1))], |side| {
                &side.per_class
            });
        let i = usize::from(class).min(row.len().saturating_sub(1));
        row.get(i).copied().unwrap_or_default()
    }

    /// Index into `assets` for a unit's class.
    pub(super) fn asset_index(&self, team: u8, class: u8) -> Option<usize> {
        self.class_draw(team, class).body
    }

    /// The asset a unit draws with.
    pub fn asset_for(&self, team: u8, class: u8) -> Option<&SoldierAsset> {
        self.assets.get(self.asset_index(team, class)?)
    }

    /// The hand weapon a unit draws while its class slot `slot` is in hand,
    /// if that slot has a model ([`DrawPlan::weapon_in_slot`]).
    pub(super) fn weapon_for(&self, team: u8, class: u8, slot: u8) -> Option<&WeaponAsset> {
        let row = self
            .native
            .get(&team)
            .map_or(&self.held[usize::from(team.min(1))], |side| &side.held);
        let i = usize::from(class).min(row.len().saturating_sub(1));
        let held = row.get(i).map_or(&[][..], Vec::as_slice);
        self.weapons.get(held_or_primary(
            held,
            self.class_draw(team, class).weapon,
            slot,
        )?)
    }

    pub fn has_model(&self, team: u8) -> bool {
        self.native
            .get(&team)
            .map_or(&self.per_class[usize::from(team.min(1))], |side| {
                &side.per_class
            })
            .iter()
            .any(|c| c.body.is_some())
    }

    /// The tick the sim just stepped: who fired (the muzzle flash's clock).
    /// [`SoldierVisuals::update`] reads the last tick's shots itself; a frame
    /// that steps several ticks calls this after each
    /// (`WalkSim::advance_with`) so an earlier tick's shot still flashes.
    pub fn note_shots(&mut self, walk: &WalkSim) {
        self.note_throws(walk);
        // A melee throw's clip starts at the trigger: the saber leaves
        // InitialSalvoDelay into it, at the frame the bake samples for the
        // release (`hero_blades::throw_bind`). A saber still out cannot be
        // thrown again.
        for (v, u) in self.units.iter_mut().zip(&walk.sim.units) {
            let (wc, _, state) = u.active_weapon();
            let emitter = (u.alive
                && u.mount.is_none()
                && state.state == core::weapon::FireState::Firing
                && walk.sim.weapon_classes[wc]
                    .force
                    .as_ref()
                    .is_some_and(|p| matches!(p.kind, core::hero_weapon::ForceKind::Emitter(_))))
            .then_some(wc);
            v.cast.firing(walk.sim.tick, emitter);
            let firing = u.alive
                && u.mount.is_none()
                && state.state == core::weapon::FireState::Firing
                && walk.sim.weapon_classes[wc].saber_throw.is_some();
            let out = u.weapon.saber_flight.is_some()
                || u.held.iter().any(|h| h.state.saber_flight.is_some());
            if firing && !v.saber_throwing && !out {
                v.cast.start(walk.sim.tick, wc);
            }
            v.saber_throwing = firing;
        }
        for (event, e) in walk.sim.events.iter().enumerate() {
            if let core::sim::Event::Hit { victim, .. } = e {
                if let Some(v) = self
                    .units
                    .get_mut(victim.index as usize)
                    .filter(|v| v.gen == victim.gen)
                {
                    v.animator.note_hit(&walk.sim, event);
                }
            }
            if let core::sim::Event::Despawn { unit } = e {
                if let Some(v) = self
                    .units
                    .get_mut(unit.index as usize)
                    .filter(|v| v.gen == unit.gen)
                {
                    v.gone = true;
                }
            }
            if let core::sim::Event::Ability {
                unit, weapon_class, ..
            } = e
            {
                if walk.sim.weapon_classes[*weapon_class as usize]
                    .force
                    .as_ref()
                    .is_some_and(|p| matches!(p.kind, core::hero_weapon::ForceKind::Emitter(_)))
                {
                    if let Some(v) = self
                        .units
                        .get_mut(unit.index as usize)
                        .filter(|v| v.gen == unit.gen)
                    {
                        v.cast.start(walk.sim.tick, *weapon_class as usize);
                    }
                }
            }
            if let core::sim::Event::Repair {
                unit, weapon_class, ..
            } = e
            {
                if let Some(v) = self.units.get_mut(unit.index as usize) {
                    if v.gen == unit.gen && v.repair_tick != Some(walk.sim.tick) {
                        v.repair_shot = Some(*weapon_class as usize);
                        v.repair_tick = Some(walk.sim.tick);
                    }
                }
            }
            let core::sim::Event::Shot {
                unit, weapon_class, ..
            } = e
            else {
                continue;
            };
            if walk.sim.is_item_gun(*weapon_class as usize) {
                continue;
            }
            let (Some(v), Some(u)) = (
                self.units.get_mut(unit.index as usize),
                walk.sim.units.get(unit.index as usize),
            ) else {
                continue;
            };
            if v.gen == u.gen {
                v.shot_tick = Some(walk.sim.tick);
                v.shot_class = Some(*weapon_class as usize);
                v.shot_ordinal =
                    crate::walker_shots::ordinal(walk, *unit, *weapon_class as usize, event);
            }
        }
    }

    pub(super) fn sync_units(&mut self, walk: &WalkSim) {
        let n = walk.sim.units.len();
        if self.units.len() < n {
            self.units.resize_with(n, UnitVisual::default);
        }
        for (v, u) in self.units.iter_mut().zip(&walk.sim.units) {
            if v.gen != u.gen {
                *v = UnitVisual {
                    gen: u.gen,
                    ..Default::default()
                };
            }
            if u.alive {
                v.seated = u.mount.is_some();
            } else if v.seated && !u.body {
                v.gone = true;
            }
        }
    }

    /// Units to draw this frame: (unit index, asset, model matrix, palette).
    pub(super) fn posed(
        &self,
        walk: &WalkSim,
    ) -> Vec<(usize, &SoldierAsset, Mat4, std::sync::Arc<[Mat4]>)> {
        self.posed_for(walk, false)
    }

    pub(super) fn posed_for(
        &self,
        walk: &WalkSim,
        view: bool,
    ) -> Vec<(usize, &SoldierAsset, Mat4, std::sync::Arc<[Mat4]>)> {
        let mut out = Vec::new();
        for (i, u) in walk.sim.units.iter().enumerate() {
            if u.droideka.is_some()
                || (view && self.lod_view.choice(i) == super::lod::Choice::Hidden)
            {
                continue;
            }
            let Some(sa) = self.asset_for(u.team, u.spec.class) else {
                continue;
            };
            let Some(v) = self.units.get(i) else { continue };
            // A droid that burst leaves no body to draw (card DROIDDEATH);
            // a cloaked soldier is not drawn (card CLOAK).
            if !u.cloak.visible()
                || v.gen != u.gen
                || (!u.alive && (u.burst || v.gone || v.dead_for > CORPSE_SECS))
            {
                continue;
            }
            if !u.alive && !v.animator.posed() {
                continue;
            }
            // A rider is hidden but in a seat that shows him (cards VTURG,
            // VRIDE), who is drawn in every view, his own too.
            let model = match (u.mount, v.seat_model) {
                (Some(mount), Some(m)) => walk
                    .sim
                    .vehicle(mount.vehicle)
                    .filter(|v| v.kind == core::vehicle::VehicleKind::Walker)
                    .and_then(|_| {
                        rider_now(&self.walker_rigs, &self.hover_nine, walk, mount, self.bf2)
                    })
                    .map_or(m, |r| r.model),
                (Some(_), None) => continue,
                // In the scope view the player's own body is not drawn.
                (None, _) if i == walk.player.index as usize && walk.player_body_hidden() => {
                    continue
                }
                (None, _) => model_of(&u.soldier),
            };
            let at = self.asset_index(u.team, u.spec.class).unwrap_or(0);
            let compute = || -> std::sync::Arc<[Mat4]> {
                let pose = v.animator.pose(&sa.asset.skeleton);
                sa.asset.skeleton.palette(&pose.locals()).into()
            };
            let (posed_for, memo) = v.palette.get_or_init(|| (at, compute()));
            // A class change within a tick, before the animator has seen it.
            let palette = if *posed_for == at {
                memo.clone()
            } else {
                compute()
            };
            out.push((i, sa, model, palette));
        }
        out
    }

    /// Current model-space launch frames, including a just-burst soldier
    /// whose body is already hidden. Falls back to the bind pose before posing.
    pub fn chunk_nodes(&self, walk: &WalkSim, unit: core::unit::UnitHandle) -> Vec<(String, Mat4)> {
        let Some(u) = walk
            .sim
            .units
            .get(unit.index as usize)
            .filter(|u| u.gen == unit.gen)
        else {
            return Vec::new();
        };
        let Some(sa) = self.asset_for(u.team, u.spec.class) else {
            return Vec::new();
        };
        let Some(v) = self
            .units
            .get(unit.index as usize)
            .filter(|v| v.gen == unit.gen)
        else {
            return Vec::new();
        };
        let sk = &sa.asset.skeleton;
        let pose = v.animator.pose(sk);
        let palette = sk.palette(&pose.locals());
        sk.names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), palette[i] * sk.global_bind[i]))
            .collect()
    }

    /// Unit `unit`'s drawn body this frame as world triangles (its skinned
    /// mesh under the pose drawn), and its named bones in the world; `None`
    /// when it is not drawn. A test's read of where the drawn body is.
    #[allow(clippy::type_complexity)]
    pub fn drawn_body(
        &self,
        walk: &WalkSim,
        unit: usize,
    ) -> Option<(Vec<[Vec3; 3]>, Vec<(String, Vec3)>)> {
        let (_, sa, model, palette) = self.posed(walk).into_iter().find(|p| p.0 == unit)?;
        let m = &sa.asset.mesh;
        let skinned: Vec<Vec3> = m
            .positions
            .iter()
            .zip(m.joints.iter().zip(&m.weights))
            .map(|(p, (j, w))| {
                let p = Vec3::from(*p);
                (0..4)
                    .filter(|&k| w[k] > 0.0)
                    .map(|k| (model * palette[usize::from(j[k])]).transform_point3(p) * w[k])
                    .sum()
            })
            .collect();
        let live_cloth =
            !sa.cloths.is_empty() && self.units[unit].cloth_states.len() == sa.cloths.len();
        let mut tris = Vec::new();
        for (k, &(start, count, _, _)) in m.submeshes.iter().enumerate() {
            if live_cloth && sa.cloths.iter().any(|c| c.raw.part == k) {
                continue;
            }
            tris.extend(
                m.indices[start as usize..(start + count) as usize]
                    .chunks_exact(3)
                    .map(|t| [t[0], t[1], t[2]].map(|i| skinned[i as usize])),
            );
        }
        if live_cloth {
            let globals: Vec<_> = palette
                .iter()
                .zip(&sa.asset.skeleton.global_bind)
                .map(|(p, b)| *p * *b)
                .collect();
            for (def, state) in sa.cloths.iter().zip(&self.units[unit].cloth_states) {
                let vertices = state.vertices(def, &globals);
                tris.extend(def.raw.triangles.iter().map(|t| {
                    t.map(|i| model.transform_point3(Vec3::from(vertices[i as usize].position)))
                }));
            }
        }
        let sk = &sa.asset.skeleton;
        let bones = (0..sk.names.len())
            .map(|b| {
                let at = (model * palette[b] * sk.global_bind[b]).transform_point3(Vec3::ZERO);
                (sk.names[b].clone(), at)
            })
            .collect();
        Some((tris, bones))
    }

    /// Every unit drawn this frame and its model matrix (a test's and a
    /// census's read of who is drawn where).
    pub fn drawn(&self, walk: &WalkSim) -> Vec<(usize, Mat4)> {
        self.posed(walk)
            .into_iter()
            .map(|(i, _, m, _)| (i, m))
            .collect()
    }

    /// Loaded in-flight model for a registered weapon class, for draw diagnostics.
    pub fn ordnance_model(&self, class: usize) -> Option<&WeaponAsset> {
        self.grenades
            .get(class)
            .copied()
            .flatten()
            .and_then(|i| self.weapons.get(i))
    }

    /// The hand weapon of every drawn unit, at the posed `hp_weapons` bone:
    /// `unit × palette[hp] × bind_global[hp]` (palette = global × inverse bind).
    pub fn weapon_draws(&self, walk: &WalkSim) -> Vec<render::DrawCmd> {
        let mut out = self.saber_hilts(walk);
        for (i, sa, model, palette) in self.posed_for(walk, true) {
            let Some(hp) = sa.hp_weapons else { continue };
            let u = &walk.sim.units[i];
            // A seated gunner holds no hand weapon (B1:114261-114266,
            // B2:155344-155345).
            if u.mount.is_some() {
                continue;
            }
            if self.saber_visual(u.team, u.spec.class).is_some() {
                continue;
            }
            let active = u.active_weapon();
            let class = &walk.sim.weapon_classes[active.0];
            if class.mine.is_some() && class.offhand_weapon {
                let hand = super::in_hand(&walk.sim.weapon_classes, active);
                if !hand.hidden {
                    if let (Some(bone), Some(weapon)) = (
                        sa.asset.skeleton.index_of("bone_l_hand"),
                        self.weapon_for(u.team, u.spec.class, hand.slot),
                    ) {
                        weapon.part_draws(
                            model * palette[bone] * sa.asset.skeleton.global_bind[bone],
                            &mut out,
                        );
                    }
                }
            }
            let hand = self.hand_of(u, &walk.sim.weapon_classes);
            if hand.hidden {
                continue;
            }
            let Some(weapon) = self.weapon_for(u.team, u.spec.class, hand.slot) else {
                continue;
            };
            let m = model * palette[hp] * sa.asset.skeleton.global_bind[hp];
            weapon.part_draws(m, &mut out);
        }
        let sim = &walk.sim;
        let thrown = sim
            .stickies
            .iter()
            .map(|g| (g.weapon_class, Mat4::from_translation(g.pos)))
            .chain(sim.droids.iter().map(|d| {
                // The model faces +Z, as the engine's models do (a soldier is
                // drawn turned by π for the same reason); the droid's aim
                // looks down -Z at yaw 0. The half turn about y is exact.
                let turn = glam::Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
                let m = Mat4::from_rotation_translation(d.rotation() * turn, d.pos);
                (d.weapon_class, m)
            }))
            .chain(sim.turrets.iter().map(|t| {
                let m = Mat4::from_rotation_translation(t.rotation(), t.pos);
                (t.weapon_class, m)
            }))
            .chain(
                sim.guided
                    .iter()
                    .filter(|r| r.owner != walk.player)
                    .map(|r| {
                        let m = Mat4::from_rotation_translation(r.drawn_rotation(), r.pos);
                        (r.weapon_class, m)
                    }),
            )
            // Missiles in flight (card UORD) draw as their class's
            // `ordnance.geometry`, the way a thrown grenade does; the bolt's
            // own module says where they are and which way they point.
            .chain(crate::bolt_vis::missiles_in_flight(walk))
            .chain(sim.mines.iter().map(|m| {
                let at = Mat4::from_rotation_translation(m.rotation, m.flight.pos);
                (m.flight.weapon_class, at)
            }));
        for (class, at) in thrown {
            let model = self.grenades.get(class).copied().flatten();
            if let Some(model) = model.and_then(|i| self.weapons.get(i)) {
                model.part_draws(at, &mut out);
            }
        }
        out
    }

    /// Every drawn unit's `bone_ribcage` in the world, by unit index: where
    /// a jet's effect rides (card JETP2, `crate::fx` `jet.rs`). `None` for a
    /// unit drawn without a model or without the bone, or in a seat.
    pub fn jet_frames(&self, walk: &WalkSim) -> Vec<Option<Mat4>> {
        let mut out = vec![None; walk.sim.units.len()];
        for (i, sa, model, palette) in self.posed(walk) {
            if walk.sim.units[i].mount.is_some() {
                continue;
            }
            let Some(b) = sa.asset.skeleton.index_of(JET_BONE) else {
                continue;
            };
            out[i] = Some(model * palette[b] * sa.asset.skeleton.global_bind[b]);
        }
        out
    }

    pub fn burn_frames(&self, walk: &WalkSim) -> Vec<Vec<Option<Mat4>>> {
        let mut out = vec![Vec::new(); walk.sim.units.len()];
        for (i, sa, model, palette) in self.posed(walk) {
            let Some((_, effects)) = walk.sim.burn_effects(&walk.sim.units[i]) else {
                continue;
            };
            out[i] = effects
                .iter()
                .map(|(bone, _)| {
                    let b = sa.asset.skeleton.index_of(bone)?;
                    Some(model * palette[b] * sa.asset.skeleton.global_bind[b])
                })
                .collect();
        }
        out
    }

    /// Every drawn unit's weapon fire point in the world, by unit index:
    /// `hp_weapons` × the weapon in hand's `FirePointName` node (the `hp_weapons`
    /// origin when the weapon model has no such node, §4.1). `None` for a
    /// unit drawn without a model or a hand node. Where the muzzle flash
    /// sits and the drawn bolt starts (`crate::bolt_vis`). Specialized rigs
    /// overlay their animated points with `DroidekaVisuals::fire_points`.
    pub fn fire_points(&self, walk: &WalkSim) -> Vec<Option<Vec3>> {
        let mut out: Vec<Option<Vec3>> = walk
            .sim
            .units
            .iter()
            .map(|u| u.droideka_gun(1).filter(|_| u.alive && !u.burst))
            .collect();
        for (i, sa, model, palette) in self.posed(walk) {
            if let (Some(hp), None) = (sa.hp_weapons, walk.sim.units[i].mount) {
                let u = &walk.sim.units[i];
                let in_hand = self.hand_of(u, &walk.sim.weapon_classes);
                let name = walk
                    .sim
                    .weapon_classes
                    .get(in_hand.class)
                    .and_then(|w| w.flash.as_ref())
                    .map_or(core::flash::DEFAULT_FIRE_POINT, |f| f.fire_point.as_str());
                let hand = model * palette[hp] * sa.asset.skeleton.global_bind[hp];
                out[i] = Some(match self.weapon_for(u.team, u.spec.class, in_hand.slot) {
                    Some(w) => fire_point(hand, &w.nodes, name),
                    None => hand.transform_point3(Vec3::ZERO),
                });
            }
        }
        for (i, u) in walk.sim.units.iter().enumerate() {
            if let Some(frame) = u
                .mount
                .filter(|_| u.alive)
                .and_then(|m| self.walker_fire_frame(walk, m))
            {
                out[i] = Some(frame.w_axis.truncate());
            }
        }
        if let Some((i, p)) = self.fp.muzzle(walk) {
            out[i] = Some(p);
        }
        out
    }

    pub fn force_points(&self, walk: &WalkSim) -> Vec<Option<Vec3>> {
        let mut out = vec![None; walk.sim.units.len()];
        for (i, sa, model, palette) in self.posed(walk) {
            if let Some(hand) = sa.asset.skeleton.index_of("bone_l_hand") {
                out[i] = Some(
                    (model * palette[hand] * sa.asset.skeleton.global_bind[hand])
                        .transform_point3(Vec3::ZERO),
                );
            }
        }
        out
    }

    fn walker_fire_frame(&self, walk: &WalkSim, mount: core::vehicle::Mount) -> Option<Mat4> {
        let v = walk.sim.vehicle(mount.vehicle)?;
        let unit = v.seats.get(mount.seat as usize)?.occupant?;
        let shot = self
            .units
            .get(unit.index as usize)
            .filter(|u| u.gen == unit.gen)
            .and_then(|visual| visual.shot_class.map(|class| (class, visual.shot_ordinal)));
        let (class, ordinal) = match shot {
            Some(shot) => shot,
            None => {
                let setup = walk.sim.seat_setup.get(v.class as usize)?.as_ref()?;
                (
                    setup
                        .guns
                        .get(mount.seat as usize)?
                        .as_ref()?
                        .weapons
                        .first()?
                        .class,
                    0,
                )
            }
        };
        crate::walker_shots::frame(walk, unit, class, ordinal)
    }

    pub fn flash_draws(&self, walk: &WalkSim) -> Vec<ParticleDraw> {
        self.flash_draws_at(walk, &self.fire_points(walk))
    }

    /// Flash particles at the drawn muzzle points, including specialized rigs.
    pub fn flash_draws_at(&self, walk: &WalkSim, points: &[Option<Vec3>]) -> Vec<ParticleDraw> {
        let Some(model) = &self.flash else {
            return Vec::new();
        };
        let now = walk.sim.tick as f64 * f64::from(core::TICK_DT);
        let mut out = Vec::new();
        for (i, u) in walk.sim.units.iter().enumerate() {
            let (Some(v), Some(Some(p))) = (self.units.get(i), points.get(i)) else {
                continue;
            };
            let Some(shot) = v.shot_tick else { continue };
            let Some(look) = walk
                .sim
                .weapon_classes
                .get(if u.mount.is_some() {
                    v.shot_class.unwrap_or(usize::MAX)
                } else {
                    self.hand_of(u, &walk.sim.weapon_classes).class
                })
                .and_then(|w| w.flash.as_ref())
                .filter(|f| f.draws() && f.model.as_deref() == Some(model.name.as_str()))
            else {
                continue;
            };
            let t = look.t(shot as f64 * f64::from(core::TICK_DT), now);
            let Some(color) = look.color_at(t) else {
                continue;
            };
            let dir = u
                .mount
                .and_then(|m| self.walker_fire_frame(walk, m))
                .map_or_else(
                    || u.aim_frame().0,
                    |m| m.z_axis.truncate().normalize_or_zero(),
                );
            let m = core::flash::matrix(dir, look.radius, *p);
            out.extend(model.draws(m, color, *p));
        }
        out
    }

    /// Skinned draws for every unit with a model (alive, or a fresh corpse),
    /// each part's specular second pass beside its base.
    /// Soldier models face +Z; sim yaw 0 faces −Z, hence the extra π.
    pub fn draws(&self, walk: &WalkSim) -> Vec<SkinnedDrawCmd> {
        let mut out = Vec::new();
        for (i, sa, model, palette) in self.posed_for(walk, true) {
            let u = &walk.sim.units[i];
            let live_cloth =
                self.units[i].cloth_states.len() == sa.cloths.len() && !sa.cloths.is_empty();
            let low = self.lod_view.choice(i) == super::lod::Choice::Low;
            let body = if low {
                sa.low.as_deref().unwrap_or(sa)
            } else {
                sa
            };
            self.body_draws(
                body,
                (u.team, u.spec.class),
                model,
                &palette,
                live_cloth && !low,
                &mut out,
            );
        }
        out
    }

    /// The parts of `team`'s class `class` (its skins over body `sa`) at
    /// `model` posed by `palette`, each with its specular second pass.
    fn body_draws(
        &self,
        sa: &SoldierAsset,
        (team, class): (u8, u8),
        model: Mat4,
        palette: &std::sync::Arc<[Mat4]>,
        live_cloth: bool,
        out: &mut Vec<SkinnedDrawCmd>,
    ) {
        let low = sa.asset.name.ends_with("_low1") || sa.asset.name.ends_with(".low");
        let rows = if low { &self.low_skins } else { &self.skins };
        let row = self
            .native
            .get(&team)
            .map_or(&rows[usize::from(team.min(1))], |side| {
                if low {
                    &side.low_skins
                } else {
                    &side.skins
                }
            });
        let skin = row
            .get(usize::from(class).min(row.len().saturating_sub(1)))
            .map_or(&[][..], Vec::as_slice);
        for (k, (mesh, tex, layer, spec)) in sa.meshes.iter().enumerate() {
            if live_cloth && sa.cloths.iter().any(|c| c.raw.part == k) {
                continue;
            }
            let base = SkinnedDrawCmd {
                mesh: *mesh,
                texture: skin.get(k).copied().flatten().or(*tex),
                color: [1.0; 4],
                model,
                palette: palette.clone(),
                layer: *layer,
                light: render::LightPath::Character,
                // BF2's local lights reach the soldier (card BPNT).
                lit: Some(crate::point_light::soldier_sphere(
                    model.transform_point3(glam::Vec3::ZERO),
                )),
            };
            out.extend(with_spec(base, *spec));
        }
    }

    /// Steps `animator` `dt` on the standing clip of the class's primary's set.
    fn still_step(&self, sa: &SoldierAsset, animator: &mut Animator, team: u8, class: u8, dt: f32) {
        if let Some(rig) = &sa.rig {
            let row = &self.anim[usize::from(team.min(1))];
            let anim = row.get(usize::from(class).min(row.len().saturating_sub(1)));
            let bank = anim
                .and_then(|a| a.banks.first())
                .map_or("rifle", |(_, b)| b.as_str());
            let input = AnimInput {
                stance: core::soldier::Stance::Stand,
                vel: glam::Vec2::ZERO,
                yaw: 0.0,
                mouse_aim: false,
                max_speed: 1.0,
                root_scale: anim.map_or(1.0, |a| a.root_scale),
                dead: false,
                hero_dead: false,
                hard_lock: false,
                set: rig.weapon_set(anim.map_or("human", |a| &a.family), bank),
                fired: false,
                throw: None,
                reloading: false,
                reload_time: 0.0,
                alert: false,
                aim: (0.0, 0.0),
            };
            animator.step(rig, &sa.asset.clips, &sa.asset.skeleton, &input, dt);
        }
    }

    /// What the class's standing soldier covers seen from above, in metres from
    /// his feet, body and weapon through his idle clip. `None` when the class
    /// has no body here (a droideka's is its own model) or its weapon no bounds
    /// (the screens' fit, `spawn_preview`).
    pub fn still_outline(&self, team: u8, class: u8) -> Option<super::Outline> {
        match self.outlines[usize::from(team.min(1))].get(usize::from(class)) {
            Some(known) => *known,
            None => self.measure_outline(team, class),
        }
    }

    /// [`Self::still_outline`], walked through the idle clip: tens of
    /// milliseconds for a team.
    fn measure_outline(&self, team: u8, class: u8) -> Option<super::Outline> {
        const IDLE_SAMPLES: usize = 24;
        const IDLE_STEP: f32 = 0.25;
        let own = &self.own_model[usize::from(team.min(1))];
        if own.get(usize::from(class)).copied().unwrap_or(false) {
            return None;
        }
        let sa = self.asset_for(team, class)?;
        let m = &sa.asset.mesh;
        let weapon = self
            .class_draw(team, class)
            .weapon
            .and_then(|w| self.weapons.get(w));
        let weapon_box = match (sa.hp_weapons, weapon) {
            (Some(hp), Some(w)) => Some((hp, w.bounds?)),
            _ => None,
        };
        let mut animator = Animator::default();
        let mut outline = super::Outline::default();
        for _ in 0..IDLE_SAMPLES {
            self.still_step(sa, &mut animator, team, class, IDLE_STEP);
            let palette = sa
                .asset
                .skeleton
                .palette(&animator.pose(&sa.asset.skeleton).locals());
            for (p, j) in m.positions.iter().zip(&m.joints) {
                let bone = palette.get(usize::from(j[0]))?;
                outline.add(bone.transform_point3(glam::Vec3::from(*p)));
            }
            if let Some((hp, (a, b))) = weapon_box {
                let hand = palette[hp] * sa.asset.skeleton.global_bind[hp];
                for k in 0..8 {
                    let pick = |bit: usize, a: f32, b: f32| if k >> bit & 1 == 0 { a } else { b };
                    outline.add(hand.transform_point3(glam::Vec3::new(
                        pick(0, a.x, b.x),
                        pick(1, a.y, b.y),
                        pick(2, a.z, b.z),
                    )));
                }
            }
        }
        Some(outline)
    }

    /// A soldier of `team`'s class `class` standing at `model` with his
    /// primary weapon in hand, outside the sim (the spawn screens' preview
    /// and line-up, card SSELB): `animator` is his own, stepped `dt` on the
    /// standing clip of the primary's set. `None` when the class has no
    /// model. Returns his body's draws and the weapon's.
    pub fn still_draws(
        &self,
        animator: &mut Animator,
        team: u8,
        class: u8,
        model: Mat4,
        dt: f32,
    ) -> Option<(Vec<SkinnedDrawCmd>, Vec<render::DrawCmd>)> {
        let sa = self.asset_for(team, class)?;
        self.still_step(sa, animator, team, class, dt);
        let palette: std::sync::Arc<[Mat4]> = sa
            .asset
            .skeleton
            .palette(&animator.pose(&sa.asset.skeleton).locals())
            .into();
        let mut body = Vec::new();
        self.body_draws(sa, (team, class), model, &palette, false, &mut body);
        let mut gun = Vec::new();
        let weapon = self
            .class_draw(team, class)
            .weapon
            .and_then(|w| self.weapons.get(w));
        if let (Some(hp), Some(w)) = (sa.hp_weapons, weapon) {
            w.part_draws(
                model * palette[hp] * sa.asset.skeleton.global_bind[hp],
                &mut gun,
            );
        }
        Some((body, gun))
    }
}
