#!/usr/bin/env python3
"""Read the authored fog and scene ranges directly from munged sky configs.

Only selected named numeric fields are returned. Values use the same DATA slot
layout as mungefs::config; absent fields stay absent rather than becoming zero.
"""
import math
import struct

import extract_lua
import version_geometry as geometry

SKY_FIELDS = {
    "fog_color": "FogColor", "fog_range": "FogRange", "near_scene_range": "NearSceneRange",
    "far_scene_range": "FarSceneRange", "ambient_color": "AmbientColor",
    "character_ambient_color": "CharacterAmbientColor", "vehicle_ambient_color": "VehicleAmbientColor",
}
LOWRES_FIELDS = {
    "fog_near": "FogNear", "fog_far": "FogFar", "fog_color": "FogColor", "max_distance": "MaxDistance",
}


def config_tree(payload, depth=0):
    if depth > 64:
        raise ValueError("sky config nesting limit")
    nodes = []
    for tag, body in geometry.chunks(payload):
        if tag == b"DATA":
            if len(body) < 5:
                raise ValueError("short sky DATA")
            key, argc = struct.unpack_from("<IB", body)
            end = 5 + 4 * argc
            if len(body) < end:
                raise ValueError("truncated sky arguments")
            slots = struct.unpack_from("<" + "I" * argc, body, 5)
            values = list(struct.unpack_from("<" + "f" * argc, body, 5))
            blob = b""
            if len(body) >= end + 4:
                length = struct.unpack_from("<I", body, end)[0]
                if end + 4 + length > len(body):
                    raise ValueError("truncated sky string blob")
                blob = body[end + 4:end + 4 + length]
            # The string offset is relative to the argument slots, as in the
            # production reader; string slots must never look like fog values.
            for i, slot in enumerate(slots):
                if 0 <= slot - 4 * argc < len(blob):
                    values[i] = None
            nodes.append({"hash": key, "values": values, "children": []})
        elif tag == b"SCOP":
            if not nodes:
                raise ValueError("sky SCOP without DATA")
            nodes[-1]["children"] = config_tree(body, depth + 1)
    return nodes


def selected(nodes, fields):
    wanted = {extract_lua.fnv1a(label): name for name, label in fields.items()}
    out = {}
    for node in nodes:
        if node["hash"] not in wanted:
            continue
        field = wanted[node["hash"]]
        values = node["values"]
        if not 1 <= len(values) <= 16 or any(v is None or not math.isfinite(v) for v in values):
            raise ValueError(f"invalid numeric sky field {field}")
        out[field] = values[0] if len(values) == 1 else values
    return out


def parse(payload):
    raw_name = geometry.child(payload, b"NAME", b"")
    name = (f"hash_{int.from_bytes(raw_name, 'little'):08x}" if len(raw_name) == 4
            else geometry.name(raw_name))
    out = {"name": name, "sky_info": {}, "low_res_terrain": {}}
    for node in config_tree(payload):
        if node["hash"] == extract_lua.fnv1a("SkyInfo"):
            out["sky_info"] = selected(node["children"], SKY_FIELDS)
        elif node["hash"] == extract_lua.fnv1a("DomeInfo"):
            for child in node["children"]:
                if child["hash"] == extract_lua.fnv1a("LowResTerrain"):
                    out["low_res_terrain"] = selected(child["children"], LOWRES_FIELDS)
    return out
