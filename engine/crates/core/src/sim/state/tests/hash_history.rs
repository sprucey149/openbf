//! Reachable history plus local continuation probes; these are not claims of
//! complete ordinary input histories colliding in every other serialized field.
use super::*;
use crate::sim::{buttons, AimTarget};
use crate::soldier::{GroundRules, Soldier, SoldierConstants, Stance};
use crate::sticky::{Anchor, Contact, Sticky, StickyClass, Surface};
use crate::TICK_DT as DT;

fn soldier_bytes(s: &Soldier) -> Vec<u8> {
    let mut out = Vec::new();
    s.hash_into(&mut |b| out.extend_from_slice(b));
    out
}

fn sticky_bytes(g: &Sticky) -> Vec<u8> {
    let mut out = Vec::new();
    g.hash_into(&mut |b| out.extend_from_slice(b));
    out
}

#[test]
fn firing_assist_history_changes_quantized_continuation_and_hash_slot() {
    let (mut s, p, target) = crate::sim::tests::duel();
    let m = AimTarget::Unit(target);
    let base = s.hash();
    s.apply_assist(
        0,
        p,
        m,
        &PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        },
        DT,
    );
    assert!(s.assist[0].magnet_timer > 0.0);
    let fired = s.hash();
    assert_ne!(base, fired);
    // Moving the same positive history to a second supported player slot
    // must differ even when all other serialized fields are held fixed.
    let second = UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0));
    s.spawn(&second).unwrap();
    let timer = s.assist[0];
    let slot0 = s.hash();
    s.assist[0] = AssistState::default();
    s.assist[1] = timer;
    assert_ne!(slot0, s.hash());
    s.assist[1] = AssistState::default();
    s.units[target.index as usize].soldier.pos.x = 2.0;
    let cold = s.apply_assist(0, p, m, &PadState::default(), DT);
    s.assist[0] = timer;
    let warm = s.apply_assist(0, p, m, &PadState::default(), DT);
    assert_ne!(
        cold.rx, warm.rx,
        "positive history survives pad quantization"
    );
    // No target means no apply; the normal tick preserves the timer.
    s.kill_environment(target, DeathCause::Fall);
    let frozen = s.assist[0].magnet_timer;
    s.step(&[PadState::default(), PadState::default()]);
    assert_eq!(s.assist[0].magnet_timer, frozen);
    s.add_cp(
        crate::sim::tests::sphere_cp("home", Vec3::ZERO, 8.0, false),
        Some(0),
    );
    s.kill_environment(p, DeathCause::Fall);
    s.respawn_slot(p.index as usize);
    let respawned = s.players[0].unwrap();
    assert_eq!(respawned.index, p.index);
    assert_ne!(respawned.gen, p.gen);
    assert!(s.unit(respawned).is_some());
    assert_eq!(
        s.assist[0].magnet_timer, frozen,
        "respawn retains controller history"
    );
}

#[test]
fn inactive_assist_residue_keeps_the_default_stream() {
    let mut s = Sim::new(42);
    let original = s.hash();
    // Reach a negative decay residue through apply, without inventing a float.
    let (mut firing, p, target) = crate::sim::tests::duel();
    let m = AimTarget::Unit(target);
    firing.apply_assist(
        0,
        p,
        m,
        &PadState {
            buttons: buttons::FIRE,
            ..PadState::default()
        },
        DT,
    );
    for _ in 0..40 {
        firing.apply_assist(0, p, m, &PadState::default(), DT);
    }
    assert!(firing.assist[0].magnet_timer <= 0.0);
    s.assist[0] = firing.assist[0];
    assert_eq!(original, s.hash());
}

