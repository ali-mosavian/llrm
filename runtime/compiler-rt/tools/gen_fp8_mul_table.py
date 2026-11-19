#!/usr/bin/env python3
"""Writes a ca65 include with the 128 x 128 FP8 multiply table: magnitude code of |a| * |b| (round to nearest even,
inf and NaN included) at index |a| * 128 + |b|. Usage: gen_fp8_mul_table.py [OUT]; FP8_M picks the width."""
import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'tests', 'asm6502'))
os.environ.setdefault('FP8_M', '2')
import gen8
gen8.op = 'mul'
out = open(sys.argv[1], 'w') if len(sys.argv) > 1 else sys.stdout
for a in range(128):
    out.write('.byte ' + ','.join(str(gen8.ref(a, b) & 0x7f) for b in range(128)) + '\n')
