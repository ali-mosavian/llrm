---
architecture-index: Pre-RA MachineCSE: row losses
architecture-order: 520
---
# Pre-RA MachineCSE: row losses (2026-10-19)

Base: phi-placement merged with feat/parity (9dc9d6d0c, #2954). 820 C/QB/Nib objects, m16/m32, O2/Os. 52 differ.

-Os bytes: none larger (15 smaller, -177 B). Operand-free defs are not shared (DEVIATE CSE2), which removed the earlier -Os rows and grep.

Method `est`: executed instructions from llrm's own cost estimate (`--dump` cost; heuristic odds, understates hot loops). Method `real`: callgrind on the native m32 run.

## Executed instructions, more (est)

1. `bench/jpeg_fixed/jpeg_fixed.c` (-O2 -m16 -march=i386) est: 4004739 to 4013054 (+0.21%)
2. `bench/jpeg_single/jpeg_single.c` (-O2 -m16 -march=i386) est: 4002530 to 4010752 (+0.21%)
3. `tests/run/c/idiom_edges.c` (-O2 -m16 -march=i386) est: 7734 to 7738 (+0.05%)
4. `tests/run/c/bench_io.c` (-O2 -m32 -march=i486) est: 2010 to 2040 (+1.49%)
5. `tests/run/c/idiom_edges.c` (-O2 -m32 -march=i486) est: 8052 to 8058 (+0.07%)
6. `tests/run/c/idioms.c` (-O2 -m32 -march=i486) est: 12350 to 12372 (+0.18%)
7. `tests/run/c/bench_io.c` (-O3 -m32 -march=i486) est: 2010 to 2040 (+1.49%)
8. `tests/run/c/idioms.c` (-O3 -m32 -march=i486) est: 14751 to 14754 (+0.02%)

## Real run

- `bench/grep/grep.c` -O2 -m32, callgrind self Ir of bench_grep, output equal to grep.out: base 184,292,653, head 184,292,653 (before the fix: 224,670,826, +21.9%; estimate +7.2%).

- `bench/grep/grep.c` -O2 m16 (bench.py, DOSBox cycles): base 276,458,982, head 276,458,788 (before the fix +3.7% kernel time).
- `bench/jpeg_fixed/jpeg_fixed.c` -O2 m16 (bench.py): kernel 345.7 ms to 347.0 ms (+0.4%), est +0.2% (row 1).
- `bench/jpeg_single/jpeg_single.c` -O2 m16 (bench.py --time, m16-narrow-66h-ops): 16,476,908 to 16,475,093 cycles (-0.01%), code 3870 to 3854 B.
