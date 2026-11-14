use super::{force, Camera, Lcg, Library, Vec3};
use core::fmath::FloatExt;
use manifest::{Manifest, ManifestLightning};
use render::{srgb_decode, srgb_encode, Lighting};

pub(super) struct Storm {
    config: ManifestLightning,
    rng: Lcg,
    time: f64,
    next_flash: f64,
    next_bolt: f64,
    flashes_left: u32,
    flash_started: f64,
    peak: f32,
    pub(super) bolts: force::State,
}

impl Storm {
    pub(super) fn load(map: &Manifest) -> Option<Self> {
        if map.source_game != "bf2" || map.map != "kamino1" {
            return None;
        }
        map.lightning.clone().map(Self::new)
    }

    fn new(config: ManifestLightning) -> Self {
        let mut rng = Lcg(0x4b41_4d31);
        let next_bolt = Self::interval(&mut rng, [4.0, 7.0]);
        Self {
            next_flash: f64::from(config.time_between_flashes[1]),
            config,
            rng,
            time: 0.0,
            next_bolt,
            flashes_left: 0,
            flash_started: f64::NEG_INFINITY,
            peak: 0.0,
            bolts: force::State::default(),
        }
    }

    fn interval(rng: &mut Lcg, [min, max]: [f32; 2]) -> f64 {
        f64::from(min + (max - min) * rng.float()).fmax(0.001)
    }

    pub(super) fn advance(&mut self, time: f64, camera: &Camera, library: Option<&Library>) {
        if time < self.time {
            *self = Self::new(self.config.clone());
        }
        loop {
            let event = self.next_flash.fmin(self.next_bolt);
            if event > time {
                break;
            }
            self.advance_bolts(event);
            let mut group = false;
            if event == self.next_flash {
                group = self.flashes_left == 0;
                if group {
                    let [min, max] = self.config.num_sub_flashes;
                    self.flashes_left = (min + (max - min + 0.9999) * self.rng.float()) as u32;
                }
                self.flashes_left = self.flashes_left.saturating_sub(1);
                self.flash_started = event;
                self.peak = self.config.brightness_min
                    + (1.0 - self.config.brightness_min) * self.rng.float();
                let gap = if self.flashes_left == 0 {
                    self.config.time_between_flashes
                } else {
                    self.config.time_between_sub_flashes
                };
                self.next_flash = event + Self::interval(&mut self.rng, gap);
            }
            if group || event == self.next_bolt {
                self.spawn_bolt(camera, library, group);
                self.next_bolt = event + Self::interval(&mut self.rng, [4.0, 7.0]);
            }
        }
        self.advance_bolts(time);
    }

    fn advance_bolts(&mut self, time: f64) {
        if time > self.time {
            self.bolts.update((time - self.time) as f32);
            self.time = time;
        }
    }

    fn spawn_bolt(&mut self, camera: &Camera, library: Option<&Library>, near: bool) {
        let look = camera.forward();
        let direction = Vec3::new(
            look.x + self.rng.float() - 0.5,
            0.01,
            look.z + self.rng.float() - 0.5,
        )
        .normalize();
        let (distance, height) = if near { (200.0, 200.0) } else { (800.0, 300.0) };
        let top = (camera.position + direction * distance).with_y(height);
        let foot = top
            + Vec3::new(
                (self.rng.float() - 0.5) * 20.0,
                -200.0,
                (self.rng.float() - 0.5) * 20.0,
            );
        if let Some(lib) = library {
            self.bolts.bolt(lib, "skybolt", top, foot, None, 1.0);
        }
    }

    pub(super) fn flash(&self, mut light: Lighting) -> Option<(Lighting, f32)> {
        let age = (self.time - self.flash_started) as f32;
        if age >= self.config.fade_time {
            return None;
        }
        let envelope = self.peak * (1.0 - age / self.config.fade_time);
        let mut flicker = Lcg((self.time * 60.0) as u32 ^ 0xc71c_2830);
        let brightness = envelope + (flicker.float() - 0.5) * (1.0 - envelope) * envelope;
        let dome = self.config.dome_tint(brightness);
        let rest = self.config.dome_tint(0.0);
        let saved_sun = light.sun_color.map(srgb_encode);
        let ambient = |rgb: [f32; 3]| {
            std::array::from_fn(|i| {
                srgb_decode(
                    (1.0 - self.config.sunlight_fade_factor)
                        * (srgb_encode(rgb[i]) + saved_sun[i] / 3.0),
                )
            })
        };
        light.ambient_sky = ambient(light.ambient_sky);
        light.ambient_ground = ambient(light.ambient_ground);
        light.sun_color = self
            .config
            .color
            .map(|c| srgb_decode(c / 255.0 * (1.0 - brightness)));
        for fog in [&mut light.fog, &mut light.water_fog].into_iter().flatten() {
            fog.color = [dome; 3];
        }
        Some((light, dome / rest.fmax(f32::EPSILON)))
    }
}
