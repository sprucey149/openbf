//! One side LVL's sheets, parsed once, and the writes a bake asks of them. A
//! replacement side bakes many models that share sheets: each model asks for
//! its sheets ([`Request`]), a [`Plan`] keeps the last word on each, and
//! [`LvlSheets::write`] writes every file once.

use anyhow::{anyhow, Result};
use mungefs::texture::Texture;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// OURS: the most workers of [`in_parallel`]. Each model bake parses its own
/// copy of the animation banks, so memory grows with the workers.
/// measure: a large side mod's wall time and peak memory at other counts.
const MAX_WORKERS: usize = 8;

/// `run` over every item on up to [`MAX_WORKERS`] threads, results in item
/// order. `done` is called on this thread as each item finishes, with the
/// number finished so far.
pub(crate) fn in_parallel<T: Sync, R: Send>(
    items: &[T],
    run: impl Fn(&T) -> R + Sync,
    mut done: impl FnMut(usize, &T),
) -> Vec<R> {
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(MAX_WORKERS)
        .min(items.len());
    let next = AtomicUsize::new(0);
    let mut results: Vec<Option<R>> = items.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::channel();
        for _ in 0..workers {
            let (tx, next, run) = (tx.clone(), &next, &run);
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(i) else { break };
                if tx.send((i, run(item))).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        for (finished, (i, result)) in rx.iter().enumerate() {
            results[i] = Some(result);
            done(finished + 1, &items[i]);
        }
    });
    results.into_iter().flatten().collect()
}

/// The sheets one model asks of its LVL: names (lowercase) and the flags
/// [`super::bake_textures_flagged`] decides their alpha by.
pub(crate) struct Request {
    pub wanted: Vec<String>,
    pub flags: HashMap<String, u32>,
}

/// Which sheets to write, and whether each keeps its alpha. A later request
/// of a sheet replaces the earlier one, as writing them in turn did.
#[derive(Default)]
pub(crate) struct Plan(BTreeMap<String, bool>);

