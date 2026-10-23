#!/usr/bin/env python3
"""Compile BASIC sources with the real BC.EXE and say which it accepts.

    bcprobe.py BC_DIR FILE.bas...      # BC_DIR holds BC.EXE (llrm-private-tools/qb45)

Runs on the dosrun fork of DOSBox-X (DOSBOX_BIN, else tools/dosbatch's pick):
one booted machine per core, every file a job forked from it, so the start-up
is paid once per core. Prints `ok`, `refused` with BC's first error, or `timeout`.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools" / "dosbatch"))
import dosbatch  # noqa: E402

DOSBOX = os.environ.get("DOSBOX_BIN", str(dosbatch.DOSBOX))
INCLUDES = (".bi", ".inc")


def verdict(out):
    severe = re.search(r"(\d+)\s+Severe", out)
    rows = out.splitlines()
    first = next((f"{rows[i].strip()} / {rows[i + 1].strip()}" for i, l in enumerate(rows[:-1]) if re.match(r"\s*\d{4}\s+\d{4}\s", l)), "")
    if severe is None:
        return "refused " + (rows[-1].strip() if rows else "no output")
    return ("refused " if int(severe.group(1)) else "ok ") + first


def machine(bc_dir, sources):
    """One booted DOSBox-X, one job per source; the verdicts in order."""
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        script = []
        for at, source in enumerate(sources):
            job = work / f"D{at}"
            job.mkdir()
            for sibling in source.parent.iterdir():  # $INCLUDE files beside the source
                if sibling.suffix.lower() in INCLUDES and sibling.is_file():
                    shutil.copy(sibling, job / sibling.name.upper())
            (job / "T.BAS").write_bytes(dosbatch.crlf(source.read_bytes().replace(b"\x1a", b"")))
            script += [f":ms 600000\n:wall 60", f"mount c {work}", f"mount v {bc_dir}", "c:", f"cd D{at}", r"V:\BC T.BAS /O; > O.TXT", "."]
        (work / "job.conf").write_text(dosbatch.CONF)
        (work / "jobs.txt").write_text("\n".join(script) + "\n")
        with open(work / "jobs.txt") as stdin, open(work / "events.txt", "w") as sink:
            subprocess.run([DOSBOX, "-nolog", "-conf", str(work / "job.conf")], stdin=stdin, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, pass_fds=(sink.fileno(),),
                           env={**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSRUN_FD": str(sink.fileno())})
        ends = [json.loads(l) for l in (work / "events.txt").read_text().splitlines() if l.startswith('{"ev":"end"')]
        result = []
        for at in range(len(sources)):
            end = ends[at] if at < len(ends) else {}
            out_file = work / f"D{at}" / "O.TXT"
            if end.get("reason") != "exit" or not out_file.exists():
                result.append("timeout" if end else "no result")
            else:
                result.append(verdict(out_file.read_bytes().decode("latin-1")))
        return result


def main():
    bc_dir, files = Path(sys.argv[1]), [Path(f) for f in sys.argv[2:]]
    cores = os.cpu_count() or 4
    slices = [files[i::cores] for i in range(cores) if files[i::cores]]
    with ThreadPoolExecutor(len(slices)) as pool:
        results = list(pool.map(lambda s: machine(bc_dir, s), slices))
    verdicts = {}
    for chunk, got in zip(slices, results):
        verdicts.update(zip(chunk, got))
    for f in files:
        print(f"{f.name}: {verdicts[f]}".rstrip())


main()
