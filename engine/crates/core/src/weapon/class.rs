//! Weapon class interpretation, presets and capability queries.

#[cfg(doc)]
use super::WeaponState;

use super::{
    pair, pf, Falloff, OrdBlend, OrdKind, OrdnanceClass, PropSource, WeaponClass, DEG,
    HASH_ZOOM_SPREAD, SNIPER_LOCK_ON_ANGLE, SNIPER_LOCK_ON_RANGE, TARGET_SOLDIER_KINDS,
    TIGHT_HOLD_CAP, TIGHT_HOLD_SECS, TIGHT_HOLD_SECS_AI,
};

impl WeaponClass {
    /// Reach of an actual shot. Cone weapons query ConeLength, not the
    /// otherwise unused projectile velocity and lifespan (PS2 emitter create).
    pub fn shot_range(&self) -> f32 {
        self.emitter
            .as_ref()
            .filter(|e| e.is_cone())
            .map_or_else(|| self.shot_ordnance().max_range(), |e| e.cone_length)
    }

    pub fn shot_ordnance(&self) -> OrdBlend {
        self.ordnance.charged(if self.max_pressed_time > 0.0 {
            0.0
        } else {
            1.0
        })
    }

    /// The stage a charge reaches: the PS2 steps on while the charge is over
    /// the next stage's threshold (B2 `FUN_00327168`). 0 is the root.
    pub fn stage_for(&self, charge: f32) -> usize {
        self.stages
            .iter()
            .take_while(|s| charge > s.threshold)
            .count()
    }

    /// Stage `k`'s class (0, or past the last: the root / the last).
    pub fn stage(&self, k: usize) -> &WeaponClass {
        match k {
            0 => self,
            k => self.stages.get(k - 1).map_or(self, |s| &*s.class),
        }
    }

    /// Stage `k`'s entry in `Sim::weapon_classes` for a root registered at
    /// `root` (an unregistered stage: the root's).
    pub fn stage_index(&self, root: usize, k: usize) -> usize {
        match k {
            0 => root,
            k => self
                .stages
                .get(k - 1)
                .map(|s| s.index)
                .filter(|i| *i != usize::MAX)
                .unwrap_or(root),
        }
    }

    /// OURS (card SPRD, a named deviation; FEEL_NOTES): the extra free
    /// buffer under the spread state's floor, radians - what
    /// [`TIGHT_HOLD_SECS`] ([`TIGHT_HOLD_SECS_AI`] for a bot) of fire at the
    /// weapon's full cadence would grow (`mult` = the shooter's
    /// [`WeaponState::spread_mult`]), capped at the class's SpreadLimit and
    /// [`TIGHT_HOLD_CAP`]. 0 for a weapon whose spread never outruns its
    /// recovery.
    /// measure: fixed-cadence held-fire spread against PS2 shots before replacing the port
    /// tight-hold floor and time window.
    pub fn tight_hold(&self, human: bool, mult: f32) -> f32 {
        let mean = |d: [f32; 2]| 0.5 * (d[0] + d[1]);
        let salvos = self.salvo_count.max(1) as f32;
        let period = mean(self.shot_delay)
            + self.initial_salvo_delay
            + (salvos - 1.0) * mean(self.salvo_delay);
        if period <= 0.0 {
            return 0.0;
        }
        let net = salvos * mult * self.spread_per_shot / period - self.spread_recover_rate;
        let secs = if human {
            TIGHT_HOLD_SECS
        } else {
            TIGHT_HOLD_SECS_AI
        };
        (secs * net.max(0.0))
            .min(self.spread_limit)
            .min(TIGHT_HOLD_CAP)
    }

