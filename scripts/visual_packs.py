"""Drop-in presentation pack discovery and source substitutions; never edits a bake."""
import hashlib
from pathlib import Path
import re
import stat

from visual_pack_formats import (STATIC_MODELS, compatible_mesh, compatible_texture,
                                 glb, json_object, ktx2)


class PackError(ValueError):
    pass


def relative(value):
    # POSIX manifest paths on every host, including Windows drives/ADS and aliases.
    if (not isinstance(value, str) or not value or '\\' in value or ':' in value
            or any(ord(c) < 32 or c in '<>"|?*' for c in value) or any(p in ('', '.', '..') or p.endswith((' ', '.'))
            or re.fullmatch(r'(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?', p)
            for p in value.split('/'))):
        raise PackError(f'unsafe relative path {value!r}; use forward slashes without drives, links or traversal')
    return value


def child(root, name, missing=False):
    path = root / relative(name)
    # Permit a symlinked lookup/bake root itself; nothing within a pack may escape.
    if not path.resolve().is_relative_to(root.resolve()):
        raise PackError(f'{path}: link escapes its root')
    if not path.exists() and missing and not path.is_symlink():
        return None
    if not stat.S_ISREG(path.stat().st_mode):
        raise PackError(f'{path}: expected a regular file')
    return path.resolve()


def fields(row, keys, where):
    if not isinstance(row, dict) or set(row) != set(keys):
        raise PackError(f'{where}: expected only {", ".join(keys)}')


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda: file.read(1024*1024), b''):
            result.update(block)
    return result.hexdigest()


def target_kind(name):
    parts = name.split('/')
    if name.endswith('.ktx2') and ('textures' in parts[:-1] or (len(parts) == 3 and parts[1] == 'load')):
        return 'ktx2'
    if name in STATIC_MODELS:
        return 'glb'
    raise PackError(f'{name}: unsupported presentation target; use native textures or an audited static GLB path (see docs/visual-packs.md)')


def discover(lookup, explicit=False):
    """Parse manifests before expensive builds. Non-pack material is reported, never installed."""
    root = Path(lookup).expanduser().absolute()
    report = dict(schema=1, lookup=str(root), explicit=explicit, packs=[], ignored=[], targets=[], warnings=[])
    if not root.exists():
        if explicit:
            raise PackError(f'--visual-packs {root}: folder is missing; create it or omit the option')
        return [], report
    if not root.is_dir():
        raise PackError(f'{root}: visual pack lookup must be a directory containing pack folders')
    packs = []
    try:
        for folder in sorted(root.iterdir()):
            manifest = folder / 'pack.json'
            if folder.is_symlink() or not folder.is_dir() or not (manifest.exists() or manifest.is_symlink()):
                report['ignored'].append(dict(path=folder.name, reason='expected a pack directory with pack.json; archives, previews and tools are not installed'))
                continue
            manifest = child(folder, 'pack.json')
            doc = json_object(manifest.read_text(encoding='utf-8'))
            fields(doc, ('version', 'replacements'), manifest)
            if type(doc['version']) is not int or doc['version'] != 1 or not isinstance(doc['replacements'], list) or not doc['replacements']:
                raise PackError(f'{manifest}: expected version 1 and a nonempty replacements array')
            pack = dict(name=folder.name, root=folder, replacements=[])
            used = {'pack.json'}
            for index, row in enumerate(doc['replacements']):
                where = f'{manifest}: replacements[{index}]'
                fields(row, ('file', 'targets'), where)
                file = relative(row['file'])
                source = child(folder, file)
                if not isinstance(row['targets'], list) or not row['targets']:
                    raise PackError(f'{where}: targets must be a nonempty array')
                targets = []
                for target in row['targets']:
                    fields(target, ('path', 'sha256'), where)
                    name = relative(target['path'])
                    kind = target_kind(name)
                    if not file.endswith('.'+kind):
                        raise PackError(f'{where}: replacement file extension must match target {name}')
                    hashes = target['sha256']
                    if not isinstance(hashes, list) or not hashes or any(not isinstance(h, str) or not re.fullmatch('[0-9a-fA-F]{64}', h) for h in hashes):
                        raise PackError(f'{where}: sha256 must list accepted baked original SHA256 hashes (64 hex characters)')
                    targets.append(dict(path=name, sha256=[h.lower() for h in hashes], kind=kind))
                pack['replacements'].append(dict(file=file, source=source, targets=targets))
                used.add(file)
            # Walk without following directory symlinks. Extra artwork/tools are informational only.
            for path in sorted(folder.rglob('*')):
                if not path.is_dir() and path.relative_to(folder).as_posix() not in used:
                    report['ignored'].append(dict(path=path.relative_to(root).as_posix(), reason='not named by pack.json; not installed'))
            report['packs'].append(dict(name=folder.name, manifest_sha256=digest(manifest)))
            packs.append(pack)
    except (OSError, ValueError, TypeError, KeyError, RuntimeError) as error:
        raise PackError(f'visual packs: {error}; fix or remove the pack folder and run pack.py again') from error
    return packs, report


