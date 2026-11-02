"""Known absolute mission facts from one fingerprinted correction revision.

Dynamic/unknown facts are never replaced with zero. The regular generator keeps
its PS2 execution evidence where present; an absent required static declaration
makes a table-only import fail clearly.
"""
import copy
import hashlib
import json
from pathlib import Path

TEAM_MAPS = {'units', 'tickets', 'names', 'classes', 'heroes', 'ai_aggr', 'bird_types'}
REQUIRED = {'units', 'tickets', 'names', 'classes'}
SOURCE_SCRIPTS = {}


def unknown(value):
    if isinstance(value, dict):
        return any(unknown(v) for v in value.values())
    if isinstance(value, list):
        return any(unknown(v) for v in value)
    return value == 'NM'


def load(paths, sources=None, *, require_source=True):
    result = {}
    for path in paths:
        document = json.loads(Path(path).read_bytes())
        if document.get('schema') != 2 or document.get('reference') != 'ps2-v1':
            raise ValueError(f'unsupported mission correction revision: {path}')
        game = document.get('game')
        if game not in ('bf1', 'bf2') or game in result:
            raise ValueError('only one correction source per game is allowed')
        if require_source:
            source = (sources or {}).get(game)
            if not source:
                raise ValueError(f'{game}: mission corrections require --gameplay-source {game}=SOURCE')
            from version_census import input_files, walk_resources, cstr, iter_chunks
            source = Path(source)
            if source.is_file() and source.suffix.lower() == '.lvl':
                raw = source.read_bytes()
            else:
                rows = list(input_files(source, {'mission.lvl'}))
                if len(rows) != 1:
                    raise ValueError(f'{game}: expected one base MISSION.LVL for source fingerprint')
                raw = rows[0][1]
            expected = next((row['sha256'] for row in document['files'] if row['file'] == 'mission.lvl'), None)
            if hashlib.sha256(raw).hexdigest() != expected:
                raise ValueError(f'{game}: unknown MISSION.LVL fingerprint; no corrections were applied')
            scripts = {}
            for _scope, tag, payload, _offset in walk_resources(raw):
                if tag == b'scr_':
                    fields = {t: p for t, p, _ in iter_chunks(payload)}
                    if b'NAME' in fields and b'BODY' in fields:
                        scripts.setdefault(cstr(fields[b'NAME']).lower(), fields[b'BODY'])
            SOURCE_SCRIPTS[game] = scripts
        missions = {}
        for file in document['files']:
            for row in file.get('missions', []):
                title = row['name'].lower()
                if title in missions and missions[title] != row['facts']:
                    raise ValueError(f'ambiguous canonical mission {game}/{title}')
                missions[title] = row['facts']
        result[game] = missions
    return result


def overlay(base, target, *, require_complete=False, context='mission'):
    if not isinstance(target, dict):
        raise ValueError(f'{context}: PS2 mission declarations are unmeasured')
    if require_complete:
        for field in REQUIRED:
            if field not in target or unknown(target[field]):
                raise ValueError(f'{context}: canonical {field} is unmeasured; a PS2 mission source is still required')
        for field in ('units', 'tickets', 'names', 'classes'):
            if not all(str(team) in target[field] for team in (1, 2)):
                raise ValueError(f'{context}: canonical {field} omits a playable team')
    out = copy.deepcopy(base)
    for field, value in target.items():
        # The whole known target field replaces the donor, including removals.
        # An NM subtree stays unmeasured and never supplies a default.
        if unknown(value):
            continue
        value = copy.deepcopy(value)
        if field in TEAM_MAPS:
            value = {int(k): v for k, v in value.items()}
        elif field == 'ai_capture':
            value = {(key.rsplit('/', 1)[0], int(key.rsplit('/', 1)[1])): v for key, v in value.items()}
        if field == 'bird_count':
            if not isinstance(value, (int, float)) or value < 0 or int(value) != value:
                raise ValueError(f'{context}: invalid canonical bird count')
            value = int(value)
        if field == 'classes':
            # BF1's omitted class maximum is None; its omitted minimum is NM
            # and is rejected above when a table is the sole gameplay source.
            value = {key: [tuple(row) for row in rows] for key, rows in value.items()}
        out[field] = value
    return out
