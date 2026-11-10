# m68k executed clocks and bytes: ours against the best of gcc and clang (2026-11-10 20:45Z, m68k-b)

TL;DR: 1112 cells (139 tests/run/c programs x 68000, 68020 x O1 O2 O3 Os). Clocks only: ours better 488, equal 40, worse than the best external 477, UNMEASURED 107. With bytes (verdict of tools/m68k_worse_cells.py, committed rates): 477 clock-worse cells split 403 WORSE / 73 ok (bytes pay) / 1 VIRTUAL-OK; 127 more cells are WORSE on bytes with clocks better or equal.

Replaces the 19:35Z card: the 68000 column is re-priced with the post-#8930 pricer (register shifts at the places the trace set; 466 of 1668 68000 totals moved, up to +43%; the 68020 needs no change, the shift term is 68000/010 only). Table file: m68k-clocks-bytes-2026-11-10.tsv, one row per cell.

Method: ours = release/next c98e231ab163 (llrm-cc md5 589675fe, llc ab16db62, llrm-m68k-llc 4eec43b1); pricer = release/next c35b60b16d (cycles c0f65958, 68000 column; 68020 column from the c98e build, cycles 061c05b0). One exe per cell and column (ours `llrm-cc -c`; gcc 15 `-msoft-float`; clang = pin clang into pin llc, shared external flags), linked with our compiler-rt archive and the harness stubs, `cycles --exe` with CYCLES_CPUS set to the exe's CPU. Each exe also runs under qemu and its stdout is compared to the reference: a wrong run is UNMEASURED. Bytes: code+data of the object by the matrix method (-S, relaxing branches, gas); library bytes are not included.

## Clocks only (better / equal / worse / UNMEASURED ours / UNMEASURED no external)

| cpu level | better | equal | worse | ours | no ext |
|---|---|---|---|---|---|
| 68000 O1 | 66 | 2 | 62 | 2 | 7 |
| 68000 O2 | 65 | 7 | 59 | 3 | 5 |
| 68000 O3 | 57 | 7 | 67 | 3 | 5 |
| 68000 Os | 62 | 12 | 56 | 3 | 6 |
| 68020 O1 | 63 | 3 | 53 | 7 | 13 |
| 68020 O2 | 69 | 3 | 49 | 7 | 11 |
| 68020 O3 | 62 | 4 | 57 | 7 | 9 |
| 68020 Os | 44 | 2 | 74 | 7 | 12 |

## Clocks and bytes (verdict per level)

| cpu level | WORSE | ok | VIRTUAL-OK |
|---|---|---|---|
| 68000 O1 | 73 | 55 | 7 |
| 68000 O2 | 60 | 75 | 0 |
| 68000 O3 | 69 | 66 | 0 |
| 68000 Os | 52 | 83 | 0 |
| 68020 O1 | 80 | 40 | 15 |
| 68020 O2 | 64 | 71 | 0 |
| 68020 O3 | 68 | 67 | 0 |
| 68020 Os | 64 | 71 | 0 |

Each level has 135 cells with numbers (4 have no byte column).

## The 107 UNMEASURED cells, by cause

| cells | column | cause |
|---|---|---|
| 25 | no external | gcc and clang both failed to compile |
| 16 | ours | timing-table gap (below) |
| 16 | no external | table gap in one, wrong output in the other |
| 12 | no external | gcc long double ABI mismatch, clang wrong output |
| 11 | ours | cycles timeout (300 s; memfuncs_runtime and shl_of_a_zero_extended_dividend; load-dependent) |
| 8 | no external | gcc flags with no gcc meaning, clang wrong output |
| 8 | ours | wrong output (os_console, all levels, both CPUs: stdin/args in this harness) |
| 7 | no external | compile failure and a table gap |
| 4 | ours | cycles failed |

## 68020 timing-table gaps (exes = cell x column)

| instruction | exes |
|---|---|
| `trap #0` (harness syscall; same count in every column, cells with only this are kept as a common lower bound) | 1502 |
| `mulu.l`/`muls.l #imm` | 73 |
| full-extension indexed `(addr,,%dN)` | 38 |
| `cmpi.l`/`cmpi.b #imm,(0,%pc)` | 21 |

## Largest gap

float_in_memory_loop: ours 3.54 M clocks (68000 O2), clang 1798, gcc 5128. The loop adds 16 x 0.5 in memory 100 times; both externals fold it, ours runs 1600 soft-float adds. No cause card yet, no owner.
