//! Distance attenuation, stereo conversion and authored mixer bus gains.

use super::{fnv1a_lowercase, level_db, Listener, SoundProp, Vec3, LISTENER_ROLLOFF, SHELLFX};

/// A 3-D sound's level 0..1 at `dist` metres from the listener: the PS2's
/// function (B1 `FUN_0044cac8`, B2 `FUN_005407e8`, the same on both; `[O]`
/// analysis/re-sound-distance.md). Full between the roll-in and the minimum
/// distance; beyond the minimum `(R - x) / (x k + R)` with `x` the distance
/// past it, `R = max - min` and `k = 10 rolloff` - about `R / (k x)` over
/// most of the range, reaching 0 at the maximum; silent from the mute
/// distance out. Inside the roll-in distance it falls towards the listener
/// the same way. A 2-D (`flat`) property is not attenuated at all.
pub fn attenuation(dist: f32, p: &SoundProp) -> f32 {
    if p.flat {
        return 1.0;
    }
    let k = LISTENER_ROLLOFF * p.rolloff;
    let level = |x: f32, r: f32| {
        let den = x * k + r;
        if den > 0.0 {
            ((r - x) / den).max(0.0)
        } else {
            0.0
        }
    };
    match p.roll_in_distance {
        Some(rin) if dist < rin => level(rin - dist, rin),
        _ if dist <= p.min_distance => 1.0,
        _ if dist >= p.mute_distance.unwrap_or(p.max_distance) => 0.0,
        _ => level(dist - p.min_distance, p.max_distance - p.min_distance),
    }
}
/// Where a sound at `pos` sits for `listener`: the direction to it dotted
/// with the listener's right, -1 (left) .. 1 (right); 0 at the listener.
pub fn side(listener: &Listener, pos: Vec3) -> f32 {
    let (sy, cy) = listener.yaw.sin_cos();
    let right = Vec3::new(cy, 0.0, -sy);
    let d = pos - listener.pos;
    let dist = d.length();
    if dist < 1e-3 {
        return 0.0;
    }
    (d.dot(right) / dist).clamp(-1.0, 1.0)
}
/// The PS2's per-ear split of a 3-D voice of amplitude `a` (gain x level x
/// bus) at side `s` ([`side`]): p = 0.5 + 0.5 s, left = a (1 - p), right =
/// a p - linear, so the ears sum to `a` and a centred sound is half `a` in
/// each (output mode 3, every reference state of both games; B1
/// `FUN_0044cb78` 510409-510411, B2 `FUN_00540b98`; `[O]`
/// analysis/re-mix-headroom.md 1.1 claim 4). A 2-D sound is `(a, a)`.
pub fn ears_3d(a: f32, s: f32) -> (f32, f32) {
    let p = 0.5 + 0.5 * s.clamp(-1.0, 1.0);
    (a * (1.0 - p), a * p)
}
/// What the mixer is told for a mono take that must reach the ears at
/// `(left, right)`: (volume dB, pan -1..1). kira pans by L x sqrt(1 - m)
/// sqrt 2, R x sqrt(m) sqrt 2 with m = (pan + 1) / 2, so amplitude
/// sqrt((l^2 + r^2) / 2) at m = r^2 / (l^2 + r^2) lands on (l, r) exactly.
pub fn mixer_pair(ears: (f32, f32)) -> (f32, f32) {
    let (l, r) = ears;
    let e = l * l + r * r;
    if e <= 0.0 {
        return (level_db(0.0), 0.0);
    }
    let pan = 2.0 * r * r / e - 1.0;
    (level_db((e * 0.5).sqrt()), pan.clamp(-1.0, 1.0))
}
/// The final gain of each mixer bus a property may name, by name hash, for
/// the map's game (`source_game` `bf1` / `bf2`): the bus's disc gain x its
/// parents' x the option slider over it (`[O]` analysis/re-mix-headroom.md
/// 1.4 claims 10-12; the effects voice x 0.63 on BF2 is the RAM's
/// `+0x44 = +0xf4 x bus +0x28`). The sliders are the ones every reference
/// state holds (BF1 effects 10 voice 8, BF2 effects 7 voice 9 chatter 10):
/// PROVISIONAL [H] as the factory defaults (FEEL_NOTES). The shell's two,
/// `shellfx` 0.4 (the menu clicks) and `shellmusic` 0.6, are CORE.LVL's on
/// both discs, parent root (`analysis/re-bf2-menus.md` §4.2, §4.4). A bus
/// not listed (`root`, the battle music's) plays at 1.0. The `voiceover`
/// duck of `main` is not here (card VODK).
/// trace: clean-profile PS2 option sliders and mixer bus levels in both games before replacing
/// the assumed factory gains.
pub fn bus_table(game: &str) -> Vec<(u32, f32)> {
    let rows: &[(&str, f32)] = match game {
        "bf2" => &[
            ("soundfx", 0.63),
            ("battlechatter", 0.9),
            ("voiceover", 0.63),
            ("ambience", 0.378),
            ("shellfx", 0.4),
            ("shellmusic", 0.6),
        ],
        _ => &[
            ("soundfx", 1.0),
            ("voiceover", 0.64),
            ("ambience", 0.6),
            ("shellfx", 0.4),
            ("shellmusic", 0.6),
        ],
    };
    let mut t: Vec<(u32, f32)> = rows.iter().map(|(n, g)| (fnv1a_lowercase(n), *g)).collect();
    // The `root` bus of 1.4 claim 10 (0.4 x the effects slider) is
    // `shellfx`, so the `shellfx` row above shadows this one; which BF2
    // gain is right is open.
    t.push((SHELLFX, if game == "bf2" { 0.4 * 0.7 } else { 0.4 }));
    t
}
