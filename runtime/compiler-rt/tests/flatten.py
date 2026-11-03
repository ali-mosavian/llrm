#!/usr/bin/env python3
"""flatten.py FILE [SUFFIX]: FILE with its local #include "..." lines inlined, recursively. The flatteners (x86_*kernels.sh, x86_m16_soft_*.sh)
concatenate sources into one freestanding file. Includes that name no file beside the includer (softfloat.h, sf_helpers.h, ...) stay for the caller;
SUFFIX is appended to every static function of the file, so files that define the same helper can share a translation unit."""
import os, re, sys

def flat(path):
    out = []
    for line in open(path).read().split('\n'):
        m = re.match(r'#include "([^"]+)"', line)
        sub = os.path.normpath(os.path.join(os.path.dirname(path), m.group(1))) if m else None
        out.append(flat(sub) if sub and os.path.isfile(sub) else line)
    return '\n'.join(out)

def flatten(path, suffix=''):
    s = flat(path)
    if suffix:
        for name in set(re.findall(r'^static (?:inline )?[\w\s\*]*?\b(\w+)\(', s, re.M)):
            s = re.sub(r'\b%s\b' % name, name + suffix, s)
    return s

if __name__ == '__main__':
    print(flatten(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else ''))
