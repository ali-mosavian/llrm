#!/usr/bin/env python3
"""Compile BASIC sources with the real BC.EXE and say which it accepts.

    bcprobe.py [--root DIR] BC_DIR[,BC_DIR...] FILE.bas...

BC_DIR holds BC.EXE (llrm-private-tools/qb45, /pds71, /vbdos). Runs on the
dosrun fork of DOSBox-X: four booted machines per core. The work is one queue
of (compiler, file) pairs that every machine takes from as it frees up, so a
slow compile never leaves a core idle, and all the compilers run at once. A job
is `:nosymbols` (the fork otherwise rescans RAM for a protected-mode compiler's
segments, seconds a job for PDS 7.1) and has a wall limit of its own.

With --root the sources are compiled in place from that directory, mounted as
C:, so a `'$INCLUDE: '..\\inc\\x.bi'` finds its file; the files must have CR LF
line ends and 8.3 names. Without it each source is copied to a directory of
its own with the .bi/.inc files beside it.

Prints `FAMILY<TAB>FILE: ok|refused ...|timeout` per pair; BCTIME=1 adds each
job's emulated and CPU time on stderr.
"""

import json
import os
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

from tqdm import tqdm

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools" / "dosbatch"))
import dosbatch  # noqa: E402

# DOSBOX_BIN, else a dosbox-x-fork checkout beside the repository or in $HOME, else the cargo build's
FORKS = [Path(__file__).resolve().parents[3].parent / "dosbox-x-fork" / "src" / "dosbox-x", Path.home() / "dosbox-x-fork" / "src" / "dosbox-x"]
DOSBOX = os.environ.get("DOSBOX_BIN") or str(next((f for f in FORKS if f.exists()), dosbatch.DOSBOX))
INCLUDES = (".bi", ".inc")
JOB_SECONDS = 30  # host seconds a compile may take, in the fork's own clock


def verdict(out):
    severe = re.search(r"(\d+)\s+Severe", out)
    rows = out.splitlines()
    first = next((f"{rows[i].strip()} / {rows[i + 1].strip()}" for i, l in enumerate(rows[:-1]) if re.match(r"\s*\d{4}\s+\d{4}\s", l)), "")
    if severe is None:
        return "refused " + (rows[-1].strip() if rows else "no output")
    return ("refused " if int(severe.group(1)) else "ok ") + first


def errors(out):
    """Every error BC listed: (code, source text, caret column, message), and the loose ones ("Label not defined: X")."""
    rows = out.splitlines()
    found, loose = [], []
    for i, row in enumerate(rows):
        match = re.match(r"\s*([0-9A-F]{4})\s+([0-9A-F]{4})\s{1,4}(.*)$", row)
        if match and i + 1 < len(rows) and "^" in rows[i + 1]:
            caret = rows[i + 1].index("^")
            found.append((match.group(1), match.group(3).rstrip(), caret, rows[i + 1][caret + 1:].strip()))
        elif re.match(r"^[A-Z][A-Za-z ]+: ", row.lstrip("\x07")) and not row.startswith("Microsoft"):
            loose.append(row.replace("\x07", "").strip())
    return found, loose


def compiler(bc_dir):
    """BC's path on V:, by the family's layout: QB 4.5 beside the root, PDS 7.1 in BINB, VBDOS in BIN."""
    if (bc_dir / "BINB" / "BC.EXE").exists():
        return "pds71", r"V:\BINB\BC"
    if (bc_dir / "BIN" / "BC.EXE").exists():
        return "vbdos", r"V:\BIN\BC"
    return "qb45", r"V:\BC"


def switches(source):
    """The switches dosbatch gives: error trapping needs /E and /X; event trapping /W."""
    text = source.read_bytes().upper()
    return (" /E /X" if b"ON ERROR" in text or b"RESUME" in text else "") + (" /W" if b"ON " in text or b"KEY" in text or b"TIMER" in text or b"PLAY" in text or b"PEN" in text or b"STRIG" in text or b"COM(" in text else "")


