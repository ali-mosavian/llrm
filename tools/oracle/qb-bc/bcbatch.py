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
CORPUS = HERE.parents[2] / "crates/frontends/qb/lalrspike/corpus"
from tqdm import tqdm  # noqa: E402

import bcprobe  # noqa: E402

TOOLS = Path(os.environ['LLRM_PRIVATE_TOOLS'])
FAMILIES = {"qb45": TOOLS / "qb45", "pds71": TOOLS / "pds71", "vbdos": TOOLS / "vbdos"}
ISOLATED = re.compile(r"^\s*(DEF|CONST|OPTION|DIM|REDIM|ERASE|SELECT|ON ERROR|ON LOCAL|FOR|DO|WHILE|SYSTEM|END|STOP|RUN|CHAIN)\b", re.I)
BATCH = int(os.environ.get("BCBATCH", "1"))


def build(items):
    """[(file name, source, ids)]"""
    prologue = (CORPUS / "prologue.bas").read_text()
    epilogue = (CORPUS / "epilogue.bas").read_text()
    groups, loose = [], []
    for item in items:
        (groups if ISOLATED.match(item["text"]) else loose).append(item)
    chunks = [[item] for item in groups] + [loose[i:i + BATCH] for i in range(0, len(loose), BATCH)]
    out = []
    for n, chunk in enumerate(chunks):
        body = "".join(source_line(item, chunk) for item in chunk)
        out.append((f"b{n}.bas", prologue + body + epilogue, [item["id"] for item in chunk]))
    return out


def run_round(chunks, prologue, epilogue, work, tag):
    """{chunk index: {family: text}} for one round of BC runs; a chunk is a list of items."""
    files = {}
    for n, chunk in enumerate(chunks):
        path = work / f"{tag}{n}.bas"
        path.write_text(prologue + "".join(source_line(item, chunk) for item in chunk) + epilogue)
        files[path] = n
    tasks = [(directory, path) for path in files for directory in FAMILIES.values()]
    os.environ["BCRAW"] = "1"
    with tqdm(total=len(tasks), unit="job", desc=f"BC {tag}", file=sys.stderr, mininterval=5) as bar:
        raw = bcprobe.probe(tasks, None, bar.update)
    out = {}
    for (directory, path), text in raw.items():
        out.setdefault(files[path], {})[bcprobe.compiler(directory)[0]] = text
    return out


def source_line(item, chunk):
    """One catalog statement; a lone one carries no ' #id comment (a comment after a dangling comma changes VBDOS's verdict)."""
    tag = "" if len(chunk) == 1 else f" ' #{item['id']}"
    return f"{item['text']}{tag}\n"


def complete(text):
    return text != "timeout" and re.search(r"\d+\s+Severe\s+Error", text) is not None


def run_items(items, tag):
    """{id: {family: [messages] | None}} for one pass over the items."""
    prologue = (CORPUS / "prologue.bas").read_text()
    epilogue = (CORPUS / "epilogue.bas").read_text()
    by_id = {item["id"]: item for item in items}
    groups = [item for item in items if ISOLATED.match(item["text"])]
    loose = [item for item in items if not ISOLATED.match(item["text"])]
    chunks = [[item] for item in groups] + [loose[i:i + BATCH] for i in range(0, len(loose), BATCH)]
    work = Path(tempfile.mkdtemp(prefix="bcbatch"))
    results = {item["id"]: {family: None for family in FAMILIES} for item in items}
    for depth in range(8):
        if not chunks:
            break
        outputs = run_round(chunks, prologue, epilogue, work, f"{tag}r{depth}_")
        again = {}
        for n, chunk in enumerate(chunks):
            for family in FAMILIES:
                text = outputs[n][family]
                if complete(text):
                    found, stray = bcprobe.errors(text)
                    if len(chunk) == 1 and stray:
                        i = chunk[0]["id"]
                        results[i][family] = (results[i][family] or []) + stray
                    for _, echoed, _, message in found:
                        m = re.search(r"#(\d+)\s*$", echoed)
                        if len(chunk) == 1:
                            i = chunk[0]["id"]
                            message = message.replace("\x07", "").strip()
                            results[i][family] = (results[i][family] or []) + [message]
                        elif m and int(m.group(1)) in by_id:
                            i = int(m.group(1))
                            message = message.replace("\x07", "").strip()
                            results[i][family] = (results[i][family] or []) + [message]
                elif len(chunk) == 1:
                    results[chunk[0]["id"]][family] = ["CRASH"]
                else:
                    again[tuple(item["id"] for item in chunk)] = chunk
        chunks = []
        for chunk in again.values():
            half = len(chunk) // 2
            chunks += [chunk[:half], chunk[half:]]
        for chunk in chunks:
            for item in chunk:
                results[item["id"]] = {family: None for family in FAMILIES}
    return results


def main():
    catalog, target = Path(sys.argv[1]), Path(sys.argv[2])
    only = set(sys.argv[3:])
    items = [json.loads(line) for line in catalog.read_text().splitlines()]
    if only:
        items = [item for item in items if item["group"] in only]
    final = {item["id"]: {family: None for family in FAMILIES} for item in items}
    active = items
    # BC finds some errors only in its second pass, which does not run while the first pass has
    # errors: take out what was refused and look at the rest again until nothing new is refused.
    for round_number in range(1 if BATCH == 1 else 8):
        results = run_items(active, f"p{round_number}")
        refused = set()
        for i, per_family in results.items():
            if any(per_family.values()):
                refused.add(i)
                for family, messages in per_family.items():
                    if messages and final[i][family] is None:
                        final[i][family] = messages
        # a statement refused by one family only stays active for the others
        still = []
        for item in active:
            i = item["id"]
            if i not in refused or not all(results[i][family] for family in FAMILIES):
                still.append(item)
        print(f"round {round_number}: {len(refused)} refused of {len(active)}", file=sys.stderr)
        if not refused:
            break
        # items refused by all families leave; the others stay for the next round
        active = [item for item in still if not all(final[item["id"]][family] for family in FAMILIES)]
    target.write_text(json.dumps(final))


if __name__ == "__main__":
    main()
