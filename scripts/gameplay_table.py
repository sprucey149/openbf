#!/usr/bin/env python3
"""Generate a PS2 reference table from version_census JSON, without asset payloads.

Usage: python3 scripts/gameplay_table.py CENSUS.json ... --out DIRECTORY

This is a reviewable data export, not a runtime override. One JSON file per game
holds a pool of distinct class records, and per source LVL the (sublevel, ordinal,
pool index) of each definition, so map-local overrides and ordered properties stay
distinct while a side's copies across LVLs cost one record. Properties that only
drive sound, effects, textures, colours or HUD art, and every Label, are left out by
name (PRESENTATION below); the manifest counts them. NM is never converted to a
gameplay default.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re

import extract_lua


# ODF properties the simulation has no use for: what a class sounds and looks like, and the text a player reads.
# Listed by name and matched by the hash the disc stores. A hash nobody can name stays in the table.
PRESENTATION_NAMES = (
    "Label", "VOSound", "VOUnitType", "SoundProperty", "SoundName", "SoundOffset", "SoundPitchDev", "SoundWhenMoving",
    "AISCDriverGetInSound", "AISCFieldFollowSound", "AISCFieldMoveOutSound", "AISCResponseNoSirSound",
    "AISCResponseYesSirSound", "AcquiredTargetSound", "Ambient2Sound", "AmbientSound", "ApproachingTargetSound",
    "BoostSound", "BuildingSound", "CapturedSound", "ChangeModeSound", "ChargeSound", "ChargeSoundPitch", "CloseSound",
    "Cockpit1stPersonSound", "Cockpit3rdPersonSound", "CollisionFoliageSound", "CollisionOtherSound", "CollisionSound",
    "CollisionWaterSound", "DamageRegionSound", "DeathSound", "DeflectSound", "DischargeSound", "DisputeSound",
    "EngineSound", "FireEmptySound", "FireLoopSound", "FireSound", "FireSoundStop", "FleeSound", "Footstep0Sound",
    "Footstep2Sound", "FootstepSound0", "FootstepSound1", "FootstepSound2", "HeardEnemySound", "HidingSound",
    "HitSound", "HurtSound", "HydraulicSound", "HydraulicSoundHeight", "JumpSound", "LandSound", "LockedSound",
    "LostSound", "LowHealthSound", "OpenSound", "OrdnanceSound", "OverheatSound", "OverheatSoundPitch", "PowerupSound",
    "PreparingForDamageSound", "ProneSound", "ReloadSound", "RollSound", "SCDriverGetInSound", "SCFieldFollowSound",
    "SCFieldHoldSound", "SCFieldMoveOutSound", "SCGunnerAllClearSound", "SCGunnerGetInSound", "SCResponseYesSirSound",
    "ShieldSound", "SoldierMusic", "SquatSound", "StandSound", "StartMovementSound", "StopMovementSound",
    "TakeoffSound", "TickSound", "TickSoundPitch", "TurnOffSound", "TurnOnSound", "TurningOffSound",
    "TurretActivateSound", "TurretAmbientSound", "TurretDeactivateSound", "TurretPitchSound", "TurretPitchSoundPitch",
    "TurretStartSound", "TurretYawSound", "TurretYawSoundPitch", "VehicleCollisionSound", "WarmUpSound",
    "WeaponChangeSound", "FoleyFXClass", "FoleyFXGroup",
    "AddHitEffect", "AttachEffect", "ChargeUpEffect", "ChunkSmokeEffect", "ChunkSmokeNodeName", "ChunkTerrainEffect",
    "ChunkTrailEffect", "ContrailEffect", "DamageEffect", "DamageEffectScale", "DeathEffect", "DeflectEffect",
    "ExpireEffect", "FootWaterSplashEffect", "HitEffect", "ImpactEffect", "ImpactEffectRigid", "ImpactEffectShield",
    "ImpactEffectSoft", "ImpactEffectStatic", "ImpactEffectTerrain", "ImpactEffectWater", "JetEffect",
    "LightningEffect", "MuzzleFlashEffect", "OrdnanceEffect", "ShieldEffect", "StompEffect", "ThrustEffect",
    "TrailEffect", "WakeEffect", "WakeWaterSplashEffect", "WaterEffect", "WaterSplashEffect", "effect",
    "IconTexture", "LaserTexture", "LightSaberTexture", "MapTexture", "OverrideTexture", "OverrideTexture2",
    "ScopeTexture", "texture", "decal", "HUDTag", "HUDIndexDisplay", "HudModel",
    "BonusColor", "EndLaserGlowColor", "EndLightColor", "EnemyColor", "FadeLightColor", "FlashColor", "FlashLightColor",
    "FlashLightDuration", "FlashLightRadius", "FlareIntensity", "FriendlyColor", "GlowLength", "HoloLightIntensity",
    "HoloLightRadius", "LaserGlowColor", "LightColor", "LightDuration", "LightOdf", "LightRadius",
    "LightSaberTrailColor", "NeutralColor", "ShadowCaster", "color",
)
PRESENTATION = {extract_lua.fnv1a(n) for n in PRESENTATION_NAMES}


TOKEN = re.compile(r"[A-Za-z0-9_./\\:;<>+@*?=-]*\Z")
IDENTIFIER = re.compile(r"[A-Za-z0-9_./\\:<>+@*?= -]*\Z")
CLASS_KINDS = {"entc", "wpnc", "ordc", "expc"}


def name(value, *, spaces=False):
    """Spaces are allowed only in schema fields known to contain asset names."""
    pattern = IDENTIFIER if spaces else TOKEN
    if not isinstance(value, str) or len(value) > 256 or not pattern.fullmatch(value):
        raise ValueError("invalid identifier in census")
    return value


def integer(value, *, maximum=None):
    if type(value) is not int or value < 0 or (maximum is not None and value > maximum):
        raise ValueError("invalid nonnegative integer in census")
    return value


def numeric(value):
    if value is None or value == "NM":
        return value
    if type(value) not in (int, float):
        raise ValueError("expected numeric census value")
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError("nonfinite number in census")
    return value


def scalar(value):
    """An ODF value is a scalar or flat token/number vector, never an object."""
    if isinstance(value, list):
        if any(isinstance(v, (list, dict)) for v in value):
            raise ValueError("nested ODF property value")
        return [scalar(v) for v in value]
    if isinstance(value, str):
        return name(value)
    if value is None or isinstance(value, bool):
        return value
    return numeric(value)


def scope(value):
    if not isinstance(value, list):
        raise ValueError("scope must be a list of sublevel hashes")
    return [integer(v, maximum=0xFFFFFFFF) for v in value]


def count_or_nm(value):
    return value if value == "NM" else integer(value)


def count_nm(value):
    if isinstance(value, dict):
        return sum(count_nm(v) for v in value.values())
    if isinstance(value, list):
        return sum(count_nm(v) for v in value)
    return int(value == "NM")


def source_file(value):
    value = name(value, spaces=True)
    path = PurePosixPath(value)
    if (not value or path.is_absolute() or ".." in path.parts or "\\" in value
            or re.match(r"^[A-Za-z]:", value)):
        raise ValueError("unsafe relative LVL path")
    return path.as_posix()


def mission_facts(value):
    """Validate the explicit facts_from_calls schema, not arbitrary JSON trees."""
    if value == "NM":
        return value
    if not isinstance(value, dict):
        raise ValueError("mission facts must be an object or NM")

    def identifier(v):
        return name(v, spaces=True)

    def sequence(v, validate):
        if v == "NM":
            return v
        if not isinstance(v, list):
            raise ValueError("expected mission list")
        return [validate(x) for x in v]

    def row(v, validators):
        if v == "NM":
            return v
        if not isinstance(v, list) or len(v) != len(validators):
            raise ValueError("invalid mission record shape")
        return [validate(x) for validate, x in zip(validators, v)]

    def mapping(v, validate, *, capture=False):
        if v == "NM":
            return v
        if not isinstance(v, dict):
            raise ValueError("expected mission map")
        out = {}
        for key, child in v.items():
            if not isinstance(key, str):
                raise ValueError("invalid mission map key")
            team = key.rsplit("/", 1)[-1] if capture else key
            if not re.fullmatch(r"-?[0-9]+", team):
                raise ValueError("invalid mission team index")
            if capture:
                if "/" not in key:
                    raise ValueError("invalid capture key")
                identifier(key.rsplit("/", 1)[0])
            out[key] = validate(child)
        return out

    def boolean(v):
        if v != "NM" and type(v) is not bool:
            raise ValueError("expected mission boolean")
        return v

    def lua_team(v):
        if v == "NM":
            return v
        if type(v) is not int or not 1 <= v <= 255:
            raise ValueError("invalid Lua team number")
        return v

    numeric_fields = {"attacking", "numbots", "ai_view", "ai_snipe", "fly", "bird_count"}
    numeric_maps = {"units", "tickets", "ai_aggr"}
    name_maps = {"names", "heroes"}
    name_lists = {"death_regions", "posts", "killed", "layers"}
    out = {}
    for key, v in value.items():
        if key in numeric_fields:
            out[key] = numeric(v)
        elif key in numeric_maps:
            out[key] = mapping(v, numeric)
        elif key in name_maps:
            out[key] = mapping(v, identifier)
        elif key in name_lists:
            out[key] = sequence(v, identifier)
        elif key == "spawn":
            out[key] = None if v is None else row(v, (numeric, numeric))
        elif key == "classes":
            out[key] = mapping(v, lambda rows: sequence(rows, lambda r: row(r, (identifier, numeric, numeric))))
        elif key == "bird_types":
            out[key] = mapping(v, lambda r: row(r, (numeric, identifier)))
        elif key in ("cps", "ai_damage"):
            out[key] = sequence(v, lambda r: row(r, (identifier, numeric)))
        elif key == "local_team_relations":
            out[key] = sequence(v, lambda r: row(r, (lua_team, lua_team, boolean)))
        elif key == "class_load_order":
            out[key] = sequence(v, lambda r: row(r, (identifier, identifier)))
        elif key == "ai_capture":
            out[key] = mapping(v, boolean, capture=True)
        else:
            raise ValueError("unsupported mission fact field")
    return out


def convert(census):
    if census.get("schema") != 1:
        raise ValueError("unsupported census schema")
    source = census["source"]
    game = source["game"]
    if source["platform"] != "ps2" or game not in ("bf1", "bf2"):
        raise ValueError("reference tables require an identified PS2 census")
    files = {}
    omitted = dropped = 0
    for row in census["entities"]:
        if row["kind"] not in CLASS_KINDS:
            raise ValueError("unsupported class kind")
        file = source_file(row["file"])
        target = files.setdefault(file, {"classes": [], "missions": []})
        props = []
        for key, value in row["properties"]:
            if type(key) is not int or not 0 <= key <= 0xFFFFFFFF:
                raise ValueError("invalid property hash")
            if key in PRESENTATION:
                dropped += 1
                continue
            omitted += bool(count_nm(value))
            props.append([key, scalar(value)])
        target["classes"].append({
            "scope": scope(row["scope"]), "ordinal": integer(row["ordinal"]),
            "kind": row["kind"], "name": name(row["name"], spaces=True),
            "base": name(row["base"], spaces=True), "properties": props,
        })
    for row in census.get("missions", []):
        file = source_file(row["file"])
        target = files.setdefault(file, {"classes": [], "missions": []})
        target["missions"].append({
            "scope": scope(row["scope"]), "name": name(row["name"], spaces=True),
            "ordinal": integer(row.get("ordinal", 0)),
            "unresolved_arguments": count_or_nm(row.get("unresolved_arguments", "NM")),
            "facts": mission_facts(row["facts"]),
        })
    return game, files, omitted, dropped


def encoded(document):
    return (json.dumps(document, separators=(",", ":"), allow_nan=False) + "\n").encode()


def pooled(game, files):
    """One document per game: distinct class records once, then where each one was defined."""
    pool, index, layout = [], {}, {}
    for file, rows in sorted(files.items()):
        entries = []
        for row in rows["classes"]:
            record = {"kind": row["kind"], "name": row["name"], "base": row["base"], "properties": row["properties"]}
            key = json.dumps(record, sort_keys=True)
            if key not in index:
                index[key] = len(pool)
                pool.append(record)
            entries.append([row["scope"], row["ordinal"], index[key]])
        layout[file] = {"classes": entries, "missions": rows["missions"]}
    return {"schema": 2, "game": game, "class_pool": pool, "files": layout}


def generate(inputs, out):
    """Validate every input before writing; refuse stale or foreign output files."""
    prepared = {}
    summaries = []
    games = set()
    for path in inputs:
        raw = Path(path).read_bytes()
        census = json.loads(raw)
        game, files, omitted, dropped = convert(census)
        if game in games:
            raise ValueError("only one reference source per game")
        games.add(game)
        document = pooled(game, files)
        data = encoded(document)
        prepared[f"{game}.json"] = data
        summaries.append({"game": game, "platform": "ps2",
                          "census_sha256": hashlib.sha256(raw).hexdigest(),
                          "table": f"{game}.json", "table_sha256": hashlib.sha256(data).hexdigest(),
                          "source_files": len(files),
                          "class_definitions": sum(len(rows["classes"]) for rows in files.values()),
                          "distinct_class_records": len(document["class_pool"]),
                          "missions": sum(len(rows["missions"]) for rows in files.values()),
                          "presentation_values_left_out": dropped,
                          "omitted_property_values": omitted,
                          "unmeasured_mission_fields": sum(count_nm(m["facts"])
                              for rows in files.values() for m in rows["missions"])})
    manifest = {"schema": 2, "status": "reference_export_not_runtime_override",
                "sources": sorted(summaries, key=lambda s: s["game"])}
    prepared["manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    out = Path(out)
    if out.is_symlink():
        raise ValueError("output must not be a symlink")
    if out.exists():
        actual = {p.relative_to(out).as_posix() for p in out.rglob("*") if p.is_file()}
        if actual - prepared.keys():
            raise ValueError("output has stale or foreign files; use an empty output directory")
        if any(p.is_symlink() for p in out.rglob("*")):
            raise ValueError("output contains a symlink")
    for relative, data in prepared.items():
        target = out / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("census", nargs="+", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        manifest = generate(args.census, args.out)
    except (ValueError, KeyError, OSError, TypeError) as exc:
        parser.exit(2, f"gameplay table: {exc}\n")
    print(json.dumps({s["game"]: s["distinct_class_records"] for s in manifest["sources"]}, sort_keys=True))


if __name__ == "__main__":
    main()
