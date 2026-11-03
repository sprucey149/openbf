use super::*;
use crate::weapon::{PropSource, DEG};
use crate::TICK_DT;

const ELF_TABLE: [u8; 112] = [
    0, 0, 5, 9, 1, 2, 3, 11, 0, 0, 5, 9, 0, 0, 3, 11, 0, 0, 5, 10, 0, 0, 3, 11, 4, 4, 4, 4, 4, 4,
    4, 11, 4, 4, 4, 4, 4, 4, 4, 11, 9, 0, 7, 10, 9, 9, 3, 11, 10, 0, 8, 9, 10, 10, 3, 11, 10, 5, 8,
    8, 10, 10, 3, 11, 9, 7, 7, 7, 9, 9, 3, 11, 0, 5, 5, 8, 1, 2, 3, 11, 0, 7, 5, 7, 1, 2, 3, 11,
    12, 12, 12, 12, 12, 12, 4, 12, 13, 13, 12, 12, 12, 12, 4, 12, 0, 0, 0, 0, 0, 0, 4, 0,
];

#[derive(Clone)]
struct Rig {
    c: DroidekaClass,
    d: Droideka,
    b: Body,
    floor: f32,
}

impl Rig {
    fn new(order: Order) -> Self {
        let c = DroidekaClass::default();
        let d = Droideka::new(&c, order);
        Self {
            c,
            d,
            b: Body {
                pos: Vec3::ZERO,
                vel: Vec3::ZERO,
                yaw: 0.0,
                grounded: true,
            },
            floor: 0.0,
        }
    }

    fn tick(&mut self, input: &Input) -> Report {
        let r = self.d.step(TICK_DT, input, &self.c, &mut self.b);
        let grounded = self.b.pos.y <= self.floor;
        if grounded {
            self.b.pos.y = self.floor;
        }
        self.d.land(&mut self.b, grounded, TICK_DT);
        r
    }

    fn ticks_until(&mut self, input: &Input, max: u32, done: impl Fn(&Droideka) -> bool) -> u32 {
        for n in 1..=max {
            self.tick(input);
            if done(&self.d) {
                return n;
            }
        }
        panic!("not reached in {max} ticks: state {}", self.d.state);
    }

    fn rolling() -> Self {
        let mut r = Self::new(Order::Roll);
        r.ticks_until(&Input::default(), 60, |d| d.state == ROLLING);
        r
    }
}

fn human() -> Input {
    Input {
        human: true,
        ..Input::default()
    }
}

#[test]
fn the_transition_table_is_the_elfs_112_words() {
    let flat: Vec<u8> = TRANSITIONS.iter().flatten().copied().collect();
    assert_eq!(flat, ELF_TABLE);
}

#[test]
fn fold_takes_half_a_second_and_unfold_one_point_six_seven() {
    let c = DroidekaClass::default();
    assert!((c.state_time(IDLE_TO_ROLL) - 0.50).abs() < 1e-4);
    assert!((c.state_time(ROLL_TO_IDLE) - 1.66).abs() < 1e-3);
    let mut r = Rig::new(Order::Roll);
    let fold = r.ticks_until(&Input::default(), 60, |d| d.state == ROLLING);
    assert!(
        (fold as f32 * TICK_DT - 0.50).abs() <= TICK_DT + 1e-4,
        "{fold}"
    );
    r.d.order = Order::Deploy;
    r.tick(&Input::default());
    assert_eq!(r.d.state, ROLL_TO_IDLE);
    let unfold = r.ticks_until(&Input::default(), 120, |d| d.state == IDLE) + 1;
    assert!(
        (unfold as f32 * TICK_DT - 1.67).abs() <= TICK_DT + 1e-3,
        "{unfold}"
    );
}

