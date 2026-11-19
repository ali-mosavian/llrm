"""Runs a C program on the m16 target in the emulator and logs the reads of named globals in the order the program makes them.

    names, output = accesses(source_text, ["g1", "g2", "g3"], "-O2")

A volatile read the compiler reorders, merges or drops shows as a different log than the source order.
"""

import re
import subprocess
import tempfile
from pathlib import Path

import unicorn

import icount
import dosbatch
from dosbatch import BIN


def accesses(source: str, names: list[str], level: str) -> tuple[list[str], bytes]:
    """(the global names read, consecutive repeats of one name collapsed; the program's output) for `source` built at `level`."""
    with tempfile.TemporaryDirectory() as temp:
        work = Path(temp)
        (work / "t.c").write_text(source)
        command, env = dosbatch.c_compile(work / "t.c", work / "t.obj", dosbatch.REAL_MODE, level, None, [], ["-march=i386"])
        done = subprocess.run(command, env={**__import__("os").environ, **env}, capture_output=True, text=True)
        if done.returncode != 0:
            raise RuntimeError(done.stderr.strip()[-400:])
        listing = work / "t.map"
        dosbatch.link_c(work / "t.obj", work / "t.exe", work, listing)
        text = listing.read_text()
        where = {}
        for name in names:
            found = re.search(rf"^([0-9A-F]{{4}}):([0-9A-F]{{4}})\*?\s+_{name}\s*$", text, re.M | re.I)
            where[name] = (int(found.group(1), 16), int(found.group(2), 16))
        log: list[str] = []
        make = icount.Uc

        def traced(*args):
            uc = make(*args)
            for name, (segment, offset) in where.items():
                linear = (icount.LOAD + segment) * 16 + offset
                uc.hook_add(unicorn.UC_HOOK_MEM_READ, lambda _uc, _kind, _address, _size, _value, who: log.append(who), name, linear, linear)
            return uc

        icount.Uc = traced
        try:
            result = icount.run(work / "t.exe", None, work, whole=True)
        finally:
            icount.Uc = make
        if result.error:
            raise RuntimeError(result.error)
        collapsed = [one for at, one in enumerate(log) if at == 0 or log[at - 1] != one]
        return collapsed, result.output.strip()
