#!/usr/bin/env python3
"""Archive an inventory's objects as the library, with the reference build's where ours failed.

    library.py OURS_DIR OUT_DIR NAME --wlib WLIB [--reference REF_DIR]

OUT_DIR gets NAME.lib, the stand-alone objects (the ones the makefiles build with `-zld`:
start-up variants and mode switches, linked by name, never searched for) and manifest.json,
which says where every object came from.
"""
import argparse
import json
import shutil
import subprocess
from pathlib import Path


def load(directory):
    return {(r["dir"], r["obj"]): r for r in map(json.loads, open(Path(directory) / "results.jsonl"))}


def build(ours, out, name, wlib, reference=None):
    out = Path(out)
    members, loose = out / "members", out / "standalone"
    for directory in (members, loose):
        shutil.rmtree(directory, ignore_errors=True)
        directory.mkdir(parents=True)
    made = load(ours)
    fill = load(reference) if reference else {}
    manifest = {}
    for key, record in made.items():
        origin = None
        if record["status"] == "ok":
            source, origin = Path(ours) / "obj" / key[0] / f"{key[1]}.obj", f"llrm-c" if record["tool"] == "llrm-c" else "jwasm"
        elif fill.get(key, {}).get("status") == "ok":
            source, origin = Path(reference) / "obj" / key[0] / f"{key[1]}.obj", fill[key]["tool"]
        if origin is None:
            manifest[f"{key[0]}/{key[1]}"] = "missing"
            continue
        target = (loose if record.get("standalone") else members) / f"{key[1]}.obj"
        shutil.copy(source, target)
        manifest[f"{key[0]}/{key[1]}"] = origin + (" standalone" if record.get("standalone") else "")
    lib = (out / f"{name}.lib").resolve()
    lib.unlink(missing_ok=True)
    names = sorted(p.name for p in members.glob("*.obj"))
    (out / "members.cmd").write_text("".join(f"+{n}\n" for n in names))
    subprocess.run([str(wlib), "-q", "-b", "-n", "-fo", str(lib), "@../members.cmd"], cwd=members, check=True)
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1, sort_keys=True))
    return manifest


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("ours", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("name")
    ap.add_argument("--wlib", type=Path, required=True)
    ap.add_argument("--reference", type=Path)
    a = ap.parse_args()
    manifest = build(a.ours, a.out, a.name, a.wlib, a.reference)
    from collections import Counter
    print(dict(Counter(manifest.values())))
