//! Gameplay layout of a baked map: command posts (with their capture /
//! control regions and spawn paths) pulled out of the manifest's instances,
//! regions and paths. Runtime-only view over `assets::Manifest`.
//!
//! CP instance PROP keys (TAT1, hashes verified against the manifest):
//! Team 0xA2FD7D0C, Label 0xF69717FD, CaptureRegion 0xC6C161F9,
//! ControlRegion 0x805C1F8A, SpawnPath 0xDDE0B14F, ValueBleed 0x4EBF97DA.
//! Region/path names are matched case-insensitively (props say
//! "CP10Capture", regions are stored "cp10capture").

use crate::assets::Manifest;
use crate::mission::MissionDef;
use core::announcer::{self, PostEvent, PostLines};
use core::nav::{Arc, Barrier, Hint, Hub, NavGraph, RouteTable};
use core::region::Region;
use core::route_patch::RoutePatches;
use glam::{Vec2, Vec3};
use std::path::Path;

pub const PROP_TEAM: u32 = 0xA2FD_7D0C;
pub const PROP_LABEL: u32 = 0xF697_17FD;
pub const PROP_CAPTURE_REGION: u32 = 0xC6C1_61F9;
pub const PROP_CONTROL_REGION: u32 = 0x805C_1F8A;
pub const PROP_SPAWN_PATH: u32 = 0xDDE0_B14F;
pub const PROP_VALUE_BLEED: u32 = 0x4EBF_97DA;
/// `AISpawnWeight` (BF2; re-command-walker-post §6 [O]).
pub const PROP_AI_SPAWN_WEIGHT: u32 = 0xE072_82C2;

/// Per-faction attack / defend post values, in FACTION ORDER `[Alliance,
/// Empire, Republic, CIS, Locals]`. These ten are RAW PROP HASHES read off the
/// disc (analysis/re-cp-capture.md §176-177), retained as provenance. They
/// equal `classdb::fnv1a_lowercase` of the key names: the game hashes each
/// byte ORed with 0x20, which differs from ordinary ASCII lowercasing.
pub const PROP_VALUE_ATK: [u32; 5] = [
    0xA340_D6B5,
    0x4DB4_4670,
    0x9E78_B410,
    0x00E7_3245,
    0x573C_98A4,
];
pub const PROP_VALUE_DEF: [u32; 5] = [
    0x44A2_C4A6,
    0xA867_9457,
    0x8F55_40CB,
    0x5BF8_5AE8,
    0xE689_EF93,
];
/// A post that authors no value for a faction takes the command-post class
/// constructor's own default.
pub const VALUE_DEFAULT: i32 = 100;

#[derive(Debug, Clone)]
pub struct SpawnNode {
    pub pos: Vec3,
    /// Yaw in the engine's convention (0 = −Z, positive left), derived from
    /// the stored (w, x, y, z) quaternion as 2·atan2(y, w). PROVISIONAL
    /// mapping of the editor's facing to ours.
    pub yaw: f32,
}

#[derive(Debug, Clone)]
pub struct SpawnPath {
    pub name: String,
    pub nodes: Vec<SpawnNode>,
}

#[derive(Debug, Clone)]
pub struct CommandPostDef {
    pub name: String,
    pub label: String,
    pub class: String,
    /// Starting owner: 0 = neutral, 1/2 = the two main teams (mission
    /// numbering; core::battle uses 0-based team indices = this − 1).
    pub team: u8,
    pub pos: Vec3,
    pub capture: Option<Region>,
    pub control: Option<Region>,
    pub spawn_path: Option<String>,
    pub value_bleed: i32,
    /// `AISpawnWeight`: the post's weight in a bot's draw of where to spawn
    /// (BF2 only; the constructor's 100 where the instance authors none —
    /// on the discs one instance does, BF2 KAS2's `baserush`, 1000 — and
    /// clamped ≥ 0 as the PS2's setter).
    pub ai_spawn_weight: f32,
    /// `Value_ATK_<faction>` per faction, in [`PROP_VALUE_ATK`]'s order
    /// (Alliance, Empire, Republic, CIS, Locals); [`VALUE_DEFAULT`] where the
    /// post authors none.
    pub value_atk: [i32; 5],
    /// `Value_DEF_<faction>`, same order and same default.
    pub value_def: [i32; 5],
    pub major: bool,
    /// The post's `VO_<Listener>_<Subject><Event>` lines (card ANNC,
    /// analysis/re-announcer.md §2.1); empty on a post that authors none.
    pub vo: PostLines,
}

