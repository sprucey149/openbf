//! The static gameplay digest (issue #168, `analysis/plan/LAN_MULTIPLAYER.md`
//! §4.3 and §7): a hash over everything the sim READS that [`Sim::hash`]
//! does not cover and that comes from the map, the class tables, the
//! mission or the session settings. Two peers that built the same round
//! from different copies of the game agree on it exactly when those copies
//! simulate the same; a mismatch names its group
//! ([`Sim::static_digest_groups`]).
//!
//! Every field of [`Sim`] is named below, so a new field does not compile
//! until it is placed in a group or marked as hashed or live state. Take the
//! digest right after the round is built (tick 0): a few groups ("props",
//! "modes") carry the live state of setup objects, which is equal on every
//! peer at tick 0 and moves afterwards.
//!
//! Not covered: the game's animation providers `hero_rider_frames` and
//! `tow_frames` (trait objects with no data view; they come from the same
//! baked animations the class tables do).
//!
//! Only what the simulation reads (issue #168, LAN §7: PS2, PC and Xbox
//! copies must agree when they simulate alike): the class tables are fed
//! without their sound, effect and texture names
//! ([`crate::digest::PRESENTATION_FIELDS`]); `vehicle_damage_effects` (drawn
//! only) is left out. The terrain is digested whole: nothing in the sim keeps
//! a soldier or ground vehicle inside the boundary (it bounds only flyers and
//! remote droids), and a query off the grid clamps onto its edge, so every
//! cell can be read; the bake corrects PC and Xbox copies over the whole grid.

use super::*;
use crate::digest::Fnv64;

/// The digest groups in the order [`Sim::static_digest_groups`] lists them.
pub const STATIC_DIGEST_GROUPS: [&str; 12] = [
    "mission",
    "session",
    "weapons",
    "classes",
    "modes",
    "vehicles",
    "props",
    "collision",
    "terrain",
    "water",
    "bounds",
    "nav",
];

impl Sim {
    /// The hash ([`crate::digest::Fnv64`]) of [`Sim::static_digest_groups`]: each group's name and
    /// digest, in order.
    pub fn static_digest(&self) -> u64 {
        let mut f = Fnv64::new();
        for (name, digest) in self.static_digest_groups() {
            f.eat(name.as_bytes());
            f.u64(digest);
        }
        f.finish()
    }