    /// BF1 `all_weap_inf_rifle` + `all_weap_inf_rifle_ord` (SIDE/ALL.LVL,
    /// byte-verified, report §8.1).
    pub fn bf1_rifle() -> Self {
        Self {
            name: "all_weap_inf_rifle".into(),
            rounds_per_clip: 50,
            ammo_per_shot: 1,
            reload_time: 1.5,
            trigger_single: false,
            display_refire: true,
            // The DISC value (`re-ai-sniper.md` §3), not the class-ctor
            // default: this fixture is byte-verified against the ODF and
            // `fill_classes_keep_the_v0_fixtures` compares the two whole.
            // It changes nothing here — the rifle fails the table's other
            // two gates anyway. (LEAD ADDENDUM 4b says 1.0 for this
            // constructor; see the report's Deviations.)
            zoom_max: 2.5,
            zoom_min: 2.5,
            zoom_rate: 0.0,
            zoom_turn_div: [2.5, 2.5],
            zoom_first_person: false,
            ai_bubble_size: 1.0,
            ai_bubble_dist: 20.0,
            ai_bubble_clamp: 10.0,
            max_pressed_time: 0.0,
            lock_time: 0.0,
            lock_on_range: 64.0,
            target_lock_max: super::TARGET_LOCK_MAX_DEFAULT,
            target_lock_lose: super::TARGET_LOCK_LOSE_DEFAULT,
            // LockOnAngle 1.0; the bolt's reach 80 × 1.5 (OrdnanceName after
            // LockOnRange); TargetEnemy 1, TargetPerson/Animal/Droid 1,
            // TargetVehicle/Building 0 (every BF1 side's rifle, disc census).
            lock_on_angle: DEG,
            extreme_range: 120.0,
            ai_range: [0.0, 16.0, 48.0],
            barrage: [3, 6],
            barrage_delay: 2.0,
            target_team: 1,
            target_kind: 0x07,
            ai_target_kind: 0x07,
            heat_per_shot: 0.0,
            heat_recover_rate: 0.0,
            heat_threshold: 0.0,
            shot_delay: [0.2, 0.2],
            salvo_count: 1,
            shots_per_salvo: 1,
            salvo_delay: [0.0, 0.0],
            initial_salvo_delay: 0.0,
            pitch_spread: 0.25 * DEG,
            yaw_spread: 0.25 * DEG,
            shot_tilts: Vec::new(),
            stages: Vec::new(),
            posture_spread: [[0.0; 3]; 3],
            spread_per_shot: 1.1 * DEG,
            spread_recover_rate: 4.8 * DEG,
            spread_threshold: 1.6 * DEG,
            spread_limit: 6.0 * DEG,
            ordnance: OrdnanceClass {
                velocity: 80.0,
                life_span: 1.5,
                max_damage: 60.0,
                scales: [1.0, 1.0, 1.0, 0.0, 0.1, 0.1, 0.1],
                shield_scale: 1.0,
                falloff: None,
                team_bonus_scale: 0.0,
                ordnance_sound: Some("com_weap_inf_ord_hum_sm".into()),
                collision_sound: Some("imp_weap_ord_exp".into()),
                geometry: None,
                // re-bolt §2's BF1 rifle row.
                laser: Some(crate::laser::LaserLook {
                    texture: Some("redlaser_D".into()),
                    glow_color: [253, 43, 43, 100],
                    // Parsed from the disc; BF1 never reads it (§5.1).
                    light_color: [148, 25, 25, 255],
                    light_radius: 4.0,
                    ..crate::laser::LaserLook::rifle()
                }),
                // re-fx §7's census: every surface ord_explosion.
                sticky: None,
                explosion: None,
                kind: OrdKind::Other,
                gravity: 0.0,
                expire: None,
                knots: Vec::new(),
                min_damage: None,
                missile: None,
                fat_ray: None,
                beam: None,
                tow: None,
                impact: Some(crate::impact::ImpactLook {
                    soft: Some("ord_explosion".into()),
                    rigid: Some("ord_explosion".into()),
                    building: Some("ord_explosion".into()),
                    terrain: Some("ord_explosion".into()),
                    water: Some("watersplash_ord".into()),
                    shield: Some("ord_explosion".into()),
                    ..Default::default()
                }),
            },
            secondary: None,
            fire_sound: Some("all_weap_inf_rifle_fire".into()),
            fire_loop_sound: None,
            muzzle_effect: None,
            reload_sound: Some("all_weap_inf_reload_med".into()),
            empty_sound: Some("com_weap_inf_ammo_empty".into()),
            change_sound: Some("all_weap_inf_equip_med".into()),
            // re-muzzle-flash §2's BF1 Alliance rifle row.
            thrown: None,
            offhand_weapon: false,
            medals_type: None,
            repair: None,
            dispenser: None,
            mine: None,
            ability: None,
            force: None,
            saber_throw: None,
            saber: None,
            emitter: None,
            cloak: None,
            guided: None,
            remote: None,
            turret: None,
            flash: Some(crate::flash::MuzzleFlashLook {
                radius: 0.6,
                length: 0.025,
                color: [255, 100, 100, 255],
                model: Some(crate::flash::BF1_MODEL.into()),
                ..crate::flash::MuzzleFlashLook::default()
            }),
        }
    }

