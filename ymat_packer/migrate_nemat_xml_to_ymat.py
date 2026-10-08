#!/usr/bin/env python3
from __future__ import annotations
import argparse, json
from pathlib import Path
import xml.etree.ElementTree as ET

def parse_bool(value: str | None, default=False) -> bool:
    if value is None:
        return default
    return value.strip().lower() in {"1","true","yes","on"}

def parse_value(kind: str, raw: str):
    kind = kind.strip().lower()
    raw = raw.strip()
    if kind == "float":
        return float(raw)
    if kind == "int":
        return int(raw, 0)
    if kind == "bool":
        return parse_bool(raw)
    if kind in {"float2","float3","float4","color"}:
        parts = [float(part.strip()) for part in raw.split(",") if part.strip()]
        expected = {"float2":2,"float3":3,"float4":4,"color":4}[kind]
        if len(parts) != expected:
            raise ValueError(f"{kind} requires {expected} components, got {len(parts)}: {raw!r}")
        return parts
    return raw

def convert(path: Path) -> dict:
    root = ET.parse(path).getroot()
    if root.tag != "NematMaterialLibrary":
        raise ValueError(f"{path}: expected NematMaterialLibrary, got {root.tag}")
    materials = []
    for node in root.findall("./Material"):
        name = (node.get("name") or "").strip()
        shader = (node.get("shader") or "").strip()
        if not name or not shader:
            raise ValueError(f"{path}: material requires name/shader")
        surface = node.find("./Surface")
        blend = "opaque"
        two_sided = False
        alpha_cutoff = None
        if surface is not None:
            blend = (surface.get("blend") or "opaque").strip().lower()
            two_sided = parse_bool(surface.get("two_sided"), False)
            if surface.get("alpha_cutoff"):
                alpha_cutoff = float(surface.get("alpha_cutoff"))
        textures = []
        textures_node = node.find("./Textures")
        if textures_node is not None:
            for tex in textures_node.findall("./Texture"):
                slot = (tex.get("slot") or "").strip()
                ref = (tex.get("ref") or "").strip()
                if not slot or not ref:
                    raise ValueError(f"{path}: material {name}: texture requires slot/ref")
                textures.append({"slot": slot, "ref": ref, "required": True})
        params = []
        params_node = node.find("./Params")
        if params_node is not None:
            for param in params_node.findall("./Param"):
                pname = (param.get("name") or "").strip()
                ptype = (param.get("type") or "").strip()
                raw = param.get("value")
                if not pname or not ptype or raw is None:
                    raise ValueError(f"{path}: material {name}: param requires name/type/value")
                params.append({"name": pname, "type": ptype, "value": parse_value(ptype, raw)})
        material = {
            "name": name,
            "shader": shader,
            "surface_domain": "surface",
            "shading_model": "pbr",
            "blend": blend,
            "two_sided": two_sided,
            "textures": textures,
            "params": params,
        }
        if alpha_cutoff is not None:
            material["alpha_cutoff"] = alpha_cutoff
        materials.append(material)
    if not materials:
        raise ValueError(f"{path}: no materials")
    return {"schema": "northstar.ymat.v1", "materials": materials}

def main() -> int:
    ap = argparse.ArgumentParser(description="One-time NEMAT XMLtype -> YMAT JSON migration")
    ap.add_argument("input", type=Path)
    ap.add_argument("output", type=Path)
    ns = ap.parse_args()
    doc = convert(ns.input)
    ns.output.parent.mkdir(parents=True, exist_ok=True)
    ns.output.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print(f"YMAT_MIGRATE_OK materials={len(doc['materials'])} output={ns.output}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
