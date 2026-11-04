use glam::{Mat3, Mat4, Vec3, Vec3Swizzles};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    pub height: [f32; 2],
    pub distance: f32,
    pub velocity: [[f32; 2]; 3],
    pub acceleration: Vec3,
    pub direction_factor: f32,
    pub lifetime: f32,
    rng: u32,
    id: u32,
    basis: Mat3,
    position: Vec3,
    speed: Vec3,
    age: f32,
    alpha: f32,
    clock: Option<f64>,
}

impl Flight {
    pub fn new(
        height: [f32; 2],
        distance: f32,
        velocity: [[f32; 2]; 3],
        acceleration: Vec3,
        direction_factor: f32,
        lifetime: f32,
        seed: u32,
    ) -> Self {
        Self {
            height,
            distance,
            velocity,
            acceleration,
            direction_factor,
            lifetime,
            rng: seed,
            id: seed,
            basis: Mat3::IDENTITY,
            position: Vec3::ZERO,
            speed: Vec3::ZERO,
            age: 0.0,
            alpha: 0.0,
            clock: None,
        }
    }

    fn random(&mut self) -> f32 {
        crate::rand::unit(&mut self.rng)
    }

    fn sample(&mut self, range: [f32; 2]) -> f32 {
        range[0] + (range[1] - range[0]) * self.random()
    }

    fn spawn(&mut self) {
        let radial = loop {
            let p = Vec3::new(self.random() - 0.5, 0.0, self.random() - 0.5);
            if p.length_squared() > 0.0 {
                break p.normalize();
            }
        };
        let height = self.sample(self.height);
        self.position = radial * self.distance + Vec3::Y * height;
        let mut forward = Vec3::Y.cross(radial);
        if self.random() > 0.5 {
            forward = -forward;
        }
        if self.direction_factor > 0.0 {
            forward -= radial * (self.direction_factor * 0.5 * (1.0 + self.random()));
        }
        forward = forward.normalize();
        let right = Vec3::Y.cross(forward).normalize();
        self.basis = Mat3::from_cols(right, forward.cross(right), forward);
        self.speed = Vec3::new(
            self.sample(self.velocity[0]),
            self.sample(self.velocity[1]),
            self.sample(self.velocity[2]),
        );
        self.age = self.lifetime * (0.8 + 0.2 * self.random());
        self.alpha = 0.0;
    }

    pub(super) fn id(&self) -> u32 {
        self.id
    }

    pub(super) fn update_shared(&mut self, clock: f64, seed: &mut u32) {
        let private = self.rng;
        self.rng = *seed;
        self.update(clock);
        *seed = self.rng;
        self.rng = private;
    }

    pub fn update(&mut self, clock: f64) {
        let Some(previous) = self.clock.replace(clock) else {
            self.spawn();
            self.alpha = 1.0;
            return;
        };
        let dt = (clock - previous).max(0.0) as f32;
        if dt == 0.0 {
            return;
        }
        if self.alpha >= 0.0
            && (self.position.xz().length_squared() > (self.distance + 10.0).powi(2)
                || self.age > self.lifetime)
        {
            self.alpha = -1.0;
        }
        let old_alpha = self.alpha;
        self.alpha = (self.alpha + dt * 0.25).min(1.0);
        self.age += dt;
        if old_alpha < 0.0 && self.alpha >= 0.0 {
            self.spawn();
            return;
        }
        self.speed += self.acceleration * dt;
        self.position += self.basis * self.speed * dt;
    }

    pub fn model(&self) -> Mat4 {
        Mat4::from_cols(
            self.basis.x_axis.extend(0.0),
            self.basis.y_axis.extend(0.0),
            self.basis.z_axis.extend(0.0),
            self.position.extend(1.0),
        )
    }

    pub fn opacity(&self) -> f32 {
        self.alpha.abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_speed_moves_the_ship_without_moving_its_basis() {
        let mut f = Flight::new(
            [100.0; 2],
            600.0,
            [[0.0; 2], [0.0; 2], [100.0; 2]],
            Vec3::ZERO,
            0.5,
            1000.0,
            5,
        );
        f.update(0.0);
        let before = f.model();
        f.update(0.5);
        assert!((f.position.distance(before.w_axis.truncate()) - 50.0).abs() < 0.001);
        assert_eq!(f.basis, Mat3::from_mat4(before));
        assert_eq!(f.opacity(), 1.0);
        let state = f;
        f.update(0.5);
        assert_eq!(f.position, state.position);
    }

    #[test]
    fn exit_fades_then_restarts_about_the_world_origin() {
        let mut f = Flight::new([100.0; 2], 300.0, [[0.0; 2]; 3], Vec3::ZERO, 0.5, 1000.0, 7);
        f.update(0.0);
        assert_eq!(f.opacity(), 1.0);
        f.position.x += 5000.0;
        f.update(1.0);
        assert!(f.alpha < 0.0);
        f.update(4.0);
        assert_eq!(f.alpha, 0.0);
        assert!((f.position.xz().length() - 300.0).abs() < 0.001);
        assert_eq!(f.position.y, 100.0);
    }
}