#[test]
fn a_bot_rolls_at_spawn_and_a_player_stays_deployed_until_crouch() {
    let mut bot = Rig::new(Order::Roll);
    bot.tick(&Input::default());
    assert_eq!(bot.d.state, IDLE_TO_ROLL);
    let mut player = Rig::new(Order::Deploy);
    for _ in 0..10 {
        player.tick(&human());
    }
    assert_eq!(player.d.state, IDLE);
    let fold = Input {
        fold: true,
        ..human()
    };
    player.tick(&fold);
    assert_eq!(player.d.state, IDLE_TO_ROLL);
    player.ticks_until(&human(), 60, |d| d.state == ROLLING);
    player.tick(&fold);
    assert_eq!(player.d.state, ROLL_TO_IDLE);
}

#[test]
fn unrolling_waits_for_the_floor_and_headroom() {
    let mut r = Rig::rolling();
    r.d.order = Order::Deploy;
    r.b.pos.y = 3.0;
    r.b.grounded = false;
    r.d.step(TICK_DT, &Input::default(), &r.c, &mut r.b);
    assert_eq!(r.d.state, ROLLING);
    r.b.grounded = true;
    let cramped = Input {
        cramped: true,
        ..Input::default()
    };
    r.d.step(TICK_DT, &cramped, &r.c, &mut r.b);
    assert_eq!(r.d.state, ROLLING);
    r.d.step(TICK_DT, &Input::default(), &r.c, &mut r.b);
    assert_eq!(r.d.state, ROLL_TO_IDLE);
}

#[test]
fn the_ball_tops_out_at_twelve_metres_a_second() {
    let mut r = Rig::rolling();
    let full = Input {
        fwd: 1.0,
        ..Input::default()
    };
    for _ in 0..90 {
        r.tick(&full);
    }
    let x0 = r.b.pos;
    for _ in 0..30 {
        r.tick(&full);
    }
    let speed = (r.b.pos - x0).length();
    assert!((speed - 12.0).abs() < 12.0 * 0.02, "{speed}");
    assert!((r.d.omega - r.c.ball_omega_cap()).abs() < 1e-3);
    assert!(r.b.pos.z < x0.z, "rolls along its facing, -Z at yaw 0");
}

#[test]
fn the_ball_falls_five_metres_in_half_a_second() {
    let mut r = Rig::rolling();
    r.b.pos.y = 5.0;
    r.b.grounded = false;
    let mut ticks = 0;
    while r.b.pos.y > 0.0 {
        r.tick(&Input::default());
        ticks += 1;
        assert!(ticks < 60);
    }
    let t = ticks as f32 * TICK_DT;
    assert!((t - 0.505).abs() <= TICK_DT + 1e-3, "{t}");
}

#[test]
fn the_ball_turns_slower_at_full_stick_and_leans_into_the_turn() {
    let mut r = Rig::rolling();
    let still = Input {
        turn: 1.0,
        ..human()
    };
    r.tick(&still);
    let zero_stick = -r.b.yaw;
    assert!((zero_stick - 2.5 * TICK_DT).abs() < 1e-5);
    let mut r = Rig::rolling();
    let full = Input {
        turn: 1.0,
        fwd: 1.0,
        ..human()
    };
    r.tick(&full);
    assert!((-r.b.yaw - 2.0 * TICK_DT).abs() < 1e-5);
    assert!(r.d.lean > 0.0);
}

#[test]
fn a_bot_ball_turns_at_the_walk_max_turn_speed() {
    for fwd in [0.0, 1.0] {
        let mut r = Rig::rolling();
        r.tick(&Input {
            turn: 1.0,
            fwd,
            ..Input::default()
        });
        assert!(
            (-r.b.yaw - 1.25 * TICK_DT).abs() < 1e-5,
            "{fwd}: {}",
            r.b.yaw
        );
    }
}

#[test]
fn a_human_turning_the_ball_leans_as_if_rolling_forward() {
    let mut r = Rig::rolling();
    r.tick(&Input {
        turn: 1.0,
        ..human()
    });
    assert!(r.d.lean > 0.0, "{}", r.d.lean);
    assert_eq!(
        r.d.omega, 0.0,
        "the fifth of forward stays under the friction"
    );
    let mut bot = Rig::rolling();
    bot.tick(&Input {
        turn: 1.0,
        ..Input::default()
    });
    assert!(bot.d.lean < 0.0, "{}", bot.d.lean);
}

