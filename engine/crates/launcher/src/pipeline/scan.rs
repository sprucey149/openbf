//! Native Rust input detection, validation, and build planning.
//!
//! Replaces `scripts/input_scan.py`, `scripts/console_input.py`, and
//! `scripts/dlc_inputs.py` for launcher-driven setup runs, supporting every
//! combination of PS2, Xbox, and PC sources (both BF1+BF2 and BF2-only, plus
//! optional DLC heroes).

use crate::config::{LauncherConfig, Platform};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const SECTOR: u64 = 2048;
const XBOX_MAGIC: &[u8] = b"MICROSOFT*XBOX*MEDIA";
const XBOX_PARTITIONS: [u64; 4] = [0, 0x1830_0000, 0x0FD9_0000, 0x0208_0000];
const MAX_LVL: u64 = 128 << 20;
const MAX_SOUND_LVL: u64 = 32 << 20;

const WRAPPERS: &[(&[u8], &str)] = &[
    (b"CISO", "a CSO compressed disc image"),
    (b"ZISO", "a ZSO compressed disc image"),
    (b"MComprHD", "a CHD compressed disc image"),
    (b"PK\x03\x04", "a zip archive"),
    (b"7z\xbc\xaf\x27\x1c", "a 7z archive"),
    (b"Rar!", "a rar archive"),
    (b"\x1f\x8b", "a gzip archive"),
];

/// Which original game (`bf1` or `bf2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameId {
    Bf1,
    Bf2,
}

impl GameId {
    pub fn as_str(self) -> &'static str {
        match self {
            GameId::Bf1 => "bf1",
            GameId::Bf2 => "bf2",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            GameId::Bf1 => "Battlefront I",
            GameId::Bf2 => "Battlefront II",
        }
    }
}

/// Validated primary source for one game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameSource {
    Ps2Iso(PathBuf),
    XboxIso(PathBuf),
    /// Resolved `_LVL_PC` directory containing `CORE.LVL`, `MISSION.LVL`, etc.
    PcLvlDir(PathBuf),
}

impl GameSource {
    pub fn platform(&self) -> Platform {
        match self {
            GameSource::Ps2Iso(_) => Platform::Ps2,
            GameSource::XboxIso(_) => Platform::Xbox,
            GameSource::PcLvlDir(_) => Platform::Pc,
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            GameSource::Ps2Iso(_) => "PS2",
            GameSource::XboxIso(_) => "Xbox",
            GameSource::PcLvlDir(_) => "PC",
        }
    }
}

/// Resolved, validated plan for a launcher setup run.
#[derive(Clone, Debug)]
pub struct BuildPlan {
    pub bf1: Option<GameSource>,
    pub bf2: GameSource,
    pub dlc_dir: Option<PathBuf>,
}

impl BuildPlan {
    pub fn has_bf1(&self) -> bool {
        self.bf1.is_some()
    }

    /// Validate the user's selected BF1, BF2, and optional DLC paths.
    pub fn from_config(config: &LauncherConfig) -> Result<Self> {
        let bf2_path = config
            .bf2
            .path
            .as_deref()
            .ok_or_else(|| anyhow!("Battlefront II source is required to build"))?;
        let bf2 = validate_game_source(GameId::Bf2, config.bf2.platform, bf2_path)?;
        let bf1 = match &config.bf1.path {
            Some(p) => Some(validate_game_source(GameId::Bf1, config.bf1.platform, p)?),
            None => None,
        };
        let dlc_dir = match &config.dlc_dir {
            Some(p) => {
                if !p.exists() {
                    bail!("DLC folder not found: {}", p.display());
                }
                Some(p.clone())
            }
            None => None,
        };
        Ok(Self { bf1, bf2, dlc_dir })
    }
}

/// Case-insensitive component lookup under `base` (matches `rebake.sh`'s `ci`).
pub fn ci_path(base: &Path, rel: &str) -> PathBuf {
    let mut cur = base.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
        let direct = cur.join(part);
        if direct.exists() {
            cur = direct;
            continue;
        }
        let found = std::fs::read_dir(&cur).ok().and_then(|rd| {
            let mut matches: Vec<PathBuf> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.eq_ignore_ascii_case(part))
                })
                .collect();
            matches.sort();
            matches.into_iter().next()
        });
        cur = found.unwrap_or(direct);
    }
    cur
}