#[derive(Debug, Clone, Default)]
pub struct MapLayout {
    /// The DISC's own map name (`Manifest::map`: `tat1`, `hoth`, `kamino1`),
    /// not the baked directory's. Read by [`crate::walk::WalkSim::rescue_spawn`]
    /// for the one map whose start it must not move.
    pub map: String,
    pub cps: Vec<CommandPostDef>,
    pub paths: Vec<SpawnPath>,
    /// Every named world region the manifest could resolve, in manifest order.
    /// Looked up by name (case-insensitively) — see [`MapLayout::region`].
    pub regions: Vec<(String, Region)>,
}

/// What [`MapLayout::apply_mission`] did, for the caller's log.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MissionApplied {
    /// Command posts left in the layout afterwards.
    pub kept: usize,
    /// Dropped because the mission's `CommandPost:New` roster does not name
    /// them (BF2 only; a BF1 mission names none and drops nothing).
    pub dropped_by_roster: usize,
    /// Dropped by a `KillObject` call.
    pub killed: usize,
    /// Re-owned by `SetObjectTeam` / `SetProperty(cp, "Team", …)`.
    pub reassigned: usize,
    /// `AddDeathRegion` volumes, resolved, in the mission's call order.
    pub death_regions: Vec<Region>,
    /// `AddDeathRegion` names the manifest has no region for.
    pub missing_death_regions: Vec<String>,
    /// The soldiers' fly-height ceiling for `Sim::fly_ceiling` (card JETP):
    /// the mission's `SetMaxPlayerFlyHeight`, else the engine's 200.
    pub fly_ceiling: f32,
    /// The four `Set*FlyHeight` calls, for `Sim::flight_limits`.
    pub flight_limits: core::sim::FlightLimits,
}