    /// BF2 `rep_weap_inf_rifle` → `com_weap_inf_rifle` + `com_weap_inf_rifle_ord`
    /// (SIDE/REP.LVL, byte-verified, report §8.2).
    pub fn bf2_rifle() -> Self {
        Self {
            name: "com_weap_inf_rifle".into(),
            lock_time: 0.0,
            lock_on_range: 40.0,
            target_lock_max: super::TARGET_LOCK_MAX_DEFAULT,
            target_lock_lose: super::TARGET_LOCK_LOSE_DEFAULT,
            // The bolt's reach 160 × 0.8 (the BF1 rule, PROVISIONAL for BF2:
            // B2 FUN_00328fb0's OrdnanceName default is unread). The Target*
            // bits and LockOnAngle 1.0 are the disc's, as BF1's.
            // trace: BF2 rifle ordnance-name inheritance and shipped bolt velocity/lifetime
            // before replacing this fixture reach.
            extreme_range: 128.0,
            ai_range: [0.0, 24.0, 64.0],
            barrage: [3, 6],
            barrage_delay: 2.0,
            spread_per_shot: 1.2 * DEG,
            spread_limit: 5.0 * DEG,
            ordnance: OrdnanceClass {
                velocity: 160.0,
                life_span: 0.8,
                max_damage: 80.0,
                scales: [1.0, 1.0, 1.0, 0.1, 0.1, 0.1, 0.1],
                shield_scale: 1.0,
                falloff: Some(Falloff {
                    start_time: 0.4,
                    duration: 0.2,
                    end_damage: 50.0,
                }),
                team_bonus_scale: 0.25,
                ordnance_sound: Some("com_weap_inf_ord_hum_sm".into()),
                collision_sound: Some("imp_weap_ord_exp".into()),
                geometry: None,
                // re-bolt §2's BF2 Republic rifle row (red End inherited).
                laser: Some(crate::laser::LaserLook {
                    texture: Some("com_sfx_laser_blue".into()),
                    glow_color: [92, 136, 250, 100],
                    end_glow_color: [160, 43, 43, 100],
                    fade_light: true,
                    fade_start: 0.4,
                    fade_duration: 0.2,
                    light_color: [36, 53, 98, 255],
                    end_light_color: [94, 25, 25, 255],
                    light_radius: 4.0,
                    ..crate::laser::LaserLook::rifle()
                }),
                // re-fx §7's census: every surface com_sfx_ord_exp; BF2
                // never reads ImpactEffectShield, so the ctor's stands.
                sticky: None,
                explosion: None,
                kind: OrdKind::Other,
                gravity: 0.0,
                expire: None,
                knots: Vec::new(),
                min_damage: None,
                missile: None,
                fat_ray: None,
                beam: None,
                tow: None,
                impact: Some(crate::impact::ImpactLook {
                    soft: Some("com_sfx_ord_exp".into()),
                    rigid: Some("com_sfx_ord_exp".into()),
                    building: Some("com_sfx_ord_exp".into()),
                    terrain: Some("com_sfx_ord_exp".into()),
                    water: Some("com_sfx_watersplash_ord".into()),
                    shield: Some("com_sfx_shieldimpact_sm".into()),
                    ..Default::default()
                }),
            },
            fire_sound: Some("rep_weap_inf_rifle_fire".into()),
            reload_sound: Some("com_weap_inf_reload_med".into()),
            empty_sound: Some("com_weap_inf_ammo_empty".into()),
            change_sound: Some("com_weap_inf_equip_med".into()),
            // `com_weap_inf_rifle`'s ScoreForMedalsType (the rifle award).
            medals_type: Some(1),
            // re-muzzle-flash §2's BF2 Republic rifle row.
            flash: Some(crate::flash::MuzzleFlashLook {
                radius: 0.6,
                length: 0.025,
                color: [100, 100, 255, 255],
                model: Some(crate::flash::BF2_MODEL.into()),
                ..crate::flash::MuzzleFlashLook::default()
            }),
            ..Self::bf1_rifle()
        }
    }