#[test]
fn hazard_owner_survives_slot_reuse_and_changes_predelay_flee() {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    let spec = UnitSpec::trooper(0, Vec3::ZERO);
    let old = s.spawn(&spec).unwrap();
    s.add_hazard_by(Vec3::new(1.0, 0.0, 0.0), 1000, Some(old));
    s.kill_environment(old, DeathCause::Fall);
    let current = s.spawn(&spec).unwrap();
    assert_eq!(old.index, current.index);
    assert_ne!(old.gen, current.gen);
    assert_eq!(s.hazards[0].owner, Some(old));
    let stale_hash = s.hash();
    let mut stale = crate::ai::AiState::default();
    crate::ai::think(&s, current, &mut stale);
    assert_ne!(stale.behaviour, crate::ai::Behaviour::Evade);
    // Local owner comparison uses two handles obtained from actual spawning.
    s.hazards[0].owner = Some(current);
    assert_ne!(stale_hash, s.hash());
    let mut own = crate::ai::AiState::default();
    crate::ai::think(&s, current, &mut own);
    assert_eq!(own.behaviour, crate::ai::Behaviour::Evade);
    let owned = s.hash();
    s.hazards[0].owner = None;
    assert_ne!(owned, s.hash());
}

fn grounded_tick(s: &mut Soldier, pad: &PadState, c: &SoldierConstants) {
    let from = s.pos.y;
    s.step(DT, pad, c, GroundRules::Bf2);
    s.resolve_ground(DT, 0.0, Vec3::Y, from, GroundRules::Bf2);
}

#[test]
fn depleted_history_is_hashed_and_refuses_an_otherwise_valid_jump() {
    let mut s = Soldier::new(Vec3::ZERO);
    let c = SoldierConstants {
        energy_cost_jump: 110.0,
        ..SoldierConstants::default()
    };
    let jump = PadState {
        buttons: buttons::JUMP,
        ..PadState::default()
    };
    grounded_tick(&mut s, &jump, &c);
    assert!(s.energy_depleted);
    for _ in 0..100 {
        grounded_tick(&mut s, &PadState::default(), &c);
        if s.grounded && s.stance == Stance::Stand && s.lock_timer == 0.0 && s.energy > 5.0 {
            break;
        }
    }
    assert!(
        s.energy_depleted
            && s.energy < crate::soldier::SoldierConstants::default().energy_recover_threshold
    );
    assert_eq!(s.stance, Stance::Stand);
    // Equalized local continuation: both latch values occur in normal life;
    // every other field is shared to isolate the latch's control effect.
    let mut available = s.clone();
    available.energy_depleted = Soldier::new(Vec3::ZERO).energy_depleted;
    let depleted_bytes = soldier_bytes(&s);
    let available_bytes = soldier_bytes(&available);
    assert_eq!(
        &depleted_bytes[..available_bytes.len()],
        available_bytes.as_slice()
    );
    assert_ne!(depleted_bytes, available_bytes);
    grounded_tick(&mut s, &jump, &c);
    grounded_tick(&mut available, &jump, &c);
    assert_eq!(s.stance, Stance::Stand);
    assert_eq!(available.stance, Stance::Jump);
}

#[test]
fn prone_hard_lock_survives_floor_loss_and_landing_soft_timer() {
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    let press = PadState {
        buttons: buttons::ROLL_STANCE,
        ..PadState::default()
    };
    for _ in 0..2 {
        for _ in 0..20 {
            grounded_tick(&mut s, &PadState::default(), &c);
        }
        grounded_tick(&mut s, &press, &c);
    }
    assert_eq!(s.stance, Stance::Prone);
    for _ in 0..50 {
        grounded_tick(&mut s, &PadState::default(), &c);
    }
    grounded_tick(&mut s, &press, &c);
    assert_eq!(s.stance, Stance::Stand);
    assert!(s.lock_hard);
    for _ in 0..32 {
        let from = s.pos.y;
        s.step(DT, &PadState::default(), &c, GroundRules::Bf2);
        s.resolve_ground(DT, -100.0, Vec3::Y, from, GroundRules::Bf2);
    }
    assert_eq!(s.stance, Stance::Fall);
    // A lower floor catches the actual falling pose, then the next update
    // enters Stand and installs the soft timer without clearing hardness.
    let landing_y = s.pos.y;
    s.resolve_ground(DT, landing_y, Vec3::Y, landing_y, GroundRules::Bf2);
    s.step(DT, &PadState::default(), &c, GroundRules::Bf2);
    assert_eq!(s.stance, Stance::Stand);
    assert_eq!(s.lock_timer, c.soft_lock);
    assert!(s.lock_hard);
    let mut soft = s.clone();
    soft.lock_hard = Soldier::new(Vec3::ZERO).lock_hard;
    assert_ne!(soldier_bytes(&s), soldier_bytes(&soft));
    let turn = PadState {
        rx: i16::MAX,
        ..PadState::default()
    };
    s.step(DT, &turn, &c, GroundRules::Bf2);
    soft.step(DT, &turn, &c, GroundRules::Bf2);
    assert_ne!(s.yaw, soft.yaw);
}

