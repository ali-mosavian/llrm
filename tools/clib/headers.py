#!/usr/bin/env python3
"""Generate Open Watcom's DOS headers from the pinned source tree with its own wsplice.

    headers.py --ow SRC --wsplice PATH --out DIR

The installed headers of a release are not the ones the pinned clib sources were written
against; this is `bld/hdr/dos/makefile`'s rule for each `hdos` line of `hfiles.mif`.
"""
import argparse
import re
import shutil
import subprocess
from pathlib import Path


def generate(ow, wsplice, out):
    """Headers into `out`; `bld/hdr` is copied to a scratch tree first: wsplice reads a generated
    `crwatpro.gsp` (the copyright year spliced into `crwatpro.sp`) from its include path."""
    work = out.parent / (out.name + ".hdr")
    if work.exists():
        shutil.rmtree(work)
    shutil.copytree(ow / "bld" / "hdr", work)
    hdr = work
    subprocess.run(["awk", "-f", str(hdr / "cpyyear.awk"), "-v", "CPYYEAR=2026", "-v", "OUTFILE=crwatpro.gsp",
                    str(hdr / "crwatpro.sp")], cwd=hdr, check=True)
    made = 0
    for line in (hdr / "hfiles.mif").read_text().splitlines():
        parts = line.split()
        if len(parts) < 3 or parts[0] != "!inject" or "hdos" not in parts[2:]:
            continue
        rel = parts[1][len("h/"):]
        stem = rel[: -len(".h")] if rel.endswith(".h") else rel
        for src in (hdr / "watcom" / "dos" / f"{stem}.mh", hdr / "watcom" / f"{stem}.mh"):
            if src.exists():
                break
        else:
            continue
        only = {"ctype", "inttypes", "locale", "math", "signal", "stdarg", "stdbool", "stddef", "stdint", "stdio",
                "stdlib", "string", "time", "wctype", "wchar"}
        extra = ["-kCONLY"] if stem in only else []
        target = out / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run([str(wsplice), "-u", "-i..", "-kDOS", *extra, "../common.sp", str(src), str(target)],
                       cwd=hdr / "dos", check=True, capture_output=True)
        made += 1
    return made


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--ow", type=Path, required=True)
    ap.add_argument("--wsplice", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    print(generate(a.ow, a.wsplice, a.out), "headers")