fn validate_game_source(game: GameId, platform: Platform, raw: &Path) -> Result<GameSource> {
    if !raw.exists() {
        bail!("{}: path not found ({})", game.title(), raw.display());
    }
    match platform {
        Platform::Ps2 => {
            if !raw.is_file() {
                bail!(
                    "{}: expected a PS2 .iso file, got {}",
                    game.title(),
                    raw.display()
                );
            }
            check_not_compressed_wrapper(raw)?;
            if !is_iso9660(raw)? {
                bail!(
                    "{}: {} is not a PS2 ISO9660 disc image",
                    game.title(),
                    raw.display()
                );
            }
            Ok(GameSource::Ps2Iso(raw.to_path_buf()))
        }
        Platform::Xbox => {
            if !raw.is_file() {
                bail!(
                    "{}: expected an Xbox .iso file, got {}",
                    game.title(),
                    raw.display()
                );
            }
            check_not_compressed_wrapper(raw)?;
            if !is_xbox_iso(raw)? {
                bail!(
                    "{}: {} is not an original Xbox XDVDFS disc image",
                    game.title(),
                    raw.display()
                );
            }
            Ok(GameSource::XboxIso(raw.to_path_buf()))
        }
        Platform::Pc => {
            if !raw.is_dir() {
                bail!(
                    "{}: expected a PC install folder, got {}",
                    game.title(),
                    raw.display()
                );
            }
            let lvl_dir = resolve_pc_lvl_dir(raw, game)?;
            Ok(GameSource::PcLvlDir(lvl_dir))
        }
    }
}

fn check_not_compressed_wrapper(path: &Path) -> Result<()> {
    let mut f = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut head = [0u8; 16];
    let n = f.read(&mut head)?;
    for (magic, desc) in WRAPPERS {
        if n >= magic.len() && &head[..magic.len()] == *magic {
            bail!(
                "{} is {desc}; decompress it to a plain .iso first",
                path.display()
            );
        }
    }
    Ok(())
}

fn is_iso9660(path: &Path) -> Result<bool> {
    let mut f = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let len = f.metadata()?.len();
    if len < 17 * SECTOR {
        return Ok(false);
    }
    f.seek(SeekFrom::Start(16 * SECTOR))?;
    let mut buf = [0u8; 7];
    f.read_exact(&mut buf)?;
    Ok(&buf == b"\x01CD001\x01")
}

fn is_xbox_iso(path: &Path) -> Result<bool> {
    let mut f = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let len = f.metadata()?.len();
    for base in XBOX_PARTITIONS {
        let off = base + 32 * SECTOR;
        if off + (XBOX_MAGIC.len() as u64) > len {
            continue;
        }
        f.seek(SeekFrom::Start(off))?;
        let mut buf = [0u8; 20];
        if f.read_exact(&mut buf).is_ok() && buf == XBOX_MAGIC {
            return Ok(true);
        }
    }
    Ok(false)
}

fn has_ucfb_header(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut head = [0u8; 4];
    f.read_exact(&mut head).is_ok() && &head == b"ucfb"
}

fn looks_like_pc_lvl_dir(dir: &Path, game: GameId) -> bool {
    let core = ci_path(dir, "CORE.LVL");
    let mission = ci_path(dir, "MISSION.LVL");
    if !has_ucfb_header(&core) || !has_ucfb_header(&mission) {
        return false;
    }
    match game {
        GameId::Bf1 => has_ucfb_header(&ci_path(dir, "COMMON/ENG/COMMON.LVL")),
        GameId::Bf2 => has_ucfb_header(&ci_path(dir, "INGAME.LVL")),
    }
}