/// Build the sim's planning graph (`core::nav`) from a baked map.
///
/// - The PLAN is the manifest's own: [`crate::assets::Scene::apply_mode_layer`]
///   has already replaced the base plan with the mode layer's when that layer
///   ships one, and merged both worlds' hints and barriers.
/// - The route tables come from the sidecar the manifest NAMES. Never rebuild
///   that path from a layer name: layer names are not unique on the discs, so
///   the bake de-duplicates the keys (`routes_hoth_objective_2.bin`).
/// - `None` when the map authors no graph, or when its graph has no hubs (BF2
///   KAM1 ships an EMPTY `plan` chunk).
///
/// A sidecar that is missing or does not parse is reported and the graph is
/// kept with an EMPTY table: the hubs, arcs and barriers are still good, and
/// `core::nav`'s BFS fallback can route on them.
pub fn nav_graph(m: &Manifest, dir: &Path) -> Option<NavGraph> {
    let nav = m.nav.as_ref()?;
    let hints = nav
        .hints
        .iter()
        .map(|h| Hint {
            name: h.name.clone(),
            kind: h.kind,
            pos: Vec3::from(h.xform[3]),
            rows: [h.xform[0], h.xform[1], h.xform[2]],
            radius: h.radius,
            mode: h.mode,
            // `PrimaryStance` / `SecondaryStance` are BITMASKS and their
            // reading is PER GAME (BF2 masks `& 3` and so never prones; BF1
            // keeps the raw byte) — RE lead-verified, see
            // `core::commander::stance_mask`. This is the one place that holds
            // both the authored value and the manifest's `source_game`, so the
            // sim reads a mask and needs no game flag of its own.
            stance_mask: core::commander::stance_mask(h.primary_stance, &m.source_game),
            secondary_stance_mask: core::commander::stance_mask(h.secondary_stance, &m.source_game),
            peek_sides: core::commander::peek_sides(h.secondary_stance),
            command_post: h.command_post.clone(),
        })
        .collect();
    // A `BARR` is a 2-D oriented rectangle in XZ: `pos` is the loader's world
    // position (the bake already negated z), the rotation rows are as stored
    // and scale SIZE by their own lengths exactly as `regn` does, and SIZE is
    // read as HALF extents ([H] — RE_LOG "`BARR`" leaves half-vs-full open and
    // names half, like `regn`, as the hypothesis).
    let barriers = nav
        .barriers
        .iter()
        .map(|b| {
            let axis = |row: [f32; 3], fallback: Vec2| {
                let v = Vec3::from(row);
                let len = v.length().max(1e-6);
                let xz = Vec2::new(v.x, v.z);
                let n = xz.length();
                if n > 1e-6 {
                    (xz / n, len)
                } else {
                    (fallback, len)
                }
            };
            let (ax, lx) = axis(b.rows[0], Vec2::X);
            let (az, lz) = axis(b.rows[2], Vec2::Y);
            Barrier {
                centre: Vec2::new(b.pos[0], b.pos[2]),
                axes: [ax, az],
                half: Vec2::new(b.size[0] * lx, b.size[2] * lz),
                mask: b.flags,
                // Every barrier on the discs loads ENABLED (blocking). A BF2
                // mission's ScriptPostLoad Enable/DisableBarriers calls are
                // replayed by `MissionDef::apply_barriers` (BNDS); the calls
                // a script makes later in the round are not modelled.
                enabled: true,
            }
        })
        .collect();
    let plan = nav.plan.as_ref()?;
    let hubs: Vec<Hub> = plan
        .hubs
        .iter()
        .map(|h| Hub {
            pos: Vec3::from(h.pos),
            radius: h.radius,
            arcs: h.arcs.clone(),
        })
        .collect();
    let arcs: Vec<Arc> = plan
        .arcs
        .iter()
        .map(|a| Arc {
            a: a.a,
            b: a.b,
            type_mask: a.type_mask,
            flags: a.flags,
        })
        .collect();
    let path = dir.join(&plan.routes);
    let routes = match pak::read(&path).map_err(|e| e.to_string()) {
        Ok(bytes) => match RouteTable::parse(&bytes) {
            Ok(t) if t.hub_count == hubs.len() => t,
            Ok(t) => {
                eprintln!(
                    "nav {}: {} has {} hubs, the plan has {} — routing on the BFS fallback",
                    m.map,
                    plan.routes,
                    t.hub_count,
                    hubs.len()
                );
                RouteTable::empty(hubs.len())
            }
            Err(e) => {
                eprintln!("nav {}: {} — routing on the BFS fallback", m.map, e);
                RouteTable::empty(hubs.len())
            }
        },
        Err(e) => {
            eprintln!(
                "nav {}: {} unreadable ({e}) — routing on the BFS fallback",
                m.map,
                path.display()
            );
            RouteTable::empty(hubs.len())
        }
    };
    let mut g = NavGraph::new(hubs, arcs, routes, barriers, hints)?;
    // BF2 tests a hub as an XZ disc, BF1 as a sphere (card LOC3).
    g.hub_discs = m.source_game == "bf2";
    // The names a mission's Enable/DisableBarriers address (card BNDS).
    g.barrier_names = nav
        .barriers
        .iter()
        .map(|b| crate::classdb::fnv1a_lowercase(&b.name))
        .collect();
    // BF2's `plnp` patches (`core::route_patch`): attached with the mask at 0
    // — the PS2 constructor's — so the table routes as authored until a
    // mission blocks a group. A sidecar that does not fit is reported and
    // dropped: the authored table is still the all-open one.
    if let Some(pp) = plan.plnp.as_ref() {
        let attached = pak::read(dir.join(&pp.records))
            .map_err(|e| e.to_string())
            .and_then(|b| RoutePatches::parse(pp.groups.clone(), &b))
            .and_then(|p| g.attach_patches(p));
        if let Err(e) = attached {
            eprintln!("nav {}: {} — no route patches ({e})", m.map, pp.records);
        }
    }
    Some(g)
}

fn prop(props: &[(u32, String)], key: u32) -> Option<&str> {
    props.iter().find(|p| p.0 == key).map(|p| p.1.as_str())
}

/// The five per-faction values under `keys`, in the same order, defaulting to
/// [`VALUE_DEFAULT`] where the prop is absent or unparseable.
fn faction_values(props: &[(u32, String)], keys: &[u32; 5]) -> [i32; 5] {
    let mut out = [VALUE_DEFAULT; 5];
    for (i, &k) in keys.iter().enumerate() {
        if let Some(v) = prop(props, k).and_then(|s| s.trim().parse().ok()) {
            out[i] = v;
        }
    }
    out
}