impl Plan {
    /// Add a request; returns the sheets of it the LVL has, in LVL order.
    pub fn add(&mut self, sheets: &LvlSheets, request: &Request) -> Vec<String> {
        let names = sheets.names(&request.wanted);
        for name in &names {
            let flags = request
                .flags
                .get(name)
                .copied()
                .unwrap_or(crate::detail::ALPHA_IS_MEANINGFUL);
            self.0.insert(
                name.clone(),
                flags & crate::detail::ALPHA_IS_MEANINGFUL != 0,
            );
        }
        names
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An LVL's decoded `tex_` chunks, with the normal maps and environment
/// cubes its models pair them with.
pub(crate) struct LvlSheets {
    all: Vec<Texture>,
    first: HashMap<String, usize>,
    bumps: HashMap<String, crate::bump::Companion>,
    /// [`crate::envmap`] cubes, keyed by diffuse.
    envs: HashMap<String, crate::envmap::Cube>,
}

impl LvlSheets {
    pub fn parse(inner: &[u8]) -> Result<Self> {
        // Decode everything first: a wanted texture's `_dtl` companion can sit
        // anywhere in the LVL, including before the base (crate::detail).
        let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
        let all: Vec<Texture> = chunks
            .iter()
            .filter(|c| &c.tag == b"tex_")
            .filter_map(|c| Texture::parse(c).ok())
            .collect();
        let mut first = HashMap::new();
        for (i, t) in all.iter().enumerate() {
            first.entry(t.name.to_ascii_lowercase()).or_insert(i);
        }
        let models = crate::bump::lvl_models(&chunks);
        let bumps = crate::bump::companions(&models);
        let envs = crate::envmap::companions(&models)
            .into_iter()
            .filter_map(|(base, cube)| Some((base, crate::envmap::find_cube(&chunks, &cube)?)))
            .collect();
        Ok(Self {
            all,
            first,
            bumps,
            envs,
        })
    }

    /// The names of `wanted` (lowercase) the LVL has, each once, in LVL order.
    pub fn names(&self, wanted: &[String]) -> Vec<String> {
        let mut found: Vec<(usize, &String)> = wanted
            .iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .filter_map(|name| Some((*self.first.get(name)?, name)))
            .collect();
        found.sort_unstable();
        found.into_iter().map(|(_, name)| name.clone()).collect()
    }

    /// Write every planned sheet and its companions into `dir`, with the
    /// classic-plus tier from `pc` beside each where the chain has one.
    pub fn write(
        &self,
        plan: &Plan,
        dir: &Path,
        pc: Option<&crate::tier::PcTextures>,
    ) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let sheets: Vec<(&String, bool)> = plan.0.iter().map(|(n, a)| (n, *a)).collect();
        let written = in_parallel(
            &sheets,
            |&(name, alpha_carried)| self.write_one(name, alpha_carried, dir, pc),
            |_, _| {},
        );
        let mut tier2_written = 0usize;
        for tier2 in written {
            tier2_written += usize::from(tier2?);
        }
        if pc.is_some() {
            eprintln!(
                "  tier 2 written beside the PS2 base: {tier2_written} of {}",
                sheets.len()
            );
        }
        Ok(())
    }

    /// Whether a second tier was written.
    fn write_one(
        &self,
        name: &str,
        alpha_carried: bool,
        dir: &Path,
        pc: Option<&crate::tier::PcTextures>,
    ) -> Result<bool> {
        let t = &self.all[self.first[name]];
        // Both tiers, side by side, exactly as `crate::map:1004-1033` writes
        // them (PLAN §Classic-plus assets, the maintainer 2026-09-03): the base file is
        // ALWAYS the PS2 disc's pixels — the sim reads only that — and the
        // classic-plus override, where the chain has one, is written beside it
        // as `<name>.tier2.ktx2` for the runtime to pick between.
        //
        // The alpha state is the PS2 texture's on BOTH files. The side bake
        // decides it here from the per-texture OR of the material flags (the
        // map bake decides it per draw), so the flags land on whichever
        // pixels are written; and where a draw does want alpha, the override
        // takes the PS2 mask instead of its own channel, which is gloss/spec
        // on several shipped sheets — a MASK draw would otherwise cut out
        // along a mask nobody authored and the tier flip would change the
        // silhouette.
        let mut r = crate::tier::Resolved::from_ps2(t);
        let usage = crate::tier::Usage {
            sky: false,
            alpha_carried,
        };
        let mut tier2 = crate::tier::tier2(t, pc, usage);
        if !alpha_carried {
            r.make_opaque();
            if let Some(o) = tier2.as_mut() {
                o.make_opaque();
            }
        } else if let Some(o) = tier2.as_mut() {
            o.take_alpha_from(&r);
        }
        r.write_pair(dir, name, tier2.as_ref())?;
        // The detail companion stays PS2: `tier::tier2` refuses a
        // detail-compressed base outright (exclusion 1), so a base and its
        // companion cannot come from two different tiers.
        crate::detail::write_detail_for(dir, t, &self.all)?;
        crate::bump::write_bump_for(dir, name, &self.bumps, &self.all)?;
        let base = name.to_ascii_lowercase();
        crate::envmap::write_cube(dir, &base, self.envs.get(&base))?;
        Ok(tier2.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_results_keep_item_order_and_report_every_item() {
        let items: Vec<usize> = (0..100).collect();
        let mut reported = Vec::new();
        let out = in_parallel(&items, |i| i * 2, |n, _| reported.push(n));
        assert_eq!(out, items.iter().map(|i| i * 2).collect::<Vec<_>>());
        assert_eq!(reported, (1..=100).collect::<Vec<_>>());
        assert!(in_parallel(&[] as &[usize], |i| *i, |_, _| {}).is_empty());
    }
}
