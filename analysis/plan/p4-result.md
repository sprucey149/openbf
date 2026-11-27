# P4 result: one mod-summary builder, one cache-update helper, one card header (2026-11-27)

## What changed
- `engine/crates/mungefs/src/modscan.rs`: the quick and full inspections share
  `base_discovered` (folder name, map summaries, custom sides, base type and
  compatibility, AddonMissions capability), `map_summaries`, `compatibility`
  and `scan_source_lvl` (the serial and parallel LVL scans). The full
  inspection reads `openbf_mod.ron` once: its `replacements` feed the resolver
  and the rest is merged at the end (was two parses per mod).
- `engine/crates/launcher/src/mods_worker.rs`: `WorkerCache::deep_context`
  replaces the two catalog/facts setup blocks, `WorkerCache::remember` the
  three mod-index inserts, and `ready_cache_notes` the three ready-cache
  warning/unsupported reads. The re-reads of the cache metadata after the
  status check are kept (same file reads as before). The Retarget logic is
  untouched.
- `engine/crates/launcher/src/ui_mods.rs`: `draw_card_header` replaces the
  duplicated header code of the mod card and the extension card.
- Net: 269 insertions, 348 deletions (-79 lines) across the three files.

## Tests
- `cargo test -p mungefs modscan`: 0 tests match the filter (mungefs has no
  modscan tests); the inspections are covered by the next two.
- `cargo test -p launcher mods`: 9 passed, 1 ignored (needs installed addon data).
- `cargo test -p bake mods`: 6 passed (includes the real ABR addon conversion).
- Equivalence: `bake mod-inspect` and `bake mod-convert` built from main and
  from this branch on three local mods (HF5 and BS1 extracted from
  community_mods, plus a synthetic side pack with an `openbf_mod.ron` that sets
  name/version/author/dependencies and an `ignore` replacement). Inspect RON,
  convert logs and the data dirs (caches) are byte-identical (`diff -r`).
  The `ignore` replacement shows as `Inactive("ignored by openbf_mod.ron")`,
  so the single parse still feeds the resolver. The conversions fail the same
  way on both builds (no stock catalog in the local bake; BS1 over the size
  limit; HF5 has an unknown PC side source), so no converted cache directory
  could be compared.
- `cargo clippy -p mungefs -p launcher --all-targets`: no new warnings in the
  changed code. `-D warnings` does not pass on main either: the current
  toolchain flags `extensions` (registry.rs if_same_then_else, a sort_by) and
  older code in mungefs/launcher (chunks_exact, `scan_mods_with_cache` type,
  `run_convert_queue` arguments).
- One-player gate (release `game`, after merging main): every row matches.

## Deviations
- About -79 lines rather than -110.
- Clippy acceptance run without `-D warnings` because of the warnings above.
- No Mods-page scan of an installed mod set (none on this machine); the bake
  inspect/convert comparison stands in for it.

## Left
- The shared safe-walk and the Arc snapshot (excluded from this packet).
- The pre-existing clippy warnings above.
