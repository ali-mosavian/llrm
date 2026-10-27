#!/usr/bin/env python3
"""BC (all three families) on whole program files, each alone.

    bcfiles.py programs.jsonl programs/ results.json     (ids name p{id:05d}.bas)
"""

import json
import os
import sys
from pathlib import Path

HERE = Path(__file__).parent
CORPUS = HERE.parents[2] / "crates/frontends/qb/lalrspike/corpus"
from tqdm import tqdm  # noqa: E402

import bcprobe  # noqa: E402

TOOLS = Path(os.environ['LLRM_PRIVATE_TOOLS'])
FAMILIES = {"qb45": TOOLS / "qb45", "pds71": TOOLS / "pds71", "vbdos": TOOLS / "vbdos"}


def main():
    index, folder, target = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    ids = [json.loads(line)["id"] for line in index.read_text().splitlines()]
    done = json.loads(target.read_text()) if target.exists() else {}
    limit = int(os.environ.get("SLICE", "1200"))
    todo = [i for i in ids if str(i) not in done][:limit]
    print(f"{len(done)} done, {len(todo)} in this slice", file=sys.stderr)
    ids = todo
    files = {folder / f"p{i:05d}.bas": i for i in ids}
    tasks = [(d, f) for f in files for d in FAMILIES.values()]
    os.environ["BCRAW"] = "1"
    with tqdm(total=len(tasks), unit="job", desc="BC", file=sys.stderr, mininterval=10) as bar:
        raw = bcprobe.probe(tasks, None, bar.update)
    results = {i: {family: None for family in FAMILIES} for i in ids}
    for (directory, path), text in raw.items():
        family = bcprobe.compiler(directory)[0]
        i = files[path]
        if text == "timeout" or "Severe" not in text:
            results[i][family] = ["CRASH"]
            continue
        found, loose = bcprobe.errors(text)
        messages = [m.replace("\x07", "").strip() for _, _, _, m in found] + loose
        if not messages and int(__import__("re").search(r"(\d+)\s+Severe", text).group(1)):
            messages = ["UNPLACED"]
        results[i][family] = messages or None
    done.update({str(i): v for i, v in results.items()})
    target.write_text(json.dumps(done))


if __name__ == "__main__":
    main()
