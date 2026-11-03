#!/usr/bin/env python3
"""basevec.py MODE < vec.inc > vec_s.inc: the records for the stub run. A stub `ret` leaves the first operand in HL (low word) and DE (high word), so
each record expects exactly that and the driver takes its success path, as in a library run. MODE 4: drv.s compares DE then HL; MODE 2: drv16.s compares DE only.
Without it the stub run failed every record and paid the driver's report code (~100 T-states a call), so the library cycles came out that much low."""
import re, sys
mode = int(sys.argv[1])
for line in sys.stdin:
    m = re.match(r'(\s*\.word\s+)(.*)$', line)
    if not m: sys.stdout.write(line); continue
    w = [int(x, 16) for x in m.group(2).split(',')]
    w[4], w[5] = (w[1], w[0]) if mode == 4 else (w[1], 0)
    sys.stdout.write(m.group(1) + ','.join('0x%04x' % x for x in w) + '\n')