#[test]
fn boost_doubles_the_top_speed_and_spends_the_shared_bar() {
    let mut r = Rig::rolling();
    let boost = Input {
        fwd: 1.0,
        boost: true,
        ..human()
    };
    for _ in 0..30 {
        r.tick(&boost);
    }
    assert!(r.d.boosting);
    assert!(r.d.omega > r.c.ball_omega_cap());
    let want = r.c.shield.max - r.c.ball_sprint_drain() * 30.0 * TICK_DT;
    assert!(
        (r.d.shield.points - want).abs() < 40.0,
        "{}",
        r.d.shield.points
    );
    for _ in 0..90 {
        r.tick(&boost);
    }
    assert!(!r.d.boosting && r.d.sprint_spent);
    assert!(r.d.shield.points < r.c.ball_sprint_recover());
    let walk = Input {
        fwd: 0.5,
        boost: true,
        ..human()
    };
    let mut r = Rig::rolling();
    r.tick(&walk);
    assert!(!r.d.boosting, "boost needs the stick past 0.9");
}

#[test]
fn the_ball_jump_reaches_its_height_once_per_landing() {
    let mut r = Rig::rolling();
    let jump = Input {
        jump: true,
        ..human()
    };
    let rep = r.tick(&jump);
    assert!(rep.jumped);
    assert_eq!(
        r.d.shield.points,
        r.c.shield.max - r.c.ball_jump_cost(false)
    );
    let mut apex: f32 = 0.0;
    let again = r.tick(&jump);
    assert!(!again.jumped, "no second jump in the air");
    for _ in 0..60 {
        apex = apex.max(r.b.pos.y);
        r.tick(&human());
    }
    assert!((apex - 1.354).abs() < 0.03, "{apex}");
    let mut walker = Rig::new(Order::Deploy);
    assert!(!walker.tick(&jump).jumped, "no jump deployed");
}

#[test]
fn the_walk_loop_moves_at_max_speed_and_strafes() {
    let mut r = Rig::new(Order::Deploy);
    let fwd = Input {
        fwd: 1.0,
        ..human()
    };
    for _ in 0..30 {
        r.tick(&fwd);
    }
    assert!(
        matches!(r.d.state, LEFT_TO_RIGHT | RIGHT_TO_LEFT),
        "{}",
        r.d.state
    );
    let z0 = r.b.pos.z;
    for _ in 0..60 {
        r.tick(&fwd);
    }
    let speed = (z0 - r.b.pos.z) / 2.0;
    assert!((speed - 2.5).abs() < 2.5 * 0.03, "{speed}");
    let mut s = Rig::new(Order::Deploy);
    let right = Input {
        strafe: 1.0,
        ..human()
    };
    for _ in 0..120 {
        s.tick(&right);
    }
    assert!(s.b.pos.x > 1.0, "{:?}", s.b.pos);
    assert!(s.b.yaw.abs() < 1e-6, "the body keeps its aim");
}

#[test]
fn walking_backwards_plays_the_clips_in_reverse() {
    let mut r = Rig::new(Order::Deploy);
    let back = Input {
        fwd: -1.0,
        ..human()
    };
    r.tick(&back);
    assert_eq!(r.d.state, LEFT_TO_IDLE);
    assert_eq!(r.d.dir, DIR_BACK);
    assert!(r.d.clip_t < r.c.duration[usize::from(LEFT_TO_IDLE)]);
    for _ in 0..60 {
        r.tick(&back);
    }
    assert!(r.b.pos.z > 1.0, "{:?}", r.b.pos);
}

