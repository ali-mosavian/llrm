#!/usr/bin/env python3
"""Compile BASIC sources with the real BC.EXE under DOSBox and say which it accepts.

    bcprobe.py BC_DIR FILE.bas...      # BC_DIR holds BC.EXE (llrm-private-tools/qb45)

Prints `ok` or `refused` per file with BC's first error line, or `timeout`.
One DOSBox per file, one per core, each killed after 25 s: BC can wait for
input on a bad $INCLUDE.
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path


def probe(bc_dir, source):
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        shutil.copy(bc_dir / "BC.EXE", work / "BC.EXE")
        for sibling in source.parent.iterdir():  # $INCLUDE files beside the source
            if sibling.is_file() and sibling.suffix.lower() in (".bi", ".inc") and len(sibling.stem) <= 8:
                shutil.copy(sibling, work / sibling.name.upper())
        data = source.read_bytes().replace(b"\r\n", b"\n").replace(b"\x1a", b"").replace(b"\n", b"\r\n")
        (work / "T.BAS").write_bytes(data)
        (work / "RUN.BAT").write_bytes(b"@echo off\r\nBC T.BAS /O; > O.TXT\r\necho done > DONE.TXT\r\n")
        conf = f"[sdl]\nwindowresolution=320x200\n[cpu]\ncycles=fixed 20000\n[autoexec]\nmount c {work}\nc:\nRUN.BAT\nexit\n"
        (work / "d.conf").write_text(conf)
        env = dict(os.environ, SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
        subprocess.run(["timeout", "-s", "KILL", "25", "dosbox", "-conf", str(work / "d.conf"), "-noconsole"], env=env, capture_output=True)
        if not (work / "DONE.TXT").exists():
            return "timeout"
        out = (work / "O.TXT").read_bytes().decode("latin-1") if (work / "O.TXT").exists() else ""
        if os.environ.get("BCRAW"):
            print(out)
        severe = re.search(r"(\d+)\s+Severe", out)
        rows = out.splitlines()
        first = next((f"{rows[i].strip()} / {rows[i + 1].strip()}" for i, l in enumerate(rows[:-1]) if re.match(r"\s*\d{4}\s+\d{4}\s", l)), "")
        if severe is None:
            return "refused " + (rows[-1].strip() if rows else "no output")
        return ("refused " if int(severe.group(1)) else "ok ") + first


def main():
    bc_dir, files = Path(sys.argv[1]), [Path(f) for f in sys.argv[2:]]
    with ThreadPoolExecutor(os.cpu_count() or 4) as pool:
        for source, verdict in zip(files, pool.map(lambda f: probe(bc_dir, f), files)):
            print(f"{source.name}: {verdict}".rstrip())


main()
