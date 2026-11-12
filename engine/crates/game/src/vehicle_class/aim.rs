use super::{float, floats, h};

/// The aimer flag B2 `FUN_00302f10` stores at class+0x118 (B1 +0x358): the
/// fire ring skips the aimer (B2 `FUN_00301d28`, B1 `FUN_0026d198`), so it
/// only turns (the IFT-X's `lgun_x` / `rgun_x` pivots).
const NO_FIRE: u32 = 0x407e801e;

/// One aimer of a seat weapon (B2 `FUN_00302f10`, B1 around B1:192246):
/// `WeaponName` and every `NextAimer` start one, `AimerNodeName` names it,
/// and every aimer key until the next one is its own, those before the
/// name included. Its barrels are four slots picked by `NextBarrel` (the
/// cursor restarts at each aimer, B2 `FUN_00302d80`): `BarrelNodeName`
/// writes a slot's node and fire point, `FirePointName` its fire point.
#[derive(Clone, Debug, PartialEq)]
pub struct AimerSpec {
    pub node: String,
    pub weapon: usize,
    pub fire_points: Vec<String>,
    pub yaw: [f32; 2],
    pub pitch: [f32; 2],
    pub barrels: Vec<(String, f32)>,
    pub no_fire: bool,
    pub slots: Vec<BarrelSlot>,
    slot: usize,
}

/// A barrel slot: its `BarrelNodeName` node and the point it fires from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BarrelSlot {
    pub node: Option<String>,
    pub fire_point: Option<String>,
}

impl AimerSpec {
    pub(super) fn new(node: &str, weapon: usize) -> Self {
        Self {
            node: node.trim().into(),
            weapon,
            fire_points: Vec::new(),
            yaw: [-30.0, 30.0],
            pitch: [-45.0, 45.0],
            barrels: Vec::new(),
            no_fire: false,
            slots: Vec::new(),
            slot: 0,
        }
    }

    /// Whether the aimer is still waiting for its `AimerNodeName`.
    pub(super) fn unnamed(&self) -> bool {
        self.node.is_empty()
    }

    pub(super) fn name(&mut self, node: &str) {
        self.node = node.trim().into();
    }

    /// Whether anything but its weapon was authored for it.
    pub(super) fn authored(&self) -> bool {
        !self.node.is_empty() || !self.slots.is_empty() || self.no_fire
    }

    /// The points the fire ring steps through, one per barrel slot (its
    /// fire point); an aimer with none fires from its node.
    pub fn muzzles(&self) -> Vec<&str> {
        let points: Vec<_> = self
            .slots
            .iter()
            .filter_map(|s| s.fire_point.as_deref())
            .collect();
        if points.is_empty() && !self.node.is_empty() {
            return vec![self.node.as_str()];
        }
        points
    }

    fn slot(&mut self) -> &mut BarrelSlot {
        if self.slots.len() <= self.slot {
            self.slots.resize(self.slot + 1, BarrelSlot::default());
        }
        &mut self.slots[self.slot]
    }

    pub(super) fn set(&mut self, k: u32, v: &str) {
        if k == h("FirePointName") {
            self.fire_points.push(v.trim().into());
            self.slot().fire_point = Some(v.trim().into());
        } else if k == h("NextBarrel") {
            self.slot = (self.slot + 1).min(3);
        } else if k == NO_FIRE {
            self.no_fire = float(v).is_some_and(|x| x != 0.0);
        } else if k == h("AimerYawLimits") {
            if let Some(l) = floats(v) {
                self.yaw = l;
            }
        } else if k == h("AimerPitchLimits") {
            if let Some(l) = floats(v) {
                self.pitch = l;
            }
        } else if k == h("BarrelNodeName") {
            self.barrels.push((v.trim().into(), 0.0));
            let s = self.slot();
            s.node = Some(v.trim().into());
            s.fire_point = Some(v.trim().into());
        } else if k == h("BarrelRecoil") || k == h("BarrelLength") {
            if let (Some(b), Some(x)) = (self.barrels.last_mut(), float(v)) {
                b.1 = if k == h("BarrelLength") { x * 0.25 } else { x };
            }
            self.slot();
        }
    }
}