fn is_64bit_pe(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut dos = [0u8; 64];
    if f.read_exact(&mut dos).is_err() || &dos[0..2] != b"MZ" {
        return false;
    }
    let pe_off = u32::from_le_bytes([dos[60], dos[61], dos[62], dos[63]]) as u64;
    if f.seek(SeekFrom::Start(pe_off)).is_err() {
        return false;
    }
    let mut hdr = [0u8; 6];
    if f.read_exact(&mut hdr).is_err() || &hdr[0..4] != b"PE\0\0" {
        return false;
    }
    let machine = u16::from_le_bytes([hdr[4], hdr[5]]);
    machine == 0x8664 || machine == 0xAA64
}

fn has_remaster_exe_beside(lvl_dir: &Path, stop_at: &Path) -> bool {
    let mut cur = lvl_dir.to_path_buf();
    for _ in 0..=3 {
        if let Ok(rd) = std::fs::read_dir(&cur) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                    && is_64bit_pe(&p)
                {
                    return true;
                }
            }
        }
        if cur == stop_at {
            break;
        }
        let Some(parent) = cur.parent() else {
            break;
        };
        cur = parent.to_path_buf();
    }
    false
}

/// Locate `_LVL_PC` inside a user-picked PC install folder (accepts either the
/// install root, `GameData`, `GameData/Data`, or `_LVL_PC` directly).
pub fn resolve_pc_lvl_dir(root: &Path, game: GameId) -> Result<PathBuf> {
    if looks_like_pc_lvl_dir(root, game) {
        if has_remaster_exe_beside(root, root) {
            bail!(
                "{}: 64-bit Classic Collection is not supported; use the original 32-bit PC release",
                game.title()
            );
        }
        return Ok(root.to_path_buf());
    }

    // Fast path: standard Steam/GOG/DVD layout.
    for rel in ["GameData/Data/_LVL_PC", "Data/_LVL_PC", "_LVL_PC"] {
        let cand = ci_path(root, rel);
        if cand.is_dir() && looks_like_pc_lvl_dir(&cand, game) {
            if has_remaster_exe_beside(&cand, root) {
                bail!(
                    "{}: 64-bit Classic Collection is not supported; use the original 32-bit PC release",
                    game.title()
                );
            }
            return Ok(cand);
        }
    }

    // Bounded breadth-first fallback up to depth 4.
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((root.to_path_buf(), 0usize));
    while let Some((dir, depth)) = queue.pop_front() {
        if depth >= 4 {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut subdirs: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subdirs.sort();
        for sub in subdirs {
            if looks_like_pc_lvl_dir(&sub, game) {
                if has_remaster_exe_beside(&sub, root) {
                    bail!(
                        "{}: 64-bit Classic Collection is not supported; use the original 32-bit PC release",
                        game.title()
                    );
                }
                return Ok(sub);
            }
            queue.push_back((sub, depth + 1));
        }
    }

    bail!(
        "{}: could not find PC _LVL_PC game data under {}",
        game.title(),
        root.display()
    )
}

// ---------------------------------------------------------------------------
// Optional DLC hero detection (ports scripts/dlc_inputs.py)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct DlcSource {
    pub path: PathBuf,
    pub platform: &'static str, // "ps2" or "pc"
    pub size: u64,
    pub member: Option<String>,
    pub offset: u64,
}

impl DlcSource {
    pub fn read_bytes(&self) -> Result<Vec<u8>> {
        let mut f = File::open(&self.path)
            .with_context(|| format!("opening DLC source {}", self.path.display()))?;
        f.seek(SeekFrom::Start(self.offset))?;
        let mut buf = vec![0u8; usize::try_from(self.size).context("DLC LVL too large")?];
        f.read_exact(&mut buf)
            .context("DLC source changed or was truncated after detection")?;
        Ok(buf)
    }
}

