//! A `wrld` chunk's object animations (`anim`) and animation groups
//! (`anmg`), the data `PlayAnimation` and its kin drive.
//!
//! `anim`: INFO = name NUL, f32 duration, u8 loop, u8 local; then `POSK`
//! and `ROTK` keys of 41 bytes: f32 time, f32[3] value, u8 interpolation,
//! f32[3] out tangent, f32[3] in tangent. BF2 FUN_00272ef8 reads them
//! (INFO 0x4f464e49, ROTK 0x4b544f52, POSK 0x4b534f50); FUN_00273168 stores
//! the duration at anim+8, loop at +4, local at +5. An interpolation byte
//! that is not 1 (linear) or 2 (spline) is stored as 0 (step).
//!
//! `anmg`: INFO = group name NUL, u8 play at load, u8 restart-on-state flag
//! (group+0xc bit 16); `ANIM` = animation name NUL, object name NUL, one per
//! entry; `NOHI` sets bit 17, which keeps the group off the hierarchy pass.
//! BF2 FUN_00274280.

use crate::{le, Chunk};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub time: f32,
    pub value: [f32; 3],
    /// 0 step, 1 linear, 2 spline.
    pub interp: u8,
    pub tangent_out: [f32; 3],
    pub tangent_in: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Anim {
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    pub local: bool,
    pub pos: Vec<Key>,
    pub rot: Vec<Key>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub play: bool,
    pub restart_on_state: bool,
    pub no_hierarchy: bool,
    /// (animation name, object name).
    pub entries: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldAnims {
    pub anims: Vec<Anim>,
    pub groups: Vec<Group>,
}

fn cstr(p: &[u8], at: usize) -> Option<(String, usize)> {
    let rest = p.get(at..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    let s = std::str::from_utf8(&rest[..end]).ok()?.to_owned();
    Some((s, at + end + 1))
}

fn key(p: &[u8]) -> Option<Key> {
    if p.len() < 41 {
        return None;
    }
    let f = |o: usize| le::f32_at(p, o);
    let interp = match p[16] {
        1 => 1,
        2 => 2,
        _ => 0,
    };
    Some(Key {
        time: f(0),
        value: [f(4), f(8), f(12)],
        interp,
        tangent_out: [f(17), f(21), f(25)],
        tangent_in: [f(29), f(33), f(37)],
    })
}

fn anim(c: &Chunk<'_>) -> Option<Anim> {
    let info = c.child(b"INFO")?;
    let (name, at) = cstr(info.payload, 0)?;
    let p = info.payload;
    if p.len() < at + 6 {
        return None;
    }
    let mut a = Anim {
        name,
        duration: le::f32_at(p, at),
        looping: p[at + 4] != 0,
        local: p[at + 5] != 0,
        pos: Vec::new(),
        rot: Vec::new(),
    };
    for k in c.children().flatten() {
        match &k.tag {
            b"POSK" => a.pos.extend(key(k.payload)),
            b"ROTK" => a.rot.extend(key(k.payload)),
            _ => {}
        }
    }
    Some(a)
}

fn group(c: &Chunk<'_>) -> Option<Group> {
    let info = c.child(b"INFO")?;
    let (name, at) = cstr(info.payload, 0)?;
    let byte = |i: usize| info.payload.get(at + i).copied().unwrap_or(0) != 0;
    let mut g = Group {
        name,
        play: byte(0),
        restart_on_state: byte(1),
        no_hierarchy: false,
        entries: Vec::new(),
    };
    for e in c.children().flatten() {
        match &e.tag {
            b"ANIM" => {
                let Some((anim, at)) = cstr(e.payload, 0) else {
                    continue;
                };
                let Some((object, _)) = cstr(e.payload, at) else {
                    continue;
                };
                g.entries.push((anim, object));
            }
            b"NOHI" => g.no_hierarchy = true,
            _ => {}
        }
    }
    Some(g)
}

/// The animations and groups of one `wrld`.
pub fn parse(wrld: &Chunk<'_>) -> WorldAnims {
    let mut out = WorldAnims::default();
    for c in wrld.children().flatten() {
        match &c.tag {
            b"anim" => out.anims.extend(anim(&c)),
            b"anmg" => out.groups.extend(group(&c)),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::from(*tag);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        out
    }

    fn f32_bytes(v: f32) -> [u8; 4] {
        v.to_le_bytes()
    }

    fn key_bytes(time: f32, value: [f32; 3], interp: u8) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&f32_bytes(time));
        for v in value {
            out.extend_from_slice(&f32_bytes(v));
        }
        out.push(interp);
        for v in [0.5, 0.25, 0.125] {
            out.extend_from_slice(&f32_bytes(v));
        }
        for v in [-0.5, -0.25, -0.125] {
            out.extend_from_slice(&f32_bytes(v));
        }
        assert_eq!(out.len(), 41, "the PS2 key is 41 bytes");
        out
    }

    #[test]
    fn parses_one_animation_and_group_with_their_keys() {
        let mut info = b"my_anim\0".to_vec();
        info.extend_from_slice(&f32_bytes(4.5));
        info.extend_from_slice(&[1, 0]);
        let k = key_bytes(1.25, [1.0, -2.0, 3.0], 2);
        let mut anim_payload = chunk(b"INFO", &info);
        anim_payload.extend(chunk(b"POSK", &k));
        anim_payload.extend(chunk(b"ROTK", &key_bytes(0.0, [0.0, 0.0, 0.0], 0)));

        let mut ginfo = b"my_group\0".to_vec();
        ginfo.extend_from_slice(&[1, 1]);
        let mut group_payload = chunk(b"INFO", &ginfo);
        group_payload.extend(chunk(b"ANIM", b"my_anim\0my_object\0"));
        group_payload.extend(chunk(b"NOHI", &[]));

        let mut wrld_payload = chunk(b"anim", &anim_payload);
        wrld_payload.extend(chunk(b"anmg", &group_payload));
        let wrld = chunk(b"wrld", &wrld_payload);
        let c = crate::chunks(&wrld).next().unwrap().unwrap();
        let out = parse(&c);

        assert_eq!(out.anims.len(), 1);
        let a = &out.anims[0];
        assert_eq!(a.name, "my_anim");
        assert_eq!(a.duration, 4.5);
        assert!(a.looping);
        assert!(!a.local);
        assert_eq!(a.pos.len(), 1);
        assert_eq!(a.pos[0].time, 1.25);
        assert_eq!(a.pos[0].value, [1.0, -2.0, 3.0]);
        assert_eq!(a.pos[0].interp, 2);
        assert_eq!(a.pos[0].tangent_out, [0.5, 0.25, 0.125]);
        assert_eq!(a.pos[0].tangent_in, [-0.5, -0.25, -0.125]);
        assert_eq!(a.rot.len(), 1);
        assert_eq!(a.rot[0].interp, 0, "an unknown interpolation is step");

        assert_eq!(out.groups.len(), 1);
        let g = &out.groups[0];
        assert_eq!(g.name, "my_group");
        assert!(g.play && g.restart_on_state && g.no_hierarchy);
        assert_eq!(g.entries, vec![("my_anim".into(), "my_object".into())]);
    }
}