#[test]
fn the_legs_catch_up_with_turn_clips_past_the_threshold() {
    let mut r = Rig::new(Order::Deploy);
    let turn = Input {
        turn: 1.0,
        ..human()
    };
    let mut seen_turn = false;
    for _ in 0..30 {
        r.tick(&turn);
        seen_turn |= r.d.state == TURN_RIGHT;
        assert!(r.d.legs.abs() <= r.c.turn_threshold + 0.2, "{}", r.d.legs);
    }
    assert!(seen_turn);
    let t = r.c.state_time(TURN_LEFT);
    assert!((t - 0.137).abs() < 0.005, "{t}");
}

#[test]
fn no_shot_leaves_while_folding_rolling_or_unfolding() {
    let mut r = Rig::new(Order::Roll);
    assert!(!r.d.no_fire());
    let rep = r.tick(&Input::default());
    assert!(rep.weapons_off);
    assert!(r.d.no_fire());
    r.ticks_until(&Input::default(), 60, |d| d.state == ROLLING);
    assert!(r.d.no_fire());
    r.d.order = Order::Deploy;
    r.tick(&Input::default());
    assert!(r.d.no_fire());
    let mut on = false;
    for _ in 0..80 {
        on |= r.tick(&Input::default()).weapons_on;
    }
    assert!(on && !r.d.no_fire());
}

#[test]
fn deep_water_deals_ten_every_update_after_a_second() {
    let c = DroidekaClass::default();
    let mut d = Droideka::new(&c, Order::Deploy);
    let mut dealt = Vec::new();
    for _ in 0..40 {
        dealt.push(d.water_tick(TICK_DT, Some(1.6), &c));
    }
    let first = dealt.iter().position(|&x| x > 0.0).unwrap();
    assert!(
        (first as f32 * TICK_DT - 1.0).abs() <= TICK_DT + 1e-4,
        "{first}"
    );
    assert!(dealt[first..].iter().all(|&x| x == 10.0));
    assert_eq!(d.water_tick(TICK_DT, Some(1.4), &c), 0.0);
    assert_eq!(d.water, c.water_interval);
}

#[test]
fn the_shield_toggles_drains_refills_and_drops_in_the_ball() {
    let c = ShieldClass::default();
    let mut s = Shield::full(&c);
    assert!(!s.on);
    assert_eq!(s.tick(TICK_DT, true, false, true, &c), Some(true));
    assert_eq!(
        s.points, 3000.0,
        "a full BF2 shield does not drain until hit"
    );
    s.absorb(80.0, 1.0);
    for _ in 0..30 {
        s.tick(TICK_DT, false, false, true, &c);
    }
    assert!((s.points - (2920.0 - 100.0)).abs() < 0.05, "{}", s.points);
    assert_eq!(s.tick(TICK_DT, true, false, true, &c), Some(false));
    for _ in 0..30 {
        s.tick(TICK_DT, false, false, true, &c);
    }
    assert!((s.points - 3000.0).abs() < 0.05 || s.points > 2810.0);
    s.tick(TICK_DT, true, false, true, &c);
    assert!(s.on);
    assert_eq!(s.tick(TICK_DT, false, true, true, &c), Some(false));
    assert_eq!(
        s.tick(TICK_DT, true, true, true, &c),
        None,
        "no raise while transforming"
    );
    s.on = true;
    s.points = 0.0;
    assert_eq!(s.tick(TICK_DT, false, false, true, &c), Some(false));
    let bf1 = ShieldClass {
        drains_full: true,
        ..c.clone()
    };
    let mut full = Shield::full(&bf1);
    full.tick(TICK_DT, true, false, true, &bf1);
    assert!(full.points < 3000.0);
}

#[test]
fn the_shield_takes_damage_first_and_passes_on_what_it_cannot_hold() {
    let mut s = Shield {
        points: 50.0,
        on: true,
    };
    assert_eq!(s.absorb(80.0, 1.0), 30.0);
    assert_eq!(s.points, 0.0);
    let mut scaled = Shield {
        points: 100.0,
        on: true,
    };
    assert_eq!(scaled.absorb(40.0, 2.0), 0.0);
    assert_eq!(scaled.points, 20.0);
    assert_eq!(scaled.absorb(40.0, 2.0), 30.0);
    let mut down = Shield {
        points: 100.0,
        on: false,
    };
    assert_eq!(down.absorb(40.0, 1.0), 40.0);
    assert_eq!(down.points, 100.0);
}

