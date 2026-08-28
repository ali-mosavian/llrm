#!/usr/bin/env python3
"""
Tests for the OMF reader, against objects BC actually produced.

    python3 tests/test_omf.py

The fixtures in fixtures/omf are real BC output, one per configuration that
emits a different shape, plus one module with an ON GOTO and a SELECT CASE
because those are where the intra-segment references live. They run on the
host in milliseconds, which is the point of moving this work off the target.

The first test is the one that matters: read a file and write it back, and
require the bytes to be identical. A pass that means to move code has to be
trusted not to disturb what it is not changing, and nothing else here is
worth anything until that holds.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from qbopt import omf

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIX = os.path.join(HERE, 'fixtures', 'omf')
bad = 0


def check(cond, what):
    global bad
    if not cond:
        print('FAIL %s' % what)
        bad += 1


def fixtures():
    return sorted(f for f in os.listdir(FIX) if f.endswith('.obj'))


print('-- round trip')
for f in fixtures():
    p = os.path.join(FIX, f)
    d = open(p, 'rb').read()
    recs = omf.parse(d)
    check(b''.join(r.emit() for r in recs) == d, '%s is not byte-identical' % f)
    check(len(recs) > 10, '%s decoded only %d records' % (f, len(recs)))

print('-- the module code segment is found')
for f in fixtures():
    recs = omf.read(os.path.join(FIX, f))
    segs = omf.segments(recs)
    code = [s for s in segs[1:] if s and s[0].endswith('_CODE')]
    check(code and code[0][1] > 0, '%s has no non-empty _CODE segment' % f)

print('-- the runtime calls are named, not guessed at')
# B$CPI4 is the long compare; every configuration reaches it through an
# EXTDEF, which is the whole reason this is easier here than at run time.
for f in ('vbdos-g3.obj', 'vbdos-g2.obj', 'pds-g2.obj', 'qb45.obj'):
    recs = omf.read(os.path.join(FIX, f))
    exts = omf.externals(recs)
    named = [omf.LOCNAME.get(x.loc, x.loc) for x in omf.fixups(recs)
             if x.target == 'external' and exts[x.index] == 'B$CPI4']
    check(named == ['ptr16:16'], '%s: B$CPI4 fixups are %r' % (f, named))

print('-- threads are resolved')
# BC leans on THREAD subrecords: 34 of the 40 fixups in the jump-table
# module refer to a thread rather than naming their target. Anything that
# means to move code has to resolve them or it cannot see most of the
# relocations at all.
recs = omf.read(os.path.join(FIX, 'jumptable.obj'))
fx = omf.fixups(recs)
check(len(fx) == 40, 'jumptable has %d fixups, expected 40' % len(fx))
check(not any(x.target == 'thread' for x in fx), 'some targets left unresolved')

print('-- an ON GOTO table is relocated, so code may be moved')
# The three labels of "ON k GOTO L1, L2, L3" appear as three consecutive
# offset16 fixups into the module's own code segment. That they are fixups
# at all is what makes moving code tractable: BC does not bake intra-segment
# offsets into the code where a rewriter could not see them.
segs = omf.segments(recs)
code_i = next(i for i, s in enumerate(segs) if s and s[0] == 'JT_CODE')
tbl = sorted(x.offset for x in fx
             if x.target == 'segment' and x.index == code_i
             and x.loc == omf.LOC_OFF16)
check(tbl == [0x0A, 0x40, 0x42, 0x44], 'code self-references at %s' % [hex(t) for t in tbl])

print()
print('ALL PASS' if bad == 0 else 'FAILURES %d' % bad)
sys.exit(1 if bad else 0)