/// A post's VO table out of its instance props: every
/// `VO_<Listener>_<Subject><Event>` over the four commander factions.
fn post_lines(props: &[(u32, String)]) -> PostLines {
    let mut out = PostLines::default();
    for l in 0..announcer::FACTIONS.len() {
        for s in 0..announcer::FACTIONS.len() {
            for e in PostEvent::ALL {
                let key = crate::classdb::fnv1a_lowercase(&announcer::post_property(l, s, e));
                if let Some(line) = prop(props, key).filter(|v| !v.is_empty()) {
                    out.set(l, s, e, line);
                }
            }
        }
    }
    out
}

impl MapLayout {
    pub fn from_manifest(m: &Manifest) -> MapLayout {
        let region = |name: &str| -> Option<Region> {
            let r = m
                .regions
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(name))?;
            Region::from_rows(&r.shape, r.rotation, r.translation, r.size)
        };
        let mut cps = Vec::new();
        for inst in &m.instances {
            let Some(cap_name) = prop(&inst.props, PROP_CAPTURE_REGION) else {
                continue; // not a command post
            };
            cps.push(CommandPostDef {
                name: inst.name.clone(),
                label: prop(&inst.props, PROP_LABEL).unwrap_or("").to_owned(),
                class: inst.class.clone(),
                team: prop(&inst.props, PROP_TEAM)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0),
                pos: Vec3::from(inst.translation),
                capture: region(cap_name),
                control: prop(&inst.props, PROP_CONTROL_REGION).and_then(region),
                spawn_path: prop(&inst.props, PROP_SPAWN_PATH).map(str::to_owned),
                // BF1: scalar ValueBleed (class default 100). BF2: per-faction
                // ValueBleed_<faction> (identical across main factions in
                // retail; class default 10) — first present wins.
                value_bleed: prop(&inst.props, PROP_VALUE_BLEED)
                    .or_else(|| {
                        ["Alliance", "Empire", "Republic", "CIS"]
                            .iter()
                            .find_map(|f| {
                                prop(
                                    &inst.props,
                                    crate::classdb::fnv1a_lowercase(&format!("ValueBleed_{f}")),
                                )
                            })
                    })
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(100),
                ai_spawn_weight: prop(&inst.props, PROP_AI_SPAWN_WEIGHT)
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .map_or(core::cp::AI_SPAWN_WEIGHT_DEFAULT, |w| w.max(0.0)),
                value_atk: faction_values(&inst.props, &PROP_VALUE_ATK),
                value_def: faction_values(&inst.props, &PROP_VALUE_DEF),
                major: inst.class.contains("major"),
                vo: post_lines(&inst.props),
            });
        }
        let regions = m
            .regions
            .iter()
            .filter_map(|r| {
                Region::from_rows(&r.shape, r.rotation, r.translation, r.size)
                    .map(|v| (r.name.clone(), v))
            })
            .collect();
        let paths = m
            .paths
            .iter()
            .map(|p| SpawnPath {
                name: p.name.clone(),
                nodes: p
                    .nodes
                    .iter()
                    .map(|(pos, q)| SpawnNode {
                        pos: Vec3::from(*pos),
                        // `fmath`, not std: the yaw reaches `Sim::hash` via
                        // the respawn, so the host libm must not pick it.
                        yaw: 2.0 * core::fmath::atan2(q[2], q[0]),
                    })
                    .collect(),
            })
            .collect();
        MapLayout {
            map: m.map.clone(),
            cps,
            paths,
            regions,
        }
    }

    pub fn path(&self, name: &str) -> Option<&SpawnPath> {
        self.paths
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// A named world region (`wrld regn`), matched case-insensitively — props
    /// say "CP10Capture" where the region is stored "cp10capture".
    pub fn region(&self, name: &str) -> Option<Region> {
        self.regions
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, r)| *r)
    }

    /// Fold a mission's command-post edits into this layout, and resolve its
    /// death regions.
    ///
    /// - `posts` (the BF2 `ObjectiveConquest` roster, in `CommandPost:New`
    ///   order) RESTRICTS the layout to the posts it names; the LAYOUT's own
    ///   order is kept, not the roster's, because the sim indexes posts by
    ///   position and the AI's `target_cp` is that index. Empty (every BF1
    ///   mission) keeps the world layer's posts as they are.
    /// - `killed` (`KillObject`) drops posts outright — BF2 Hoth's `CP7`.
    /// - `cps` re-owns posts (1-based team as stored in
    ///   [`CommandPostDef::team`]; 0 = neutral). BF2 Hoth's world layer
    ///   authors all five `hoth_cps` to one side and only the mission splits
    ///   them.
    /// - `death_regions` (`AddDeathRegion`) are resolved by name.
    ///
    /// Names are matched case-insensitively; an unknown one is reported once
    /// and skipped.
    pub fn apply_mission(&mut self, m: &MissionDef) -> MissionApplied {
        let named = |list: &[String], name: &str| list.iter().any(|k| k.eq_ignore_ascii_case(name));

        let before = self.cps.len();
        if !m.posts.is_empty() {
            self.cps.retain(|c| named(&m.posts, &c.name));
            for want in &m.posts {
                if !self.cps.iter().any(|c| c.name.eq_ignore_ascii_case(want)) {
                    eprintln!(
                        "mission {}/{}: roster names {want:?}, which the world layer has no post for",
                        m.map, m.mode
                    );
                }
            }
        }
        let dropped_by_roster = before - self.cps.len();

        let before = self.cps.len();
        self.cps.retain(|c| !named(&m.killed, &c.name));
        let killed = before - self.cps.len();

        let mut reassigned = 0;
        for (name, team) in &m.cps {
            match self
                .cps
                .iter_mut()
                .find(|c| c.name.eq_ignore_ascii_case(name))
            {
                Some(c) => {
                    c.team = *team;
                    reassigned += 1;
                }
                None => eprintln!(
                    "mission {}/{}: cp {name:?} re-owned by the script is not in the layout",
                    m.map, m.mode
                ),
            }
        }

        let mut death_regions = Vec::new();
        let mut missing_death_regions = Vec::new();
        for name in &m.death_regions {
            match self.region(name) {
                Some(r) => death_regions.push(r),
                None => {
                    eprintln!(
                        "mission {}/{}: death region {name:?} is not in the manifest",
                        m.map, m.mode
                    );
                    missing_death_regions.push(name.clone());
                }
            }
        }

        MissionApplied {
            kept: self.cps.len(),
            dropped_by_roster,
            killed,
            reassigned,
            death_regions,
            missing_death_regions,
            fly_ceiling: m
                .max_player_fly_height
                .unwrap_or(core::soldier::DEFAULT_FLY_CEILING),
            flight_limits: core::sim::FlightLimits {
                player: [
                    m.min_player_fly_height.unwrap_or(-200.0),
                    m.max_player_fly_height.unwrap_or(200.0),
                ],
                ai: [
                    m.min_fly_height.unwrap_or(-200.0),
                    m.max_fly_height.unwrap_or(200.0),
                ],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five posts (`cp1`..`cp5`), each with its own capture sphere, plus one
    /// `sarlac01` region for the death-region path.
    fn manifest() -> Manifest {
        let mut instances = String::new();
        for i in 1..=5 {
            instances.push_str(&format!(
                r#"(class: "cp_c", name: "CP{i}",
                    rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: ({i},0,0),
                    props: [(2734521612, "1"), (3334562297, "cp{i}capture")]),"#
            ));
        }
        let regions: String = (1..=5)
            .map(|i| {
                format!(
                    r#"(shape: "sphere", name: "cp{i}capture",
                        rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: ({i},0,0),
                        size: (0.1,4,0.1), props: []),"#
                )
            })
            .collect();
        ron::from_str(&format!(
            r#"(
                map: "t", source_game: "bf2",
                terrain_name: None, sky_name: None, terrain_glb: None,
                models: [], textures: [],
                classes: [(name: "cp_c", base: "commandpost", geometry: None)],
                geometries: [],
                regions: [{regions}
                    (shape: "sphere", name: "sarlac01",
                     rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,-5,0),
                     size: (0.1,9,0.1), props: []),
                ],
                instances: [{instances}],
            )"#
        ))
        .expect("manifest parses")
    }

    fn mission(src: &str) -> MissionDef {
        ron::from_str(src).expect("mission parses")
    }

    /// PROP 0xA2FD7D0C = Team, 0xC6C161F9 = CaptureRegion — the two literals
    /// the synthetic manifest above spells in decimal.
    #[test]
    fn synthetic_manifest_uses_the_real_prop_hashes() {
        assert_eq!(PROP_TEAM, 2_734_521_612);
        assert_eq!(PROP_CAPTURE_REGION, 3_334_562_297);
    }

    /// The ten hashes are transcribed as LITERALS (the disc numbers the Phase
    /// B addendum lists) — and, MEASURED here, all ten are also exactly
    /// `classdb::fnv1a_lowercase` of their key names. The addendum says they
    /// are "NOT fnv1a of the key names"; that is a correction, not a change:
    /// the port's own lowercasing is a byte-wise `| 0x20`, under which
    /// `Value_ATK_Alliance` hashes to 0xA340D6B5. (A `to_lowercase()`-style
    /// hash — `_` is 0x5F, `_ | 0x20` is 0x7F — does NOT, which is the
    /// "naive lookup finds nothing" the addendum ran into.) The literals stay
    /// because they are what the disc data was read with; this test pins BOTH
    /// facts so neither can drift.
    #[test]
    fn the_value_prop_hashes_are_the_disc_literals_and_the_name_hashes() {
        for (i, f) in ["Alliance", "Empire", "Republic", "CIS", "Locals"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                PROP_VALUE_ATK[i],
                crate::classdb::fnv1a_lowercase(&format!("Value_ATK_{f}")),
                "ATK {f}"
            );
            assert_eq!(
                PROP_VALUE_DEF[i],
                crate::classdb::fnv1a_lowercase(&format!("Value_DEF_{f}")),
                "DEF {f}"
            );
        }
        assert_eq!(PROP_VALUE_ATK[0], 0xA340_D6B5);
        assert_eq!(PROP_VALUE_DEF[4], 0xE689_EF93);
        // No hash collides with any other prop key we read.
        let mut all = vec![
            PROP_TEAM,
            PROP_LABEL,
            PROP_CAPTURE_REGION,
            PROP_CONTROL_REGION,
            PROP_SPAWN_PATH,
            PROP_VALUE_BLEED,
        ];
        all.extend(PROP_VALUE_ATK);
        all.extend(PROP_VALUE_DEF);
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "prop key hashes are distinct");
    }

    /// A hand-built instance carrying the raw hashes: the five ATK and five
    /// DEF values land in faction order, and a faction the post says nothing
    /// about takes the class ctor's 100.
    #[test]
    fn the_raw_value_props_reach_command_post_def() {
        let props: String = PROP_VALUE_ATK
            .iter()
            .enumerate()
            // Locals (row 4) is left out on purpose: it must default.
            .take(4)
            .map(|(i, h)| format!(r#"({h}, "{}"),"#, (i as i32 + 1) * 10))
            .chain(
                PROP_VALUE_DEF
                    .iter()
                    .enumerate()
                    .map(|(i, h)| format!(r#"({h}, "{}"),"#, (i as i32 + 1) * 5)),
            )
            .collect();
        let m: Manifest = ron::from_str(&format!(
            r#"(
                map: "t", source_game: "bf2",
                terrain_name: None, sky_name: None, terrain_glb: None,
                models: [], textures: [],
                classes: [], geometries: [],
                regions: [(shape: "sphere", name: "cpxcapture",
                    rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,0,0),
                    size: (0.1,4,0.1), props: [])],
                instances: [(class: "cp_c", name: "CPX",
                    rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,0,0),
                    props: [(3334562297, "cpxcapture"), {props}]),],
            )"#
        ))
        .expect("manifest parses");
        let l = MapLayout::from_manifest(&m);
        assert_eq!(l.cps.len(), 1);
        assert_eq!(l.cps[0].value_atk, [10, 20, 30, 40, VALUE_DEFAULT]);
        assert_eq!(l.cps[0].value_def, [5, 10, 15, 20, 25]);
        // A post that authors none of them is all defaults.
        let plain = MapLayout::from_manifest(&manifest());
        assert!(plain
            .cps
            .iter()
            .all(|c| c.value_atk == [VALUE_DEFAULT; 5] && c.value_def == [VALUE_DEFAULT; 5]));
    }

    #[test]
    fn apply_mission_restricts_to_roster_and_kills_and_reassigns() {
        let mut l = MapLayout::from_manifest(&manifest());
        assert_eq!(l.cps.len(), 5);
        let m = mission(
            r#"(
                map: "t", mode: "con", source_game: "bf2",
                teams: [(faction: "imp", units: 16, tickets: 150),
                        (faction: "all", units: 16, tickets: 150)],
                ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25,
                era: Some("gcw"),
                posts: ["cp1", "CP2", "cp3", "cp4"],
                killed: ["CP4"],
                cps: [("cp1", 1), ("cp3", 2)],
                death_regions: ["Sarlac01", "nosuchregion"],
            )"#,
        );
        let a = l.apply_mission(&m);
        // cp5 is off the roster; cp4 is on it but killed.
        assert_eq!(
            (a.kept, a.dropped_by_roster, a.killed, a.reassigned),
            (3, 1, 1, 2)
        );
        // LAYOUT order is preserved — the sim indexes posts by position.
        let names: Vec<&str> = l.cps.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["CP1", "CP2", "CP3"]);
        assert_eq!(l.cps[0].team, 1);
        assert_eq!(l.cps[1].team, 1, "untouched posts keep the world's team");
        assert_eq!(l.cps[2].team, 2);
        // One death region resolves, one does not.
        assert_eq!(a.death_regions.len(), 1);
        assert_eq!(a.missing_death_regions, ["nosuchregion"]);
        assert!(a.death_regions[0].contains(Vec3::new(0.0, -5.0, 0.0)));
    }

    /// BF1 inlines conquest and hands no roster to an objective, so its
    /// missions must leave the world layer's posts exactly as they are.
    #[test]
    fn bf1_mission_with_empty_roster_keeps_world_posts() {
        let mut l = MapLayout::from_manifest(&manifest());
        let m = mission(
            r#"(
                map: "t", mode: "con", source_game: "bf1",
                teams: [(faction: "imp", units: 25, tickets: 250),
                        (faction: "all", units: 25, tickets: 250)],
                ai_alive_cap: 25, spawn_delay: 10.0, spawn_interval: 0.25,
                era: Some("gcw"),
            )"#,
        );
        let a = l.apply_mission(&m);
        assert_eq!(
            (a.kept, a.dropped_by_roster, a.killed, a.reassigned),
            (5, 0, 0, 0)
        );
        assert_eq!(l.cps.len(), 5);
        assert!(a.death_regions.is_empty());
        // No SetMaxPlayerFlyHeight: the engine's ceiling (card JETP).
        assert_eq!(a.fly_ceiling, 200.0);
        let m = mission(
            r#"(
                map: "t", mode: "con", source_game: "bf2",
                teams: [(faction: "imp", units: 20, tickets: 150),
                        (faction: "all", units: 20, tickets: 150)],
                ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25,
                max_player_fly_height: Some(14.0),
            )"#,
        );
        assert_eq!(l.apply_mission(&m).fly_ceiling, 14.0);
    }

    /// The per-game half of the `PrimaryStance` bitmask rule lives HERE, in
    /// the one place that holds both the authored value and the manifest's
    /// `source_game` (RE lead-verified — `core::commander::stance_mask`): the
    /// SAME hint is prone-only on a BF1 map and stand on a BF2 one.
    #[test]
    fn a_hints_stance_mask_is_read_per_game() {
        let dir = std::env::temp_dir().join(format!("openbf-hint-stance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        // PrimaryStance 4, SecondaryStance 0x1a = stance bits 2 (prone) + both
        // peek sides (8 | 16).
        let src = |game: &str| {
            format!(
                r#"(
            map: "t", source_game: "{game}",
            terrain_name: None, sky_name: None, terrain_glb: None,
            models: [], textures: [], classes: [], geometries: [],
            regions: [], instances: [],
            nav: Some((
                plan: Some((
                    hubs: [(name: "Hub0", pos: (0, 0, 0), radius: 2.0, arcs: [])],
                    arcs: [], routes: "nav/none.bin",
                )),
                hints: [(name: "hint0", kind: 2,
                         xform: ((1,0,0),(0,1,0),(0,0,1),(5,0,6)),
                         primary_stance: Some(4), secondary_stance: Some(26))],
            )),
        )"#
            )
        };
        let mask = |game: &str| {
            let m: Manifest = ron::from_str(&src(game)).expect("manifest parses");
            let g = nav_graph(&m, &dir).expect("graph");
            let h = &g.hints[0];
            (h.stance_mask, h.secondary_stance_mask, h.peek_sides)
        };
        use core::commander::{MASK_CROUCH, MASK_PRONE, MASK_STAND};
        // BF1 keeps the raw byte: 4 is prone alone, and 26 & 7 = 2 is crouch.
        assert_eq!(mask("bf1"), (MASK_PRONE, MASK_CROUCH, 3));
        // BF2 masks `& 3`: 4 becomes 0 -> the ctor default, stand. Its AI never
        // prones, and the peek sides are read the same way on both.
        assert_eq!(mask("bf2"), (MASK_STAND, MASK_CROUCH, 3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A manifest with a graph: the sidecar is opened through the path the
    /// MANIFEST names (layer keys are de-duplicated, so it cannot be rebuilt
    /// from a layer name), and a `BARR` becomes an XZ rectangle.
    #[test]
    fn nav_graph_reads_the_sidecar_the_manifest_names() {
        use core::nav::Mover;
        let dir =
            std::env::temp_dir().join(format!("openbf-nav-graph-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nav")).expect("temp dir");
        // routes_h.bin: two hubs, five types; SOLDIER hub 0 has one row whose
        // entry for hub 1 is weight 31 slot 0.
        let mut bin = vec![2u8, 0, 5];
        for t in 0..5 {
            for h in 0..2 {
                if t == 0 && h == 0 {
                    bin.extend_from_slice(&[1, 0, 31 << 3]);
                } else {
                    bin.push(0);
                }
            }
        }
        std::fs::write(dir.join("nav/routes_h.bin"), &bin).expect("sidecar");

        let src = r#"(
            map: "t", source_game: "bf2",
            terrain_name: None, sky_name: None, terrain_glb: None,
            models: [], textures: [], classes: [], geometries: [],
            regions: [], instances: [],
            nav: Some((
                plan: Some((
                    hubs: [
                        (name: "Hub0", pos: (0, 3, 0), radius: 2.0, arcs: [0]),
                        (name: "Hub1", pos: (20, 3, 0), radius: 2.0, arcs: [0]),
                    ],
                    arcs: [(name: "Connection0", a: 0, b: 1, type_mask: 1, flags: 0)],
                    routes: "nav/routes_h.bin",
                )),
                hints: [(name: "hint0", kind: 1,
                         xform: ((1,0,0),(0,1,0),(0,0,1),(5,0,6)))],
                barriers: [(name: "barrier0", pos: (10, 0, 5),
                            rows: ((1,0,0),(0,1,0),(0,0,1)),
                            size: (4, 2, 1), flags: 1)],
            )),
        )"#;
        let m: Manifest = ron::from_str(src).expect("manifest parses");
        let g = nav_graph(&m, &dir).expect("graph");
        assert_eq!((g.hubs.len(), g.arcs.len()), (2, 1));
        assert_eq!(g.routes.hub_count, 2);
        assert_eq!(
            g.next_hop(0, 1, Mover::SOLDIER, None, None, 0),
            Some((0, 1))
        );
        // The hint comes across too (`core::commander` picks from them).
        assert_eq!(g.hints.len(), 1);
        assert_eq!(g.hints[0].pos, Vec3::new(5.0, 0.0, 6.0));
        // It authors no `PrimaryStance`, and a hint that authors none still
        // asks for STAND (both hint ctors default to it) — the mask is never 0.
        assert_eq!(g.hints[0].stance_mask, core::commander::MASK_STAND);
        assert_eq!(
            g.hints[0].secondary_stance_mask,
            core::commander::MASK_STAND
        );
        assert_eq!(g.hints[0].peek_sides, 0);
        // The barrier is an XZ rectangle at (pos.x, pos.z) with HALF sizes.
        assert_eq!(g.barriers.len(), 1);
        let b = g.barriers[0];
        assert_eq!(
            (b.centre, b.half, b.mask),
            (Vec2::new(10.0, 5.0), Vec2::new(4.0, 1.0), 1)
        );
        assert!(b.contains(Vec2::new(13.0, 5.5)));
        assert!(!b.contains(Vec2::new(13.0, 7.0)));

        // No sidecar: the graph still builds, and the BFS fallback routes on
        // it (the table answers nothing).
        std::fs::remove_file(dir.join("nav/routes_h.bin")).expect("rm");
        let g = nav_graph(&m, &dir).expect("graph without a table");
        assert_eq!(g.next_hop(0, 1, Mover::SOLDIER, None, None, 0), None);
        assert_eq!(g.route_bfs(0, 1, Mover::SOLDIER, &|_| true), Some((0, 1)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A map with no `nav` at all has no graph — the AI walks straight at its
    /// post, as it did before Phase A.
    #[test]
    fn a_manifest_without_nav_has_no_graph() {
        assert!(nav_graph(&manifest(), std::path::Path::new(".")).is_none());
    }

    #[test]
    fn region_lookup_is_case_insensitive() {
        let l = MapLayout::from_manifest(&manifest());
        assert!(l.region("SARLAC01").is_some());
        assert!(l.region("sarlac01").is_some());
        assert!(l.region("nope").is_none());
    }
}