#[test]
fn sticky_constructor_identity_and_contact_history_are_hashed() {
    let owner = UnitHandle { index: 0, gen: 1 };
    let c = StickyClass::new(true);
    let a = Sticky::new(Vec3::Y, Vec3::NEG_Y, &c, 10.0, owner, 0, 0);
    let mut b = Sticky::new(Vec3::Y, Vec3::NEG_Y, &c, 10.0, owner, 0, 1);
    assert_ne!(sticky_bytes(&a), sticky_bytes(&b), "class index");
    b = Sticky::new(Vec3::Y, Vec3::NEG_Y, &c, 10.0, owner, 1, 0);
    assert_ne!(sticky_bytes(&a), sticky_bytes(&b), "launch team");
    b = Sticky::new(
        Vec3::Y,
        Vec3::NEG_Y,
        &c,
        10.0,
        UnitHandle { gen: 2, ..owner },
        0,
        0,
    );
    assert_ne!(sticky_bytes(&a), sticky_bytes(&b), "owner generation");
    // BF2 fast bounces do not count or arm. A first contact clock remains
    // independent of both flags; the caller records it as tick_sticky does.
    let bounce_class = StickyClass { stick_mask: 0, ..c };
    let mut bounced = Sticky::new(
        Vec3::Y,
        Vec3::new(20.0, -10.0, 0.0),
        &bounce_class,
        10.0,
        owner,
        0,
        0,
    );
    assert_eq!(
        bounced.contact(
            &bounce_class,
            Vec3::ZERO,
            Vec3::Y,
            Surface::Terrain,
            crate::sticky::PUSH_OFF
        ),
        Contact::Bounced
    );
    assert!(!bounced.armed && bounced.bounces == 0);
    let no_clock = sticky_bytes(&bounced);
    bounced.touched = Some(0);
    assert_ne!(
        no_clock,
        sticky_bytes(&bounced),
        "Some(0) differs from absent"
    );
    let first = sticky_bytes(&bounced);
    bounced.touched = Some(1);
    assert_ne!(first, sticky_bytes(&bounced));
}