    /// The static digest by group ([`STATIC_DIGEST_GROUPS`]), so a peer whose
    /// copy differs can be told which part of the game differs.
    pub fn static_digest_groups(&self) -> Vec<(&'static str, u64)> {
        let Sim {
            // Hashed by `Sim::hash`, or live state it covers.
            tick: _,
            difficulty: _,
            team_difficulty: _,
            rng: _,
            lcg: _,
            melee_lcg: _,
            melee_lcg_active: _,
            tow_cables: _,
            tow_falls: _,
            spawn_choice: _,
            prev_spawn_bits: _,
            held_eaten: _,
            spawn_post_choice: _,
            spawn_screens: _,
            spawn_stage: _,
            side_choice: _,
            class_carousel: _,
            class_ammo: _,
            ai_spawned: _,
            units: _,
            vehicles: _,
            bolts: _,
            beams: _,
            stickies: _,
            mines: _,
            next_mine_id: _,
            guided: _,
            next_guided_id: _,
            droids: _,
            next_droid_id: _,
            strikes: _,
            turrets: _,
            next_turret_id: _,
            pickups: _,
            battle: _,
            medal_careers: _,
            assist: _,
            assist_lead: _,
            magnet: _,
            magnet_hidden: _,
            lock_on: _,
            foot_lock: _,
            players: _,
            domination: _,
            outcome: _,
            waves: _,
            native_teams: _,
            hazards: _,
            last_warning_id: _,
            commander: _,
            arc_fails: _,
            tokens: _,
            focus_born: _,
            // Per-tick inputs, outputs, caches and measurements.
            spawn_points: _,
            spawn_costs: _,
            reticle_pick: _,
            events: _,
            press_edges: _,
            shot_flights: _,
            target_hints: _,
            unit_grid: _,
            ai_updates: _,
            ai_pads: _,
            hit_history: _,
            view_lags: _,
            nav_stats: _,
            ai_wall_ms: _,
            tick_cost: _,
            bot_census: _,
            think_hist: _,
            think_threads: _,
            // Trait objects with no data view (module header).
            hero_rider_frames: _,
            tow_frames: _,
            // mission
            time_limit,
            respawn_delay,
            ai_alive_cap,
            team_combatants,
            friendly_fire_scale,
            mission_ai,
            omni_radius,
            ground_rules,
            extra_humans,
            kill_floor,
            item_pool,
            flyer_bots,
            ground_flyer_map,
            ally_spawns,
            team_relations,
            // session
            team_handicap_on,
            cheats,
            slot_cheats,
            assist_intensity,
            pad_look_sensitivity,
            assist_armed,
            assist_lead_on,
            slot_assist,
            // weapons, classes, modes
            weapon_classes,
            classes,
            item_classes,
            station_classes,
            test_rosters,
            heroes,
            deathmatch,
            space_assault,
            ctf,
            // vehicles
            hover_params,
            walker_params,
            flyer_params,
            flight_limits,
            landing_regions,
            flyer_paths,
            seat_setup,
            blast_setup,
            // Drawn only: the game's smoke and fire on a damaged vehicle.
            vehicle_damage_effects: _,
            slice_time,
            repair_scale,
            weak_spot_factor,
            hull_parts,
            missile_target_points,
            // props
            cps,
            doors,
            vehicle_pads,
            building_turrets,
            stations,
            station_bodies,
            station_wreck_collision,
            station_live_collision,
            // collision, terrain, water, bounds
            collision,
            heightfield,
            water_level,
            water_mask,
            water_oscillates,
            boundary,
            death_regions,
            damage_regions,
            fly_ceiling,
            // nav
            nav,
            flank,
            positions,
            nav_floors,
            nav_unwalkable,
            nav_trapped,
            spawn_seats,
        } = self;
        let mut meshes = crate::collision::MeshDigests::new();
        let mut out = Vec::with_capacity(STATIC_DIGEST_GROUPS.len());
        let mut group = |name: &'static str, feed: &mut dyn FnMut(&mut Fnv64)| {
            let mut f = Fnv64::new();
            feed(&mut f);
            out.push((name, f.finish()));
        };
        group("mission", &mut |f| {
            f.debug(&(time_limit, respawn_delay, ai_alive_cap, team_combatants));
            f.debug(&(friendly_fire_scale, mission_ai, omni_radius, ground_rules));
            f.debug(&(extra_humans, kill_floor, item_pool, flyer_bots));
            f.debug(&(ground_flyer_map, ally_spawns, team_relations));
        });
        group("session", &mut |f| {
            f.debug(&(team_handicap_on, cheats, slot_cheats, assist_intensity));
            f.debug(&(pad_look_sensitivity, assist_armed, assist_lead_on));
            f.debug(slot_assist);
        });
        group("weapons", &mut |f| f.debug_sim(weapon_classes));
        group("classes", &mut |f| {
            f.debug_sim(&(classes, item_classes, station_classes));
        });
        group("modes", &mut |f| {
            f.debug_sim(&(test_rosters, heroes, deathmatch, space_assault, ctf));
        });
        group("vehicles", &mut |f| {
            f.debug_sim(&(hover_params, walker_params, flyer_params, flight_limits));
            f.debug_sim(&(landing_regions, flyer_paths, seat_setup, blast_setup));
            f.debug_sim(&(slice_time, repair_scale));
            f.debug_sim(&(weak_spot_factor, missile_target_points));
            f.count(hull_parts.len());
            for parts in hull_parts {
                match parts {
                    None => f.eat(b"-"),
                    Some(parts) => {
                        f.count(parts.len());
                        for p in parts.iter() {
                            p.feed_digest(f, &mut meshes);
                        }
                    }
                }
            }
        });
        group("props", &mut |f| {
            f.debug_sim(&(cps, doors, vehicle_pads, stations));
            f.count(building_turrets.len());
            for t in building_turrets {
                f.debug(&(&t.spec, t.rot, t.aim_sphere, t.instance, t.fall));
                f.debug(&(&t.posed, t.prefer_walkable_exit));
                for parts in [&t.fallen_parts, &t.standing_parts] {
                    f.count(parts.len());
                    for p in parts {
                        p.feed_digest(f, &mut meshes);
                    }
                }
            }
            f.debug(station_bodies);
            for set in [station_wreck_collision, station_live_collision] {
                f.count(set.len());
                for parts in set {
                    f.count(parts.len());
                    for p in parts {
                        p.feed_digest(f, &mut meshes);
                    }
                }
            }
        });
        group("collision", &mut |f| collision.feed_digest(f, &mut meshes));
        group("terrain", &mut |f| match heightfield {
            None => f.eat(b"-"),
            Some(h) => {
                f.u32(h.size);
                f.f32s(&[h.unit, h.offset_x, h.offset_z]);
                f.count(h.heights.len());
                f.f32s(&h.heights);
            }
        });
        group("water", &mut |f| {
            f.debug(&(water_level, water_mask, water_oscillates));
        });
        group("bounds", &mut |f| {
            f.debug(&(boundary, death_regions, damage_regions, fly_ceiling));
        });
        group("nav", &mut |f| {
            match nav {
                None => f.eat(b"-"),
                Some(g) => g.feed_digest(f),
            }
            // The table's build statistics hold a wall-clock time: only
            // its positions (its cell index follows from them).
            f.debug(&(flank, &positions.positions, nav_floors));
            f.debug(&(nav_unwalkable, nav_trapped, spawn_seats));
        });
        debug_assert!(out.iter().map(|g| g.0).eq(STATIC_DIGEST_GROUPS));
        out
    }
}
