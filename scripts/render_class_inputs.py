"""Read the class inputs governing a placed model's lifetime and other consumers."""
import math

from world_fingerprint import digest, read_ron


def fnv(name):
    value = 0x811c9dc5
    for byte in name.encode():
        value = ((value ^ (byte | 0x20)) * 0x01000193) & 0xffffffff
    return value


HEALTH_LIFETIME_KEYS = {fnv(name): name for name in
                        ('MaxHealth', 'LifeSpan', 'LifeTime', 'RespawnTime', 'SpawnDelay', 'IsDestructible', 'Destructible')}
VISIBILITY_KEYS = {fnv(name): name for name in
                   ('DestroyedGeometryName', 'DamagedGeometryName', 'DamageGeometryName', 'DeadGeometryName',
                    'UnbuiltGeometryName', 'AnimationName', 'Animation', 'AnimationBank', 'AnimationTrigger',
                    'IdleAnimation', 'DamageAnimation', 'DeathAnimation', 'DestroyAnimation')}


class ClassInputs:
    """Conservative ClassDb chain inventory with duplicate-definition checks.

    The runtime loads map classes before shared side/common classes. Names are
    documented as unique, but borrowing a neighbor requires that fact to hold
    in the actual input: conflicting definitions remain uncertified. Inspecting
    all game class files is conservative across the authored faction choices.
    """
    def __init__(self, assets, folder, game, reader=read_ron):
        self.definitions, records = {}, []
        paths = [folder / 'classes.ron', *sorted((assets / 'classes').glob(game + '_*.ron'))]
        for path in paths:
            if not path.is_file():
                continue
            rows = reader(path)
            if not isinstance(rows, list):
                raise ValueError(f'invalid class dependency file: {path}')
            records.append((str(path.relative_to(assets)), rows))
            for row in rows:
                if not isinstance(row, dict) or not isinstance(row.get('name'), str) or not isinstance(row.get('base'), str):
                    raise ValueError(f'invalid class dependency row: {path}')
                self.definitions.setdefault(row['name'].lower(), []).append(row)
        if not records:
            raise ValueError('no raw class dependency inventory')
        self.sha256 = digest(records)
        self.files = len(records)

    def chain(self, name):
        seen, chain = set(), []
        while name.lower() in self.definitions:
            name = name.lower()
            if name in seen or len(chain) >= 16:
                raise ValueError('cyclic or overlong class dependency chain')
            seen.add(name)
            candidates = self.definitions[name]
            signatures = {digest(value) for value in candidates}
            if len(signatures) != 1:
                raise ValueError(f'ambiguous class dependency definition: {name}')
            row = candidates[0]
            chain.append(row)
            name = row['base']
        return chain, name.lower()

    def static_lifetime(self, names):
        chains = []
        for name in dict.fromkeys(name.lower() for name in names):
            chain, engine_base = self.chain(name)
            if not chain:
                return dict(eligible=False, reason=f'raw class missing: {name}')
            chains.extend(chain)
            if engine_base not in ('prop', 'building'):
                return dict(eligible=False, reason=f'runtime engine base is {engine_base}, not a static prop/building')
        for row in chains:
            for key, text in row.get('props', []):
                key = int(key)
                if key in HEALTH_LIFETIME_KEYS:
                    try:
                        number = float(str(text).strip())
                    except ValueError:
                        return dict(eligible=False, reason=f'unresolved {HEALTH_LIFETIME_KEYS[key]} visibility dependency')
                    if not math.isfinite(number) or number > 0:
                        return dict(eligible=False, reason=f'authored {HEALTH_LIFETIME_KEYS[key]} requires lifetime/visibility proof')
                if key in VISIBILITY_KEYS and str(text).strip().lower() not in ('', 'none', 'clear'):
                    return dict(eligible=False, reason=f'authored {VISIBILITY_KEYS[key]} requires lifetime/visibility proof')
        return dict(eligible=True, reason='complete unambiguous static class chain; no authored lifetime/animation replacement',
                    class_inputs_sha256=self.sha256, class_files=self.files)

    def missing_direct_class_consumers(self, name):
        """Whether a missing ManifestClass also avoids raw-class spawned art.

        vehicle_pads::add_turrets and roster::add_stations require their dynamic
        engine bases. A raw prop/building/commandpost, or no raw definition,
        reaches neither branch. MapLayout's post parser is independent of this
        lookup; its full instance/region/path inputs must be checked separately.
        """
        chain, base = self.chain(name)
        skipped = not chain or base in ('prop', 'building', 'commandpost')
        return dict(skip=skipped, engine_base=base if chain else None, class_chain_sha256=digest(chain),
                    class_inputs_sha256=self.sha256, class_files=self.files,
                    reason='no raw class or static/post engine base; station/turret raw-class consumers skip'
                    if skipped else 'raw class has a dynamic consumer requiring its own alignment proof')