fn read_slice(f: &mut File, offset: u64, len: usize, limit: u64) -> Result<Vec<u8>> {
    let end = offset
        .checked_add(len as u64)
        .ok_or_else(|| anyhow!("offset overflow"))?;
    if end > limit {
        bail!("source extent outside file");
    }
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

fn inspect_dlc_lvl(f: &mut File, base: u64, size: u64) -> Result<Option<&'static str>> {
    if !(8..=MAX_LVL).contains(&size) {
        return Ok(None);
    }
    let head = read_slice(f, base, 8, base + size)?;
    let expected_len = u32::try_from(size - 8).unwrap_or(u32::MAX);
    if &head[0..4] != b"ucfb" || u32::from_le_bytes([head[4], head[5], head[6], head[7]]) != expected_len {
        return Ok(None);
    }
    let mut has_fisto = false;
    let mut has_ventress = false;
    let mut has_ps2 = false;
    let mut has_pc = false;
    let mut stack: Vec<([u8; 4], u64, u64, usize)> = vec![(*b"ucfb", 8, size, 0)];
    let mut count = 0usize;

    while let Some((parent, mut pos, end, depth)) = stack.pop() {
        while pos < end {
            if end - pos < 8 {
                bail!("truncated LVL chunk");
            }
            let hdr = read_slice(f, base + pos, 8, base + size)?;
            let tag = [hdr[0], hdr[1], hdr[2], hdr[3]];
            let length = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as u64;
            let body = pos + 8;
            let limit = body + length;
            if limit > end {
                bail!("LVL chunk outside parent");
            }
            count += 1;
            if count > 65536 || depth > 16 {
                bail!("DLC LVL structure exceeds bounds");
            }
            if &parent == b"entc" && &tag == b"TYPE" && length <= 128 {
                let name = read_slice(f, base + body, length as usize, base + size)?;
                if name == b"dlc_hero_fisto\0" {
                    has_fisto = true;
                } else if name == b"dlc_hero_ventress\0" {
                    has_ventress = true;
                }
            }
            if &parent == b"segm" {
                if &tag == b"_PS2" {
                    has_ps2 = true;
                } else if &tag == b"VBUF" || &tag == b"VDAT" {
                    has_pc = true;
                }
            }
            if &tag == b"lvl_" && length >= 8 {
                stack.push((tag, body + 8, limit, depth + 1));
            } else if matches!(&tag, b"ucfb" | b"modl" | b"segm" | b"entc") {
                stack.push((tag, body, limit, depth + 1));
            }
            pos = (limit + 3) & !3;
        }
    }

    if has_fisto && has_ventress {
        match (has_ps2, has_pc) {
            (true, false) => Ok(Some("ps2")),
            (false, true) => Ok(Some("pc")),
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn detect_dlc_file(path: &Path) -> Vec<DlcSource> {
    let Ok(mut f) = File::open(path) else {
        return Vec::new();
    };
    let Ok(meta) = f.metadata() else {
        return Vec::new();
    };
    let size = meta.len();
    if let Ok(Some(platform)) = inspect_dlc_lvl(&mut f, 0, size) {
        return vec![DlcSource {
            path: path.to_path_buf(),
            platform,
            size,
            member: None,
            offset: 0,
        }];
    }
    // Check ISO9660 members if this is an ISO image.
    if size < 17 * SECTOR {
        return Vec::new();
    }
    let Ok(pvd) = read_slice(&mut f, 16 * SECTOR, SECTOR as usize, size) else {
        return Vec::new();
    };
    if &pvd[..7] != b"\x01CD001\x01" {
        return Vec::new();
    }
    let root_lba = u32::from_le_bytes([pvd[158], pvd[159], pvd[160], pvd[161]]) as u64;
    let root_len = u32::from_le_bytes([pvd[166], pvd[167], pvd[168], pvd[169]]) as u64;
    let mut stack = vec![(String::new(), root_lba, root_len)];
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut records = 0usize;
    let mut found = Vec::new();

    while let Some((prefix, lba, len)) = stack.pop() {
        if !seen.insert(lba) {
            continue;
        }
        total += len;
        if seen.len() > 4096 || len > (4 << 20) || total > (16 << 20) || prefix.len() > 4096 {
            return Vec::new();
        }
        let Ok(data) = read_slice(&mut f, lba * SECTOR, len as usize, size) else {
            return Vec::new();
        };
        let mut pos = 0usize;
        while pos < data.len() {
            let rec_len = data[pos] as usize;
            if rec_len == 0 {
                pos = (pos / 2048 + 1) * 2048;
                continue;
            }
            if rec_len < 34 || pos + rec_len > data.len() {
                return Vec::new();
            }
            let rec = &data[pos..pos + rec_len];
            pos += rec_len;
            records += 1;
            let name_len = rec[32] as usize;
            if records > 65536 || 33 + name_len > rec.len() {
                return Vec::new();
            }
            let raw_name = &rec[33..33 + name_len];
            if raw_name == [0] || raw_name == [1] {
                continue;
            }
            let Ok(name_str) = std::str::from_utf8(raw_name) else {
                return Vec::new();
            };
            let clean = name_str.split(';').next().unwrap_or("");
            if clean.contains('/') || clean.contains('\\') || clean == "." || clean == ".." {
                return Vec::new();
            }
            let extent_off =
                u32::from_le_bytes([rec[2], rec[3], rec[4], rec[5]]) as u64 * SECTOR;
            let extent_len = u32::from_le_bytes([rec[10], rec[11], rec[12], rec[13]]) as u64;
            if extent_off.saturating_add(extent_len) > size {
                return Vec::new();
            }
            let member = format!("{prefix}{clean}");
            if rec[25] & 2 != 0 {
                stack.push((format!("{member}/"), extent_off / SECTOR, extent_len));
            } else if let Ok(Some(platform)) = inspect_dlc_lvl(&mut f, extent_off, extent_len) {
                found.push(DlcSource {
                    path: path.to_path_buf(),
                    platform,
                    size: extent_len,
                    member: Some(member),
                    offset: extent_off,
                });
            }
        }
    }
    found
}

/// Discover all Kit Fisto / Asajj Ventress side LVLs under `root`.
pub fn detect_dlc(root: &Path) -> Vec<DlcSource> {
    if root.is_file() {
        return detect_dlc_file(root);
    }
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                found.extend(detect_dlc_file(&p));
            }
        }
    }
    found
}