#[test]
fn a_shot_from_inside_the_sphere_passes_it() {
    let sphere = Sphere {
        center: Vec3::new(0.0, 1.0, 0.0),
        radius: 1.25,
    };
    let t = ray_sphere(Vec3::new(10.0, 1.0, 0.0), Vec3::NEG_X, 20.0, sphere).unwrap();
    assert!((t - 8.75).abs() < 1e-4);
    assert!(ray_sphere(Vec3::new(0.5, 1.0, 0.0), Vec3::NEG_X, 20.0, sphere).is_none());
    assert!(ray_sphere(Vec3::new(10.0, 3.0, 0.0), Vec3::NEG_X, 20.0, sphere).is_none());
}

#[test]
fn any_event_while_rolling_stops_the_ball() {
    let mut r = Rig::rolling();
    let full = Input {
        fwd: 1.0,
        ..Input::default()
    };
    for _ in 0..30 {
        r.tick(&full);
    }
    assert!(r.b.vel.length() > 1.0);
    r.d.alive = false;
    r.d.step(TICK_DT, &full, &r.c, &mut r.b);
    assert_eq!(r.d.state, DEAD);
    assert_eq!(r.b.vel, Vec3::ZERO);
}

#[test]
fn death_on_foot_passes_through_dying() {
    let mut r = Rig::new(Order::Deploy);
    r.d.alive = false;
    r.tick(&Input::default());
    assert_eq!(r.d.state, DYING);
    r.tick(&Input::default());
    assert_eq!(r.d.state, DEAD);
    assert!(!r.d.shield.on);
}

#[test]
fn the_same_input_gives_the_same_hash() {
    let run = || {
        let mut r = Rig::new(Order::Roll);
        let mut bytes = Vec::new();
        for i in 0..200 {
            let input = Input {
                fwd: ((i % 37) as f32 / 18.0 - 1.0).clamp(-1.0, 1.0),
                turn: ((i % 23) as f32 / 11.0 - 1.0).clamp(-1.0, 1.0),
                jump: i % 50 == 0,
                shield: i % 70 == 0,
                ..human()
            };
            if i == 120 {
                r.d.order = Order::Deploy;
            }
            r.tick(&input);
        }
        r.d.hash_into(&mut |b| bytes.extend_from_slice(b));
        bytes
    };
    assert_eq!(run(), run());
}

