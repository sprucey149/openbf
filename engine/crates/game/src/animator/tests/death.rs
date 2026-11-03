use super::*;
use crate::animator::hit::Direction;

#[test]
fn last_hit_direction_selects_death_and_missing_clips_fall_back() {
    for mut rig in [rig_bf1(), rig_bf2()] {
        let mut clips = clips();
        for d in Direction::ALL {
            let k = clips.len();
            let name = format!("human_rifle_{}", d.death_name());
            clips.push(clip(k, &name, 15, false));
            rig.map.push(Some(k));
            rig.sets.clips.push(SetClip {
                name,
                frames: 15,
                looping: Some(false),
                blend: None,
            });
            if let Some(t) = &mut rig.sets.bf1 {
                let id = bf1::SPECIAL
                    .iter()
                    .position(|n| *n == d.death_name())
                    .unwrap();
                t.special[id] = Some(k as u16);
                t.s[0][id] = Some(k as u16);
            } else {
                let p = bf2::simple_pair(d.death_name()).unwrap();
                let slots = rig.sets.bf2[0].sets.get_mut("rifle").unwrap();
                slots[2 * p] = Some(k as u16);
                slots[2 * p + 1] = Some(k as u16);
            }
        }
        for d in Direction::ALL {
            let mut a = Animator::new();
            a.hit.direction = d;
            a.step(
                &rig,
                &clips,
                &skel(),
                &AnimInput {
                    dead: true,
                    ..input(0.0)
                },
                0.0,
            );
            assert_eq!(
                clips[a.upper_clip().unwrap()].name,
                format!("human_rifle_{}", d.death_name())
            );
            assert_eq!(a.upper_clip(), a.lower_clip());
        }
        rig.map.pop(); // right unavailable
                       // Mapping accesses require a missing entry, not a shortened table.
        rig.map.push(None);
        assert_eq!(
            rig.death_slots(0, Direction::Right),
            rig.death_slots(0, Direction::Front)
        );
    }
}
