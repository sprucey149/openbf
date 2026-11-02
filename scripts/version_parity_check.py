#!/usr/bin/env python3
"""Compare independently baked source combinations and their seeded headless rounds.

  version_parity_check.py --case ps2=ASSETS=MISSIONS --case pc=ASSETS=MISSIONS \
      --map bf1_nab1 --map bf2_yav1 --round-bin ROUND_TO_OUTCOME --out receipt.json

Every case must contain each selected map and its source-specific common/side
assets. No shared bake is modified. Nonzero exit means a difference, missing
input, fallback roster, failed round, or a round that did not finish. Hash equality
alone never labels an unmeasured input or unfinished round identical.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

import world_fingerprint


def run_round(binary, assets, missions, key, ticks, seed, log_dir):
    env = {k: v for k, v in os.environ.items() if not k.startswith(('SWEEP_', 'BLACKBOX_', 'SLOT0_', 'ROUND_'))}
    env.update(SWEEP_ASSET_DIR=str(assets.resolve()), SWEEP_MISSION_DIR=str(missions.resolve()),
               SWEEP_THREADS='1', SWEEP_PROXY='0:good', SWEEP_HASH_EVERY='1', OPENBF_DEV='0')
    result = subprocess.run([str(binary.resolve()), str(ticks), str(seed), key], env=env,
                            text=True, capture_output=True, timeout=300)
    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / (key + '.stdout')).write_text(result.stdout)
    (log_dir / (key + '.stderr')).write_text(result.stderr)
    hashes = re.findall(r'\bhash ([0-9a-f]{16})\b', result.stdout)
    if result.returncode or len(hashes) != 1:
        raise ValueError(f'round failed/ambiguous hash (exit {result.returncode}); see {log_dir / key}')
    if any(s in result.stdout for s in ('LOAD FAILED', 'rosters unavailable', 'built-in troopers', 'common assets unreadable', 'NOTE: no baked common_')):
        raise ValueError(f'round used missing gameplay/fallback inputs; see {log_dir / key}')
    finished = ' wins at tick ' in result.stdout
    checkpoints = re.findall(r'checkpoint \S+ (\d+) ([0-9a-f]{16})', result.stdout)
    return {'hash': hashes[0], 'checkpoints': checkpoints, 'finished': finished, 'exit_code': result.returncode}


def compare(cases, maps, round_bin, out, ticks=36000, seed=42):
    rows = []
    for label, assets, missions in cases:
        for key in maps:
            row = {'source': label, 'map': key}
            try:
                identity = world_fingerprint.fingerprint(assets, missions, key)
                row['world'] = identity
                target = assets / key / 'world-fingerprint.json'
                target.write_text(json.dumps(identity, indent=2, sort_keys=True) + '\n')
                row['round'] = run_round(round_bin, assets, missions, key, ticks, seed, out.parent / 'rounds' / label)
            except (ValueError, OSError, KeyError, IndexError, subprocess.TimeoutExpired) as exc:
                row['error'] = str(exc)
            rows.append(row)
    for key in maps:
        selected = [r for r in rows if r['map'] == key]
        reference = selected[0]
        for row in selected:
            if 'error' in row:
                row['status'] = 'unmeasured'
            elif 'world' not in reference or 'round' not in reference:
                row['status'] = 'reference_failed'
            else:
                row['different_components'] = [k for k, h in row['world']['components'].items()
                                               if h != reference['world']['components'].get(k)]
                row['outside_terrain_differs'] = row['world']['terrain_all_sha256'] != reference['world']['terrain_all_sha256']
                row['sim_hash_identical'] = (row['round']['hash'] == reference['round']['hash']
                                             and row['round']['checkpoints'] == reference['round']['checkpoints'])
                row['status'] = ('identical' if row['world'].get('complete', False) and reference['world'].get('complete', False)
                                 and not row['different_components'] and row['sim_hash_identical']
                                 and row['round']['finished'] and reference['round']['finished'] else 'differs_or_unfinished')
    receipt = {'schema': 1, 'reference': cases[0][0], 'ticks': ticks, 'seed': seed,
               'scenario': 'headless_conquest_proxy_0_good', 'hash_every': 1, 'maps': rows}
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', action='append', required=True, metavar='NAME=ASSETS=MISSIONS')
    parser.add_argument('--map', action='append', required=True)
    parser.add_argument('--round-bin', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--ticks', type=int, default=36000)
    parser.add_argument('--seed', type=int, default=42)
    args = parser.parse_args()
    try:
        cases = [(name, Path(assets), Path(missions)) for name, assets, missions in (s.split('=', 2) for s in args.case)]
        if len({name for name, _, _ in cases}) != len(cases) or any(not re.fullmatch('[A-Za-z0-9_-]+', name) for name, _, _ in cases):
            raise ValueError('case names must be unique path-safe names')
        receipt = compare(cases, args.map, args.round_bin, args.out, args.ticks, args.seed)
    except (ValueError, OSError) as exc:
        parser.exit(2, f'version parity: {exc}\n')
    for row in receipt['maps']:
        print(f"{row['source']} {row['map']}: {row['status']} {row.get('different_components', row.get('error', ''))}")
    return 0 if all(r['status'] == 'identical' for r in receipt['maps']) else 1


if __name__ == '__main__':
    raise SystemExit(main())
