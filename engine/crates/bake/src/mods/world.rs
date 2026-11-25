//! Follow archive selectors recorded from a legacy mission's ReadDataFile calls.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use mungefs::addon::AddonWorldLoad;
use mungefs::Chunk;
use sha2::{Digest, Sha256};

pub(super) struct WorldInput {
    pub bytes: Vec<u8>,
    pub selectors: BTreeMap<u32, String>,
}

const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_WORLD_BYTES: usize = 256 * 1024 * 1024;

fn begin_chunk(out: &mut Vec<u8>, tag: &[u8; 4]) -> usize {
    let start = out.len();
    out.extend_from_slice(tag);
    out.extend_from_slice(&[0; 4]);
    start
}

fn finish_chunk(out: &mut Vec<u8>, start: usize) {
    let len = out.len() - start - 8;
    out[start + 4..start + 8].copy_from_slice(&(len as u32).to_le_bytes());
    out.resize(out.len().next_multiple_of(4), 0);
}

fn append(out: &mut Vec<u8>, chunk: &Chunk<'_>) -> Result<()> {
    ensure!(
        out.len() + chunk.payload.len() + 12 <= MAX_WORLD_BYTES,
        "selected world archives exceed the conversion size limit"
    );
    let start = begin_chunk(out, &chunk.tag);
    out.extend_from_slice(chunk.payload);
    finish_chunk(out, start);
    Ok(())
}

pub(super) fn read_world(loads: &[AddonWorldLoad], primary: &Path) -> Result<WorldInput> {
    let mut bytes = Vec::new();
    let mut selectors = BTreeMap::new();
    let outer = begin_chunk(&mut bytes, b"ucfb");
    for load in loads {
        let mut raw = Vec::new();
        File::open(&load.path)?
            .take(MAX_ARCHIVE_BYTES + 1)
            .read_to_end(&mut raw)?;
        ensure!(
            raw.len() as u64 <= MAX_ARCHIVE_BYTES,
            "archive {} exceeds the conversion size limit",
            load.rel_path
        );
        let root = mungefs::open(&raw).with_context(|| load.rel_path.clone())?;
        let mut wrapped = BTreeMap::new();
        let is_primary = load.path == primary;
        for chunk in mungefs::chunks(root) {
            let chunk = chunk?;
            if &chunk.tag == b"lvl_" {
                for sub in chunk.children() {
                    let sub = sub?;
                    wrapped
                        .entry(sub.tag_u32())
                        .or_insert_with(Vec::new)
                        .push(sub);
                }
            } else {
                append(&mut bytes, &chunk)?;
            }
        }
        for selector in &load.selectors {
            let hash = mungefs::fnv1a_lowercase(selector);
            let sublevels = wrapped
                .get(&hash)
                .with_context(|| format!("{} has no sublevel {selector}", load.rel_path))?;
            for sub in sublevels {
                if is_primary {
                    let wrapper = begin_chunk(&mut bytes, b"lvl_");
                    append(&mut bytes, sub)?;
                    finish_chunk(&mut bytes, wrapper);
                    selectors.insert(hash, selector.clone());
                } else {
                    for chunk in sub.children() {
                        append(&mut bytes, &chunk?)?;
                    }
                }
            }
        }
    }
    finish_chunk(&mut bytes, outer);
    Ok(WorldInput { bytes, selectors })
}

/// Within one cache, hard-link byte-identical immutable assets across map
/// variants. A filesystem without hard links can still use the converted files.
#[derive(Default)]
pub(super) struct SharedWorldFiles(BTreeMap<[u8; 32], PathBuf>);

impl SharedWorldFiles {
    pub(super) fn share(&mut self, dir: &Path) -> io::Result<()> {
        let mut paths = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
        paths.sort_by_key(|entry| entry.file_name());
        for entry in paths {
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                self.share(&path)?;
            } else if entry.file_type()?.is_file() {
                let mut digest = Sha256::new();
                io::copy(&mut File::open(&path)?, &mut digest)?;
                let digest = digest.finalize().into();
                if let Some(first) = self.0.get(&digest) {
                    let link = path.with_extension("world-share");
                    if fs::hard_link(first, &link).is_ok() && fs::rename(&link, &path).is_err() {
                        let _ = fs::remove_file(link);
                    }
                } else {
                    self.0.insert(digest, path);
                }
            }
        }
        Ok(())
    }
}

pub(super) fn link_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let output_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            link_tree(&source_path, &output_path)?;
        } else if fs::hard_link(&source_path, &output_path).is_err() {
            fs::copy(&source_path, &output_path)?;
        }
    }
    Ok(())
}