    pub fn award_locked(&self, all_awards: bool) -> bool {
        !all_awards && self.guided.as_ref().is_some_and(|g| g.requires_award)
    }

    pub fn dispenses(&self) -> bool {
        self.dispenser.is_some()
            || self.mine.is_some()
            || self.remote.is_some()
            || self.turret.is_some()
    }

    /// Name of the ordnance class this weapon fires (`OrdnanceName` /
    /// legacy `Ordnance`).
    pub fn ordnance_name(src: &dyn PropSource) -> Option<String> {
        src.prop("OrdnanceName")
            .or_else(|| src.prop("Ordnance"))
            .map(|s| s.trim().to_owned())
    }

    /// Parse a `wpnc` PROP list (BF1 FUN_002864f0 + FUN_00289b48; BF2
    /// FUN_00328fb0 + FUN_00330810) with the traced transforms: deg→rad,
    /// ShotDelay/SalvoDelay pairs, min clamps, `MaxSpread` and
    /// `SpreadRecover` legacy aliases, RoundsPerClip < 1 → infinite.
    pub fn from_props(name: &str, src: &dyn PropSource, ordnance: OrdnanceClass) -> Self {
        let mut w = Self {
            name: name.to_owned(),
            ordnance,
            ..Self::default()
        };
        if let Some(v) = pf(src, "RoundsPerClip") {
            w.rounds_per_clip = if v < 1.0 { 0 } else { v as u32 };
        }
        if let Some(v) = pf(src, "AmmoPerShot") {
            w.ammo_per_shot = v.max(0.0) as u32;
        }
        if let Some(v) = pf(src, "ReloadTime") {
            w.reload_time = v.max(0.01);
        }
        if let Some(v) = pf(src, "TriggerSingle") {
            w.trigger_single = v != 0.0;
        }
        if let Some(v) = pf(src, "DisplayRefire") {
            w.display_refire = v != 0.0;
        }
        if let Some(v) = pf(src, "OffhandWeapon") {
            w.offhand_weapon = v != 0.0;
        }
        if let Some(v) = pf(src, "ScoreForMedalsType") {
            w.medals_type = Some(v as i8);
        }
        // The zoom keys (`analysis/re-zoom.md` §2): a written 0 reads 1.0
        // (B2:320352-320361, B1:209974-209983), and `ZoomMin` / `ZoomMax`
        // are copied to the turn divisors. The PS2 applies the keys in file
        // order, so a divisor key BEFORE its zoom key is overwritten there;
        // here the divisor keys always win (no disc class authors one).
        let one = |v: f32| if v == 0.0 { 1.0 } else { v };
        if let Some(v) = pf(src, "ZoomMin") {
            w.zoom_min = one(v);
            w.zoom_turn_div[0] = w.zoom_min;
        }
        if let Some(v) = pf(src, "ZoomMax") {
            w.zoom_max = one(v);
            w.zoom_turn_div[1] = w.zoom_max;
        }
        if let Some(v) = pf(src, "ZoomRate") {
            w.zoom_rate = v;
        }
        if let Some(v) = pf(src, "ZoomTurnDivisorMin") {
            w.zoom_turn_div[0] = one(v);
        }
        if let Some(v) = pf(src, "ZoomTurnDivisorMax") {
            w.zoom_turn_div[1] = one(v);
        }
        if let Some(v) = pf(src, "ZoomFirstPerson") {
            w.zoom_first_person = v != 0.0;
        }
        if let Some(v) = pf(src, "AIBubbleSizeMultiplier") {
            w.ai_bubble_size = v;
        }
        if let Some(v) = pf(src, "AIBubbleScaleDistDivider") {
            w.ai_bubble_dist = v;
        }
        if let Some(v) = pf(src, "AIBubbleScaleClamp") {
            w.ai_bubble_clamp = v;
        }
        if let Some(v) = pf(src, "MaxPressedTime") {
            w.max_pressed_time = v;
        }
        if let Some(v) = pf(src, "LockOnRange") {
            w.lock_on_range = v;
        }
        if let Some(v) = pf(src, "TargetLockMaxDistance") {
            w.target_lock_max = v;
        }
        if let Some(v) = pf(src, "TargetLockMaxDistanceLose") {
            w.target_lock_lose = v;
        }
        // The cone: both keys write C+0xd8, so which wins depends on the
        // ODF's order; no shipped class writes both.
        if let Some(v) = pf(src, "AutoAimSize") {
            w.lock_on_angle = v * 0.072;
        }
        if let Some(v) = pf(src, "LockOnAngle") {
            w.lock_on_angle = v * DEG;
        }
        // ExtremeRange (C+0xd0). On the PS2 the ExtremeRange and LockOnRange
        // setters clamp it up to LockOnRange, and OrdnanceName overwrites it
        // with the ordnance's reach, unclamped (B1:210249-210258,
        // 209842-209855). `PropSource` keeps no order, so the port's rule is
        // the shipped one: every BF1 blaster names its ordnance after
        // LockOnRange and writes no ExtremeRange, so a class with an
        // ordnance and no ExtremeRange takes the reach (Velocity × LifeSpan,
        // FUN_00277d30); only the mortars write ExtremeRange, and they have
        // no magnet (LockOnRange 0).
        w.extreme_range = match pf(src, "ExtremeRange") {
            Some(v) => v.max(w.lock_on_range),
            None if Self::ordnance_name(src).is_some() => {
                w.ordnance.velocity * w.ordnance.life_span
            }
            None => w.lock_on_range,
        };
        for (slot, key) in ["MinRange", "OptimalRange", "MaxRange"].iter().enumerate() {
            if let Some(v) = pf(src, key) {
                w.ai_range[slot] = v;
            }
        }
        for (slot, key) in ["BarrageMin", "BarrageMax"].iter().enumerate() {
            if let Some(v) = pf(src, key) {
                w.barrage[slot] = v.clamp(0.0, 255.0) as u8;
            }
        }
        if let Some(v) = pf(src, "BarrageDelay") {
            w.barrage_delay = v;
        }
        let flag = |key: &str| pf(src, key).map(|v| v != 0.0);
        let update_mask = |word: &mut u32, keys: &[&str]| {
            for (bit, key) in keys.iter().enumerate() {
                match flag(key) {
                    Some(true) => *word |= 1 << bit,
                    Some(false) => *word &= !(1 << bit),
                    None => {}
                }
            }
        };
        update_mask(
            &mut w.target_team,
            &["TargetEnemy", "TargetNeutral", "TargetFriendly"],
        );
        if let Some(all) = flag("TargetAll") {
            w.target_kind = if all { 0x1f } else { 0 };
        }
        update_mask(
            &mut w.target_kind,
            &[
                "TargetPerson",
                "TargetAnimal",
                "TargetDroid",
                "TargetVehicle",
                "TargetBuilding",
            ],
        );
        w.ai_target_kind = w.target_kind;
        update_mask(
            &mut w.ai_target_kind,
            &[
                "AITargetPerson",
                "AITargetAnimal",
                "AITargetDroid",
                "AITargetVehicle",
                "AITargetBuilding",
            ],
        );
        if let Some(v) = pf(src, "HeatPerShot") {
            w.heat_per_shot = v;
        }
        if let Some(v) = pf(src, "HeatRecoverRate") {
            w.heat_recover_rate = v;
        }
        if let Some(v) = pf(src, "HeatThreshold") {
            w.heat_threshold = v;
        }
        if let Some(p) = pair(src, "ShotDelay") {
            w.shot_delay = [p[0].max(0.01), p[1].max(p[0].max(0.01))];
        }
        if let Some(v) = pf(src, "SalvoCount") {
            w.salvo_count = v.max(0.0) as u32;
        }
        if let Some(v) = pf(src, "ShotsPerSalvo") {
            w.shots_per_salvo = v.max(0.0) as u32;
        }
        if let Some(p) = pair(src, "SalvoDelay") {
            w.salvo_delay = p;
        }
        if let Some(v) = pf(src, "InitialSalvoDelay") {
            w.initial_salvo_delay = v;
        }
        // Legacy MaxSpread: pitch = 0.025·v, yaw = 0.05·v (NOT deg→rad).
        if let Some(v) = pf(src, "MaxSpread") {
            w.pitch_spread = 0.025 * v;
            w.yaw_spread = 0.05 * v;
        }
        if let Some(v) = pf(src, "PitchSpread") {
            w.pitch_spread = v * DEG;
        }
        if let Some(v) = pf(src, "YawSpread") {
            w.yaw_spread = v * DEG;
        }
        for (p, stance) in ["Stand", "Crouch", "Prone"].iter().enumerate() {
            if let Some(v) = pf(src, &format!("{stance}StillSpread")) {
                w.posture_spread[p][0] = v * DEG;
            }
            if let Some(v) = pf(src, &format!("{stance}MoveSpread")) {
                w.posture_spread[p][1] = v * DEG;
            }
            if let Some(v) = src
                .prop_hash(HASH_ZOOM_SPREAD[p])
                .and_then(|s| s.trim().parse::<f32>().ok())
            {
                w.posture_spread[p][2] = v * DEG;
            }
        }
        if let Some(v) = pf(src, "SpreadPerShot").or_else(|| {
            src.prop_hash(0xEF49_35B0)
                .and_then(|s| s.trim().parse().ok())
        }) {
            w.spread_per_shot = v * DEG;
        }
        // Legacy SpreadRecover: rate = v × SpreadPerShot, limit = rate / ShotDelay.
        if let Some(v) = pf(src, "SpreadRecover") {
            w.spread_recover_rate = v * w.spread_per_shot;
            w.spread_limit = w.spread_recover_rate / w.shot_delay[0];
        }
        if let Some(v) = pf(src, "SpreadRecoverRate") {
            w.spread_recover_rate = v * DEG;
        }
        if let Some(v) = pf(src, "SpreadThreshold") {
            w.spread_threshold = v * DEG;
        }
        if let Some(v) = pf(src, "SpreadLimit") {
            w.spread_limit = v * DEG;
        }
        w.fire_sound = src.prop("FireSound").map(|s| s.trim().to_owned());
        w.fire_loop_sound = src.prop("FireLoopSound").map(|s| s.trim().to_owned());
        w.muzzle_effect = src.prop("MuzzleFlashEffect").map(|s| s.trim().to_owned());
        w.reload_sound = src.prop("ReloadSound").map(|s| s.trim().to_owned());
        w.empty_sound = src.prop("FireEmptySound").map(|s| s.trim().to_owned());
        w.change_sound = src.prop("WeaponChangeSound").map(|s| s.trim().to_owned());
        w
    }

