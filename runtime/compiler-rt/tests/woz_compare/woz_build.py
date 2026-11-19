"""Builds the original Rankin/Woz machine code from its listing (read at run time, never stored in the repo) into a sim65
image with the driver of woz_driver.s, and runs it on a table of records."""
import os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
LISTING = os.environ.get('WOZ_LISTING', os.path.expanduser('~/scratch/hand-sources/site/floats/wozfp3.txt'))
ENTRY = {'add': 0xF46E, 'sub': 0xF468, 'mul': 0xF48C, 'div': 0xF4B2, 'i2f': 0xF451, 'f2i': 0xF640}

def sh(*a, **k):
    r = subprocess.run(a, capture_output=True, **k)
    if r.returncode and not k.get('ok'):
        sys.exit('FAILED: %s\n%s' % (' '.join(a), (r.stdout + r.stderr)[-800:].decode(errors='replace')))
    return r

def instructions():
    ins = {}
    for ln in open(LISTING):
        m = re.match(r'^([0-9A-F]{4}): ((?:[0-9A-F]{2} ?)+)', ln)
        if m: ins[int(m.group(1), 16)] = [int(x, 16) for x in m.group(2).split()]
    return ins

def prepare(T):
    ins = instructions(); secs = {}
    for a in sorted(ins): secs.setdefault('WOZA' if a < 0xF600 else 'WOZB', []).append((a, ins[a]))
    with open(f'{T}/woz_code.s', 'w') as f:
        for k, lst in secs.items():
            f.write('.segment "%s"\n' % k)
            for a, b in lst: f.write('.byte ' + ','.join(str(x) for x in b) + '\n')
    cfg = open('/usr/share/cc65/cfg/sim6502.cfg').read()
    cfg = cfg.replace('size = $FDF0 - __STACKSIZE__', 'size = $F000 - $0200 - __STACKSIZE__')
    cfg = cfg.replace('MEMORY {', 'MEMORY {\n    WA: file = "", start = $F425, size = $D7;\n    WB: file = "", start = $F63D, size = $21;', 1)
    cfg = cfg.replace('SEGMENTS {', 'SEGMENTS {\n    WOZA: load = MAIN, run = WA, type = rw, define = yes;\n    WOZB: load = MAIN, run = WB, type = rw, define = yes;', 1)
    open(f'{T}/woz.cfg', 'w').write(cfg)
    sh('ca65', '-t', 'sim6502', '-o', f'{T}/code.o', f'{T}/woz_code.s')

def run(T, op, records, defs=()):
    """Chunks of 250 (the driver counts records in one byte)."""
    return b"".join(run1(T, op, records[i:i + 250], defs) for i in range(0, len(records), 250))

def run1(T, op, records, defs=()):
    """records: list of 8-byte lists (X2 M2 X1 M1). Returns sim65's stdout bytes (4 bytes per record) for -D DUMP."""
    n = len(records)
    open(f'{T}/woz_equ.inc', 'w').write('SIGN=$F3\nX2=$F4\nM2=$F5\nX1=$F8\nM1=$F9\nfpregs=$F4\nNREC=%d\ntarget=$%04X\n' % (n, ENTRY[op]))
    with open(f'{T}/woz_tab.inc', 'w') as f:
        for r in records: f.write('.byte ' + ','.join(str(x) for x in r) + '\n')
    sh('ca65', '-t', 'sim6502', '-D', 'DUMP', *defs, '-I', T, '-o', f'{T}/d_dump.o', f'{HERE}/woz_driver.s')
    sh('ld65', '-C', f'{T}/woz.cfg', '-o', f'{T}/w_dump.bin', f'{T}/d_dump.o', f'{T}/code.o', '/usr/share/cc65/lib/sim6502.lib')
    return subprocess.run(['sim65', '-x', '2000000000', f'{T}/w_dump.bin'], capture_output=True).stdout
