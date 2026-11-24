//! Battle-load decode prefetch. OURS: no disc behaviour, only where the
//! decode work runs.
//!
//! Rules:
//! - [`record`] opens a session on the calling thread (the event thread's
//!   battle install). Every texture, skinned GLB and GLB that thread decodes
//!   while the session is open is noted, in order, and the list is kept per
//!   load key, in memory and under `openbf_data/cache/preload/`.
//! - [`warm`] (the loading worker) decodes the key's last list side by side
//!   into a shared table before the install starts. Only the recording
//!   thread takes from it; any other caller decodes as before.
//! - A taken entry is the same file's decode, so the result never differs
//!   from the serial load: a missing or stale list only costs the time it
//!   saved. The table is emptied when the session closes and when the next
//!   warm starts.
//! - OURS: the texture budget [`BUDGET_BYTES`] bounds the extra memory the
//!   table holds at once. measure: peak working set on the largest map.

use super::{CpuMesh, CpuTexture, SkinnedAsset};
use anyhow::Result;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const BUDGET_BYTES: usize = 1 << 30;

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Texture(PathBuf, String),
    Skinned(PathBuf),
    Glb(PathBuf),
}

enum Value {
    Texture(CpuTexture),
    Skinned(SkinnedAsset),
    Glb(CpuMesh, Vec<[u8; 4]>),
}

struct Session {
    key: String,
    seen: HashSet<Key>,
    order: Vec<Key>,
    hits: usize,
}

static READY: Mutex<Option<HashMap<Key, Value>>> = Mutex::new(None);
static LISTS: Mutex<Option<HashMap<String, Vec<Key>>>> = Mutex::new(None);

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

/// The load key both sides agree on: the map directory and the mode.
pub fn load_key(map_dir: &Path, mode: &str) -> String {
    let map = map_dir
        .file_name()
        .map_or_else(|| map_dir.to_string_lossy(), |n| n.to_string_lossy());
    format!("{map}_{mode}")
}

fn list_path(key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    crate::profile::data_dir()
        .join("cache")
        .join("preload")
        .join(format!("{safe}.txt"))
}

fn encode(keys: &[Key]) -> String {
    let mut out = String::new();
    let plain = |s: &str| !s.contains(['\t', '\n', '\r']);
    for k in keys {
        match k {
            Key::Texture(dir, name) => {
                let dir = dir.to_string_lossy();
                if plain(&dir) && plain(name) {
                    out.push_str(&format!("t\t{dir}\t{name}\n"));
                }
            }
            Key::Skinned(p) | Key::Glb(p) => {
                let p = p.to_string_lossy();
                if plain(&p) {
                    let tag = if matches!(k, Key::Skinned(_)) {
                        's'
                    } else {
                        'g'
                    };
                    out.push_str(&format!("{tag}\t{p}\n"));
                }
            }
        }
    }
    out
}

fn decode(text: &str) -> Vec<Key> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            match (f.next()?, f.next()?, f.next()) {
                ("t", dir, Some(name)) => Some(Key::Texture(dir.into(), name.to_owned())),
                ("s", p, None) => Some(Key::Skinned(p.into())),
                ("g", p, None) => Some(Key::Glb(p.into())),
                _ => None,
            }
        })
        .collect()
}

fn list_for(key: &str) -> Vec<Key> {
    if let Some(l) = LISTS
        .lock()
        .ok()
        .and_then(|m| m.as_ref()?.get(key).cloned())
    {
        return l;
    }
    std::fs::read_to_string(list_path(key))
        .map(|t| decode(&t))
        .unwrap_or_default()
}

fn texture_bytes(t: &CpuTexture) -> usize {
    let own = t.rgba.len() + t.extra_mips.iter().map(Vec::len).sum::<usize>();
    own + t.detail.as_deref().map_or(0, texture_bytes)
}

/// Drop what a warm decoded for a load that will not install.
pub fn discard() {
    clear();
}

fn clear() {
    if let Ok(mut r) = READY.lock() {
        *r = None;
    }
}