fn fnv1a_lowercase(name: &str) -> u32 {
    let mut value = 2166136261u32;
    for byte in name.bytes() {
        value = (value ^ u32::from(byte | 0x20)).wrapping_mul(16777619);
    }
    value
}

/// Find the smallest sound LVL defining Fisto's bubble sounds under `root`.
pub fn detect_hero_sounds(root: &Path) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let wanted = fnv1a_lowercase("hero_weap_energyBall_throw").to_le_bytes();
    let mut best: Option<(usize, PathBuf)> = None;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                let Ok(meta) = p.metadata() else {
                    continue;
                };
                if meta.len() > MAX_SOUND_LVL {
                    continue;
                }
                let Ok(data) = std::fs::read(&p) else {
                    continue;
                };
                if data.len() >= 4
                    && &data[..4] == b"ucfb"
                    && data.windows(4).any(|w| w == wanted)
                    && best.as_ref().is_none_or(|(len, _)| data.len() < *len)
                {
                    best = Some((data.len(), p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_pc_lvl_dir_from_install_root_and_rejects_missing() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_scan_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let lvl = dir.join("GameData").join("Data").join("_LVL_PC");
        std::fs::create_dir_all(&lvl).unwrap();
        std::fs::write(lvl.join("CORE.LVL"), b"ucfb\0\0\0\0").unwrap();
        std::fs::write(lvl.join("MISSION.LVL"), b"ucfb\0\0\0\0").unwrap();
        std::fs::write(lvl.join("INGAME.LVL"), b"ucfb\0\0\0\0").unwrap();

        let resolved = resolve_pc_lvl_dir(&dir, GameId::Bf2).expect("resolve bf2 pc");
        assert_eq!(resolved, lvl);
        assert!(resolve_pc_lvl_dir(&dir, GameId::Bf1).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fnv1a_lowercase_matches_python_hash() {
        // Matches dlc_inputs._fnv1a_lowercase("hero_weap_energyBall_throw").
        assert_eq!(fnv1a_lowercase("hero_weap_energyBall_throw"), 0x8259_dc8a);
    }
}
