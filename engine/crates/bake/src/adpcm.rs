//! SPU2/VAG ADPCM decode (RE: analysis/re-sound-banks.md, verified recipe).
//! 16-byte blocks: byte0 = (filter<<4)|shift, byte1 = flags, bytes 2..15 =
//! 28 nibbles (low nibble first) → 28 s16 PCM frames per block.

const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

pub fn decode(adpcm: &[u8]) -> Vec<i16> {
    let mut out = Vec::with_capacity(adpcm.len() / 16 * 28);
    let (mut h1, mut h2) = (0i32, 0i32);
    for block in adpcm.as_chunks::<16>().0 {
        let shift = (block[0] & 0x0F) as i32;
        let filter = ((block[0] >> 4) as usize).min(4);
        let (f0, f1) = FILTERS[filter];
        for i in 0..28 {
            let byte = block[2 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
            // sign-extend the 4-bit nibble into the top of an i16, then shift
            let s = (((nibble as i32) << 12) as i16 as i32) >> shift;
            let pcm = (s + (h1 * f0 + h2 * f1) / 64).clamp(-32768, 32767);
            h2 = h1;
            h1 = pcm;
            out.push(pcm as i16);
        }
    }
    out
}

pub fn decode_bank_sample(format: u32, raw: &[u8]) -> Vec<i16> {
    if format == 2 {
        raw.chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    } else if format == mungefs::stream::FORMAT_IMA {
        decode_ima(raw)
    } else {
        decode(raw)
    }
}

/// Minimal mono s16 WAV writer.
pub fn wav_mono_s16(rate: u32, pcm: &[i16]) -> Vec<u8> {
    let data_len = pcm.len() * 2;
    let mut w = Vec::with_capacity(44 + data_len);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in pcm {
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}

/// Goertzel power at `freq` Hz — used by tests to confirm tone frequency.
pub fn goertzel(pcm: &[i16], rate: f64, freq: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * freq / rate;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for &x in pcm {
        let s0 = x as f64 + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - coeff * s1 * s2
}

const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

const IMA_INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

pub fn decode_ima(adpcm: &[u8]) -> Vec<i16> {
    let mut out = Vec::with_capacity(adpcm.len() / 36 * 64);
    for blk in adpcm.as_chunks::<36>().0 {
        let mut predictor = i32::from(i16::from_le_bytes([blk[0], blk[1]]));
        let mut step_index = i32::from(blk[2].min(88));
        out.push(predictor as i16);
        for i in 0..63 {
            let byte = blk[4 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0x0f } else { byte >> 4 };
            let step = IMA_STEP_TABLE[step_index as usize];
            let mut diff = step >> 3;
            if (nibble & 4) != 0 {
                diff += step;
            }
            if (nibble & 2) != 0 {
                diff += step >> 1;
            }
            if (nibble & 1) != 0 {
                diff += step >> 2;
            }
            if (nibble & 8) != 0 {
                predictor -= diff;
            } else {
                predictor += diff;
            }
            predictor = predictor.clamp(-32768, 32767);
            step_index = (step_index + IMA_INDEX_TABLE[nibble as usize]).clamp(0, 88);
            out.push(predictor as i16);
        }
    }
    out
}

#[cfg(test)]
mod xbox_tests {
    use super::*;

    /// The disc's own sample banks of one sound LVL.
    fn read_xbox_sound(rel: &str) -> Option<Vec<u8>> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../iso/");
        let iso = format!("{root}Star Wars - Battlefront (USA, Europe).xiso.iso");
        let file = std::fs::File::open(iso).ok()?;
        let mut disc = mungefs::xdvdfs::Xdvdfs::open(std::io::BufReader::new(file)).ok()?;
        let at = disc.find(rel)?;
        let entry = disc.files[at].clone();
        disc.read_file(&entry).ok()
    }

    fn pearson(a: &[f64], b: &[f64]) -> f64 {
        let n = a.len().min(b.len());
        let m = |s: &[f64]| s[..n].iter().sum::<f64>() / n.max(1) as f64;
        let (ma, mb) = (m(a), m(b));
        let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
        for i in 0..n {
            let (u, v) = (a[i] - ma, b[i] - mb);
            ab += u * v;
            aa += u * u;
            bb += v * v;
        }
        ab / (aa * bb).sqrt().max(1e-9)
    }

    /// RMS of 256-frame windows, which survives the two discs' different rates.
    fn envelope(pcm: &[i16], rate: u32, base: u32) -> Vec<f64> {
        let step = (256.0 * f64::from(rate) / f64::from(base)).max(1.0);
        let mut out = Vec::new();
        let mut at = 0.0;
        while (at + step) as usize <= pcm.len() {
            let win = &pcm[at as usize..(at + step) as usize];
            let sum: f64 = win.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
            out.push((sum / win.len() as f64).sqrt());
            at += step;
        }
        out
    }

    /// Real data: BF1 Geonosis's samples decode to the frame count the Xbox
    /// bank declares, and the same id on the PS2 disc has the same loudness
    /// over time. That is a codec check that needs no ears.
    #[test]
    fn xbox_samples_sound_like_their_ps2_twins() {
        let Some(xbox) = read_xbox_sound("Data/_LVL_XBOX/Sound/geo.lvl") else {
            eprintln!("SKIP: no Xbox BF1 disc");
            return;
        };
        let ps2_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/GEO.LVL"
        );
        let Ok(ps2) = std::fs::read(ps2_path) else {
            eprintln!("SKIP: no PS2 BF1 sound tree");
            return;
        };
        let xbanks = mungefs::soundbank::find_banks_deep(mungefs::open(&xbox).unwrap());
        let pbanks = mungefs::soundbank::find_banks_deep(mungefs::open(&ps2).unwrap());
        let (mut compared, mut alike, mut decoded) = (0usize, 0usize, 0usize);
        for b in &xbanks {
            assert_eq!(b.format, mungefs::stream::FORMAT_IMA);
            for s in b.samples.iter().filter(|s| s.alias_of.is_none()) {
                let raw = &b.data[s.offset..s.offset + s.size];
                let pcm = decode_bank_sample(b.format, raw);
                assert_eq!(pcm.len(), s.frames as usize, "{:08x}", s.id_hash);
                decoded += 1;
                let twin = pbanks
                    .iter()
                    .flat_map(|p| p.samples.iter().map(move |t| (p, t)))
                    .find(|(_, t)| t.id_hash == s.id_hash && t.alias_of.is_none());
                let Some((p, t)) = twin else {
                    continue;
                };
                let other = decode_bank_sample(p.format, &p.data[t.offset..t.offset + t.size]);
                let (ex, ep) = (
                    envelope(&pcm, s.frequency, 22050),
                    envelope(&other, t.frequency, 22050),
                );
                if ex.len().min(ep.len()) < 16 {
                    continue;
                }
                compared += 1;
                alike += usize::from(pearson(&ex, &ep) > 0.9);
            }
        }
        eprintln!("decoded {decoded} samples; {alike}/{compared} twins alike");
        assert!(decoded > 100, "{decoded}");
        assert!(alike * 10 >= compared * 8, "{alike}/{compared}");
    }

    /// Real data: BF1 Geonosis's music stream reads as a two-channel IMA stream
    /// whose segments fit the data, and its first block starts where it says.
    #[test]
    fn xbox_streams_parse_as_ima() {
        let Some(xbox) = read_xbox_sound("Data/_LVL_XBOX/Sound/geo.lvl") else {
            eprintln!("SKIP: no Xbox BF1 disc");
            return;
        };
        let streams = mungefs::stream::find_streams(mungefs::open(&xbox).unwrap());
        assert!(!streams.is_empty());
        for s in &streams {
            assert_eq!(s.format, mungefs::stream::FORMAT_IMA);
            assert_eq!(s.channels, 2);
            for g in &s.segments {
                // The record's frame count spans every substream; a stream of
                // several counts its padding too.
                let bytes = g.size + if s.substreams > 1 { g.padding } else { 0 };
                assert_eq!(
                    (bytes / (36 * 2) * 64) as u32,
                    g.frames,
                    "{:08x}",
                    g.id_hash
                );
                assert!(g.offset + g.size <= s.data.len(), "{:08x}", g.id_hash);
            }
        }
    }
}
