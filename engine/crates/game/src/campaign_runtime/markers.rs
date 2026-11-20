//! Marker queries the HUD and minimap read: the world hints for the player's
//! team and the map-only markers (regions and hints with no world anchor).

use super::*;

impl CampaignRuntime {
    /// The world hints for the player's team (BF2 team = sim team + 1).
    pub fn objective_markers(&self, sim: &Sim) -> Vec<CampaignMarker> {
        self.markers(sim, true)
    }

    /// Map markers include regions and markers whose world hint is disabled.
    pub fn map_markers(&self, sim: &Sim) -> Vec<CampaignMarker> {
        self.markers(sim, false)
    }

    fn markers(&self, sim: &Sim, world: bool) -> Vec<CampaignMarker> {
        let team = self.w.player_team + 1;
        let mut out = Vec::new();
        for m in self
            .w
            .markers
            .iter()
            .filter(|m| m.scale > 0.0 && (m.team == team || m.team == 0) && (!world || m.pulse))
        {
            // OURS: the hint hangs 2 m over the object's origin.
            // trace: the hint list's draw (`DAT_0066af58`) for its anchor.
            let lift = if world { Vec3::Y * 2.0 } else { Vec3::ZERO };
            let mut push = |pos: Vec3| {
                out.push(CampaignMarker {
                    pos,
                    icon: m.icon.clone(),
                    scale: m.scale,
                    pulse: m.pulse,
                })
            };
            match &m.target {
                MarkTarget::Obj(o) => {
                    // A destroyed station remains a repair objective until
                    // its script removes the marker (user's geo1c frame 7).
                    if self.w.obj_alive(sim, *o) || matches!(o, Ud::Station(_)) {
                        // A destructable's geometry can be baked well away
                        // from its instance origin (spa3's life_int_cis is
                        // ~49 m off). Put the marker on the actual body so
                        // the player aims at the object, not empty space.
                        let p = match o {
                            Ud::Station(si) => sim
                                .stations
                                .get(usize::from(*si))
                                .and_then(|st| st.instance)
                                .and_then(|i| sim.collision.instance_centre(i))
                                .or_else(|| self.w.obj_pos(sim, *o)),
                            _ => self.w.obj_pos(sim, *o),
                        };
                        if let Some(p) = p {
                            push(p + lift);
                        }
                    }
                }
                // A region marker is the map's only (`FUN_001ad960` keeps
                // it in the map table and gives no world hint).
                MarkTarget::Region(id) => {
                    if !world {
                        if let Some((_, region)) = self.w.regions.get(*id as usize) {
                            push(region.center);
                        }
                    }
                }
                MarkTarget::Class(c) => {
                    for o in self.w.objects_of_class(sim, c) {
                        if let Some(p) = self.w.obj_pos(sim, o) {
                            push(p + lift);
                        }
                    }
                }
            }
        }
        out
    }
}