def resolve(packs, report, baked):
    """Validate all payloads, match originals once per named path, return native writer entries."""
    baked = Path(baked)
    replacements, installed, hashes = [], {}, {}
    try:
        for pack in packs:
            for row in pack['replacements']:
                raw = row['source'].read_bytes()
                replacement_hash = hashlib.sha256(raw).hexdigest()
                parse = ktx2 if row['file'].endswith('.ktx2') else glb
                payload = parse(raw)  # Invalid payloads fail even when no original matches.
                del raw
                for target in row['targets']:
                    name = target['path']
                    stock = child(baked, name, missing=True)
                    if stock is not None and name not in hashes:
                        hashes[name] = digest(stock)
                    actual = hashes.get(name)
                    result = dict(pack=pack['name'], file=row['file'], path=name,
                                  accepted_sha256=target['sha256'], stock_sha256=actual,
                                  replacement_sha256=replacement_hash)
                    if stock is None:
                        result['status'] = 'missing'
                    elif actual not in target['sha256']:
                        result['status'] = 'hash_mismatch'
                    else:
                        # Case-fold conflicts too: installed packs must be portable to case-insensitive hosts.
                        key = name.casefold()
                        if key in installed:
                            raise PackError(f'duplicate matching target {name}: {installed[key]} and {pack["name"]}; remove one pack or mapping')
                        original = parse(stock.read_bytes())
                        if target['kind'] == 'ktx2':
                            compatible_texture(original, payload)
                        else:
                            compatible_mesh(original, payload)
                        installed[key] = pack['name']
                        replacements.append(['assets_baked/'+name, str(row['source'])])
                        result['status'] = 'installed'
                    report['targets'].append(result)
        for pack in packs:
            if not any(t['pack'] == pack['name'] and t['status'] == 'installed' for t in report['targets']):
                report['warnings'].append(f'visual pack {pack["name"]!r} matched no baked originals; check source profile, paths and accepted hashes in .sources.json')
    except (OSError, ValueError, TypeError, KeyError, IndexError, AttributeError, OverflowError, RuntimeError) as error:
        raise PackError(f'visual packs: {pack["name"]}/{row["file"]}: {error}; fix or remove the pack and run pack.py again') from error
    report['counts'] = {status: sum(t['status'] == status for t in report['targets']) for status in ('installed', 'missing', 'hash_mismatch')}
    report['counts'].update(packs=len(packs), originals_hashed=len(hashes), ignored=len(report['ignored']))
    return sorted(replacements)


def summary(report):
    c = report['counts']
    lines = [f'Visual packs: {c["packs"]} found, {c["installed"]} installed, {c["missing"]} missing, '
             f'{c["hash_mismatch"]} hash mismatch; {c["originals_hashed"]} originals hashed.']
    for row in report['ignored'][:5]:
        lines.append(f'  ignored {row["path"]}: {row["reason"]}')
    if len(report['ignored']) > 5:
        lines.append(f'  {len(report["ignored"])-5} more ignored files listed in .sources.json')
    return lines + ['WARNING: '+w for w in report['warnings']]
