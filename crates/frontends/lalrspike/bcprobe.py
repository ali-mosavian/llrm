#!/usr/bin/env python3
"""Compile BASIC sources with the real BC.EXE and say which it accepts.

    bcprobe.py [--root DIR] BC_DIR FILE.bas...      # BC_DIR holds BC.EXE (llrm-private-tools/qb45)

Runs on the dosrun fork of DOSBox-X: four booted machines per core, every file
a job forked from one of them, so the start-up is paid once per machine.
Prints `ok`, `refused` with BC's first error, or `timeout`.

With --root the sources are compiled in place from that directory, mounted as
C:, so a `'$INCLUDE: '..\\inc\\x.bi'` finds its file; the files must have CR LF
line ends and 8.3 names. Without it each source is copied to a directory of
its own with the .bi/.inc files beside it.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from tqdm import tqdm

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools" / "dosbatch"))
import dosbatch  # noqa: E402

# DOSBOX_BIN, else a dosbox-x-fork checkout beside the repository or in $HOME, else the cargo build's
FORKS = [Path(__file__).resolve().parents[3].parent / "dosbox-x-fork" / "src" / "dosbox-x", Path.home() / "dosbox-x-fork" / "src" / "dosbox-x"]
DOSBOX = os.environ.get("DOSBOX_BIN") or str(next((f for f in FORKS if f.exists()), dosbatch.DOSBOX))
INCLUDES = (".bi", ".inc")


def verdict(out):
    severe = re.search(r"(\d+)\s+Severe", out)
    rows = out.splitlines()
    first = next((f"{rows[i].strip()} / {rows[i + 1].strip()}" for i, l in enumerate(rows[:-1]) if re.match(r"\s*\d{4}\s+\d{4}\s", l)), "")
    if severe is None:
        return "refused " + (rows[-1].strip() if rows else "no output")
    return ("refused " if int(severe.group(1)) else "ok ") + first


def compiler(bc_dir):
    """BC's path on V:, by the family's layout: QB 4.5 beside the root, PDS 7.1 in BINB, VBDOS in BIN."""
    if (bc_dir / "BINB" / "BC.EXE").exists():
        return "pds71", r"V:\BINB\BC"
    if (bc_dir / "BIN" / "BC.EXE").exists():
        return "vbdos", r"V:\BIN\BC"
    return "qb45", r"V:\BC"


def switches(source):
    """The switches dosbatch gives: error trapping needs /E and /X."""
    text = source.read_bytes().upper()
    return " /E /X" if b"ON ERROR" in text or b"RESUME" in text else ""


def machine(bc_dir, sources, root, progress):
    """One booted DOSBox-X, one job per source; the verdicts in order."""
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        script = []
        for at, source in enumerate(sources):
            head = [":ms 600000", ":wall 60"]
            if root:
                rel = source.resolve().relative_to(root.resolve())
                folder = "\\".join(rel.parts[:-1])
                script += [*head, f"mount c {root}", f"mount d {work}", f"mount v {bc_dir}", "c:", f"cd \\{folder}", rf"{compiler(bc_dir)[1]} {rel.name},NUL,NUL /O{switches(source)}; > D:\O{at}.TXT", "."]
                continue
            job = work / f"D{at}"
            job.mkdir()
            for sibling in source.parent.iterdir():  # $INCLUDE files beside the source
                if sibling.suffix.lower() in INCLUDES and sibling.is_file():
                    shutil.copy(sibling, job / sibling.name.upper())
            (job / "T.BAS").write_bytes(dosbatch.crlf(source.read_bytes().replace(b"\x1a", b"")))
            script += [*head, f"mount c {work}", f"mount v {bc_dir}", "c:", f"cd D{at}", rf"{compiler(bc_dir)[1]} T.BAS /O{switches(source)}; > O.TXT", "."]
        (work / "job.conf").write_text(dosbatch.CONF)
        (work / "jobs.txt").write_text("\n".join(script) + "\n")
        events = work / "events.txt"
        with open(work / "jobs.txt") as stdin, open(events, "w") as sink:
            process = subprocess.Popen([DOSBOX, "-nolog", "-conf", str(work / "job.conf")], stdin=stdin, stdout=subprocess.DEVNULL,
                                       stderr=subprocess.DEVNULL, pass_fds=(sink.fileno(),),
                                       env={**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSRUN_FD": str(sink.fileno())})
            counted = 0
            while process.poll() is None:  # each job's end event moves the bar
                time.sleep(0.1)
                done = events.read_text().count('{"ev":"end"')
                progress(done - counted)
                counted = done
            progress(events.read_text().count('{"ev":"end"') - counted)
        ends = [json.loads(l) for l in events.read_text().splitlines() if l.startswith('{"ev":"end"')]
        result = []
        for at in range(len(sources)):
            end = ends[at] if at < len(ends) else {}
            out_file = work / (f"O{at}.TXT" if root else f"D{at}/O.TXT")
            if os.environ.get("BCTIME"):
                print(f"{sources[at].name}: reason={end.get('reason')} emulated_ms={end.get('ms')} cpu_ms={end.get('cpu_ms')}", file=sys.stderr)
            if end.get("reason") != "exit" or not out_file.exists():
                result.append("timeout" if end else "no result")
            else:
                text = out_file.read_bytes().decode("latin-1")
                if os.environ.get("BCRAW"):
                    print(text, file=sys.stderr)
                result.append(verdict(text))
        return result


def main():
    args = sys.argv[1:]
    root = None
    if args[:1] == ["--root"]:
        root, args = Path(args[1]), args[2:]
    bc_dir, files = Path(args[0]), [Path(f) for f in args[1:]]
    machines = 4 * (os.cpu_count() or 4)  # four booted machines per core
    slices = [files[i::machines] for i in range(machines) if files[i::machines]]
    lock = threading.Lock()
    with tqdm(total=len(files), unit="file", desc="BC", file=sys.stderr) as bar:
        def progress(n):
            with lock:
                bar.update(n)

        with ThreadPoolExecutor(len(slices)) as pool:
            results = list(pool.map(lambda s: machine(bc_dir, s, root, progress), slices))
    verdicts = {}
    for chunk, got in zip(slices, results):
        verdicts.update(zip(chunk, got))
    for f in files:
        print(f"{f}: {verdicts[f]}".rstrip())


main()
