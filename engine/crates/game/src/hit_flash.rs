//! Local damage flash, shared by BF1 and BF2 maps (BF2 HUD).
//! BF2 FUN_001dfb38 B2:91408-91437 matches BF1 FUN_001bc190
//! B1:64387-64420: threshold uses health AFTER damage; full strength at
//! damage = 20% of health left. BF2 FUN_0045e7f0 B2:521848-521895
//! sets an immediate onset and a fade across the entire lifetime.
//! trace: PCSX2 T2 for shake/rumble and any resulting aim motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flash {
    pub alpha: u8,
    pub duration: f32,
}

impl Flash {
    pub fn from_damage(damage: f32, health_left: f32) -> Option<Self> {
        if health_left <= 0.0 || damage <= 0.02 * health_left {
            return None;
        }
        let scale = (damage / (0.2 * health_left)).min(1.0);
        Some(Self {
            alpha: (32.0 + 96.0 * scale) as u8,
            duration: 0.15 + 0.15 * scale,
        })
    }

    pub fn alpha_at(self, age: f32) -> f32 {
        f32::from(self.alpha) / 255.0 * (1.0 - age / self.duration).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flash_threshold_alpha_duration_and_fade() {
        for damage in [0.0, 1.0, 2.0] {
            assert_eq!(Flash::from_damage(damage, 100.0), None);
        }
        assert_eq!(Flash::from_damage(100.0, 0.0), None);
        let low = Flash::from_damage(2.001, 100.0).unwrap();
        assert_eq!(low.alpha, 41); // threshold is strictly above the lower endpoint
        assert!((low.duration - 0.165).abs() < 0.0001);
        let mid = Flash::from_damage(10.0, 100.0).unwrap();
        assert_eq!(mid.alpha, 80);
        assert!((mid.duration - 0.225).abs() < 0.0001);
        let full = Flash::from_damage(20.0, 100.0).unwrap();
        assert_eq!(
            full,
            Flash {
                alpha: 128,
                duration: 0.3
            }
        );
        assert_eq!(full, Flash::from_damage(500.0, 1.0).unwrap());
        assert_eq!(full.alpha_at(0.0), 128.0 / 255.0);
        assert_eq!(full.alpha_at(0.15), 64.0 / 255.0);
        assert_eq!(full.alpha_at(0.3), 0.0);
        assert_eq!(full.alpha_at(0.5), 0.0);
    }
}
