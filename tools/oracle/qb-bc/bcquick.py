#!/usr/bin/env python3
"""Ask the three real BCs about a few lines: bcquick.py 'LINE' ... (or lines on stdin); each line is checked on its own in the prologue's module."""
import os, tempfile, re, sys
from pathlib import Path

import bcprobe

HERE = Path(__file__).parent
CORPUS = HERE.parents[2] / "crates/frontends/qb/lalrspike/corpus"
TOOLS = Path(os.environ['LLRM_PRIVATE_TOOLS'])


def run(lines, prologue=True):
    """{family: {line index: [messages]}}, every line compiled alone."""
    pro = (CORPUS / 'prologue.bas').read_text() if prologue else ''
    epi = (CORPUS / 'epilogue.bas').read_text() if prologue else ''
    path = Path(tempfile.mkdtemp()) / 'q.bas'
    m = bcprobe.Machine()
    result = {fam: {} for fam in ('qb45', 'pds71', 'vbdos')}
    for i, line in enumerate(lines):
        path.write_text(pro + f"{line}\n" + epi)
        for fam in result:
            script, out = bcprobe.job_script(TOOLS / fam, path, None, m)
            m.run(script)
            found, _ = bcprobe.errors(out.read_bytes().decode('latin-1'))
            if found:
                result[fam][i] = [message.replace('\x07', '').strip() for _, _, _, message in found]
    m.stop()
    return result


if __name__ == '__main__':
    lines = sys.argv[1:] or [l.rstrip('\n') for l in sys.stdin if l.strip()]
    r = run(lines)
    for i, l in enumerate(lines):
        print(f"{l:44}" + "  ".join(f"{fam}:{'ok' if i not in r[fam] else '/'.join(r[fam][i])}" for fam in r))