#[test]
fn class_props_read_the_disc_values() {
    struct Props(Vec<(&'static str, &'static str)>, Vec<(u32, &'static str)>);
    impl PropSource for Props {
        fn prop(&self, key: &str) -> Option<&str> {
            self.0
                .iter()
                .find(|p| p.0.eq_ignore_ascii_case(key))
                .map(|p| p.1)
        }
        fn prop_hash(&self, h: u32) -> Option<&str> {
            self.1.iter().find(|p| p.0 == h).map(|p| p.1)
        }
    }
    let class = Props(
        vec![
            ("MaxSpeed", "2.5"),
            ("TurnThreshold", "30.0"),
            ("BallMoveSpeed", "6.0"),
            ("BallJumpHeight", "1.27"),
            ("RollTime", "0.50"),
            ("UnrollTime", "1.66"),
            ("PitchLimits", "-25.0 25.0"),
            ("AimerPitchLimits", "-12.0 18.0"),
            ("AimerYawLimits", "-10.0 30.0"),
        ],
        vec![(0xdc52_4dd9, "1.0")],
    );
    let shield = Props(
        vec![
            ("MaxShield", "3000"),
            ("AddShield", "-100"),
            ("AddShieldOff", "300"),
            ("ShieldOffset", "0.0 1.0 0.125"),
            ("ShieldRadius", "1.25"),
            ("ShieldSound", "\"droid_fx7_hum\""),
        ],
        Vec::new(),
    );
    let c = DroidekaClass::from_props(&class, Some(&shield));
    assert!((c.turn_threshold - 30.0 * DEG).abs() < 1e-6);
    assert!((c.jump_vy - 4.989).abs() < 1e-3);
    assert_eq!(c.shield.max, 3000.0);
    assert_eq!(c.shield.sphere.center, Vec3::new(0.0, 1.0, 0.125));
    assert!((c.ball_omega_cap() - 6.0 / 0.65).abs() < 1e-4);
    assert!((c.pitch_limits.1 - 25.0 * DEG).abs() < 1e-6);
    assert_eq!(c.aimer_pitch_limits, (-12.0 * DEG, 18.0 * DEG));
    assert_eq!(c.aimer_yaw_limits, (-10.0 * DEG, 30.0 * DEG));
    assert_eq!(c.shield.sound.as_deref(), Some("droid_fx7_hum"));
    assert_eq!(c.shield.off_sound, None);
}

#[test]
fn the_camera_takes_the_class_sections_by_form_and_falls_back_without_them() {
    let s = |eye_y: f32, y: f32, deg: f32| crate::unit::CameraSection {
        eye_y,
        offset: Vec3::new(0.0, y, 4.5),
        tilt: deg.to_radians(),
    };
    let c = DroidekaClass {
        cameras: vec![
            (CAMERA_STAND, s(1.8, 0.5, 5.0)),
            (CAMERA_STAND_ZOOM, s(1.8, 0.5, 1.0)),
            (CAMERA_CROUCH, s(1.0, 0.3, 5.0)),
        ],
        ..DroidekaClass::default()
    };
    assert_eq!(c.camera_section(Form::Walk, false), Some(s(1.8, 0.5, 5.0)));
    assert_eq!(c.camera_section(Form::Walk, true), Some(s(1.8, 0.5, 1.0)));
    assert_eq!(c.camera_section(Form::Ball, false), Some(s(1.0, 0.3, 5.0)));
    assert_eq!(
        DroidekaClass::default().camera_section(Form::Walk, false),
        None
    );
}

#[test]
fn the_engine_runs_while_folding_and_rolling_only() {
    let c = DroidekaClass::default();
    let mut d = Droideka::new(&c, Order::Roll);
    for (state, on) in [
        (IDLE, false),
        (LEFT_TO_RIGHT, false),
        (IDLE_TO_ROLL, true),
        (ROLLING, true),
        (ROLL_TO_IDLE, false),
    ] {
        d.state = state;
        assert_eq!(d.engine_on(), on, "state {state}");
    }
}

#[test]
fn refused_ball_jump_explains_the_spent_bar_but_not_airtime() {
    let mut r = Rig::rolling();
    r.d.shield.points = 0.0;
    r.d.sprint_spent = true;
    let jump = Input {
        jump: true,
        ..human()
    };
    let report = r.tick(&jump);
    assert!(!report.jumped);
    assert_eq!(report.cues().collect::<Vec<_>>(), vec![Cue::EnergyRefused]);
    r.d.air = BALL_JUMP_WINDOW;
    let report = r.tick(&jump);
    assert!(!report.energy_refused, "airtime is not an energy refusal");
    r.d.air = 0.0;
    r.d.sprint_spent = false;
    r.d.shield.points = 1.0;
    let report = r.tick(&jump);
    assert!(
        report.jumped && !report.energy_refused,
        "the soldier's payment lets the last jump empty the bar"
    );
    assert!(r.d.shield.points < 0.0);
    assert!(r.d.sprint_spent, "an overdrawn bar is spent");
}

/// Roll until the ball is down again after a jump.
fn land(r: &mut Rig) {
    r.ticks_until(&human(), 120, |d| d.air == 0.0);
    r.tick(&human());
}

#[test]
fn a_ball_jump_costs_the_class_share_of_the_shared_bar() {
    let mut r = Rig::rolling();
    // BF2's droideka: EnergyCostJump 30 of EnergyBar 100, of a 3000 shield.
    assert_eq!(r.c.ball_jump_cost(false), 900.0);
    assert_eq!(r.c.ball_jump_cost(true), 900.0, "no EnergyCostJumpSprint");
    let jump = Input {
        jump: true,
        ..human()
    };
    let before = r.d.shield.points;
    assert!(r.tick(&jump).jumped);
    assert_eq!(r.d.shield.points, before - 900.0);
    land(&mut r);
    // A dearer sprinting cost, where a class authors one, is the one paid.
    r.c.energy_cost_jump_sprint = 50.0;
    let full = r.c.shield.max;
    r.d.shield.points = full;
    let sprint = Input {
        fwd: 1.0,
        boost: true,
        ..human()
    };
    r.tick(&sprint);
    assert!(r.d.boosting);
    let at = r.d.shield.points;
    assert!(
        r.tick(&Input {
            jump: true,
            ..sprint
        })
        .jumped
    );
    assert_eq!(r.d.shield.points, at - 1500.0);
    // The stamina cheat: free.
    land(&mut r);
    let at = r.d.shield.points;
    let free = Input {
        jump: true,
        free_sprint: true,
        ..human()
    };
    assert!(r.tick(&free).jumped);
    assert!(r.d.shield.points >= at, "{} {at}", r.d.shield.points);
}

#[test]
fn ball_jumps_are_refused_only_on_a_spent_bar_and_resume_at_the_recover_share() {
    let mut r = Rig::rolling();
    let jump = Input {
        jump: true,
        ..human()
    };
    let mut jumps = 0;
    let refused = loop {
        let report = r.tick(&jump);
        if report.energy_refused {
            break report;
        }
        assert!(report.jumped, "a bar that is not spent always jumps");
        jumps += 1;
        assert!(jumps < 10, "the bar never ran out");
        land(&mut r);
    };
    assert!(jumps >= 3, "{jumps} jumps from a full bar");
    assert!(!refused.jumped);
    // The refusal follows the visible bar: spent and under the recover share.
    assert!(r.d.sprint_spent);
    let (bar, recover) = (r.d.shield.points, r.c.ball_sprint_recover());
    assert!(bar < recover, "{bar} {recover}");
    // Still refused while it refills, until the recover share.
    while r.d.shield.points < recover - 1.0 {
        assert!(r.tick(&jump).energy_refused, "{}", r.d.shield.points);
    }
    r.ticks_until(&human(), 30, |d| !d.sprint_spent);
    assert!(r.tick(&jump).jumped);
}

#[test]
fn ball_sprint_and_ball_jump_share_the_one_bar() {
    // A sprint that empties the bar refuses the jump.
    let mut r = Rig::rolling();
    let sprint = Input {
        fwd: 1.0,
        boost: true,
        ..human()
    };
    r.ticks_until(&sprint, 200, |d| d.sprint_spent);
    let report = r.tick(&Input {
        jump: true,
        ..sprint
    });
    assert!(!report.jumped && report.energy_refused);
    // A jump that empties the bar refuses the sprint.
    let mut r = Rig::rolling();
    r.d.shield.points = 100.0;
    assert!(
        r.tick(&Input {
            jump: true,
            ..human()
        })
        .jumped
    );
    assert!(r.d.sprint_spent && r.d.shield.points < r.c.ball_sprint_recover());
    land(&mut r);
    r.tick(&sprint);
    assert!(!r.d.boosting, "no sprint on a bar a jump spent");
    // And a sprint spends what a jump left.
    let mut r = Rig::rolling();
    assert!(
        r.tick(&Input {
            jump: true,
            ..human()
        })
        .jumped
    );
    land(&mut r);
    let left = r.d.shield.points;
    assert!(left < r.c.shield.max);
    r.tick(&sprint);
    assert!(r.d.boosting && r.d.shield.points < left);
}

#[test]
fn the_jump_button_leaves_a_standing_droidekas_shield_alone() {
    let mut r = Rig::new(Order::Deploy);
    let jump = Input {
        jump: true,
        ..human()
    };
    let full = r.c.shield.max;
    for _ in 0..10 {
        let report = r.tick(&jump);
        assert!(!report.jumped && !report.energy_refused);
    }
    assert_eq!(r.d.shield.points, full);
    // Raised, it drains at its own rate only (a full one holds); jumping
    // changes nothing.
    r.d.shield.points = full - 500.0;
    r.tick(&Input {
        shield: true,
        ..human()
    });
    assert!(r.d.shield.on);
    let mut plain = r.clone();
    for _ in 0..10 {
        r.tick(&jump);
        plain.tick(&human());
    }
    assert!(r.d.shield.on);
    assert_eq!(r.d.shield.points, plain.d.shield.points);
    assert!(r.d.shield.points < full - 500.0);
    assert!(!r.d.sprint_spent);
}

/// The approved shared bar applies the soldier's PS2 debt/threshold rule
/// to the ball, and keeps that same balance when the shield stands up.
#[test]
fn ps2_stamina_shared_bar_keeps_jump_debt_until_shield_recharge_repays_it() {
    let mut r = Rig::rolling();
    r.d.shield.points = 1.0;
    r.c.shield.rate_off = 0.0;
    assert!(
        r.tick(&Input {
            jump: true,
            ..human()
        })
        .jumped
    );
    assert_eq!(r.d.shield.points, -899.0);
    assert!(r.d.sprint_spent);
    r.d.state = IDLE;
    r.d.order = Order::Deploy;
    r.c.shield.rate_off = 300.0;
    r.tick(&Input {
        shield: true,
        ..human()
    });
    assert!(!r.d.shield.on, "a bar in debt cannot raise the shield");
    assert_eq!(r.d.shield.points, -889.0);
    r.d.shield.points = 589.0;
    r.tick(&human());
    assert!(r.d.sprint_spent);
    r.tick(&human());
    assert!(!r.d.sprint_spent, "20 percent recovery share is 600 points");
    assert!(r.d.shield.points < 750.0);
    r.tick(&Input {
        shield: true,
        ..human()
    });
    assert!(r.d.shield.on);
    let before = r.d.shield.points;
    r.tick(&human());
    assert!(
        r.d.shield.points < before,
        "standing shield spends the same bar"
    );
}

#[test]
fn ps2_stamina_ball_sprint_debt_is_bounded_by_the_shared_class_floor() {
    let mut r = Rig::rolling();
    r.d.shield.points = 1.0;
    r.d.boosting = true;
    r.d.tick_sprint(10.0, &human(), &r.c);
    assert_eq!(r.d.shield.points, -900.0);
    assert!(r.d.sprint_spent && !r.d.boosting);
}

#[test]
fn shield_debt_has_empty_bar_cues_and_cannot_absorb_damage() {
    for points in [0.0, -900.0] {
        let mut r = Rig::rolling();
        r.c.shield.rate_off = 0.0;
        r.d.shield.points = points;
        r.d.shield.on = true;
        let report = r.tick(&Input {
            jump: true,
            shield: true,
            ..human()
        });
        assert!(!r.d.shield.on);
        assert_eq!(
            report.cues().collect::<Vec<_>>(),
            vec![Cue::ShieldOff, Cue::EnergyRefused]
        );
        let report = r.tick(&Input {
            jump: true,
            ..human()
        });
        assert_eq!(
            report.cues().collect::<Vec<_>>(),
            vec![Cue::EnergyRefused],
            "no repeated shield-off sound"
        );
        assert_eq!(r.d.shield.absorb(25.0, 1.0), 25.0);
        assert_eq!(
            r.d.shield.points, points,
            "damage must not erase or increase debt"
        );
    }
}