    pub fn infinite_ammo(&self) -> bool {
        self.rounds_per_clip == 0
    }

    /// Can a human zoom this weapon at all (`re-zoom.md` §4 step 4: a class
    /// whose `ZoomMax` is 1.0 or less never)?
    pub fn zooms(&self) -> bool {
        self.zoom_max > 1.0
    }

    /// The zoom press (B1 `FUN_00283c80`, B2 `FUN_003259a0`, §3): from off,
    /// step 0 (`ZoomMin`); from step 0 on to step 1 (`ZoomMax`) when the two
    /// differ and the class has no `ZoomRate`; else off. `step` is the step
    /// the weapon is at, `None` when not zoomed.
    pub fn zoom_press(&self, step: Option<u8>) -> Option<u8> {
        match step {
            None => Some(0),
            Some(0) if self.zoom_min != self.zoom_max && self.zoom_rate == 0.0 => Some(1),
            Some(_) => None,
        }
    }

    /// The zoom value and the turn divisor at `step`.
    pub fn zoom_at(&self, step: u8) -> (f32, f32) {
        let k = usize::from(step.min(1));
        ([self.zoom_min, self.zoom_max][k], self.zoom_turn_div[k])
    }

    /// The CLASS half of the PS2's AI sniper-dispersion predicate
    /// (`GetYawSpread`, BF2 `FUN_0032e4b8` B2:323340-323346 / BF1
    /// `FUN_002889e8` B1:211248-211254 — `analysis/re-ai-sniper.md` §2
    /// conditions 1, 3 and 4). The other two conditions are the caller's: the
    /// shooter is an AI (fire point owner index < 0) and it has a current
    /// target — [`crate::ai::think`] answers both.
    ///
    /// `>=` is load-bearing: `imp_weap_inf_sniperrifle` ships `ZoomMax`
    /// exactly 6.0 (§3), and a `>` would drop the Scout Trooper out of the
    /// table. Note `yaw_spread` is written by `YawSpread` AND by the legacy
    /// `MaxSpread`, exactly as on the PS2.
    pub fn ai_distance_table(&self) -> bool {
        self.yaw_spread == 0.0 && self.trigger_single && self.zoom_max >= 6.0
    }

