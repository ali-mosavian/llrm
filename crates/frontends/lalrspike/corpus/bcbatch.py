#!/usr/bin/env python3
"""Run the catalog through the real BC of all three families, many statements to a program.

    bcbatch.py catalog.jsonl results.json [GROUP...]

results.json: {id: {"qb45": null | [messages], "pds71": ..., "vbdos": ...}}; null is accepted.
"""

import json
import os
import re
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent))
from tqdm import tqdm  # noqa: E402

import bcprobe  # noqa: E402

TOOLS = Path("/home/user/llrm-private-tools")
FAMILIES = {"qb45": TOOLS / "qb45", "pds71": TOOLS / "pds71", "vbdos": TOOLS / "vbdos"}
ISOLATED = re.compile(r"^\s*(DEF|CONST|OPTION|DIM|REDIM|ERASE|SELECT|ON ERROR|ON LOCAL|FOR|DO|WHILE|SYSTEM|END|STOP|RUN|CHAIN)\b", re.I)
BATCH = 24


def build(items):
    """[(file name, source, ids)]"""
    prologue = (HERE / "prologue.bas").read_text()
    epilogue = (HERE / "epilogue.bas").read_text()
    groups, loose = [], []
    for item in items:
        (groups if ISOLATED.match(item["text"]) else loose).append(item)
    chunks = [[item] for item in groups] + [loose[i:i + BATCH] for i in range(0, len(loose), BATCH)]
    out = []
    for n, chunk in enumerate(chunks):
        body = "".join(f"{item['text']} ' #{item['id']}\n" for item in chunk)
        out.append((f"b{n}.bas", prologue + body + epilogue, [item["id"] for item in chunk]))
    return out


def run_round(chunks, prologue, epilogue, work, tag):
    """{chunk index: {family: text}} for one round of BC runs; a chunk is a list of items."""
    files = {}
    for n, chunk in enumerate(chunks):
        path = work / f"{tag}{n}.bas"
        path.write_text(prologue + "".join(f"{item['text']} ' #{item['id']}\n" for item in chunk) + epilogue)
        files[path] = n
    tasks = [(directory, path) for path in files for directory in FAMILIES.values()]
    os.environ["BCRAW"] = "1"
    with tqdm(total=len(tasks), unit="job", desc=f"BC {tag}", file=sys.stderr, mininterval=5) as bar:
        raw = bcprobe.probe(tasks, None, bar.update)
    out = {}
    for (directory, path), text in raw.items():
        out.setdefault(files[path], {})[bcprobe.compiler(directory)[0]] = text
    return out


def complete(text):
    return text != "timeout" and re.search(r"\d+\s+Severe\s+Error", text) is not None


def main():
    catalog, target = Path(sys.argv[1]), Path(sys.argv[2])
    only = set(sys.argv[3:])
    items = [json.loads(line) for line in catalog.read_text().splitlines()]
    if only:
        items = [item for item in items if item["group"] in only]
    prologue = (HERE / "prologue.bas").read_text()
    epilogue = (HERE / "epilogue.bas").read_text()
    by_id = {item["id"]: item for item in items}
    groups = [item for item in items if ISOLATED.match(item["text"])]
    loose = [item for item in items if not ISOLATED.match(item["text"])]
    chunks = [[item] for item in groups] + [loose[i:i + BATCH] for i in range(0, len(loose), BATCH)]
    work = Path(tempfile.mkdtemp(prefix="bcbatch"))
    results = {item["id"]: {family: None for family in FAMILIES} for item in items}
    crashed = {}
    for depth in range(8):
        if not chunks:
            break
        outputs = run_round(chunks, prologue, epilogue, work, f"r{depth}_")
        again = []
        for n, chunk in enumerate(chunks):
            for family in FAMILIES:
                text = outputs[n][family]
                if complete(text):
                    found, loose_errors = bcprobe.errors(text)
                    for _, echoed, _, message in found:
                        m = re.search(r"#(\d+)\s*$", echoed)
                        if m and int(m.group(1)) in by_id:
                            i = int(m.group(1))
                            message = message.replace("\x07", "").strip()
                            results[i][family] = (results[i][family] or []) + [message]
                else:
                    if len(chunk) == 1:
                        results[chunk[0]["id"]][family] = ["CRASH"]
                        crashed[chunk[0]["id"]] = text[-80:] if text != "timeout" else "timeout"
                    else:
                        again.append((family, chunk))
        # chunks any family left incomplete are split and run again for every family (simple: for all)
        retry = {tuple(item["id"] for item in chunk): chunk for _, chunk in again}
        chunks = []
        for chunk in retry.values():
            half = len(chunk) // 2
            chunks += [chunk[:half], chunk[half:]]
        # the families that did complete on a split chunk simply run again: harmless
        # reset the results of the items being rerun so errors are not doubled
        for chunk in chunks:
            for item in chunk:
                results[item["id"]] = {family: None for family in FAMILIES}
    target.write_text(json.dumps(results))
    print(f"{len(crashed)} statements crash BC alone", file=sys.stderr)


if __name__ == "__main__":
    main()
