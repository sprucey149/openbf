//! The BF2 shell's bake (card BSHLA, `analysis/re-bf2-shell.md` §1, §4):
//! `<common_bf2>/shell/shell.ron` and the frame packs beside it.
//!
//! The background movie (`MOVIES/SHELL.MVS`, MPEG-2 512 × 448 at 29.97 fps)
//! is transcoded at bake time to one **frame pack** per segment: every frame
//! a JPEG, the game decoding the one it shows on a worker thread. The pack
//! is ours (the PS2 decodes the MPEG-2 on its IPU); its layout:
//!
//! ```text
//! b"OBJM"  u32 version (1)  u32 width  u32 height
//! u32 fps numerator  u32 fps denominator  u32 frame count n
//! (n + 1) × u32 byte offsets of the frames, from the start of the file
//! the JPEGs
//! ```
//! All numbers little-endian.

use serde::{Deserialize, Serialize};

/// `shell.ron`: what the shell bake wrote.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShellIndex {
    /// The movie properties the menus play (the disc's `mcfg`), each its
    /// segments' frame packs in the property's order (`movies/<file>`).
    pub movies: Vec<ShellMovie>,
    /// The shell's own sheets baked to `textures/<name>.ktx2`.
    pub textures: Vec<String>,
    /// The shell music's stream property (`sounds/` holds SOUND/SHELL.LVL's
    /// bank and the stream's index).
    pub music: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShellMovie {
    pub name: String,
    /// Frame pack file names under `movies/`, in play order.
    pub segments: Vec<String>,
    /// Optional demuxed audio file name under `movies/` (for campaign cutscenes/transitions).
    /// The default keeps `shell.ron` baked before campaign audio loadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
}

pub const PACK_MAGIC: &[u8; 4] = b"OBJM";
pub const PACK_VERSION: u32 = 1;
const HEAD: usize = 28;

/// A frame pack read back: the frames are byte ranges of `bytes`.
#[derive(Debug, Clone)]
pub struct FramePack {
    pub width: u32,
    pub height: u32,
    pub fps: (u32, u32),
    offsets: Vec<u32>,
    bytes: Vec<u8>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

impl FramePack {
    /// Lay `frames` (encoded images) out as a pack.
    pub fn write(width: u32, height: u32, fps: (u32, u32), frames: &[Vec<u8>]) -> Vec<u8> {
        let n = frames.len();
        let mut out = Vec::new();
        out.extend_from_slice(PACK_MAGIC);
        for v in [PACK_VERSION, width, height, fps.0, fps.1, n as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let mut at = (HEAD + 4 * (n + 1)) as u32;
        for f in frames {
            out.extend_from_slice(&at.to_le_bytes());
            at += f.len() as u32;
        }
        out.extend_from_slice(&at.to_le_bytes());
        for f in frames {
            out.extend_from_slice(f);
        }
        out
    }

    /// Parse a pack; `None` when the header or the offsets do not hold.
    pub fn parse(bytes: Vec<u8>) -> Option<FramePack> {
        if bytes.get(..4)? != PACK_MAGIC || u32_at(&bytes, 4)? != PACK_VERSION {
            return None;
        }
        let n = u32_at(&bytes, 24)? as usize;
        let offsets: Vec<u32> = (0..=n)
            .map(|k| u32_at(&bytes, HEAD + 4 * k))
            .collect::<Option<_>>()?;
        let sorted = offsets.windows(2).all(|w| w[0] <= w[1]);
        if !sorted || *offsets.last()? as usize > bytes.len() {
            return None;
        }
        Some(FramePack {
            width: u32_at(&bytes, 8)?,
            height: u32_at(&bytes, 12)?,
            fps: (u32_at(&bytes, 16)?, u32_at(&bytes, 20)?.max(1)),
            offsets,
            bytes,
        })
    }

    pub fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Frame `k`'s encoded bytes.
    pub fn frame(&self, k: usize) -> Option<&[u8]> {
        let (a, b) = (*self.offsets.get(k)?, *self.offsets.get(k + 1)?);
        self.bytes.get(a as usize..b as usize)
    }

    /// Seconds the pack plays.
    pub fn seconds(&self) -> f32 {
        self.len() as f32 * self.fps.1 as f32 / self.fps.0.max(1) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_reads_back_what_was_written() {
        let frames = vec![vec![1, 2, 3], vec![], vec![9; 5]];
        let p = FramePack::parse(FramePack::write(512, 448, (30000, 1001), &frames)).unwrap();
        assert_eq!(
            (p.width, p.height, p.fps, p.len()),
            (512, 448, (30000, 1001), 3)
        );
        for (k, f) in frames.iter().enumerate() {
            assert_eq!(p.frame(k).unwrap(), &f[..]);
        }
        assert!(p.frame(3).is_none());
        assert!(FramePack::parse(b"OBJX".to_vec()).is_none());
    }
}