class Machine:
    """One booted DOSBox-X that takes a job at a time on its standard input."""

    def __init__(self):
        self.work = Path(tempfile.mkdtemp())
        (self.work / "job.conf").write_text(dosbatch.CONF)
        self.events = self.work / "events.txt"
        self.jobs = 0
        self.process = None
        self.start()

    def start(self):
        sink = open(self.events, "w")
        self.process = subprocess.Popen([DOSBOX, "-nolog", "-conf", str(self.work / "job.conf")], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                                        stderr=subprocess.DEVNULL, pass_fds=(sink.fileno(),),
                                        env={**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSRUN_FD": str(sink.fileno())})
        sink.close()
        self.reader = open(self.events)
        self.ended = 0

    def stop(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=5)
        except Exception:
            self.process.kill()
        shutil.rmtree(self.work, ignore_errors=True)

    def run(self, script):
        """Send one job and wait for its end event; that event, or None when the machine died."""
        self.process.stdin.write(script.encode())
        self.process.stdin.flush()
        deadline = time.monotonic() + JOB_SECONDS + 60
        buffer = ""
        while time.monotonic() < deadline:
            more = self.reader.read()
            buffer += more
            lines = buffer.split("\n")
            buffer = lines.pop()  # what is after the last newline is not a whole line yet
            for line in lines:
                if line.startswith('{"ev":"end"'):
                    return json.loads(line)
            if not more:
                if self.process.poll() is not None:
                    return None
                time.sleep(0.01)
        self.process.kill()
        return None


def job_script(bc_dir, source, root, machine):
    """The job's text, its directory and the file BC's output lands in."""
    machine.jobs += 1
    name = f"J{machine.jobs}"
    head = [f":ms 600000", f":wall {JOB_SECONDS}", ":nosymbols"]
    exe = compiler(bc_dir)[1]
    if root:
        rel = source.resolve().relative_to(root.resolve())
        folder = "\\".join(rel.parts[:-1])
        out = machine.work / f"{name}.TXT"
        lines = [f"mount c {root}", f"mount d {machine.work}", f"mount v {bc_dir}", "c:", f"cd \\{folder}", rf"{exe} {rel.name},NUL,NUL /O{switches(source)}; > D:\{name}.TXT"]
    else:
        job = machine.work / name
        job.mkdir()
        for sibling in source.parent.iterdir():  # $INCLUDE files beside the source
            if sibling.suffix.lower() in INCLUDES and sibling.is_file():
                # BC reads CRLF text; an include with bare LFs is read wrong
                (job / sibling.name.upper()).write_bytes(dosbatch.crlf(sibling.read_bytes().replace(b"\x1a", b"")))
        (job / "T.BAS").write_bytes(dosbatch.crlf(source.read_bytes().replace(b"\x1a", b"")))
        out = job / "O.TXT"
        lines = [f"mount c {machine.work}", f"mount v {bc_dir}", "c:", f"cd {name}", rf"{exe} T.BAS /O{switches(source)}; > O.TXT"]
    return "\n".join([*head, *lines, ".", ""]), out


def probe(tasks, root=None, progress=lambda n: None):
    """{(bc_dir, source): verdict} for every pair, run on a pool of machines sharing one queue."""
    work = queue.Queue()
    for task in tasks:
        work.put(task)
    results, lock = {}, threading.Lock()

    def worker():
        machine = Machine()
        while True:
            try:
                bc_dir, source = work.get_nowait()
            except queue.Empty:
                break
            script, out = job_script(bc_dir, source, root, machine)
            end = machine.run(script)
            if end is None:  # the machine died or hung: this pair is a timeout, and a fresh machine takes over
                answer = "timeout"
                machine.stop()
                machine = Machine()
            elif end.get("reason") != "exit" or not out.exists():
                answer = "timeout"
            else:
                text = out.read_bytes().decode("latin-1")
                if os.environ.get("BCTIME"):
                    print(f"{compiler(bc_dir)[0]} {source.name}: emulated_ms={end.get('ms')} cpu_ms={end.get('cpu_ms')}", file=sys.stderr)
                answer = verdict(text) if not os.environ.get('BCRAW') else text
            with lock:
                results[(bc_dir, source)] = answer
            progress(1)
        machine.stop()

    threads = [threading.Thread(target=worker) for _ in range(min(len(tasks), 4 * (os.cpu_count() or 4)))]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    return results


def main():
    args = sys.argv[1:]
    root = None
    if args[:1] == ["--root"]:
        root, args = Path(args[1]), args[2:]
    bc_dirs, files = [Path(d) for d in args[0].split(",")], [Path(f) for f in args[1:]]
    tasks = [(d, f) for f in files for d in bc_dirs]
    with tqdm(total=len(tasks), unit="job", desc="BC", file=sys.stderr) as bar:
        results = probe(tasks, root, bar.update)
    for d, f in tasks:
        print(f"{compiler(d)[0]}\t{f}: {results[(d, f)]}".rstrip())


if __name__ == "__main__":
    main()
