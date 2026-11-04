//! BF2 authored flight routes: manifest Properties into simulation setup.

use super::{Manifest, Vec3};

/// The PS2 path property setter's admission fields, decoded after baking.
pub fn flight_paths(m: &Manifest) -> Vec<core::vehicle::flyer::FlyerPath> {
    use crate::classdb::fnv1a_lowercase as h;
    use core::vehicle::flyer::FlyerPath;
    m.paths
        .iter()
        .filter(|p| {
            m.source_game.eq_ignore_ascii_case("bf2")
                && p.name
                    .to_ascii_lowercase()
                    .split_once(' ')
                    .is_some_and(|(kind, _)| matches!(kind, "entitypath" | "type_entitypath"))
        })
        .map(|p| {
            let mut path = FlyerPath {
                nodes: p.nodes.iter().map(|(at, _)| Vec3::from(*at)).collect(),
                name: h(p.name.split_once(' ').map_or("", |(_, name)| name)),
                ..FlyerPath::default()
            };
            for (key, value) in &p.flight_props {
                if ["Class", "Class1", "Class2", "Class3", "Class4"]
                    .iter()
                    .any(|s| h(s) == *key)
                {
                    if path.classes.len() < 5 {
                        let (name, limit) = value
                            .split_once(' ')
                            .map_or((value.as_str(), u32::MAX), |(name, count)| {
                                (name, class_limit(count))
                            });
                        path.classes.push((h(name), limit));
                    }
                } else if *key == h("Name") {
                    path.name = h(value);
                } else if *key == h("EnableObject") {
                    path.enable_instance = m
                        .instances
                        .iter()
                        .position(|i| i.name.eq_ignore_ascii_case(value))
                        .map(|i| i as u32);
                } else if let Ok(number) = value.parse::<f32>() {
                    if !number.is_finite() {
                        continue;
                    }
                    if *key == h("MaxEntities") {
                        path.max_entities = number as u32;
                    } else if *key == h("Direction") {
                        path.direction = (number as i32).clamp(-1, 1) as i8;
                    } else if *key == h("SingleDirection") {
                        path.single_direction = number > 0.0;
                    } else if *key == h("Frequency") {
                        path.frequency = number;
                    }
                }
            }
            path
        })
        .collect()
}

// FUN_002f9980 calls FUN_00520f78: signed decimal-prefix reader, stored in
// an unsigned 32-bit field. Thus "2.5" is 2 and "-1" is unlimited. This is
// distinct from MaxEntities' saturating unsigned float reader FUN_00490a78.
fn class_limit(text: &str) -> u32 {
    let text = text.trim_start();
    let sign = usize::from(text.starts_with(['-', '+']));
    let digits = text.as_bytes()[sign..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    if digits == 0 {
        return 0;
    }
    text[..sign + digits].parse::<i64>().unwrap_or_else(|_| {
        if text.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    }) as u32
}
