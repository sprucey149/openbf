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
use core::fmath::FloatExt;
use core::nav::{Arc, Barrier, EscapeHeadings, FloorEscape, Hint, Hub, NavGraph, RouteTable};
use core::region::Region;
use core::route_patch::RoutePatches;
use glam::{Vec2, Vec3};
use std::path::Path;

pub const PROP_TEAM: u32 = 0xA2FD_7D0C;
pub const PROP_LABEL: u32 = 0xF697_17FD;
pub const PROP_CAPTURE_REGION: u32 = 0xC6C1_61F9;
pub const PROP_CONTROL_REGION: u32 = 0x805C_1F8A;
pub const PROP_SPAWN_PATH: u32 = 0xDDE0_B14F;
pub const PROP_ALLY_PATH: u32 = 0xDAD2_15B0;
pub const PROP_ALLY_COUNT: u32 = 0x7136_FD3E;
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
    /// Authored `AllyPath`, separate from this post's own spawn path.
    pub ally_path: Option<String>,
    /// Authored `AllyCount` budget, before any class default.
    pub ally_count: Option<u32>,
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
    /// A space hangar's post: it spawns its team and is never taken.
    pub space: bool,
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
    /// The mission's landing regions, the ones the map holds.
    pub landing_regions: Vec<Region>,
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
                let len = v.length().fmax(1e-6);
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
    // OURS: enter the Bazaar walkway at its ramp before crossing it.
    // measure: Yavin H83/H84 floor traces on every source profile (#108).
    if m.source_game == "bf2"
        && m.map == "yavin1"
        && g.arcs
            .get(111)
            .is_some_and(|a| a.touches(83) && a.touches(84))
    {
        g.floor_join_arcs.push(111);
    }
    // OURS: keep overlapping soldiers outside the courtyard column.
    // measure: BF2 Yavin Clone Wars Elite seed 1 enclosure (#108).
    if m.source_game == "bf2" && m.map == "yavin1" {
        g.separation_wall = m
            .instances
            .iter()
            .position(|i| {
                i.name == "colume15"
                    && i.class == "yav_bldg_column"
                    && Vec3::from(i.translation)
                        .distance_squared(Vec3::new(91.493, -71.034, -64.648))
                        < 0.01
            })
            .and_then(|i| u32::try_from(i).ok());
    }
    // OURS: keep separation from pushing bots through the corridor seam.
    // measure: Tantive IV lower corridor floor and phase traces (#108).
    if m.source_game == "bf2" && m.map == "tan1" {
        g.separation_sweep = m
            .instances
            .iter()
            .position(|i| {
                i.class == "tan4_bldg_corridor_c"
                    && Vec3::from(i.translation).distance_squared(Vec3::new(-277.0, 2.0, 140.0))
                        < 0.01
            })
            .and_then(|i| u32::try_from(i).ok())
            .map(|instance| core::nav::SeparationSweep {
                origin: core::collision::Aabb {
                    min: Vec3::new(-275.89, 1.8, 142.0),
                    max: Vec3::new(-275.70, 2.2, 142.55),
                },
                contact: core::collision::Aabb {
                    min: Vec3::new(-275.89, 1.8, 142.36),
                    max: Vec3::new(-275.75, 2.2, 142.43),
                },
                instance,
                direction: Vec3::Z,
            });
        if g.hub(20).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-276.98, 147.84)) < 0.01
        }) {
            // OURS: keep a grounded step on the supported side of the lower seam.
            // measure: original floor edge and Tantive IV Elite step trace (#108).
            g.floor_seam = Some(core::nav::FloorSeam {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-274.6, 1.8, 151.9),
                    max: Vec3::new(-273.9, 2.2, 152.52),
                },
                inward: Vec3::Z,
            });
        }
    }
    // OURS: step out of the path of an approaching hover tank on the west street.
    // measure: BF1 Theed Hard seed 2 first hull contact and ordinary side step (#108).
    if m.source_game == "bf1"
        && m.map == "naboo2"
        && g.arcs.first().is_some_and(|a| a.touches(3) && a.touches(9))
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-169.0, -50.15, 29.0),
                    max: Vec3::new(-167.15, -49.9, 32.8),
                },
                hub: 3,
                headings: EscapeHeadings::Compass(&[180.0]),
            },
            motion: ApproachMotion::YieldToHover(Vec3::new(-178.5, -50.04, 30.3)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: clear the lower street before redrawing the Theed wall approach.
    // measure: BF2 Theed Hard seed 2 westward walk (#108).
    if m.source_game == "bf2" && m.map == "naboo2" && g.hub(21).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(104.85, 7.75, -71.25),
                    max: Vec3::new(106.15, 8.0, -70.35),
                },
                hub: 21,
                headings: EscapeHeadings::Compass(&[180.0]),
            },
            motion: ApproachMotion::Stalled {
                exit: Vec3::new(82.0, 7.86, -69.4),
                descend: false,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: step and hop off the Theed plaza curb before redrawing the leg.
    // measure: BF2 Theed Hard seed 1 ordinary input and the original curb (#108).
    if m.source_game == "bf2" && m.map == "naboo2" && g.hub(38).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(144.55, 8.25, -31.0),
                    max: Vec3::new(144.9, 8.48, -30.64),
                },
                hub: 38,
                headings: EscapeHeadings::Compass(&[180.0]),
            },
            motion: ApproachMotion::PinnedHop {
                exit: Vec3::new(134.0, 7.86, -29.5),
                delay: 4,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: take the lower Harbor path from the edge of the rock shelf.
    // measure: Hard seed 2 ordinary input and the original Harbor descent (#108).
    if m.source_game == "bf1" && m.map == "rhenvar1" && g.hub(51).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-31.25, 3.90, 101.8),
                    max: Vec3::new(-28.7, 4.2, 103.5),
                },
                hub: 51,
                headings: EscapeHeadings::Compass(&[225.0, 235.0, 245.0]),
            },
            motion: ApproachMotion::Stalled {
                exit: Vec3::new(-31.4, 0.4, 95.8),
                descend: true,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: clear the Great Spire's overhang on the lower terrain.
    // measure: BF1 Geonosis Hard seed 3 same-life descent (#108).
    if m.source_game == "bf1" && m.map == "geo1" && g.hub(23).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(102.9, -51.1, -112.1),
                    max: Vec3::new(104.0, -50.65, -111.1),
                },
                hub: 23,
                headings: EscapeHeadings::Compass(&[135.0]),
            },
            motion: ApproachMotion::Stalled {
                exit: Vec3::new(89.5, -62.58, -97.4),
                descend: true,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: clear the exterior of the Mos Eisley entry before replanning.
    // measure: the Elite seed 2 same-life walk around the entry (#108).
    if m.source_game == "bf1"
        && m.map == "tat2"
        && g.hub(38).is_some()
        && m.instances.iter().any(|i| {
            i.class == "tat2_bldg_entry2"
                && Vec3::from(i.translation).distance_squared(Vec3::new(-140.0, 15.0, 66.0)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-139.1, 15.05, 62.8),
                    max: Vec3::new(-137.85, 15.55, 64.0),
                },
                hub: 38,
                headings: EscapeHeadings::Compass(&[225.0, 180.0, 135.0]),
            },
            motion: ApproachMotion::PinnedWalk(Vec3::new(-153.5, 15.0, 58.0)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: take the clear side of the boulder before following the arc.
    // measure: BF2 Geonosis Elite seed 1 same-life approach (#108).
    if m.source_game == "bf2"
        && m.map == "geo1"
        && g.arcs
            .get(33)
            .is_some_and(|a| a.touches(2) && a.touches(19))
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(66.2, -64.4, 213.65),
                    max: Vec3::new(67.4, -63.3, 215.0),
                },
                hub: 19,
                headings: EscapeHeadings::Compass(&[0.0, 30.0]),
            },
            motion: ApproachMotion::AlongArc {
                arc: 33,
                velocity: Vec3::Z,
                exit: Vec3::new(88.5, -61.55, 217.1),
                descend: false,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: cross the higher lip beside the Yavin walkway.
    // measure: Elite seed 3 ordinary climb and the rejoin toward H84 (#108).
    if m.source_game == "bf2" && m.map == "yavin1" && g.hub(84).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(33.5, -65.85, 36.9),
                    max: Vec3::new(34.4, -65.28, 38.9),
                },
                hub: 84,
                headings: EscapeHeadings::Compass(&[90.0]),
            },
            motion: ApproachMotion::PinnedWalk(Vec3::new(33.9, -63.36, 48.15)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: take the upper cell passage before turning onto the lower floor.
    // measure: Jabba's Palace Elite seed 1 same-life upper passage (#108).
    if m.source_game == "bf2"
        && m.map == "tat3"
        && g.arcs.get(64).is_some_and(|a| a.touches(30))
        && m.instances.iter().any(|i| {
            i.class == "tat3_bldg_cell_corridor"
                && Vec3::from(i.translation).distance_squared(Vec3::new(-90.0, 4.0, 125.0)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        let exit = Vec3::new(-51.0, -4.5, 167.8);
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-45.55, -4.62, 174.45),
                    max: Vec3::new(-44.69, -4.37, 175.05),
                },
                hub: 30,
                headings: EscapeHeadings::CompassTo {
                    angles: &[240.0],
                    exit,
                },
            },
            motion: ApproachMotion::AlongArc {
                arc: 64,
                velocity: Vec3::new(0.6, 0.0, -1.0),
                exit,
                descend: false,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: follow the lower deck away from the wall before replanning.
    // measure: Mygeeto Elite seed 3 lower-floor rejoin (#108).
    if m.source_game == "bf2" && m.map == "myg1" && g.hub(115).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-66.38, -0.2, -33.35),
                    max: Vec3::new(-65.85, 0.2, -32.15),
                },
                hub: 115,
                headings: EscapeHeadings::CompassTo {
                    angles: &[105.0],
                    exit: Vec3::new(-68.7, 0.0, -18.0),
                },
            },
            motion: ApproachMotion::ReturningOnArc(153),
            finish_on_heading_plane: true,
        });
    }
    // OURS: walk south along the upper platform and away from the rim.
    // measure: Geonosis Elite seed 2 same-life upper-platform walk (#108).
    if m.source_game == "bf2" && m.map == "geo1" && g.hub(30).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        let exit = Vec3::new(163.8, -39.0, 141.4);
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(164.36, -39.25, 153.68),
                    max: Vec3::new(164.56, -39.0, 154.02),
                },
                hub: 30,
                headings: EscapeHeadings::CompassTo {
                    angles: &[255.0],
                    exit,
                },
            },
            motion: ApproachMotion::Stalled {
                exit,
                descend: false,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: keep tactical hops from entering the walled Theed courtyard.
    // measure: first approach and landing of the Elite seed 3 jet trooper (#108).
    if m.source_game == "bf2" && m.map == "naboo2" && g.hub(56).is_some() {
        g.hop_takeoff_keepout.push(core::collision::Aabb {
            min: Vec3::new(-131.3, 7.55, 32.3),
            max: Vec3::new(-127.7, 8.3, 35.3),
        });
    }
    // OURS: let deployed Droidekas walk out from the Harbor walls.
    // measure: alternate Hard seed 1 same-life tower and debris walks (#108).
    if m.source_game == "bf1" && m.map == "rhenvar1" && g.hub(70).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.extend([
            FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(-31.04, 3.6, 102.1),
                        max: Vec3::new(-30.58, 4.22, 103.35),
                    },
                    hub: 53,
                    headings: EscapeHeadings::Compass(&[270.0, 255.0]),
                },
                motion: ApproachMotion::DroidekaStalledDescent(Vec3::new(-30.9, 1.37, 97.55)),
                finish_on_heading_plane: true,
            },
            FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(-98.92, 7.9, -10.15),
                        max: Vec3::new(-98.38, 8.2, -9.42),
                    },
                    hub: 70,
                    headings: EscapeHeadings::Compass(&[225.0, 270.0]),
                },
                motion: ApproachMotion::DroidekaStalled(Vec3::new(-102.5, 8.05, -13.7)),
                finish_on_heading_plane: true,
            },
            FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(-27.2, 1.68, 74.91),
                        max: Vec3::new(-26.35, 2.18, 75.53),
                    },
                    hub: 55,
                    headings: EscapeHeadings::Compass(&[180.0]),
                },
                motion: ApproachMotion::DroidekaStalled(Vec3::new(-36.08, 2.16, 75.21)),
                finish_on_heading_plane: true,
            },
        ]);
    }
    // OURS: keep the upper Citadel footing when turning by the retaining wall.
    // measure: alternate Hard seed 1 ordinary upper crossing (#108).
    if m.source_game == "bf1" && m.map == "rhenvar2" && g.hub(62).is_some() {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-103.95, 9.73, -225.55),
                    max: Vec3::new(-102.97, 10.27, -224.81),
                },
                hub: 62,
                headings: EscapeHeadings::Compass(&[225.0, 180.0]),
            },
            motion: ApproachMotion::DroidekaApproaching(Vec3::new(-108.65, 9.08, -229.85)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: keep the hover trooper on the street when the attack launch
    // would strike the underside of the roof. Authored jet legs still work.
    // measure: alternate Hard seed 1 Theed attack launch and roof landing (#108).
    if m.source_game == "bf2" && m.map == "naboo2" && g.hub(46).is_some() {
        g.attack_takeoff_keepout.push(core::collision::Aabb {
            min: Vec3::new(-5.65, 8.65, -26.45),
            max: Vec3::new(-3.20, 9.37, -23.8),
        });
    }
    // OURS: back around the Mos Eisley corner before taking the next leg.
    // measure: BF2 Mos Eisley alternate Hard seed 1 same-life walk (#108).
    if m.source_game == "bf2"
        && m.map == "tat2"
        && g.arcs.get(48).is_some_and(|arc| arc.touches(57))
    {
        use core::nav::{ApproachMotion, FloorApproach};
        let exit = Vec3::new(14.1, 0.17, -68.85);
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(24.42, -0.15, -76.55),
                    max: Vec3::new(25.65, 0.35, -75.71),
                },
                hub: 57,
                headings: EscapeHeadings::CompassTo {
                    angles: &[135.0],
                    exit,
                },
            },
            motion: ApproachMotion::AlongArc {
                arc: 48,
                velocity: Vec3::X,
                exit,
                descend: false,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: back out along the plaza wall before replanning the corner.
    // measure: BF1 Mos Eisley Hard seed 1 and the original wall placement (#108).
    if m.source_game == "bf1"
        && m.map == "tat2"
        && g.hub(6).is_some()
        && m.instances.iter().any(|i| {
            i.class == "tat2_bldg_wall_40m"
                && Vec3::from(i.translation).distance_squared(Vec3::new(-102.0, 15.0, 95.0)) < 0.01
        })
    {
        g.floor_escapes.push(FloorEscape {
            bounds: core::collision::Aabb {
                min: Vec3::new(-91.0, 14.85, 94.8),
                max: Vec3::new(-87.3, 15.85, 96.1),
            },
            hub: 6,
            headings: EscapeHeadings::Compass(&[180.0, 225.0]),
        });
    }
    // OURS: retreat along the exterior of the hull before drawing a new leg.
    // measure: BF2 Geonosis Hard seed 1 and the original hull placement (#108).
    if m.source_game == "bf2"
        && m.map == "geo1"
        && g.hub(3).is_some()
        && m.instances.iter().any(|i| {
            i.class == "geo_bldg_technounion_hull"
                && Vec3::from(i.translation).distance_squared(Vec3::new(-16.649, -58.404, 165.482))
                    < 0.01
        })
    {
        g.floor_escapes.push(FloorEscape {
            bounds: core::collision::Aabb {
                min: Vec3::new(-10.6, -63.5, 172.1),
                max: Vec3::new(-8.7, -62.85, 173.6),
            },
            hub: 3,
            headings: EscapeHeadings::Compass(&[0.0, 315.0, 45.0]),
        });
    }
    // OURS: stay on the walkable lip after dismounting by the cave mouth.
    // measure: the Hard seed 3 same-life approach and original cave placement (#108).
    if m.source_game == "bf2"
        && m.map == "geo1"
        && g.arcs
            .get(86)
            .is_some_and(|a| a.touches(60) && a.touches(29))
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-22.3, -66.1, 65.4),
                    max: Vec3::new(-20.35, -65.45, 67.1),
                },
                hub: 59,
                headings: EscapeHeadings::Compass(&[90.0]),
            },
            motion: ApproachMotion::AlongArc {
                arc: 86,
                velocity: Vec3::NEG_Z,
                exit: Vec3::new(-22.15, -65.86, 84.2),
                descend: false,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: back out along the Harbor shelf before drawing another leg.
    // measure: Hard seed 1 and the original Harbor ledge (#108).
    if m.source_game == "bf1" && m.map == "rhenvar1" && g.hub(24).is_some() {
        g.floor_escapes.push(FloorEscape {
            bounds: core::collision::Aabb {
                min: Vec3::new(-130.0, 2.1, 149.4),
                max: Vec3::new(-127.45, 3.1, 152.4),
            },
            hub: 24,
            headings: EscapeHeadings::CompassTo {
                angles: &[180.0, 135.0],
                exit: Vec3::new(-143.9, 2.3, 150.4),
            },
        });
    }
    // OURS: the H78 stair approach is playable beyond its rim barrier.
    // measure: original Mygeeto downhill motion and both ledge controls (#108).
    if m.source_game == "bf2"
        && m.map == "myg1"
        && g.arcs
            .get(78)
            .is_some_and(|a| a.touches(78) && a.touches(79))
    {
        g.floor_support_arcs.push(78);
    }
    // OURS: climb the side of the lower shelf back to the stair approach.
    // measure: BF2 Mygeeto Hard Clone Wars seed 1 same-life climb (#108).
    if m.source_game == "bf2"
        && m.map == "myg1"
        && g.hub(5).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-62.381165, 188.29848)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-68.4, -3.45, 177.4),
                    max: Vec3::new(-66.65, -2.4, 179.4),
                },
                hub: 5,
                headings: EscapeHeadings::Compass(&[45.0, 135.0, 0.0]),
            },
            motion: ApproachMotion::Returning,
            finish_on_heading_plane: true,
        });
    }
    // OURS: steer around the support under the Docks toward the beach.
    // measure: native under-deck walk and Elite seed 3 recovery (#108).
    if m.source_game == "bf1"
        && m.map == "kas2"
        && g.hub(131)
            .is_some_and(|h| Vec2::new(h.pos.x, h.pos.z).distance(Vec2::new(159.0, 10.30)) < 0.1)
    {
        let pocket = core::collision::Aabb {
            min: Vec3::new(184.0, -6.3, 8.0),
            max: Vec3::new(190.0, -5.5, 15.0),
        };
        g.floor_escapes.push(FloorEscape {
            bounds: pocket,
            hub: 131,
            headings: EscapeHeadings::AroundSupport,
        });
        // OURS: use the deployed body to clear the support on the beach side.
        // measure: BF1 Docks Hard Clone Wars seed 1 under-deck walk (#108).
        g.floor_approaches.push(core::nav::FloorApproach {
            escape: FloorEscape {
                bounds: pocket,
                hub: 131,
                headings: EscapeHeadings::Compass(&[135.0, 130.0, 140.0, 145.0, 150.0]),
            },
            motion: core::nav::ApproachMotion::DroidekaStalled(Vec3::new(176.0, -6.04, 19.5)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: unroll to cross the shallow groove beside the Docks spawn.
    // measure: BF1 Docks Easy and Hard Clone Wars seed 1 deck exits (#108).
    if m.source_game == "bf1"
        && m.map == "kas2"
        && g.hub(0).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(109.52, 290.05)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(107.0, -0.65, 289.6),
                    max: Vec3::new(113.5, 0.0, 292.8),
                },
                hub: 0,
                headings: EscapeHeadings::Compass(&[135.0, 140.0, 130.0, 145.0, 125.0]),
            },
            motion: ApproachMotion::DroidekaStalled(Vec3::new(100.4, 0.0, 299.2)),
            finish_on_heading_plane: true,
        });
        // OURS: walk down the west side of the outer deck to the beach.
        // measure: BF1 Docks Elite Clone Wars seed 6 shelf descent (#108).
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(185.8, -3.6, 81.9),
                    max: Vec3::new(187.1, -2.9, 82.8),
                },
                hub: 0,
                headings: EscapeHeadings::Compass(&[180.0, 175.0, 185.0, 190.0]),
            },
            motion: ApproachMotion::DroidekaReturning(Vec3::new(174.0, -5.94, 83.5)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: unroll around the pier and walk down from the upper dock shelf.
    // measure: original BF1 floor support and the two Docks Hard bot lives (#108).
    if m.source_game == "bf1"
        && m.map == "kas2"
        && g.hub(132).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(52.18, 9.47)) < 0.01
        })
        && g.hub(72).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(229.18, -10.75)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.extend([
            FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(41.62, -6.05, 25.90),
                        max: Vec3::new(41.95, -5.66, 26.42),
                    },
                    hub: 132,
                    headings: EscapeHeadings::Compass(&[350.0]),
                },
                motion: ApproachMotion::DroidekaPinned {
                    exit: Vec3::new(56.8, -5.76, 23.5),
                    min_windows: 2,
                },
                finish_on_heading_plane: true,
            },
            FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(250.77, 7.83, -35.36),
                        max: Vec3::new(251.34, 8.21, -34.84),
                    },
                    hub: 72,
                    headings: EscapeHeadings::Compass(&[135.0]),
                },
                motion: ApproachMotion::DroidekaReturning(Vec3::new(244.23, -1.0, -26.46)),
                finish_on_heading_plane: true,
            },
        ]);
    }
    // OURS: turn away from the upper seam toward the lower passage.
    // measure: Tantive IV original floor support and Hard recovery traces (#108).
    if m.source_game == "bf2"
        && m.map == "tan1"
        && g.arcs
            .get(31)
            .is_some_and(|a| a.touches(42) && a.touches(18))
        && g.hub(18).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-276.52, 120.57)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        let exit = Vec3::new(-273.9, 8.18, 135.4);
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-266.4, 8.0, 143.9),
                    max: Vec3::new(-262.5, 10.3, 145.9),
                },
                hub: 18,
                headings: EscapeHeadings::CompassTo {
                    angles: &[223.0],
                    exit,
                },
            },
            motion: ApproachMotion::AlongArc {
                arc: 31,
                velocity: Vec3::NEG_X,
                exit,
                descend: true,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: tactical hops from the Bespin decks need a walkable landing.
    // measure: native Dark Trooper landings and deck-edge falls (#135).
    if m.source_game == "bf1" && m.map == "bes1" {
        g.hop_support_areas.push(core::collision::Aabb {
            min: Vec3::new(f32::MIN, 90.0, f32::MIN),
            max: Vec3::new(f32::MAX, 170.0, f32::MAX),
        });
    }
    // OURS: walk from the lower service level to the clear platform route.
    // measure: original BF1 Bespin lower walkway and the four bot lives (#108).
    if m.source_game == "bf1"
        && m.map == "bes1"
        && g.hub(4)
            .is_some_and(|h| Vec2::new(h.pos.x, h.pos.z).distance(Vec2::new(21.03, -158.52)) < 0.1)
        && g.hub(5)
            .is_some_and(|h| Vec2::new(h.pos.x, h.pos.z).distance(Vec2::new(1.10, -191.95)) < 0.1)
    {
        const HEADINGS: &[f32] = &[0.0, 15.0];
        for (hub, low_z, high_z) in [(4, -173.0, -143.0), (5, -198.0, -173.0)] {
            g.floor_escapes.push(FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-26.0, 98.4, low_z),
                    max: Vec3::new(14.0, 99.5, high_z),
                },
                hub,
                headings: EscapeHeadings::Compass(HEADINGS),
            });
        }
    }
    // OURS: unroll on the shelf and descend to the walkable Docks approach.
    // measure: original shelf placements and the extended Clone Wars replays (#108).
    if m.map == "kas2" {
        use core::nav::{ApproachMotion, FloorApproach};
        let walks: &[(Vec3, Vec3, u16, &[f32], Vec3)] = match m.source_game.as_str() {
            "bf1" => &[
                (
                    Vec3::new(-27.7, 7.7, -35.1),
                    Vec3::new(-26.5, 8.3, -33.8),
                    55,
                    &[135.0],
                    Vec3::new(-38.0, -5.76, -22.9),
                ),
                (
                    Vec3::new(189.9, -2.6, 36.6),
                    Vec3::new(191.2, -1.95, 38.2),
                    50,
                    &[135.0],
                    Vec3::new(175.3, -5.94, 40.6),
                ),
            ],
            "bf2" => &[(
                Vec3::new(123.1, 13.8, 115.0),
                Vec3::new(124.55, 14.5, 116.4),
                8,
                &[0.0],
                Vec3::new(133.3, 14.16, 119.0),
            )],
            _ => &[],
        };
        for &(min, max, hub, headings, exit) in walks {
            g.floor_approaches.push(FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb { min, max },
                    hub,
                    headings: EscapeHeadings::Compass(headings),
                },
                motion: ApproachMotion::DroidekaReturning(exit),
                finish_on_heading_plane: true,
            });
        }
    }
    // OURS: walk across the deck ramps without rolling over their exposed edges.
    // measure: BF1 Bespin Platforms Easy Clone Wars seed 4 approaches (#108).
    if m.source_game == "bf1" && m.map == "bes1" {
        use core::nav::{ApproachMotion, FloorApproach};
        let crossings = [
            (
                Vec3::new(63.3, 98.6, 154.05),
                Vec3::new(64.15, 99.05, 155.1),
                51,
                &[300.0][..],
                Vec3::new(68.68, 99.93, 145.72),
            ),
            (
                Vec3::new(63.3, 98.6, 154.05),
                Vec3::new(64.15, 99.05, 155.4),
                61,
                &[300.0][..],
                Vec3::new(68.68, 99.93, 145.72),
            ),
            (
                Vec3::new(67.5, 98.6, 154.55),
                Vec3::new(68.7, 99.05, 156.1),
                61,
                &[180.0, 185.0, 190.0][..],
                Vec3::new(63.65, 98.92, 154.65),
            ),
            (
                Vec3::new(-107.15, 107.6, -99.8),
                Vec3::new(-106.25, 108.3, -98.35),
                14,
                &[235.0, 245.0][..],
                Vec3::new(-112.5, 109.12, -109.45),
            ),
            (
                Vec3::new(151.45, 107.9, 17.05),
                Vec3::new(152.25, 108.25, 18.15),
                92,
                &[180.0][..],
                Vec3::new(147.3, 108.02, 17.65),
            ),
            (
                Vec3::new(169.8, 98.3, 128.6),
                Vec3::new(170.65, 98.85, 129.65),
                59,
                &[210.0, 195.0][..],
                Vec3::new(160.75, 99.86, 124.3),
            ),
        ];
        for (min, max, hub, headings, exit) in crossings {
            g.floor_approaches.push(FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb { min, max },
                    hub,
                    headings: EscapeHeadings::Compass(headings),
                },
                motion: ApproachMotion::DroidekaApproaching(exit),
                finish_on_heading_plane: true,
            });
        }
        // OURS: walk off the east ramp before choosing a tactical hop.
        // measure: BF1 Bespin Platforms Easy GCW seed 6 flight runout (#108).
        g.hop_takeoff_keepout.push(core::collision::Aabb {
            min: Vec3::new(192.0, 98.2, 148.0),
            max: Vec3::new(207.0, 105.0, 174.0),
        });
    }
    // OURS: unroll at the rim and take the middle of the deck crossing.
    // measure: BF1 Bespin Platforms Normal Clone Wars seed 1 (#108).
    if m.source_game == "bf1"
        && m.map == "bes1"
        && g.hub(91).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(17.43578, 84.05567)) < 0.01
        })
        && g.arcs
            .get(108)
            .is_some_and(|a| a.touches(90) && a.touches(91))
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(16.2, 107.7, 83.7),
                    max: Vec3::new(18.3, 108.2, 84.6),
                },
                hub: 90,
                headings: EscapeHeadings::Compass(&[0.0]),
            },
            motion: ApproachMotion::DroidekaStalled(Vec3::new(25.3, 108.83, 84.17)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: keep a jet landing on the approaching side of this hall.
    // measure: original BF2 Coruscant hall walk and Hard seed 2 landing (#108).
    if m.source_game == "bf2" && m.map == "cor1" {
        g.jet_landing_wall = m
            .instances
            .iter()
            .position(|i| {
                i.class == "cor1_bldg_minor_hall_02"
                    && i.name == "cor1_bldg_minor_hall_02"
                    && Vec3::from(i.translation).distance_squared(Vec3::new(0.0, 3.0, -60.0)) < 0.01
            })
            .and_then(|i| u32::try_from(i).ok());
    }
    // OURS: the Felucia slope can return a pinned bot toward H4.
    // measure: the Elite 3 soldier's same-life walk and original slope (#108).
    if m.source_game == "bf2"
        && m.map == "fel1"
        && g.hub(4).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(112.116, -29.390)) < 0.01
        })
    {
        g.floor_escapes.push(FloorEscape {
            bounds: core::collision::Aabb {
                min: Vec3::new(137.5, 16.65, -62.0),
                max: Vec3::new(140.5, 17.25, -59.0),
            },
            hub: 4,
            headings: EscapeHeadings::Compass(&[130.7]),
        });
    }
    // OURS: tactical hops from the Death Star decks can sample the void.
    // measure: the unsupported landings, valid hops and original floor (#108).
    if m.source_game == "bf2"
        && m.map == "dea1"
        && g.hub(36).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-171.51, -244.58)) < 0.01
        })
    {
        g.hop_support_areas.extend([
            core::collision::Aabb {
                min: Vec3::new(-200.0, 51.8, -256.0),
                max: Vec3::new(-103.0, 52.15, -231.0),
            },
            core::collision::Aabb {
                min: Vec3::new(-159.0, 51.8, -192.0),
                max: Vec3::new(-144.0, 52.15, -177.0),
            },
            core::collision::Aabb {
                min: Vec3::new(-191.3, 48.9, -234.0),
                max: Vec3::new(-188.3, 49.9, -231.2),
            },
            core::collision::Aabb {
                min: Vec3::new(-122.0, 51.8, -276.0),
                max: Vec3::new(-91.8, 52.16, -248.9),
            },
        ]);
        // OURS: keep tactical hops out of unsupported landings across the hall wall.
        // measure: original placements east of the wall on the upper and lower floors (#108).
        g.hop_landing_areas.extend([
            (
                core::collision::Aabb {
                    min: Vec3::new(-139.0, 41.5, -180.0),
                    max: Vec3::new(-108.0, 52.2, -95.0),
                },
                0.0,
            ),
            // OURS: leave room to coast down past the hangar ramp.
            // measure: Death Star Elite 6 overshoot below the deck (#108).
            (
                core::collision::Aabb {
                    min: Vec3::new(-159.0, 51.8, -260.0),
                    max: Vec3::new(-150.0, 52.2, -251.0),
                },
                12.0,
            ),
        ]);
        // OURS: turn along the upper deck before the approach reaches the edge.
        // measure: Death Star Elite Clone Wars seed 1 upper ledge walk (#108).
        if g.arcs
            .get(20)
            .is_some_and(|a| a.touches(36) && a.touches(37))
        {
            g.floor_approaches.push(core::nav::FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(-176.0, 51.8, -249.0),
                        max: Vec3::new(-172.0, 52.15, -247.0),
                    },
                    hub: 42,
                    headings: EscapeHeadings::Compass(&[80.0, 90.0]),
                },
                motion: core::nav::ApproachMotion::AlongArc {
                    arc: 20,
                    velocity: Vec3::X,
                    exit: Vec3::new(-170.1, 51.99, -229.0),
                    descend: false,
                },
                finish_on_heading_plane: true,
            });
        }
        // OURS: reach the cover on the lower floor by its authored ramp.
        // measure: native ramp walk and the Hard seed 2 defender (#108).
        let cover = Vec3::new(-156.26059, 46.999992, -271.94254);
        if g.hub(41).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-154.3374, -272.27023)) < 0.01
        }) && g
            .hints
            .iter()
            .any(|h| h.kind == 2 && h.pos.distance_squared(cover) < 0.01)
        {
            g.defend_hint_hub = Some((cover, 41));
        }
    }
    // OURS: take the outer deck on the approach to the depot.
    // measure: Hard seed 3 same-life walk around the depot (#108).
    if m.source_game == "bf2"
        && m.map == "kamino1"
        && g.arcs
            .get(107)
            .is_some_and(|a| a.touches(100) && a.touches(103))
        && m.instances.iter().any(|i| {
            i.name == "ctf1"
                && i.class == "kam_bldg_central_sample_depot"
                && Vec3::from(i.translation).distance_squared(Vec3::new(-190.606, 69.294, 32.193))
                    < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-173.2, 69.15, 39.9),
                    max: Vec3::new(-171.45, 69.45, 40.9),
                },
                hub: 100,
                headings: EscapeHeadings::Compass(&[160.0, 165.0]),
            },
            motion: ApproachMotion::AlongArc {
                arc: 107,
                velocity: Vec3::NEG_X,
                exit: Vec3::new(-190.05, 68.305, 45.2),
                descend: false,
            },
            finish_on_heading_plane: true,
        });
    }
    // OURS: take the clear side of the fallen column before the wall slide
    // leads into its shoulder.
    // measure: BF1 Naboo ruins ordinary walks and H0 approach replays (#108).
    if m.source_game == "bf1"
        && m.map == "naboo1"
        && g.hub(0).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-11.0, -112.35)) < 0.1
        })
        && m.instances.iter().any(|i| {
            i.class == "nab1_prop_ruins_column1"
                && Vec3::from(i.translation).distance_squared(Vec3::new(-38.0, -108.33, -124.0))
                    < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        for (min, max, angle, motion, finish_on_heading_plane) in [
            (
                Vec3::new(-44.9, -111.1, -125.5),
                Vec3::new(-42.5, -110.35, -122.7),
                &[315.0][..],
                ApproachMotion::Sliding,
                false,
            ),
            (
                Vec3::new(-49.5, -110.6, -122.4),
                Vec3::new(-47.0, -109.75, -120.45),
                &[0.0][..],
                ApproachMotion::Toward(Vec3::X),
                true,
            ),
        ] {
            g.floor_approaches.push(FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb { min, max },
                    hub: 0,
                    headings: EscapeHeadings::Compass(angle),
                },
                motion,
                finish_on_heading_plane,
            });
        }
    }
    // OURS: keep soldiers outside the marked rocks when the wall pass
    // pushes toward their interior. Walk around the m9 shoulder.
    // measure: BF2 Docks original placement walks and enclosure replays (#108).
    if m.source_game == "bf2" && m.map == "kas2" {
        use core::nav::{ApproachMotion, FloorApproach};
        for (name, class, pos) in [
            (
                "kas2_prop_rock_m10",
                "kas2_prop_rock_m",
                Vec3::new(212.042, -0.117, -0.149),
            ),
            (
                "kas2_prop_rock_l11",
                "kas2_prop_rock_l",
                Vec3::new(184.737, 9.382, 174.983),
            ),
            (
                "kas2_prop_rock_m16",
                "kas2_prop_rock_m",
                Vec3::new(142.404, 6.889, -47.341),
            ),
            (
                "kas2_prop_rock_l8",
                "kas2_prop_rock_l",
                Vec3::new(231.161, 1.732, 12.725),
            ),
        ] {
            g.wall_approach_props.extend(
                m.instances
                    .iter()
                    .position(|i| {
                        i.name == name
                            && i.class == class
                            && Vec3::from(i.translation).distance_squared(pos) < 0.01
                    })
                    .and_then(|i| u32::try_from(i).ok()),
            );
        }
        if m.instances.iter().any(|i| {
            i.name == "kas2_prop_rock_m9"
                && i.class == "kas2_prop_rock_m"
                && Vec3::from(i.translation).distance_squared(Vec3::new(235.374, 2.618, 7.886))
                    < 0.01
        }) {
            g.floor_approaches.push(FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(237.12, 6.48, 7.1),
                        max: Vec3::new(238.55, 7.3, 9.45),
                    },
                    hub: 2,
                    headings: EscapeHeadings::Compass(&[225.0, 270.0, 135.0, 90.0, 180.0]),
                },
                motion: ApproachMotion::Any,
                finish_on_heading_plane: true,
            });
        }
    }
    // OURS: reject tactical hops across the Citadel roof from the lower path.
    // measure: Rhen Var Citadel Easy and Hard seed 2 roof landings (#108).
    if m.source_game == "bf1" && m.map == "rhenvar2" {
        g.hop_roof_areas.push(core::collision::Aabb {
            min: Vec3::new(-288.0, -1.0, -250.0),
            max: Vec3::new(-270.0, 3.5, -224.0),
        });
        // OURS: take the lower ramp where a long attack launch overshoots
        // the approach and lands behind the retaining wall.
        // measure: Citadel Clone Wars Hard seed 2 landing and ramp walk (#108).
        g.attack_takeoff_keepout.push(core::collision::Aabb {
            min: Vec3::new(-274.8, -0.4, -251.5),
            max: Vec3::new(-273.8, 0.4, -250.3),
        });
    }
    // OURS: the Cloud City upper route overlooks a lower street.
    // measure: original BF1 landing and the Hard seed 1 tactical hop (#108).
    if m.source_game == "bf1"
        && m.map == "bespin2"
        && g.hub(34).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-164.14655, 81.42638)) < 0.01
        })
    {
        g.hop_support_areas.push(core::collision::Aabb {
            min: Vec3::new(-145.0, -33.25, 81.0),
            max: Vec3::new(-141.0, -32.9, 83.0),
        });
    }
    // OURS: keep the BF1 jet trooper outside the placed trunk on contact.
    // measure: the Elite seed 2 wall phase and original trunk motion (#108).
    if m.map == "end1" && m.source_game == "bf1" {
        g.wall_approach_props.extend(
            m.instances
                .iter()
                .position(|i| {
                    i.class == "end_prop_trunk03"
                        && Vec3::from(i.translation)
                            .distance_squared(Vec3::new(171.469, 3.577, 9.651))
                            < 0.01
                })
                .and_then(|i| u32::try_from(i).ok()),
        );
    }
    // OURS: walk around the BF2 trunk before the foot capsule enters it.
    // measure: the Elite seed 2 approach and original standing walks (#108).
    if m.map == "end1"
        && m.source_game == "bf2"
        && g.hub(11).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(64.30054, 11.128441)) < 0.01
        })
        && m.instances.iter().any(|i| {
            i.name == "end_prop_trunk0215"
                && i.class == "end_prop_trunk02"
                && Vec3::from(i.translation).distance_squared(Vec3::new(92.448, 6.123, -42.944))
                    < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(99.4, -1.0, -54.7),
                    max: Vec3::new(102.2, -0.2, -51.6),
                },
                hub: 11,
                headings: EscapeHeadings::Compass(&[90.0, 180.0]),
            },
            motion: ApproachMotion::Toward(Vec3::new(-0.59, 0.0, 0.81)),
            finish_on_heading_plane: true,
        });
    }
    // OURS: take the climb back from the platform's lower slope and require
    // a clear floor route to the two separated sniper positions.
    // measure: BF2 Kamino ordinary walks, stock hint floor and Hard 2 (#108).
    if m.source_game == "bf2" && m.map == "kamino1" {
        use core::nav::{ApproachMotion, FloorApproach};
        if g.hub(63).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-82.62875, 145.43706)) < 0.01
        }) {
            g.floor_approaches.push(FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(-59.25, 63.8, 125.2),
                        max: Vec3::new(-57.65, 64.75, 127.0),
                    },
                    hub: 63,
                    headings: EscapeHeadings::Compass(&[180.0, 135.0]),
                },
                motion: ApproachMotion::Returning,
                finish_on_heading_plane: true,
            });
        }
        for (name, pos) in [
            ("hintnode165", Vec3::new(103.621506, 53.892616, 71.80047)),
            ("hintnode166", Vec3::new(108.362335, 53.89262, 74.68021)),
        ] {
            if let Some(hint) = g
                .hints
                .iter()
                .find(|h| h.name == name && h.kind == 0 && h.pos.distance_squared(pos) < 0.01)
            {
                g.snipe_hints_need_floor.push(hint.pos);
            }
        }
    }
    // OURS: go around the arena floor slab before the approach can wedge.
    // measure: BF1 ordinary placement walks and the Hard 3 and Easy 6 routes (#108).
    if m.map == "yav2"
        && m.source_game == "bf1"
        && g.hub(101).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-25.547054, -0.622696)) < 0.01
        })
        && g.hub(25).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-40.514587, -3.255507)) < 0.01
        })
        && m.instances.iter().any(|i| {
            i.class == "yav2_bldg_floor"
                && Vec3::from(i.translation).distance_squared(Vec3::new(-26.224, -58.048, -15.836))
                    < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        for (hub, headings) in [
            (101, &[90.0, 135.0, 180.0, 45.0, 315.0][..]),
            (25, &[90.0, 45.0][..]),
        ] {
            g.floor_approaches.push(FloorApproach {
                escape: FloorEscape {
                    bounds: core::collision::Aabb {
                        min: Vec3::new(-24.4, -62.2, -15.6),
                        max: Vec3::new(-21.55, -59.8, -12.4),
                    },
                    hub,
                    headings: EscapeHeadings::Compass(headings),
                },
                motion: ApproachMotion::Toward(Vec3::Z),
                finish_on_heading_plane: true,
            });
        }
    }
    // OURS: walk out of the two blocked approaches in deployed form.
    // measure: BF2 Geonosis Elite 2 and Normal 3 same-life walks (#108).
    if m.source_game == "bf2" && m.map == "geo1" {
        use core::nav::{ApproachMotion, FloorApproach};
        for (hub, anchor, min, max, angles, exit) in [
            (
                18,
                Vec2::new(57.556374, 294.0754),
                Vec3::new(62.45, -63.55, 289.1),
                Vec3::new(63.8, -62.9, 291.0),
                &[315.0, 310.0, 320.0][..],
                Vec3::new(71.4, -63.42, 284.0),
            ),
            (
                23,
                Vec2::new(197.15863, 104.52315),
                Vec3::new(178.2, -39.16, 125.0),
                Vec3::new(179.65, -39.08, 126.1),
                &[180.0, 175.0, 185.0][..],
                Vec3::new(169.8, -39.12, 125.4),
            ),
        ] {
            if g.hub(hub)
                .is_some_and(|h| Vec2::new(h.pos.x, h.pos.z).distance_squared(anchor) < 0.01)
            {
                g.floor_approaches.push(FloorApproach {
                    escape: FloorEscape {
                        bounds: core::collision::Aabb { min, max },
                        hub,
                        headings: EscapeHeadings::Compass(angles),
                    },
                    motion: ApproachMotion::DroidekaStalled(exit),
                    finish_on_heading_plane: true,
                });
            }
        }
    }
    // OURS: use the supported crossing between the two Kamino rim hubs.
    // measure: BF1 Kamino H49 to H48 and the Normal seed 3 foot replay (#108).
    if m.source_game == "bf1"
        && m.map == "kamino1"
        && g.hub(49).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(12.414408, 435.6653)) < 0.01
        })
        && g.hub(48).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-1.093877, 419.1284)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(9.9, 61.8, 434.8),
                    max: Vec3::new(11.8, 62.1, 438.0),
                },
                hub: 48,
                headings: EscapeHeadings::Compass(&[235.0, 237.0, 238.0, 239.0, 240.0, 242.0]),
            },
            motion: ApproachMotion::Droideka,
            finish_on_heading_plane: true,
        });
    }
    // OURS: climb west in deployed form from the pocket under the palace approach.
    // measure: BF2 Jabba Clone Wars Hard 1 and Elite 4 ordinary walks (#108).
    if m.source_game == "bf2"
        && m.map == "tat3"
        && g.hub(6).is_some_and(|h| {
            Vec2::new(h.pos.x, h.pos.z).distance_squared(Vec2::new(-46.313396, 90.14933)) < 0.01
        })
    {
        use core::nav::{ApproachMotion, FloorApproach};
        g.floor_approaches.push(FloorApproach {
            escape: FloorEscape {
                bounds: core::collision::Aabb {
                    min: Vec3::new(-41.0, -9.95, 93.4),
                    max: Vec3::new(-35.6, -8.6, 98.8),
                },
                hub: 6,
                headings: EscapeHeadings::Compass(&[
                    135.0, 130.0, 140.0, 145.0, 155.0, 160.0, 90.0,
                ]),
            },
            motion: ApproachMotion::DroidekaReturning(Vec3::new(-45.5, -7.996, 101.2)),
            finish_on_heading_plane: true,
        });
    }
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
        Self::from_battle_manifest(m, false)
    }

    pub fn from_battle_manifest(m: &Manifest, hunt: bool) -> MapLayout {
        let region = |name: &str| -> Option<Region> {
            let r = m
                .regions
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(name))?;
            Region::from_rows(&r.shape, r.rotation, r.translation, r.size)
        };
        let mut cps = Vec::new();
        for inst in &m.instances {
            // A space hangar's post has no capture region: its control region is where
            // it stands, and only its spawns matter.
            let has_spawn_path = prop(&inst.props, PROP_SPAWN_PATH).is_some();
            let space_post =
                inst.class.eq_ignore_ascii_case("com_bldg_controlzone_ctf") && has_spawn_path;
            let hunt_post =
                hunt && has_spawn_path && inst.class.eq_ignore_ascii_case("com_bldg_controlzone");
            let cap_name = match prop(&inst.props, PROP_CAPTURE_REGION) {
                Some(name) => Some(name),
                None if space_post => match prop(&inst.props, PROP_CONTROL_REGION) {
                    Some(name) => Some(name),
                    None if m.spawn_only_posts => None,
                    None => continue,
                },
                None if hunt_post => None,
                None => continue, // not a command post
            };
            cps.push(CommandPostDef {
                name: inst.name.clone(),
                label: prop(&inst.props, PROP_LABEL).unwrap_or("").to_owned(),
                class: inst.class.clone(),
                team: prop(&inst.props, PROP_TEAM)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0),
                pos: Vec3::from(inst.translation),
                capture: cap_name.and_then(region),
                control: prop(&inst.props, PROP_CONTROL_REGION).and_then(region),
                spawn_path: prop(&inst.props, PROP_SPAWN_PATH).map(str::to_owned),
                ally_path: prop(&inst.props, PROP_ALLY_PATH).map(str::to_owned),
                ally_count: prop(&inst.props, PROP_ALLY_COUNT).and_then(|s| s.trim().parse().ok()),
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
                    .map_or(core::cp::AI_SPAWN_WEIGHT_DEFAULT, |w| w.fmax(0.0)),
                value_atk: faction_values(&inst.props, &PROP_VALUE_ATK),
                value_def: faction_values(&inst.props, &PROP_VALUE_DEF),
                major: inst.class.contains("major"),
                space: space_post && prop(&inst.props, PROP_CAPTURE_REGION).is_none(),
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
            landing_regions: m
                .landing_regions
                .iter()
                .filter_map(|name| self.region(name))
                .collect(),
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
mod space_tests {
    use super::*;

    /// A hangar's post authors a control region and a spawn path and no capture region.
    /// Any other class with no capture region is not a post.
    #[test]
    fn a_space_hangars_post_is_a_post_that_cannot_be_taken() {
        let ctl = crate::classdb::fnv1a_lowercase("ControlRegion");
        let spawn = crate::classdb::fnv1a_lowercase("SpawnPath");
        let m: Manifest = ron::from_str(&format!(
            r#"(map: "s", source_game: "bf2", terrain_name: None, sky_name: None,
                terrain_glb: None, models: [], textures: [], classes: [], geometries: [], regions: [
                (shape: "sphere", name: "hangar", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), size: (5,5,5), props: []),
            ], instances: [
                (class: "com_bldg_controlzone_ctf", name: "cp", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), props: [({ctl}, "hangar"), ({spawn}, "hs")]),
                (class: "spa_prop_console", name: "prop", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), props: [({ctl}, "hangar"), ({spawn}, "hs")]),
            ])"#
        ))
        .unwrap();
        let layout = MapLayout::from_manifest(&m);
        assert_eq!(layout.cps.len(), 1);
        assert!(layout.cps[0].space);
        assert!(layout.cps[0].capture.is_some());
    }

    /// A mission's landing regions reach the sim's list by name; a name the map lacks
    /// falls out, and no other mission grows a list.
    #[test]
    fn landing_regions_come_through_the_mission() {
        let m: Manifest = ron::from_str(
            r#"(map: "s", source_game: "bf2", terrain_name: None, sky_name: None,
                terrain_glb: None, models: [], textures: [], classes: [], geometries: [], regions: [
                (shape: "sphere", name: "hangar", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), size: (5,5,5), props: []),
            ], instances: [])"#,
        )
        .unwrap();
        let mut layout = MapLayout::from_manifest(&m);
        let mission: MissionDef = ron::from_str(
            r#"(map: "s", mode: "ass", source_game: "bf2", teams: [
                (faction: "rep", units: 1, tickets: 1, classes: []),
                (faction: "cis", units: 1, tickets: 1, classes: [])],
                ai_alive_cap: 1, spawn_delay: 1.0, spawn_interval: 1.0, cps: [],
                death_regions: [], landing_regions: ["hangar", "not_here"])"#,
        )
        .unwrap();
        assert_eq!(layout.apply_mission(&mission).landing_regions.len(), 1);
    }
}

#[cfg(test)]
mod tests;