#[test]
fn contact_generated_anchor_follows_only_the_original_unit_generation() {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    let owner = s
        .spawn(&UnitSpec::trooper(0, Vec3::new(20.0, 0.0, 0.0)))
        .unwrap();
    let target_spec = UnitSpec::trooper(1, Vec3::ZERO);
    let target = s.spawn(&target_spec).unwrap();
    let class = StickyClass {
        gravity: 0.0,
        ..StickyClass::new(true)
    };
    s.weapon_classes[0].ordnance.sticky = Some(class.clone());
    s.stickies.push(Sticky::new(
        Vec3::new(0.0, 1.0, -1.0),
        Vec3::Z * 30.0,
        &class,
        100.0,
        owner,
        0,
        0,
    ));
    s.tick_stickies(DT);
    let g = &s.stickies[0];
    assert!(matches!(g.anchor, Some(Anchor::Unit { handle, .. }) if handle == target));
    assert!(!g.moving);
    let before = g.pos;
    let attached_bytes = sticky_bytes(g);
    let mut world = g.clone();
    world.anchor = Some(Anchor::World);
    assert_ne!(attached_bytes, sticky_bytes(&world));
    let world_bytes = sticky_bytes(&world);
    world.anchor = None;
    assert_eq!(
        world_bytes,
        sticky_bytes(&world),
        "inert anchors are equivalent"
    );
    s.units[target.index as usize].soldier.pos.x += 2.0;
    s.tick_stickies(DT);
    assert_eq!(s.stickies[0].pos, before + Vec3::X * 2.0);
    s.kill_environment(target, DeathCause::Fall);
    let reused = s.spawn(&target_spec).unwrap();
    assert_eq!(reused.index, target.index);
    assert_ne!(reused.gen, target.gen);
    s.units[reused.index as usize].soldier.pos.x += 10.0;
    let stale = s.stickies[0].pos;
    s.tick_stickies(DT);
    assert_eq!(
        s.stickies[0].pos, stale,
        "stale anchor does not follow reused slot"
    );
}

#[test]
fn sticky_class_and_historical_launch_team_change_real_continuation() {
    fn scene(team: u8, class_index: usize) -> (Sim, UnitHandle) {
        let mut s = Sim::empty(7);
        s.heightfield = Some(flat_ground());
        s.friendly_fire_scale = 0.0;
        let owner = s
            .spawn(&UnitSpec::trooper(team, Vec3::new(30.0, 0.0, 0.0)))
            .unwrap();
        let victim = s.spawn(&UnitSpec::trooper(1, Vec3::ZERO)).unwrap();
        let class = StickyClass {
            gravity: 0.0,
            explosion: Some(crate::weapon::ExplosionClass {
                damage: 50.0,
                damage_inner: 3.0,
                damage_outer: 5.0,
                ..crate::weapon::ExplosionClass::default()
            }),
            ..StickyClass::new(true)
        };
        s.weapon_classes[0].ordnance.sticky = Some(class.clone());
        let mut other = s.weapon_classes[0].clone();
        other
            .ordnance
            .sticky
            .as_mut()
            .unwrap()
            .explosion
            .as_mut()
            .unwrap()
            .damage = 100.0;
        s.weapon_classes.push(other);
        let mut g = Sticky::new(
            Vec3::Y,
            Vec3::NEG_Y,
            &class,
            DT * 0.5,
            owner,
            team,
            class_index,
        );
        assert_eq!(
            g.contact(
                &class,
                Vec3::Y,
                Vec3::Y,
                Surface::Terrain,
                crate::sticky::PUSH_OFF
            ),
            Contact::Stuck
        );
        s.stickies.push(g);
        // A live grenade retains launch ownership/team across real slot reuse.
        s.kill_environment(owner, DeathCause::Fall);
        let replacement = s
            .spawn(&UnitSpec::trooper(1 - team, Vec3::new(30.0, 0.0, 0.0)))
            .unwrap();
        assert_eq!(replacement.index, owner.index);
        assert_ne!(replacement.gen, owner.gen);
        assert_eq!(s.stickies[0].owner, owner);
        assert_eq!(s.stickies[0].team, team);
        (s, victim)
    }
    let (mut enemy, victim) = scene(0, 0);
    let (mut friendly, friend_victim) = scene(1, 0);
    let (mut stronger, strong_victim) = scene(0, 1);
    enemy.tick_stickies(DT);
    friendly.tick_stickies(DT);
    stronger.tick_stickies(DT);
    let hp = enemy.unit(victim).unwrap().health.cur;
    assert!(
        hp < friendly.unit(friend_victim).unwrap().health.cur,
        "historical team controls friendly damage"
    );
    assert!(
        stronger.unit(strong_victim).unwrap().health.cur < hp,
        "historical class selects explosion"
    );
    assert_eq!(
        enemy.unit(victim).unwrap().last_hit.unwrap().0.gen,
        1,
        "credit keeps historical owner generation"
    );
}