    /// A sniper rifle: a scope weapon (`ZoomFirstPerson`) in the AI's
    /// sniper table. On the discs: BF1's four `*_weap_inf_sniperrifle`, BF2's
    /// `com_weap_inf_sniper_rifle` and `com_weap_award_sniper_rifle` and
    /// their side children. The bowcaster, the award and target pistols and
    /// the scoped vehicle guns fail the table.
    pub fn is_sniper_rifle(&self) -> bool {
        self.zoom_first_person && self.ai_distance_table()
    }

    /// OURS (the maintainer 2026-10-28: "give bf1 and bf2 snipers aim assist with
    /// controller"): every disc sniper ships `LockOnRange 0`, so it never has
    /// a magnet. Give a sniper rifle one: the reach is the rifle's own
    /// `MaxRange` (128 m on every disc sniper, the AI's band for it), the cone
    /// its own `LockOnAngle` (1° on every disc sniper, the rifle's), the kinds
    /// it may take soldiers, animals and droids (the disc's bits) plus
    /// vehicles when its round hurts them (BF2's VehicleScale 0.1; BF1's is
    /// 0). The cone's length is at least the new reach. The mouse rule stays:
    /// a mouse-aimed pad is never pulled (`Sim::step` stage 2). Only the
    /// human's magnet reads these fields; the AI's own kinds
    /// (`ai_target_kind`) are left as parsed.
    /// measure: controller sniper-assist acquisition and pull at staged distances before
    /// replacing these deliberately added magnet values.
    pub fn give_sniper_assist(&mut self) {
        if !self.is_sniper_rifle() || self.lock_on_range > 0.0 {
            return;
        }
        self.lock_on_range = if self.ai_range[2] > 0.0 {
            self.ai_range[2]
        } else {
            SNIPER_LOCK_ON_RANGE
        };
        if self.lock_on_angle <= 0.0 {
            self.lock_on_angle = SNIPER_LOCK_ON_ANGLE;
        }
        self.target_team |= 1;
        self.target_kind |= TARGET_SOLDIER_KINDS;
        if self.ordnance.scales[3] > 0.0 {
            self.target_kind |= 1 << 3;
        }
        self.extreme_range = self.extreme_range.max(self.lock_on_range);
    }

    /// An infantry rocket launcher: the weapons whose vehicle critical hits
    /// count toward BF2's demolition award ("%s rocket launcher critical
    /// hits on a vehicle", `analysis/re-hitzones.md` §3). BF2: the PS2's
    /// own key, `ScoreForMedalsType` 2 (`com_weap_inf_rocket_launcher` and
    /// the four sides' `*_weap_inf_rocket_launcher`, the award and hero
    /// guided launchers; not the EMP launcher, which writes -1). BF1 has no
    /// award key: its infantry launchers by name, `*_weap_inf_rocket_launcher`
    /// and `*_weap_inf_torpedo_launcher` (the four sides' rocket launchers).
    pub fn rocket_launcher(&self) -> bool {
        match self.medals_type {
            Some(t) => t == 2,
            None => {
                let n = self.name.to_ascii_lowercase();
                n.contains("_weap_inf_")
                    && (n.ends_with("rocket_launcher") || n.ends_with("torpedo_launcher"))
            }
        }
    }
}
