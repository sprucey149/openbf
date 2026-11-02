//! Apply a public instruction list to the player's own donor resource.
//!
//! `scripts/gameplay_programs.py` writes these and checks each against the reference. Every result here is checked
//! against the table's SHA-256 before use, so a list that does not rebuild the same bytes refuses the bake.

use super::rebuild::chunk;
use super::*;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(super) struct Programs {
    schema: u32,
    reference: String,
    game: String,
    platform: String,
    collision: Vec<CollisionProgram>,
    skeleton: Vec<SkeletonProgram>,
    route: Vec<Vec<Vec<u32>>>,
    bank: Vec<BankProgram>,
}

impl Programs {
    pub(super) fn matches(&self, table: &Table) -> bool {
        self.schema == table.schema
            && self.reference == table.reference
            && self.game == table.game
            && self.platform == table.platform
    }
}

#[derive(Debug, Deserialize)]
struct CollisionProgram {
    n: String,
    b: [f32; 6],
    #[serde(default)]
    m: Option<Vec<u8>>,
    v: Vec<f64>,
    p: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct SkeletonProgram {
    name: String,
    bones: Vec<Bone>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Bone {
    Copied(usize),
    Moved(usize, [f32; 12]),
    Written(String, String, [f32; 12]),
}

type Streams = [Vec<i32>; 7];
/// A bone as the skeleton stores it: name, parent, transform.
type RawBone = (Vec<u8>, Vec<u8>, [u8; 48]);

#[derive(Debug, Deserialize)]
struct BankProgram {
    name: String,
    size: u32,
    version: u32,
    spare: u16,
    clips: Vec<ClipProgram>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ClipProgram {
    Donor {
        from: usize,
        #[serde(default)]
        mask: Option<[u32; 2]>,
        #[serde(default)]
        roots: Vec<(usize, f32, f32, f32, f32, Streams)>,
    },
    Written {
        mask: [u32; 2],
        crc: u32,
        frames: u16,
        tracks: Vec<(u32, f32, f32, f32, f32, Streams)>,
    },
}

fn program<'a, T>(list: &'a [T], index: usize, kind: &str) -> Result<&'a T> {
    list.get(index)
        .with_context(|| format!("no {kind} program {index}"))
}

fn checked(bytes: Vec<u8>, sha256: &str, what: &str) -> Result<Vec<u8>> {
    if digest(&bytes) != sha256 {
        bail!("{what}: the instructions did not rebuild the PS2's resource from this donor");
    }
    Ok(bytes)
}

fn key(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

fn add(a: f32, b: f32) -> f32 {
    (f64::from(a) + f64::from(b)) as f32
}

fn vertices(ops: &[f64], donor: &[[f32; 3]]) -> Result<Vec<[f32; 3]>> {
    let mut out = Vec::new();
    let mut at = 0;
    let arg =
        |i: usize| -> Result<f64> { ops.get(i).copied().context("vertex instructions end early") };
    let index = |value: f64| -> Result<[f32; 3]> {
        donor
            .get(value as usize)
            .copied()
            .context("vertex instruction outside the donor")
    };
    while at < ops.len() {
        match ops[at] as u32 {
            0 => {
                let (start, count) = (arg(at + 1)? as usize, arg(at + 2)? as usize);
                out.extend_from_slice(
                    donor
                        .get(start..start + count)
                        .context("vertex copy outside the donor")?,
                );
                at += 3;
            }
            1 => {
                let (start, count) = (arg(at + 1)? as usize, arg(at + 2)? as usize);
                let d = [
                    arg(at + 3)? as f32,
                    arg(at + 4)? as f32,
                    arg(at + 5)? as f32,
                ];
                for v in donor
                    .get(start..start + count)
                    .context("vertex move outside the donor")?
                {
                    out.push([add(v[0], d[0]), add(v[1], d[1]), add(v[2], d[2])]);
                }
                at += 6;
            }
            2 => {
                let mut v = index(arg(at + 1)?)?;
                *v.get_mut(arg(at + 2)? as usize)
                    .context("vertex move names no axis")? = arg(at + 3)? as f32;
                out.push(v);
                at += 4;
            }
            3 => {
                let (a, b) = (index(arg(at + 1)?)?, index(arg(at + 2)?)?);
                out.push([0, 1, 2].map(|k| ((f64::from(a[k]) + f64::from(b[k])) / 2.0) as f32));
                at += 3;
            }
            4 => {
                out.push([
                    arg(at + 1)? as f32,
                    arg(at + 2)? as f32,
                    arg(at + 3)? as f32,
                ]);
                at += 4;
            }
            _ => bail!("unknown vertex instruction"),
        }
    }
    Ok(out)
}

fn polygons(
    ops: &[u32],
    built: &[[f32; 3]],
    donor: Option<&mungefs::coll::CollisionMesh>,
) -> Result<Vec<Vec<u16>>> {
    let mut first = HashMap::new();
    for (i, v) in built.iter().enumerate() {
        first.entry(key(*v)).or_insert(i);
    }
    let mut out = Vec::new();
    let mut at = 0;
    while at < ops.len() {
        if ops[at] == 0 {
            let start = *ops.get(at + 1).context("polygon instructions end early")? as usize;
            let count = *ops.get(at + 2).context("polygon instructions end early")? as usize;
            let donor = donor.context("polygon copy without a donor")?;
            for polygon in donor
                .polygons
                .get(start..start + count)
                .context("polygon copy outside the donor")?
            {
                let mut row = Vec::with_capacity(polygon.len());
                for &v in polygon {
                    let position = key(donor.vertices[usize::from(v)]);
                    let index = first
                        .get(&position)
                        .context("copied polygon has no such vertex")?;
                    row.push(u16::try_from(*index)?);
                }
                out.push(row);
            }
            at += 3;
        } else {
            let n = ops[at] as usize;
            let row = ops
                .get(at + 1..at + 1 + n)
                .context("polygon instructions end early")?;
            out.push(
                row.iter()
                    .map(|&v| u16::try_from(v))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            at += 1 + n;
        }
    }
    Ok(out)
}

fn text(tag: &[u8; 4], value: &str) -> Vec<u8> {
    chunk(tag, &[value.as_bytes(), &[0]].concat())
}

fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

pub(super) fn collision(
    programs: &Programs,
    target: &Collision,
    donor: Option<&mungefs::Chunk<'_>>,
) -> Result<Vec<u8>> {
    let p = program(&programs.collision, target.program, "collision")?;
    let donor = donor
        .map(mungefs::coll::CollisionMesh::parse)
        .transpose()
        .map_err(|e| anyhow!("{e}"))?;
    let none = Vec::new();
    let from = donor.as_ref().map_or(&none, |d| &d.vertices);
    let built = vertices(&p.v, from)?;
    let leaves = polygons(&p.p, &built, donor.as_ref())?;
    let mut tree = Vec::new();
    let mut indices = 0u32;
    for leaf in &leaves {
        indices += leaf.len() as u32;
        let mut payload = vec![u8::try_from(leaf.len())?];
        payload.extend([0u8; 6]);
        payload.extend(leaf.iter().flat_map(|v| v.to_le_bytes()));
        tree.extend(chunk(b"LEAF", &payload));
    }
    let mut info = Vec::with_capacity(40);
    for value in [built.len() as u32, 0, leaves.len() as u32, indices] {
        info.extend(value.to_le_bytes());
    }
    info.extend(f32s(&p.b));
    let mut body = text(b"NAME", &target.at.name);
    if let Some(mask) = &p.m {
        body.extend(chunk(b"MASK", mask));
    }
    body.extend(text(b"NODE", &p.n));
    body.extend(chunk(b"INFO", &info));
    body.extend(chunk(
        b"POSI",
        &built.iter().flat_map(|v| f32s(v)).collect::<Vec<u8>>(),
    ));
    body.extend(chunk(b"TREE", &tree));
    checked(chunk(b"coll", &body), &target.sha256, &target.at.name)
}

/// A skeleton's bones as (name, parent, transform), read exactly as the donor holds them.
fn donor_bones(skel: &mungefs::Chunk<'_>) -> Result<Vec<RawBone>> {
    let bad = || anyhow!("malformed donor skeleton");
    let info = skel.child(b"INFO").ok_or_else(bad)?.payload;
    let end = info.iter().position(|&b| b == 0).ok_or_else(bad)?;
    let count = info.get(end + 1..end + 5).ok_or_else(bad)?;
    let count = u32::from_le_bytes(count.try_into()?) as usize;
    let split = |tag: &[u8; 4]| -> Result<Vec<Vec<u8>>> {
        let c = skel.child(tag).ok_or_else(bad)?;
        Ok(c.payload.split(|&b| b == 0).map(<[u8]>::to_vec).collect())
    };
    let (names, parents) = (split(b"NAME")?, split(b"PRNT")?);
    let xfrm = skel.child(b"XFRM").ok_or_else(bad)?.payload;
    (0..count)
        .map(|i| {
            let x = xfrm.get(i * 48..i * 48 + 48).ok_or_else(bad)?;
            Ok((
                names.get(i).ok_or_else(bad)?.clone(),
                parents.get(i).ok_or_else(bad)?.clone(),
                x.try_into()?,
            ))
        })
        .collect()
}

fn skeleton(p: &SkeletonProgram, donor: Option<&mungefs::Chunk<'_>>) -> Result<Vec<u8>> {
    let from = donor.map(donor_bones).transpose()?.unwrap_or_default();
    let (mut names, mut parents, mut xfrm) = (Vec::new(), Vec::new(), Vec::new());
    let get = |i: usize| from.get(i).context("bone instruction outside the donor");
    for bone in &p.bones {
        let (name, parent, numbers) = match bone {
            Bone::Copied(i) => {
                let b = get(*i)?;
                (b.0.clone(), b.1.clone(), b.2.to_vec())
            }
            Bone::Moved(i, x) => {
                let b = get(*i)?;
                (b.0.clone(), b.1.clone(), f32s(x))
            }
            Bone::Written(name, parent, x) => (
                name.clone().into_bytes(),
                parent.clone().into_bytes(),
                f32s(x),
            ),
        };
        names.extend(name);
        names.push(0);
        parents.extend(parent);
        parents.push(0);
        xfrm.extend(numbers);
    }
    let mut info = p.name.as_bytes().to_vec();
    info.push(0);
    info.extend((p.bones.len() as u32).to_le_bytes());
    let mut body = chunk(b"INFO", &info);
    body.extend(chunk(b"NAME", &names));
    body.extend(chunk(b"PRNT", &parents));
    body.extend(chunk(b"XFRM", &xfrm));
    Ok(chunk(b"skel", &body))
}

fn route(tag: &[u8; 4], ops: &[Vec<u32>], donor: Option<&mungefs::Chunk<'_>>) -> Result<Vec<u8>> {
    let from = donor.map_or(&[][..], |c| c.payload);
    let mut out = Vec::new();
    for op in ops {
        match op.as_slice() {
            [0, start, count] => {
                let (start, count) = (*start as usize * 4, *count as usize * 4);
                out.extend_from_slice(
                    from.get(start..start + count)
                        .context("route copy outside the donor")?,
                );
            }
            [1, words @ ..] => out.extend(words.iter().flat_map(|w| w.to_le_bytes())),
            [2, bytes @ ..] => {
                for b in bytes {
                    out.push(u8::try_from(*b)?);
                }
            }
            _ => bail!("unknown route instruction"),
        }
    }
    Ok(chunk(tag, &out))
}

/// The bank format's delta stream; the Python generator packs identically.
fn pack_stream(values: &[i16], out: &mut Vec<u8>) {
    let Some(&first) = values.first() else {
        return;
    };
    out.extend(first.to_le_bytes());
    let (mut current, mut at) = (first, 1);
    while at < values.len() {
        let mut run = 0;
        while at + run < values.len() && values[at + run] == current && run < 255 {
            run += 1;
        }
        if run >= 2 {
            out.extend([0x80, run as u8]);
            at += run;
            continue;
        }
        let step = i32::from(values[at]) - i32::from(current);
        if (-126..=127).contains(&step) {
            out.push(step as i8 as u8);
        } else {
            out.push(0x81);
            out.extend(values[at].to_le_bytes());
        }
        current = values[at];
        at += 1;
    }
}

fn put_streams(streams: &Streams, frames: u16, tada: &mut Vec<u8>) -> Result<[u32; 7]> {
    let mut offsets = [0u32; 7];
    for (slot, runs) in streams.iter().enumerate() {
        if runs.is_empty() {
            continue;
        }
        let mut values = Vec::with_capacity(usize::from(frames));
        for pair in runs.chunks_exact(2) {
            values.extend(std::iter::repeat_n(
                i16::try_from(pair[0])?,
                pair[1] as usize,
            ));
        }
        if values.len() != usize::from(frames) {
            bail!("animation stream length differs from its clip");
        }
        offsets[slot] = tada.len() as u32;
        pack_stream(&values, tada);
    }
    Ok(offsets)
}

fn track_record(bone: u32, offsets: &[u32; 7], base: [f32; 3], scale: f32) -> Vec<u8> {
    let mut row = bone.to_le_bytes().to_vec();
    row.extend(offsets.iter().flat_map(|o| o.to_le_bytes()));
    row.extend(f32s(&base));
    row.extend(scale.to_le_bytes());
    row
}

fn wide(mask: [u32; 2]) -> u64 {
    u64::from(mask[0]) | u64::from(mask[1]) << 32
}

fn bank(p: &BankProgram, donor: Option<&mungefs::Chunk<'_>>) -> Result<Vec<u8>> {
    let from = donor
        .map(mungefs::anim::parse_zaa)
        .transpose()
        .map_err(|e| anyhow!("{e}"))?;
    let mut tada = from.as_ref().map_or_else(Vec::new, |b| b.tada.to_vec());
    let (mut mina, mut tnja) = (Vec::new(), Vec::new());
    let mut clip_row = |mask: u64, crc: u32, frames: u16, bones: usize| -> Result<()> {
        mina.extend(mask.to_le_bytes());
        mina.extend(crc.to_le_bytes());
        mina.extend(frames.to_le_bytes());
        mina.extend(u16::try_from(bones)?.to_le_bytes());
        Ok(())
    };
    for clip in &p.clips {
        match clip {
            ClipProgram::Donor {
                from: slot,
                mask,
                roots,
            } => {
                let source = from.as_ref().context("animation clip without a donor")?;
                let info = source
                    .clips
                    .get(*slot)
                    .context("animation clip outside the donor")?;
                let first = source.first_track(*slot);
                for index in 0..usize::from(info.bones) {
                    if let Some(row) = roots.iter().find(|r| r.0 == index) {
                        let raw = source
                            .tnja
                            .get((first + index) * 0x30..(first + index) * 0x30 + 4)
                            .context("animation track outside the donor")?;
                        let offsets = put_streams(&row.5, info.frames, &mut tada)?;
                        tnja.extend(track_record(
                            u32::from_le_bytes(raw.try_into()?),
                            &offsets,
                            [row.1, row.2, row.3],
                            row.4,
                        ));
                    } else {
                        tnja.extend_from_slice(
                            source
                                .tnja
                                .get((first + index) * 0x30..(first + index + 1) * 0x30)
                                .context("animation track outside the donor")?,
                        );
                    }
                }
                let mask = mask.map_or(info.bone_mask, wide);
                clip_row(mask, info.name_crc, info.frames, usize::from(info.bones))?;
            }
            ClipProgram::Written {
                mask,
                crc,
                frames,
                tracks,
            } => {
                for row in tracks {
                    let offsets = put_streams(&row.5, *frames, &mut tada)?;
                    tnja.extend(track_record(row.0, &offsets, [row.1, row.2, row.3], row.4));
                }
                clip_row(wide(*mask), *crc, *frames, tracks.len())?;
            }
        }
    }
    let mut data = b"SMNA".to_vec();
    data.extend(p.size.to_le_bytes());
    data.extend(p.version.to_le_bytes());
    data.extend(((tnja.len() / 0x30) as u32).to_le_bytes());
    data.extend((tada.len() as u32).to_le_bytes());
    data.extend(u16::try_from(p.clips.len())?.to_le_bytes());
    data.extend(p.spare.to_le_bytes());
    data.extend(chunk(b"MINA", &mina));
    data.extend(chunk(b"TNJA", &tnja));
    data.extend(chunk(b"TADA", &tada));
    let mut body = text(b"NAME", &p.name);
    body.extend(chunk(b"BIN_", &data));
    Ok(chunk(b"zaa_", &body))
}

pub(super) fn resource(
    programs: &Programs,
    target: &Resource,
    donor: Option<&mungefs::Chunk<'_>>,
) -> Result<Vec<u8>> {
    let built = match target.kind.as_str() {
        "skel" => skeleton(
            program(&programs.skeleton, target.program, "skeleton")?,
            donor,
        )?,
        "zaa_" => bank(program(&programs.bank, target.program, "bank")?, donor)?,
        "plan" | "plnp" | "path" | "PATH" => {
            let tag: &[u8; 4] = target.kind.as_bytes().try_into()?;
            route(
                tag,
                program(&programs.route, target.program, "route")?,
                donor,
            )?
        }
        _ => bail!("unsupported gameplay resource kind"),
    };
    checked(built, &target.sha256, &target.at.name)
}