/// Decode `key`'s last recorded list side by side (module rules). Returns
/// how many entries were decoded.
pub fn warm(key: &str) -> usize {
    clear();
    let list = list_for(key);
    if list.is_empty() {
        return 0;
    }
    if let Ok(mut r) = READY.lock() {
        *r = Some(HashMap::new());
    }
    let next = AtomicUsize::new(0);
    let bytes = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map_or(2, |n| n.get().saturating_sub(2))
        .clamp(1, 8);
    std::thread::scope(|s| {
        for i in 0..threads {
            let (list, next, bytes) = (&list, &next, &bytes);
            let _ = std::thread::Builder::new()
                .name(format!("loading-prefetch-{i}"))
                .spawn_scoped(s, move || loop {
                    let k = match list.get(next.fetch_add(1, Ordering::Relaxed)) {
                        Some(k) => k,
                        None => break,
                    };
                    let value = match k {
                        Key::Texture(dir, name) => {
                            if bytes.load(Ordering::Relaxed) >= BUDGET_BYTES {
                                continue;
                            }
                            super::decode_texture(dir, name).ok().map(|t| {
                                bytes.fetch_add(texture_bytes(&t), Ordering::Relaxed);
                                Value::Texture(t)
                            })
                        }
                        Key::Skinned(p) => {
                            super::mesh::decode_skinned_glb(p).ok().map(Value::Skinned)
                        }
                        Key::Glb(p) => super::mesh::decode_glb_joints(p)
                            .ok()
                            .map(|(m, j)| Value::Glb(m, j)),
                    };
                    if let (Some(v), Ok(mut r)) = (value, READY.lock()) {
                        if let Some(map) = r.as_mut() {
                            map.insert(k.clone(), v);
                        }
                    }
                });
        }
    });
    READY
        .lock()
        .ok()
        .and_then(|r| r.as_ref().map(HashMap::len))
        .unwrap_or(0)
}

/// The open session; closing it keeps the list and empties the table.
pub struct Recording {
    _not_send: std::marker::PhantomData<*const ()>,
}

/// Open a session on this thread for the load `key` (module rules).
pub fn record(key: &str) -> Recording {
    SESSION.with(|s| {
        *s.borrow_mut() = Some(Session {
            key: key.to_owned(),
            seen: HashSet::new(),
            order: Vec::new(),
            hits: 0,
        })
    });
    Recording {
        _not_send: std::marker::PhantomData,
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        let session = SESSION.with(|s| s.borrow_mut().take());
        let left = READY
            .lock()
            .ok()
            .and_then(|mut r| r.take())
            .map_or(0, |m| m.len());
        let Some(s) = session else { return };
        println!(
            "loading: prefetch key={} decodes={} hits={} unused={left}",
            s.key,
            s.order.len(),
            s.hits
        );
        if s.order.is_empty() {
            return;
        }
        let path = list_path(&s.key);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, encode(&s.order));
        if let Ok(mut l) = LISTS.lock() {
            l.get_or_insert_with(HashMap::new).insert(s.key, s.order);
        }
    }
}

fn take(key: Key) -> Option<Value> {
    let fresh = SESSION.with(|s| {
        let mut s = s.borrow_mut();
        let s = s.as_mut()?;
        Some(if s.seen.insert(key.clone()) {
            s.order.push(key.clone());
            true
        } else {
            false
        })
    })?;
    if !fresh {
        return None;
    }
    let v = READY.lock().ok()?.as_mut()?.remove(&key)?;
    SESSION.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.hits += 1;
        }
    });
    Some(v)
}

pub(super) fn texture(
    dir: &Path,
    name: &str,
    decode: impl FnOnce() -> Result<CpuTexture>,
) -> Result<CpuTexture> {
    match take(Key::Texture(dir.to_owned(), name.to_owned())) {
        Some(Value::Texture(t)) => Ok(t),
        _ => decode(),
    }
}

pub(super) fn skinned(
    path: &Path,
    decode: impl FnOnce() -> Result<SkinnedAsset>,
) -> Result<SkinnedAsset> {
    match take(Key::Skinned(path.to_owned())) {
        Some(Value::Skinned(a)) => Ok(a),
        _ => decode(),
    }
}

pub(super) fn glb(
    path: &Path,
    decode: impl FnOnce() -> Result<(CpuMesh, Vec<[u8; 4]>)>,
) -> Result<(CpuMesh, Vec<[u8; 4]>)> {
    match take(Key::Glb(path.to_owned())) {
        Some(Value::Glb(m, j)) => Ok((m, j)),
        _ => decode(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_round_trips() {
        let keys = vec![
            Key::Texture("a/b".into(), "tex".into()),
            Key::Skinned("a/s.glb".into()),
            Key::Glb("a/g.glb".into()),
        ];
        assert!(decode(&encode(&keys)) == keys);
    }

    #[test]
    fn only_the_recording_thread_takes() {
        let k = Key::Glb("x/only.glb".into());
        assert!(take(k.clone()).is_none());
        let _r = record("test-only");
        assert!(take(k.clone()).is_none());
        assert!(SESSION.with(|s| s.borrow().as_ref().unwrap().order.len()) == 1);
        assert!(take(k).is_none());
        assert!(SESSION.with(|s| s.borrow().as_ref().unwrap().order.len()) == 1);
        SESSION.with(|s| s.borrow_mut().as_mut().unwrap().order.clear());
    }
}
